# FROZEN SPECIFICATION — CHPawn-FrozenKing v1.0
# DO NOT MODIFY THIS FILE.
# Source: Russell & Norvig 4th Ed Chapter 5 + DECISIONS.md

================================================================================
FROZEN VALUES
================================================================================

PIECE VALUES (centipawns):
  PAWN   = 100
  KNIGHT = 300   ← NOT 320. NOT 325. 300.
  BISHOP = 300   ← NOT 325. NOT 315. 300.
  ROOK   = 500
  QUEEN  = 900
  KING   = 20000
  CHECKMATE = 1_000_000

SEARCH:
  MAX_DEPTH      = 6  (base depth, overridden by iterative deepening)
  DELTA          = 200  (quiescence delta pruning margin)
  MAX_EXTENSIONS = 4    (check extension cap per path)

TIME:
  time_per_move = remaining_time / 30

TRANSPOSITION TABLE:
  DEFAULT_MB     = 64
  CLUSTER_SIZE   = 3  (entries per cluster)
  CLUSTER_ALIGN  = 32 (bytes)
  Entry layout: key(u16) + move(u16) + score(i16) + eval(i16) + depth(u8) + flags(u8)
  Entry size: 10 bytes

MOVE ORDERING:
  CAPTURE_BASE    = 10000
  KILLER_SCORE    = 9000
  PROMOTION_SCORE = 5000
  QUIET_SCORE     = 0
  MVV formula: victim_value * 10 - attacker_value + CAPTURE_BASE

UCI:
  id name   = "CHPawn-FrozenKing"
  id author = "CHP"
  Hash option: "option name Hash type spin default 64 min 1 max 65536"

PST SOURCE: Michniewski Simplified Evaluation Function
  See frozen/pst.rs — all 384 values immutable

NO MCTS. NO NEURAL NETWORKS. NO INTERNAL OPENING BOOK. EVER.
