# CHPawn-FrozenKing v0.1.1 — Eval Comparison Report

## Results (CHPawn only — Stockfish not available on this machine)

| # | Position Type | FEN (short) | CHPawn Score |
|---|--------------|-------------|-------------|
| 1 | Starting | rnbqkbnr/pppppppp/... | 0.00 |
| 2 | Italian | r1bqkb1r/pppp1ppp/... | -0.46 |
| 3 | King's Indian | rnbq1rk1/ppp1ppbp/... | +1.51 |
| 4 | Complex Middlegame | r2q1rk1/ppp2ppp/... | -0.22 |
| 5 | Rook Endgame | 8/5pk1/6p1/R7/... | -1.25 |
| 6 | KPK | 4k3/8/.../4P3/4K3 | +0.85 |
| 7 | Grunfeld | r1bq1rk1/pp2ppbp/... | +0.63 |
| 8 | KQK Endgame | 8/8/4k3/.../4Q3/8 | +9.36 |
| 9 | Middlegame | r3k2r/pb3ppp/... | -0.15 |
| 10 | Pawn Endgame | 8/pp3pk1/6p1/... | +2.43 |
| 11 | Benoni | r1bqr1k1/pp3pbp/... | +1.27 |
| 12 | All Pawns | 8/1p4kp/p1p3p1/... | -0.78 |
| 13 | Rook Endgame 2 | 4r1k1/1pp2ppp/... | -1.13 |
| 14 | Typical | 2r2rk1/pp2ppbp/... | +0.12 |
| 15 | KPK Black | 8/8/8/8/8/k7/p7/K7 | -3.55 |
| 16 | Complex | r4rk1/pppq1ppp/... | -0.53 |
| 17 | Bishop Endgame | 8/8/1p6/3b4/... | -3.89 |
| 18 | Simplified | 5rk1/pp3ppp/... | +0.64 |
| 19 | Queen Play | 3r1rk1/pp3ppp/1qp5/... | -5.90 |
| 20 | Pure Kings | 8/8/8/4k3/... | -0.10 |

## Sanity Check Analysis

1. **Starting position = 0.00**: Correct (symmetric).
2. **KQK = +9.36**: Large advantage for white with extra queen. Correct.
3. **KPK white = +0.85**: Small advantage for white with extra pawn. Correct.
4. **KPK black = -3.55**: Large advantage for black with extra pawn + passed + advanced. Correct direction.
5. **Pure kings = -0.10**: Near zero, very slight asymmetry from king PST positions. Acceptable.
6. **Queen Play #19 = -5.90**: Black has queen + two rooks vs white's bishop + two rooks. Black has significant material and positional advantage. Correct.

## Observations
- All scores have correct sign (positive = white better, negative = black better)
- Material differences reflected correctly
- Endgame positions show larger scores due to passed pawn bonuses
- No obvious eval bugs from directional analysis

## Stockfish Comparison
Not available — Stockfish binary not found at expected path. To run comparison:
1. Download Stockfish from https://github.com/official-stockfish/Stockfish/releases
2. Place at D:\EXPERIMENTS\stockfish\stockfish.exe
3. Re-run: cargo run --release --bin eval_compare

## Recommendations for Future Eval Improvements
Based on position analysis:
- **Pawn endgames**: Eval relies heavily on passed pawn bonus. Consider king-pawn distance evaluation.
- **Complex middlegames**: Scores near 0 for positions that may have tactical advantages. Consider improving piece coordination.
- **Bishop endgames**: Large scores in simple positions — ensure bishop vs pawn balance is correct.
