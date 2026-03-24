# CHPawn-FrozenKing v0.0.6 — Build Report

## Result
v0.0.6 **complete** — 3 Tier 3 features. All gates pass. Zero warnings. No regressions.

## Features Added
1. **Countermove Heuristic** — Tracks which move refutes each opponent move. Countermove
   scores COUNTERMOVE_SCORE=8000 (below killers=9000, above quiet history). Stored on
   quiet beta cutoff. Source: chessprogramming.org/Countermove_Heuristic. Expected +15-25 ELO.

2. **Capture History Table** — capture_hist[color][to_sq][captured_role]. Updated on capture
   beta cutoffs with depth^2 bonus. Added to capture ordering on top of MVV-LVA+SEE.
   Source: chessprogramming.org/History_Heuristic. Expected +10-15 ELO.

3. **Complexity-Based Time Management** — Track best move stability across ID iterations.
   Stable 3+ iterations: use 50% budget. Move changed: use 150% budget. Hard limit
   unchanged (base*3). Expected +10-20 ELO.

## Sigma Gates
| Gate | Target | Result | Status |
|------|--------|--------|--------|
| Illegal moves | 0 | 0 | PASS |
| Endgame match rate | >=90% | 50/50 (100%) | PASS |
| Pruning efficiency | >=50% | 100% | PASS |
| Max position time | <900s | 3.9s | PASS |

## Expected ELO Gain
| Feature | Estimated ELO |
|---------|--------------|
| Countermove Heuristic | +15-25 |
| Capture History | +10-15 |
| Complexity Time Mgmt | +10-20 |
| **Total v0.0.6** | **+35-60** |
| **Cumulative** | **~2250-2850** |

## Test Suite
94/94 tests pass. Zero compiler warnings.

## Frozen Value Verification
All constants verified. MCTS grep: 0. Neural grep: 0. No drift.

## Files Modified
- `src/movepick.rs` — Countermove table, capture history, updated order_moves signature
- `src/search.rs` — prev_move threading, countermove/capture history storage, stability tracking
- `src/time.rs` — Added budget_ms() accessor
- `DECISIONS.md` — 3 new entries
- `innovation_log.md` — v0.0.6 build log

## Next Steps
- Arena tournament re-test with v0.0.6
- If competitive at 2200+: prepare CCRL submission
