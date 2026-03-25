# CHPawn-FrozenKing — Chess Logic Review
# CHP-BOUND: Every check in this file must be consistent with frozen/spec.md and dead_ends.md.
# Run this after CODE_REVIEW.md passes. This is the chess-specific logic layer.
# No release without CHESS_LOGIC_REVIEW passing.

cd D:\EXPERIMENTS\CHPawn-FrozenKing

Read these files FIRST in order:
1. frozen/spec.md
2. frozen/pst.rs
3. dead_ends.md
4. DECISIONS.md
5. src/eval.rs
6. src/search.rs
7. src/movepick.rs
8. src/time.rs
9. src/tablebase.rs
10. src/main.rs

Do not skip any file. Do not reorder. Read all 10 before writing anything.

================================================================================
SECTION 1 — CHP PROTOCOL VERIFICATION (HARD BLOCKERS)
================================================================================

These are non-negotiable. Any failure is a HARD BLOCK. Do not release.

CHP-1: NO MCTS
  grep -r "rollout\|playout\|UCB\|visit_count\|backpropagate\|MonteCarloNode\|c_puct\|num_simulations" src/
  Must return zero real hits. Comments do not count as hits.
  If hit found: HARD BLOCK. Log to dead_ends.md as new dead end.

CHP-2: NO NEURAL NETWORKS
  grep -r "tch\|burn\|candle\|tract\|onnxruntime\|torch\|tensorflow\|neural\|embedding\|Linear\|relu\|sigmoid" src/
  Must return zero real hits.
  If hit found: HARD BLOCK. Log to dead_ends.md.

CHP-3: NO INTERNAL OPENING BOOK
  grep -r "polyglot\|book\|opening_move\|book_move\|probe_book" src/
  Must return zero real hits (exclude comments).
  If hit found: HARD BLOCK. Opening book violates CCRL rules.

CHP-4: FROZEN PIECE VALUES — DO NOT CHANGE EVER
  Verify in src/eval.rs:
    PAWN   == 100  (NOT 90, NOT 110)
    KNIGHT == 300  (NOT 320, NOT 325)
    BISHOP == 300  (NOT 325, NOT 315)
    ROOK   == 500  (NOT 475, NOT 525)
    QUEEN  == 900  (NOT 875, NOT 950)
    KING   == 20000
  Any deviation: HARD BLOCK. Source: frozen/spec.md.

CHP-5: PST SOURCE INTEGRITY
  Verify frozen/pst.rs has NOT been modified.
  Spot check 10 values against Michniewski Simplified Evaluation Function:
    PST_KNIGHT[27] == 20  (d5 center)
    PST_KNIGHT[28] == 20  (e5 center)
    PST_KNIGHT[0]  == -50 (a8 corner)
    PST_PAWN[8]    == 50  (a7 rank 7)
    PST_BISHOP[0]  == -20 (a8 corner)
    PST_KING_MG[62] == 30 (g1 castled)
    PST_KING_EG[27] == 40 (d4 center endgame)
    PST_ROOK[8]     == 5  (7th rank)
  Any deviation: HARD BLOCK. PST values are immutable per CHP.

CHP-6: ALGORITHM INTEGRITY
  Verify the search is minimax + alpha-beta (NOT something else):
    - negamax function present in src/search.rs? YES required.
    - iterative_deepening present? YES required.
    - quiescence_nm present with NO depth parameter? YES required.
    - Any depth cap in quiescence? NO — dead end DE-6.
    - Null move skips K+P only positions? YES — dead end DE-7.
    - Futility pruning guarded by !in_check AND alpha < MATE_THRESHOLD? YES — dead end DE-8.

================================================================================
SECTION 2 — CHESS RULE CORRECTNESS
================================================================================

These verify the engine plays legal chess correctly.

CHESS-1: MOVE LEGALITY
  The engine uses shakmaty::Position::legal_moves() for all move generation.
  Verify: no manual move generation exists anywhere in src/
  grep -r "fn generate_moves\|fn pseudo_legal\|fn make_move" src/
  Must return zero hits — all move gen delegated to shakmaty.

CHESS-2: THREEFOLD REPETITION
  Verify threefold repetition is detected in src/search.rs negamax:
    - Zobrist hash history passed through entire search tree? YES required.
    - rep_count counts occurrences in history[..len-1]? YES required.
    - Returns DRAW when rep_count >= 2? YES required.
    - Test: position that would repeat 3 times returns DRAW score.
  Write and run this test:
    Position: "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1"
    History contains this hash twice already.
    Search should detect repetition and return 0 (DRAW).

CHESS-3: FIFTY MOVE RULE
  Verify in src/search.rs negamax:
    - pos.halfmoves() >= 100 check present at top of negamax? YES required.
    - Returns DRAW when triggered? YES required.
    - NOT applied at root (engine must still return a move)? YES required.
  Write and run this test:
    Position with halfmoves = 100: search returns DRAW score.
    Position with halfmoves = 99: search continues normally.

CHESS-4: CHECKMATE DETECTION
  Verify in src/search.rs:
    - legal_moves().is_empty() AND pos.is_check() returns -CHECKMATE + ply? YES required.
    - legal_moves().is_empty() AND NOT in_check returns DRAW (stalemate)? YES required.
    - CHECKMATE score is 1_000_000? YES required per frozen/spec.md.
    - Mate score adjusted by ply (shorter mate = higher score)? YES required.
  Write and run this test:
    Mate in 1 position: engine finds mate, score >= CHECKMATE - 100.
    Stalemate position: engine returns DRAW score exactly.

CHESS-5: EN PASSANT
  shakmaty handles en passant automatically via legal_moves().
  Verify: no manual en passant code exists.
  grep -r "en_passant\|enpassant\|ep_square" src/search.rs src/eval.rs src/movepick.rs
  Any manual en passant logic: WARNING (shakmaty should handle it).

CHESS-6: CASTLING
  shakmaty handles castling automatically.
  Verify in src/movepick.rs pack_move():
    Move::Castle { king, rook } is handled correctly.
    King square used as from, rook square used as to.
  Write and run this test:
    Position where castling is legal: castling move appears in ordered moves.
    Castling move packs and unpacks correctly through TT.

CHESS-7: PROMOTION
  Verify in src/movepick.rs:
    PROMOTION_SCORE == 5000.
    Promotions score above quiet moves but below winning captures.
    Under-promotion (to N/B/R) is handled — pack_move encodes promotion piece.
  Write and run this test:
    Position with pawn on 7th rank: promotion moves present in ordered list.
    Queen promotion scores higher than knight promotion in ordering.

================================================================================
SECTION 3 — EVALUATION LOGIC CORRECTNESS
================================================================================

EVAL-1: SYMMETRY
  Run: evaluate(starting_position) == 0
  Run: evaluate(kings_only) == 0
  For any position P, evaluate(P from white) == -evaluate(mirror(P) from black)
  This must hold. Asymmetric eval is a bug.

EVAL-2: ENDGAME DETECTION
  Verify: endgame = board.queens().count() == 0
  This is correct for basic detection.
  WARNING: This is simplistic — a position with queens on both sides
  but all other pieces gone is still "middlegame" by this logic.
  Log this as known limitation in DECISIONS.md if not already there.

EVAL-3: PASSED PAWN CORRECTNESS
  A passed pawn has NO enemy pawns on same file OR adjacent files AHEAD of it.
  Verify is_passed_pawn():
    - Checks same file ahead? YES required.
    - Checks adjacent files (file-1, file+1) ahead? YES required.
    - "Ahead" is correct direction for each color? YES required.
      White: ranks > current rank. Black: ranks < current rank.
  Write and run these tests:
    White pawn e5, black pawn e6: NOT passed (blocked same file).
    White pawn e5, black pawn d6: NOT passed (adjacent file blocks).
    White pawn e5, black pawn d4: PASSED (enemy pawn is BEHIND, not ahead).
    White pawn e5, no black pawns on d/e/f ahead: PASSED.

EVAL-4: KING SAFETY DIRECTION
  Verify for black king: shield pawns are on ranks BELOW the king (rank - 1).
  Verify for white king: shield pawns are on ranks ABOVE the king (rank + 1).
  A common bug: using rank+1 for both colors.
  Write and run this test:
    Black king on g8, black pawns on f7/g7/h7: shield bonus applied.
    White king on g1, white pawns on f2/g2/h2: shield bonus applied.
    Both positions: shield bonus == KING_SHIELD_BONUS * 3.

EVAL-5: ROOK FILE DISTINCTION
  Verify evaluate_rook_files():
    Open file: NO pawns of EITHER color on rook's file → ROOK_OPEN_FILE_BONUS.
    Semi-open: NO friendly pawns but ENEMY pawns on rook's file → ROOK_SEMI_OPEN_FILE_BONUS.
    Closed: friendly pawns on rook's file → NO bonus.
  A common bug: giving semi-open bonus when there are friendly pawns.
  Write and run these tests:
    White rook e1, no pawns on e file: open file bonus.
    White rook e1, black pawn e5, no white pawn e: semi-open bonus.
    White rook e1, white pawn e4: no bonus.

EVAL-6: OUTPOST DIRECTION
  Verify is_outpost():
    For WHITE: "enemy half" means rank >= 4 (ranks 5-8 in 1-indexed).
    For BLACK: "enemy half" means rank <= 3 (ranks 1-4 in 1-indexed).
    Enemy pawns that could attack the outpost: checked on CORRECT adjacent files
    in the CORRECT direction for each color.
  A common bug: using wrong rank threshold or wrong direction for black.

EVAL-7: MOBILITY EXCLUDES ENEMY PAWN ATTACKS
  Verify evaluate_mobility():
    White pieces: safe squares = attacked squares NOT defended by BLACK pawns.
    Black pieces: safe squares = attacked squares NOT defended by WHITE pawns.
    A common bug: using wrong color's pawn attacks for each side.
  Write and run this test:
    White knight on e4 with black pawns on d5/f5 controlling center:
    Mobility lower than knight on e4 with no enemy pawns nearby.

================================================================================
SECTION 4 — SEARCH LOGIC CORRECTNESS
================================================================================

SEARCH-1: NEGAMAX SCORE PERSPECTIVE
  Verify: negamax always returns score from SIDE-TO-MOVE perspective.
  Verify: evaluate(pos) returns WHITE perspective.
  Verify: conversion in negamax: if black to move, negate eval.
  A common bug: mixing white-centric and STM-centric scores.
  Write and run this test:
    From white's perspective, winning position scores positive.
    Same position with black to move: negamax returns negative (bad for black).

SEARCH-2: TT SCORE INTEGRITY
  Verify score_to_tt() clamps to [-32000, 32000]:
    CHECKMATE = 1_000_000 overflows i16 (max 32767).
    Scores must be clamped before storing.
  Verify: when retrieving from TT, clamped scores don't corrupt search.
  Write and run this test:
    Store mate score in TT: retrieved score is clamped, not overflowed.
    Normal eval scores (~±500): stored and retrieved exactly.

SEARCH-3: NULL MOVE CORRECTNESS
  Verify make_null_move_pos():
    - Turn flipped? YES required.
    - Board unchanged? YES required.
    - En passant cleared? YES required (no ep after null move).
    - Castling rights preserved? YES required.
  Verify null move R=2 exactly per frozen spec.
  Verify null move not applied: in check, K+P only, mate score beta.

SEARCH-4: LMR CORRECTNESS
  Verify LMR conditions:
    - move_index >= LMR_THRESHOLD (2)? YES required.
    - depth >= 3? YES required.
    - NOT in check? YES required.
    - Move is quiet (not capture, not promotion)? YES required.
  Verify formula: max(LMR_BASE_REDUCTION, (ln(depth) * ln(move_index) / 2) as i32)
  Verify: re-search at full depth if reduced score > alpha.
  A common bug: applying LMR to captures or check positions.

SEARCH-5: ASPIRATION WINDOW CORRECTNESS
  Verify: depth 1 always uses full window (NEG_INF, INF).
  Verify: fail-low widens alpha, fail-high widens beta.
  Verify: window >= 800cp falls back to full window.
  Verify: white-centric score compared against white-centric asp bounds.
  A common bug: comparing STM score against white-centric bounds when black moves.

SEARCH-6: QUIESCENCE CORRECTNESS
  Verify quiescence_nm():
    - Stand-pat cutoff: if stand_pat >= beta return beta? YES required.
    - Only searches CAPTURES (not quiet moves)? YES required.
    - Delta pruning: stand_pat + best_cap + DELTA < alpha → return alpha? YES required.
    - DELTA == 200 per frozen spec? YES required.
    - NO depth parameter (unbounded per dead end DE-6)? YES required.

SEARCH-7: MOVE ORDERING COMPLETENESS
  Verify order_moves() ordering from highest to lowest:
    1. TT move (i32::MAX)
    2. Winning captures (MVV-LVA + CAPTURE_BASE = 10000+)
    3. Killer moves (KILLER_SCORE = 9000)
    4. Countermoves (COUNTERMOVE_SCORE = 8000)
    5. Quiet moves with history bonus
    6. Losing captures (LOSING_CAPTURE_BASE = -1000 + SEE)
  Verify: losing captures score BELOW all quiet moves (score < 0).
  Write and run this test:
    Queen takes defended pawn (SEE = -800): scores below quiet moves.
    Pawn takes queen (SEE = +800): scores above all other moves.

================================================================================
SECTION 5 — UCI PROTOCOL CORRECTNESS
================================================================================

UCI-1: REQUIRED COMMANDS
  Verify main.rs handles all required UCI commands:
    - "uci" → outputs id name, id author, options, uciok
    - "isready" → outputs readyok
    - "ucinewgame" → resets position, TT, picker, history
    - "position startpos moves ..." → parses correctly
    - "position fen <fen> moves ..." → parses correctly
    - "go wtime btime movestogo movetime depth" → all params parsed
    - "go winc binc" → parsed and used in time management
    - "stop" → sets stop flag immediately
    - "quit" → exits cleanly

UCI-2: BESTMOVE OUTPUT
  Verify: bestmove always output after go command.
  Verify: bestmove format is lowercase UCI algebraic (e2e4, not E2E4).
  Verify: if no legal moves found, outputs "bestmove 0000" (not crash).

UCI-3: INFO LINES
  Verify info lines output during search:
    format: "info depth N score cp N nodes N time N nps N pv <move>"
  Verify: score is in centipawns (cp), not internal units.
  Verify: nps calculated correctly (nodes * 1000 / time_ms).

UCI-4: HASH OPTION
  Verify setoption name Hash value N → calls tt.resize(N).
  Verify: TT size defaults to 64MB.
  Verify: valid range 1-65536 MB accepted.

================================================================================
SECTION 6 — SIGMA GATE (MATHEMATICAL PROOF)
================================================================================

This is the CHP proof layer. Cannot be weakened. Cannot be removed.

SIGMA-1: RUN THE BENCHMARK
  cargo run --release --bin benchmark 2>&1
  
  Must pass all gates:
    GATE 1: illegal_moves == 0          HARD BLOCKER — illegal move = disqualified from CCRL
    GATE 2: pass_rate >= 90%            50/50 minimum (100% target)
    GATE 3: pruning_rate >= 50%         alpha-beta working
    GATE 4: max_time < 900s             performance acceptable

SIGMA-2: ENDGAME TYPES COVERED
  Verify the benchmark tests all 7 endgame types:
    KQvK   — queen vs king
    KRvK   — rook vs king
    KQvKR  — queen vs rook
    KBBvK  — two bishops vs king
    KBNvK  — bishop and knight vs king (hardest — DTZ up to 53)
    KQvKB  — queen vs bishop
    KQvKN  — queen vs knight
  All 7 must be present. This is the formal verification claim.

SIGMA-3: SYZYGY FILES PRESENT
  Verify syzygy/ directory contains at minimum:
    KQvK.rtbw, KQvK.rtbz
    KRvK.rtbw, KRvK.rtbz
    KQvKR.rtbw, KQvKR.rtbz
    KBBvK.rtbw, KBBvK.rtbz
    KBNvK.rtbw, KBNvK.rtbz
    KQvKB.rtbw, KQvKB.rtbz
    KQvKN.rtbw, KQvKN.rtbz
  Any missing file: BLOCKING.

SIGMA-4: PROOF STATEMENT
  If all sigma gates pass, include this in CHESS_LOGIC_REVIEW_vXXX.md:
  
  "SIGMA GATE VERIFIED: CHPawn plays mathematically perfect chess in all
   tested endgame positions. 50/50 positions. 7 endgame types. Zero illegal
   moves. This is a formal proof, not a benchmark. The ground truth is
   Syzygy tablebases — retrograde analysis from every possible checkmate.
   Anyone can reproduce this result."

================================================================================
SECTION 7 — KNOWN LIMITATIONS LOG
================================================================================

These are known weaknesses. Document them but do not block release.

LIMIT-1: Endgame detection uses queens==0 only.
  Impact: PST transition may be abrupt in some positions.
  Status: Known. Will improve in future version.

LIMIT-2: King safety only counts adjacent pieces, not distant attackers.
  Impact: Long-range battery attacks (rook+queen on open file) not penalized.
  Status: Known. More sophisticated king safety deferred.

LIMIT-3: No pawn hash table.
  Impact: Pawn structure recalculated every node (slow).
  Status: Known. Pawn hash table deferred to future version.

LIMIT-4: Mobility calculated on every node including quiescence.
  Impact: Expensive. Slows search.
  Status: Known. Fast/slow eval split deferred.

LIMIT-5: No tapered evaluation.
  Impact: Abrupt transition between middlegame and endgame PST.
  Status: Known. Tapered eval deferred.

================================================================================
WRITE REVIEW FILE
================================================================================

Write CHESS_LOGIC_REVIEW_vXXX.md with this structure:

---
# CHPawn-FrozenKing vXXX — Chess Logic Review

## CHP Protocol Status
VERIFIED — all CHP constraints hold
OR
VIOLATED — [list violations — HARD BLOCK]

## Section 1 — CHP Verification
[CHP-1 through CHP-6: PASS or HARD BLOCK]

## Section 2 — Chess Rule Correctness
[CHESS-1 through CHESS-7: PASS, WARNING, or FAIL]

## Section 3 — Evaluation Logic
[EVAL-1 through EVAL-7: PASS, WARNING, or FAIL]

## Section 4 — Search Logic
[SEARCH-1 through SEARCH-7: PASS, WARNING, or FAIL]

## Section 5 — UCI Protocol
[UCI-1 through UCI-4: PASS or FAIL]

## Section 6 — Sigma Gate
[SIGMA-1 through SIGMA-4: PASS or HARD BLOCK]
[Include proof statement if 50/50 passes]

## Section 7 — Known Limitations
[List active limitations from LIMIT-1 through LIMIT-5]

## Issues Found
HARD BLOCK: [list — must fix, cannot release]
CRITICAL:   [list — must fix before release]
WARNING:    [list — should fix]
MINOR:      [list — nice to fix]

## Verdict
CHESS LOGIC VERIFIED — ready to release
OR
NOT VERIFIED — [blocking issues listed above]
---

================================================================================
RELEASE CRITERIA
================================================================================

Only release if ALL of the following are true:
  - Section 1: All CHP checks pass (zero HARD BLOCKS)
  - Section 2: All chess rules correct (no FAIL)
  - Section 3: All eval logic correct (no FAIL)
  - Section 4: All search logic correct (no FAIL)
  - Section 5: All UCI commands correct (no FAIL)
  - Section 6: 50/50 sigma gate (HARD BLOCK if fails)
  - Verdict: CHESS LOGIC VERIFIED

Stop when CHESS_LOGIC_REVIEW_vXXX.md is written and verdict is VERIFIED.
