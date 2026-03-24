# RESEARCH.md — Engine Source Analysis
# Fetched 2026-03-23 from GitHub raw source files
# Purpose: Extract MVV-LVA, TT, and PST implementation details
#          from three modern engines for CHPawn-FrozenKing v1.0 decisions

================================================================================
ENGINE 1: Viridithas (Rust) — cosmobobak/viridithas (branch: master)
================================================================================

## Files examined
- src/movepicker.rs — capture scoring
- src/transpositiontable.rs — TT implementation
- src/evaluation.rs — eval and piece values

## 1. MVV-LVA / Capture Move Ordering

Viridithas does NOT use pure MVV-LVA. It uses MVV + capture history:

```
score = WINNING_CAPTURE_BONUS + MVV_SCORE[capture] + tactical_hist_bonus
```

Constants:
- WINNING_CAPTURE_BONUS = 10,000,000

MVV_SCORE table (indexed by piece type):
```
const MVV_SCORE: [i32; 6] = [0, 2400, 2400, 4800, 9600, 0];
```

Mapping:
- Index 0 (Pawn):   0
- Index 1 (Knight):  2,400
- Index 2 (Bishop):  2,400
- Index 3 (Rook):    4,800
- Index 4 (Queen):   9,600
- Index 5 (King):    0

Note: No attacker component in the MVV table — attacker influence comes
from the tactical_hist[threat_to][capture][piece][to] bonus instead.
SEE check is deferred to move yielding, not applied during scoring.

## 2. Transposition Table

Entry structure — 10 bytes:
```rust
pub struct TTEntry {
    pub key: u16,         // 2 bytes — truncated from 64-bit Zobrist
    pub m: Option<Move>,  // 2 bytes
    pub score: i16,       // 2 bytes
    pub depth: u8,        // 1 byte
    pub info: PackedInfo, // 1 byte (5 bits age, 1 bit PV, 2 bits bound)
    pub evaluation: i16,  // 2 bytes
}
```

Cluster: 3 entries per cluster, 32-byte aligned, 2 bytes padding:
```rust
const CLUSTER_SIZE: usize = 3;

#[repr(C, align(32))]
struct TTCluster {
    entries: [TTEntry; 3],
    padding: [u8; 2],
}
```

Key storage: 16-bit truncated (`key as u16`).
Full 64-bit key used for cluster index via fixed-point multiplication.

Replacement policy — depth-preferred with age weighting:
```rust
let insert_priority =
    depth + insert_flag_bonus + (age_differential * age_differential) / 4 + i32::from(pv);
```
- Flag bonuses: Exact=3, Lower=2, Upper=1
- Replace when: different position, OR new is Exact and old isn't,
  OR insert_priority * 3 >= record_priority * 2

Bound flags (2 bits):
- None  = 0
- Upper = 1 (score <= actual)
- Lower = 2 (score >= actual)
- Exact = 3

Default size: not hardcoded; set via resize(bytes).
Thread safety: atomic u64 operations on cluster access.

## 3. Piece Square Tables

NOT PRESENT. Viridithas uses NNUE for evaluation.

SEE piece values (used for move ordering and pruning, not eval):
- Pawn:   233 cp
- Knight: 446 cp
- Bishop: 446 cp
- Rook:   716 cp
- Queen:  1253 cp
- King:   0 cp

================================================================================
ENGINE 2: Alexandria (C++) — PGG106/Alexandria (branch: master)
================================================================================

## Files examined
- src/search.cpp — search and TT usage
- src/movepicker.cpp — capture scoring
- src/ttable.h — TT entry structure
- src/ttable.cpp — TT replacement policy
- src/eval.h — evaluation
- src/types.h — piece values and constants
- src/history.cpp — capture history scoring

## 1. MVV-LVA / Capture Move Ordering

Alexandria does NOT use pure MVV-LVA. It uses SEE value + capture history:

```cpp
moveList->moves[i].score = SEEValue[capturedPiece] * 16 + GetCapthistScore(pos, sd, move)
```

SEEValue array (defined in types.h):
```cpp
constexpr int SEEValue[15] = { 100, 422, 422, 642, 1015, 0,
                               100, 422, 422, 642, 1015, 0, 0, 0, 0 };
```

Mapping (duplicated for white/black pieces):
- Pawn (WP=0, BP=6):   100
- Knight (WN=1, BN=7): 422
- Bishop (WB=2, BB=8): 422
- Rook (WR=3, BR=9):   642
- Queen (WQ=4, BQ=10): 1015
- King (WK=5, BK=11):  0

Capture history: indexed by [PieceTo(move)][capturedPiece].
Updated with depth-scaled bonus/malus after search.

SEE filtering: captures with poor SEE are demoted to "bad captures"
stage. Threshold: SEEThreshold = -score / 32 + 236.

## 2. Transposition Table

Entry structure — 10 bytes:
```cpp
struct TTEntry {
    PackedMove move;     // 2 bytes — default NOMOVE
    int16_t score;       // 2 bytes — default SCORE_NONE
    int16_t eval;        // 2 bytes — default SCORE_NONE
    TTKey ttKey;         // 2 bytes — 16-bit hash key
    uint8_t depth;       // 1 byte
    uint8_t ageBoundPV;  // 1 byte (5 bits age, 1 bit PV, 2 bits bound)
};
```

Bucket: 3 entries per bucket, 32-byte aligned, 2-byte padding.

Key storage: 16-bit truncated from full ZobristKey.

Replacement policy — depth-based with age weighting (Stockfish-inspired):
- Score formula: `depth - ((MAX_AGE + TTAge - age) & AGE_MASK) * 4`
- Selects entry with lowest age-adjusted depth score in bucket
- Overwrite conditions (any true):
  1. bound == HFEXACT (exact score)
  2. Key doesn't match (different position)
  3. New search deeper: `depth + 5 + 2*pv > tte->depth`
  4. Entry is stale: entry age != current TTAge

Age system: MAX_AGE=32, AGE_MASK=31, 5-bit counter.
Flag types: HFNONE, plus upper/lower/exact bounds.

Score constants:
- MATE_SCORE: 32000
- MATE_FOUND: 31744 (MATE_SCORE - MAXPLY)
- SCORE_NONE: 32001
- MAXPLY/MAXDEPTH: 256

Default size: not hardcoded; requires explicit MB allocation via InitTT(MB).
Bucket count: `(ONE_MB * MB / sizeof(TTBucket)) - 3`.

## 3. Piece Square Tables

NOT PRESENT. Alexandria uses NNUE (Finny NNUE variant) for evaluation.

Material values used for scaling (from eval.h):
- Pawn: 100, Knight: 422, Bishop: 422, Rook: 642, Queen: 1015

Eval pipeline:
1. Raw NNUE output via NNUE::output(pos, FinnyPointer)
2. Clamped within mate boundaries
3. Scaled by 50-move rule: (200 - fiftyMoveCounter) / 200
4. Material scaling: (eval * scale) / 1024, where scale = (22400 + materialValue) / 32
5. Correction factor added
6. Final clamp

================================================================================
ENGINE 3: Stormphrax (C++) — Ciekce/Stormphrax (branch: main)
================================================================================

## Files examined
- src/search.cpp — search and TT usage
- src/movepick.h — capture scoring
- src/ttable.h — TT entry structure
- src/see.h — SEE values
- src/tunable.h — tunable parameters including SEE values

## 1. MVV-LVA / Capture Move Ordering

Stormphrax does NOT use traditional MVV-LVA. It uses SEE value + noisy history:

```cpp
score += m_history.noisyScore(move, captured, m_pos.threats()) / 8;
score += see::value(captured);
```

For promotion captures: adds `see::value(Queen) - see::value(Pawn)`.

SEE values (from tunable.h, tunable defaults):
- Pawn:   100 (range: 50-200, step: 7.5)
- Knight: 450 (range: 300-700, step: 25)
- Bishop: 450 (range: 300-700, step: 25)
- Rook:   650 (range: 400-1000, step: 30)
- Queen:  1250 (range: 800-1600, step: 40)

Values stored in `tunable::g_seeValues` array (13 elements, i32).
All values are auto-tunable with SPSA.

## 2. Transposition Table

Entry structure — 10 bytes:
```cpp
struct Entry {
    u16 key;          // 2 bytes — 16-bit hash verification
    i16 score;        // 2 bytes
    i16 staticEval;   // 2 bytes
    Move move;        // 2 bytes (presumably)
    u8 offsetDepth;   // 1 byte — depth + offset of 7
    u8 agePvFlag;     // 1 byte (5 bits age, 1 bit PV, 2 bits flag)
};
```

Cluster: 3 entries per cluster, aligned to 32 bytes.

Key storage: 16-bit from 64-bit position key.
Cluster indexing: `(key * clusterCount) >> 64` (single-mul fixed-point).

Depth offset: 7 (allows storing negative depths).

Flag types (2 bits):
```cpp
enum class TtFlag : u8 {
    kNone = 0,
    kUpperBound,
    kLowerBound,
    kExact,
};
```

Replacement policy: age-based generational.
Age cycles: `(age + 1) % 32`, 5-bit counter.

Default size: 64 MiB.
Size range: 1 to 67,108,864 MiB.

## 3. Piece Square Tables

NOT PRESENT. Stormphrax uses NNUE for evaluation.

Key tunable search parameters (for reference):
- Razoring margin: 315
- Reverse futility pruning margin: 71
- Futility pruning margin: 261
- Probcut threshold: 303
- LMR quiet base: 83, divisor: 218
- LMR noisy base: -12, divisor: 248
- Max history: 15769
- Default moves to go (time mgmt): 19
- Material scaling base: 26500

================================================================================
CROSS-ENGINE COMPARISON
================================================================================

## MVV-LVA Summary

| Feature              | Viridithas          | Alexandria          | Stormphrax          |
|----------------------|---------------------|---------------------|---------------------|
| Pure MVV-LVA?        | No                  | No                  | No                  |
| Victim scoring       | MVV_SCORE table     | SEEValue * 16       | see::value()        |
| Attacker component   | Via capture history | Via capture history | Via noisy history   |
| History integration  | Yes (tactical_hist) | Yes (captHist)      | Yes (noisyScore/8)  |
| SEE filtering        | Deferred to yield   | Threshold-based     | During generation   |

None of these engines use the classic MVV-LVA formula
(victim_value - attacker_value). All three use victim-value + learned
capture history instead.

## SEE Piece Values Comparison

| Piece  | Viridithas | Alexandria | Stormphrax | CHPawn-FrozenKing (DD) |
|--------|------------|------------|------------|------------------------|
| Pawn   | 233        | 100        | 100        | 100                    |
| Knight | 446        | 422        | 450        | 300                    |
| Bishop | 446        | 422        | 450        | 300                    |
| Rook   | 716        | 642        | 650        | 500                    |
| Queen  | 1253       | 1015       | 1250       | 900                    |
| King   | 0          | 0          | 0          | 20000                  |

## Transposition Table Comparison

| Feature             | Viridithas       | Alexandria       | Stormphrax       |
|---------------------|------------------|------------------|------------------|
| Entry size          | 10 bytes         | 10 bytes         | 10 bytes         |
| Entries per cluster | 3                | 3                | 3                |
| Cluster alignment   | 32 bytes         | 32 bytes         | 32 bytes         |
| Key stored          | 16-bit           | 16-bit           | 16-bit           |
| Depth field         | u8               | u8               | u8 (offset +7)   |
| Score field         | i16              | i16              | i16              |
| Static eval field   | i16              | i16              | i16              |
| Move field          | 2 bytes          | 2 bytes          | 2 bytes          |
| Packed info         | 1 byte           | 1 byte           | 1 byte           |
| Age bits            | 5                | 5                | 5                |
| PV bit              | 1                | 1                | 1                |
| Bound bits          | 2                | 2                | 2                |
| Replacement         | Depth+age hybrid | Depth+age hybrid | Age-based        |
| Default size        | Not hardcoded    | Not hardcoded    | 64 MiB           |

All three engines converge on nearly identical TT layouts:
- 10-byte entry, 3 per 32-byte cluster
- 16-bit key verification
- 1-byte packed age/PV/bound field (5+1+2 bits)
- Separate score and static eval fields

## Piece Square Tables

NONE of the three engines use traditional piece square tables.
All three use NNUE (neural network) evaluation.

This means there are no PST values to extract from these engines.
CHPawn-FrozenKing's PST source (Michniewski Simplified Evaluation Function)
remains the correct reference for a non-NNUE engine as decided in DD08.

================================================================================
RELEVANCE TO CHPawn-FrozenKing v1.0
================================================================================

## DD01 — MVV-LVA Move Ordering
Decision in DECISIONS.md: Sort captures by (victim_value - attacker_value).

Research finding: Modern engines don't use classic MVV-LVA. They use
victim value + learned capture history. However, capture history requires
search infrastructure (history tables, bonus/malus updates) that adds
significant complexity.

Recommendation for v1.0: Keep classic MVV-LVA as decided. It is the
correct choice for a first submission. Capture history is a v1.1+ upgrade.

If using MVV-only (no attacker component), Viridithas's table provides
a clean reference:
```
MVV_SCORE = [Pawn=0, Knight=2400, Bishop=2400, Rook=4800, Queen=9600, King=0]
```
This is simply the base piece values multiplied by a scaling factor.

## DD04 — Transposition Table
Decision in DECISIONS.md: 64MB, always-replace, entry contains
Zobrist hash, depth, score, flag, best move.

Research finding: All three engines use virtually identical TT layouts.
The industry standard is:
- 10-byte entries, 3 per 32-byte cluster
- 16-bit key (NOT full 64-bit hash — saves 6 bytes per entry)
- Packed age/PV/bound byte
- Separate score and static eval fields

Stormphrax uses 64MB default — matches DD04.
All three use depth+age hybrid replacement, not pure always-replace.

Recommendation for v1.0: Keep always-replace as decided. It is simpler
and correct. The 10-byte entry / 3-per-cluster layout is worth adopting
as it is the universal standard. Store 16-bit key, not full 64-bit.

## DD08 — Piece Square Tables
Decision in DECISIONS.md: Michniewski Simplified Evaluation Function.

Research finding: All three engines use NNUE. No PST values to extract.
This confirms that Michniewski is the correct reference for a non-NNUE
engine — there is no modern handcrafted alternative to compare against.

================================================================================
END OF RESEARCH
================================================================================
