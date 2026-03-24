use std::io::{self, BufRead, Write};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use shakmaty::{Chess, CastlingMode, Color, Move, Position};
use shakmaty::fen::Fen;
use shakmaty::uci::Uci;
use chpawn_frozen_king::movepick::MovePicker;
use chpawn_frozen_king::search::{iterative_deepening, zobrist_key};
use chpawn_frozen_king::tablebase::TablebaseProber;
use chpawn_frozen_king::time::TimeManager;
use chpawn_frozen_king::tt::TranspositionTable;

const DEFAULT_HASH_MB: usize = 64;
const DEFAULT_DEPTH: u8 = 64;

fn main() {
    let tb = TablebaseProber::new("syzygy");
    let mut position = Chess::default();
    let mut tt = TranspositionTable::new(DEFAULT_HASH_MB);
    let mut picker = MovePicker::new();
    let stop_flag = Arc::new(AtomicBool::new(false));
    let mut pos_history: Vec<u64> = vec![zobrist_key(&Chess::default())];

    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut stdout = stdout.lock();

    for line in stdin.lock().lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => break,
        };
        let line = line.trim().to_string();
        if line.is_empty() {
            continue;
        }

        let tokens: Vec<&str> = line.split_whitespace().collect();
        if tokens.is_empty() {
            continue;
        }

        match tokens[0] {
            "uci" => {
                writeln!(stdout, "id name CHPawn-FrozenKing").unwrap();
                writeln!(stdout, "id author CHP").unwrap();
                writeln!(stdout, "option name Hash type spin default 64 min 1 max 65536").unwrap();
                writeln!(stdout, "uciok").unwrap();
                stdout.flush().unwrap();
            }
            "isready" => {
                writeln!(stdout, "readyok").unwrap();
                stdout.flush().unwrap();
            }
            "ucinewgame" => {
                position = Chess::default();
                tt.clear();
                picker.clear();
                pos_history = vec![zobrist_key(&Chess::default())];
            }
            "setoption" => {
                // setoption name Hash value <n>
                if let Some(val) = parse_setoption(&tokens) {
                    tt.resize(val as usize);
                }
            }
            "position" => {
                let (pos, history) = parse_position_with_history(&tokens[1..]);
                position = pos;
                pos_history = history;
            }
            "go" => {
                stop_flag.store(false, Ordering::Relaxed);
                let go_params = parse_go(&tokens[1..]);
                let tb_ref = if tb.is_available() { Some(&tb) } else { None };

                let is_white = position.turn() == Color::White;
                let tm = if let Some(mt) = go_params.movetime {
                    TimeManager::new(0, 0, 0, Some(mt), is_white, stop_flag.clone())
                } else if go_params.wtime > 0 || go_params.btime > 0 {
                    TimeManager::new(
                        go_params.wtime, go_params.btime,
                        go_params.movestogo, None,
                        is_white, stop_flag.clone(),
                    )
                } else if go_params.depth > 0 {
                    TimeManager::infinite(stop_flag.clone())
                } else {
                    TimeManager::infinite(stop_flag.clone())
                };

                let depth = if go_params.depth > 0 {
                    go_params.depth
                } else {
                    DEFAULT_DEPTH
                };

                let mut info_callback = |d: u8, score: i32, nodes: u64, time_ms: u64, mv: &Move| {
                    let nps = if time_ms > 0 { nodes * 1000 / time_ms } else { nodes };
                    let uci_move = Uci::from_standard(mv);
                    writeln!(
                        stdout,
                        "info depth {} score cp {} nodes {} time {} nps {} pv {}",
                        d, score, nodes, time_ms, nps, uci_move
                    ).unwrap();
                    stdout.flush().unwrap();
                };

                let (_, best_move) = iterative_deepening(
                    &position, depth, &tm, &mut tt, &mut picker,
                    tb_ref, &pos_history, &mut info_callback,
                );

                if let Some(ref mv) = best_move {
                    let uci_move = Uci::from_standard(mv);
                    writeln!(stdout, "bestmove {}", uci_move).unwrap();
                } else {
                    writeln!(stdout, "bestmove 0000").unwrap();
                }
                stdout.flush().unwrap();
            }
            "stop" => {
                stop_flag.store(true, Ordering::Relaxed);
            }
            "quit" => {
                break;
            }
            _ => {}
        }
    }
}

struct GoParams {
    wtime: u64,
    btime: u64,
    movestogo: u64,
    movetime: Option<u64>,
    depth: u8,
}

fn parse_go(tokens: &[&str]) -> GoParams {
    let mut params = GoParams {
        wtime: 0,
        btime: 0,
        movestogo: 0,
        movetime: None,
        depth: 0,
    };

    let mut i = 0;
    while i < tokens.len() {
        match tokens[i] {
            "wtime" if i + 1 < tokens.len() => {
                params.wtime = tokens[i + 1].parse().unwrap_or(0);
                i += 2;
            }
            "btime" if i + 1 < tokens.len() => {
                params.btime = tokens[i + 1].parse().unwrap_or(0);
                i += 2;
            }
            "movestogo" if i + 1 < tokens.len() => {
                params.movestogo = tokens[i + 1].parse().unwrap_or(0);
                i += 2;
            }
            "movetime" if i + 1 < tokens.len() => {
                params.movetime = Some(tokens[i + 1].parse().unwrap_or(1000));
                i += 2;
            }
            "depth" if i + 1 < tokens.len() => {
                params.depth = tokens[i + 1].parse().unwrap_or(0);
                i += 2;
            }
            "infinite" => {
                // No time limit
                i += 1;
            }
            _ => {
                i += 1;
            }
        }
    }

    params
}

fn parse_setoption(tokens: &[&str]) -> Option<u64> {
    // setoption name Hash value <n>
    if tokens.len() >= 5
        && tokens[1] == "name"
        && tokens[2].eq_ignore_ascii_case("Hash")
        && tokens[3] == "value"
    {
        return tokens[4].parse().ok();
    }
    None
}

fn parse_position_with_history(tokens: &[&str]) -> (Chess, Vec<u64>) {
    if tokens.is_empty() {
        let pos = Chess::default();
        let history = vec![zobrist_key(&pos)];
        return (pos, history);
    }

    let (mut pos, moves_start) = if tokens[0] == "startpos" {
        let idx = tokens.iter().position(|&t| t == "moves").unwrap_or(tokens.len());
        (Chess::default(), idx)
    } else if tokens[0] == "fen" {
        let moves_idx = tokens.iter().position(|&t| t == "moves").unwrap_or(tokens.len());
        let fen_str = tokens[1..moves_idx].join(" ");
        let fen: Fen = match fen_str.parse() {
            Ok(f) => f,
            Err(_) => {
                let pos = Chess::default();
                return (pos.clone(), vec![zobrist_key(&pos)]);
            }
        };
        let pos: Chess = match fen.into_position(CastlingMode::Standard) {
            Ok(p) => p,
            Err(_) => {
                let pos = Chess::default();
                return (pos.clone(), vec![zobrist_key(&pos)]);
            }
        };
        (pos, moves_idx)
    } else {
        let pos = Chess::default();
        return (pos.clone(), vec![zobrist_key(&pos)]);
    };

    let mut history = vec![zobrist_key(&pos)];

    if moves_start < tokens.len() && tokens[moves_start] == "moves" {
        for &move_str in &tokens[moves_start + 1..] {
            let uci: Uci = match move_str.parse() {
                Ok(u) => u,
                Err(_) => break,
            };
            let mv: Move = match uci.to_move(&pos) {
                Ok(m) => m,
                Err(_) => break,
            };
            pos.play_unchecked(&mv);
            history.push(zobrist_key(&pos));
        }
    }

    (pos, history)
}
