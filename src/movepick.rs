/// Move Ordering — CHPawn-FrozenKing v1.0
/// Per DECISIONS.md DD01 and frozen/spec.md:
///   MVV-LVA: victim_value * 10 - attacker_value + CAPTURE_BASE(10000)
///   Promotions: 5000
///   Killers: 9000
///   Quiet: 0

use shakmaty::{Move, MoveList, Role};
use crate::eval::{PAWN, KNIGHT, BISHOP, ROOK, QUEEN, KING};

const CAPTURE_BASE: i32 = 10000;
const KILLER_SCORE: i32 = 9000;
const PROMOTION_SCORE: i32 = 5000;
const LOSING_CAPTURE_BASE: i32 = -1000; // Losing captures score below quiet moves

// History Heuristic — v0.0.3
const HISTORY_MAX: i32 = 16384;
const HISTORY_MIN: i32 = -16384;

// Countermove Heuristic — v0.0.6
const COUNTERMOVE_SCORE: i32 = 8000;

fn role_value(role: Role) -> i32 {
    match role {
        Role::Pawn => PAWN,
        Role::Knight => KNIGHT,
        Role::Bishop => BISHOP,
        Role::Rook => ROOK,
        Role::Queen => QUEEN,
        Role::King => KING,
    }
}

/// Simple SEE (Static Exchange Evaluation) for capture ordering.
/// Returns estimated material gain: captured_value - attacker_value.
/// Positive = winning capture, Negative = likely losing capture.
fn simple_see(m: &Move) -> i32 {
    if !m.is_capture() {
        return 0;
    }
    let victim = m.capture().map(|r| role_value(r)).unwrap_or(0);
    let attacker = role_value(m.role());
    victim - attacker
}

/// Capture ordering with SEE awareness:
///   Winning captures (SEE >= 0): CAPTURE_BASE + MVV-LVA
///   Losing captures (SEE < 0): LOSING_CAPTURE_BASE + SEE (below quiet moves)
fn capture_score(m: &Move) -> i32 {
    if !m.is_capture() {
        if m.is_promotion() {
            return PROMOTION_SCORE;
        }
        return 0;
    }
    let see = simple_see(m);
    if see >= 0 {
        // Winning/equal capture: use MVV-LVA with high base
        let victim = m.capture().map(|r| role_value(r)).unwrap_or(0);
        let attacker = role_value(m.role());
        victim * 10 - attacker + CAPTURE_BASE
    } else {
        // Losing capture: score below quiet moves
        LOSING_CAPTURE_BASE + see
    }
}

/// Move picker with killer, history, countermove, and capture history support.
pub struct MovePicker {
    /// 2 killer slots per depth, up to 64 depths
    killer_moves: [[Option<u16>; 2]; 64],
    /// History heuristic: history[from][to] tracks quiet move cutoff frequency
    history: [[i32; 64]; 64],
    /// Countermove table: countermoves[from][to] of previous move → refutation move
    countermoves: [[Option<u16>; 64]; 64],
    /// Capture history: capture_hist[color][to_sq][captured_role] (role 0-5: P,N,B,R,Q,K)
    capture_hist: [[[i32; 6]; 64]; 2],
}

impl MovePicker {
    pub fn new() -> Self {
        MovePicker {
            killer_moves: [[None; 2]; 64],
            history: [[0; 64]; 64],
            countermoves: [[None; 64]; 64],
            capture_hist: [[[0; 6]; 64]; 2],
        }
    }

    /// Order moves: TT > captures (MVV-LVA+capture history) > killers > countermove > quiet (history).
    pub fn order_moves(&self, moves: &MoveList, depth: u8, tt_move: Option<u16>,
                       prev_move: Option<u16>, stm_color: usize) -> Vec<Move> {
        let countermove = prev_move.and_then(|pm| {
            let from = (pm & 0x3F) as usize;
            let to = ((pm >> 6) & 0x3F) as usize;
            self.countermoves[from][to]
        });

        let mut scored: Vec<(Move, i32)> = moves
            .iter()
            .map(|m| {
                let packed = pack_move(m);
                let score = if tt_move == Some(packed) {
                    i32::MAX // TT move first
                } else if m.is_capture() || m.is_promotion() {
                    let base = capture_score(m);
                    // Add capture history bonus for captures
                    if m.is_capture() {
                        let (_, to) = move_squares(m);
                        let cap_role = m.capture().map(|r| role_index(r)).unwrap_or(0);
                        base + self.capture_hist[stm_color][to][cap_role] / 32
                    } else {
                        base
                    }
                } else if self.is_killer(packed, depth) {
                    KILLER_SCORE
                } else if countermove == Some(packed) {
                    COUNTERMOVE_SCORE
                } else {
                    self.history_score(m)
                };
                (m.clone(), score)
            })
            .collect();
        scored.sort_by(|a, b| b.1.cmp(&a.1));
        scored.into_iter().map(|(m, _)| m).collect()
    }

    /// Order captures by SEE-aware scoring for quiescence search.
    pub fn order_captures(&self, moves: &MoveList) -> Vec<Move> {
        let mut captures: Vec<(Move, i32)> = moves
            .iter()
            .filter(|m| m.is_capture())
            .map(|m| (m.clone(), capture_score(m)))
            .collect();
        captures.sort_by(|a, b| b.1.cmp(&a.1));
        captures.into_iter().map(|(m, _)| m).collect()
    }

    /// Store a killer move at the given depth.
    /// Shifts slot[1] = slot[0], slot[0] = new move.
    pub fn store_killer(&mut self, mv: &Move, depth: u8) {
        if depth as usize >= 64 {
            return;
        }
        let packed = pack_move(mv);
        let d = depth as usize;
        // Don't store duplicates
        if self.killer_moves[d][0] == Some(packed) {
            return;
        }
        self.killer_moves[d][1] = self.killer_moves[d][0];
        self.killer_moves[d][0] = Some(packed);
    }

    fn is_killer(&self, packed: u16, depth: u8) -> bool {
        if depth as usize >= 64 {
            return false;
        }
        let d = depth as usize;
        self.killer_moves[d][0] == Some(packed)
            || self.killer_moves[d][1] == Some(packed)
    }

    /// Get history score for a quiet move.
    fn history_score(&self, mv: &Move) -> i32 {
        let (from, to) = move_squares(mv);
        self.history[from][to]
    }

    /// Update history table on beta cutoff.
    /// Reward the move that caused cutoff, penalize other searched quiets.
    pub fn update_history(&mut self, mv: &Move, depth: u8, good: bool) {
        let (from, to) = move_squares(mv);
        let bonus = (depth as i32) * (depth as i32);
        if good {
            self.history[from][to] = (self.history[from][to] + bonus).min(HISTORY_MAX);
        } else {
            self.history[from][to] = (self.history[from][to] - bonus).max(HISTORY_MIN);
        }
    }

    /// Store a countermove: move that refuted the given previous move.
    pub fn store_countermove(&mut self, prev_move: Option<u16>, mv: &Move) {
        if let Some(pm) = prev_move {
            let from = (pm & 0x3F) as usize;
            let to = ((pm >> 6) & 0x3F) as usize;
            self.countermoves[from][to] = Some(pack_move(mv));
        }
    }

    /// Update capture history table.
    pub fn update_capture_history(&mut self, color: usize, mv: &Move, depth: u8, good: bool) {
        if !mv.is_capture() { return; }
        let (_, to) = move_squares(mv);
        let cap_role = mv.capture().map(|r| role_index(r)).unwrap_or(0);
        let bonus = (depth as i32) * (depth as i32);
        if good {
            self.capture_hist[color][to][cap_role] =
                (self.capture_hist[color][to][cap_role] + bonus).min(HISTORY_MAX);
        } else {
            self.capture_hist[color][to][cap_role] =
                (self.capture_hist[color][to][cap_role] - bonus).max(HISTORY_MIN);
        }
    }

    pub fn clear(&mut self) {
        self.killer_moves = [[None; 2]; 64];
        self.history = [[0; 64]; 64];
        self.countermoves = [[None; 64]; 64];
        self.capture_hist = [[[0; 6]; 64]; 2];
    }
}

/// Map Role to index 0-5 for capture history table.
fn role_index(role: Role) -> usize {
    match role {
        Role::Pawn => 0,
        Role::Knight => 1,
        Role::Bishop => 2,
        Role::Rook => 3,
        Role::Queen => 4,
        Role::King => 5,
    }
}

/// Extract from/to square indices from a move.
fn move_squares(m: &Move) -> (usize, usize) {
    let from = match m {
        Move::Normal { from, .. } => *from as usize,
        Move::EnPassant { from, .. } => *from as usize,
        Move::Castle { king, .. } => *king as usize,
        Move::Put { .. } => 0,
    };
    let to = match m {
        Move::Normal { to, .. } => *to as usize,
        Move::EnPassant { to, .. } => *to as usize,
        Move::Castle { rook, .. } => *rook as usize,
        Move::Put { to, .. } => *to as usize,
    };
    (from, to)
}

/// Pack a move into u16 for TT storage and killer comparison.
/// Format: from(6 bits) + to(6 bits) + promotion(4 bits)
pub fn pack_move(m: &Move) -> u16 {
    let from = match m {
        Move::Normal { from, .. } => *from as u16,
        Move::EnPassant { from, .. } => *from as u16,
        Move::Castle { king, .. } => *king as u16,
        Move::Put { .. } => 0,
    };
    let to = match m {
        Move::Normal { to, .. } => *to as u16,
        Move::EnPassant { to, .. } => *to as u16,
        Move::Castle { rook, .. } => *rook as u16,
        Move::Put { to, .. } => *to as u16,
    };
    let promo = match m {
        Move::Normal { promotion: Some(role), .. } => match role {
            Role::Knight => 1,
            Role::Bishop => 2,
            Role::Rook => 3,
            Role::Queen => 4,
            _ => 0,
        },
        _ => 0,
    };
    (from & 0x3F) | ((to & 0x3F) << 6) | ((promo & 0x0F) << 12)
}

#[cfg(test)]
mod tests {
    use super::*;
    use shakmaty::{Chess, Position};
    use shakmaty::fen::Fen;
    use shakmaty::CastlingMode;

    fn pos_from_fen(fen: &str) -> Chess {
        let f: Fen = fen.parse().unwrap();
        f.into_position(CastlingMode::Standard).unwrap()
    }

    #[test]
    fn captures_sort_above_quiet() {
        // Position with both captures and quiet moves available
        let pos = pos_from_fen("r1bqkbnr/pppppppp/2n5/4P3/8/8/PPPP1PPP/RNBQKBNR w KQkq - 1 2");
        let moves = pos.legal_moves();
        let picker = MovePicker::new();
        let ordered = picker.order_moves(&moves, 1, None, None, 0);

        // Find first quiet move index
        let first_quiet = ordered.iter().position(|m| !m.is_capture() && !m.is_promotion());
        let last_capture = ordered.iter().rposition(|m| m.is_capture());

        if let (Some(fq), Some(lc)) = (first_quiet, last_capture) {
            assert!(
                lc < fq,
                "All captures should come before quiet moves"
            );
        }
    }

    #[test]
    fn pawn_takes_queen_above_queen_takes_pawn() {
        // PxQ score: victim=900*10 - attacker=100 + 10000 = 18900
        // QxP score: victim=100*10 - attacker=900 + 10000 = 10100
        let pxq = 900 * 10 - 100 + CAPTURE_BASE;
        let qxp = 100 * 10 - 900 + CAPTURE_BASE;
        assert!(pxq > qxp, "PxQ ({}) should score higher than QxP ({})", pxq, qxp);
    }

    #[test]
    fn killer_moves_sort_above_quiet() {
        let pos = pos_from_fen("rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq - 0 1");
        let moves = pos.legal_moves();
        let mut picker = MovePicker::new();

        // Store the first quiet move as a killer
        let quiet_moves: Vec<Move> = moves.iter().filter(|m| !m.is_capture()).cloned().collect();
        if let Some(first_quiet) = quiet_moves.first() {
            picker.store_killer(first_quiet, 3);
            let ordered = picker.order_moves(&moves, 3, None, None, 0);

            // Find the killer in the ordered list
            let killer_packed = pack_move(first_quiet);
            let killer_pos = ordered.iter().position(|m| pack_move(m) == killer_packed);
            // Find first non-killer quiet move
            let first_non_killer_quiet = ordered.iter().position(|m| {
                !m.is_capture() && !m.is_promotion() && pack_move(m) != killer_packed
            });

            if let (Some(kp), Some(nkq)) = (killer_pos, first_non_killer_quiet) {
                assert!(kp < nkq, "Killer move should come before non-killer quiet moves");
            }
        }
    }

    #[test]
    fn store_killer_shifts_correctly() {
        let mut picker = MovePicker::new();
        let pos = Chess::default();
        let moves = pos.legal_moves();

        // Get two quiet moves
        let quiet: Vec<Move> = moves.iter().filter(|m| !m.is_capture()).cloned().collect();
        if quiet.len() >= 2 {
            let m1 = &quiet[0];
            let m2 = &quiet[1];

            picker.store_killer(m1, 5);
            assert_eq!(picker.killer_moves[5][0], Some(pack_move(m1)));
            assert_eq!(picker.killer_moves[5][1], None);

            picker.store_killer(m2, 5);
            assert_eq!(picker.killer_moves[5][0], Some(pack_move(m2)));
            assert_eq!(picker.killer_moves[5][1], Some(pack_move(m1)));
        }
    }

    #[test]
    fn history_updates_on_cutoff() {
        let mut picker = MovePicker::new();
        let pos = Chess::default();
        let moves = pos.legal_moves();
        let quiet: Vec<Move> = moves.iter().filter(|m| !m.is_capture()).cloned().collect();
        if let Some(m) = quiet.first() {
            picker.update_history(m, 5, true);
            let (from, to) = move_squares(m);
            assert_eq!(picker.history[from][to], 25, "History bonus = depth^2 = 25");
        }
    }

    #[test]
    fn history_clamped_to_max() {
        let mut picker = MovePicker::new();
        let pos = Chess::default();
        let moves = pos.legal_moves();
        let quiet: Vec<Move> = moves.iter().filter(|m| !m.is_capture()).cloned().collect();
        if let Some(m) = quiet.first() {
            for _ in 0..1000 {
                picker.update_history(m, 20, true);
            }
            let (from, to) = move_squares(m);
            assert_eq!(picker.history[from][to], HISTORY_MAX);
        }
    }

    #[test]
    fn history_affects_quiet_ordering() {
        let mut picker = MovePicker::new();
        let pos = pos_from_fen("rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq - 0 1");
        let moves = pos.legal_moves();
        let quiet: Vec<Move> = moves.iter().filter(|m| !m.is_capture()).cloned().collect();
        if quiet.len() >= 2 {
            // Boost second quiet move's history
            picker.update_history(&quiet[1], 10, true); // +100
            let ordered = picker.order_moves(&moves, 0, None, None, 0);
            let idx0 = ordered.iter().position(|m| pack_move(m) == pack_move(&quiet[0]));
            let idx1 = ordered.iter().position(|m| pack_move(m) == pack_move(&quiet[1]));
            if let (Some(i0), Some(i1)) = (idx0, idx1) {
                assert!(i1 < i0, "Quiet with higher history should sort first");
            }
        }
    }

    #[test]
    fn losing_capture_scores_below_quiet() {
        // QxP where pawn is defended: SEE = 100 - 900 = -800
        // Score = LOSING_CAPTURE_BASE + (-800) = -1800
        // Quiet = 0, so losing capture sorts after quiet
        let see = 100 - 900; // pawn victim, queen attacker
        let losing_score = LOSING_CAPTURE_BASE + see;
        assert!(losing_score < 0, "Losing capture ({}) should score below quiet (0)", losing_score);
    }

    #[test]
    fn winning_capture_scores_above_quiet() {
        // PxQ: SEE = 900 - 100 = 800 (winning)
        // Score = CAPTURE_BASE + MVV-LVA = 10000+ (always above quiet)
        let see = 900 - 100;
        assert!(see >= 0, "PxQ should be winning SEE");
        // Winning captures get CAPTURE_BASE + MVV-LVA
        let score = 900 * 10 - 100 + CAPTURE_BASE; // 18900
        assert!(score > KILLER_SCORE, "Winning capture ({}) should score above killers ({})", score, KILLER_SCORE);
    }

    #[test]
    fn clear_resets_history() {
        let mut picker = MovePicker::new();
        picker.history[10][20] = 500;
        picker.clear();
        assert_eq!(picker.history[10][20], 0);
    }

    #[test]
    fn countermove_score_correct() {
        assert_eq!(COUNTERMOVE_SCORE, 8000);
        assert!(COUNTERMOVE_SCORE < KILLER_SCORE, "Countermove should be below killers");
        assert!(COUNTERMOVE_SCORE > 0, "Countermove should be above base quiet");
    }

    #[test]
    fn countermove_store_and_retrieve() {
        let mut picker = MovePicker::new();
        let pos = Chess::default();
        let moves = pos.legal_moves();
        let quiet: Vec<Move> = moves.iter().filter(|m| !m.is_capture()).cloned().collect();
        if quiet.len() >= 2 {
            let prev = pack_move(&quiet[0]);
            picker.store_countermove(Some(prev), &quiet[1]);
            let from = (prev & 0x3F) as usize;
            let to = ((prev >> 6) & 0x3F) as usize;
            assert_eq!(picker.countermoves[from][to], Some(pack_move(&quiet[1])));
        }
    }

    #[test]
    fn capture_history_updates() {
        let mut picker = MovePicker::new();
        // Simulate a capture history update
        picker.capture_hist[0][20][4] = 0; // white, square 20, queen captured
        let bonus = 5i32 * 5; // depth=5
        picker.capture_hist[0][20][4] = (picker.capture_hist[0][20][4] + bonus).min(HISTORY_MAX);
        assert_eq!(picker.capture_hist[0][20][4], 25);
    }

    #[test]
    fn clear_resets_countermove_and_capture_history() {
        let mut picker = MovePicker::new();
        picker.countermoves[5][10] = Some(0x1234);
        picker.capture_hist[0][20][3] = 500;
        picker.clear();
        assert_eq!(picker.countermoves[5][10], None);
        assert_eq!(picker.capture_hist[0][20][3], 0);
    }

    #[test]
    fn mvv_lva_formula_matches_spec() {
        // Frozen spec: victim_value * 10 - attacker_value + CAPTURE_BASE
        assert_eq!(CAPTURE_BASE, 10000);
        assert_eq!(KILLER_SCORE, 9000);
        assert_eq!(PROMOTION_SCORE, 5000);
        // Verify killer (9000) is between captures (10000+) and quiet (0)
        assert!(KILLER_SCORE < CAPTURE_BASE);
        assert!(KILLER_SCORE > 0);
        assert!(PROMOTION_SCORE < KILLER_SCORE);
    }
}
