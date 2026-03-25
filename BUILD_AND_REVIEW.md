cd D:\EXPERIMENTS\CHPawn-FrozenKing

# ============================================================
# STEP 1 — READ THESE FILES FIRST BEFORE WRITING ANY CODE
# ============================================================

Read in this exact order:
1. DECISIONS.md
2. frozen/spec.md
3. frozen/pst.rs
4. dead_ends.md
5. innovation_log.md
6. CODE_REVIEW.md
7. CHESS_LOGIC_REVIEW.md

Do not write a single line of code until all seven are read.

# ============================================================
# STEP 2 — BUILD
# ============================================================

Run the build prompt for this version.
Build every feature. Builder → Critic → Reviewer after each one.
cargo test after each feature.
Benchmark after all features.
Write REPORT_vXXX.md when build is complete.

# ============================================================
# STEP 3 — MANDATORY CODE REVIEW (runs automatically after build)
# ============================================================

When REPORT_vXXX.md is written, immediately run CODE_REVIEW.md.
Do not skip this step. Do not ask. Just run it.

Update REVIEW_VERSION to match current version.

Run all 7 parts in order:
  Part 1 — Prior detection (greps)
  Part 2 — Frozen value verification
  Part 3 — Critical checks (15 items)
  Part 4 — Dead code cleanup (zero warnings required)
  Part 5 — Regression tests (all must pass)
  Part 6 — Sigma gate (50/50 required)
  Part 7 — Evaluation sanity checks

Write REVIEW_vXXX.md.

# ============================================================
# STEP 4 — MANDATORY CHESS LOGIC REVIEW (runs automatically after code review)
# ============================================================

When REVIEW_vXXX.md says CLEAN, immediately run CHESS_LOGIC_REVIEW.md.
Do not skip this step. Do not ask. Just run it.

Run all 7 sections in order:
  Section 1 — CHP protocol verification (HARD BLOCKERS)
  Section 2 — Chess rule correctness
  Section 3 — Evaluation logic correctness
  Section 4 — Search logic correctness
  Section 5 — UCI protocol correctness
  Section 6 — Sigma gate (mathematical proof)
  Section 7 — Known limitations log

Write CHESS_LOGIC_REVIEW_vXXX.md.

# ============================================================
# STEP 5 — RELEASE DECISION
# ============================================================

Only print release message if ALL of these are true:
  - REVIEW_vXXX.md says CLEAN
  - CHESS_LOGIC_REVIEW_vXXX.md says CHESS LOGIC VERIFIED
  - Zero compiler warnings
  - All tests pass
  - 50/50 sigma gate passes

If both pass, print exactly:
  "BUILD COMPLETE. REVIEW CLEAN. CHESS LOGIC VERIFIED. Ready to release vX.X.X.
   Run git commands to ship."

If either has issues:
  Fix them.
  Re-run the failed review.
  Do not print release message until both pass.

# ============================================================
# STOP CONDITION
# ============================================================

Stop only when:
  - REPORT_vXXX.md written
  - REVIEW_vXXX.md says CLEAN
  - CHESS_LOGIC_REVIEW_vXXX.md says CHESS LOGIC VERIFIED
  - Zero compiler warnings
  - All tests pass
  - 50/50 sigma gate passes
  - "BUILD COMPLETE. REVIEW CLEAN. CHESS LOGIC VERIFIED." printed

Do not stop before all conditions are met.
