/// Time Management — CHPawn-FrozenKing v0.0.8
/// Per DECISIONS.md DD03-C:
///   base_time = remaining_time / 40 + increment  (sudden death)
///   base_time = remaining_time / (movestogo + 5) + increment  (known movestogo)
///   hard_limit = base_time * 2  (never exceed this)
///   If movetime provided: budget = movetime (no hard limit)

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Instant;

pub struct TimeManager {
    start: Instant,
    budget_ms: u64,
    hard_limit_ms: u64,
    stop_flag: Arc<AtomicBool>,
}

impl TimeManager {
    /// Create a new TimeManager.
    /// - wtime/btime: remaining time in ms for white/black
    /// - winc/binc: increment per move in ms for white/black
    /// - movestogo: moves until next time control (0 = sudden death)
    /// - movetime: if Some, use this as exact budget (overrides wtime/btime)
    /// - is_white: true if engine is playing white
    pub fn new(
        wtime: u64,
        btime: u64,
        winc: u64,
        binc: u64,
        movestogo: u64,
        movetime: Option<u64>,
        is_white: bool,
        stop_flag: Arc<AtomicBool>,
    ) -> Self {
        if let Some(mt) = movetime {
            return TimeManager {
                start: Instant::now(),
                budget_ms: mt,
                hard_limit_ms: mt,
                stop_flag,
            };
        }

        let remaining = if is_white { wtime } else { btime };
        let increment = if is_white { winc } else { binc };
        let base_time = if movestogo > 0 {
            remaining / (movestogo + 5) + increment
        } else {
            remaining / 40 + increment
        };
        let hard_limit = base_time * 2;

        TimeManager {
            start: Instant::now(),
            budget_ms: base_time,
            hard_limit_ms: hard_limit,
            stop_flag,
        }
    }

    /// Create a TimeManager for fixed-depth search (infinite time).
    pub fn infinite(stop_flag: Arc<AtomicBool>) -> Self {
        TimeManager {
            start: Instant::now(),
            budget_ms: u64::MAX,
            hard_limit_ms: u64::MAX,
            stop_flag,
        }
    }

    pub fn elapsed_ms(&self) -> u64 {
        self.start.elapsed().as_millis() as u64
    }

    /// Get base budget in milliseconds.
    pub fn budget_ms(&self) -> u64 {
        self.budget_ms
    }

    /// Soft stop: exceeded base budget. Used between ID iterations.
    pub fn should_stop(&self) -> bool {
        self.stop_flag.load(Ordering::Relaxed) || self.elapsed_ms() >= self.budget_ms
    }

    /// Hard stop: never think longer than this.
    pub fn hard_stop(&self) -> bool {
        self.stop_flag.load(Ordering::Relaxed) || self.elapsed_ms() >= self.hard_limit_ms
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_stop_flag() -> Arc<AtomicBool> {
        Arc::new(AtomicBool::new(false))
    }

    #[test]
    fn should_stop_false_immediately() {
        let tm = TimeManager::new(60000, 60000, 0, 0, 0, None, true, make_stop_flag());
        assert!(!tm.should_stop());
    }

    #[test]
    fn should_stop_after_budget() {
        let tm = TimeManager::new(30, 30, 0, 0, 0, None, true, make_stop_flag());
        while !tm.should_stop() {}
        assert!(tm.should_stop());
    }

    #[test]
    fn movetime_overrides_wtime_btime() {
        let tm = TimeManager::new(60000, 60000, 0, 0, 0, Some(500), true, make_stop_flag());
        assert_eq!(tm.budget_ms, 500);
    }

    #[test]
    fn white_uses_wtime() {
        let tm = TimeManager::new(30000, 60000, 0, 0, 0, None, true, make_stop_flag());
        // Budget = 30000 / 40 = 750ms (v0.0.8)
        assert_eq!(tm.budget_ms, 750);
    }

    #[test]
    fn black_uses_btime() {
        let tm = TimeManager::new(30000, 60000, 0, 0, 0, None, false, make_stop_flag());
        // Budget = 60000 / 40 = 1500ms (v0.0.8)
        assert_eq!(tm.budget_ms, 1500);
    }

    #[test]
    fn sudden_death_divides_by_40() {
        let tm = TimeManager::new(40000, 40000, 0, 0, 0, None, true, make_stop_flag());
        // 40000 / 40 = 1000
        assert_eq!(tm.budget_ms, 1000);
    }

    #[test]
    fn movestogo_divides_by_movestogo_plus_5() {
        let tm = TimeManager::new(60000, 60000, 0, 0, 40, None, true, make_stop_flag());
        // 60000 / (40 + 5) = 1333
        assert_eq!(tm.budget_ms, 1333);
    }

    #[test]
    fn hard_limit_is_2x_budget() {
        let tm = TimeManager::new(60000, 60000, 0, 0, 0, None, true, make_stop_flag());
        // budget = 60000 / 40 = 1500, hard_limit = 1500 * 2 = 3000
        assert_eq!(tm.budget_ms, 1500);
        assert_eq!(tm.hard_limit_ms, 3000);
    }

    #[test]
    fn increment_added_to_budget() {
        let tm = TimeManager::new(40000, 40000, 500, 500, 0, None, true, make_stop_flag());
        // 40000 / 40 + 500 = 1000 + 500 = 1500
        assert_eq!(tm.budget_ms, 1500);
    }

    #[test]
    fn movetime_sets_hard_limit_equal() {
        let tm = TimeManager::new(60000, 60000, 0, 0, 0, Some(500), true, make_stop_flag());
        assert_eq!(tm.budget_ms, 500);
        assert_eq!(tm.hard_limit_ms, 500);
    }

    #[test]
    fn stop_flag_causes_immediate_stop() {
        let flag = make_stop_flag();
        let tm = TimeManager::new(60000, 60000, 0, 0, 0, None, true, flag.clone());
        assert!(!tm.should_stop());
        flag.store(true, Ordering::Relaxed);
        assert!(tm.should_stop());
    }
}
