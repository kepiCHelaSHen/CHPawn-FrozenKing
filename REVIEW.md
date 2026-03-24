# Code Review — CHPawn-FrozenKing v1.0
# Date: 2026-03-24

## CRITICAL Issues

### C1 — No threefold repetition detection in search
**File:** search.rs (negamax, root_search)
**Impact:** Engine will loop forever in drawn positions during CCRL games. No position
history is tracked. The engine could repeat the same moves indefinitely, losing on time.
CCRL disqualifier: "Engine hangs → disqualified."
**Fix:** Pass position hash history through search. Return DRAW on repetition.

### C2 — No fifty-move rule detection in search
**File:** search.rs (negamax)
**Impact:** Engine doesn't recognize 50-move rule draws. Will keep searching for wins
in drawn endgame positions. In CCRL 40/15 games with long endgames, the engine could
waste all its time searching a drawn position.
**Fix:** Check `pos.halfmoves() >= 100` at start of negamax. Return DRAW.

### C3 — TT mate score overflow — i16 truncation corrupts entries
**File:** search.rs lines 164, 311 — `best_score as i16`
**Impact:** CHECKMATE = 1,000,000 overflows i16 (max 32,767). Rust `as i16` silently
truncates, producing garbage scores in the TT. A stored mate score of 999,997 becomes
-32,035 after truncation. This corrupted entry could cause the engine to think a winning
position is losing, potentially causing illegal-seeming play or missed mates.
**Fix:** Clamp scores to i16-safe range (±32,000) before TT storage.

## WARNING Issues

### W1 — TT mate score ply adjustment missing
**File:** search.rs lines 233-247 (TT probe), 311 (TT store)
**Impact:** Mate scores include ply distance from root (`-CHECKMATE + ply`). When stored
in TT and probed from a different ply, the mate distance is wrong. Engine may not prefer
shortest mate. Low practical impact since mates are found at terminal nodes.
**Note:** Fixing C3 (clamping) makes this moot for v1.0. Proper ply adjustment is a v1.1
improvement.

### W2 — Unused constants and functions generating compiler warnings
**Files:** search.rs (BASE_DEPTH, MATE_THRESHOLD, order_moves_simple)
**Impact:** No functional impact. Noise in build output.

## MINOR Issues

### M1 — Endgame detection uses queens==0
**File:** eval.rs line 59
**Impact:** KQvKR (queen present) uses middlegame king PST — king doesn't centralize.
Suboptimal but tablebases handle these positions. Acceptable for v1.0.

### M2 — OOM risk on very large Hash values
**File:** tt.rs resize(), main.rs setoption
**Impact:** Setting Hash to 65536 (64GB) would cause OOM panic. CCRL uses 256-512MB.
Not a real-world issue.

### M3 — writeln!().unwrap() could panic if stdout closes
**File:** main.rs lines 44-47, 101, 112
**Impact:** If the GUI disconnects mid-search, the engine panics instead of exiting
gracefully. Standard practice for UCI engines. Not a CCRL issue.

### M4 — Dead code from skeleton (old minimax, old quiescence)
**File:** search.rs lines 416-567
**Impact:** ~150 lines of dead code. Used only by benchmark's pruning rate test.
Not a bug but code hygiene issue.

### M5 — Check extension test has dead variable assignments
**File:** search.rs lines 668-670
**Impact:** Test works but has unused intermediate variables.

## Items Verified OK

| Check | Status | Notes |
|-------|--------|-------|
| Stop flag frequency | OK | Every 2048 nodes (~0.76ms at 2.7M nps) |
| Quiescence infinite loop | OK | Captures reduce pieces; terminates naturally |
| Move packing in TT | OK | Packed moves used only for ordering, never unpacked |
| PST indexing | OK | White=sq^56, Black=sq. Verified with A2/A7 examples |
| UCI setoption Hash | OK | Resize works correctly for normal values |
| Integer overflow (non-mate) | OK | All eval scores fit i32; MVV-LVA max=200000 fits i32 |
| bestmove always sent | OK | root_search always finds ≥1 move before checking time |
| Frozen values correct | OK | P=100 N=300 B=300 R=500 Q=900 DELTA=200 MAX_EXT=4 |
| No MCTS/NN/book | OK | grep verified clean |
