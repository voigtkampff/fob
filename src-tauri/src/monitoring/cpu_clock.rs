//! Effective CPU frequency for the status bar (0.8.13).
//!
//! Windows' performance counters report how fast the cores actually run, not
//! what the BIOS asked for. Two readings multiply into the number Task
//! Manager shows as "Speed":
//!
//! - `\Processor Information(_Total)\% Processor Performance` — current
//!   frequency over the machine's nominal frequency (above 100 under turbo);
//! - the nominal frequency itself, from
//!   `HKLM\HARDWARE\DESCRIPTION\System\CentralProcessor\0` value `~MHz`,
//!   which Windows fills in at boot from the ACPI/CPUID data (3418 on a
//!   3.4 GHz base i7-13700K, close to the marketed 3400).
//!
//! No TDP or spec-sheet number is used: if the registry value or the counter
//! is missing, there is no reading and the UI shows nothing. `~MHz` is also
//! the base used to judge throttling in `overlay.rs`.
//!
//! PDH needs two collections before it reports a rate, so the first sample of
//! a run is `None` and the second one is real.

#[cfg(target_os = "windows")]
use std::sync::OnceLock;

fn effective_mhz(percent: f64, base_mhz: f64) -> Option<f64> {
    if !percent.is_finite()
        || !(0.0..=1000.0).contains(&percent)
        || !base_mhz.is_finite()
        || !(100.0..=10_000.0).contains(&base_mhz)
    {
        return None;
    }
    let mhz = base_mhz * percent / 100.0;
    (mhz > 0.0).then_some(mhz)
}

/// One PDH query for the machine-wide `% Processor Performance`.
#[derive(Debug, Default)]
pub struct CpuClock {
    #[cfg(target_os = "windows")]
    query: Option<native::Query>,
}

impl CpuClock {
    /// Effective MHz, or `None` before the second collection and whenever
    /// PDH or the nominal frequency is unavailable.
    pub fn read(&mut self) -> Option<f64> {
        #[cfg(target_os = "windows")]
        {
            let base = base_mhz()?;
            if self.query.is_none() {
                self.query = native::Query::open();
                // First rate needs a second collection.
                return None;
            }
            let percent = self.query.as_mut().and_then(native::Query::sample);
            if percent.is_none() {
                self.query = None;
            }
            percent.and_then(|p| effective_mhz(p, base))
        }
        #[cfg(not(target_os = "windows"))]
        {
            None
        }
    }
}

/// The nominal CPU frequency Windows reports for this machine, MHz. Read once.
#[cfg(target_os = "windows")]
pub fn base_mhz() -> Option<f64> {
    static BASE: OnceLock<Option<u32>> = OnceLock::new();
    BASE.get_or_init(native::registry_base_mhz).map(f64::from)
}

/// The nominal CPU frequency Windows reports for this machine, MHz. Other
/// platforms have no such value; callers show nothing.
#[cfg(not(target_os = "windows"))]
pub fn base_mhz() -> Option<f64> {
    None
}

#[cfg(target_os = "windows")]
mod native {
    use std::ffi::c_void;
    use windows::core::{w, PCWSTR};
    use windows::Win32::Foundation::ERROR_SUCCESS;
    use windows::Win32::System::Performance::{
        PdhAddEnglishCounterW, PdhCloseQuery, PdhCollectQueryData, PdhGetFormattedCounterValue,
        PdhOpenQueryW, PDH_FMT_COUNTERVALUE, PDH_FMT_DOUBLE, PDH_HCOUNTER, PDH_HQUERY,
    };
    use windows::Win32::System::Registry::{RegGetValueW, HKEY_LOCAL_MACHINE, RRF_RT_REG_DWORD};

    const KEY: PCWSTR = w!(r"HARDWARE\DESCRIPTION\System\CentralProcessor\0");
    const VALUE: PCWSTR = w!("~MHz");
    const PDH_CSTATUS_VALID_DATA: u32 = 0;
    const PDH_CSTATUS_NEW_DATA: u32 = 1;

    /// `~MHz` as a DWORD; None when the value is missing or not a DWORD.
    pub(super) fn registry_base_mhz() -> Option<u32> {
        unsafe {
            let mut value = 0u32;
            let mut size = std::mem::size_of::<u32>() as u32;
            let status = RegGetValueW(
                HKEY_LOCAL_MACHINE,
                KEY,
                VALUE,
                RRF_RT_REG_DWORD,
                None,
                Some(&mut value as *mut u32 as *mut c_void),
                Some(&mut size),
            );
            (status == ERROR_SUCCESS && size as usize == std::mem::size_of::<u32>())
                .then_some(value)
                .filter(|mhz| *mhz > 0)
        }
    }

    /// The PDH query. Access is serialized by the monitor that owns it.
    #[derive(Debug)]
    pub(super) struct Query {
        query: PDH_HQUERY,
        counter: PDH_HCOUNTER,
    }

    impl Query {
        pub(super) fn open() -> Option<Self> {
            unsafe {
                let mut query = PDH_HQUERY::default();
                if PdhOpenQueryW(PCWSTR::null(), 0, &mut query) != 0 {
                    return None;
                }
                let mut counter = PDH_HCOUNTER::default();
                if PdhAddEnglishCounterW(
                    query,
                    w!(r"\Processor Information(_Total)\% Processor Performance"),
                    0,
                    &mut counter,
                ) != 0
                {
                    PdhCloseQuery(query);
                    return None;
                }
                PdhCollectQueryData(query); // first sample; rates need two
                Some(Self { query, counter })
            }
        }

        pub(super) fn sample(&mut self) -> Option<f64> {
            unsafe {
                if PdhCollectQueryData(self.query) != 0 {
                    return None;
                }
                let mut value = PDH_FMT_COUNTERVALUE::default();
                if PdhGetFormattedCounterValue(self.counter, PDH_FMT_DOUBLE, None, &mut value) != 0
                {
                    return None;
                }
                matches!(value.CStatus, PDH_CSTATUS_VALID_DATA | PDH_CSTATUS_NEW_DATA)
                    .then(|| value.Anonymous.doubleValue)
            }
        }
    }

    // PDH handles are plain handles; access is serialized by the monitor
    // that owns them (same as gpu/windows.rs).
    unsafe impl Send for Query {}

    impl Drop for Query {
        fn drop(&mut self) {
            unsafe {
                PdhCloseQuery(self.query);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn effective_mhz_is_nominal_times_the_counter() {
        assert_eq!(effective_mhz(100.0, 3418.0), Some(3418.0));
        assert_eq!(effective_mhz(155.0, 3418.0), Some(5297.9));
        assert_eq!(effective_mhz(60.0, 3418.0), Some(2050.8));
    }

    #[test]
    fn implausible_readings_are_not_frequencies() {
        assert_eq!(effective_mhz(f64::NAN, 3418.0), None);
        assert_eq!(effective_mhz(-1.0, 3418.0), None);
        assert_eq!(effective_mhz(1001.0, 3418.0), None);
        assert_eq!(effective_mhz(0.0, 3418.0), None);
        assert_eq!(effective_mhz(100.0, 0.0), None);
        assert_eq!(effective_mhz(100.0, 99.0), None);
        assert_eq!(effective_mhz(100.0, f64::NAN), None);
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn the_machine_reports_a_nominal_frequency() {
        let base = base_mhz().expect("registry ~MHz");
        assert!((100.0..=10_000.0).contains(&base), "{base} MHz");
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn this_machine_reports_an_effective_frequency() {
        let mut clock = CpuClock::default();
        assert_eq!(clock.read(), None, "first collection is a baseline");
        std::thread::sleep(std::time::Duration::from_millis(800));
        let mhz = clock.read().expect("PDH % Processor Performance");
        assert!((100.0..=10_000.0).contains(&mhz), "{mhz} MHz");
    }
}
