# CHPawn-FrozenKing v0.0.8 — Build Report

## Result
v0.0.8 **complete** — Time management fix + eval tuning. All gates pass. Zero warnings.

## Fixes Applied
1. **Time Management (DD03-C)** — Conservative time allocation to avoid flagging.
   - remaining/40 (was /20) + winc/binc increment support
   - hard_limit = base * 2 (was * 3)
   - Added winc/binc to GoParams and TimeManager::new signature

2. **Eval Constant Tuning** — Stronger weights for middlegame features.
   - MOBILITY_WEIGHT: [0,2,8,8,4,2,0] (was [0,1,4,4,2,1,0])
   - PASSED_PAWN_BONUS: [0,20,40,60,100,150,200,0] (was [0,10,20,30,50,75,100,0])
   - KING_ATTACKER_PENALTY: -30 (was -10)
   - KING_SHIELD_BONUS: 15 (was 10)
   - KNIGHT_OUTPOST_BONUS: 50 (was 30)
   - BISHOP_OUTPOST_BONUS: 35 (was 20)

## Sigma Gates
| Gate | Target | Result | Status |
|------|--------|--------|--------|
| Illegal moves | 0 | 0 | PASS |
| Endgame match rate | >=90% | 50/50 (100%) | PASS |
| Pruning efficiency | >=50% | 100% | PASS |
| Max position time | <900s | 4.1s | PASS |

## Test Suite
107/107 tests pass. Zero compiler warnings.

## Files Modified
- `src/time.rs` — /40 divisor, *2 hard limit, winc/binc parameters
- `src/main.rs` — winc/binc in GoParams, parse_go, TimeManager calls
- `src/eval.rs` — 6 constant value updates
- `src/bin/selfplay.rs` — Updated TimeManager::new call
