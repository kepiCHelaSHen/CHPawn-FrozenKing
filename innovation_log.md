# Innovation Log — CHPawn-FrozenKing
# Append only. Never delete.

---

## 2026-03-23 — Project scaffolded

- All decisions locked in DECISIONS.md (DD01-DD10)
- Frozen spec written: frozen/spec.md + frozen/pst.rs (Michniewski 384 values)
- Dead ends pre-loaded: MCTS, neural eval, wrong piece values, self-generated PST,
  internal book, always-replace TT, quiescence depth cap
- RESEARCH.md: Viridithas TT spec adopted for DD04 depth+age hybrid
- CCRL requirements locked in
- Sigma gates: Gate 1 (30-position endgame) + Gate 2 (10 self-play games)
- Baseline: 30/30 (100%) from verified Rust engine in 8.1 seconds
- Target ELO: 2200-2600 CCRL
- Status: PRIVATE — do not push to public GitHub

Ready for STEP 0 → copy verified engine skeleton → build milestones.

---

## 2026-03-23 — STEP 0 COMPLETE: skeleton copied, builds clean, UCI verified

- Copied all source files from D:\EXPERIMENTS\chp-chess-engine-rs\src\ to src/
- Copied Cargo.toml, updated: name="chpawn-frozen-king", lib name="chpawn_frozen_king"
- Copied Syzygy tablebases (KQvK, KRvK, KQvKR)
- cargo build: SUCCESS (warnings only: unused imports)
- cargo test: 16/16 PASS
- UCI verify: responds with id name, id author, uciok
- Note: UCI identity still shows "CHP Chess Engine" — will fix in Milestone 6
- Dead ends avoided: none triggered (skeleton is verified clean)
- Next: Milestone 1 — tt.rs (Transposition Table)

---

## 2026-03-23 — Milestones 1-6 COMPLETE

### Milestone 1 — tt.rs (Transposition Table)
- 10-byte TTEntry, 32-byte TTCluster (compile-time asserts)
- Depth+age hybrid replacement per DD04
- 16-bit key truncation, packed flags byte (age5+pv1+bound2)
- Tests: 11/11 PASS
- Dead ends avoided: DE-5 (no always-replace)

### Milestone 2 — movepick.rs (Move Ordering)
- MVV-LVA: victim_value*10 - attacker_value + 10000 (per frozen spec)
- Killers: 2 slots per depth, score=9000
- TT move ordering support
- Tests: 5/5 PASS

### Milestone 3 — time.rs (Time Management)
- DD03A: remaining_time / 30 (simple allocation)
- movetime override, stop flag via Arc<AtomicBool>
- Tests: 8/8 PASS

### Milestone 4 — eval.rs (PST Integration)
- PST sourced from frozen/pst.rs via #[path] module (no copy, no drift)
- Square indexing: white = sq^56, black = sq (CPW display order)
- Endgame detection: no queens on board
- King PST: MG (corner) vs EG (center) based on queen presence
- Tests: 9/9 PASS (7 new eval + 2 search that depend on eval)

### Milestone 5 — search.rs (PVS + TT + Killers + Check Extensions + ID)
- Converted to negamax internally (cleaner PVS implementation)
- PVS: null window [alpha, alpha+1], re-search on fail
- TT probe at every node, store after every node
- Check extensions: depth += 1 when in check, capped at MAX_EXTENSIONS=4
- Killer moves: store on beta cutoff for quiet moves
- Iterative deepening: depth 1..=max_depth with time management
- Kept minimax for backward-compatible pruning rate benchmark
- Tests: 14/14 PASS

### Milestone 6 — main.rs (UCI Update)
- id name = "CHPawn-FrozenKing" ✓
- id author = "CHP" ✓
- Hash option: default 64, min 1, max 65536 ✓
- setoption name Hash value N → tt.resize(N) ✓
- go wtime/btime/movestogo/movetime/depth parsing ✓
- stop command sets atomic flag ✓
- info lines during search (depth, score, nodes, time, nps, pv) ✓
- bestmove in lowercase UCI algebraic ✓
- No internal book ✓
- ~2.7M nps on starting position ✓

### Critic Checks (all pass)
- MCTS grep: 0 hits ✓
- Neural grep: 0 actual hits (only Rust `match` keyword) ✓
- Eval drift: BISHOP=300, KNIGHT=300 confirmed ✓
- Book grep: 0 hits ✓
- Frozen values: all 7 constants verified ✓
- FALSE POSITIVES CAUGHT: None (clean build)

### Gate 1 Results (30-position endgame benchmark)
- Gate 1a: illegal_moves = 0 → PASS
- Gate 1b: pass_rate = 30/30 (100%) → PASS
- Gate 1c: pruning_rate = 100% → PASS
- Gate 1d: max_time = 3210ms → PASS
- ALL SIGMA GATES PASSED

### Gate 2 Results (10-game self-play)
- 10/10 games completed → PASS
- All moves legal → PASS
- No crashes or hangs → PASS
- Results: 0 white wins, 0 black wins, 10 draws (expected for symmetric self-play)
- Avg moves/game: 98
- Note: Added 3-fold repetition detection to prevent infinite drawn games

### Full test suite
- 48/48 tests pass across all modules
- Zero regressions from skeleton

### FALSE POSITIVES CAUGHT: None
- Clean build across all 6 milestones
- Likely prevented by: dead_ends.md pre-loading + frozen/pst.rs module reference strategy
- Log as anomaly per CHP protocol

### REPORT.md WRITTEN — BUILD COMPLETE
- All sigma gates passed
- CCRL submission ready
- Next: Version 1.1 (aspiration windows + dynamic time management)

---

## 2026-03-24 — v0.0.2 COMPLETE: LMR + Aspiration Windows + Dynamic Time Management

### Feature 1 — Late Move Reductions (DD-LMR)
- LMR_THRESHOLD=2, LMR_REDUCTION=1
- Quiet moves after first 2 at depth >= 3 (not in check) reduced by 1 ply
- Re-search at full depth if reduced search beats alpha
- Integrates with PVS: reduced null-window → full-depth null-window → full window
- Tests: 2 new (constants + correctness), all pass

### Feature 2 — Aspiration Windows (DD07)
- ASPIRATION_WINDOW=50 centipawns
- After depth 1: search within [prev_score-50, prev_score+50]
- Fail low: widen alpha by 2x, re-search
- Fail high: widen beta by 2x, re-search
- Window >= 800cp: fall back to full window
- Tests: 2 new (constant + mate finding), all pass

### Feature 3 — Dynamic Time Management (DD03-B)
- Sudden death: remaining_time / 20 (was /30)
- Known movestogo: remaining_time / (movestogo + 5)
- hard_limit = base_time * 3
- should_stop(): soft stop between ID iterations
- hard_stop(): hard limit inside search nodes
- Tests: 6 updated (new formulas), 2 new (hard limit, movetime), all pass

### Sigma Gates (50-position benchmark)
- Gate 1: illegal_moves = 0 → PASS
- Gate 2: pass_rate = 50/50 (100%) → PASS
- Gate 3: pruning_rate = 100% → PASS
- Gate 4: max_time = 4477ms → PASS
- ALL SIGMA GATES PASSED

### Test Suite
- 54/54 tests pass (was 48 in v1.0, +6 new)
- Zero regressions

### Dead ends avoided
- No null move pruning (still deferred to v1.2 per DD05)
- No history heuristic (not in spec)

### FALSE POSITIVES CAUGHT: None
- Clean build, all frozen values intact

### REPORT_v002.md WRITTEN — v0.0.2 BUILD COMPLETE

---

## 2026-03-24 — v0.0.3 COMPLETE: Null Move Pruning + History Heuristic + SEE

Context: CHPawn went 0-19 against 2200 ELO engines. Root cause: depth gap.
Opponents reaching depth 14, CHPawn reaching depth 9. This session closes the gap.

### Feature 1 — Null Move Pruning (DD05)
- R=2 (null move reduction)
- Conditions: depth >= 3, not in check, has non-pawn pieces, beta not mate
- Zugzwang detection: has_non_pawn_pieces() — skip if only K+P
- MATE_THRESHOLD=900,000
- make_null_move_pos() via shakmaty Setup (flip turn, clear EP)
- Tests: 4 new (constants, zugzwang skip, pieces detect, position creation)
- Dead end DE-7 added: null move in zugzwang positions

### Feature 2 — History Heuristic (DD-HISTORY)
- history[from][to] table, 64x64, clamped to [-16384, 16384]
- Bonus = depth * depth
- On beta cutoff: reward cutoff move, penalize other searched quiets
- Quiet move ordering uses history score (replaces flat 0)
- Tests: 4 new (update, clamp, ordering, clear)

### Feature 3 — SEE Capture Ordering (DD-SEE)
- Simple SEE: captured_value - attacker_value
- Winning captures (SEE >= 0): CAPTURE_BASE + MVV-LVA (as before)
- Losing captures (SEE < 0): LOSING_CAPTURE_BASE + SEE (below quiet moves)
- Stops wasting time on Qxp-defended-by-pawn
- Tests: 2 new (losing below quiet, winning above killers)

### Sigma Gates (50-position benchmark)
- Gate 1: illegal_moves = 0 → PASS
- Gate 2: pass_rate = 50/50 (100%) → PASS
- Gate 3: pruning_rate = 100% → PASS
- Gate 4: max_time = 3725ms → PASS
- ALL SIGMA GATES PASSED

### Test Suite
- 64/64 tests pass (was 54 in v0.0.2, +10 new)
- Zero regressions

### Frozen Value Verification
- BISHOP=300, KNIGHT=300 ✓
- DELTA=200, MAX_EXTENSIONS=4 ✓
- MCTS grep: 0 hits ✓
- Neural grep: 0 hits ✓
- No piece value drift ✓

### FALSE POSITIVES CAUGHT: None
- Clean build, all frozen values intact

### REPORT_v003.md WRITTEN — v0.0.3 BUILD COMPLETE
