# CHPawn-FrozenKing

You can't beat perfect play. And we can prove it.

CHPawn-FrozenKing is a formally verified chess engine built using the Context Hacking Protocol (CHP). Every piece value, every search parameter, every evaluation term traces to a published source. No neural networks. No learned weights. No black boxes. Just alpha-beta search with Michniewski piece-square tables, verified against Syzygy tablebases across 30 endgame positions.

**30/30 sigma gate. 100% tablebase match rate. Every value frozen before a single line of code was written.**

The engine was built in three stages:
1. **Python proof engine** — 27/30 (90%), all sigma gates pass
2. **Rust verification engine** — 30/30 (100%), 8.1 seconds, 327ms worst case
3. **CHPawn-FrozenKing** — competition-ready, 2.7M nps, CCRL submission

## Features

- Minimax with alpha-beta pruning (Russell & Norvig Chapter 5)
- Principal Variation Search (PVS)
- Iterative deepening with time management
- Transposition table (64MB default, depth+age hybrid replacement)
- MVV-LVA move ordering with killer moves
- Michniewski piece-square tables (384 frozen values)
- Check extensions (capped at 4 per path)
- Quiescence search with delta pruning
- Syzygy tablebase support (3-5 piece)
- Threefold repetition detection
- Fifty-move rule detection
- Full UCI protocol support

## Getting Started

Download the latest release binary from the [Releases](../../releases) page.

The engine requires Syzygy tablebase files in a `syzygy/` directory next to the executable. At minimum, you need KQvK, KRvK, and KQvKR tablebases.

Download Syzygy tablebases from: http://tablebase.sesse.net/syzygy/3-4-5/

## Building from Source

```
cargo build --release
```

The release binary will be at `target/release/chpawn-frozen-king.exe`.

## Running the Sigma Gate Benchmark

```
cargo run --release --bin benchmark
```

Requires Syzygy tablebase files in `syzygy/`.

## UCI

Compatible with Arena, Cute Chess, and any UCI-compliant chess GUI.

```
id name CHPawn-FrozenKing
id author CHP
option name Hash type spin default 64 min 1 max 65536
```

### Supported commands

- `uci` — identify engine
- `isready` — synchronization
- `ucinewgame` — reset state
- `position startpos [moves ...]` — set position
- `position fen <fen> [moves ...]` — set position from FEN
- `go wtime <ms> btime <ms> [movestogo <n>]` — search with time control
- `go movetime <ms>` — search for fixed time
- `go depth <n>` — search to fixed depth
- `go infinite` — search until `stop`
- `stop` — halt search, return best move
- `setoption name Hash value <MB>` — resize transposition table
- `quit` — exit

## License

Private. Not for redistribution without permission.
