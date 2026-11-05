//! winperf: "on since" for the uptime chip.
//!
//! With Fast Startup (hybrid shutdown, the Windows default) "Shut down" only
//! logs the user off and hibernates the kernel, so the kernel uptime that
//! sysinfo and Task Manager show keeps counting across days the PC was off.
//! What a user means by "on since" is the last power-on after "Shut down":
//! the newest Fast Startup resume, hibernation resume or full boot. Sleep is
//! not a start. In the System log (readable without administrator rights):
//! - Microsoft-Windows-Kernel-Boot 27, "The boot type was 0x0/0x1/0x2":
//!   every start; 0x1 is a Fast Startup resume, 0x2 a resume from hibernation.
//!   (v0.5.0 asked for provider Kernel-Power here, which never logs event 27,
//!   so only the cold-boot event below matched and the chip kept showing the
//!   kernel uptime.)
//! - Microsoft-Windows-Kernel-General 12: the OS started (cold boot).
//! We take the newest of those that isn't before the kernel boot. When
//! nothing is found the chip shows the kernel uptime, labelled as such.

#![cfg_attr(not(target_os = "windows"), allow(dead_code))]

/// Seconds since the Unix epoch for an event's `SystemTime` attribute
/// (`<TimeCreated SystemTime='2026-11-02T01:23:45.1234567Z'/>`, always UTC).
pub fn event_time(xml: &str) -> Option<u64> {
    let start = xml.find("SystemTime=")? + "SystemTime=".len();
    let quote = xml[start..].chars().next()?;
    let rest = &xml[start + 1..];
    let value = &rest[..rest.find(quote)?];
    parse_utc(value)
}

/// "YYYY-MM-DDTHH:MM:SS[.fraction]Z" -> Unix seconds.
pub fn parse_utc(value: &str) -> Option<u64> {
    let value = value.trim().strip_suffix('Z')?;
    let (date, time) = value.split_once('T')?;
    let mut d = date.split('-').map(|p| p.parse::<i64>().ok());
    let (y, m, day) = (d.next()??, d.next()??, d.next()??);
    let time = time.split('.').next()?;
    let mut t = time.split(':').map(|p| p.parse::<i64>().ok());
    let (hh, mm, ss) = (t.next()??, t.next()??, t.next()??);
    if !(1..=12).contains(&m) || !(1..=31).contains(&day) || hh > 23 || mm > 59 || ss > 60 {
        return None;
    }
    // Days from civil (Howard Hinnant).
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe - 719_468;
    u64::try_from(days * 86_400 + hh * 3_600 + mm * 60 + ss).ok()
}

/// What kind of start an event records.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StartKind {
    Cold,
    FastStartup,
    Hibernate,
}
impl StartKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Cold => "cold",
            Self::FastStartup => "fast",
            Self::Hibernate => "hibernate",
        }
    }
}

/// Value of `<Tag ...>value</Tag>` or `<Data Name='name'>value</Data>`.
fn element<'a>(xml: &'a str, open: &str, close: &str) -> Option<&'a str> {
    let start = xml.find(open)?;
    let after = &xml[start + open.len()..];
    let body = &after[after.find('>')? + 1..];
    Some(body[..body.find(close)?].trim())
}

/// Parse one rendered start event: its time and kind.
pub fn start_event(xml: &str) -> Option<(u64, StartKind)> {
    let time = event_time(xml)?;
    let provider = {
        let at = xml.find("Provider Name=")? + "Provider Name=".len();
        let quote = xml[at..].chars().next()?;
        let rest = &xml[at + 1..];
        &rest[..rest.find(quote)?]
    };
    let id: u32 = element(xml, "<EventID", "</EventID>")?.parse().ok()?;
    let kind = match (provider, id) {
        ("Microsoft-Windows-Kernel-Boot", 27) => {
            let boot_type = ["<Data Name='BootType'", "<Data Name=\"BootType\""]
                .iter()
                .find_map(|open| element(xml, open, "</Data>"))
                .and_then(|v| {
                    let v = v.trim_start_matches("0x");
                    u32::from_str_radix(v, 16).ok()
                })?;
            match boot_type {
                1 => StartKind::FastStartup,
                2 => StartKind::Hibernate,
                _ => StartKind::Cold,
            }
        }
        ("Microsoft-Windows-Kernel-General", 12) => StartKind::Cold,
        _ => return None,
    };
    Some((time, kind))
}

/// When the PC was last started: the newest start event that is not older
/// than the kernel boot (a minute of slack for logging) nor in the future;
/// the kernel boot itself when there is none.
pub fn started_at(now: u64, kernel_boot: u64, events: &[u64]) -> u64 {
    events
        .iter()
        .copied()
        .filter(|&t| t + 60 >= kernel_boot && t <= now + 60)
        .max()
        .map_or(kernel_boot, |t| t.min(now).max(kernel_boot))
}

#[cfg(target_os = "windows")]
fn latest_start_event() -> Result<(u64, StartKind), String> {
    use windows::core::w;
    use windows::Win32::System::EventLog::{
        EvtClose, EvtNext, EvtQuery, EvtQueryChannelPath, EvtQueryReverseDirection, EvtRender,
        EvtRenderEventXml, EVT_HANDLE,
    };
    unsafe {
        let query = EvtQuery(
            None,
            w!("System"),
            w!("*[System[(Provider[@Name='Microsoft-Windows-Kernel-Boot'] and EventID=27) or (Provider[@Name='Microsoft-Windows-Kernel-General'] and EventID=12)]]"),
            (EvtQueryChannelPath.0 | EvtQueryReverseDirection.0) as u32,
        )
        .map_err(|e| format!("EvtQuery(System): {e}"))?;
        let mut result =
            Err("no Kernel-Boot 27 or Kernel-General 12 event in the System log".to_string());
        // Newest first; skip anything we can't parse (e.g. an unknown boot type layout).
        'events: for _ in 0..8 {
            let mut events = [0isize; 4];
            let mut returned = 0u32;
            if let Err(e) = EvtNext(query, &mut events, 2_000, 0, &mut returned) {
                if result.is_err()
                    && e.code() != windows::Win32::Foundation::ERROR_NO_MORE_ITEMS.to_hresult()
                {
                    result = Err(format!("EvtNext: {e}"));
                }
                break;
            }
            for &raw in &events[..returned as usize] {
                let event = EVT_HANDLE(raw);
                let mut buffer = vec![0u16; 8192];
                let (mut used, mut count) = (0u32, 0u32);
                let rendered = EvtRender(
                    None,
                    event,
                    EvtRenderEventXml.0 as u32,
                    (buffer.len() * 2) as u32,
                    Some(buffer.as_mut_ptr().cast()),
                    &mut used,
                    &mut count,
                );
                let xml = rendered.is_ok().then(|| {
                    let len = (used as usize / 2).min(buffer.len());
                    String::from_utf16_lossy(&buffer[..len])
                });
                let _ = EvtClose(event);
                match xml.as_deref().map(start_event) {
                    Some(Some(found)) if result.is_err() => {
                        result = Ok(found);
                    }
                    _ => {}
                }
            }
            if result.is_ok() {
                // close the remaining handles of this batch already done; stop.
                break 'events;
            }
        }
        let _ = EvtClose(query);
        result
    }
}

#[cfg(not(target_os = "windows"))]
fn latest_start_event() -> Result<(u64, StartKind), String> {
    Err("not Windows".into())
}

/// The last power-on (Unix seconds and kind), or `None` where we can't
/// tell better than the kernel uptime (non-Windows, event log unavailable).
/// Re-read every 5 minutes so a resume while Fob keeps running is seen.
pub fn last_start(now: u64, kernel_uptime: u64) -> Option<(u64, StartKind)> {
    use std::sync::Mutex;
    use std::time::{Duration, Instant};
    type Cached = Option<(Instant, Result<(u64, StartKind), String>)>;
    static CACHE: Mutex<Cached> = Mutex::new(None);
    let mut cache = CACHE.lock().ok()?;
    let fresh = matches!(&*cache, Some((at, _)) if at.elapsed() < Duration::from_secs(300));
    if !fresh {
        let value = latest_start_event();
        *cache = Some((Instant::now(), value));
    }
    let (event, kind) = cache.as_ref()?.1.clone().ok()?;
    let at = started_at(now, now.saturating_sub(kernel_uptime), &[event]);
    // An event from before the kernel boot was dropped: that is a cold boot.
    Some((at, if at == event { kind } else { StartKind::Cold }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_event_xml_times() {
        let xml =
            "<Event><System><Provider Name='Microsoft-Windows-Kernel-Power'/><EventID>27</EventID>\
                   <TimeCreated SystemTime='2026-11-02T01:23:45.1234567Z'/></System></Event>";
        assert_eq!(event_time(xml), Some(1_793_582_625));
        assert_eq!(
            event_time(r#"<TimeCreated SystemTime="1970-01-01T00:00:10Z"/>"#),
            Some(10)
        );
        assert_eq!(parse_utc("2000-02-29T00:00:00Z"), Some(951_782_400));
        assert_eq!(parse_utc("2026-11-02T01:23:45"), None); // must be UTC
        assert_eq!(parse_utc("2026-13-02T01:23:45Z"), None);
        assert_eq!(event_time("<TimeCreated/>"), None);
    }

    const BOOT_FAST: &str = "<Event xmlns='http://schemas.microsoft.com/win/2004/08/events/event'><System><Provider Name='Microsoft-Windows-Kernel-Boot' Guid='{15ca44ff-4d7a-4baa-bba5-0998955e531e}'/><EventID>27</EventID><Version>0</Version><Level>4</Level><TimeCreated SystemTime='2026-11-02T00:58:12.4991350Z'/><Channel>System</Channel></System><EventData><Data Name='BootType'>1</Data><Data Name='LoadOptions'></Data></EventData></Event>";
    const BOOT_COLD: &str = "<Event><System><Provider Name=\"Microsoft-Windows-Kernel-Boot\"/><EventID>27</EventID><TimeCreated SystemTime=\"2026-10-30T22:00:05.0Z\"/></System><EventData><Data Name=\"BootType\">0x0</Data></EventData></Event>";
    const GENERAL_12: &str = "<Event><System><Provider Name='Microsoft-Windows-Kernel-General' Guid='{a68ca8b7-004f-d7b6-a698-07e2de0f1f5d}'/><EventID>12</EventID><TimeCreated SystemTime='2026-10-30T22:00:01.5Z'/></System><EventData><Data Name='MajorVersion'>10</Data></EventData></Event>";
    const WAKE: &str = "<Event><System><Provider Name='Microsoft-Windows-Power-Troubleshooter' Guid='{cdc05e28-c449-49c6-b9d2-88cf761644df}'/><EventID Qualifiers='0'>1</EventID><TimeCreated SystemTime='2026-11-02T08:30:00.0Z'/></System><EventData><Data Name='SleepTime'>2026-11-01T16:00:00.0Z</Data><Data Name='WakeTime'>2026-11-02T08:29:55.0Z</Data></EventData></Event>";
    const OLD_KERNEL_POWER: &str = "<Event><System><Provider Name='Microsoft-Windows-Kernel-Power'/><EventID>27</EventID><TimeCreated SystemTime='2026-11-02T00:58:12Z'/></System></Event>";

    #[test]
    fn recognises_start_and_wake_events() {
        assert_eq!(
            start_event(BOOT_FAST),
            Some((
                parse_utc("2026-11-02T00:58:12Z").unwrap(),
                StartKind::FastStartup
            ))
        );
        assert_eq!(start_event(BOOT_COLD).map(|e| e.1), Some(StartKind::Cold));
        assert_eq!(start_event(GENERAL_12).map(|e| e.1), Some(StartKind::Cold));
        // Waking from sleep is not a start.
        assert_eq!(start_event(WAKE), None);
        // The provider v0.5.0 queried by mistake is not a start event.
        assert_eq!(start_event(OLD_KERNEL_POWER), None);
        // His case: kernel booted 2 days 21 h ago, Fast Startup resume this morning.
        let now = parse_utc("2026-11-02T11:00:00Z").unwrap();
        let kernel_boot = now - (2 * 24 + 21) * 3600;
        let (t, _) = start_event(BOOT_FAST).unwrap();
        assert_eq!(started_at(now, kernel_boot, &[t]), t);
        assert!(now - started_at(now, kernel_boot, &[t]) < 11 * 3600);
    }

    #[test]
    fn fast_startup_resume_wins_over_old_kernel_boot() {
        let now = 1_000_000;
        let kernel_boot = now - 69 * 3600; // "up 2 days, 21 hr"
        let resumed = now - 3 * 3600; // Fast Startup resume this morning
        assert_eq!(started_at(now, kernel_boot, &[resumed]), resumed);
        // Cold boot: the event is logged a few seconds after the kernel boot.
        assert_eq!(started_at(now, now - 600, &[now - 590]), now - 590);
        // Events older than the kernel boot (previous boots) are ignored.
        assert_eq!(started_at(now, now - 600, &[now - 9000]), now - 600);
        // A clock oddity never puts the start in the future.
        assert_eq!(started_at(now, now - 600, &[now + 30]), now);
        assert_eq!(started_at(now, now - 600, &[]), now - 600);
    }
}
