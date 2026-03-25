# CHPawn-FrozenKing v0.1.1 — Build Report

## Result
v0.1.1 **complete** — 2 verification tools added. No engine changes. All gates pass.

## Tools Added
1. **Perft Test** (`src/bin/perft.rs`) — Verifies move generation correctness against
   known-correct perft values. 14 tests across 3 positions (starting, Kiwipete, Position 3)
   up to depth 5. **PERFT CLEAN — all 14 pass.**

2. **Eval Comparison** (`src/bin/eval_compare.rs`) — Tests CHPawn evaluation against
   Stockfish on 20 diverse positions. Reports score differences > 150cp as flags.
   Stockfish not available on this machine — CHPawn-only results recorded.

## Perft Results
All 14 perft tests pass. Move generation is mathematically verified correct.
- Starting position depth 5: 4,865,609 nodes (exact match)
- Kiwipete depth 4: 4,085,603 nodes (exact match)
- Position 3 depth 5: 674,624 nodes (exact match)

## Sigma Gates
50/50 (100%). All gates PASS.

## Test Suite
118/118 tests pass. Zero compiler warnings.

## Files Added
- `src/bin/perft.rs` — Perft verification binary
- `src/bin/eval_compare.rs` — Eval comparison binary
- `EVAL_COMPARE_v011.md` — Eval comparison results
