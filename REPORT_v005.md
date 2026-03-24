# CHPawn-FrozenKing v0.0.5 — Build Report

## Result
v0.0.5 **complete** — 4 search efficiency features + dead code cleanup. All gates pass.
Zero compiler warnings. No regressions.

## Context
CHPawn v0.0.4 has strong evaluation but still loses on depth against 2200+ opponents.
This version adds search pruning and move ordering improvements to go deeper.

## Features Added
1. **Futility Pruning** — At depth <= 3, skip quiet non-PV moves where static eval + margin
   can't reach alpha. FUTILITY_MARGIN = [0, 100, 200, 300]. Disabled in check and near mate.
   Source: chessprogramming.org/Futility_Pruning. Expected +20-40 ELO.

2. **Razoring** — At depth <= 2, if static eval is far below alpha, drop to quiescence.
   RAZOR_MARGIN = [0, 300, 500]. Disabled in check.
   Source: chessprogramming.org/Razoring. Expected +15-25 ELO.

3. **Logarithmic LMR** — Replaced flat 1-ply LMR with log formula:
   `reduction = max(1, ln(depth) * ln(move_index) / 2)`. Deeper nodes and later moves get
   larger reductions. At depth 6 move 10: reduction=2. At depth 10 move 20: reduction=3.
   Source: chessprogramming.org/Late_Move_Reductions. Expected +10-20 ELO.

4. **Internal Iterative Deepening** — When no TT move at depth >= 4, do a shallow search
   (depth - 2) first to find a good move to try first. IID_DEPTH_THRESHOLD=4, IID_REDUCTION=2.
   Source: chessprogramming.org/Internal_Iterative_Deepening. Expected +10-20 ELO.

## Dead Code Cleanup
Removed: `root_search`, `order_moves_simple`, `order_captures_simple`, old `quiescence`.
Fixed all unused imports and variables. **Zero compiler warnings** across entire codebase.

## Sigma Gates
| Gate | Target | Result | Status |
|------|--------|--------|--------|
| Illegal moves | 0 | 0 | PASS |
| Endgame match rate | >=90% | 50/50 (100%) | PASS |
| Pruning efficiency | >=50% | 100% | PASS |
| Max position time | <900s | 4.1s | PASS |

## Performance
| Metric | v0.0.4 | v0.0.5 |
|--------|--------|--------|
| Endgame match rate | 50/50 | 50/50 |
| 50-position time | 82.6s | 85.1s |
| Max position time | 3.9s | 4.1s |
| Compiler warnings | 9 | **0** |

## Expected ELO Gain
| Feature | Estimated ELO |
|---------|--------------|
| Futility Pruning | +20-40 |
| Razoring | +15-25 |
| Logarithmic LMR | +10-20 |
| IID | +10-20 |
| **Total v0.0.5** | **+55-105** |
| **Cumulative** | **~2200-2800** |

## Test Suite
89/89 tests pass:
- eval: 23 tests (unchanged)
- search: 29 tests (+6 new: futility margin, razor margin, IID constants, log LMR x3, mate-with-pruning)
- tt: 11 tests (unchanged)
- movepick: 11 tests (unchanged)
- time: 9 tests (unchanged)
- tablebase: 3 tests (unchanged)

## Frozen Value Verification
All constants verified:
- PAWN=100, KNIGHT=300, BISHOP=300, ROOK=500, QUEEN=900, KING=20000
- DELTA=200, MAX_EXTENSIONS=4
- FUTILITY_MARGIN=[0,100,200,300], RAZOR_MARGIN=[0,300,500]
- IID_DEPTH_THRESHOLD=4, IID_REDUCTION=2
- LMR_BASE_REDUCTION=1, LMR_THRESHOLD=2
- MCTS grep: 0 hits. Neural grep: 0 hits.

## Zero Warnings Confirmed
`cargo build` produces zero warnings across all binaries (lib, benchmark, main, selfplay).

## False Positives Caught
None. Clean build.

## Next Steps
- Arena tournament re-test with v0.0.5
- If competitive at 2200: prepare for CCRL submission
- If still losing: consider mobility evaluation, countermove heuristic
