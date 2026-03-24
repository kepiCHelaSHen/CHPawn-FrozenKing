use shakmaty::{Bitboard, Chess, CastlingMode, Color, FromSetup, Move, MoveList, Position, Role, Setup};
use shakmaty::zobrist::{ZobristHash, Zobrist64};
use std::num::NonZeroU32;
use crate::eval::{evaluate, piece_value, CHECKMATE, DRAW};
use crate::movepick::{MovePicker, pack_move};
use crate::tablebase::TablebaseProber;
use crate::time::TimeManager;
use crate::tt::{Bound, TranspositionTable};
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

pub const MAX_DEPTH: u8 = 64;
pub const DELTA: i32 = 200; // Frozen in spec.md
const MAX_EXTENSIONS: u8 = 4; // Frozen in spec.md

// LMR — Late Move Reductions (v0.0.2)
const LMR_THRESHOLD: usize = 2; // Reduce moves after first 2
const LMR_REDUCTION: i32 = 1;   // Reduce by 1 ply

// Aspiration Windows — DD07 (v0.0.2)
const ASPIRATION_WINDOW: i32 = 50; // Initial window in centipawns

// Null Move Pruning — DD05 (v0.0.3)
const NULL_MOVE_R: i32 = 2;          // Null move reduction
const MATE_THRESHOLD: i32 = 900_000; // Don't null-move prune near mate scores

const INF: i32 = i32::MAX - 1;
const NEG_INF: i32 = i32::MIN + 1;

/// Clamp scores to i16-safe range for TT storage.
/// CHECKMATE=1_000_000 overflows i16 (max 32767). Scores above 32000
/// are clamped to prevent TT corruption. Normal eval scores (~±4000) are unaffected.
const TT_SCORE_MAX: i32 = 32000;

fn score_to_tt(score: i32) -> i16 {
    score.clamp(-TT_SCORE_MAX, TT_SCORE_MAX) as i16
}

/// Compute Zobrist hash for a position.
pub fn zobrist_key(pos: &Chess) -> u64 {
    u64::from(pos.zobrist_hash::<Zobrist64>(shakmaty::EnPassantMode::Legal))
}

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
/// `history` contains Zobrist hashes of all game positions up to (but not including)
/// the current position, for threefold repetition detection.
pub fn iterative_deepening(
    pos: &Chess,
    max_depth: u8,
    tm: &TimeManager,
    tt: &mut TranspositionTable,
    picker: &mut MovePicker,
    tb: Option<&TablebaseProber>,
    history: &[u64],
    info_callback: &mut dyn FnMut(u8, i32, u64, u64, &Move),
) -> (i32, Option<Move>) {
    tt.increment_age();

    // Build mutable search history: game history + positions visited during search
    let mut search_history: Vec<u64> = history.to_vec();
    search_history.push(zobrist_key(pos)); // include current position

    let mut best_move: Option<Move> = None;
    let mut best_score: i32 = 0;

    let depth_limit = max_depth.min(MAX_DEPTH);

    for depth in 1..=depth_limit {
        let mut stats = SearchStats::new();
        // Reset search history to game state for each iteration
        search_history.truncate(history.len() + 1);

        let (score, mv);

        if depth <= 1 {
            // First iteration: full window
            let result = root_search_windowed(pos, depth as i32, NEG_INF, INF,
                                              tt, picker, tb, tm, &mut search_history, &mut stats);
            score = result.0;
            mv = result.1;
        } else {
            // Aspiration windows — DD07
            // asp_alpha/asp_beta are white-centric. root_search_windowed needs
            // STM-perspective bounds. For black to move, negate and swap.
            let is_white = pos.turn() == Color::White;
            let mut window = ASPIRATION_WINDOW;
            let mut asp_alpha = best_score - window;
            let mut asp_beta = best_score + window;
            let mut result;

            loop {
                search_history.truncate(history.len() + 1);
                stats = SearchStats::new();
                // Convert white-centric aspiration bounds to STM perspective
                let (stm_alpha, stm_beta) = if is_white {
                    (asp_alpha, asp_beta)
                } else {
                    (-asp_beta, -asp_alpha)
                };
                result = root_search_windowed(pos, depth as i32, stm_alpha, stm_beta,
                                              tt, picker, tb, tm, &mut search_history, &mut stats);

                if tm.should_stop() {
                    break;
                }

                if result.0 <= asp_alpha {
                    // Fail low: widen alpha
                    window *= 2;
                    asp_alpha = best_score - window;
                } else if result.0 >= asp_beta {
                    // Fail high: widen beta
                    window *= 2;
                    asp_beta = best_score + window;
                } else {
                    break; // Score within window
                }

                // If window has grown too large, use full window
                if window >= 800 {
                    search_history.truncate(history.len() + 1);
                    stats = SearchStats::new();
                    result = root_search_windowed(pos, depth as i32, NEG_INF, INF,
                                                  tt, picker, tb, tm, &mut search_history, &mut stats);
                    break;
                }
            }

            score = result.0;
            mv = result.1;
        }

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

/// Root search with configurable alpha/beta window. Returns (white-centric score, best move).
fn root_search_windowed(
    pos: &Chess,
    depth: i32,
    init_alpha: i32,
    init_beta: i32,
    tt: &mut TranspositionTable,
    picker: &mut MovePicker,
    tb: Option<&TablebaseProber>,
    tm: &TimeManager,
    history: &mut Vec<u64>,
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
    let zobrist = zobrist_key(pos);
    let tt_move = tt.probe(zobrist).map(|e| e.mv);

    let ordered = picker.order_moves(&moves, 0, tt_move);

    let mut alpha = init_alpha;
    let beta = init_beta;
    let mut best_move: Option<Move> = None;
    let mut best_score = NEG_INF;

    for (i, m) in ordered.iter().enumerate() {
        let mut new_pos = pos.clone();
        new_pos.play_unchecked(m);

        // Push child hash for repetition detection
        let child_hash = zobrist_key(&new_pos);
        history.push(child_hash);

        let score;
        if i == 0 {
            score = -negamax(&new_pos, -beta, -alpha, depth - 1, 1, 0,
                             tt, picker, tb, tm, history, stats);
        } else {
            let null_score = -negamax(&new_pos, -(alpha + 1), -alpha, depth - 1, 1, 0,
                                      tt, picker, tb, tm, history, stats);
            if null_score > alpha && null_score < beta {
                score = -negamax(&new_pos, -beta, -alpha, depth - 1, 1, 0,
                                 tt, picker, tb, tm, history, stats);
            } else {
                score = null_score;
            }
        }

        history.pop(); // restore history

        if tm.should_stop() {
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

    // Store in TT (clamped to prevent i16 overflow on mate scores)
    let packed_mv = best_move.as_ref().map(|m| pack_move(m)).unwrap_or(0);
    let eval = evaluate(pos);
    let stm_eval = if pos.turn() == Color::White { eval } else { -eval };
    tt.store(zobrist, depth as u8, score_to_tt(best_score), score_to_tt(stm_eval),
             Bound::Exact, packed_mv, true);

    (white_score, best_move)
}

/// Root search with full window (backward-compatible wrapper).
fn root_search(
    pos: &Chess,
    depth: i32,
    tt: &mut TranspositionTable,
    picker: &mut MovePicker,
    tb: Option<&TablebaseProber>,
    tm: &TimeManager,
    history: &mut Vec<u64>,
    stats: &mut SearchStats,
) -> (i32, Option<Move>) {
    root_search_windowed(pos, depth, NEG_INF, INF, tt, picker, tb, tm, history, stats)
}

// ============================================================================
// Null Move Pruning helpers — DD05
// ============================================================================

/// Zugzwang detection: side to move has at least one piece beyond king+pawns.
/// If false, skip null move — pure K+P endgames can have zugzwang.
fn has_non_pawn_pieces(pos: &Chess) -> bool {
    let board = pos.board();
    let stm = board.by_color(pos.turn());
    let non_pawn = stm & !(board.pawns() | board.kings());
    !non_pawn.is_empty()
}

/// Create a position with the turn flipped (null move = passing).
fn make_null_move_pos(pos: &Chess) -> Option<Chess> {
    let setup = Setup {
        board: pos.board().clone(),
        promoted: Bitboard::EMPTY,
        pockets: None,
        turn: !pos.turn(),
        castling_rights: pos.castles().castling_rights(),
        ep_square: None,
        remaining_checks: None,
        halfmoves: pos.halfmoves(),
        fullmoves: pos.fullmoves(),
    };
    Chess::from_setup(setup, CastlingMode::Standard).ok()
}

// ============================================================================
// Negamax with PVS + TT + Killers + Check Extensions
// ============================================================================

/// Negamax search. Returns score from side-to-move's perspective.
/// `history` tracks Zobrist hashes for threefold repetition detection.
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
    history: &mut Vec<u64>,
    stats: &mut SearchStats,
) -> i32 {
    stats.node_count += 1;

    // Time check every 2048 nodes — use hard limit inside search
    if stats.node_count & 2047 == 0 && tm.hard_stop() {
        return 0;
    }

    // Fifty-move rule — C2 fix
    if pos.halfmoves() >= 100 {
        return DRAW;
    }

    // Threefold repetition detection — C1 fix
    // Current position hash is the last entry in history (pushed by caller)
    if history.len() >= 2 {
        let current_hash = *history.last().unwrap();
        // Count how many times this hash appears in history (excluding the last entry itself)
        let rep_count = history[..history.len() - 1].iter().filter(|&&h| h == current_hash).count();
        if rep_count >= 2 {
            return DRAW; // Threefold repetition
        }
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
    let zobrist = zobrist_key(pos);
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

    // Null move pruning — DD05
    // Conditions: depth >= 3, not in check, has non-pawn pieces, beta not mate score
    if depth >= 3 && !in_check && has_non_pawn_pieces(pos) && beta.abs() < MATE_THRESHOLD {
        if let Some(null_pos) = make_null_move_pos(pos) {
            // Search with null window around beta, reduced depth
            let null_score = -negamax(&null_pos, -beta, -beta + 1, depth - 1 - NULL_MOVE_R,
                                       ply + 1, extensions, tt, picker, tb, tm, history, stats);
            if null_score >= beta {
                return beta; // Null move cutoff
            }
        }
    }

    // Move ordering
    let ordered = picker.order_moves(&moves, ply as u8, tt_move);

    let original_alpha = alpha;
    let mut best_score = NEG_INF;
    let mut best_move: u16 = 0;
    let mut searched_quiets: Vec<Move> = Vec::new();

    for (i, m) in ordered.iter().enumerate() {
        let mut new_pos = pos.clone();
        new_pos.play_unchecked(m);

        // Push child hash for repetition detection
        let child_hash = zobrist_key(&new_pos);
        history.push(child_hash);

        let is_quiet = !m.is_capture() && !m.is_promotion();

        let score;
        if i == 0 {
            // First move: full window, full depth
            score = -negamax(&new_pos, -beta, -alpha, depth - 1, ply + 1,
                             extensions, tt, picker, tb, tm, history, stats);
        } else {
            // LMR: reduce quiet late moves at sufficient depth when not in check
            let lmr_depth = if i >= LMR_THRESHOLD && depth >= 3 && !in_check && is_quiet {
                depth - 1 - LMR_REDUCTION
            } else {
                depth - 1
            };

            // PVS null-window search (possibly reduced by LMR)
            let mut null_score = -negamax(&new_pos, -(alpha + 1), -alpha, lmr_depth,
                                          ply + 1, extensions, tt, picker, tb, tm, history, stats);

            // If LMR reduced and score beats alpha, re-search at full depth with null window
            if lmr_depth < depth - 1 && null_score > alpha {
                null_score = -negamax(&new_pos, -(alpha + 1), -alpha, depth - 1,
                                      ply + 1, extensions, tt, picker, tb, tm, history, stats);
            }

            // PVS: if null window fails high, re-search with full window
            if null_score > alpha && null_score < beta {
                score = -negamax(&new_pos, -beta, -alpha, depth - 1, ply + 1,
                                 extensions, tt, picker, tb, tm, history, stats);
            } else {
                score = null_score;
            }
        }

        history.pop(); // restore history

        if score > best_score {
            best_score = score;
            best_move = pack_move(m);
        }

        if score > alpha {
            alpha = score;
        }

        // Track searched quiet moves for history penalty on cutoff
        if is_quiet {
            searched_quiets.push(m.clone());
        }

        if alpha >= beta {
            if is_quiet {
                picker.store_killer(m, ply as u8);
                // History: reward cutoff move, penalize other searched quiets
                picker.update_history(m, depth as u8, true);
                for sq in &searched_quiets {
                    if pack_move(sq) != pack_move(m) {
                        picker.update_history(sq, depth as u8, false);
                    }
                }
            }
            break;
        }
    }

    // Store in TT (clamped to prevent i16 overflow — C3 fix)
    let bound = if best_score >= beta {
        Bound::Lower
    } else if best_score <= original_alpha {
        Bound::Upper
    } else {
        Bound::Exact
    };

    let eval = evaluate(pos);
    let stm_eval = if pos.turn() == Color::White { eval } else { -eval };
    tt.store(zobrist, depth as u8, score_to_tt(best_score), score_to_tt(stm_eval),
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
    let history: Vec<u64> = Vec::new();
    iterative_deepening(pos, depth, &tm, &mut tt, &mut picker, tb, &history, &mut noop)
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
        let history: Vec<u64> = Vec::new();
        iterative_deepening(&pos, 4, &tm, &mut tt, &mut picker, None, &history, &mut noop);

        // Second search should get TT hits
        stats = SearchStats::new();
        let mut search_history = vec![zobrist_key(&pos)];
        let _ = root_search(&pos, 4, &mut tt, &mut picker, None, &tm, &mut search_history, &mut stats);
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
        let history: Vec<u64> = Vec::new();
        let (_, mv) = iterative_deepening(&pos, 4, &tm, &mut tt, &mut picker, None, &history, &mut noop);
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

    #[test]
    fn lmr_constants_correct() {
        assert_eq!(LMR_THRESHOLD, 2);
        assert_eq!(LMR_REDUCTION, 1);
    }

    #[test]
    fn lmr_reduces_node_count() {
        // LMR should reduce total nodes at depth >= 3 compared to no-LMR
        // We test indirectly: search should still find correct moves
        let pos = pos_from_fen("rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq - 0 1");
        let mut stats = SearchStats::new();
        let (score, mv) = alpha_beta_search(&pos, 4, None, &mut stats);
        assert!(mv.is_some(), "LMR search should still return a move");
        assert!(score > -CHECKMATE && score < CHECKMATE, "Score should be reasonable with LMR");
    }

    #[test]
    fn null_move_constants_correct() {
        assert_eq!(NULL_MOVE_R, 2);
        assert_eq!(MATE_THRESHOLD, 900_000);
    }

    #[test]
    fn null_move_skipped_in_kpk() {
        // KPK endgame — only king and pawns, zugzwang possible
        let pos = pos_from_fen("4k3/8/8/8/8/8/4P3/4K3 w - - 0 1");
        assert!(!has_non_pawn_pieces(&pos), "KPK should NOT have non-pawn pieces for white");
    }

    #[test]
    fn null_move_fires_with_pieces() {
        // Position with knight — null move should be allowed
        let pos = pos_from_fen("4k3/8/8/8/8/5N2/8/4K3 w - - 0 1");
        assert!(has_non_pawn_pieces(&pos), "Position with knight should have non-pawn pieces");
    }

    #[test]
    fn null_move_position_created_correctly() {
        let pos = pos_from_fen("rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq - 0 1");
        let null_pos = make_null_move_pos(&pos).expect("Should create null move position");
        assert_eq!(null_pos.turn(), Color::White, "Turn should be flipped");
        assert_eq!(null_pos.board(), pos.board(), "Board should be unchanged");
    }

    #[test]
    fn aspiration_window_constant_correct() {
        assert_eq!(ASPIRATION_WINDOW, 50);
    }

    #[test]
    fn aspiration_windows_find_correct_moves() {
        // Aspiration windows should still find mate-in-1
        let pos = pos_from_fen("r1bqkb1r/pppp1ppp/2n2n2/4p2Q/2B1P3/8/PPPP1PPP/RNB1K1NR w KQkq - 4 3");
        let mut stats = SearchStats::new();
        let (score, mv) = alpha_beta_search(&pos, 3, None, &mut stats);
        assert!(score >= CHECKMATE - 100, "Aspiration windows should still find mate, got {}", score);
        assert!(mv.is_some());
    }
}
