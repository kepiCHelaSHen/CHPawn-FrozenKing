# CLAUDE.md — CHPawn-FrozenKing v1.0
# One-go Claude Code build prompt.
# Do not stop until REPORT.md is written and all sigma gates pass.
# This engine is PRIVATE. Do not push to any public repository.

================================================================================
READ THESE FIRST — IN ORDER — BEFORE WRITING ANY CODE
================================================================================

1. D:\EXPERIMENTS\CHPawn-FrozenKing\DECISIONS.md      ← THE LAW
2. D:\EXPERIMENTS\CHPawn-FrozenKing\frozen\spec.md    ← FROZEN VALUES
3. D:\EXPERIMENTS\CHPawn-FrozenKing\frozen\pst.rs     ← FROZEN PST
4. D:\EXPERIMENTS\CHPawn-FrozenKing\dead_ends.md      ← WHAT NOT TO DO
5. D:\EXPERIMENTS\CHPawn-FrozenKing\RESEARCH.md       ← REFERENCE IMPLS

Do not write a single line of code until all five are read.

================================================================================
STEP 0 — SETUP
================================================================================

0a. Verify Rust is installed:
    rustc --version && cargo --version

0b. Copy verified engine as skeleton:
    Copy ALL files from D:\EXPERIMENTS\chp-chess-engine-rs\src\ 
    to D:\EXPERIMENTS\CHPawn-FrozenKing\src\
    
    Copy Cargo.toml from verified engine, update:
      name = "chpawn-frozen-king"
      Add Cargo.toml entries for any new dependencies needed

0c. Copy Syzygy files:
    Copy D:\EXPERIMENTS\chp-chess-engine-rs\syzygy\*.rt* 
    to D:\EXPERIMENTS\CHPawn-FrozenKing\syzygy\

0d. Verify skeleton builds:
    cd D:\EXPERIMENTS\CHPawn-FrozenKing
    cargo build
    cargo test

0e. Verify UCI works in skeleton:
    echo "uci" | cargo run --release
    Should output: id name + uciok

0f. Log in innovation_log.md:
    "STEP 0 COMPLETE: skeleton copied, builds clean, UCI verified"

================================================================================
ARCHITECTURE
================================================================================

src/
  main.rs        — UCI protocol loop (stdin/stdout) — FROM SKELETON
  eval.rs        — material eval + PST evaluation — EXTEND
  search.rs      — minimax + alpha-beta + quiescence + PVS + killers — EXTEND
  tablebase.rs   — Syzygy probing — FROM SKELETON
  tt.rs          — transposition table — NEW
  time.rs        — time management — NEW
  movepick.rs    — move ordering (MVV-LVA + killers) — NEW

frozen/
  spec.md        — frozen algorithm spec
  pst.rs         — frozen Michniewski PST values (384 values)

benchmark/
  src/bin/benchmark.rs — 30-position sigma gate (FROM SKELETON)

syzygy/          — tablebase files (already copied in STEP 0)

================================================================================
FROZEN VALUES — VERIFY THESE BEFORE EVERY MILESTONE
================================================================================

Piece values (centipawns):
  PAWN=100, KNIGHT=300, BISHOP=300, ROOK=500, QUEEN=900, KING=20000

Search constants:
  MAX_DEPTH=6 (base, overridden by iterative deepening)
  DELTA=200 (quiescence delta pruning)
  MAX_EXTENSIONS=4 (check extension limit)

Time management:
  time_per_move = remaining_time / 30

Transposition table:
  Default size: 64MB
  Entry: key(u16) + move(u16) + score(i16) + depth(u8) + flags(u8) + eval(i16)
  Cluster: 3 entries per 32-byte aligned cluster
  Replacement: depth+age hybrid (see DECISIONS.md DD04 for exact formula)

PST source: Michniewski Simplified Evaluation Function
  All 384 values in frozen/pst.rs — DO NOT MODIFY

UCI identity:
  id name CHPawn-FrozenKing
  id author CHP

CCRL hash option:
  option name Hash type spin default 64 min 1 max 65536

================================================================================
MILESTONE 1 — tt.rs (Transposition Table)
================================================================================

Build: src/tt.rs — new file

Implement exactly per DECISIONS.md DD04 and RESEARCH.md Viridithas spec:

pub enum Bound { Exact, Lower, Upper }

pub struct TTEntry {
    key: u16,        // truncated Zobrist
    mv: u16,         // packed move
    score: i16,      // centipawn score
    eval: i16,       // static eval
    depth: u8,       // search depth
    flags: u8,       // packed: age(5) + pv(1) + bound(2)
}

struct TTCluster {
    entries: [TTEntry; 3],
    padding: [u8; 2],  // 32-byte alignment
}

pub struct TranspositionTable {
    clusters: Vec<TTCluster>,
    age: u8,
}

impl TranspositionTable:
    pub fn new(mb: usize) -> Self
    pub fn probe(&self, key: u64) -> Option<TTEntry>
    pub fn store(&mut self, key: u64, depth: u8, score: i16, 
                 bound: Bound, mv: Option<Move>, eval: i16)
    pub fn resize(&mut self, mb: usize)
    pub fn clear(&mut self)
    pub fn increment_age(&mut self)

Replacement policy (frozen from DECISIONS.md):
    priority = depth + flag_bonus + age_diff^2/4 + pv_bonus
    flag_bonus: Exact=3, Lower=2, Upper=1
    Replace when: different key OR (new is Exact AND old is not)
                  OR new_priority * 3 >= old_priority * 2

Tests:
  - Store and retrieve same position
  - Deeper entry survives shallow overwrite attempt
  - resize() changes capacity correctly
  - clear() removes all entries
  - 64MB default fits in memory

CRITIC CHECKS:
  - Entry size == 10 bytes? (must be exact)
  - Cluster size == 32 bytes? (alignment critical)
  - Replacement formula matches DECISIONS.md exactly?
  - No always-replace policy (that's a regression)

---

================================================================================
MILESTONE 2 — movepick.rs (Move Ordering)
================================================================================

Build: src/movepick.rs — new file

pub struct MovePicker {
    killer_moves: [[Option<Move>; 2]; 64],  // 2 slots per depth, 64 depths
}

impl MovePicker:
    pub fn new() -> Self
    pub fn order_moves(&self, moves: &MoveList, depth: u8) -> Vec<Move>
        // Order: captures (MVV-LVA) → killers → quiet moves
    pub fn order_captures(&self, moves: &MoveList) -> Vec<Move>
        // Captures only, MVV-LVA sorted
    pub fn store_killer(&mut self, mv: Move, depth: u8)
        // Shift killer slots: slot[1] = slot[0], slot[0] = mv
    pub fn clear(&mut self)

MVV-LVA score (frozen from DECISIONS.md DD01):
    fn mvv_lva(victim: Role, attacker: Role) -> i32:
        victim_value(victim) * 10 - attacker_value(attacker) + 10000
    Promotions: 5000
    Non-captures: 0
    Killers: 9000 (above quiet, below captures)

Tests:
  - Captures sort above quiet moves
  - Pawn takes queen scores higher than queen takes pawn
  - Killer moves sort above non-killer quiet moves
  - store_killer shifts correctly

CRITIC CHECKS:
  - MVV-LVA formula exactly matches DECISIONS.md DD01?
  - Killer score (9000) between captures (10000+) and quiet (0)?
  - No history heuristic (not in spec for v1.0)

---

================================================================================
MILESTONE 3 — time.rs (Time Management)
================================================================================

Build: src/time.rs — new file

pub struct TimeManager {
    start: Instant,
    budget_ms: u64,
}

impl TimeManager:
    pub fn new(wtime: u64, btime: u64, movestogo: u64, 
               movetime: Option<u64>, color: Color) -> Self
        // budget = remaining_time_for_color / 30
        // if movetime provided: budget = movetime
    pub fn elapsed_ms(&self) -> u64
    pub fn should_stop(&self) -> bool
        // return elapsed_ms() >= budget_ms

Tests:
  - should_stop() returns false immediately after creation
  - should_stop() returns true after budget elapsed
  - movetime overrides wtime/btime calculation
  - White uses wtime, black uses btime

CRITIC CHECKS:
  - Division is remaining_time / 30 exactly per DECISIONS.md DD03?
  - No dynamic adjustment (that's DD03-B, not in v1.0)

---

================================================================================
MILESTONE 4 — eval.rs (Add PST)
================================================================================

Extend existing eval.rs from skeleton to add PST evaluation.

BEFORE WRITING ANY CODE:
  Read frozen/pst.rs — ALL 384 values
  Read https://www.chessprogramming.org/Simplified_Evaluation_Function
  Verify: every value in frozen/pst.rs matches Michniewski exactly

New evaluate() function:
    score = material_score + pst_score
    material_score = sum(piece_values white) - sum(piece_values black)  [unchanged]
    pst_score = sum(PST[piece][square] for white) 
              - sum(PST[piece][square] for black)
    
    Square indexing: white pieces use square as-is (A1=0)
                    black pieces use mirrored square (flip rank)
    
    Return from WHITE's perspective (positive = white winning)

Tests:
  - Starting position still evaluates near 0 (PST symmetric)
  - Knight on e4 scores higher than knight on a1
  - King in corner scores higher than king in center (endgame)
  - BISHOP still == 300, KNIGHT still == 300 (material unchanged)
  - PST values are Michniewski exactly — spot check 10 values

CRITIC CHECKS (MOST IMPORTANT IN ENTIRE BUILD):
  grep -r "piece_square\|pst\|positional" src/eval.rs
  → Any non-Michniewski values: HARD BLOCK
  Spot check: Knight PST center value == Michniewski exactly?
  Spot check: King PST corner value (endgame) == Michniewski exactly?
  If ANY value deviates from frozen/pst.rs: BLOCKING

---

================================================================================
MILESTONE 5 — search.rs (PVS + TT + Killers + Check Extensions)
================================================================================

Extend existing search.rs from skeleton. This is the big milestone.
Build in this order: TT integration → PVS → Killers → Check Extensions

5a. Integrate TranspositionTable:
    - Probe TT at start of each node
    - If hit with sufficient depth: return cached score
    - Store result in TT after each node
    - Pass TT through search as &mut reference
    - Call tt.increment_age() at start of each new search

5b. Add PVS (Principal Variation Search) per DECISIONS.md DD10:
    - Search first move with full window [alpha, beta]
    - Search remaining moves with null window [alpha, alpha+1]
    - If null window search > alpha: re-search with full window
    - Apply at all non-quiescence nodes

5c. Add Killer moves per DECISIONS.md DD09:
    - Store beta-cutoff quiet moves as killers
    - Pass MovePicker through search
    - Use movepick.order_moves() instead of direct legal_moves()

5d. Add Check Extensions per DECISIONS.md DD06:
    - If position is in check: depth += 1
    - Track extension count, cap at MAX_EXTENSIONS=4

5e. Add Iterative Deepening per DECISIONS.md DD02:
    pub fn iterative_deepening(pos: &Chess, tm: &TimeManager, 
                                tt: &mut TranspositionTable) -> Move
        for depth in 1..=MAX_DEPTH:
            let (score, mv) = alpha_beta_search(pos, depth, tt)
            best_move = mv
            if tm.should_stop(): break
        return best_move

Tests:
  - Same mate-in-1 found with TT as without
  - TT hit rate > 0 on repeated positions
  - Iterative deepening returns move before time expires
  - Check extension fires on check positions
  - Killer moves stored and retrieved correctly
  - PVS returns same score as full alpha-beta on test positions

CRITIC CHECKS:
  - PVS null window is [alpha, alpha+1] exactly?
  - TT probe returns None on miss (not a default value)?
  - Check extension capped at MAX_EXTENSIONS=4?
  - Killers cleared between games (not between depths)?

---

================================================================================
MILESTONE 6 — main.rs (Update UCI for Hash + No Book)
================================================================================

Extend existing main.rs from skeleton:

6a. Add Hash option:
    On "uci" output include:
      option name Hash type spin default 64 min 1 max 65536
    
    Handle "setoption name Hash value <n>":
      tt.resize(n)  // resize transposition table to n MB

6b. Verify no internal opening book:
    Engine must NOT play from an internal book
    Must think from move 1
    
6c. Update go handler to use new components:
    - Parse wtime, btime, movestogo, movetime, depth
    - Create TimeManager from parsed values
    - Call iterative_deepening(pos, &tm, &mut tt)
    - Output info lines during search:
        info depth <n> score cp <n> nodes <n> time <ms> pv <move>
    - Output bestmove <move> after search

6d. Handle stop command:
    Set atomic flag that TimeManager checks
    Output bestmove immediately when stop received

Tests:
  - "uci" outputs Hash option
  - "setoption name Hash value 128" resizes TT
  - "go wtime 60000 btime 60000 movestogo 40" returns bestmove
  - "go movetime 1000" returns bestmove within ~1000ms
  - "stop" during search returns bestmove immediately
  - Engine plays different moves in different positions (not stuck)

CRITIC CHECKS:
  - No internal opening book code anywhere
  - bestmove format is lowercase UCI algebraic (e2e4 not E2E4)
  - info lines output before bestmove (not after)
  - Hash option name is exactly "Hash" (case sensitive for CCRL)

---

================================================================================
MILESTONE 7 — SIGMA GATES
================================================================================

GATE 1 — Run 30-position endgame benchmark:
    cargo run --release --bin benchmark

    Must pass:
      GATE 1a: illegal_moves == 0        HARD BLOCKER
      GATE 1b: pass_rate >= 90%          27/30 minimum
      GATE 1c: pruning_rate >= 50%       alpha-beta working
      GATE 1d: max_time < 900s           performance check

    Expected: pass_rate >= 100% (baseline from verified engine)
    If pass_rate < 90%: STOP. New features broke endgame play. Debug.

GATE 2 — Run 10 self-play games:

    Install cutechess-cli or use arena for self-play, OR write a simple
    self-play loop:
      Engine plays itself for 10 games at movetime=1000ms
      Verify: no crashes, no hangs, all games complete
      Verify: all moves are legal
      Verify: correct UCI output throughout
      Verify: engine responds to stop

    Both gates must pass before writing REPORT.md.

---

================================================================================
CRITIC ROLE — AFTER EVERY MILESTONE
================================================================================

STEP 1 — PRIOR DETECTION (run before reading any code):

  grep -r "rollout\|playout\|UCB\|visit_count\|MonteCarloNode" src/
  → Any hit: HARD BLOCK. Log as FALSE POSITIVE CAUGHT.

  grep -r "tch\|burn\|candle\|onnx\|neural\|embedding" src/
  → Any hit: HARD BLOCK.

  grep -r "325\|positional\|mobility\|king_safety" src/eval.rs
  → Any hit: BLOCKING — check against frozen/pst.rs

  Check frozen values in eval.rs:
    BISHOP==300, KNIGHT==300, PAWN==100, ROOK==500, QUEEN==900
    DELTA==200, MAX_EXTENSIONS==4
  → Any wrong: BLOCKING

  Check UCI in main.rs:
    id name == "CHPawn-FrozenKing"?
    id author == "CHP"?
    Hash option present?
    No internal book?
  → Any wrong: BLOCKING

STEP 2 — GATE SCORES:
  Gate 1 Frozen compliance:   1.0 (BLOCKING if <1.0)
  Gate 2 Architecture:        >=0.85
  Gate 3 Scientific validity: >=0.85
  Gate 4 Drift check:         >=0.85

STEP 3 — If Gate 1 < 1.0: fix before proceeding.

================================================================================
FALSE POSITIVE PROTOCOL
================================================================================

Most likely false positives for this build:

FP-A: MCTS — AlphaGo prior
  Signal: rollout, playout, UCB, visit_count
  Action: HARD BLOCK. Log. Fix. Re-run Critic.

FP-B: Wrong PST values — LLM generates its own instead of Michniewski
  Signal: Any PST value not matching frozen/pst.rs
  Action: BLOCKING. This is the #1 risk in this build.
  Fix: Delete generated PST. Use frozen/pst.rs exactly.

FP-C: Piece value drift
  Signal: BISHOP != 300 or KNIGHT != 300
  Action: BLOCKING.

FP-D: Always-replace TT (regression from verified engine)
  Signal: No priority calculation in store()
  Action: BLOCKING. Must use depth+age hybrid per DD04.

FP-E: Internal opening book
  Signal: Any book file, polyglot code, or hardcoded first moves
  Action: BLOCKING. CCRL requires no internal book.

When caught:
  Log in innovation_log.md as FALSE POSITIVE CAUGHT
  Document: type, what was generated, detection method, fix, verification
  This is the CHP money shot — document it clearly.

================================================================================
UPDATE LOGS AFTER EVERY MILESTONE
================================================================================

Append to innovation_log.md:
  - Milestone N complete
  - Dead ends avoided (list them)
  - Critic gate scores
  - False positive caught? (full doc if yes)
  - cargo test results
  - Next milestone focus

Update state_vector.md.

================================================================================
REPORT.md — WRITE WHEN COMPLETE
================================================================================

# CHPawn-FrozenKing v1.0 — Build Report

## Result
Phase 1 [complete / incomplete]

## Sigma Gates
| Gate | Target | Result | Status |
|------|--------|--------|--------|
| Illegal moves | 0 | [n] | PASS/FAIL |
| Endgame match rate | >=90% | [n/30] | PASS/FAIL |
| Pruning efficiency | >=50% | [n%] | PASS/FAIL |
| Self-play games | 10/10 complete | [n/10] | PASS/FAIL |

## Performance
| Metric | Verified Engine | CHPawn v1.0 |
|--------|----------------|-------------|
| Endgame match rate | 30/30 (100%) | [n/30] |
| 30-position time | 8.1s | [n]s |
| Nodes/second | [n] | [n] |

## False Positive Caught
[Document or "None caught — log as anomaly"]

## UCI Verification
[Confirm all CCRL-required commands work]

## CCRL Submission Readiness
[ ] 64-bit Windows binary builds with: cargo build --release --target x86_64-pc-windows-msvc
[ ] Engine responds to all required UCI commands
[ ] No internal opening book
[ ] Hash option configurable
[ ] No crashes in 10 self-play games
[ ] bestmove output is lowercase UCI algebraic

## Next Steps
[What version 1.1 will add]

================================================================================
IF SOMETHING BREAKS
================================================================================

Rust-specific issues:
  Borrow checker on board during search:
    → Use pos.play(&move) returns new position — never mutate in place

  shakmaty-syzygy version mismatch:
    → cargo search shakmaty-syzygy for latest version

  TT cluster alignment issues:
    → Use #[repr(C, align(32))] on TTCluster struct

  UCI not recognized by test GUI:
    → Every line ends with \n
    → No extra output between go and bestmove
    → bestmove must be LOWERCASE

If milestone fails twice:
  Log in dead_ends.md with full details
  Write REPORT.md: "Build incomplete. Failed at Milestone N."
  Stop. State exactly what broke.

================================================================================
GO — BUT READ ALL 5 FILES FIRST
================================================================================

Read DECISIONS.md, frozen/spec.md, frozen/pst.rs, dead_ends.md, RESEARCH.md.
Then run STEP 0.
Then build milestone by milestone.
Do not stop until REPORT.md is written.
