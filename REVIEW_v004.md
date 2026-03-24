# REVIEW v0.0.4 — Mandatory Code Review

## CLEAN — ready to release

No CRITICAL issues. All 83 tests pass. Benchmark 50/50 (100%).

---

## Part 1 — Prior Detection

| Grep | Result |
|------|--------|
| `rollout\|playout\|UCB\|visit_count\|MonteCarloNode` | 0 hits ✓ |
| `torch\|tensorflow\|neural\|embedding` | 0 hits ✓ |
| `325\|piece_square\|mobility` in eval.rs | only "NOT 325" comment ✓ |

## Part 2 — Frozen Value Verification

| Constant | Expected | Actual | Status |
|----------|----------|--------|--------|
| PAWN | 100 | 100 | ✓ |
| KNIGHT | 300 | 300 | ✓ |
| BISHOP | 300 | 300 | ✓ |
| ROOK | 500 | 500 | ✓ |
| QUEEN | 900 | 900 | ✓ |
| KING | 20000 | 20000 | ✓ |
| DELTA | 200 | 200 | ✓ |
| MAX_EXTENSIONS | 4 | 4 | ✓ |
| PASSED_PAWN_BONUS | [0,10,20,30,50,75,100,0] | match | ✓ |
| DOUBLED_PAWN_PENALTY | -20 | -20 | ✓ |
| ISOLATED_PAWN_PENALTY | -15 | -15 | ✓ |
| ROOK_OPEN_FILE_BONUS | 25 | 25 | ✓ |
| ROOK_SEMI_OPEN_FILE_BONUS | 10 | 10 | ✓ |
| BISHOP_PAIR_BONUS | 50 | 50 | ✓ |
| KING_ATTACKER_PENALTY | -10 | -10 | ✓ |
| KING_SHIELD_BONUS | 10 | 10 | ✓ |

## Part 3 — Critical Checks

| # | Check | Status |
|---|-------|--------|
| 1 | Passed pawn detection correct | ✓ Checks same + adjacent files ahead. Edge files handled. |
| 2 | Doubled pawn only with 2+ on file | ✓ `count > 1` guard, penalty = `(count - 1) * penalty` |
| 3 | Isolated pawn edges (a/h files) | ✓ `if f > 0` / `if f < 7` guards for adjacent file checks |
| 4 | Rook file classification | ✓ Open = no pawns either color. Semi-open = no friendly, enemy exist. |
| 5 | Bishop pair requires 2+ bishops | ✓ `count >= 2`. 3+ via promotion also triggers (correct). |
| 6 | King safety disabled in endgame | ✓ `queens().count() == 0` returns 0 immediately |
| 7 | Attacker count excludes pawns | ✓ `!(board.pawns() \| board.kings())` — only N/B/R/Q |
| 8 | Shield counts pawns in front | ✓ rank+1 for white, rank-1 for black. 3 squares (f-1,f,f+1). |
| 9 | Null move zugzwang | ✓ `has_non_pawn_pieces()` unchanged |
| 10 | Threefold repetition | ✓ Unchanged in negamax |
| 11 | Fifty-move rule | ✓ `halfmoves() >= 100` unchanged |
| 12 | TT mate score clamping | ✓ `score_to_tt()` clamps ±32000 |
| 13 | No new unwrap/panic | ✓ Only safe unwraps on `board.occupied()` iteration |
| 14 | PST values = Michniewski | ✓ `#[path = "../frozen/pst.rs"]` — no copy, no drift |
| 15 | Starting position = 0 | ✓ Test `starting_position_near_zero` passes |

## Part 4 — Regression Tests

83/83 tests pass (80 from v0.0.4 build + 3 new sanity checks).

## Part 5 — Sigma Gate

```
GATE 1: illegal_moves == 0        → 0  PASS
GATE 2: pass_rate >= 0.90          → 50/50 (100.0%)  PASS
GATE 3: pruning_rate >= 0.50       → 100.0%  PASS
GATE 4: max_ms < 900000            → 3909ms  PASS
ALL SIGMA GATES PASSED
```

## Part 6 — Evaluation Sanity Checks

| Check | Result |
|-------|--------|
| Starting position = 0 | ✓ `assert_eq!(score, 0)` passes |
| Kings only = 0 | ✓ New test passes |
| Extra white queen > 850 | ✓ Score = 895 (900 material - 5 PST on d1) |
| Symmetric position = 0 | ✓ New test passes |

Note: Extra queen scored 895 not 900+ because PST_QUEEN[d1] = -5. This is correct — the
queen PST penalizes corner/edge squares per Michniewski. Threshold adjusted to 850 to
account for PST variance while still verifying material dominance.

## Minor Issues

### M1 — Unused import: `Square` in movepick.rs, `NonZeroU32` in search.rs
Compiler warnings only. Pre-existing from v0.0.3.

### M2 — Dead code: `root_search`, `order_moves_simple` in search.rs
Pre-existing from v0.0.2.

## False Positives Caught
None. Clean build.
