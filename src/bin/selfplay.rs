use shakmaty::{Chess, Color, Move, Position};
use shakmaty::uci::Uci;
use chpawn_frozen_king::movepick::MovePicker;
use chpawn_frozen_king::search::{iterative_deepening, zobrist_key};
use chpawn_frozen_king::tablebase::TablebaseProber;
use chpawn_frozen_king::time::TimeManager;
use chpawn_frozen_king::tt::TranspositionTable;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

fn main() {
    let tb = TablebaseProber::new("syzygy");
    let num_games = 10;
    let movetime_ms = 100; // 100ms per move for self-play

    println!("==========================================================================");
    println!("CHPawn-FrozenKing — Self-Play ({} games, {}ms/move)", num_games, movetime_ms);
    println!("==========================================================================");
    println!();

    let mut completed = 0;
    let mut white_wins = 0;
    let mut black_wins = 0;
    let mut draws = 0;
    let mut total_moves = 0;
    let mut all_legal = true;

    for game in 1..=num_games {
        let mut pos = Chess::default();
        let mut tt = TranspositionTable::new(32);
        let mut picker = MovePicker::new();
        let stop_flag = Arc::new(AtomicBool::new(false));
        let mut move_count = 0;
        let mut game_moves: Vec<String> = Vec::new();
        let mut pos_history: Vec<u64> = vec![zobrist_key(&pos)];

        let result = loop {
            // Check for game-ending conditions
            let moves = pos.legal_moves();
            if moves.is_empty() {
                if pos.is_check() {
                    // Checkmate
                    if pos.turn() == Color::White {
                        break "0-1"; // Black wins
                    } else {
                        break "1-0"; // White wins
                    }
                } else {
                    break "1/2-1/2"; // Stalemate
                }
            }

            // Draw by 50-move rule
            if pos.halfmoves() >= 100 {
                break "1/2-1/2";
            }

            // 3-fold repetition detection
            let hash = zobrist_key(&pos);
            let rep_count = pos_history.iter().filter(|&&h| h == hash).count();
            if rep_count >= 2 {
                break "1/2-1/2"; // 3-fold repetition
            }

            // Draw by insufficient material (just kings)
            if pos.board().occupied().count() <= 2 {
                break "1/2-1/2";
            }

            // Max moves to prevent infinite games
            if move_count >= 300 {
                break "1/2-1/2";
            }

            let is_white = pos.turn() == Color::White;
            let tm = TimeManager::new(0, 0, 0, Some(movetime_ms), is_white, stop_flag.clone());
            let tb_ref = if tb.is_available() { Some(&tb) } else { None };

            let mut noop = |_: u8, _: i32, _: u64, _: u64, _: &Move| {};
            let (_, best_move) = iterative_deepening(
                &pos, 64, &tm, &mut tt, &mut picker, tb_ref, &pos_history, &mut noop,
            );

            if let Some(ref mv) = best_move {
                // Verify the move is legal
                let legal_moves = pos.legal_moves();
                let is_legal = legal_moves.iter().any(|lm| {
                    Uci::from_standard(lm).to_string() == Uci::from_standard(mv).to_string()
                });

                if !is_legal {
                    eprintln!("ILLEGAL MOVE in game {}: {}", game, Uci::from_standard(mv));
                    all_legal = false;
                    break "ILLEGAL";
                }

                game_moves.push(Uci::from_standard(mv).to_string());
                pos.play_unchecked(mv);
                pos_history.push(zobrist_key(&pos));
                move_count += 1;
            } else {
                // Engine couldn't find a move — shouldn't happen
                eprintln!("NO MOVE in game {} at move {}", game, move_count + 1);
                break "ERROR";
            }
        };

        total_moves += move_count;

        match result {
            "1-0" => { white_wins += 1; completed += 1; }
            "0-1" => { black_wins += 1; completed += 1; }
            "1/2-1/2" => { draws += 1; completed += 1; }
            _ => { /* error case */ }
        }

        println!(
            "Game {:2}: {} ({} moves) — {}",
            game,
            result,
            move_count,
            if game_moves.len() >= 6 {
                format!("{}...", game_moves[..6].join(" "))
            } else {
                game_moves.join(" ")
            }
        );
    }

    println!();
    println!("==========================================================================");
    println!("RESULTS");
    println!("==========================================================================");
    println!("Games completed: {}/{}", completed, num_games);
    println!("White wins: {}, Black wins: {}, Draws: {}", white_wins, black_wins, draws);
    println!("Total moves: {}, Avg moves/game: {}", total_moves, total_moves / num_games.max(1));
    println!("All moves legal: {}", if all_legal { "YES" } else { "NO" });
    println!();

    if completed == num_games as u32 && all_legal {
        println!("GATE 2 PASSED: All {} games completed, all moves legal", num_games);
    } else {
        println!("GATE 2 FAILED");
        if !all_legal {
            println!("  Reason: Illegal move detected");
        }
        if completed < num_games as u32 {
            println!("  Reason: {} games did not complete", num_games as u32 - completed);
        }
    }
}
