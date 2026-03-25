use shakmaty::{Chess, CastlingMode, Position};
use shakmaty::fen::Fen;

fn perft(pos: &Chess, depth: u8) -> u64 {
    if depth == 0 {
        return 1;
    }
    let moves = pos.legal_moves();
    let mut count = 0u64;
    for m in &moves {
        let mut new_pos = pos.clone();
        new_pos.play_unchecked(m);
        count += perft(&new_pos, depth - 1);
    }
    count
}

fn pos_from_fen(fen: &str) -> Chess {
    let f: Fen = fen.parse().unwrap();
    f.into_position(CastlingMode::Standard).unwrap()
}

struct PerftTest {
    name: &'static str,
    fen: &'static str,
    depth: u8,
    expected: u64,
}

fn main() {
    let tests = vec![
        // Starting position
        PerftTest { name: "Start d1", fen: "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1", depth: 1, expected: 20 },
        PerftTest { name: "Start d2", fen: "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1", depth: 2, expected: 400 },
        PerftTest { name: "Start d3", fen: "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1", depth: 3, expected: 8902 },
        PerftTest { name: "Start d4", fen: "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1", depth: 4, expected: 197281 },
        PerftTest { name: "Start d5", fen: "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1", depth: 5, expected: 4865609 },
        // Kiwipete
        PerftTest { name: "Kiwi d1", fen: "r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1", depth: 1, expected: 48 },
        PerftTest { name: "Kiwi d2", fen: "r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1", depth: 2, expected: 2039 },
        PerftTest { name: "Kiwi d3", fen: "r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1", depth: 3, expected: 97862 },
        PerftTest { name: "Kiwi d4", fen: "r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1", depth: 4, expected: 4085603 },
        // Position 3
        PerftTest { name: "Pos3 d1", fen: "8/2p5/3p4/KP5r/1R3p1k/8/4P1P1/8 w - - 0 1", depth: 1, expected: 14 },
        PerftTest { name: "Pos3 d2", fen: "8/2p5/3p4/KP5r/1R3p1k/8/4P1P1/8 w - - 0 1", depth: 2, expected: 191 },
        PerftTest { name: "Pos3 d3", fen: "8/2p5/3p4/KP5r/1R3p1k/8/4P1P1/8 w - - 0 1", depth: 3, expected: 2812 },
        PerftTest { name: "Pos3 d4", fen: "8/2p5/3p4/KP5r/1R3p1k/8/4P1P1/8 w - - 0 1", depth: 4, expected: 43238 },
        PerftTest { name: "Pos3 d5", fen: "8/2p5/3p4/KP5r/1R3p1k/8/4P1P1/8 w - - 0 1", depth: 5, expected: 674624 },
    ];

    println!("==========================================================================");
    println!("CHPawn-FrozenKing — Perft Verification");
    println!("==========================================================================");
    println!();
    println!("{:<12} {:<12} {:<12} {:<12} {}", "Test", "Depth", "Expected", "Actual", "Result");
    println!("{}", "-".repeat(65));

    let mut all_pass = true;
    for t in &tests {
        let pos = pos_from_fen(t.fen);
        let actual = perft(&pos, t.depth);
        let pass = actual == t.expected;
        if !pass { all_pass = false; }
        println!("{:<12} {:<12} {:<12} {:<12} {}",
            t.name, t.depth, t.expected, actual,
            if pass { "PASS" } else { "FAIL" });
    }

    println!();
    if all_pass {
        println!("PERFT CLEAN — all {} tests pass. Move generation verified correct.", tests.len());
    } else {
        println!("PERFT FAILED — move generation bug detected!");
        std::process::exit(1);
    }
}
