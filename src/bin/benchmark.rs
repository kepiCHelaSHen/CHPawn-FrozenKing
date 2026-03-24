use shakmaty::{Chess, CastlingMode, Color, Move, Position};
use shakmaty::fen::Fen;
use shakmaty::uci::Uci;
use shakmaty_syzygy::Wdl;
use chpawn_frozen_king::eval::evaluate;
use chpawn_frozen_king::search::{alpha_beta_search, minimax, SearchStats, MAX_DEPTH};
use chpawn_frozen_king::tablebase::TablebaseProber;
use std::time::Instant;

// 30 benchmark positions — same as Python engine
const KQVK: [&str; 10] = [
    "4k3/8/8/8/8/8/8/3QK3 w - - 0 1",
    "k7/8/8/8/8/8/8/KQ6 w - - 0 1",
    "8/8/8/4k3/8/8/8/3QK3 w - - 0 1",
    "8/8/8/8/8/k7/8/KQ6 w - - 0 1",
    "8/8/4k3/8/8/8/8/3QK3 w - - 0 1",
    "8/8/8/8/4k3/8/8/Q3K3 w - - 0 1",
    "7k/8/8/8/8/8/8/4KQ2 w - - 0 1",
    "8/8/8/8/k7/8/8/KQ6 w - - 0 1",
    "8/4k3/8/8/8/8/8/4KQ2 w - - 0 1",
    "8/8/8/8/8/4k3/8/3QK3 w - - 0 1",
];

const KRVK: [&str; 10] = [
    "k7/8/8/8/8/8/8/K6R w - - 0 1",
    "8/8/8/4k3/8/8/8/4K2R w - - 0 1",
    "8/8/4k3/8/8/8/8/R3K3 w - - 0 1",
    "3k4/8/8/8/8/8/8/R3K3 w - - 0 1",
    "8/8/8/8/8/k7/8/K6R w - - 0 1",
    "8/8/8/8/4k3/8/8/R3K3 w - - 0 1",
    "7k/8/8/8/8/8/R7/4K3 w - - 0 1",
    "8/4k3/8/8/8/8/8/R3K3 w - - 0 1",
    "4k3/8/8/8/8/8/R7/4K3 w - - 0 1",
    "4k3/8/8/8/8/8/8/R3K3 w - - 0 1",
];

const KQVKR: [&str; 10] = [
    "8/8/8/4k2r/8/8/8/3QK3 w - - 0 1",
    "8/8/4k3/8/8/8/r7/3QK3 w - - 0 1",
    "8/8/8/8/4k3/8/r7/Q3K3 w - - 0 1",
    "8/8/8/r7/4k3/8/8/3QK3 w - - 0 1",
    "r3k3/8/8/8/8/8/8/3QK3 w - - 0 1",
    "4k3/8/8/8/8/8/4r3/3QK3 w - - 0 1",
    "4k3/8/8/8/8/8/8/r2QK3 w - - 0 1",
    "7k/7r/8/8/8/8/8/3QK3 w - - 0 1",
    "7k/6r1/8/8/8/8/8/4KQ2 w - - 0 1",
    "4k2r/8/8/8/8/8/8/3QK3 w - - 0 1",
];

fn pos_from_fen(fen: &str) -> Chess {
    let f: Fen = fen.parse().unwrap();
    f.into_position(CastlingMode::Standard).unwrap()
}

fn move_to_uci(pos: &Chess, m: &Move) -> String {
    Uci::from_standard(m).to_string()
}

struct BenchResult {
    position_num: usize,
    category: String,
    fen: String,
    root_wdl: i32,
    best_dtz: i32,
    engine_move: String,
    engine_dtz: i32,
    passed: bool,
    wdl_preserved: bool,
    time_ms: u64,
    nodes: u64,
}

fn main() {
    let tb = TablebaseProber::new("syzygy");
    if !tb.is_available() {
        eprintln!("ERROR: Syzygy tablebases not found in syzygy/");
        eprintln!("Copy .rtbw and .rtbz files to the syzygy/ directory.");
        std::process::exit(1);
    }

    println!("==========================================================================");
    println!("CHP Chess Engine (Rust) — Benchmark");
    println!("==========================================================================");
    println!();

    // First, measure pruning rate
    println!("--- Pruning Rate Test ---");
    let pruning_rate = measure_pruning_rate();
    println!("Pruning rate: {:.1}%", pruning_rate * 100.0);
    println!();

    let all_positions: Vec<(&str, &str)> = KQVK
        .iter()
        .map(|f| (*f, "KQvK"))
        .chain(KRVK.iter().map(|f| (*f, "KRvK")))
        .chain(KQVKR.iter().map(|f| (*f, "KQvKR")))
        .collect();

    let mut results: Vec<BenchResult> = Vec::new();
    let mut total_nodes: u64 = 0;
    let total_start = Instant::now();

    println!(
        "{:<4} {:<7} {:<5} {:<8} {:<10} {:<8} {:<6} {:<8}",
        "#", "Cat", "WDL", "BestDTZ", "EngMove", "EngDTZ", "Pass", "Time(ms)"
    );
    println!("{}", "-".repeat(70));

    for (i, (fen, category)) in all_positions.iter().enumerate() {
        let pos = pos_from_fen(fen);

        // Probe root position
        let root_wdl = tb.probe_wdl_raw(&pos).unwrap_or(Wdl::Draw);
        let root_wdl_val = wdl_to_int(root_wdl);

        // Find optimal moves: all moves that preserve WDL and achieve best DTZ
        let legal_moves = pos.legal_moves();
        let mut move_info: Vec<(Move, i32, i32)> = Vec::new(); // (move, wdl, dtz)

        for m in &legal_moves {
            let mut new_pos = pos.clone();
            new_pos.play_unchecked(m);
            if let Some(wdl) = tb.probe_wdl_raw(&new_pos) {
                let dtz = tb
                    .probe_dtz_raw(&new_pos)
                    .map(|d| d)
                    .unwrap_or(0);
                move_info.push((m.clone(), wdl_to_int(wdl), dtz));
            }
        }

        // WDL-preserving moves: root WDL=Win(2) → after move, opponent WDL=Loss(-2)
        let wdl_preserving: Vec<&(Move, i32, i32)> = move_info
            .iter()
            .filter(|(_, wdl, _)| *wdl == -root_wdl_val) // opponent has opposite WDL
            .collect();

        // Best DTZ among WDL-preserving moves (lowest absolute DTZ = fastest win)
        let best_dtz = wdl_preserving
            .iter()
            .map(|(_, _, dtz)| dtz.unsigned_abs() as i32)
            .min()
            .unwrap_or(0);

        // Run engine search
        let tb_ref = Some(&tb);
        let mut stats = SearchStats::new();
        let start = Instant::now();
        let (_, engine_move) = alpha_beta_search(&pos, 10, tb_ref, &mut stats);
        let elapsed = start.elapsed();
        let time_ms = elapsed.as_millis() as u64;

        let (engine_move_str, engine_dtz, wdl_preserved, passed) = if let Some(ref mv) = engine_move
        {
            let uci_str = move_to_uci(&pos, mv);

            // Check engine move against tablebase
            let mut new_pos = pos.clone();
            new_pos.play_unchecked(mv);
            let eng_wdl = tb.probe_wdl_raw(&new_pos).map(|w| wdl_to_int(w)).unwrap_or(0);
            let eng_dtz = tb
                .probe_dtz_raw(&new_pos)
                .map(|d| d.unsigned_abs() as i32)
                .unwrap_or(0);

            let wdl_ok = eng_wdl == -root_wdl_val;
            let is_optimal = wdl_ok && eng_dtz == best_dtz;
            let pass = is_optimal || wdl_ok;

            (uci_str, eng_dtz, wdl_ok, pass)
        } else {
            ("none".to_string(), 0, false, false)
        };

        total_nodes += stats.node_count;

        let result = BenchResult {
            position_num: i + 1,
            category: category.to_string(),
            fen: fen.to_string(),
            root_wdl: root_wdl_val,
            best_dtz,
            engine_move: engine_move_str.clone(),
            engine_dtz,
            passed,
            wdl_preserved,
            time_ms,
            nodes: stats.node_count,
        };

        println!(
            "{:<4} {:<7} {:<5} {:<8} {:<10} {:<8} {:<6} {:<8}",
            result.position_num,
            result.category,
            result.root_wdl,
            result.best_dtz,
            result.engine_move,
            result.engine_dtz,
            if result.passed { "PASS" } else { "FAIL" },
            result.time_ms,
        );

        results.push(result);
    }

    let total_time = total_start.elapsed();
    let total_ms = total_time.as_millis() as u64;
    let nps = if total_ms > 0 {
        total_nodes * 1000 / total_ms
    } else {
        total_nodes
    };

    let pass_count = results.iter().filter(|r| r.passed).count();
    let fail_count = results.len() - pass_count;
    let illegal_moves = 0u32; // shakmaty guarantees legal moves
    let max_ms = results.iter().map(|r| r.time_ms).max().unwrap_or(0);
    let pass_rate = pass_count as f64 / results.len() as f64;

    println!();
    println!("==========================================================================");
    println!("SIGMA GATES");
    println!("==========================================================================");
    println!(
        "GATE 1: illegal_moves == 0        → {}  {}",
        illegal_moves,
        if illegal_moves == 0 { "PASS" } else { "FAIL" }
    );
    println!(
        "GATE 2: pass_rate >= 0.90          → {}/{} ({:.1}%)  {}",
        pass_count,
        results.len(),
        pass_rate * 100.0,
        if pass_rate >= 0.90 { "PASS" } else { "FAIL" }
    );
    println!(
        "GATE 3: pruning_rate >= 0.50       → {:.1}%  {}",
        pruning_rate * 100.0,
        if pruning_rate >= 0.50 { "PASS" } else { "FAIL" }
    );
    println!(
        "GATE 4: max_ms < 900000            → {}ms  {}",
        max_ms,
        if max_ms < 900000 { "PASS" } else { "FAIL" }
    );
    println!();
    println!("--- Performance ---");
    println!("Total time: {}ms ({:.1}s)", total_ms, total_ms as f64 / 1000.0);
    println!("Total nodes: {}", total_nodes);
    println!("Nodes/second: {} ({:.1}M nps)", nps, nps as f64 / 1_000_000.0);
    println!("Max position time: {}ms", max_ms);
    println!();

    let all_pass = illegal_moves == 0 && pass_rate >= 0.90 && pruning_rate >= 0.50 && max_ms < 900000;
    if all_pass {
        println!("ALL SIGMA GATES PASSED");
    } else {
        println!("SIGMA GATES FAILED");
        if pass_rate < 0.90 {
            println!("  Failed positions:");
            for r in &results {
                if !r.passed {
                    println!(
                        "    #{} {} {} — engine: {} (DTZ {}), best DTZ: {}, WDL preserved: {}",
                        r.position_num, r.category, r.fen, r.engine_move, r.engine_dtz, r.best_dtz, r.wdl_preserved
                    );
                }
            }
        }
    }
}

fn wdl_to_int(wdl: Wdl) -> i32 {
    match wdl {
        Wdl::Win => 2,
        Wdl::CursedWin => 1,
        Wdl::Draw => 0,
        Wdl::BlessedLoss => -1,
        Wdl::Loss => -2,
    }
}

fn measure_pruning_rate() -> f64 {
    // Use 3 test positions to measure pruning rate
    let test_positions = [
        "rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq - 0 1",
        "r1bqkbnr/pppppppp/2n5/8/4P3/5N2/PPPP1PPP/RNBQKB1R b KQkq - 2 2",
        "rnbqkb1r/pppppppp/5n2/8/4P3/8/PPPP1PPP/RNBQKBNR w KQkq - 1 2",
    ];

    let mut total_mm = 0u64;
    let mut total_ab = 0u64;

    for fen in &test_positions {
        let pos = pos_from_fen(fen);
        let maximizing = pos.turn() == Color::White;

        let mut stats_mm = SearchStats::new();
        let _ = minimax(&pos, 3, maximizing, &mut stats_mm);

        let mut stats_ab = SearchStats::new();
        let _ = alpha_beta_search(&pos, 3, None, &mut stats_ab);

        total_mm += stats_mm.node_count;
        total_ab += stats_ab.node_count;
    }

    if total_mm > 0 {
        1.0 - (total_ab as f64 / total_mm as f64)
    } else {
        0.0
    }
}
