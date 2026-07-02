//! Lightweight wall-clock phase timing for hot paths (activation, retrieval).
//!
//! Emits ONE structured `tracing` event per timed operation under the
//! `codescout::perf` target, e.g.:
//! `INFO codescout::perf: op="activate_project" phases=[("agent_activate", 412), ...] total_ms=812`
//!
//! Permanent instrumentation: the numbers that justified the activation/LSP
//! perf fixes are also the regression alarm that protects them.

use std::time::Instant;

pub struct PhaseTimer {
    op: &'static str,
    t0: Instant,
    last: Instant,
    phases: Vec<(&'static str, u128)>,
}

impl PhaseTimer {
    pub fn start(op: &'static str) -> Self {
        let now = Instant::now();
        Self {
            op,
            t0: now,
            last: now,
            phases: Vec::new(),
        }
    }

    /// Record the time since the previous lap (or start) under `name`.
    pub fn lap(&mut self, name: &'static str) {
        let now = Instant::now();
        self.phases
            .push((name, now.duration_since(self.last).as_millis()));
        self.last = now;
    }

    /// Emit the single summary event. Consumes the timer.
    pub fn finish(self) {
        tracing::info!(
            target: "codescout::perf",
            op = self.op,
            phases = ?self.phases,
            total_ms = self.t0.elapsed().as_millis() as u64,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lap_records_each_phase_in_order() {
        let mut t = PhaseTimer::start("test_op");
        t.lap("a");
        std::thread::sleep(std::time::Duration::from_millis(5));
        t.lap("b");
        assert_eq!(t.phases.len(), 2);
        assert_eq!(t.phases[0].0, "a");
        assert_eq!(t.phases[1].0, "b");
        assert!(t.phases[1].1 >= 5, "second lap must include the sleep");
        t.finish(); // must not panic
    }
}
