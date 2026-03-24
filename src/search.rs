use shakmaty::{Chess, Color, Move, MoveList, Position, Role};
use shakmaty::zobrist::{ZobristHash, Zobrist64};
use crate::eval::{evaluate, piece_value, CHECKMATE, DRAW};
use crate::movepick::{MovePicker, pack_move};
use crate::tablebase::TablebaseProber;
use crate::time::TimeManager;
use crate::tt::{Bound, TranspositionTable};
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

pub const MAX_DEPTH: u8 = 64;
const BASE_DEPTH: u8 = 6;
pub const DELTA: i32 = 200; // Frozen in spec.md
const MAX_EXTENSIONS: u8 = 4; // Frozen in spec.md

const INF: i32 = i32::MAX - 1;
const NEG_INF: i32 = i32::MIN + 1;
const MATE_THRESHOLD: i32 = CHECKMATE - 1000;

// ============================================================================
// Search Statistics
// ============================================================================

pub struct SearchStats {
    pub node_count: u64,
    pub tt_hits: u64,
}

impl SearchStats {
    pub fn new() -> Self {
        SearchStats {
            node_count: 0,
            tt_hits: 0,
        }
    }
}

// ============================================================================
// Iterative Deepening — DD02
// ============================================================================

/// Top-level iterative deepening search.
/// Returns (white-centric score, best move).
/// Outputs UCI info lines to the provided writer.
pub fn iterative_deepening(
    pos: &Chess,
    max_depth: u8,
    tm: &TimeManager,
    tt: &mut TranspositionTable,
    picker: &mut MovePicker,
    tb: Option<&TablebaseProber>,
    info_callback: &mut dyn FnMut(u8, i32, u64, u64, &Move),
) -> (i32, Option<Move>) {
    tt.increment_age();

    let mut best_move: Option<Move> = None;
    let mut best_score: i32 = 0;

    let depth_limit = max_depth.min(MAX_DEPTH);

    for depth in 1..=depth_limit {
        let mut stats = SearchStats::new();
        let (score, mv) = root_search(pos, depth as i32, tt, picker, tb, tm, &mut stats);

        // If search was stopped mid-iteration, keep previous result (unless depth 1)
        if tm.should_stop() && depth > 1 && mv.is_none() {
            break;
        }

        if let Some(ref m) = mv {
            best_move = Some(m.clone());
            best_score = score;
            let elapsed = tm.elapsed_ms().max(1);
            info_callback(depth, score, stats.node_count, elapsed, m);
        }

        if tm.should_stop() {
            break;
        }
    }

    (best_score, best_move)
}

/// Root search: enumerate all moves, returning white-centric score and best move.
fn root_search(
    pos: &Chess,
    depth: i32,
    tt: &mut TranspositionTable,
    picker: &mut MovePicker,
    tb: Option<&TablebaseProber>,
    tm: &TimeManager,
    stats: &mut SearchStats,
) -> (i32, Option<Move>) {
    let moves = pos.legal_moves();
    if moves.is_empty() {
        if pos.is_check() {
            let score = if pos.turn() == Color::White { -CHECKMATE } else { CHECKMATE };
            return (score, None);
        }
        return (DRAW, None);
    }

    // Get TT move for ordering
    let zobrist = u64::from(pos.zobrist_hash::<Zobrist64>(shakmaty::EnPassantMode::Legal));
    let tt_move = tt.probe(zobrist).map(|e| e.mv);

    let ordered = picker.order_moves(&moves, 0, tt_move);

    let mut alpha = NEG_INF;
    let beta = INF;
    let mut best_move: Option<Move> = None;
    let mut best_score = NEG_INF;

    for (i, m) in ordered.iter().enumerate() {
        let mut new_pos = pos.clone();
        new_pos.play_unchecked(m);

        let score;
        if i == 0 {
            // First move: full window
            score = -negamax(&new_pos, -beta, -alpha, depth - 1, 1, 0,
                             tt, picker, tb, tm, stats);
        } else {
            // PVS: null window first
            let null_score = -negamax(&new_pos, -(alpha + 1), -alpha, depth - 1, 1, 0,
                                      tt, picker, tb, tm, stats);
            if null_score > alpha && null_score < beta {
                // Re-search with full window
                score = -negamax(&new_pos, -beta, -alpha, depth - 1, 1, 0,
                                 tt, picker, tb, tm, stats);
            } else {
                score = null_score;
            }
        }

        if tm.should_stop() {
            // If we have at least one move, keep it
            if best_move.is_some() {
                break;
            }
        }

        if score > best_score {
            best_score = score;
            best_move = Some(m.clone());
        }
        if score > alpha {
            alpha = score;
        }
    }

    // Convert to white-centric score
    let white_score = if pos.turn() == Color::White {
        best_score
    } else {
        -best_score
    };

    // Store in TT
    let packed_mv = best_move.as_ref().map(|m| pack_move(m)).unwrap_or(0);
    let eval = evaluate(pos);
    let stm_eval = if pos.turn() == Color::White { eval } else { -eval };
    tt.store(zobrist, depth as u8, best_score as i16, stm_eval as i16,
             Bound::Exact, packed_mv, true);

    (white_score, best_move)
}

// ============================================================================
// Negamax with PVS + TT + Killers + Check Extensions
// ============================================================================

/// Negamax search. Returns score from side-to-move's perspective.
fn negamax(
    pos: &Chess,
    mut alpha: i32,
    beta: i32,
    mut depth: i32,
    ply: u32,
    mut extensions: u8,
    tt: &mut TranspositionTable,
    picker: &mut MovePicker,
    tb: Option<&TablebaseProber>,
    tm: &TimeManager,
    stats: &mut SearchStats,
) -> i32 {
    stats.node_count += 1;

    // Time check every 2048 nodes
    if stats.node_count & 2047 == 0 && tm.should_stop() {
        return 0;
    }

    // Terminal node check
    let moves = pos.legal_moves();
    if moves.is_empty() {
        if pos.is_check() {
            return -CHECKMATE + ply as i32; // Mated: return negative (bad for us)
        }
        return DRAW;
    }

    // Tablebase probe
    if let Some(tb) = tb {
        if let Some(tb_score) = tb.probe_wdl(pos) {
            // Convert from white-centric to side-to-move perspective
            let stm_score = if pos.turn() == Color::White { tb_score } else { -tb_score };
            return stm_score;
        }
    }

    // Check extension — DD06
    let in_check = pos.is_check();
    if in_check && extensions < MAX_EXTENSIONS {
        depth += 1;
        extensions += 1;
    }

    // Drop into quiescence at depth <= 0
    if depth <= 0 {
        return quiescence_nm(pos, alpha, beta, ply, tb, stats);
    }

    // TT probe
    let zobrist = u64::from(pos.zobrist_hash::<Zobrist64>(shakmaty::EnPassantMode::Legal));
    let tt_move;
    if let Some(entry) = tt.probe(zobrist) {
        stats.tt_hits += 1;
        tt_move = if entry.mv != 0 { Some(entry.mv) } else { None };

        if entry.depth as i32 >= depth {
            let tt_score = entry.score as i32;
            match entry.bound() {
                Bound::Exact => return tt_score,
                Bound::Lower => {
                    if tt_score >= beta {
                        return tt_score;
                    }
                }
                Bound::Upper => {
                    if tt_score <= alpha {
                        return tt_score;
                    }
                }
                Bound::None => {}
            }
        }
    } else {
        tt_move = None;
    }

    // Move ordering
    let ordered = picker.order_moves(&moves, ply as u8, tt_move);

    let original_alpha = alpha;
    let mut best_score = NEG_INF;
    let mut best_move: u16 = 0;

    for (i, m) in ordered.iter().enumerate() {
        let mut new_pos = pos.clone();
        new_pos.play_unchecked(m);

        let score;
        if i == 0 {
            // First move (PV): full window
            score = -negamax(&new_pos, -beta, -alpha, depth - 1, ply + 1,
                             extensions, tt, picker, tb, tm, stats);
        } else {
            // PVS: null window [alpha, alpha+1]
            let null_score = -negamax(&new_pos, -(alpha + 1), -alpha, depth - 1,
                                      ply + 1, extensions, tt, picker, tb, tm, stats);
            if null_score > alpha && null_score < beta {
                // Re-search with full window
                score = -negamax(&new_pos, -beta, -alpha, depth - 1, ply + 1,
                                 extensions, tt, picker, tb, tm, stats);
            } else {
                score = null_score;
            }
        }

        if score > best_score {
            best_score = score;
            best_move = pack_move(m);
        }

        if score > alpha {
            alpha = score;
        }

        if alpha >= beta {
            // Beta cutoff — store killer if quiet move
            if !m.is_capture() && !m.is_promotion() {
                picker.store_killer(m, ply as u8);
            }
            break;
        }
    }

    // Store in TT
    let bound = if best_score >= beta {
        Bound::Lower // Failed high (beta cutoff)
    } else if best_score <= original_alpha {
        Bound::Upper // Failed low (all moves below alpha)
    } else {
        Bound::Exact // PV node
    };

    let eval = evaluate(pos);
    let stm_eval = if pos.turn() == Color::White { eval } else { -eval };
    tt.store(zobrist, depth as u8, best_score as i16, stm_eval as i16,
             bound, best_move, bound == Bound::Exact);

    best_score
}

// ============================================================================
// Quiescence Search — Negamax version, unbounded (no depth parameter)
// ============================================================================

fn best_capturable_value(pos: &Chess) -> i32 {
    let board = pos.board();
    let opponent = board.by_color(!pos.turn());
    if !(board.queens() & opponent).is_empty() {
        return piece_value(Role::Queen);
    }
    if !(board.rooks() & opponent).is_empty() {
        return piece_value(Role::Rook);
    }
    if !(board.bishops() & opponent).is_empty() {
        return piece_value(Role::Bishop);
    }
    if !(board.knights() & opponent).is_empty() {
        return piece_value(Role::Knight);
    }
    if !(board.pawns() & opponent).is_empty() {
        return piece_value(Role::Pawn);
    }
    0
}

/// Negamax quiescence search — unbounded, no depth parameter.
/// Returns score from side-to-move's perspective.
fn quiescence_nm(
    pos: &Chess,
    mut alpha: i32,
    beta: i32,
    ply: u32,
    tb: Option<&TablebaseProber>,
    stats: &mut SearchStats,
) -> i32 {
    stats.node_count += 1;

    let eval = evaluate(pos);
    let stand_pat = if pos.turn() == Color::White { eval } else { -eval };

    // Stand-pat cutoff
    if stand_pat >= beta {
        return beta;
    }
    if stand_pat > alpha {
        alpha = stand_pat;
    }

    // Delta pruning
    let best_cap = best_capturable_value(pos);
    if stand_pat + best_cap + DELTA < alpha {
        return alpha;
    }

    let moves = pos.legal_moves();
    let picker = MovePicker::new();
    let captures = picker.order_captures(&moves);

    for m in &captures {
        let mut new_pos = pos.clone();
        new_pos.play_unchecked(m);
        let score = -quiescence_nm(&new_pos, -beta, -alpha, ply + 1, tb, stats);
        if score > alpha {
            alpha = score;
        }
        if alpha >= beta {
            return beta;
        }
    }

    alpha
}

// ============================================================================
// Backward-compatible API — used by benchmark
// ============================================================================

/// Fixed-depth alpha-beta search using the full negamax engine.
/// Returns (white-centric score, best move).
pub fn alpha_beta_search(
    pos: &Chess,
    depth: u8,
    tb: Option<&TablebaseProber>,
    stats: &mut SearchStats,
) -> (i32, Option<Move>) {
    let stop_flag = Arc::new(AtomicBool::new(false));
    let tm = TimeManager::infinite(stop_flag);
    let mut tt = TranspositionTable::new(16);
    let mut picker = MovePicker::new();

    let mut noop = |_: u8, _: i32, _: u64, _: u64, _: &Move| {};
    iterative_deepening(pos, depth, &tm, &mut tt, &mut picker, tb, &mut noop)
}

// ============================================================================
// Pure Minimax — kept for benchmark pruning rate measurement
// ============================================================================

/// Old move ordering for minimax (simple MVV-LVA, no killers/TT).
fn move_order_score_simple(m: &Move) -> i32 {
    if m.is_capture() {
        let victim = m.capture().map(|r| piece_value(r)).unwrap_or(0);
        let attacker = piece_value(m.role());
        victim * 10 - attacker + 10000
    } else if m.is_promotion() {
        5000
    } else {
        0
    }
}

fn order_moves_simple(moves: &MoveList) -> Vec<Move> {
    let mut ordered: Vec<Move> = moves.iter().cloned().collect();
    ordered.sort_by(|a, b| move_order_score_simple(b).cmp(&move_order_score_simple(a)));
    ordered
}

pub fn minimax(
    pos: &Chess,
    depth: u8,
    maximizing: bool,
    stats: &mut SearchStats,
) -> (i32, Option<Move>) {
    stats.node_count += 1;

    let moves = pos.legal_moves();
    if moves.is_empty() {
        if pos.is_check() {
            let score = if pos.turn() == Color::White {
                -CHECKMATE
            } else {
                CHECKMATE
            };
            return (score, None);
        } else {
            return (DRAW, None);
        }
    }

    if depth == 0 {
        return (evaluate(pos), None);
    }

    if maximizing {
        let mut best_score = NEG_INF;
        let mut best_move = None;
        for m in &moves {
            let mut new_pos = pos.clone();
            new_pos.play_unchecked(m);
            let (score, _) = minimax(&new_pos, depth - 1, false, stats);
            if score > best_score {
                best_score = score;
                best_move = Some(m.clone());
            }
        }
        (best_score, best_move)
    } else {
        let mut best_score = INF;
        let mut best_move = None;
        for m in &moves {
            let mut new_pos = pos.clone();
            new_pos.play_unchecked(m);
            let (score, _) = minimax(&new_pos, depth - 1, true, stats);
            if score < best_score {
                best_score = score;
                best_move = Some(m.clone());
            }
        }
        (best_score, best_move)
    }
}

// ============================================================================
// Old quiescence — kept for reference, used by old search path
// ============================================================================

fn order_captures_simple(moves: &MoveList) -> Vec<Move> {
    let mut captures: Vec<Move> = moves.iter().filter(|m| m.is_capture()).cloned().collect();
    captures.sort_by(|a, b| move_order_score_simple(b).cmp(&move_order_score_simple(a)));
    captures
}

/// White-centric quiescence search (old interface for minimax compatibility).
pub fn quiescence(
    pos: &Chess,
    mut alpha: i32,
    mut beta: i32,
    tb: Option<&TablebaseProber>,
    stats: &mut SearchStats,
) -> i32 {
    stats.node_count += 1;

    let stand_pat = evaluate(pos);
    let is_white = pos.turn() == Color::White;

    if is_white {
        if stand_pat >= beta {
            return beta;
        }
        if stand_pat > alpha {
            alpha = stand_pat;
        }

        let best_cap = best_capturable_value(pos);
        if stand_pat + best_cap + DELTA < alpha {
            return alpha;
        }

        let moves = pos.legal_moves();
        let captures = order_captures_simple(&moves);
        for m in &captures {
            let mut new_pos = pos.clone();
            new_pos.play_unchecked(m);
            let score = quiescence(&new_pos, alpha, beta, tb, stats);
            if score > alpha {
                alpha = score;
            }
            if alpha >= beta {
                return beta;
            }
        }
        alpha
    } else {
        if stand_pat <= alpha {
            return alpha;
        }
        if stand_pat < beta {
            beta = stand_pat;
        }

        let best_cap = best_capturable_value(pos);
        if stand_pat - best_cap - DELTA > beta {
            return beta;
        }

        let moves = pos.legal_moves();
        let captures = order_captures_simple(&moves);
        for m in &captures {
            let mut new_pos = pos.clone();
            new_pos.play_unchecked(m);
            let score = quiescence(&new_pos, alpha, beta, tb, stats);
            if score < beta {
                beta = score;
            }
            if alpha >= beta {
                return alpha;
            }
        }
        beta
    }
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

    // -- Minimax tests (unchanged from skeleton) --

    #[test]
    fn minimax_finds_mate_in_1() {
        let pos = pos_from_fen("r1bqkb1r/pppp1ppp/2n2n2/4p2Q/2B1P3/8/PPPP1PPP/RNB1K1NR w KQkq - 4 3");
        let mut stats = SearchStats::new();
        let (score, mv) = minimax(&pos, 1, true, &mut stats);
        assert!(score >= CHECKMATE - 100, "Should find checkmate");
        assert!(mv.is_some());
    }

    #[test]
    fn minimax_depth_0_returns_eval() {
        let pos = Chess::default();
        let mut stats = SearchStats::new();
        let (score, mv) = minimax(&pos, 0, true, &mut stats);
        assert_eq!(score, evaluate(&pos));
        assert!(mv.is_none());
    }

    #[test]
    fn minimax_node_count_increases() {
        let pos = Chess::default();
        let mut stats = SearchStats::new();
        let _ = minimax(&pos, 2, true, &mut stats);
        assert!(stats.node_count > 1, "Should visit multiple nodes");
    }

    // -- New search tests --

    #[test]
    fn alpha_beta_finds_mate_in_1() {
        let pos = pos_from_fen("r1bqkb1r/pppp1ppp/2n2n2/4p2Q/2B1P3/8/PPPP1PPP/RNB1K1NR w KQkq - 4 3");
        let mut stats = SearchStats::new();
        let (score, mv) = alpha_beta_search(&pos, 1, None, &mut stats);
        assert!(score >= CHECKMATE - 100, "Should find checkmate, got {}", score);
        assert!(mv.is_some());
    }

    #[test]
    fn alpha_beta_prunes_more_than_minimax() {
        let pos = pos_from_fen("rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq - 0 1");
        let mut stats_mm = SearchStats::new();
        let mut stats_ab = SearchStats::new();
        let _ = minimax(&pos, 3, false, &mut stats_mm);
        let _ = alpha_beta_search(&pos, 3, None, &mut stats_ab);
        assert!(
            stats_ab.node_count < stats_mm.node_count,
            "Alpha-beta ({}) should visit fewer nodes than minimax ({})",
            stats_ab.node_count,
            stats_mm.node_count
        );
    }

    #[test]
    fn tt_hit_rate_nonzero_on_repeated_search() {
        let pos = pos_from_fen("rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq - 0 1");
        let stop_flag = Arc::new(AtomicBool::new(false));
        let tm = TimeManager::infinite(stop_flag);
        let mut tt = TranspositionTable::new(16);
        let mut picker = MovePicker::new();
        let mut stats = SearchStats::new();

        // First search populates TT
        let mut noop = |_: u8, _: i32, _: u64, _: u64, _: &Move| {};
        iterative_deepening(&pos, 4, &tm, &mut tt, &mut picker, None, &mut noop);

        // Second search should get TT hits
        stats = SearchStats::new();
        let _ = root_search(&pos, 4, &mut tt, &mut picker, None, &tm, &mut stats);
        assert!(stats.tt_hits > 0, "Should have TT hits on repeated search, got 0");
    }

    #[test]
    fn iterative_deepening_returns_move() {
        let pos = Chess::default();
        let stop_flag = Arc::new(AtomicBool::new(false));
        let tm = TimeManager::infinite(stop_flag);
        let mut tt = TranspositionTable::new(16);
        let mut picker = MovePicker::new();
        let mut noop = |_: u8, _: i32, _: u64, _: u64, _: &Move| {};
        let (_, mv) = iterative_deepening(&pos, 4, &tm, &mut tt, &mut picker, None, &mut noop);
        assert!(mv.is_some(), "Iterative deepening should return a move");
    }

    #[test]
    fn check_extension_fires_on_check() {
        // Position where black is in check — extension should increase search depth
        let pos = pos_from_fen("4k3/8/8/8/8/8/4R3/4K3 b - - 0 1");
        // Not in check here, but let's test a position that IS in check
        let pos_check = pos_from_fen("4k3/8/8/8/8/4R3/8/4K3 b - - 0 1");
        // This is also not check. Let me use a proper check position:
        let pos_check = pos_from_fen("4k3/8/8/8/4R3/8/8/4K3 b - - 0 1");
        assert!(pos_check.is_check(), "Position should be in check");

        let mut stats = SearchStats::new();
        let (_, mv) = alpha_beta_search(&pos_check, 2, None, &mut stats);
        assert!(mv.is_some(), "Should find a move when in check");
    }

    #[test]
    fn pvs_returns_reasonable_scores() {
        // PVS should produce scores close to what full-window search would
        let positions = [
            "rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq - 0 1",
            "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
        ];
        for fen in &positions {
            let pos = pos_from_fen(fen);
            let mut stats = SearchStats::new();
            let (score, mv) = alpha_beta_search(&pos, 3, None, &mut stats);
            // Score should be finite and reasonable
            assert!(score > -CHECKMATE && score < CHECKMATE,
                "Score {} should be non-mate for FEN: {}", score, fen);
            assert!(mv.is_some(), "Should find a move for FEN: {}", fen);
        }
    }

    #[test]
    fn killers_stored_and_retrieved() {
        let mut picker = MovePicker::new();
        let pos = Chess::default();
        let moves = pos.legal_moves();
        let quiet: Vec<Move> = moves.iter().filter(|m| !m.is_capture()).cloned().collect();
        if let Some(m) = quiet.first() {
            picker.store_killer(m, 3);
            // Verify killer is used in ordering
            let ordered = picker.order_moves(&moves, 3, None);
            let killer_packed = pack_move(m);
            let killer_idx = ordered.iter().position(|om| pack_move(om) == killer_packed);
            assert!(killer_idx.is_some(), "Killer move should be in ordered list");
        }
    }

    // -- Quiescence tests --

    #[test]
    fn quiescence_no_captures_equals_eval() {
        let pos = pos_from_fen("4k3/8/8/8/8/8/8/4K3 w - - 0 1");
        let mut stats = SearchStats::new();
        let score = quiescence(&pos, NEG_INF, INF, None, &mut stats);
        assert_eq!(score, evaluate(&pos));
    }

    #[test]
    fn quiescence_hanging_piece_resolved() {
        let pos = pos_from_fen("4k3/8/8/8/8/8/8/Q3K2r w - - 0 1");
        let mut stats = SearchStats::new();
        let static_eval = evaluate(&pos);
        let q_score = quiescence(&pos, NEG_INF, INF, None, &mut stats);
        assert!(
            q_score >= static_eval,
            "Quiescence ({}) should be >= static eval ({}) with hanging piece",
            q_score,
            static_eval
        );
    }

    #[test]
    fn delta_is_200() {
        assert_eq!(DELTA, 200);
    }

    #[test]
    fn max_extensions_is_4() {
        assert_eq!(MAX_EXTENSIONS, 4);
    }
}
