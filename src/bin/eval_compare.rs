use shakmaty::{Chess, CastlingMode};
use shakmaty::fen::Fen;
use chpawn_frozen_king::eval::evaluate;
use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};
use std::path::Path;

const STOCKFISH_PATH: &str = r"D:\EXPERIMENTS\stockfish\stockfish.exe";

struct EvalResult {
    fen: String,
    chpawn_cp: i32,
    stockfish_cp: Option<i32>,
    diff: Option<i32>,
    flagged: bool,
}

fn pos_from_fen(fen: &str) -> Chess {
    let f: Fen = fen.parse().unwrap();
    f.into_position(CastlingMode::Standard).unwrap()
}

fn get_stockfish_eval(fen: &str) -> Option<i32> {
    if !Path::new(STOCKFISH_PATH).exists() {
        return None;
    }

    let mut child = Command::new(STOCKFISH_PATH)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;

    let stdin = child.stdin.as_mut()?;
    writeln!(stdin, "uci").ok()?;
    writeln!(stdin, "isready").ok()?;
    writeln!(stdin, "position fen {}", fen).ok()?;
    writeln!(stdin, "eval").ok()?;
    writeln!(stdin, "quit").ok()?;
    stdin.flush().ok()?;

    let stdout = child.stdout.take()?;
    let reader = BufReader::new(stdout);

    let mut final_eval = None;
    for line in reader.lines().map_while(Result::ok) {
        // Stockfish eval output: "Final evaluation: +0.26 (white side)"
        if line.contains("Final evaluation") {
            if let Some(val_str) = line.split_whitespace().nth(2) {
                if let Ok(val) = val_str.parse::<f64>() {
                    final_eval = Some((val * 100.0) as i32);
                }
            }
        }
    }

    let _ = child.wait();
    final_eval
}

fn main() {
    let positions = vec![
        "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
        "r1bqkb1r/pppp1ppp/2n2n2/4p3/2B1P3/5N2/PPPP1PPP/RNBQK2R w KQkq - 4 4",
        "rnbq1rk1/ppp1ppbp/3p1np1/8/2PPP3/2N2N2/PP2BPPP/R1BQK2R w KQ - 0 7",
        "r2q1rk1/ppp2ppp/2np1n2/2b1p3/2B1P1b1/2NP1N2/PPP2PPP/R1BQR1K1 w - - 0 9",
        "8/5pk1/6p1/R7/5P2/r5K1/8/8 w - - 0 1",
        "4k3/8/8/8/8/8/4P3/4K3 w - - 0 1",
        "r1bq1rk1/pp2ppbp/2np1np1/3p4/3P4/2NBPN2/PPP2PPP/R1BQ1RK1 w - - 0 8",
        "8/8/4k3/8/8/4K3/4Q3/8 w - - 0 1",
        "r3k2r/pb3ppp/1p1qpn2/2pp4/3P4/1P2PN2/PBP1QPPP/R3K2R w KQkq - 0 10",
        "8/pp3pk1/6p1/3P4/2P5/1P3KP1/P7/8 w - - 0 1",
        "r1bqr1k1/pp3pbp/2np1np1/2pP4/4P3/2NB1N2/PP3PPP/R1BQR1K1 w - - 0 10",
        "8/1p4kp/p1p3p1/P1P5/1P6/6KP/8/8 w - - 0 1",
        "4r1k1/1pp2ppp/p1p5/8/8/P1P2PP1/1P4KP/4R3 w - - 0 1",
        "2r2rk1/pp2ppbp/2nq1np1/3p4/3P4/2N1PN2/PP2BPPP/R2Q1RK1 w - - 0 10",
        "8/8/8/8/8/k7/p7/K7 w - - 0 1",
        "r4rk1/pppq1ppp/2np1n2/2b1p3/2B1P1b1/2NP1N2/PPPQ1PPP/R1B2RK1 w - - 6 9",
        "8/8/1p6/3b4/1P6/8/5K2/3k4 w - - 0 1",
        "5rk1/pp3ppp/4b3/3p4/8/2P2N2/PP3PPP/4R1K1 w - - 0 1",
        "3r1rk1/pp3ppp/1qp5/4p3/8/1P2P3/PBP2PPP/3RR1K1 w - - 0 1",
        "8/8/8/4k3/8/4K3/8/8 w - - 0 1",
    ];

    let has_stockfish = Path::new(STOCKFISH_PATH).exists();

    println!("==========================================================================");
    println!("CHPawn-FrozenKing — Eval Comparison");
    println!("==========================================================================");
    if !has_stockfish {
        println!("NOTE: Stockfish not found at {}. Showing CHPawn eval only.", STOCKFISH_PATH);
    }
    println!();

    let mut results: Vec<EvalResult> = Vec::new();

    for fen in &positions {
        let pos = pos_from_fen(fen);
        let chpawn_cp = evaluate(&pos);

        let stockfish_cp = if has_stockfish {
            get_stockfish_eval(fen)
        } else {
            None
        };

        let (diff, flagged) = match stockfish_cp {
            Some(sf) => {
                let d = (chpawn_cp - sf).abs();
                (Some(d as i32), d > 150)
            }
            None => (None, false),
        };

        results.push(EvalResult {
            fen: fen.to_string(),
            chpawn_cp,
            stockfish_cp,
            diff,
            flagged,
        });
    }

    // Print results
    println!("{:<60} {:>10} {:>10} {:>8} {}", "FEN", "CHPawn", "SF", "Diff", "Status");
    println!("{}", "-".repeat(100));
    for r in &results {
        let sf_str = r.stockfish_cp.map(|s| format!("{:+.2}", s as f64 / 100.0)).unwrap_or("N/A".to_string());
        let diff_str = r.diff.map(|d| format!("{}cp", d)).unwrap_or("N/A".to_string());
        let status = if r.flagged { "FLAG" } else { "PASS" };
        let short_fen = if r.fen.len() > 58 { &r.fen[..58] } else { &r.fen };
        println!("{:<60} {:>+10.2} {:>10} {:>8} {}",
            short_fen, r.chpawn_cp as f64 / 100.0, sf_str, diff_str, status);
    }

    // Summary
    let flagged_count = results.iter().filter(|r| r.flagged).count();
    let total = results.len();
    println!();
    println!("Total: {}/{}. Flagged (diff > 150cp): {}", total, total, flagged_count);

    if flagged_count > 0 {
        println!();
        println!("FLAGGED positions (CHPawn vs Stockfish diff > 150cp):");
        for r in &results {
            if r.flagged {
                println!("  {} | CHPawn: {:+.2} | SF: {:+.2} | Diff: {}cp",
                    r.fen,
                    r.chpawn_cp as f64 / 100.0,
                    r.stockfish_cp.unwrap_or(0) as f64 / 100.0,
                    r.diff.unwrap_or(0));
            }
        }
    }
}
