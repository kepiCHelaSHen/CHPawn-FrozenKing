# CHPawn-FrozenKing v0.1.0 — Build Report

## Result
v0.1.0 **complete** — 3 eval blindspot fixes from game analysis. All gates pass.

## Features Added
1. **Backward Pawn Penalty** — Penalizes pawns that can't be defended by friendly pawns
   and whose stop square is controlled by an enemy pawn. BACKWARD_PAWN_PENALTY=-25.
   Source: chessprogramming.org/Backward_Pawn.

2. **Space Advantage** — Counts safe squares attacked by advanced pawns (ranks 3-5 for
   white, 4-6 for black). SPACE_WEIGHT=2.
   Source: chessprogramming.org/Space.

3. **Improved Endgame Detection** — Replaced queens==0 with game phase calculation.
   Phase = knights*1 + bishops*1 + rooks*2 + queens*4. Endgame when phase <= 8.
   Fixes eval inconsistencies when queens exist but few other pieces remain.
   Source: chessprogramming.org/Tapered_Eval.

## Sigma Gates
| Gate | Target | Result | Status |
|------|--------|--------|--------|
| Illegal moves | 0 | 0 | PASS |
| Endgame match rate | >=90% | 50/50 (100%) | PASS |
| Pruning efficiency | >=50% | 100% | PASS |
| Max position time | <900s | 4.3s | PASS |

## Test Suite
118/118 tests pass. Zero compiler warnings.
