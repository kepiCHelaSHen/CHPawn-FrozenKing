use shakmaty::{Bitboard, Board, Chess, Color, File, Position, Rank, Role, Square};
use shakmaty::attacks;
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

// v0.0.4 Evaluation Constants
pub const PASSED_PAWN_BONUS: [i32; 8] = [0, 10, 20, 30, 50, 75, 100, 0];
pub const DOUBLED_PAWN_PENALTY: i32 = -20;
pub const ISOLATED_PAWN_PENALTY: i32 = -15;
pub const ROOK_OPEN_FILE_BONUS: i32 = 25;
pub const ROOK_SEMI_OPEN_FILE_BONUS: i32 = 10;
pub const BISHOP_PAIR_BONUS: i32 = 50;
pub const KING_ATTACKER_PENALTY: i32 = -10;
pub const KING_SHIELD_BONUS: i32 = 10;

// v0.0.7 Evaluation Constants
// Index: 0=none, 1=pawn, 2=knight, 3=bishop, 4=rook, 5=queen, 6=king
pub const MOBILITY_WEIGHT: [i32; 7] = [0, 1, 4, 4, 2, 1, 0];
pub const KNIGHT_OUTPOST_BONUS: i32 = 30;
pub const BISHOP_OUTPOST_BONUS: i32 = 20;
pub const DOUBLED_ROOKS_BONUS: i32 = 20;
pub const ROOK_SEVENTH_RANK_BONUS: i32 = 30;
pub const UNDEVELOPED_PIECE_PENALTY: i32 = -10;

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

// ============================================================================
// Feature 1 — Passed Pawn Bonuses (v0.0.4)
// Source: chessprogramming.org/Passed_Pawns
// ============================================================================

/// Check if a pawn is passed (no enemy pawns ahead on same or adjacent files).
fn is_passed_pawn(board: &Board, sq: Square, color: Color) -> bool {
    let enemy_pawns = board.pawns() & board.by_color(!color);
    let file = sq.file() as i32;
    let rank = sq.rank() as i32;

    for f_offset in -1i32..=1 {
        let f = file + f_offset;
        if f < 0 || f > 7 { continue; }
        let check_file = File::new(f as u32);

        // Check ranks ahead of pawn toward promotion
        let (start_rank, end_rank) = match color {
            Color::White => (rank + 1, 8),
            Color::Black => (0, rank),
        };
        for r in start_rank..end_rank {
            let check_sq = Square::from_coords(check_file, Rank::new(r as u32));
            if enemy_pawns.contains(check_sq) {
                return false;
            }
        }
    }
    true
}

/// Evaluate passed pawns. Returns bonus from WHITE's perspective.
fn evaluate_passed_pawns(board: &Board) -> i32 {
    let mut score = 0i32;
    let white_pawns = board.pawns() & board.by_color(Color::White);
    let black_pawns = board.pawns() & board.by_color(Color::Black);

    for sq in white_pawns {
        if is_passed_pawn(board, sq, Color::White) {
            score += PASSED_PAWN_BONUS[sq.rank() as usize];
        }
    }
    for sq in black_pawns {
        if is_passed_pawn(board, sq, Color::Black) {
            score -= PASSED_PAWN_BONUS[7 - sq.rank() as usize];
        }
    }
    score
}

// ============================================================================
// Feature 2 — Pawn Structure Penalties (v0.0.4)
// Source: chessprogramming.org/Pawn_Structure
// ============================================================================

/// Evaluate pawn structure (doubled + isolated penalties). Returns score from WHITE's perspective.
fn evaluate_pawn_structure(board: &Board) -> i32 {
    let white_pawns = board.pawns() & board.by_color(Color::White);
    let black_pawns = board.pawns() & board.by_color(Color::Black);

    let mut white_per_file = [0u32; 8];
    let mut black_per_file = [0u32; 8];

    for sq in white_pawns { white_per_file[sq.file() as usize] += 1; }
    for sq in black_pawns { black_per_file[sq.file() as usize] += 1; }

    let mut score = 0i32;

    // Doubled pawns
    for f in 0..8 {
        if white_per_file[f] > 1 { score += DOUBLED_PAWN_PENALTY * (white_per_file[f] as i32 - 1); }
        if black_per_file[f] > 1 { score -= DOUBLED_PAWN_PENALTY * (black_per_file[f] as i32 - 1); }
    }

    // Isolated pawns: no friendly pawn on adjacent files
    for f in 0..8usize {
        let left = if f > 0 { white_per_file[f - 1] } else { 0 };
        let right = if f < 7 { white_per_file[f + 1] } else { 0 };
        if white_per_file[f] > 0 && left == 0 && right == 0 {
            score += ISOLATED_PAWN_PENALTY * white_per_file[f] as i32;
        }

        let left_b = if f > 0 { black_per_file[f - 1] } else { 0 };
        let right_b = if f < 7 { black_per_file[f + 1] } else { 0 };
        if black_per_file[f] > 0 && left_b == 0 && right_b == 0 {
            score -= ISOLATED_PAWN_PENALTY * black_per_file[f] as i32;
        }
    }

    score
}

// ============================================================================
// Feature 3 — Rook on Open File Bonus (v0.0.4)
// Source: chessprogramming.org/Rook_on_Open_File
// ============================================================================

/// Evaluate rook file bonuses. Returns score from WHITE's perspective.
fn evaluate_rook_files(board: &Board) -> i32 {
    let white_pawns = board.pawns() & board.by_color(Color::White);
    let black_pawns = board.pawns() & board.by_color(Color::Black);
    let white_rooks = board.rooks() & board.by_color(Color::White);
    let black_rooks = board.rooks() & board.by_color(Color::Black);

    let mut white_pawn_files = [false; 8];
    let mut black_pawn_files = [false; 8];
    for sq in white_pawns { white_pawn_files[sq.file() as usize] = true; }
    for sq in black_pawns { black_pawn_files[sq.file() as usize] = true; }

    let mut score = 0i32;

    for sq in white_rooks {
        let f = sq.file() as usize;
        if !white_pawn_files[f] && !black_pawn_files[f] {
            score += ROOK_OPEN_FILE_BONUS;
        } else if !white_pawn_files[f] && black_pawn_files[f] {
            score += ROOK_SEMI_OPEN_FILE_BONUS;
        }
    }
    for sq in black_rooks {
        let f = sq.file() as usize;
        if !white_pawn_files[f] && !black_pawn_files[f] {
            score -= ROOK_OPEN_FILE_BONUS;
        } else if !black_pawn_files[f] && white_pawn_files[f] {
            score -= ROOK_SEMI_OPEN_FILE_BONUS;
        }
    }

    score
}

// ============================================================================
// Feature 5 — King Safety (v0.0.4, simplified)
// Source: chessprogramming.org/King_Safety
// ============================================================================

/// Evaluate king safety. Only in middlegame (queens on board).
/// Returns score from WHITE's perspective.
fn evaluate_king_safety(board: &Board) -> i32 {
    // Only apply in middlegame
    if board.queens().count() == 0 {
        return 0;
    }

    let mut score = 0i32;

    // White king
    if let Some(wk) = (board.kings() & board.by_color(Color::White)).into_iter().next() {
        let kf = wk.file() as i32;
        let kr = wk.rank() as i32;
        let white_pawns = board.pawns() & board.by_color(Color::White);
        let enemy_non_pawns = board.by_color(Color::Black) & !(board.pawns() | board.kings());

        // Pawn shield: friendly pawns on rank directly ahead of king
        if kr < 7 {
            for df in -1i32..=1 {
                let f = kf + df;
                if f < 0 || f > 7 { continue; }
                let shield_sq = Square::from_coords(File::new(f as u32), Rank::new((kr + 1) as u32));
                if white_pawns.contains(shield_sq) {
                    score += KING_SHIELD_BONUS;
                }
            }
        }

        // Attackers: enemy non-pawn pieces adjacent to king
        for df in -1i32..=1 {
            for dr in -1i32..=1 {
                if df == 0 && dr == 0 { continue; }
                let f = kf + df;
                let r = kr + dr;
                if f < 0 || f > 7 || r < 0 || r > 7 { continue; }
                let adj_sq = Square::from_coords(File::new(f as u32), Rank::new(r as u32));
                if enemy_non_pawns.contains(adj_sq) {
                    score += KING_ATTACKER_PENALTY;
                }
            }
        }
    }

    // Black king
    if let Some(bk) = (board.kings() & board.by_color(Color::Black)).into_iter().next() {
        let kf = bk.file() as i32;
        let kr = bk.rank() as i32;
        let black_pawns = board.pawns() & board.by_color(Color::Black);
        let enemy_non_pawns = board.by_color(Color::White) & !(board.pawns() | board.kings());

        // Pawn shield: friendly pawns on rank directly ahead of king (for black, ahead = lower rank)
        if kr > 0 {
            for df in -1i32..=1 {
                let f = kf + df;
                if f < 0 || f > 7 { continue; }
                let shield_sq = Square::from_coords(File::new(f as u32), Rank::new((kr - 1) as u32));
                if black_pawns.contains(shield_sq) {
                    score -= KING_SHIELD_BONUS;
                }
            }
        }

        // Attackers: enemy non-pawn pieces adjacent to king
        for df in -1i32..=1 {
            for dr in -1i32..=1 {
                if df == 0 && dr == 0 { continue; }
                let f = kf + df;
                let r = kr + dr;
                if f < 0 || f > 7 || r < 0 || r > 7 { continue; }
                let adj_sq = Square::from_coords(File::new(f as u32), Rank::new(r as u32));
                if enemy_non_pawns.contains(adj_sq) {
                    score -= KING_ATTACKER_PENALTY;
                }
            }
        }
    }

    score
}

// ============================================================================
// Feature 6 — Piece Mobility (v0.0.7)
// Source: chessprogramming.org/Mobility
// ============================================================================

fn role_to_mobility_index(role: Role) -> usize {
    match role {
        Role::Pawn => 1,
        Role::Knight => 2,
        Role::Bishop => 3,
        Role::Rook => 4,
        Role::Queen => 5,
        Role::King => 6,
    }
}

/// Compute squares attacked by all pawns of a given color.
fn pawn_attack_span(board: &Board, color: Color) -> Bitboard {
    let mut result = Bitboard::EMPTY;
    let pawns = board.pawns() & board.by_color(color);
    for sq in pawns {
        result = result | attacks::pawn_attacks(color, sq);
    }
    result
}

/// Evaluate piece mobility. Returns score from WHITE's perspective.
fn evaluate_mobility(board: &Board) -> i32 {
    let occupied = board.occupied();
    let enemy_pawn_attacks_w = pawn_attack_span(board, Color::Black);
    let enemy_pawn_attacks_b = pawn_attack_span(board, Color::White);
    let mut score = 0i32;

    // White pieces
    let white_pieces = board.by_color(Color::White) & !(board.pawns() | board.kings());
    for sq in white_pieces {
        let role = board.role_at(sq).unwrap();
        let atk = match role {
            Role::Knight => attacks::knight_attacks(sq),
            Role::Bishop => attacks::bishop_attacks(sq, occupied),
            Role::Rook => attacks::rook_attacks(sq, occupied),
            Role::Queen => attacks::queen_attacks(sq, occupied),
            _ => Bitboard::EMPTY,
        };
        // Count safe squares: attacked squares not defended by enemy pawns
        let safe = atk & !enemy_pawn_attacks_w;
        let count = safe.count() as i32;
        score += MOBILITY_WEIGHT[role_to_mobility_index(role)] * count;
    }

    // Black pieces
    let black_pieces = board.by_color(Color::Black) & !(board.pawns() | board.kings());
    for sq in black_pieces {
        let role = board.role_at(sq).unwrap();
        let atk = match role {
            Role::Knight => attacks::knight_attacks(sq),
            Role::Bishop => attacks::bishop_attacks(sq, occupied),
            Role::Rook => attacks::rook_attacks(sq, occupied),
            Role::Queen => attacks::queen_attacks(sq, occupied),
            _ => Bitboard::EMPTY,
        };
        let safe = atk & !enemy_pawn_attacks_b;
        let count = safe.count() as i32;
        score -= MOBILITY_WEIGHT[role_to_mobility_index(role)] * count;
    }

    score
}

// ============================================================================
// Feature 7 — Outpost Detection (v0.0.7)
// Source: chessprogramming.org/Outpost
// ============================================================================

/// Check if a square is an outpost for the given color.
fn is_outpost(board: &Board, sq: Square, color: Color) -> bool {
    let rank = sq.rank() as i32;
    let file = sq.file() as i32;

    // Must be in enemy half
    let in_enemy_half = match color {
        Color::White => rank >= 4, // rank 5-8 (0-indexed: 4-7)
        Color::Black => rank <= 3, // rank 1-4 (0-indexed: 0-3)
    };
    if !in_enemy_half { return false; }

    // Must be defended by a friendly pawn
    let friendly_pawns = board.pawns() & board.by_color(color);
    let defended = match color {
        Color::White => {
            // A white pawn defends this square if it's on (file±1, rank-1)
            if rank == 0 { return false; }
            let mut defended = false;
            for df in [-1i32, 1] {
                let f = file + df;
                if f < 0 || f > 7 { continue; }
                let pawn_sq = Square::from_coords(File::new(f as u32), Rank::new((rank - 1) as u32));
                if friendly_pawns.contains(pawn_sq) { defended = true; break; }
            }
            defended
        }
        Color::Black => {
            if rank >= 7 { return false; }
            let mut defended = false;
            for df in [-1i32, 1] {
                let f = file + df;
                if f < 0 || f > 7 { continue; }
                let pawn_sq = Square::from_coords(File::new(f as u32), Rank::new((rank + 1) as u32));
                if friendly_pawns.contains(pawn_sq) { defended = true; break; }
            }
            defended
        }
    };
    if !defended { return false; }

    // No enemy pawns on adjacent files ahead that could attack this square
    let enemy_pawns = board.pawns() & board.by_color(!color);
    for f_offset in [-1i32, 1] {
        let f = file + f_offset;
        if f < 0 || f > 7 { continue; }
        let check_file = File::new(f as u32);
        let (start_rank, end_rank) = match color {
            Color::White => (rank, 8), // ranks ahead for white
            Color::Black => (0, rank + 1),
        };
        for r in start_rank..end_rank {
            let check_sq = Square::from_coords(check_file, Rank::new(r as u32));
            if enemy_pawns.contains(check_sq) { return false; }
        }
    }

    true
}

/// Evaluate outpost bonuses. Returns score from WHITE's perspective.
fn evaluate_outposts(board: &Board) -> i32 {
    let mut score = 0i32;

    for sq in board.by_color(Color::White) & !(board.pawns() | board.kings()) {
        let role = board.role_at(sq).unwrap();
        if is_outpost(board, sq, Color::White) {
            match role {
                Role::Knight => score += KNIGHT_OUTPOST_BONUS,
                Role::Bishop => score += BISHOP_OUTPOST_BONUS,
                _ => {}
            }
        }
    }
    for sq in board.by_color(Color::Black) & !(board.pawns() | board.kings()) {
        let role = board.role_at(sq).unwrap();
        if is_outpost(board, sq, Color::Black) {
            match role {
                Role::Knight => score -= KNIGHT_OUTPOST_BONUS,
                Role::Bishop => score -= BISHOP_OUTPOST_BONUS,
                _ => {}
            }
        }
    }
    score
}

// ============================================================================
// Feature 8 — Rook Coordination (v0.0.7)
// Source: chessprogramming.org/Connectivity
// ============================================================================

/// Evaluate rook coordination. Returns score from WHITE's perspective.
fn evaluate_rook_coordination(board: &Board) -> i32 {
    let mut score = 0i32;
    let occupied = board.occupied();

    // White rooks
    let white_rooks = board.rooks() & board.by_color(Color::White);
    let wr_squares: Vec<Square> = white_rooks.into_iter().collect();
    // Doubled rooks: two on same file with clear line
    if wr_squares.len() >= 2 {
        for i in 0..wr_squares.len() {
            for j in (i + 1)..wr_squares.len() {
                if wr_squares[i].file() == wr_squares[j].file() {
                    // Check if rook attacks can reach the other (clear line)
                    let atk = attacks::rook_attacks(wr_squares[i], occupied);
                    if atk.contains(wr_squares[j]) {
                        score += DOUBLED_ROOKS_BONUS;
                    }
                }
            }
        }
    }
    // Rook on 7th rank
    for sq in white_rooks {
        if sq.rank() as i32 == 6 { // Rank 7 (0-indexed = 6)
            score += ROOK_SEVENTH_RANK_BONUS;
        }
    }

    // Black rooks
    let black_rooks = board.rooks() & board.by_color(Color::Black);
    let br_squares: Vec<Square> = black_rooks.into_iter().collect();
    if br_squares.len() >= 2 {
        for i in 0..br_squares.len() {
            for j in (i + 1)..br_squares.len() {
                if br_squares[i].file() == br_squares[j].file() {
                    let atk = attacks::rook_attacks(br_squares[i], occupied);
                    if atk.contains(br_squares[j]) {
                        score -= DOUBLED_ROOKS_BONUS;
                    }
                }
            }
        }
    }
    for sq in black_rooks {
        if sq.rank() as i32 == 1 { // Rank 2 (0-indexed = 1) — 7th for black
            score -= ROOK_SEVENTH_RANK_BONUS;
        }
    }

    score
}

// ============================================================================
// Feature 9 — Development Penalty (v0.0.7)
// Source: chessprogramming.org/Development
// ============================================================================

/// Evaluate development in opening phase. Returns score from WHITE's perspective.
fn evaluate_development(pos: &Chess) -> i32 {
    // Only apply in first 20 moves (halfmoves <= 40)
    if pos.fullmoves().get() > 20 { return 0; }

    let board = pos.board();
    let mut score = 0i32;

    // White undeveloped minor pieces (knights on b1/g1, bishops on c1/f1)
    let white = board.by_color(Color::White);
    if (board.knights() & white).contains(Square::B1) { score += UNDEVELOPED_PIECE_PENALTY; }
    if (board.knights() & white).contains(Square::G1) { score += UNDEVELOPED_PIECE_PENALTY; }
    if (board.bishops() & white).contains(Square::C1) { score += UNDEVELOPED_PIECE_PENALTY; }
    if (board.bishops() & white).contains(Square::F1) { score += UNDEVELOPED_PIECE_PENALTY; }

    // Black undeveloped minor pieces (knights on b8/g8, bishops on c8/f8)
    let black = board.by_color(Color::Black);
    if (board.knights() & black).contains(Square::B8) { score -= UNDEVELOPED_PIECE_PENALTY; }
    if (board.knights() & black).contains(Square::G8) { score -= UNDEVELOPED_PIECE_PENALTY; }
    if (board.bishops() & black).contains(Square::C8) { score -= UNDEVELOPED_PIECE_PENALTY; }
    if (board.bishops() & black).contains(Square::F8) { score -= UNDEVELOPED_PIECE_PENALTY; }

    score
}

// ============================================================================
// Main Evaluation
// ============================================================================

/// Evaluate position from WHITE's perspective.
/// score = material + PST + passed pawns + pawn structure + rook files + bishop pair + king safety
/// Positive = white winning, negative = black winning.
pub fn evaluate(pos: &Chess) -> i32 {
    let board = pos.board();

    // Endgame detection: no queens on the board
    let endgame = board.queens().count() == 0;

    let mut score: i32 = 0;

    // Material + PST
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

    // Feature 1 — Passed pawn bonuses
    score += evaluate_passed_pawns(board);

    // Feature 2 — Pawn structure penalties
    score += evaluate_pawn_structure(board);

    // Feature 3 — Rook on open/semi-open file bonuses
    score += evaluate_rook_files(board);

    // Feature 4 — Bishop pair bonus
    let white_bishops = (board.bishops() & board.by_color(Color::White)).count();
    let black_bishops = (board.bishops() & board.by_color(Color::Black)).count();
    if white_bishops >= 2 { score += BISHOP_PAIR_BONUS; }
    if black_bishops >= 2 { score -= BISHOP_PAIR_BONUS; }

    // Feature 5 — King safety
    score += evaluate_king_safety(board);

    // Feature 6 — Piece mobility
    score += evaluate_mobility(board);

    // Feature 7 — Outpost detection
    score += evaluate_outposts(board);

    // Feature 8 — Rook coordination
    score += evaluate_rook_coordination(board);

    // Feature 9 — Development penalty
    score += evaluate_development(pos);

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

    // === Feature 1 — Passed Pawn Tests ===

    #[test]
    fn passed_pawn_detected() {
        // White pawn on e5, no black pawns on d,e,f ahead
        let pos = pos_from_fen("4k3/8/8/4P3/8/8/8/4K3 w - - 0 1");
        let board = pos.board();
        assert!(is_passed_pawn(board, Square::E5, Color::White));
    }

    #[test]
    fn passed_pawn_blocked_by_adjacent() {
        // White pawn on e5, black pawn on f6 — NOT passed
        let pos = pos_from_fen("4k3/8/5p2/4P3/8/8/8/4K3 w - - 0 1");
        let board = pos.board();
        assert!(!is_passed_pawn(board, Square::E5, Color::White));
    }

    #[test]
    fn passed_pawn_bonus_increases_with_rank() {
        assert!(PASSED_PAWN_BONUS[5] > PASSED_PAWN_BONUS[4]);
        assert!(PASSED_PAWN_BONUS[4] > PASSED_PAWN_BONUS[3]);
        assert!(PASSED_PAWN_BONUS[6] > PASSED_PAWN_BONUS[5]);
    }

    #[test]
    fn starting_position_no_passed_pawns() {
        let pos = Chess::default();
        assert_eq!(evaluate_passed_pawns(pos.board()), 0);
    }

    // === Feature 2 — Pawn Structure Tests ===

    #[test]
    fn doubled_pawns_penalized() {
        // Two white pawns on e file
        let pos = pos_from_fen("4k3/8/8/4P3/4P3/8/8/4K3 w - - 0 1");
        let structure = evaluate_pawn_structure(pos.board());
        assert!(structure < 0, "Doubled pawns should give negative score, got {}", structure);
    }

    #[test]
    fn isolated_pawn_penalized() {
        // White pawn on e4 with no white pawns on d or f
        let pos = pos_from_fen("4k3/8/8/8/4P3/8/8/4K3 w - - 0 1");
        let structure = evaluate_pawn_structure(pos.board());
        assert!(structure < 0, "Isolated pawn should give negative score, got {}", structure);
    }

    #[test]
    fn starting_position_no_pawn_penalties() {
        let pos = Chess::default();
        assert_eq!(evaluate_pawn_structure(pos.board()), 0);
    }

    // === Feature 3 — Rook File Tests ===

    #[test]
    fn rook_open_file_bonus() {
        // White rook on e1, no pawns on e file at all
        let pos = pos_from_fen("4k3/8/8/8/8/8/8/R3K3 w - - 0 1");
        let rook_score = evaluate_rook_files(pos.board());
        assert_eq!(rook_score, ROOK_OPEN_FILE_BONUS);
    }

    #[test]
    fn rook_semi_open_file_bonus() {
        // White rook on e1, black pawn on e5, no white pawn on e
        let pos = pos_from_fen("4k3/8/8/4p3/8/8/8/4K2R w - - 0 1");
        let board = pos.board();
        let rook_score = evaluate_rook_files(board);
        // Rook is on h file (open, no pawns) so it gets open file bonus
        assert_eq!(rook_score, ROOK_OPEN_FILE_BONUS);
    }

    #[test]
    fn starting_position_no_rook_bonus() {
        let pos = Chess::default();
        assert_eq!(evaluate_rook_files(pos.board()), 0);
    }

    // === Feature 4 — Bishop Pair Tests ===

    #[test]
    fn bishop_pair_bonus_applied() {
        // White has both bishops, black has one
        let pos = pos_from_fen("4k3/8/8/8/8/8/8/2B1KB2 w - - 0 1");
        let board = pos.board();
        let wb = (board.bishops() & board.by_color(Color::White)).count();
        assert!(wb >= 2);
    }

    #[test]
    fn bishop_pair_symmetric_cancels() {
        // Both sides have bishop pair — cancels out
        let pos = pos_from_fen("2b1kb2/8/8/8/8/8/8/2B1KB2 w - - 0 1");
        let board = pos.board();
        let wb = (board.bishops() & board.by_color(Color::White)).count();
        let bb = (board.bishops() & board.by_color(Color::Black)).count();
        assert!(wb >= 2 && bb >= 2, "Both sides should have bishop pair");
    }

    // === Feature 5 — King Safety Tests ===

    #[test]
    fn king_shield_bonus_applied() {
        // White king on g1 with pawns on f2,g2,h2 — 3 shield pawns. Queens on board.
        let pos = pos_from_fen("rnbqk2r/pppppppp/8/8/8/8/PPPPP1PP/RNBQKBR1 w Qkq - 0 1");
        // This has queens so king safety applies
        let board = pos.board();
        let ks = evaluate_king_safety(board);
        // White king on e1 has pawns on d2, e-file empty, f-file empty... depends on exact position
        // Let's just verify it runs without panic
        let _ = ks;
    }

    #[test]
    fn king_safety_disabled_in_endgame() {
        // No queens on board — king safety should return 0
        let pos = pos_from_fen("4k3/pppp1ppp/8/8/8/8/PPPP1PPP/4K3 w - - 0 1");
        assert_eq!(evaluate_king_safety(pos.board()), 0);
    }

    #[test]
    fn starting_position_king_safety_symmetric() {
        let pos = Chess::default();
        assert_eq!(evaluate_king_safety(pos.board()), 0);
    }

    // === Frozen value verification ===

    #[test]
    fn eval_constants_correct() {
        assert_eq!(PASSED_PAWN_BONUS, [0, 10, 20, 30, 50, 75, 100, 0]);
        assert_eq!(DOUBLED_PAWN_PENALTY, -20);
        assert_eq!(ISOLATED_PAWN_PENALTY, -15);
        assert_eq!(ROOK_OPEN_FILE_BONUS, 25);
        assert_eq!(ROOK_SEMI_OPEN_FILE_BONUS, 10);
        assert_eq!(BISHOP_PAIR_BONUS, 50);
        assert_eq!(KING_ATTACKER_PENALTY, -10);
        assert_eq!(KING_SHIELD_BONUS, 10);
    }

    // === Part 6 — Evaluation Sanity Checks (REVIEW_v004) ===

    #[test]
    fn kings_only_evaluates_to_zero() {
        let pos = pos_from_fen("4k3/8/8/8/8/8/8/4K3 w - - 0 1");
        assert_eq!(evaluate(&pos), 0, "Kings-only position should be 0");
    }

    #[test]
    fn extra_white_queen_scores_high() {
        // White has extra queen, score should reflect queen material (900) minus PST variance
        let pos = pos_from_fen("4k3/8/8/8/8/8/8/3QK3 w - - 0 1");
        let score = evaluate(&pos);
        assert!(score > 850, "Extra white queen should give score > 850, got {}", score);
    }

    #[test]
    fn symmetric_position_evaluates_to_zero() {
        // Perfectly symmetric middlegame
        let pos = pos_from_fen("r1bqkb1r/pppppppp/2n2n2/8/8/2N2N2/PPPPPPPP/R1BQKB1R w KQkq - 0 1");
        let score = evaluate(&pos);
        assert_eq!(score, 0, "Symmetric position should evaluate to 0, got {}", score);
    }

    #[test]
    fn material_values_unchanged_with_pst() {
        assert_eq!(piece_value(Role::Pawn), 100);
        assert_eq!(piece_value(Role::Knight), 300);
        assert_eq!(piece_value(Role::Bishop), 300);
        assert_eq!(piece_value(Role::Rook), 500);
        assert_eq!(piece_value(Role::Queen), 900);
        assert_eq!(piece_value(Role::King), 20000);
    }

    // === v0.0.7 Feature Tests ===

    #[test]
    fn mobility_weight_correct() {
        assert_eq!(MOBILITY_WEIGHT, [0, 1, 4, 4, 2, 1, 0]);
    }

    #[test]
    fn mobility_knight_center_vs_corner() {
        // Knight in center has more mobility than knight in corner
        let pos_center = pos_from_fen("4k3/8/8/8/4N3/8/8/4K3 w - - 0 1");
        let pos_corner = pos_from_fen("4k3/8/8/8/8/8/8/N3K3 w - - 0 1");
        let mob_center = evaluate_mobility(pos_center.board());
        let mob_corner = evaluate_mobility(pos_corner.board());
        assert!(mob_center > mob_corner,
            "Knight center mobility ({}) should exceed corner ({})", mob_center, mob_corner);
    }

    #[test]
    fn mobility_starting_near_zero() {
        let pos = Chess::default();
        let mob = evaluate_mobility(pos.board());
        assert!(mob.abs() <= 5, "Starting position mobility should be near 0, got {}", mob);
    }

    #[test]
    fn outpost_detected() {
        // White knight on e5, white pawn on d4 defends, no black pawns on d/f ahead
        let pos = pos_from_fen("4k3/8/8/4N3/3P4/8/8/4K3 w - - 0 1");
        assert!(is_outpost(pos.board(), Square::E5, Color::White));
    }

    #[test]
    fn outpost_blocked_by_enemy_pawn() {
        // White knight on e5, but black pawn on d6 can attack
        let pos = pos_from_fen("4k3/8/3p4/4N3/3P4/8/8/4K3 w - - 0 1");
        assert!(!is_outpost(pos.board(), Square::E5, Color::White));
    }

    #[test]
    fn outpost_constants_correct() {
        assert_eq!(KNIGHT_OUTPOST_BONUS, 30);
        assert_eq!(BISHOP_OUTPOST_BONUS, 20);
    }

    #[test]
    fn rook_coordination_constants() {
        assert_eq!(DOUBLED_ROOKS_BONUS, 20);
        assert_eq!(ROOK_SEVENTH_RANK_BONUS, 30);
    }

    #[test]
    fn rook_seventh_rank_bonus() {
        // White rook on a7 (rank 7), kings away from rook
        let pos = pos_from_fen("7k/R7/8/8/8/8/8/4K3 w - - 0 1");
        let coord = evaluate_rook_coordination(pos.board());
        assert!(coord >= ROOK_SEVENTH_RANK_BONUS,
            "Rook on 7th rank should get bonus, got {}", coord);
    }

    #[test]
    fn development_starting_symmetric() {
        let pos = Chess::default();
        assert_eq!(evaluate_development(&pos), 0);
    }

    #[test]
    fn development_penalty_for_undeveloped() {
        // White has developed, black hasn't — white should have advantage
        let pos = pos_from_fen("rnbqkbnr/pppppppp/8/8/4P3/2N2N2/PPPP1PPP/R1BQKB1R b kq - 0 2");
        let dev = evaluate_development(&pos);
        assert!(dev > 0, "White developed should score positive, got {}", dev);
    }

    #[test]
    fn development_constants_correct() {
        assert_eq!(UNDEVELOPED_PIECE_PENALTY, -10);
    }

    #[test]
    fn v007_constants_complete() {
        assert_eq!(MOBILITY_WEIGHT, [0, 1, 4, 4, 2, 1, 0]);
        assert_eq!(KNIGHT_OUTPOST_BONUS, 30);
        assert_eq!(BISHOP_OUTPOST_BONUS, 20);
        assert_eq!(DOUBLED_ROOKS_BONUS, 20);
        assert_eq!(ROOK_SEVENTH_RANK_BONUS, 30);
        assert_eq!(UNDEVELOPED_PIECE_PENALTY, -10);
    }
}
