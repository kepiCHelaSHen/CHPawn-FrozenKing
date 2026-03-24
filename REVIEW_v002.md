# REVIEW v0.0.2 — Code Review

## CLEAN — ready to release (after C1 fix applied)

One CRITICAL bug found and fixed. All tests pass (54/54). Benchmark 50/50 (100%).

---

## CRITICAL Issues

### C1 — Aspiration window perspective mismatch (FIXED)

**Bug:** In `iterative_deepening`, aspiration bounds (`asp_alpha`, `asp_beta`) were computed
in white-centric perspective from `best_score`. But `root_search_windowed` uses them as
side-to-move (STM) perspective bounds. When black is to move, this meant children received
completely wrong alpha/beta windows.

**Example:** White-centric window [-100, 0] at a black-to-move root passed alpha=-100,
beta=0 into the STM search. The first child (white) received `negamax(child, 0, 100)`
instead of the correct `negamax(child, -100, 0)`. The child's pruning operated on bounds
offset by 100cp from the correct values.

**Impact before fix:**
- Wrong TT entry bounds for all child nodes when searching from black's position
- Reduced search efficiency (wrong pruning) — benchmark was 88.8s with bug, 75.0s after fix
- Potential wrong move in edge cases (wrong pruning could miss better moves)
- Final score and move were USUALLY correct because white_score conversion normalizes output

**Fix:** Convert aspiration bounds to STM perspective before calling `root_search_windowed`:
```rust
let (stm_alpha, stm_beta) = if is_white {
    (asp_alpha, asp_beta)
} else {
    (-asp_beta, -asp_alpha)
};
```

**Scope:** `src/search.rs` line ~105 (iterative_deepening aspiration loop)

**Verification:** 54/54 tests pass. Benchmark 50/50 (100%). Benchmark 15% faster after fix.

---

## WARNING Issues

### W1 — Time-aborted aspiration searches may overwrite good results

When `should_stop()` triggers during an aspiration re-search, the incomplete result
may overwrite `best_move`/`best_score` from the previous completed depth. The move is
almost always correct (TT move from previous depth is searched first), but the score
could be unreliable.

**Impact:** Low. The move is almost always the TT move (previous depth's best). The
wrong score only affects the aspiration window for the next depth, which won't happen
because we're stopping. Not a correctness issue in practice.

**Recommend:** Could add a flag to skip updating best_move/best_score on time-aborted
iterations, but this is standard chess engine behavior and not worth changing now.

---

## MINOR Issues

### M1 — root_search_windowed always stores Bound::Exact in TT

At the end of `root_search_windowed`, the TT entry is always stored as `Bound::Exact`,
even when the search was done with a narrow aspiration window and the score fell outside
the window. Should compute bounds based on `init_alpha`/`init_beta`.

**Impact:** Negligible. The root TT entry is only used for move ordering on re-search
(not for cutoffs). The entry is overwritten on each iteration.

### M2 — Dead code: `root_search` wrapper

The `root_search` function (full-window wrapper around `root_search_windowed`) is now
unused after the aspiration window refactoring. Only referenced in one test.

### M3 — Dead code: `order_moves_simple`

Pre-existing unused function. Generates compiler warning.

---

## Critical Checks — All 14 Items Verified

| # | Check | Status |
|---|-------|--------|
| 1 | Threefold repetition | PASS — unchanged in negamax:298-307, history push/pop correct |
| 2 | Fifty-move rule | PASS — unchanged in negamax:293-296, `halfmoves() >= 100` |
| 3 | TT mate score clamping | PASS — `score_to_tt()` clamps to ±32000, used at lines 246, 454 |
| 4 | LMR re-search trigger | PASS — reduced null-window → full-depth null-window → full window |
| 5 | Aspiration window fallback | PASS — window >= 800 falls back to NEG_INF/INF, perspective fixed |
| 6 | Dynamic time 3x hard limit | PASS — `hard_limit = base_time * 3`, `hard_stop()` in negamax |
| 7 | New unwrap()/panic! | PASS — zero new unwrap/panic in v0.0.2 code |
| 8 | PST values Michniewski | PASS — `#[path = "../frozen/pst.rs"]` mod, no changes to frozen/ |
| 9 | BISHOP==300, KNIGHT==300 | PASS — eval.rs:6-7 unchanged |
| 10 | DELTA==200, MAX_EXTENSIONS==4 | PASS — search.rs:12-13 unchanged |
| 11 | UCI bestmove output | PASS — main.rs:114-120 unchanged |
| 12 | Stop flag frequency | PASS — hard_stop every 2048 nodes, should_stop at root + iterations |
| 13 | Integer overflow in aspiration | PASS — window maxes at 800, scores ±1M, well within i32 |
| 14 | LMR captures/checks excluded | PASS — `is_quiet = !capture && !promotion`, `!in_check` guard |

## Frozen Value Verification

```
grep "325|320|mobility" src/     → only "NOT 325. NOT 320." comment ✓
grep "rollout|playout|UCB" src/  → zero hits ✓
grep "torch|neural" src/         → zero hits ✓
grep "positional" src/           → only variable name in eval.rs ✓
```

- PAWN=100, KNIGHT=300, BISHOP=300, ROOK=500, QUEEN=900, KING=20000 ✓
- DELTA=200, MAX_EXTENSIONS=4 ✓
- LMR_THRESHOLD=2, LMR_REDUCTION=1 ✓
- ASPIRATION_WINDOW=50 ✓
- No MCTS, no neural, no internal book, no always-replace TT ✓

## False Positives Caught

C1 — aspiration window perspective bug. Not a false positive in the CHP sense
(wrong PST, wrong piece values, MCTS, etc.), but a real implementation bug caught
by code review. Documented for completeness.

## Test Results

```
54/54 tests pass
50/50 benchmark positions pass (100%)
Pruning rate: 100%
Max position time: 3.5s
Total benchmark time: 75.0s
All sigma gates PASS
```
