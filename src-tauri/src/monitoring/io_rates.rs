//! Per-process disk read and write speed from the cumulative I/O counters.
//!
//! Each process's counters are compared with the previous capture of the
//! same process (pid plus start time), and the growth is divided by the wall
//! time between the two captures. A speed exists only when both readings are
//! real, so a first reading, an unreadable process or a counter that went
//! backwards shows as unknown, never as an idle 0 B/s.

use std::collections::HashMap;
use std::time::{Duration, Instant};

/// A process: pid plus start time, so a reused pid starts a new baseline.
pub(crate) type Identity = (u32, u64);

/// Bytes a process has read and written since it started.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct IoTotals {
    pub read: u64,
    pub written: u64,
}

impl IoTotals {
    /// sysinfo leaves both totals at 0 when it cannot read the counters
    /// (access denied on Windows, another user's /proc/<pid>/io on Linux).
    /// A process that has truly read and written nothing reads as unknown
    /// too, which is the safe side of that ambiguity.
    pub fn readable(read: u64, written: u64) -> Option<Self> {
        (read > 0 || written > 0).then_some(Self { read, written })
    }
}

/// Bytes per second over the last capture interval.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct IoRate {
    pub read: u64,
    pub written: u64,
}

#[derive(Debug, Default)]
pub(crate) struct IoRates {
    baseline: HashMap<Identity, IoTotals>,
    at: Option<Instant>,
}

impl IoRates {
    /// Speeds for this capture, then this capture becomes the baseline.
    ///
    /// Processes that left are dropped from the baseline here. A window
    /// longer than `max_window` (paused updates, a stalled capture) is not
    /// reported as a current speed; it only resets the baseline.
    pub fn advance(
        &mut self,
        now: Instant,
        max_window: Duration,
        readings: impl IntoIterator<Item = (Identity, Option<IoTotals>)>,
    ) -> HashMap<Identity, IoRate> {
        let window = self
            .at
            .map(|at| now.saturating_duration_since(at))
            .filter(|w| !w.is_zero() && *w <= max_window);
        let mut rates = HashMap::new();
        let mut next = HashMap::new();
        for (identity, totals) in readings {
            let Some(totals) = totals else { continue };
            if let (Some(window), Some(before)) = (window, self.baseline.get(&identity)) {
                if let Some(rate) = rate(*before, totals, window) {
                    rates.insert(identity, rate);
                }
            }
            next.insert(identity, totals);
        }
        self.baseline = next;
        self.at = Some(now);
        rates
    }
}

fn rate(before: IoTotals, now: IoTotals, window: Duration) -> Option<IoRate> {
    let read = now.read.checked_sub(before.read)?;
    let written = now.written.checked_sub(before.written)?;
    let seconds = window.as_secs_f64();
    let per_second = |bytes: u64| (bytes as f64 / seconds).round() as u64;
    Some(IoRate {
        read: per_second(read),
        written: per_second(written),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const MAX: Duration = Duration::from_millis(7500);

    fn totals(read: u64, written: u64) -> Option<IoTotals> {
        IoTotals::readable(read, written)
    }

    #[test]
    fn speed_is_bytes_per_second_over_the_real_interval() {
        let mut io = IoRates::default();
        let t0 = Instant::now();
        assert!(io
            .advance(t0, MAX, [((7, 1), totals(1_000, 500))])
            .is_empty());
        // 3 MiB read and 1.5 MiB written over 3 s: 1 MiB/s and 512 KiB/s.
        let rates = io.advance(
            t0 + Duration::from_secs(3),
            MAX,
            [((7, 1), totals(1_000 + 3 * 1_048_576, 500 + 1_572_864))],
        );
        assert_eq!(
            rates[&(7, 1)],
            IoRate {
                read: 1_048_576,
                written: 524_288
            }
        );
    }

    #[test]
    fn an_idle_process_reads_zero_and_an_unread_one_has_no_speed() {
        let mut io = IoRates::default();
        let t0 = Instant::now();
        io.advance(t0, MAX, [((1, 1), totals(10, 10)), ((2, 1), totals(0, 0))]);
        let rates = io.advance(
            t0 + Duration::from_secs(3),
            MAX,
            [((1, 1), totals(10, 10)), ((2, 1), totals(0, 0))],
        );
        assert_eq!(
            rates[&(1, 1)],
            IoRate {
                read: 0,
                written: 0
            }
        );
        assert!(!rates.contains_key(&(2, 1)));
    }

    #[test]
    fn a_reused_pid_starts_a_new_baseline() {
        let mut io = IoRates::default();
        let t0 = Instant::now();
        io.advance(t0, MAX, [((9, 100), totals(5_000_000, 0))]);
        // pid 9 is a different process now; its lifetime total is not a speed.
        let rates = io.advance(
            t0 + Duration::from_secs(3),
            MAX,
            [((9, 200), totals(9_000_000, 0))],
        );
        assert!(rates.is_empty());
        let rates = io.advance(
            t0 + Duration::from_secs(6),
            MAX,
            [((9, 200), totals(9_000_300, 0))],
        );
        assert_eq!(rates[&(9, 200)].read, 100);
    }

    #[test]
    fn exited_processes_leave_the_baseline() {
        let mut io = IoRates::default();
        let t0 = Instant::now();
        for tick in 0..50u32 {
            let live = (0..10).map(|i| ((tick * 10 + i, 1), totals(1, 1)));
            io.advance(t0 + Duration::from_secs(tick as u64), MAX, live);
        }
        assert_eq!(io.baseline.len(), 10);
    }

    #[test]
    fn counter_gaps_and_long_windows_are_not_reported_as_speed() {
        let mut io = IoRates::default();
        let t0 = Instant::now();
        io.advance(t0, MAX, [((1, 1), totals(500, 500))]);
        // Went backwards: no speed this time, new baseline.
        let rates = io.advance(
            t0 + Duration::from_secs(3),
            MAX,
            [((1, 1), totals(400, 600))],
        );
        assert!(rates.is_empty());
        // Unreadable for one capture: unknown, and the process is not idle.
        let rates = io.advance(t0 + Duration::from_secs(6), MAX, [((1, 1), None)]);
        assert!(rates.is_empty());
        let rates = io.advance(
            t0 + Duration::from_secs(9),
            MAX,
            [((1, 1), totals(700, 700))],
        );
        assert!(rates.is_empty(), "the gap left no baseline");
        // Updates paused for a minute: that average is not a current speed.
        let rates = io.advance(
            t0 + Duration::from_secs(69),
            MAX,
            [((1, 1), totals(9_000, 700))],
        );
        assert!(rates.is_empty());
        let rates = io.advance(
            t0 + Duration::from_secs(72),
            MAX,
            [((1, 1), totals(9_300, 700))],
        );
        assert_eq!(rates[&(1, 1)].read, 100);
    }

    #[test]
    fn two_captures_at_the_same_instant_carry_no_speed() {
        let mut io = IoRates::default();
        let t0 = Instant::now();
        io.advance(t0, MAX, [((1, 1), totals(1, 1))]);
        assert!(io.advance(t0, MAX, [((1, 1), totals(2, 2))]).is_empty());
    }
}
