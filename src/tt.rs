/// Transposition Table — CHPawn-FrozenKing v1.0
/// Per DECISIONS.md DD04 and frozen/spec.md:
///   10-byte entries, 3 per 32-byte cluster, depth+age hybrid replacement.
///   Key: 16-bit truncated Zobrist. Flags: packed age(5)+pv(1)+bound(2).

/// Bound type for TT entries.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum Bound {
    None = 0,
    Upper = 1,  // score <= actual (failed low)
    Lower = 2,  // score >= actual (failed high / beta cutoff)
    Exact = 3,  // exact score from PV node
}

impl Bound {
    fn from_u8(v: u8) -> Self {
        match v & 0x03 {
            0 => Bound::None,
            1 => Bound::Upper,
            2 => Bound::Lower,
            3 => Bound::Exact,
            _ => unreachable!(),
        }
    }

    fn flag_bonus(self) -> i32 {
        match self {
            Bound::Exact => 3,
            Bound::Lower => 2,
            Bound::Upper => 1,
            Bound::None => 0,
        }
    }
}

/// TT entry — exactly 10 bytes per frozen spec.
#[derive(Clone, Copy)]
#[repr(C)]
pub struct TTEntry {
    pub key: u16,    // 2 bytes — truncated Zobrist
    pub mv: u16,     // 2 bytes — packed move (0 = no move)
    pub score: i16,  // 2 bytes — centipawn score
    pub eval: i16,   // 2 bytes — static eval
    pub depth: u8,   // 1 byte — search depth
    flags: u8,       // 1 byte — packed: age(5) + pv(1) + bound(2)
}

const _ASSERT_ENTRY_SIZE: () = assert!(std::mem::size_of::<TTEntry>() == 10);

impl TTEntry {
    const EMPTY: Self = TTEntry {
        key: 0,
        mv: 0,
        score: 0,
        eval: 0,
        depth: 0,
        flags: 0,
    };

    pub fn bound(&self) -> Bound {
        Bound::from_u8(self.flags & 0x03)
    }

    pub fn is_pv(&self) -> bool {
        (self.flags >> 2) & 1 != 0
    }

    pub fn age(&self) -> u8 {
        self.flags >> 3
    }

    fn set_flags(&mut self, bound: Bound, pv: bool, age: u8) {
        self.flags = (age << 3) | ((pv as u8) << 2) | (bound as u8);
    }

    fn is_empty(&self) -> bool {
        self.key == 0 && self.mv == 0 && self.depth == 0 && self.flags == 0
    }

    fn priority(&self, current_age: u8) -> i32 {
        let age_diff = age_difference(current_age, self.age()) as i32;
        let fb = self.bound().flag_bonus();
        let pv = if self.is_pv() { 1 } else { 0 };
        self.depth as i32 + fb + (age_diff * age_diff) / 4 + pv
    }
}

fn age_difference(current: u8, entry: u8) -> u8 {
    // Ages wrap at 32 (5 bits)
    (current.wrapping_sub(entry)) & 0x1F
}

/// Cluster of 3 entries — 32 bytes with padding.
#[repr(C, align(32))]
struct TTCluster {
    entries: [TTEntry; 3],
    _padding: [u8; 2],
}

const _ASSERT_CLUSTER_SIZE: () = assert!(std::mem::size_of::<TTCluster>() == 32);

impl TTCluster {
    const EMPTY: Self = TTCluster {
        entries: [TTEntry::EMPTY; 3],
        _padding: [0; 2],
    };
}

/// Transposition table with depth+age hybrid replacement.
pub struct TranspositionTable {
    clusters: Vec<TTCluster>,
    age: u8,
}

impl TranspositionTable {
    /// Create a new TT with the given size in MB. Default: 64 MB.
    pub fn new(mb: usize) -> Self {
        let num_clusters = mb * 1024 * 1024 / std::mem::size_of::<TTCluster>();
        let num_clusters = num_clusters.max(1);
        TranspositionTable {
            clusters: Self::alloc_clusters(num_clusters),
            age: 0,
        }
    }

    fn alloc_clusters(n: usize) -> Vec<TTCluster> {
        let mut v = Vec::with_capacity(n);
        v.resize_with(n, || TTCluster::EMPTY);
        v
    }

    fn cluster_index(&self, key: u64) -> usize {
        // Fixed-point multiplication for uniform distribution
        let len = self.clusters.len() as u64;
        ((key as u128 * len as u128) >> 64) as usize
    }

    /// Probe the TT for a position. Returns a copy of the entry if found.
    pub fn probe(&self, key: u64) -> Option<TTEntry> {
        let idx = self.cluster_index(key);
        let truncated = key as u16;
        let cluster = &self.clusters[idx];
        for entry in &cluster.entries {
            if entry.key == truncated && !entry.is_empty() {
                return Some(*entry);
            }
        }
        None
    }

    /// Store an entry in the TT using depth+age hybrid replacement.
    /// Per DECISIONS.md DD04:
    ///   priority = depth + flag_bonus + age_diff^2/4 + pv_bonus
    ///   Replace when: different key OR (new is Exact AND old is not)
    ///                 OR new_priority * 3 >= old_priority * 2
    pub fn store(
        &mut self,
        key: u64,
        depth: u8,
        score: i16,
        eval: i16,
        bound: Bound,
        mv: u16,
        pv: bool,
    ) {
        let idx = self.cluster_index(key);
        let truncated = key as u16;

        let cluster = &mut self.clusters[idx];

        // Find best replacement candidate
        let mut replace_idx = 0;
        let mut worst_priority = i32::MAX;

        for (i, entry) in cluster.entries.iter().enumerate() {
            // Exact key match — always replace this slot
            if entry.key == truncated {
                replace_idx = i;
                break;
            }

            // Empty slot — use it immediately
            if entry.is_empty() {
                replace_idx = i;
                break;
            }

            // Track lowest-priority entry for potential replacement
            let p = entry.priority(self.age);
            if p < worst_priority {
                worst_priority = p;
                replace_idx = i;
            }
        }

        let old = &cluster.entries[replace_idx];

        // Check replacement conditions (skip for key match or empty)
        if !old.is_empty() && old.key != truncated {
            let new_priority = depth as i32
                + bound.flag_bonus()
                + if pv { 1 } else { 0 };
            let old_priority = old.priority(self.age);

            // Replace when: new is Exact AND old is not
            //           OR  new_priority * 3 >= old_priority * 2
            let should_replace =
                (bound == Bound::Exact && old.bound() != Bound::Exact)
                || (new_priority * 3 >= old_priority * 2);

            if !should_replace {
                return;
            }
        }

        // Preserve existing move if new entry has no move
        let final_mv = if mv != 0 { mv } else { old.mv };

        let entry = &mut cluster.entries[replace_idx];
        entry.key = truncated;
        entry.mv = final_mv;
        entry.score = score;
        entry.eval = eval;
        entry.depth = depth;
        entry.set_flags(bound, pv, self.age);
    }

    /// Resize the TT to the given size in MB.
    pub fn resize(&mut self, mb: usize) {
        let num_clusters = (mb * 1024 * 1024 / std::mem::size_of::<TTCluster>()).max(1);
        self.clusters = Self::alloc_clusters(num_clusters);
        self.age = 0;
    }

    /// Clear all entries.
    pub fn clear(&mut self) {
        for cluster in &mut self.clusters {
            *cluster = TTCluster::EMPTY;
        }
        self.age = 0;
    }

    /// Increment the age counter (wraps at 32 for 5-bit field).
    pub fn increment_age(&mut self) {
        self.age = (self.age + 1) & 0x1F;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entry_size_is_10_bytes() {
        assert_eq!(std::mem::size_of::<TTEntry>(), 10);
    }

    #[test]
    fn cluster_size_is_32_bytes() {
        assert_eq!(std::mem::size_of::<TTCluster>(), 32);
    }

    #[test]
    fn store_and_retrieve() {
        let mut tt = TranspositionTable::new(1);
        let key: u64 = 0xDEADBEEF12345678;
        tt.store(key, 5, 150, 100, Bound::Exact, 0x1234, true);
        let entry = tt.probe(key).expect("Should find stored entry");
        assert_eq!(entry.key, key as u16);
        assert_eq!(entry.depth, 5);
        assert_eq!(entry.score, 150);
        assert_eq!(entry.eval, 100);
        assert_eq!(entry.mv, 0x1234);
        assert_eq!(entry.bound(), Bound::Exact);
        assert!(entry.is_pv());
    }

    #[test]
    fn deeper_entry_survives_shallow_overwrite() {
        let mut tt = TranspositionTable::new(1);
        let key: u64 = 0xABCD000000000000;
        // Store deep entry
        tt.store(key, 10, 200, 180, Bound::Exact, 0x1111, true);
        // Try to overwrite with shallow entry at different key mapping to same cluster
        // But with same key, it should overwrite (key match always replaces)
        // Test with different key in same cluster by filling all 3 slots:
        let key2: u64 = 0x1111000000000000;
        let key3: u64 = 0x2222000000000000;
        tt.store(key2, 1, 50, 40, Bound::Upper, 0x2222, false);
        tt.store(key3, 1, 50, 40, Bound::Upper, 0x3333, false);
        // Now try to store a shallow entry that should NOT replace the deep one
        let key4: u64 = 0x3333000000000000;
        tt.store(key4, 2, 60, 50, Bound::Upper, 0x4444, false);
        // Deep entry should still be retrievable
        let entry = tt.probe(key).expect("Deep entry should survive");
        assert_eq!(entry.depth, 10);
    }

    #[test]
    fn resize_changes_capacity() {
        let mut tt = TranspositionTable::new(1);
        let old_count = tt.clusters.len();
        tt.resize(2);
        let new_count = tt.clusters.len();
        assert!(new_count > old_count, "Resize to 2MB should have more clusters than 1MB");
    }

    #[test]
    fn clear_removes_all_entries() {
        let mut tt = TranspositionTable::new(1);
        tt.store(0xAAAA, 5, 100, 90, Bound::Exact, 0x1234, true);
        tt.store(0xBBBB, 3, 50, 40, Bound::Lower, 0x5678, false);
        tt.clear();
        assert!(tt.probe(0xAAAA).is_none());
        assert!(tt.probe(0xBBBB).is_none());
    }

    #[test]
    fn default_64mb_fits_in_memory() {
        let tt = TranspositionTable::new(64);
        let expected_clusters = 64 * 1024 * 1024 / 32;
        assert_eq!(tt.clusters.len(), expected_clusters);
    }

    #[test]
    fn probe_returns_none_on_miss() {
        let tt = TranspositionTable::new(1);
        assert!(tt.probe(0x12345678).is_none());
    }

    #[test]
    fn age_wraps_at_32() {
        let mut tt = TranspositionTable::new(1);
        for _ in 0..32 {
            tt.increment_age();
        }
        assert_eq!(tt.age, 0, "Age should wrap at 32");
    }

    #[test]
    fn bound_flags_pack_correctly() {
        let mut entry = TTEntry::EMPTY;
        entry.set_flags(Bound::Exact, true, 15);
        assert_eq!(entry.bound(), Bound::Exact);
        assert!(entry.is_pv());
        assert_eq!(entry.age(), 15);

        entry.set_flags(Bound::Lower, false, 31);
        assert_eq!(entry.bound(), Bound::Lower);
        assert!(!entry.is_pv());
        assert_eq!(entry.age(), 31);
    }

    #[test]
    fn replacement_priority_formula() {
        // priority = depth + flag_bonus + age_diff^2/4 + pv_bonus
        let mut entry = TTEntry::EMPTY;
        entry.depth = 8;
        entry.set_flags(Bound::Exact, true, 0);
        // age_diff = 0, flag_bonus = 3, pv = 1
        // priority = 8 + 3 + 0 + 1 = 12
        assert_eq!(entry.priority(0), 12);

        // With age difference of 4:
        // priority = 8 + 3 + 16/4 + 1 = 16
        assert_eq!(entry.priority(4), 16);
    }
}
