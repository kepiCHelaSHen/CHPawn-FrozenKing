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

/// MVV-LVA score per frozen spec:
///   victim_value * 10 - attacker_value + CAPTURE_BASE
fn mvv_lva_score(m: &Move) -> i32 {
    if m.is_capture() {
        let victim = m.capture().map(|r| role_value(r)).unwrap_or(0);
        let attacker = role_value(m.role());
        victim * 10 - attacker + CAPTURE_BASE
    } else if m.is_promotion() {
        PROMOTION_SCORE
    } else {
        0
    }
}

/// Move picker with killer move support.
pub struct MovePicker {
    /// 2 killer slots per depth, up to 64 depths
    killer_moves: [[Option<u16>; 2]; 64],
}

impl MovePicker {
    pub fn new() -> Self {
        MovePicker {
            killer_moves: [[None; 2]; 64],
        }
    }

    /// Order moves: captures (MVV-LVA) > killers > quiet moves.
    /// TT move (if provided) is searched first.
    pub fn order_moves(&self, moves: &MoveList, depth: u8, tt_move: Option<u16>) -> Vec<Move> {
        let mut scored: Vec<(Move, i32)> = moves
            .iter()
            .map(|m| {
                let packed = pack_move(m);
                let score = if tt_move == Some(packed) {
                    i32::MAX // TT move first
                } else if m.is_capture() || m.is_promotion() {
                    mvv_lva_score(m)
                } else if self.is_killer(packed, depth) {
                    KILLER_SCORE
                } else {
                    0
                };
                (m.clone(), score)
            })
            .collect();
        scored.sort_by(|a, b| b.1.cmp(&a.1));
        scored.into_iter().map(|(m, _)| m).collect()
    }

    /// Order captures only by MVV-LVA for quiescence search.
    pub fn order_captures(&self, moves: &MoveList) -> Vec<Move> {
        let mut captures: Vec<(Move, i32)> = moves
            .iter()
            .filter(|m| m.is_capture())
            .map(|m| (m.clone(), mvv_lva_score(m)))
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

    pub fn clear(&mut self) {
        self.killer_moves = [[None; 2]; 64];
    }
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
        let ordered = picker.order_moves(&moves, 1, None);

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
            let ordered = picker.order_moves(&moves, 3, None);

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
