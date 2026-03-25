# CHPawn-FrozenKing — Mandatory Code Review Prompt
# Run this after EVERY build, before EVERY release. No exceptions.
# Usage: Open Claude Code, paste this entire file as your prompt.
# Update REVIEW_VERSION below before running.

REVIEW_VERSION = v0.0.X  # ← UPDATE THIS BEFORE RUNNING

cd D:\EXPERIMENTS\CHPawn-FrozenKing

Read DECISIONS.md, frozen/spec.md, frozen/pst.rs, dead_ends.md first.
Do not skip any part. Do not reorder parts. Run all 7 parts in order.

================================================================================
PART 1 — PRIOR DETECTION
================================================================================

Run these greps. Zero real hits required. Any hit: HARD BLOCK — stop everything.

grep -r "rollout\|playout\|UCB\|visit_count\|MonteCarloNode" src/
grep -r "torch\|tensorflow\|neural\|embedding" src/
grep -r "325\|piece_square\|mobility" src/eval.rs

If any grep returns a real hit (not a comment, not a string literal):
  Log as FALSE POSITIVE CAUGHT in innovation_log.md
  Fix before proceeding
  Do not continue until grep is clean

================================================================================
PART 2 — FROZEN VALUE VERIFICATION
================================================================================

Verify these constants in src/eval.rs match exactly:
  PAWN                    == 100
  KNIGHT                  == 300
  BISHOP                  == 300
  ROOK                    == 500
  QUEEN                   == 900
  KING                    == 20000
  DELTA                   == 200
  MAX_EXTENSIONS          == 4
  PASSED_PAWN_BONUS       == [0, 10, 20, 30, 50, 75, 100, 0]
  DOUBLED_PAWN_PENALTY    == -20
  ISOLATED_PAWN_PENALTY   == -15
  ROOK_OPEN_FILE_BONUS    == 25
  ROOK_SEMI_OPEN_FILE_BONUS == 10
  BISHOP_PAIR_BONUS       == 50
  KING_ATTACKER_PENALTY   == -10
  KING_SHIELD_BONUS       == 10

Verify these constants in src/search.rs match exactly:
  LMR_THRESHOLD           == 2
  LMR_BASE_REDUCTION      == 1
  ASPIRATION_WINDOW       == 50
  NULL_MOVE_R             == 2

Verify these constants in src/time.rs match exactly:
  Hard limit multiplier   == 3

Any deviation from any value above: BLOCKING. Fix before continuing.

================================================================================
PART 3 — CRITICAL CHECKS
================================================================================

Check all 15 items. Flag each as PASS or FAIL.

1.  Null move zugzwang protection — has_non_pawn_pieces() present and correct?
2.  Null move disabled when in check?
3.  Null move disabled when beta is mate score?
4.  Threefold repetition detection still present in search?
5.  Fifty-move rule check still present in search?
6.  TT mate score clamping — score_to_tt() and tt_to_score() still correct?
7.  Any new unwrap() or expect() that could panic in a CCRL game?
8.  Evaluation symmetric? Run: evaluate(starting_pos) == 0?
9.  King safety disabled in endgame (when no queens on board)?
10. Stop flag checked at least every 2048 nodes?
11. LMR not applied to captures, promotions, or check positions?
12. Aspiration windows fall back to full window at >= 800cp?
13. Futility pruning disabled in check and at root?
14. Razoring disabled in check?
15. PST values — all 384 still sourced from frozen/pst.rs (Michniewski)?

Any FAIL: BLOCKING. Fix before continuing.

================================================================================
PART 4 — DEAD CODE CLEANUP
================================================================================

Run:
  cargo build 2>&1 | grep "warning"

Fix ALL of these:
  - unused import warnings
  - unused variable warnings  
  - dead code warnings
  - deprecated API warnings

Remove:
  - Any commented-out code blocks
  - Any TODO comments older than current version
  - Any #[allow(dead_code)] that isn't absolutely necessary

After cleanup, run:
  cargo build 2>&1 | grep "warning"

Must return ZERO warnings. No exceptions.

================================================================================
PART 5 — REGRESSION TESTS
================================================================================

Run:
  cargo test 2>&1

ALL tests must pass. Zero failures. Zero skipped.
If any test fails: debug, fix, re-run. Do not proceed with failing tests.

================================================================================
PART 6 — SIGMA GATE
================================================================================

Run:
  cargo run --release --bin benchmark 2>&1

Must pass all 4 gates:
  GATE 1: illegal_moves == 0        HARD BLOCKER
  GATE 2: pass_rate >= 90%          50/50 minimum
  GATE 3: pruning_rate >= 50%       alpha-beta working
  GATE 4: max_time < 900s           performance check

If GATE 1 fails: stop everything. Illegal move bug introduced. Debug immediately.
If GATE 2 fails: evaluation or search change broke endgame play. Debug immediately.
Do not release if any gate fails.

================================================================================
PART 7 — EVALUATION SANITY CHECKS
================================================================================

Write and run these quick inline checks (add to a test or run directly):
  1. evaluate(starting_position) == 0
  2. evaluate(kings_only_position) == 0
  3. evaluate(symmetric_position) == 0
  4. evaluate(extra_white_queen) > 900

All 4 must pass. If any fail: evaluation function has a bug.

================================================================================
WRITE REVIEW FILE
================================================================================

Write REVIEW_{REVIEW_VERSION}.md with this structure:

---
# CHPawn-FrozenKing {REVIEW_VERSION} — Code Review

## Verdict
CLEAN — ready to release
OR
NOT CLEAN — [list blocking issues]

## Part 1 — Prior Detection
[PASS / FAIL — details if fail]

## Part 2 — Frozen Values
[PASS / FAIL — list any deviations]

## Part 3 — Critical Checks
[15 items, each PASS or FAIL]

## Part 4 — Dead Code
[PASS / FAIL — zero warnings achieved?]

## Part 5 — Tests
[N/N pass]

## Part 6 — Sigma Gate
[50/50 or failure details]

## Part 7 — Sanity Checks
[4/4 pass or failures]

## Issues Found
CRITICAL: [list — must fix before release]
WARNING:  [list — should fix]
MINOR:    [list — nice to fix]
---

================================================================================
RELEASE CRITERIA
================================================================================

Only release if ALL of the following are true:
  - Part 1: Zero prior detection hits
  - Part 2: All frozen values exact
  - Part 3: All 15 critical checks pass
  - Part 4: Zero compiler warnings
  - Part 5: All tests pass
  - Part 6: 50/50 sigma gate
  - Part 7: All sanity checks pass
  - No CRITICAL issues in review file

If any criterion fails: fix it, re-run that part, update review file.
Do not release until all criteria pass.

Stop when REVIEW_{REVIEW_VERSION}.md is written and all criteria pass.
