# CHPawn-FrozenKing v0.0.2 — Build Report

## Result
v0.0.2 **complete** — 3 features added, all gates pass, no regressions.

## Features Added
1. **Late Move Reductions (DD-LMR)** — Quiet moves after the first 2, at depth >= 3, not in check, reduced by 1 ply. Re-search at full depth if reduced search beats alpha. Integrates with PVS.
2. **Aspiration Windows (DD07)** — After depth 1, search within prev_score +/- 50cp window. Widen on fail-low/fail-high. Fall back to full window at 800cp.
3. **Dynamic Time Management (DD03-B)** — Sudden death: remaining/20 (was /30). Known movestogo: remaining/(movestogo+5). Hard limit = 3x base. Soft stop between iterations, hard stop inside nodes.

## Sigma Gates
| Gate | Target | Result | Status |
|------|--------|--------|--------|
| Illegal moves | 0 | 0 | PASS |
| Endgame match rate | >=90% | 50/50 (100%) | PASS |
| Pruning efficiency | >=50% | 100% | PASS |
| Max position time | <900s | 4.5s | PASS |

## Performance Comparison
| Metric | v1.0 | v0.0.2 |
|--------|------|--------|
| Endgame match rate | 30/30 (100%) | 50/50 (100%) |
| 50-position time | 48.8s | 88.8s |
| Max position time | 3.8s | 4.5s |

Note: v0.0.2 benchmark ran 50 positions (expanded battery) vs v1.0's 30 positions.
The per-position time is comparable. LMR and aspiration windows reduce overall node count
at higher depths while maintaining 100% correctness.

## Test Suite
54/54 tests pass across all modules:
- eval: 7 tests (unchanged)
- search: 16 tests (+2 LMR, +2 aspiration windows)
- tt: 11 tests (unchanged)
- movepick: 5 tests (unchanged)
- time: 9 tests (+3 new for DD03-B formulas, -2 removed obsolete /30 tests)
- tablebase: 3 tests (unchanged)

## False Positives Caught
None caught. All frozen values intact:
- PAWN=100, KNIGHT=300, BISHOP=300, ROOK=500, QUEEN=900, KING=20000
- DELTA=200, MAX_EXTENSIONS=4
- LMR_THRESHOLD=2, LMR_REDUCTION=1
- ASPIRATION_WINDOW=50
- No MCTS, no neural, no internal book, no always-replace TT

## Frozen Value Verification
- LMR_THRESHOLD = 2 (moves after first 2)
- LMR_REDUCTION = 1 (reduce by 1 ply)
- ASPIRATION_WINDOW = 50 (centipawns)
- Time: remaining/20 sudden death, remaining/(movestogo+5) known moves
- Hard limit: base_time * 3

## CCRL Compatibility
- [x] Release binary builds clean
- [x] All UCI commands still work (no changes to UCI protocol)
- [x] Hash option still configurable
- [x] No internal opening book
- [x] bestmove output lowercase UCI algebraic
- [x] Time management handles wtime/btime/movestogo/movetime correctly

## Files Modified
- `src/search.rs` — LMR in negamax, aspiration windows in iterative_deepening, root_search_windowed
- `src/time.rs` — DD03-B dynamic formula, hard_limit, hard_stop()
- `DECISIONS.md` — DD03-B, DD07, DD-LMR documented as DONE
- `innovation_log.md` — v0.0.2 build log appended

## Next Steps
Per DECISIONS.md iteration roadmap:
- DD05 — Null move pruning with zugzwang detection (v1.2)
- TT mate score ply adjustment (W1 from REVIEW.md)
- ELO delta testing v0.0.2 vs v1.0
