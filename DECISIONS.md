# CHPawn-FrozenKing — Design Decision Log
# Every decision made before a single line of code was written.
# This file is the audit trail. When something breaks in week 6,
# the answer to why is in here.
# Append only. Never delete entries.

================================================================================
PROJECT IDENTITY
================================================================================

Name:           CHPawn-FrozenKing
UCI Name:       CHPawn-FrozenKing
UCI Author:     CHP
Version:        1.0
Language:       Rust
Protocol:       Context Hacking Protocol (CHP)
Visibility:     PRIVATE until CCRL submission day
Goal:           Appear on CCRL 40/15 leaderboard
Target ELO:     2200-2600
Date:           2026-03-23

Lineage:
  Python proof       → D:\EXPERIMENTS\chp-chess-engine\       PUBLIC
    Result: 27/30 (90.0%), 45 minutes, all gates pass
  Rust verification  → D:\EXPERIMENTS\chp-chess-engine-rs\    PUBLIC
    Result: 30/30 (100%), 8.1 seconds, 327ms worst case
  This engine        → D:\EXPERIMENTS\CHPawn-FrozenKing\      PRIVATE

================================================================================
CCRL SUBMISSION REQUIREMENTS (FROZEN)
================================================================================

Source: https://ccrl.chessdom.com/ccrl/4040/about.html
Time control: 40 moves in 15 minutes (repeating)
Hash: 256MB or 512MB — engine MUST accept setoption name Hash
Pondering: OFF
Opening book: CCRL provides it — engine must NOT have internal book
Tablebases: 4-6 piece Syzygy — already have this
Threads: Single thread for testing
Platform: 64-bit Windows executable

Required UCI commands:
  uci, isready, ucinewgame, position, go, stop, quit
  setoption name Hash value <MB>

Required output:
  info depth <n> score cp <n> nodes <n> time <ms> pv <move>
  bestmove <move> (lowercase algebraic: e2e4 not E2E4)

Submission: Post on CCRL forum with public GitHub download link.
            They test it themselves. No games played by you required.

Hard disqualifiers:
  - Engine crashes or hangs → disqualified
  - Cannot disable internal opening book → cannot participate
  - Illegal moves → disqualified

================================================================================
FOUNDATION — INHERITED AND FROZEN
================================================================================

These are proven in the verified engine. Not re-litigated.

FD01 — Algorithm: Minimax + Alpha-Beta Pruning (R&N 4th Ed Chapter 5)
FD02 — Base piece values: P=100, N=300, B=300, R=500, Q=900, K=20000
FD03 — Quiescence: unbounded captures, DELTA=200, no depth cap
FD04 — Tablebase: Syzygy via shakmaty-syzygy (files already in syzygy/)
FD05 — UCI: already working in verified engine — copy as skeleton
FD06 — No MCTS. No neural networks. No internal opening book. Ever.
FD07 — Sigma gate: 30-position endgame battery (30/30 baseline)
FD08 — Starting point: copy verified Rust engine as skeleton, build new features on top

================================================================================
VERSION 1.0 DECISIONS
================================================================================

## DD01 — MVV-LVA Move Ordering
Decision:   YES — already in verified engine
Date:       2026-03-23
Est. ELO:   +150-200

Frozen spec (from RESEARCH.md — Viridithas reference):
  score = victim_value * 10 - attacker_value + 10000
  All captures score above non-captures
  Promotions score 5000
  Non-captures score 0

Victim values (centipawns): P=100, N=300, B=300, R=500, Q=900
Attacker values: same as victim values

---

## DD02 — Iterative Deepening
Decision:   YES
Date:       2026-03-23
Est. ELO:   +50-100 (required for time management)

Frozen spec:
  Search depth 1, 2, 3... until time budget exhausted
  Store best move from each completed iteration
  On time expiry: return best move from last completed depth
  Minimum depth: 1 (always complete at least depth 1)

---

## DD03 — Time Management (Option A — Simple)
Decision:   YES, Option A — superseded by DD03-B in v0.0.2
Date:       2026-03-23
Est. ELO:   Required for CCRL

Original spec (v1.0):
  time_per_move = remaining_time / 30

## DD03-B — Dynamic Time Management
Decision:   YES — DONE in v0.0.2
Date:       2026-03-24
Est. ELO:   +10-20

Frozen spec:
  base_time = remaining_time / 20  (sudden death)
  base_time = remaining_time / (movestogo + 5)  (known movestogo)
  hard_limit = base_time * 3  (never think longer than this)
  movetime: budget = movetime, no hard limit multiplier
  should_stop(): soft stop at base budget (between ID iterations)
  hard_stop(): never exceed hard_limit (inside search nodes)

---

## DD04 — Transposition Table
Decision:   YES — depth+age hybrid replacement
Date:       2026-03-23
Est. ELO:   +100-150

Research finding (RESEARCH.md):
  Modern engines converge on 10-byte entries, 3 per 32-byte cluster
  Depth+age hybrid is standard — always-replace wastes deep results

Frozen spec:
  Default size: 64MB (configurable via setoption name Hash)
  Entry: key(u16) + move(u16) + score(i16) + depth(u8) + flags(u8) + eval(i16)
  Cluster: 3 entries per 32-byte aligned cluster
  Replacement: depth+age hybrid
    priority = depth + flag_bonus + age_differential^2/4 + pv_bonus
    flag_bonus: Exact=3, LowerBound=2, UpperBound=1
    Replace when: different position OR new is Exact and old isn't
                  OR new_priority * 3 >= old_priority * 2
  Zobrist hashing for position keys

---

## DD05 — Null Move Pruning
Decision:   YES — DONE in v0.0.3
Date:       2026-03-24
Est. ELO:   +100-150

Frozen spec:
  R = 2 (null move reduction)
  Conditions: depth >= 3, NOT in check, has non-pawn pieces, beta not mate score
  Zugzwang protection: has_non_pawn_pieces() — skip if only K+P
  MATE_THRESHOLD = 900,000
  On null move cutoff: return beta

---

## DD06 — Check Extensions
Decision:   YES
Date:       2026-03-23
Est. ELO:   +30-50

Frozen spec:
  When king is in check: depth += 1
  MAX_EXTENSIONS = 4 per search path (prevents explosion)

---

## DD07 — Aspiration Windows
Decision:   YES — DONE in v0.0.2
Date:       2026-03-24
Est. ELO:   +20-40

Frozen spec:
  Initial window: 50 centipawns around previous iteration score
  Fail low: widen alpha by 2x, re-search
  Fail high: widen beta by 2x, re-search
  Window >= 800cp: fall back to full window
  Depth 1 always uses full window

---

## DD08 — Piece Square Tables (PST)
Decision:   YES — separate frozen file
Date:       2026-03-23
Est. ELO:   +100-200

Source (frozen):
  Tomasz Michniewski — Simplified Evaluation Function
  https://www.chessprogramming.org/Simplified_Evaluation_Function
  ALL 384 values (6 piece types x 64 squares) trace to this source exactly.

Research finding: All three reference engines use NNUE — no PST to copy.
                  Michniewski is the only valid non-NNUE reference.

Frozen file: frozen/pst.rs — IMMUTABLE
             Separate from main spec — Critic verifies all 384 values
             independently before any other gate check.

CHP note: Highest false positive risk in entire build.
          Builder will generate its own PST values.
          Critic must grep for any value not in Michniewski table.

---

## DD09 — Killer Moves
Decision:   YES
Date:       2026-03-23
Est. ELO:   +20-40

Frozen spec:
  2 killer move slots per depth
  Store moves that cause beta cutoff but are not captures
  Try killers before quiet moves, after captures in move ordering
  Clear killer table on new search (not between iterations)

---

## DD10 — Principal Variation Search (PVS)
Decision:   YES
Date:       2026-03-23
Est. ELO:   +30-50

Frozen spec:
  Search first move with full window [alpha, beta]
  Search remaining moves with null window [alpha, alpha+1]
  If null window search returns score > alpha: re-search with full window
  Apply at all nodes except quiescence
  Works with TT, killers, MVV-LVA already locked in

## DD-LMR — Late Move Reductions
Decision:   YES — DONE in v0.0.2
Date:       2026-03-24
Est. ELO:   +30-50

Frozen spec:
  LMR_THRESHOLD = 2 (reduce moves after first 2)
  LMR_REDUCTION = 1 (reduce by 1 ply)
  Conditions: move_index >= 2 AND depth >= 3 AND not in check AND move is quiet
  Reduced search: depth - 2 instead of depth - 1
  If reduced search > alpha: re-search at full depth - 1
  Integrates with PVS: reduced null-window first, then full-depth null-window,
  then full-window if needed

---

## DD-HISTORY — History Heuristic
Decision:   YES — DONE in v0.0.3
Date:       2026-03-24
Est. ELO:   +20-40

Frozen spec:
  history[from][to] table, 64x64 indexed by squares
  HISTORY_MAX = 16384, HISTORY_MIN = -16384
  Bonus = depth * depth
  On beta cutoff: reward cutoff move, penalize other searched quiets
  Quiet move ordering: score = history[from][to]

---

## DD-SEE — Static Exchange Evaluation
Decision:   YES — DONE in v0.0.3
Date:       2026-03-24
Est. ELO:   +10-20

Frozen spec:
  Simple SEE: captured_value - attacker_value
  Winning captures (SEE >= 0): CAPTURE_BASE + MVV-LVA (unchanged)
  Losing captures (SEE < 0): LOSING_CAPTURE_BASE + SEE (below quiet moves)
  LOSING_CAPTURE_BASE = -1000

---

## DD-PASSED-PAWNS — Passed Pawn Bonuses
Decision:   YES — DONE in v0.0.4
Date:       2026-03-24
Est. ELO:   +20-40
Source:     chessprogramming.org/Passed_Pawns
Frozen spec: PASSED_PAWN_BONUS = [0, 10, 20, 30, 50, 75, 100, 0]

---

## DD-PAWN-STRUCTURE — Pawn Structure Penalties
Decision:   YES — DONE in v0.0.4
Date:       2026-03-24
Est. ELO:   +15-25
Source:     chessprogramming.org/Pawn_Structure
Frozen spec: DOUBLED_PAWN_PENALTY = -20, ISOLATED_PAWN_PENALTY = -15

---

## DD-ROOK-FILES — Rook on Open/Semi-Open File
Decision:   YES — DONE in v0.0.4
Date:       2026-03-24
Est. ELO:   +10-20
Source:     chessprogramming.org/Rook_on_Open_File
Frozen spec: ROOK_OPEN_FILE_BONUS = 25, ROOK_SEMI_OPEN_FILE_BONUS = 10

---

## DD-BISHOP-PAIR — Bishop Pair Bonus
Decision:   YES — DONE in v0.0.4
Date:       2026-03-24
Est. ELO:   +5-15
Source:     chessprogramming.org/Bishop_Pair
Frozen spec: BISHOP_PAIR_BONUS = 50

---

## DD-KING-SAFETY — King Safety Evaluation
Decision:   YES — DONE in v0.0.4
Date:       2026-03-24
Est. ELO:   +15-30
Source:     chessprogramming.org/King_Safety (simplified)
Frozen spec: KING_ATTACKER_PENALTY = -10, KING_SHIELD_BONUS = 10
             Only applied in middlegame (queens on board)

---

## DD-FUTILITY — Futility Pruning
Decision:   YES — DONE in v0.0.5
Date:       2026-03-24
Est. ELO:   +20-40
Source:     chessprogramming.org/Futility_Pruning
Frozen spec: FUTILITY_MARGIN = [0, 100, 200, 300]
             depth <= 3, not in check, not PV node, alpha not mate score

---

## DD-RAZOR — Razoring
Decision:   YES — DONE in v0.0.5
Date:       2026-03-24
Est. ELO:   +15-25
Source:     chessprogramming.org/Razoring
Frozen spec: RAZOR_MARGIN = [0, 300, 500]
             depth <= 2, not in check

---

## DD-LMR-LOG — Logarithmic LMR Formula
Decision:   YES — DONE in v0.0.5
Date:       2026-03-24
Est. ELO:   +10-20
Source:     chessprogramming.org/Late_Move_Reductions
Frozen spec: reduction = max(1, (ln(depth) * ln(move_index) / 2) as i32)

---

## DD-IID — Internal Iterative Deepening
Decision:   YES — DONE in v0.0.5
Date:       2026-03-24
Est. ELO:   +10-20
Source:     chessprogramming.org/Internal_Iterative_Deepening
Frozen spec: IID_DEPTH_THRESHOLD = 4, IID_REDUCTION = 2

---

================================================================================
VERSION 1.0 SUMMARY
================================================================================

Features v1.0: DD01+DD02+DD03A+DD04+DD06+DD08+DD09+DD10
Features v0.0.2: +DD-LMR+DD07+DD03B
Features v0.0.3: +DD05+DD-HISTORY+DD-SEE
Features v0.0.4: +DD-PASSED-PAWNS+DD-PAWN-STRUCTURE+DD-ROOK-FILES+DD-BISHOP-PAIR+DD-KING-SAFETY
Features v0.0.5: +DD-FUTILITY+DD-RAZOR+DD-LMR-LOG+DD-IID
Deferred: none

ELO estimate:
  Rust verified baseline:       ~1400-1600
  + MVV-LVA (DD01):             +150-200
  + Iterative Deepening (DD02): +50-100
  + TT depth+age (DD04):        +100-150
  + Check Extensions (DD06):    +30-50
  + PST Michniewski (DD08):     +100-200
  + Killer Moves (DD09):        +20-40
  + PVS (DD10):                 +30-50
  Total estimated:              2200-2600 CCRL

================================================================================
SIGMA GATES (FROZEN)
================================================================================

GATE 1 — Endgame battery (inherited, unchanged)
  30 positions: 10 KQvK + 10 KRvK + 10 KQvKR
  Pass rate: >= 90% (27/30 minimum)
  Baseline: 30/30 (100%) from verified engine
  Illegal moves: 0 (hard blocker)

GATE 2 — CCRL readiness
  Run 10 self-play games at default time control
  Verify: no crashes, no hangs, no illegal moves
  Verify: correct UCI output throughout (info lines + bestmove)
  Verify: engine responds to stop command correctly

================================================================================
ITERATION ROADMAP
================================================================================

1.0 — Ship. Get on CCRL. Log the ELO.
0.0.2 — LMR + Aspiration windows + Dynamic time management (DD-LMR + DD07 + DD03B) — DONE
0.0.3 — Null move pruning + History heuristic + SEE (DD05 + DD-HISTORY + DD-SEE) — DONE
0.0.4 — Eval improvements: passed pawns, pawn structure, rook files, bishop pair, king safety — DONE
0.0.5 — Search efficiency: futility pruning, razoring, log LMR, IID — DONE
1.x — Climb weekly. Every change CHP-verified. Every ELO delta logged.

================================================================================
CHANGE LOG
================================================================================

2026-03-23 — All decisions locked. DECISIONS.md final version written.
             Python result: 27/30 (90.0%), 45 min
             Rust result:   30/30 (100%), 8.1s, 327ms worst case
             RESEARCH.md: Viridithas TT spec adopted for DD04
             CCRL rules locked in
             Ready to build.
2026-03-24 — v0.0.2 features added: LMR, Aspiration Windows, Dynamic Time Management.
             50/50 benchmark pass (100%). All sigma gates pass.
             DD03-B, DD07, DD-LMR all locked.
2026-03-24 — v0.0.3 features added: Null Move Pruning, History Heuristic, SEE.
             50/50 benchmark pass (100%). All sigma gates pass.
             DD05, DD-HISTORY, DD-SEE all locked.
2026-03-24 — v0.0.4 features added: Passed pawns, pawn structure, rook files, bishop pair, king safety.
             50/50 benchmark pass (100%). All sigma gates pass.
             DD-PASSED-PAWNS, DD-PAWN-STRUCTURE, DD-ROOK-FILES, DD-BISHOP-PAIR, DD-KING-SAFETY locked.
2026-03-24 — v0.0.5 features added: Futility pruning, Razoring, Logarithmic LMR, IID.
             50/50 benchmark pass (100%). Zero compiler warnings. All sigma gates pass.
             DD-FUTILITY, DD-RAZOR, DD-LMR-LOG, DD-IID locked. Dead code cleaned up.
