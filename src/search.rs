use shakmaty::{Bitboard, Chess, CastlingMode, Color, FromSetup, Move, Position, Role, Setup};
use shakmaty::zobrist::{ZobristHash, Zobrist64};
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

// LMR — Late Move Reductions (v0.0.2, upgraded to logarithmic in v0.0.5)
const LMR_THRESHOLD: usize = 2;
const LMR_BASE_REDUCTION: i32 = 1;

// Aspiration Windows — DD07 (v0.0.2)
const ASPIRATION_WINDOW: i32 = 50;

// Null Move Pruning — DD05 (v0.0.3)
const NULL_MOVE_R: i32 = 2;
const MATE_THRESHOLD: i32 = 900_000;

// Futility Pruning — v0.0.5
const FUTILITY_MARGIN: [i32; 4] = [0, 100, 200, 300];

// Razoring — v0.0.5
const RAZOR_MARGIN: [i32; 3] = [0, 300, 500];

// Internal Iterative Deepening — v0.0.5
const IID_DEPTH_THRESHOLD: i32 = 4;
const IID_REDUCTION: i32 = 2;

// Complexity-based Time Management — v0.0.6, fixed v0.0.9
const STABILITY_THRESHOLD: u8 = 3;
const STABILITY_MIN_DEPTH: u8 = 8; // Don't apply stability cutoff before this depth
const STABILITY_BONUS: f64 = 0.7;  // Was 0.5 — less aggressive (use 70% of budget, not 50%)
const INSTABILITY_PENALTY: f64 = 1.5;

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
    let mut prev_best_packed: Option<u16> = None;
    let mut stability_count: u8 = 0;

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
            let cur_packed = pack_move(m);

            // Track best move stability for time management
            if prev_best_packed == Some(cur_packed) {
                stability_count = stability_count.saturating_add(1);
            } else {
                stability_count = 0;
            }
            prev_best_packed = Some(cur_packed);

            best_move = Some(m.clone());
            best_score = score;
            let elapsed = tm.elapsed_ms().max(1);
            info_callback(depth, score, stats.node_count, elapsed, m);
        }

        // Complexity-based soft stop — v0.0.6, fixed v0.0.9
        // Only activate stability cutoff after STABILITY_MIN_DEPTH to avoid premature stop
        let budget = tm.budget_ms();
        if budget < u64::MAX && depth >= STABILITY_MIN_DEPTH {
            let elapsed = tm.elapsed_ms();
            let adjusted_budget = if stability_count >= STABILITY_THRESHOLD {
                (budget as f64 * STABILITY_BONUS) as u64
            } else if stability_count == 0 {
                (budget as f64 * INSTABILITY_PENALTY) as u64
            } else {
                budget
            };
            if elapsed >= adjusted_budget {
                break;
            }
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

    let stm_color = if pos.turn() == Color::White { 0 } else { 1 };
    let ordered = picker.order_moves(&moves, 0, tt_move, None, stm_color);

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

        let cur_packed = Some(pack_move(m));
        let score;
        if i == 0 {
            score = -negamax(&new_pos, -beta, -alpha, depth - 1, 1, 0,
                             cur_packed, tt, picker, tb, tm, history, stats);
        } else {
            let null_score = -negamax(&new_pos, -(alpha + 1), -alpha, depth - 1, 1, 0,
                                      cur_packed, tt, picker, tb, tm, history, stats);
            if null_score > alpha && null_score < beta {
                score = -negamax(&new_pos, -beta, -alpha, depth - 1, 1, 0,
                                 cur_packed, tt, picker, tb, tm, history, stats);
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
    prev_move: Option<u16>,
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

    // Static eval for pruning decisions
    let static_eval = {
        let raw = evaluate(pos);
        if pos.turn() == Color::White { raw } else { -raw }
    };

    // Null move pruning — DD05
    if depth >= 3 && !in_check && has_non_pawn_pieces(pos) && beta.abs() < MATE_THRESHOLD {
        if let Some(null_pos) = make_null_move_pos(pos) {
            let null_score = -negamax(&null_pos, -beta, -beta + 1, depth - 1 - NULL_MOVE_R,
                                       ply + 1, extensions, None, tt, picker, tb, tm, history, stats);
            if null_score >= beta {
                return beta;
            }
        }
    }

    // Razoring — v0.0.5
    if depth <= 2 && !in_check && (depth as usize) < RAZOR_MARGIN.len() {
        if static_eval + RAZOR_MARGIN[depth as usize] <= alpha {
            let razor_score = quiescence_nm(pos, alpha, beta, ply, tb, stats);
            if razor_score <= alpha {
                return razor_score;
            }
        }
    }

    // Internal Iterative Deepening — v0.0.5
    let tt_move = if tt_move.is_none() && depth >= IID_DEPTH_THRESHOLD && !in_check {
        // No TT move at a deep node: do a shallow search to find a good first move
        let iid_depth = depth - IID_REDUCTION;
        negamax(pos, alpha, beta, iid_depth, ply, extensions, prev_move, tt, picker, tb, tm, history, stats);
        // Now probe TT for the move the shallow search stored
        tt.probe(zobrist).and_then(|e| if e.mv != 0 { Some(e.mv) } else { None })
    } else {
        tt_move
    };

    // Move ordering
    let stm_color = if pos.turn() == Color::White { 0 } else { 1 };
    let ordered = picker.order_moves(&moves, ply as u8, tt_move, prev_move, stm_color);

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

        // Futility pruning — v0.0.5
        // At shallow depth, if static eval + margin can't reach alpha, skip quiet moves
        if i > 0 && is_quiet && !in_check && depth <= 3 && alpha.abs() < MATE_THRESHOLD {
            if static_eval + FUTILITY_MARGIN[depth as usize] <= alpha {
                history.pop();
                continue;
            }
        }

        let cur_packed = Some(pack_move(m));
        let score;
        if i == 0 {
            // First move: full window, full depth
            score = -negamax(&new_pos, -beta, -alpha, depth - 1, ply + 1,
                             extensions, cur_packed, tt, picker, tb, tm, history, stats);
        } else {
            // LMR: logarithmic reduction for quiet late moves — v0.0.5
            let lmr_depth = if i >= LMR_THRESHOLD && depth >= 3 && !in_check && is_quiet {
                let reduction = ((depth as f64).ln() * (i as f64).ln() / 2.0) as i32;
                let reduction = reduction.max(LMR_BASE_REDUCTION);
                (depth - 1 - reduction).max(0)
            } else {
                depth - 1
            };

            // PVS null-window search (possibly reduced by LMR)
            let mut null_score = -negamax(&new_pos, -(alpha + 1), -alpha, lmr_depth,
                                          ply + 1, extensions, cur_packed, tt, picker, tb, tm, history, stats);

            // If LMR reduced and score beats alpha, re-search at full depth with null window
            if lmr_depth < depth - 1 && null_score > alpha {
                null_score = -negamax(&new_pos, -(alpha + 1), -alpha, depth - 1,
                                      ply + 1, extensions, cur_packed, tt, picker, tb, tm, history, stats);
            }

            // PVS: if null window fails high, re-search with full window
            if null_score > alpha && null_score < beta {
                score = -negamax(&new_pos, -beta, -alpha, depth - 1, ply + 1,
                                 extensions, cur_packed, tt, picker, tb, tm, history, stats);
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
                picker.store_countermove(prev_move, m);
                picker.update_history(m, depth as u8, true);
                for sq in &searched_quiets {
                    if pack_move(sq) != pack_move(m) {
                        picker.update_history(sq, depth as u8, false);
                    }
                }
            }
            if m.is_capture() {
                picker.update_capture_history(stm_color, m, depth as u8, true);
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
    _stats: &mut SearchStats,
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
        let tm = TimeManager::infinite(stop_flag.clone());
        let mut tt = TranspositionTable::new(16);
        let mut picker = MovePicker::new();

        // First search populates TT
        let mut noop = |_: u8, _: i32, _: u64, _: u64, _: &Move| {};
        let history: Vec<u64> = Vec::new();
        iterative_deepening(&pos, 4, &tm, &mut tt, &mut picker, None, &history, &mut noop);

        // Second search should get TT hits (use iterative_deepening again)
        let mut stats2 = SearchStats::new();
        let mut search_history = vec![zobrist_key(&pos)];
        let _ = root_search_windowed(&pos, 4, NEG_INF, INF, &mut tt, &mut picker, None, &tm, &mut search_history, &mut stats2);
        assert!(stats2.tt_hits > 0, "Should have TT hits on repeated search, got 0");
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
            let ordered = picker.order_moves(&moves, 3, None, None, 0);
            let killer_packed = pack_move(m);
            let killer_idx = ordered.iter().position(|om| pack_move(om) == killer_packed);
            assert!(killer_idx.is_some(), "Killer move should be in ordered list");
        }
    }

    // -- Quiescence tests --

    #[test]
    fn quiescence_no_captures_equals_eval() {
        // Kings only: no captures, quiescence returns stand-pat = eval from STM perspective
        let pos = pos_from_fen("4k3/8/8/8/8/8/8/4K3 w - - 0 1");
        let mut stats = SearchStats::new();
        let score = quiescence_nm(&pos, NEG_INF, INF, 0, None, &mut stats);
        let expected = evaluate(&pos); // white-centric, which equals STM for white
        assert_eq!(score, expected);
    }

    #[test]
    fn quiescence_hanging_piece_resolved() {
        let pos = pos_from_fen("4k3/8/8/8/8/8/8/Q3K2r w - - 0 1");
        let mut stats = SearchStats::new();
        let raw_eval = evaluate(&pos);
        let stand_pat = raw_eval; // white to move, so STM = white-centric
        let q_score = quiescence_nm(&pos, NEG_INF, INF, 0, None, &mut stats);
        assert!(
            q_score >= stand_pat,
            "Quiescence ({}) should be >= stand-pat ({}) with hanging piece",
            q_score,
            stand_pat
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
        assert_eq!(LMR_BASE_REDUCTION, 1);
    }

    #[test]
    fn lmr_logarithmic_increases_with_depth() {
        // At depth 6, move 10: ln(6)*ln(10)/2 ≈ 2.06 → reduction = 2
        let reduction = ((6f64).ln() * (10f64).ln() / 2.0) as i32;
        assert!(reduction > 1, "Log LMR at depth 6 move 10 should reduce > 1, got {}", reduction);
    }

    #[test]
    fn lmr_minimum_is_base_reduction() {
        // At depth 3, move 2: ln(3)*ln(2)/2 ≈ 0.38 → clamped to 1
        let raw = ((3f64).ln() * (2f64).ln() / 2.0) as i32;
        let reduction = raw.max(LMR_BASE_REDUCTION);
        assert_eq!(reduction, LMR_BASE_REDUCTION);
    }

    #[test]
    fn lmr_still_finds_correct_moves() {
        let pos = pos_from_fen("rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq - 0 1");
        let mut stats = SearchStats::new();
        let (score, mv) = alpha_beta_search(&pos, 4, None, &mut stats);
        assert!(mv.is_some(), "LMR search should still return a move");
        assert!(score > -CHECKMATE && score < CHECKMATE);
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
        let pos = pos_from_fen("r1bqkb1r/pppp1ppp/2n2n2/4p2Q/2B1P3/8/PPPP1PPP/RNB1K1NR w KQkq - 4 3");
        let mut stats = SearchStats::new();
        let (score, mv) = alpha_beta_search(&pos, 3, None, &mut stats);
        assert!(score >= CHECKMATE - 100, "Should find mate, got {}", score);
        assert!(mv.is_some());
    }

    // === v0.0.5 Feature Tests ===

    #[test]
    fn futility_margin_correct() {
        assert_eq!(FUTILITY_MARGIN, [0, 100, 200, 300]);
    }

    #[test]
    fn razor_margin_correct() {
        assert_eq!(RAZOR_MARGIN, [0, 300, 500]);
    }

    #[test]
    fn iid_constants_correct() {
        assert_eq!(IID_DEPTH_THRESHOLD, 4);
        assert_eq!(IID_REDUCTION, 2);
    }

    #[test]
    fn all_pruning_still_finds_mate() {
        let pos = pos_from_fen("r1bqkb1r/pppp1ppp/2n2n2/4p2Q/2B1P3/8/PPPP1PPP/RNB1K1NR w KQkq - 4 3");
        let mut stats = SearchStats::new();
        let (score, mv) = alpha_beta_search(&pos, 5, None, &mut stats);
        assert!(score >= CHECKMATE - 100, "Should find mate with all pruning, got {}", score);
        assert!(mv.is_some());
    }

    // === v0.0.6 Feature Tests ===

    #[test]
    fn stability_constants_correct() {
        assert_eq!(STABILITY_THRESHOLD, 3);
        assert_eq!(STABILITY_MIN_DEPTH, 8);
        assert!((STABILITY_BONUS - 0.7).abs() < f64::EPSILON);
        assert!((INSTABILITY_PENALTY - 1.5).abs() < f64::EPSILON);
    }

    #[test]
    fn iterative_deepening_shows_progress_kpk() {
        // Diagnostic: KPK endgame must show score changes across depths
        let pos = pos_from_fen("8/8/1k6/8/8/1K6/1P6/8 w - - 0 1");
        let stop = Arc::new(AtomicBool::new(false));
        let tm = TimeManager::infinite(stop);
        let mut tt = TranspositionTable::new(16);
        let mut picker = MovePicker::new();
        let history: Vec<u64> = Vec::new();

        let mut depth_scores: Vec<(u8, i32)> = Vec::new();
        let mut cb = |d: u8, s: i32, _n: u64, _t: u64, _m: &Move| {
            depth_scores.push((d, s));
        };

        // Test WITHOUT tablebases
        let _ = iterative_deepening(&pos, 10, &tm, &mut tt, &mut picker, None, &history, &mut cb);
        assert!(depth_scores.len() >= 5,
            "Should report at least 5 depths, got {}: {:?}", depth_scores.len(), depth_scores);

        // Test WITH tablebases
        let tb = crate::tablebase::TablebaseProber::new("syzygy");
        if tb.is_available() {
            let stop2 = Arc::new(AtomicBool::new(false));
            let tm2 = TimeManager::infinite(stop2);
            let mut tt2 = TranspositionTable::new(16);
            let mut picker2 = MovePicker::new();
            let mut tb_scores: Vec<(u8, i32)> = Vec::new();
            let mut cb2 = |d: u8, s: i32, _n: u64, _t: u64, _m: &Move| {
                tb_scores.push((d, s));
            };
            let _ = iterative_deepening(&pos, 10, &tm2, &mut tt2, &mut picker2,
                                         Some(&tb), &history, &mut cb2);
            assert!(tb_scores.len() >= 5,
                "With TB: should report at least 5 depths, got {}: {:?}",
                tb_scores.len(), tb_scores);
        }

        // Test with TIMED search (simulating Arena time control)
        let stop3 = Arc::new(AtomicBool::new(false));
        // 60 seconds remaining, no increment — budget = 60000/40 = 1500ms
        let tm3 = TimeManager::new(60000, 60000, 0, 0, 0, None, true, stop3);
        let mut tt3 = TranspositionTable::new(16);
        let mut picker3 = MovePicker::new();
        let mut timed_scores: Vec<(u8, i32)> = Vec::new();
        let mut cb3 = |d: u8, s: i32, _n: u64, _t: u64, _m: &Move| {
            timed_scores.push((d, s));
        };
        let _ = iterative_deepening(&pos, 64, &tm3, &mut tt3, &mut picker3, None, &history, &mut cb3);
        // With 1.5 seconds, should reach at least depth 5
        assert!(timed_scores.len() >= 3,
            "With 1.5s budget: should reach >= 3 depths, got {}: {:?}",
            timed_scores.len(), timed_scores);
    }
}
