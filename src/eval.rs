use shakmaty::{Chess, Color, Position, Role, Square};
use crate::pst;

// Frozen piece values per spec.md — DO NOT CHANGE
pub const PAWN: i32 = 100;
pub const KNIGHT: i32 = 300;
pub const BISHOP: i32 = 300; // NOT 325. NOT 320. 300.
pub const ROOK: i32 = 500;
pub const QUEEN: i32 = 900;
pub const KING: i32 = 20000;
pub const CHECKMATE: i32 = 1_000_000;
pub const DRAW: i32 = 0;

/// Material value for a piece role.
pub fn piece_value(role: Role) -> i32 {
    match role {
        Role::Pawn => PAWN,
        Role::Knight => KNIGHT,
        Role::Bishop => BISHOP,
        Role::Rook => ROOK,
        Role::Queen => QUEEN,
        Role::King => KING,
    }
}

/// PST lookup for a piece on a square.
/// PST tables are stored in CPW display order (rank 8 at index 0).
/// For white: pst_index = sq ^ 56 (flip rank from shakmaty's A1=0 convention)
/// For black: pst_index = sq (equivalent to mirroring the square)
fn pst_value(role: Role, sq: Square, color: Color, endgame: bool) -> i32 {
    let sq_idx = sq as usize;
    let pst_idx = match color {
        Color::White => sq_idx ^ 56,
        Color::Black => sq_idx,
    };
    match role {
        Role::Pawn => pst::PST_PAWN[pst_idx],
        Role::Knight => pst::PST_KNIGHT[pst_idx],
        Role::Bishop => pst::PST_BISHOP[pst_idx],
        Role::Rook => pst::PST_ROOK[pst_idx],
        Role::Queen => pst::PST_QUEEN[pst_idx],
        Role::King => {
            if endgame {
                pst::PST_KING_EG[pst_idx]
            } else {
                pst::PST_KING_MG[pst_idx]
            }
        }
    }
}

/// Evaluate position from WHITE's perspective.
/// score = material_score + pst_score
/// Positive = white winning, negative = black winning.
pub fn evaluate(pos: &Chess) -> i32 {
    let board = pos.board();

    // Endgame detection: no queens on the board
    let endgame = board.queens().count() == 0;

    let mut score: i32 = 0;

    // Iterate over all pieces, accumulating material + PST
    for sq in board.occupied() {
        let color = board.color_at(sq).unwrap();
        let role = board.role_at(sq).unwrap();
        let material = piece_value(role);
        let positional = pst_value(role, sq, color, endgame);
        let piece_score = material + positional;
        match color {
            Color::White => score += piece_score,
            Color::Black => score -= piece_score,
        }
    }

    score
}

#[cfg(test)]
mod tests {
    use super::*;
    use shakmaty::fen::Fen;
    use shakmaty::CastlingMode;

    fn pos_from_fen(fen: &str) -> Chess {
        let f: Fen = fen.parse().unwrap();
        f.into_position(CastlingMode::Standard).unwrap()
    }

    #[test]
    fn starting_position_near_zero() {
        // PST is symmetric for starting position, so eval should be 0
        let score = evaluate(&Chess::default());
        assert_eq!(score, 0, "Starting position should evaluate to 0 (symmetric PSTs)");
    }

    #[test]
    fn frozen_values() {
        assert_eq!(PAWN, 100);
        assert_eq!(KNIGHT, 300);
        assert_eq!(BISHOP, 300);
        assert_eq!(ROOK, 500);
        assert_eq!(QUEEN, 900);
        assert_eq!(KING, 20000);
    }

    #[test]
    fn knight_e4_better_than_a1() {
        // White knight on e4 vs white knight on a1 (with kings for legal position)
        let pos_e4 = pos_from_fen("4k3/8/8/8/4N3/8/8/4K3 w - - 0 1");
        let pos_a1 = pos_from_fen("4k3/8/8/8/8/8/8/N3K3 w - - 0 1");
        let score_e4 = evaluate(&pos_e4);
        let score_a1 = evaluate(&pos_a1);
        assert!(score_e4 > score_a1, "Knight on e4 ({}) should score higher than a1 ({})", score_e4, score_a1);
    }

    #[test]
    fn king_center_better_in_endgame() {
        // Endgame (no queens): king in center should be better
        let pos_center = pos_from_fen("4k3/8/8/3K4/8/8/8/8 w - - 0 1");
        let pos_corner = pos_from_fen("4k3/8/8/8/8/8/8/K7 w - - 0 1");
        let score_center = evaluate(&pos_center);
        let score_corner = evaluate(&pos_corner);
        assert!(score_center > score_corner,
            "King in center ({}) should score higher than corner ({}) in endgame",
            score_center, score_corner);
    }

    #[test]
    fn king_corner_better_in_middlegame() {
        // Middlegame (queens present): king in corner should be better
        // White king on g1 vs e4, both sides have queens
        let pos_corner = pos_from_fen("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1");
        // Starting position — king is on e1 with castling available
        // Just verify middlegame king PST penalizes center
        let pst_g1 = pst_value(Role::King, Square::G1, Color::White, false);
        let pst_e4 = pst_value(Role::King, Square::E4, Color::White, false);
        assert!(pst_g1 > pst_e4,
            "King PST MG: g1 ({}) should be higher than e4 ({})", pst_g1, pst_e4);
    }

    #[test]
    fn pst_values_are_michniewski_spot_checks() {
        // Spot check 10 values against frozen/pst.rs
        // Knight center (d4/e4): 20
        assert_eq!(pst::PST_KNIGHT[27], 20); // d5 in PST (index 27)
        assert_eq!(pst::PST_KNIGHT[28], 20); // e5 in PST (index 28)

        // Pawn rank 7 (about to promote): 50
        assert_eq!(pst::PST_PAWN[8], 50);  // a7 in PST
        assert_eq!(pst::PST_PAWN[15], 50); // h7 in PST

        // Bishop corners: -20
        assert_eq!(pst::PST_BISHOP[0], -20);  // a8 corner
        assert_eq!(pst::PST_BISHOP[63], -20); // h1 corner

        // King MG castled position (g1 in PST = index 62): 30
        assert_eq!(pst::PST_KING_MG[62], 30);

        // King EG center (d4/e4 in PST): 40
        assert_eq!(pst::PST_KING_EG[27], 40);
        assert_eq!(pst::PST_KING_EG[28], 40);

        // Rook 7th rank bonus (PST index 8-15): 5 or 10
        assert_eq!(pst::PST_ROOK[8], 5);
    }

    #[test]
    fn material_values_unchanged_with_pst() {
        // Verify material constants haven't drifted
        assert_eq!(piece_value(Role::Pawn), 100);
        assert_eq!(piece_value(Role::Knight), 300);
        assert_eq!(piece_value(Role::Bishop), 300);
        assert_eq!(piece_value(Role::Rook), 500);
        assert_eq!(piece_value(Role::Queen), 900);
        assert_eq!(piece_value(Role::King), 20000);
    }
}
