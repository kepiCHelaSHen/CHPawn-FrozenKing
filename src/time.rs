/// Time Management — CHPawn-FrozenKing v1.0
/// Per DECISIONS.md DD03 (Option A) and frozen/spec.md:
///   budget = remaining_time / 30
///   If movetime provided: budget = movetime
///   No dynamic adjustment (that's DD03-B, version 1.1).

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Instant;

pub struct TimeManager {
    start: Instant,
    budget_ms: u64,
    stop_flag: Arc<AtomicBool>,
}

impl TimeManager {
    /// Create a new TimeManager.
    /// - wtime/btime: remaining time in ms for white/black
    /// - movestogo: moves until next time control (0 = sudden death)
    /// - movetime: if Some, use this as exact budget (overrides wtime/btime)
    /// - is_white: true if engine is playing white
    pub fn new(
        wtime: u64,
        btime: u64,
        movestogo: u64,
        movetime: Option<u64>,
        is_white: bool,
        stop_flag: Arc<AtomicBool>,
    ) -> Self {
        let budget_ms = if let Some(mt) = movetime {
            mt
        } else {
            let remaining = if is_white { wtime } else { btime };
            let divisor = if movestogo > 0 { movestogo } else { 30 };
            remaining / divisor
        };

        TimeManager {
            start: Instant::now(),
            budget_ms,
            stop_flag,
        }
    }

    /// Create a TimeManager for fixed-depth search (infinite time).
    pub fn infinite(stop_flag: Arc<AtomicBool>) -> Self {
        TimeManager {
            start: Instant::now(),
            budget_ms: u64::MAX,
            stop_flag,
        }
    }

    pub fn elapsed_ms(&self) -> u64 {
        self.start.elapsed().as_millis() as u64
    }

    pub fn should_stop(&self) -> bool {
        self.stop_flag.load(Ordering::Relaxed) || self.elapsed_ms() >= self.budget_ms
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
        let tm = TimeManager::new(60000, 60000, 0, None, true, make_stop_flag());
        assert!(!tm.should_stop(), "Should not stop immediately after creation");
    }

    #[test]
    fn should_stop_after_budget() {
        // Budget = 1ms
        let tm = TimeManager::new(30, 30, 0, None, true, make_stop_flag());
        // Spin until time expires
        while !tm.should_stop() {}
        assert!(tm.should_stop());
    }

    #[test]
    fn movetime_overrides_wtime_btime() {
        let tm = TimeManager::new(60000, 60000, 0, Some(500), true, make_stop_flag());
        assert_eq!(tm.budget_ms, 500);
    }

    #[test]
    fn white_uses_wtime() {
        let tm = TimeManager::new(30000, 60000, 0, None, true, make_stop_flag());
        // Budget = 30000 / 30 = 1000ms
        assert_eq!(tm.budget_ms, 1000);
    }

    #[test]
    fn black_uses_btime() {
        let tm = TimeManager::new(30000, 60000, 0, None, false, make_stop_flag());
        // Budget = 60000 / 30 = 2000ms
        assert_eq!(tm.budget_ms, 2000);
    }

    #[test]
    fn division_is_by_30() {
        let tm = TimeManager::new(9000, 9000, 0, None, true, make_stop_flag());
        // 9000 / 30 = 300
        assert_eq!(tm.budget_ms, 300);
    }

    #[test]
    fn movestogo_used_when_nonzero() {
        let tm = TimeManager::new(60000, 60000, 40, None, true, make_stop_flag());
        // 60000 / 40 = 1500
        assert_eq!(tm.budget_ms, 1500);
    }

    #[test]
    fn stop_flag_causes_immediate_stop() {
        let flag = make_stop_flag();
        let tm = TimeManager::new(60000, 60000, 0, None, true, flag.clone());
        assert!(!tm.should_stop());
        flag.store(true, Ordering::Relaxed);
        assert!(tm.should_stop());
    }
}
