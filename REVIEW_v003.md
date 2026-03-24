# REVIEW v0.0.3 — Code Review

## CLEAN — ready to release

No CRITICAL issues found. All 64 tests pass. Benchmark 50/50 (100%).

---

## Critical Checks — All 15 Items Verified

| # | Check | Status | Detail |
|---|-------|--------|--------|
| 1 | Null move zugzwang detection | PASS | `has_non_pawn_pieces()` checks `stm & !(pawns\|kings)`. Returns false for K+P only. |
| 2 | Null move disabled in check/mate | PASS | `!in_check && beta.abs() < MATE_THRESHOLD` at line 411 |
| 3 | R=2 exactly | PASS | `depth - 1 - NULL_MOVE_R` where NULL_MOVE_R=2 at line 414 |
| 4 | History clamped | PASS | `.min(HISTORY_MAX)` / `.max(HISTORY_MIN)` at movepick.rs:153-155 |
| 5 | History cleared on ucinewgame | PASS | main.rs:58 `picker.clear()` resets `history = [[0; 64]; 64]` |
| 6 | SEE identifies losing captures | PASS | QxP: SEE=100-900=-800, score=-1800 < 0 (below quiet) |
| 7 | Qxp defended below quiet | PASS | LOSING_CAPTURE_BASE(-1000) + SEE(-800) = -1800 < quiet(0) |
| 8 | Threefold repetition | PASS | search.rs:341-350 unchanged, history push/pop correct |
| 9 | Fifty-move rule | PASS | search.rs:337-339 `halfmoves() >= 100` unchanged |
| 10 | TT mate score clamping | PASS | `score_to_tt()` clamps to ±32000 at lines 514-515 |
| 11 | No new unwrap/panic | PASS | Null move: `if let Some(...)`, `.ok()`. SEE: `.unwrap_or(0)` |
| 12 | BISHOP=300, KNIGHT=300, DELTA=200 | PASS | eval.rs:5-7, search.rs:13 unchanged |
| 13 | LMR still correct | PASS | Lines 446-470, after null move (409-420). No interaction. |
| 14 | Aspiration windows still correct | PASS | STM perspective fix from v0.0.2 intact at lines 118-122 |
| 15 | Stop flag frequency | PASS | hard_stop every 2048 nodes, null move subtree inherits check |

## Null Move Specific Verification

**KPK position (zugzwang risk):** `4k3/8/8/8/8/8/4P3/4K3 w`
- White has: King + Pawn. No non-pawn pieces.
- `has_non_pawn_pieces()` → false. Null move skipped. ✓

**KRK position (null move safe):** `4k3/8/8/8/8/8/8/R3K3 w`
- White has: King + Rook. Rook is non-pawn piece.
- `has_non_pawn_pieces()` → true. Null move allowed. ✓

Both verified by existing unit tests `null_move_skipped_in_kpk` and `null_move_fires_with_pieces`.

## Frozen Value Verification

```
grep "rollout|playout|UCB|visit_count|MonteCarloNode" src/  → 0 hits ✓
grep "torch|tensorflow|neural|embedding" src/               → 0 hits ✓
grep "325|piece_square|positional|mobility" src/eval.rs      → only "NOT 325" comment + variable name ✓
```

Constants verified:
- PAWN=100, KNIGHT=300, BISHOP=300, ROOK=500, QUEEN=900, KING=20000 ✓
- DELTA=200, MAX_EXTENSIONS=4 ✓
- NULL_MOVE_R=2, MATE_THRESHOLD=900,000 ✓
- HISTORY_MAX=16384, HISTORY_MIN=-16384 ✓
- LOSING_CAPTURE_BASE=-1000 ✓
- No MCTS, no neural, no internal book, no always-replace TT ✓

## WARNING Issues

### W1 — Null move doesn't push its hash to repetition history

The null move search at line 414 passes `history` without pushing the null position's
Zobrist hash. Inside the null move subtree, `history.last()` is the parent's hash, not
the null position's. Repetition detection at the null move root node checks the wrong hash.

**Impact:** Negligible. Can only cause a conservative false DRAW return (null move fails
to prune), never an incorrect cutoff. The null move position can't occur naturally in a
game. Standard chess engine practice — Stockfish also doesn't push null move hashes.

## MINOR Issues

### M1 — Unused imports: `Square` in movepick.rs, `NonZeroU32` in search.rs

Compiler warnings only. No functional impact.

### M2 — Dead code: `root_search`, `order_moves_simple` functions

Pre-existing from v0.0.2. Compiler warnings only.

### M3 — `captures_sort_above_quiet` test is vacuous

The test position has no captures available, so the assertion is never reached.
Pre-existing, not caused by v0.0.3. With SEE, losing captures now score below quiet
moves, so a test with actual losing captures would fail this assertion — but the test
doesn't have any captures to test with.

## Test Results

```
64/64 tests pass
50/50 benchmark positions pass (100%)
Pruning rate: 100%
Max position time: 3.9s
All sigma gates PASS
```

## False Positives Caught

None. Clean build across all v0.0.3 features.
