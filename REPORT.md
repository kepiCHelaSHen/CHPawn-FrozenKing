# CHPawn-FrozenKing v1.0 — Build Report

## Result
Phase 1 **complete**

## Sigma Gates
| Gate | Target | Result | Status |
|------|--------|--------|--------|
| Illegal moves | 0 | 0 | PASS |
| Endgame match rate | >=90% | 30/30 (100%) | PASS |
| Pruning efficiency | >=50% | 100% | PASS |
| Max position time | <900s | 3.2s | PASS |
| Self-play games | 10/10 complete | 10/10 | PASS |

## Performance
| Metric | Verified Engine | CHPawn v1.0 |
|--------|----------------|-------------|
| Endgame match rate | 30/30 (100%) | 30/30 (100%) |
| 30-position time | 8.1s | 41.4s (deeper search via ID) |
| Nodes/second | ~50K | ~2.7M nps |
| Search depth (1s) | 6 (fixed) | 9 (iterative deepening) |

## Features Implemented (per DECISIONS.md)
| Feature | Decision | Status |
|---------|----------|--------|
| DD01 MVV-LVA Move Ordering | ADD to 1.0 | DONE |
| DD02 Iterative Deepening | ADD to 1.0 | DONE |
| DD03A Simple Time Management | ADD to 1.0 | DONE |
| DD04 Transposition Table | ADD to 1.0 | DONE |
| DD06 Check Extensions | ADD to 1.0 | DONE |
| DD08 Piece Square Tables | ADD to 1.0 | DONE |
| PVS (Principal Variation Search) | ADD to 1.0 | DONE |
| Killer move heuristic | ADD to 1.0 | DONE |

## Architecture
```
src/
  main.rs        — UCI protocol loop (Hash option, time mgmt, stop)
  eval.rs        — material + Michniewski PST evaluation
  search.rs      — negamax + PVS + TT + killers + check extensions + ID
  tablebase.rs   — Syzygy probing (unchanged from skeleton)
  tt.rs          — transposition table (10-byte entry, 3/cluster, depth+age hybrid)
  time.rs        — time management (remaining/30, movetime override, stop flag)
  movepick.rs    — move ordering (TT move > MVV-LVA captures > killers > quiet)
  lib.rs         — module declarations

frozen/
  spec.md        — frozen algorithm specification
  pst.rs         — 384 Michniewski PST values (immutable, sourced via #[path])

src/bin/
  benchmark.rs   — 30-position sigma gate
  selfplay.rs    — 10-game self-play gate
```

## False Positives Caught
None caught — clean build. No MCTS, neural network, wrong piece values, self-generated
PSTs, internal book, or always-replace TT detected at any point during the build.

Log this as anomaly: zero false positives across 6 milestones is unusual. The dead_ends.md
pre-loading and frozen/pst.rs module reference strategy (no PST values in source, only
module import) may have prevented the most likely drift vectors.

## Critic Gate Scores
| Gate | Score | Threshold | Status |
|------|-------|-----------|--------|
| Frozen compliance | 1.0 | 1.0 | PASS |
| Architecture | 1.0 | 0.85 | PASS |
| Scientific validity | 1.0 | 0.85 | PASS |
| Drift check | 1.0 | 0.85 | PASS |

Frozen values verified:
- PAWN=100, KNIGHT=300, BISHOP=300, ROOK=500, QUEEN=900, KING=20000
- DELTA=200, MAX_EXTENSIONS=4
- All 384 PST values from frozen/pst.rs (Michniewski source)
- TT: 10-byte entry, 32-byte cluster, depth+age hybrid replacement

## UCI Verification
- [x] `id name CHPawn-FrozenKing`
- [x] `id author CHP`
- [x] `option name Hash type spin default 64 min 1 max 65536`
- [x] `setoption name Hash value N` resizes TT
- [x] `go wtime/btime/movestogo/movetime/depth` all parsed
- [x] `stop` sets atomic flag, returns bestmove immediately
- [x] `ucinewgame` clears TT and killers
- [x] info lines output before bestmove
- [x] bestmove in lowercase UCI algebraic (e.g., `e2e4`)
- [x] No internal opening book

## CCRL Submission Readiness
- [x] 64-bit Windows binary builds with: `cargo build --release`
- [x] Engine responds to all required UCI commands
- [x] No internal opening book
- [x] Hash option configurable (1-65536 MB)
- [x] No crashes in 10 self-play games
- [x] bestmove output is lowercase UCI algebraic
- [x] Syzygy tablebase support (KQvK, KRvK, KQvKR)

## Test Suite
48/48 unit tests pass across all modules:
- eval: 7 tests (frozen values, PST correctness, symmetry)
- search: 14 tests (mate finding, PVS, TT hits, pruning, check extensions)
- tt: 11 tests (entry/cluster size, replacement policy, store/probe)
- movepick: 5 tests (MVV-LVA ordering, killers, spec compliance)
- time: 8 tests (budget calculation, stop flag, division by 30)
- tablebase: 3 tests (KQvK, KRvK winning, startpos returns none)

## Next Steps (Version 1.1)
Per DECISIONS.md iteration roadmap:
- DD07 — Aspiration Windows (+20-40 ELO)
- DD03B — Dynamic time management (replace fixed 1/30 allocation)
- 3-fold repetition detection in search (currently only in self-play harness)
- Measurable ELO delta testing against version 1.0
