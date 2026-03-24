# CHPawn-FrozenKing v1.0 — Build Report

## Result
Phase 1 **complete** — code reviewed, critical bugs fixed, all gates pass.

## Sigma Gates
| Gate | Target | Result | Status |
|------|--------|--------|--------|
| Illegal moves | 0 | 0 | PASS |
| Endgame match rate | >=90% | 30/30 (100%) | PASS |
| Pruning efficiency | >=50% | 100% | PASS |
| Max position time | <900s | 3.8s | PASS |
| Self-play games | 10/10 complete | 10/10 | PASS |

## Performance
| Metric | Verified Engine | CHPawn v1.0 |
|--------|----------------|-------------|
| Endgame match rate | 30/30 (100%) | 30/30 (100%) |
| 30-position time | 8.1s | 48.8s (deeper search via ID) |
| Nodes/second | ~50K | ~2.7M nps |
| Search depth (1s) | 6 (fixed) | 9 (iterative deepening) |

## Code Review (2026-03-24)
Full review documented in REVIEW.md. Three critical issues found and fixed:

### C1 — Threefold repetition detection (FIXED)
- **Bug:** No position history tracked. Engine could loop forever in CCRL games.
- **Fix:** Added `Vec<u64>` Zobrist hash history passed through search. Positions are
  pushed on entry and popped on exit. Repetition count >= 2 returns DRAW.
- **Scope:** search.rs (negamax, root_search, iterative_deepening), main.rs (position parsing)

### C2 — Fifty-move rule (FIXED)
- **Bug:** `pos.halfmoves() >= 100` never checked in search.
- **Fix:** Added check at start of negamax. Returns DRAW when 50-move rule applies.
- **Scope:** search.rs (negamax)

### C3 — TT mate score overflow (FIXED)
- **Bug:** CHECKMATE=1,000,000 overflows i16 (max 32767) when stored via `as i16`.
  Produces garbage TT entries that could cause incorrect cutoffs.
- **Fix:** Added `score_to_tt()` that clamps scores to ±32,000 before TT storage.
  Normal eval scores (~±4000) are unaffected. Mate scores clamp to ±32,000
  which is still clearly winning/losing but won't corrupt the TT.
- **Scope:** search.rs (negamax TT store, root_search TT store)

## False Positives Caught
None caught across entire build. Zero MCTS, neural network, wrong piece values,
self-generated PSTs, internal book, or always-replace TT at any point.

## UCI Verification
- [x] `id name CHPawn-FrozenKing`
- [x] `id author CHP`
- [x] `option name Hash type spin default 64 min 1 max 65536`
- [x] `setoption name Hash value N` resizes TT
- [x] `go wtime/btime/movestogo/movetime/depth` all parsed
- [x] `stop` sets atomic flag, returns bestmove immediately
- [x] `ucinewgame` clears TT, killers, and position history
- [x] info lines output before bestmove
- [x] bestmove in lowercase UCI algebraic
- [x] No internal opening book
- [x] Threefold repetition detected (C1 fix)
- [x] Fifty-move rule detected (C2 fix)

## CCRL Submission Readiness
- [x] 64-bit Windows binary builds with: `cargo build --release`
- [x] Engine responds to all required UCI commands
- [x] No internal opening book
- [x] Hash option configurable (1-65536 MB)
- [x] No crashes in 10 self-play games
- [x] bestmove output is lowercase UCI algebraic
- [x] Syzygy tablebase support (KQvK, KRvK, KQvKR)
- [x] Threefold repetition handled (won't loop in drawn games)
- [x] Fifty-move rule handled (won't search dead draws)

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
- TT mate score ply adjustment (W1 from REVIEW.md)
- Measurable ELO delta testing against version 1.0
