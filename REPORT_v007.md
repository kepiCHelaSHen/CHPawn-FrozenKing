# CHPawn-FrozenKing v0.0.7 — Build Report

## Result
v0.0.7 **complete** — 4 Eval Tier 2 features. All gates pass. Zero warnings. No regressions.

## Features Added
1. **Piece Mobility** — Count safe squares per piece (excluding enemy-pawn-defended).
   MOBILITY_WEIGHT=[0,1,4,4,2,1,0]. Uses shakmaty::attacks for sliding pieces.
   Source: chessprogramming.org/Mobility. Expected +15-25 ELO.

2. **Outpost Detection** — Knights/bishops on squares in enemy half, defended by friendly
   pawn, no enemy pawns on adjacent files ahead. KNIGHT_OUTPOST_BONUS=30, BISHOP=20.
   Source: chessprogramming.org/Outpost. Expected +10-20 ELO.

3. **Rook Coordination** — Doubled rooks on same file (+20). Rook on 7th rank (+30).
   Source: chessprogramming.org/Connectivity. Expected +5-10 ELO.

4. **Development Penalty** — Penalize undeveloped minor pieces in first 20 moves (-10 each).
   Source: chessprogramming.org/Development. Expected +5-10 ELO.

## Sigma Gates
| Gate | Target | Result | Status |
|------|--------|--------|--------|
| Illegal moves | 0 | 0 | PASS |
| Endgame match rate | >=90% | 50/50 (100%) | PASS |
| Pruning efficiency | >=50% | 100% | PASS |
| Max position time | <900s | 3.8s | PASS |

## Expected ELO Gain
| Feature | Estimated ELO |
|---------|--------------|
| Piece Mobility | +15-25 |
| Outpost Detection | +10-20 |
| Rook Coordination | +5-10 |
| Development Penalty | +5-10 |
| **Total v0.0.7** | **+35-65** |
| **Cumulative** | **~2300-2900** |

## Test Suite
106/106 tests pass. Zero compiler warnings.

## Frozen Value Verification
All constants verified. MCTS grep: 0. Neural grep: 0.

## Files Modified
- `src/eval.rs` — All 4 eval features + 12 new tests + imports for shakmaty::attacks

## Next Steps
- Arena tournament re-test with v0.0.7
- If competitive at 2200+: CCRL submission
