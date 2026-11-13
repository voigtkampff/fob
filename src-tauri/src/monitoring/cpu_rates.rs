use std::{
    collections::HashMap,
    time::{Duration, Instant},
};

type Identity = (u32, u64);
#[derive(Clone, Copy, Debug)]
pub(super) struct CpuCounter {
    pub created: u64,
    pub ticks: u64,
}
#[derive(Debug, Default)]
pub(super) struct CpuRates {
    previous: Option<(Instant, u64)>,
    processes: HashMap<Identity, CpuCounter>,
}
impl CpuRates {
    pub fn advance(
        &mut self,
        now: Instant,
        max_window: Duration,
        system: Option<u64>,
        readings: impl IntoIterator<Item = (Identity, Option<CpuCounter>)>,
    ) -> HashMap<Identity, f32> {
        let elapsed = self.previous.and_then(|(at, total)| {
            let gap = now.checked_duration_since(at)?;
            (!gap.is_zero() && gap <= max_window)
                .then(|| system?.checked_sub(total))
                .flatten()
                .filter(|d| *d > 0)
        });
        let mut next = HashMap::new();
        let mut rates = HashMap::new();
        if system.is_some() {
            for (id, counter) in readings {
                let Some(counter) = counter else { continue };
                if let (Some(total), Some(old)) = (elapsed, self.processes.get(&id)) {
                    if counter.created == old.created {
                        if let Some(delta) = counter.ticks.checked_sub(old.ticks) {
                            rates.insert(
                                id,
                                (100.0 * delta as f64 / total as f64).clamp(0.0, 100.0) as f32,
                            );
                        }
                    }
                }
                next.insert(id, counter);
            }
        }
        self.processes = next;
        self.previous = system.map(|total| (now, total));
        rates
    }
}

#[cfg(target_os = "windows")]
mod native {
    use super::CpuCounter;
    use windows::Win32::{
        Foundation::{CloseHandle, FILETIME},
        System::Threading::{
            GetProcessTimes, GetSystemTimes, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION,
        },
    };
    fn ticks(time: FILETIME) -> u64 {
        ((time.dwHighDateTime as u64) << 32) | time.dwLowDateTime as u64
    }
    pub fn system() -> Option<u64> {
        let (mut idle, mut kernel, mut user) = (
            FILETIME::default(),
            FILETIME::default(),
            FILETIME::default(),
        );
        unsafe { GetSystemTimes(Some(&mut idle), Some(&mut kernel), Some(&mut user)) }.ok()?;
        ticks(kernel).checked_add(ticks(user))
    }
    pub fn process(pid: u32) -> Option<CpuCounter> {
        let handle = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) }.ok()?;
        let (mut created, mut exited, mut kernel, mut user) = (
            FILETIME::default(),
            FILETIME::default(),
            FILETIME::default(),
            FILETIME::default(),
        );
        let result =
            unsafe { GetProcessTimes(handle, &mut created, &mut exited, &mut kernel, &mut user) };
        let _ = unsafe { CloseHandle(handle) };
        result.ok()?;
        // A process may exit between sysinfo enumeration and this read.
        if ticks(exited) != 0 {
            return None;
        }
        Some(CpuCounter {
            created: ticks(created),
            ticks: ticks(kernel).checked_add(ticks(user))?,
        })
    }
}
#[cfg(target_os = "windows")]
pub(super) use native::{process, system};

#[cfg(test)]
mod tests {
    use super::*;
    const WINDOW: Duration = Duration::from_millis(7500);
    fn counter(created: u64, ticks: u64) -> Option<CpuCounter> {
        Some(CpuCounter { created, ticks })
    }
    #[test]
    fn missing_is_not_idle_and_two_good_reads_recover() {
        let mut cpu = CpuRates::default();
        let at = Instant::now();
        assert!(cpu
            .advance(at, WINDOW, Some(1000), [((7, 10), counter(1, 100))])
            .is_empty());
        let rate = cpu.advance(
            at + Duration::from_secs(3),
            WINDOW,
            Some(2000),
            [((7, 10), counter(1, 100))],
        );
        assert_eq!(rate[&(7, 10)], 0.0);
        assert!(cpu
            .advance(
                at + Duration::from_secs(6),
                WINDOW,
                Some(3000),
                [((7, 10), None)]
            )
            .is_empty());
        assert!(cpu
            .advance(
                at + Duration::from_secs(9),
                WINDOW,
                Some(4000),
                [((7, 10), counter(1, 200))]
            )
            .is_empty());
        assert_eq!(
            cpu.advance(
                at + Duration::from_secs(12),
                WINDOW,
                Some(5000),
                [((7, 10), counter(1, 250))]
            )[&(7, 10)],
            5.0
        );
    }
    #[test]
    fn no_rate_across_reuse_exit_gap_system_failure_or_counter_reset() {
        let at = Instant::now();
        for (id, created, ticks, system, gap) in [
            ((7, 10), 2, 200, Some(2000), 3),
            ((7, 11), 1, 200, Some(2000), 3),
            ((7, 10), 1, 200, Some(2000), 30),
            ((7, 10), 1, 50, Some(2000), 3),
            ((7, 10), 1, 200, None, 3),
            ((7, 10), 1, 200, Some(1000), 3),
            ((7, 10), 1, 200, Some(500), 3),
        ] {
            let mut cpu = CpuRates::default();
            cpu.advance(at, WINDOW, Some(1000), [((7, 10), counter(1, 100))]);
            assert!(cpu
                .advance(
                    at + Duration::from_secs(gap),
                    WINDOW,
                    system,
                    [(id, counter(created, ticks))]
                )
                .is_empty());
        }
        let mut cpu = CpuRates::default();
        cpu.advance(at, WINDOW, Some(1000), [((7, 10), counter(1, 100))]);
        cpu.advance(at + Duration::from_secs(3), WINDOW, Some(2000), []);
        assert!(cpu
            .advance(
                at + Duration::from_secs(6),
                WINDOW,
                Some(3000),
                [((7, 10), counter(1, 200))]
            )
            .is_empty());
    }
    #[cfg(target_os = "windows")]
    #[test]
    fn native_reader_checks_real_success_and_failure() {
        assert!(system().is_some());
        assert!(process(std::process::id()).is_some());
        assert!(process(u32::MAX).is_none());
    }
}
