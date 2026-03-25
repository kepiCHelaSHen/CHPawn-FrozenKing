# CHPawn-FrozenKing v0.0.9 — Build Report

## Result
v0.0.9 **complete** — Stability cutoff bug fixed. All gates pass.

## Bug Fixed
**Root cause**: Complexity-based time management stability cutoff had no minimum depth guard.
In simple endgames (KPK), the best move stabilizes at depth 2-3, triggering STABILITY_BONUS
at depth 4. Combined with conservative /40 time allocation, this halved the budget too early,
causing the engine to search only to depth 4-5 when it should reach depth 12+.

**Symptoms**: Arena shows same move and score at every depth. Engine appears stuck.

**Fix**:
1. Added `STABILITY_MIN_DEPTH = 8` — stability cutoff only activates after depth 8
2. Changed `STABILITY_BONUS` from 0.5 to 0.7 (use 70% budget, not 50%)
3. Removed `depth > 2` guard on instability check (replaced by min depth check)

**Diagnostic test added**: `iterative_deepening_shows_progress_kpk` verifies the engine
reports multiple depths and reaches sufficient depth in timed KPK endgames.

## Sigma Gates
| Gate | Target | Result | Status |
|------|--------|--------|--------|
| Illegal moves | 0 | 0 | PASS |
| Endgame match rate | >=90% | 50/50 (100%) | PASS |
| Pruning efficiency | >=50% | 100% | PASS |
| Max position time | <900s | 4.2s | PASS |

## Test Suite
108/108 tests pass. Zero compiler warnings.
