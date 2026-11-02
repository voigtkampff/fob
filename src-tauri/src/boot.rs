//! winperf: "on since" for the uptime chip.
//!
//! With Fast Startup (hybrid shutdown, the Windows default) "Shut down" only
//! logs the user off and hibernates the kernel, so the kernel uptime that
//! sysinfo and Task Manager show keeps counting across days the PC was off.
//! What a user means by "on since" is the last time the PC was started. Every
//! start - cold boot, Fast Startup resume or resume from hibernation - logs
//! Microsoft-Windows-Kernel-Power event 27 ("The boot type was 0x0/0x1/0x2")
//! in the System log; a cold boot also logs Kernel-General event 12 (OS
//! started). We take the newest of those that isn't before the kernel boot.
//! Sleep (S3 / Modern Standby) does not count as a start. The System log is
//! readable without administrator rights.

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
fn latest_start_event() -> Option<u64> {
    use windows::core::w;
    use windows::Win32::System::EventLog::{
        EvtClose, EvtNext, EvtQuery, EvtQueryChannelPath, EvtQueryReverseDirection, EvtRender,
        EvtRenderEventXml, EVT_HANDLE,
    };
    unsafe {
        let query = EvtQuery(
            None,
            w!("System"),
            w!("*[System[(Provider[@Name='Microsoft-Windows-Kernel-Power'] and EventID=27) or (Provider[@Name='Microsoft-Windows-Kernel-General'] and EventID=12)]]"),
            (EvtQueryChannelPath.0 | EvtQueryReverseDirection.0) as u32,
        )
        .ok()?;
        let mut events = [0isize; 1];
        let mut returned = 0u32;
        let next = EvtNext(query, &mut events, 2_000, 0, &mut returned);
        let mut result = None;
        if next.is_ok() && returned == 1 {
            let event = EVT_HANDLE(events[0]);
            let mut buffer = vec![0u16; 4096];
            let (mut used, mut count) = (0u32, 0u32);
            if EvtRender(
                None,
                event,
                EvtRenderEventXml.0 as u32,
                (buffer.len() * 2) as u32,
                Some(buffer.as_mut_ptr().cast()),
                &mut used,
                &mut count,
            )
            .is_ok()
            {
                let len = (used as usize / 2).min(buffer.len());
                result = event_time(&String::from_utf16_lossy(&buffer[..len]));
            }
            let _ = EvtClose(event);
        }
        let _ = EvtClose(query);
        result
    }
}

#[cfg(not(target_os = "windows"))]
fn latest_start_event() -> Option<u64> {
    None
}

/// Unix seconds of the last start, or `None` where we can't tell better than
/// the kernel uptime (non-Windows, event log unavailable). Re-read every
/// 5 minutes so a Fast Startup resume while winperf keeps running is seen.
pub fn last_start(now: u64, kernel_uptime: u64) -> Option<u64> {
    use std::sync::Mutex;
    use std::time::{Duration, Instant};
    static CACHE: Mutex<Option<(Instant, Option<u64>)>> = Mutex::new(None);
    let mut cache = CACHE.lock().ok()?;
    let event = match *cache {
        Some((at, value)) if at.elapsed() < Duration::from_secs(300) => value,
        _ => {
            let value = latest_start_event();
            *cache = Some((Instant::now(), value));
            value
        }
    }?;
    Some(started_at(now, now.saturating_sub(kernel_uptime), &[event]))
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
