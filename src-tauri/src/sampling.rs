//! Both windows read one interval-gated capture and one bounded in-memory history.
use crate::{
    gpu::GpuDevice,
    monitoring::{ProcessInfo, SystemStats},
    state::AppState,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, VecDeque},
    sync::Arc,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

pub fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}
pub fn interval(ms: u64) -> u64 {
    if ms == 1000 {
        1500
    } else {
        ms.clamp(1500, 30000)
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct Frame {
    pub captured_at: u64,
    pub sequence: u64,
    pub processes: Vec<ProcessInfo>,
    pub stats: SystemStats,
    pub gpus: Vec<GpuDevice>,
    /// winperf additions (sensors, commit charge).
    pub extra: Extra,
}
#[derive(Clone, Debug, Default, Serialize)]
pub struct Extra {
    pub sensors: crate::sensors::SensorReport,
    /// Windows commit charge (used, limit) in bytes.
    pub commit: Option<(u64, u64)>,
    /// winperf: Unix seconds and kind ("cold" | "fast" | "hibernate") of the
    /// last power-on; `None` = only the kernel uptime is known.
    pub started: Option<(u64, &'static str)>,
    /// winperf: per-process network rates from the sensor helper (ETW);
    /// `None` when the helper isn't running.
    pub net_apps: Option<Vec<crate::sensors::pipe::NetApp>>,
}
#[cfg(target_os = "windows")]
fn commit_charge() -> Option<(u64, u64)> {
    use windows::Win32::System::SystemInformation::{GlobalMemoryStatusEx, MEMORYSTATUSEX};
    let mut status = MEMORYSTATUSEX {
        dwLength: std::mem::size_of::<MEMORYSTATUSEX>() as u32,
        ..Default::default()
    };
    unsafe { GlobalMemoryStatusEx(&mut status) }.ok()?;
    Some((
        status
            .ullTotalPageFile
            .saturating_sub(status.ullAvailPageFile),
        status.ullTotalPageFile,
    ))
}
#[cfg(not(target_os = "windows"))]
fn commit_charge() -> Option<(u64, u64)> {
    None
}
#[derive(Clone, Debug, Serialize)]
pub struct Sample {
    pub timestamp: u64,
    pub interval: u64,
    pub cpu: Option<f64>,
    pub memory: Option<f64>,
    pub received: Option<u64>,
    pub sent: Option<u64>,
    pub gpus: BTreeMap<String, Option<f64>>,
    /// Fob: the Temp & fan card's headline readings (°C), picked like the
    /// frontend's sensorSummary (CPU package first, then any CPU sensor).
    pub cpu_temp: Option<f64>,
    pub gpu_temp: Option<f64>,
    /// Fob: storage read / write, bytes/s.
    pub disk_read: Option<u64>,
    pub disk_write: Option<u64>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Device {
    Cpu,
    Gpu,
    Other,
}

/// Which device a sensor belongs to. LibreHardwareMonitor ids start with the
/// hardware type ("/amdcpu/0/…", "/gpu-nvidia/0/…"), so an APU named
/// "… w/ Radeon 780M Graphics" is still a CPU; NVML ids start "nvml/".
/// Without an id, the hardware name decides, CPU words first. Same rules as
/// sensorDevice() in src/lib/utils/dashboard.js.
fn device(s: &crate::sensors::Sensor) -> Device {
    let id = s.id.to_lowercase();
    if !id.is_empty() {
        let kind = id
            .trim_start_matches('/')
            .split('/')
            .next()
            .unwrap_or_default();
        return match kind {
            "amdcpu" | "intelcpu" | "cpu" => Device::Cpu,
            "nvidiagpu" | "atigpu" | "nvml" => Device::Gpu,
            k if k.starts_with("gpu-") => Device::Gpu,
            _ => Device::Other,
        };
    }
    let hw = s.hardware.to_lowercase();
    let word = |w: &str| hw.split(|c: char| !c.is_alphanumeric()).any(|x| x == w);
    if hw.contains("ryzen")
        || hw.contains("intel core")
        || hw.contains("core i")
        || hw.contains("core ultra")
        || word("cpu")
        || hw.contains("processor")
    {
        Device::Cpu
    } else if ["gpu", "nvidia", "geforce", "radeon"]
        .iter()
        .any(|w| hw.contains(w))
        || word("arc")
    {
        Device::Gpu
    } else {
        Device::Other
    }
}

/// CPU temperature order (lower first): package, Tctl/Tdie, Tdie, a CCD
/// average or max, a core max or average, anything else (cpuTempRank()).
fn cpu_rank(name: &str) -> u8 {
    let n = name.to_lowercase();
    let avg_max = n.contains("average") || n.contains("max");
    if n.contains("package") {
        0
    } else if n.contains("tctl") {
        1
    } else if n.contains("tdie") && !n.contains("ccd") {
        2
    } else if n.contains("ccd") && avg_max {
        3
    } else if n.contains("core") && avg_max {
        4
    } else {
        5
    }
}

/// GPU: the core reading before hot spot and memory (gpuTempRank()).
fn gpu_rank(name: &str) -> u8 {
    let n = name.to_lowercase();
    u8::from(n.contains("hot") || n.contains("memory"))
}

/// The CPU and GPU temperature every view shows (status bar, charts); the
/// pages use sensorSummary() with the same rules.
fn headline_temps(sensors: &[crate::sensors::Sensor]) -> (Option<f64>, Option<f64>) {
    use crate::sensors::SensorKind;
    let best = |want: Device, rank: fn(&str) -> u8| {
        let mut pick: Option<&crate::sensors::Sensor> = None;
        for s in sensors {
            if s.kind == SensorKind::Temperature
                && s.value.is_finite()
                && device(s) == want
                && pick.is_none_or(|p| rank(&s.name) < rank(&p.name))
            {
                pick = Some(s);
            }
        }
        pick
    };
    (
        best(Device::Cpu, cpu_rank).map(|s| s.value),
        best(Device::Gpu, gpu_rank).map(|s| s.value),
    )
}
impl Sample {
    fn gap(timestamp: u64, interval: u64) -> Self {
        Self {
            timestamp,
            interval,
            cpu: None,
            memory: None,
            received: None,
            sent: None,
            gpus: BTreeMap::new(),
            cpu_temp: None,
            gpu_temp: None,
            disk_read: None,
            disk_write: None,
        }
    }
    fn from_frame(frame: &Frame, interval: u64) -> Self {
        let stats = &frame.stats;
        let temps = headline_temps(&frame.extra.sensors.sensors);
        let cpu = if stats.cpu_usage.is_empty() || stats.cpu_usage.iter().any(|v| !v.is_finite()) {
            None
        } else {
            Some(
                (stats.cpu_usage.iter().map(|v| *v as f64).sum::<f64>()
                    / stats.cpu_usage.len() as f64)
                    .clamp(0.0, 100.0),
            )
        };
        Self {
            cpu,
            memory: (stats.memory_total > 0).then(|| {
                (stats.memory_used as f64 / stats.memory_total as f64 * 100.0).clamp(0.0, 100.0)
            }),
            received: stats.network_rx_bytes,
            sent: stats.network_tx_bytes,
            gpus: frame
                .gpus
                .iter()
                .map(|g| (g.id.clone(), g.utilization))
                .collect(),
            cpu_temp: temps.0,
            gpu_temp: temps.1,
            disk_read: stats.disk_read_bytes,
            disk_write: stats.disk_write_bytes,
            ..Self::gap(frame.captured_at, interval)
        }
    }
}
fn append(history: &mut VecDeque<Sample>, sample: Sample) {
    let timestamp = sample.timestamp;
    if history.back().is_some_and(|s| s.timestamp > timestamp) {
        history.clear();
    }
    if let Some(last) = history.back() {
        if timestamp.saturating_sub(last.timestamp) > last.interval.max(sample.interval) * 5 / 2 {
            history.push_back(Sample::gap(
                last.timestamp + (timestamp - last.timestamp) / 2,
                sample.interval,
            ));
        }
    }
    if history.back().is_some_and(|s| s.timestamp == timestamp) {
        history.pop_back();
    }
    history.push_back(sample);
    while history.len() > 160
        || history
            .front()
            .is_some_and(|s| s.timestamp < timestamp.saturating_sub(120000))
    {
        history.pop_front();
    }
}

#[derive(Debug)]
pub struct Hub {
    pub frame: Option<Arc<Frame>>,
    pub process_gpu: bool,
    gpu_process_monitor: crate::monitoring::GpuProcessMonitor,
    history: VecDeque<Sample>,
    history_epoch: String,
    pub interval: u64,
    pub paused: bool,
    last_attempt: Option<Instant>,
    sequence: u64,
    error: Option<String>,
}
impl Default for Hub {
    fn default() -> Self {
        Self {
            frame: None,
            process_gpu: false,
            gpu_process_monitor: crate::monitoring::GpuProcessMonitor::new(),
            history: VecDeque::new(),
            history_epoch: history_epoch(),
            interval: 3000,
            paused: false,
            last_attempt: None,
            sequence: 0,
            error: None,
        }
    }
}
#[derive(Serialize)]
pub struct Snapshot {
    pub port_error: Option<String>,
    pub frame: Arc<Frame>,
    pub history: Vec<Sample>,
    #[serde(skip)]
    history_epoch: String,
    pub paused: bool,
    pub interval: u64,
    pub error: Option<String>,
}
fn history_epoch() -> String {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    format!(
        "{}-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos(),
        NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    )
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct HistoryCursor {
    epoch: String,
    timestamp: u64,
}
#[derive(Serialize)]
pub struct HistoryUpdate {
    reset: bool,
    samples: Vec<Sample>,
    cursor: Option<HistoryCursor>,
    first_timestamp: Option<u64>,
}
impl HistoryUpdate {
    fn of(history: Vec<Sample>, epoch: String, cursor: Option<HistoryCursor>) -> Self {
        let after = cursor
            .filter(|c| c.epoch == epoch)
            .and_then(|c| history.iter().position(|s| s.timestamp == c.timestamp));
        let next = history.last().map(|s| HistoryCursor {
            epoch,
            timestamp: s.timestamp,
        });
        let first_timestamp = history.first().map(|s| s.timestamp);
        Self {
            reset: after.is_none(),
            cursor: next,
            first_timestamp,
            samples: history
                .into_iter()
                .skip(after.map_or(0, |i| i + 1))
                .collect(),
        }
    }
}
#[derive(Serialize)]
pub struct ProcessSnapshot {
    pub port_error: Option<String>,
    pub frame: Arc<Frame>,
    pub history: HistoryUpdate,
    pub paused: bool,
    pub interval: u64,
    pub error: Option<String>,
}
impl Snapshot {
    pub fn for_main(self, cursor: Option<HistoryCursor>) -> ProcessSnapshot {
        ProcessSnapshot {
            port_error: self.port_error,
            frame: self.frame,
            history: HistoryUpdate::of(self.history, self.history_epoch, cursor),
            paused: self.paused,
            interval: self.interval,
            error: self.error,
        }
    }
}
impl Hub {
    fn append_history(&mut self, sample: Sample) {
        if self
            .history
            .back()
            .is_some_and(|s| s.timestamp >= sample.timestamp)
        {
            self.history_epoch = history_epoch();
        }
        append(&mut self.history, sample);
    }
    fn due(&self) -> bool {
        self.frame.is_none()
            || (!self.paused
                && self
                    .last_attempt
                    .is_none_or(|i| i.elapsed() >= Duration::from_millis(self.interval)))
    }
    pub fn pause(&mut self, paused: bool) {
        if self.paused == paused {
            return;
        }
        self.paused = paused;
        self.append_history(Sample::gap(now_ms(), self.interval));
        // Preserve the CPU minimum refresh window when resuming immediately.
        if !paused
            && self
                .last_attempt
                .is_some_and(|i| i.elapsed() >= Duration::from_millis(1500))
        {
            self.last_attempt = None;
        }
    }
    pub fn read(&mut self, state: &AppState) -> Result<Snapshot, String> {
        if self.due() {
            self.last_attempt = Some(Instant::now());
            match self.capture(state) {
                Ok(frame) => {
                    self.append_history(Sample::from_frame(&frame, self.interval));
                    self.frame = Some(Arc::new(frame));
                    self.error = None;
                }
                Err(error) => {
                    self.append_history(Sample::gap(now_ms(), self.interval));
                    self.error = Some(error);
                }
            }
        }
        Ok(Snapshot {
            port_error: None,
            frame: self.frame.clone().ok_or_else(|| {
                self.error
                    .clone()
                    .unwrap_or_else(|| "No system readings yet".into())
            })?,
            history: self.history.iter().cloned().collect(),
            history_epoch: self.history_epoch.clone(),
            paused: self.paused,
            interval: self.interval,
            error: self.error.clone(),
        })
    }
    fn capture(&mut self, state: &AppState) -> Result<Frame, String> {
        let (mut processes, stats) = {
            let mut sys = state.sys.lock().map_err(|e| e.to_string())?;
            let mut disks = state.disks.lock().map_err(|e| e.to_string())?;
            let mut networks = state.networks.lock().map_err(|e| e.to_string())?;
            sys.refresh_all();
            disks.refresh(true);
            networks.refresh(true);
            let processes = state
                .process_monitor
                .lock()
                .map_err(|e| e.to_string())?
                // Same gap rule as the history charts: a longer wait (paused,
                // stalled) is not reported as a current disk speed.
                .collect_processes(&sys, Duration::from_millis(self.interval * 5 / 2))?;
            let stats = state
                .system_monitor
                .lock()
                .map_err(|e| e.to_string())?
                .collect_stats(&sys, &networks, &disks);
            (processes, stats)
        };
        if self.process_gpu {
            let usage = self.gpu_process_monitor.collect(&processes);
            for process in &mut processes {
                process.gpu_usage = usage.get(&(process.pid, process.start_time)).copied();
            }
        } else {
            self.gpu_process_monitor.reset();
        }
        let gpus = {
            let mut gpu = state.gpu.lock().map_err(|e| e.to_string())?;
            let gpus = gpu.collect();
            // Windows PDH per-process GPU (Linux uses DRM fdinfo above).
            if let Some(usage) = &gpu.process_usage {
                for process in &mut processes {
                    let (busy, dedicated) = usage.of(process.pid);
                    process.gpu_usage.get_or_insert(busy);
                    process.gpu_memory = dedicated;
                }
            }
            gpus
        };
        let mut sensors = state.sensors.lock().map_err(|e| e.to_string())?.read(&gpus);
        let net_apps = sensors.net.take().map(|sample| {
            let names: std::collections::HashMap<u32, &str> =
                processes.iter().map(|p| (p.pid, p.name.as_str())).collect();
            crate::sensors::pipe::app_rates(&sample, |pid| names.get(&pid).map(|n| n.to_string()))
        });
        let extra = Extra {
            sensors,
            net_apps,
            commit: commit_charge(),
            started: crate::boot::last_start(now_ms() / 1000, stats.uptime)
                .map(|(at, kind)| (at, kind.as_str())),
        };
        self.sequence += 1;
        Ok(Frame {
            captured_at: now_ms(),
            sequence: self.sequence,
            processes,
            stats,
            gpus,
            extra,
        })
    }
}

#[derive(Clone, Serialize)]
pub struct BusyProcess {
    pub pid: u32,
    pub start_time: u64,
    pub name: String,
    pub cpu_usage: Option<f32>,
    pub memory_usage: u64,
    pub gpu_usage: Option<f32>,
}
impl BusyProcess {
    fn of(p: &ProcessInfo) -> Self {
        Self {
            pid: p.pid,
            start_time: p.start_time,
            name: p.name.clone(),
            cpu_usage: p.cpu_usage,
            memory_usage: p.memory_usage,
            gpu_usage: p.gpu_usage,
        }
    }
}
/// winperf: top `n` processes by a key (descending), ties by pid.
fn top_by<K: PartialOrd>(
    processes: &[ProcessInfo],
    n: usize,
    key: impl Fn(&ProcessInfo) -> Option<K>,
) -> Vec<BusyProcess> {
    let mut list: Vec<(&ProcessInfo, K)> = processes
        .iter()
        .filter_map(|p| key(p).map(|k| (p, k)))
        .collect();
    list.sort_by(|a, b| {
        b.1.partial_cmp(&a.1)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.0.pid.cmp(&b.0.pid))
    });
    list.into_iter()
        .take(n)
        .map(|(p, _)| BusyProcess::of(p))
        .collect()
}
#[derive(Serialize)]
pub struct Overview {
    pub captured_at: u64,
    pub stats: SystemStats,
    pub gpus: Vec<GpuDevice>,
    pub history: Vec<Sample>,
    pub processes: Vec<BusyProcess>,
    /// winperf detail views.
    pub top_cpu: Vec<BusyProcess>,
    pub top_gpu: Vec<BusyProcess>,
    pub top_memory: Vec<BusyProcess>,
    pub extra: Extra,
    pub paused: bool,
    pub interval: u64,
    pub error: Option<String>,
}
impl From<Snapshot> for Overview {
    fn from(s: Snapshot) -> Self {
        let all = &s.frame.processes;
        // winperf: the mini window lists the top processes by memory.
        let top_memory = top_by(all, 10, |p| Some(p.memory_usage));
        Self {
            captured_at: s.frame.captured_at,
            stats: s.frame.stats.clone(),
            gpus: s.frame.gpus.clone(),
            history: s.history,
            processes: top_memory.iter().take(5).cloned().collect(),
            top_cpu: top_by(all, 10, |p| p.cpu_usage.filter(|v| v.is_finite())),
            top_gpu: top_by(all, 10, |p| {
                p.gpu_usage.filter(|g| g.is_finite() && *g > 0.0)
            }),
            top_memory,
            extra: s.frame.extra.clone(),
            paused: s.paused,
            interval: s.interval,
            error: s.error,
        }
    }
}

#[tauri::command]
pub async fn get_overview(app: tauri::AppHandle) -> Result<Overview, String> {
    let overview: Overview = read(app).await?.into();
    // Debug builds only: WINPERF_DUMP_OVERVIEW=path writes the last reading
    // (screenshots of the mini page rendered outside the app).
    #[cfg(debug_assertions)]
    if let Some(path) = std::env::var_os("WINPERF_DUMP_OVERVIEW") {
        let _ = std::fs::write(path, serde_json::to_vec(&overview).unwrap_or_default());
    }
    Ok(overview)
}
pub async fn read(app: tauri::AppHandle) -> Result<Snapshot, String> {
    tauri::async_runtime::spawn_blocking(move || {
        use tauri::Manager;
        let state = app.state::<AppState>();
        let result = state.hub.lock().map_err(|e| e.to_string())?.read(&state);
        result
    })
    .await
    .map_err(|e| e.to_string())?
}
/// Fob: sample from app start, not only while a window is reading, so the
/// 120 s history charts are full when the user first opens a view. Before
/// this, a tray-only Fob took no readings until a window asked, and every
/// chart started mid-way. `Hub::read` is interval-gated (and idle while
/// paused), so polling it here adds no extra captures when windows read too.
pub fn sample_in_background(app: tauri::AppHandle) {
    std::thread::Builder::new()
        .name("fob-sampler".into())
        .spawn(move || loop {
            {
                use tauri::Manager;
                let state = app.state::<AppState>();
                if let Ok(mut hub) = state.hub.lock() {
                    let _ = hub.read(&state);
                };
            }
            std::thread::sleep(Duration::from_millis(500));
        })
        .expect("start the sampler thread");
}
#[tauri::command]
pub async fn set_monitor_paused(
    paused: bool,
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    use tauri::Emitter;
    state.hub.lock().map_err(|e| e.to_string())?.pause(paused);
    app.emit("monitor-control", ()).map_err(|e| e.to_string())
}
#[tauri::command]
pub async fn set_monitor_interval(
    milliseconds: u64,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    state.hub.lock().map_err(|e| e.to_string())?.interval = interval(milliseconds);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn assert_main_history(
        hub: &Hub,
        current: &mut Vec<Sample>,
        cursor: &mut Option<HistoryCursor>,
    ) -> bool {
        let update = HistoryUpdate::of(
            hub.history.iter().cloned().collect(),
            hub.history_epoch.clone(),
            cursor.clone(),
        );
        let reset = update.reset;
        if reset {
            *current = update.samples;
        } else {
            current.retain(|s| {
                update
                    .first_timestamp
                    .is_some_and(|first| s.timestamp >= first)
            });
            current.extend(update.samples);
        }
        *cursor = update.cursor;
        assert_eq!(
            serde_json::to_value(&*current).unwrap(),
            serde_json::to_value(&hub.history).unwrap()
        );
        reset
    }
    #[test]
    fn main_history_resumes_rolls_over_and_resets_without_changing_other_readers() {
        let mut hub = Hub::default();
        let (mut current, mut cursor) = (Vec::new(), None);
        for i in 0..180 {
            hub.append_history(Sample::gap(10000 + i * 1500, 1500));
            if i % 7 == 0 {
                assert_main_history(&hub, &mut current, &mut cursor);
            }
        }
        assert_main_history(&hub, &mut current, &mut cursor);
        let update = HistoryUpdate::of(
            hub.history.iter().cloned().collect(),
            hub.history_epoch.clone(),
            cursor.clone(),
        );
        assert!(!update.reset);
        assert!(update.samples.is_empty());
        // A second consumer starts with the full buffer, not the first one's cursor.
        let mut independent = Vec::new();
        let mut independent_cursor = None;
        assert!(assert_main_history(
            &hub,
            &mut independent,
            &mut independent_cursor
        ));
        for i in 180..300 {
            hub.append_history(Sample::gap(10000 + i * 1500, 1500));
        }
        assert!(assert_main_history(&hub, &mut current, &mut cursor));
        assert!(assert_main_history(
            &hub,
            &mut independent,
            &mut independent_cursor
        ));
        // Replacement and backward wall clock invalidate cursors even if timestamps match.
        hub.append_history(Sample::gap(hub.history.back().unwrap().timestamp, 3000));
        assert!(assert_main_history(&hub, &mut current, &mut cursor));
        hub.append_history(Sample::gap(5000, 3000));
        assert!(assert_main_history(&hub, &mut current, &mut cursor));
        let mut restarted = Hub::default();
        restarted.append_history(Sample::gap(5000, 3000));
        assert!(assert_main_history(&restarted, &mut current, &mut cursor));
        // Pause and failed/late captures can add history without a new frame sequence.
        restarted.append_history(Sample::gap(20000, 3000));
        assert!(!assert_main_history(&restarted, &mut current, &mut cursor));
    }

    #[test]
    fn headline_temps_pick_the_cpu_package_and_the_gpu() {
        use crate::sensors::{Sensor, SensorKind};
        let s = |hardware: &str, name: &str, kind, value| Sensor {
            hardware: hardware.into(),
            id: String::new(),
            name: name.into(),
            kind,
            value,
            source: "test".into(),
        };
        let list = vec![
            s(
                "Example Chip",
                "Temperature #1",
                SensorKind::Temperature,
                30.0,
            ),
            s(
                "Example Hybrid CPU",
                "Core #1",
                SensorKind::Temperature,
                48.0,
            ),
            s(
                "Example Hybrid CPU",
                "CPU Package",
                SensorKind::Temperature,
                53.0,
            ),
            s(
                "NVIDIA Example GPU",
                "GPU Core",
                SensorKind::Temperature,
                40.0,
            ),
            s("Example Chip", "Fan #1", SensorKind::Fan, 1100.0),
        ];
        assert_eq!(headline_temps(&list), (Some(53.0), Some(40.0)));
        assert_eq!(headline_temps(&list[..2]), (Some(48.0), None));
        assert_eq!(headline_temps(&[]), (None, None));
    }
    #[test]
    fn headline_temps_match_each_machine() {
        use crate::sensors::{Sensor, SensorKind};
        let fixture: serde_json::Value =
            serde_json::from_str(include_str!("../../tests/fixtures/headline-temps.json")).unwrap();
        for case in fixture["cases"].as_array().unwrap() {
            let sensors: Vec<Sensor> = case["sensors"]
                .as_array()
                .unwrap()
                .iter()
                .map(|s| Sensor {
                    hardware: s["hardware"].as_str().unwrap().into(),
                    id: s["id"].as_str().unwrap().into(),
                    name: s["name"].as_str().unwrap().into(),
                    kind: SensorKind::Temperature,
                    value: s["value"].as_f64().unwrap(),
                    source: "fixture".into(),
                })
                .collect();
            // Expected: a sensor id, or the value when the case has no ids.
            let want = |key: &str| match &case[key] {
                serde_json::Value::Null => None,
                serde_json::Value::Number(n) => n.as_f64(),
                v => sensors
                    .iter()
                    .find(|s| s.id == v.as_str().unwrap())
                    .map(|s| s.value),
            };
            let machine = case["machine"].as_str().unwrap();
            assert_eq!(
                headline_temps(&sensors),
                (want("cpu"), want("gpu")),
                "{machine}"
            );
        }
    }
    #[test]
    fn history_is_sampled_from_app_start() {
        let main = include_str!("main.rs");
        let setup = main.split(".setup(").nth(1).unwrap();
        let setup = &setup[..setup.find("Ok(())").unwrap()];
        assert!(setup.contains("sampling::sample_in_background(app.handle().clone())"));
    }
    #[test]
    fn bounded_history_clock_changes_and_missing_reads() {
        let mut h = VecDeque::new();
        append(&mut h, Sample::gap(10000, 3000));
        append(&mut h, Sample::gap(40000, 3000));
        assert_eq!(
            h.iter().map(|s| s.timestamp).collect::<Vec<_>>(),
            vec![10000, 25000, 40000]
        );
        for i in 0..10000 {
            append(&mut h, Sample::gap(50000 + i, 3000));
        }
        assert_eq!(h.len(), 160);
        append(&mut h, Sample::gap(500000, 3000));
        assert!(h.iter().all(|s| s.timestamp >= 380000));
        append(&mut h, Sample::gap(100, 3000));
        assert_eq!(h.len(), 1);
        append(&mut h, Sample::gap(100, 3000));
        assert_eq!(h.len(), 1);
    }
    #[test]
    fn shared_cache_pause_and_cadence() {
        let mut hub = Hub::default();
        assert!(hub.due());
        assert_eq!(interval(1000), 1500);
        assert_eq!(interval(300000), 30000);
        hub.pause(true);
        hub.pause(true);
        assert_eq!(hub.history.len(), 1);
        hub.pause(false);
        assert!(!hub.paused);
        // Capture readiness is interval-gated using monotonic time, not wall-clock.
        hub.last_attempt = Some(Instant::now());
        // A frame is required before the cache may suppress the first capture.
        assert!(hub.due());
    }
    #[test]
    fn changing_interval_does_not_create_a_false_gap() {
        let mut h = VecDeque::new();
        append(&mut h, Sample::gap(10000, 3000));
        append(&mut h, Sample::gap(40000, 30000));
        assert_eq!(h.len(), 2);
    }
    #[test]
    fn both_readers_share_one_capture_and_pause() {
        let state = AppState::new();
        let mut hub = Hub::default();
        let main = hub.read(&state).unwrap();
        let companion = hub.read(&state).unwrap();
        assert!(Arc::ptr_eq(&main.frame, &companion.frame));
        assert_eq!(main.history.len(), 1);
        assert_eq!(companion.history.len(), 1);
        hub.pause(true);
        let paused = hub.read(&state).unwrap();
        assert!(paused.paused);
        assert!(Arc::ptr_eq(&main.frame, &paused.frame));
        assert!(paused.history.last().unwrap().cpu.is_none());
        hub.last_attempt = Some(Instant::now() - Duration::from_secs(4));
        hub.pause(false);
        let resumed = hub.read(&state).unwrap();
        assert_eq!(resumed.frame.sequence, main.frame.sequence + 1);
        let overview: Overview = resumed.into();
        assert!(overview.processes.len() <= 5);
        assert!(overview.top_cpu.len() <= 10);
        let mem: Vec<u64> = overview.top_memory.iter().map(|p| p.memory_usage).collect();
        assert!(mem.windows(2).all(|w| w[0] >= w[1]));
    }
    #[test]
    fn gpu_history_keeps_adapter_identity_and_real_zeros() {
        let state = AppState::new();
        let mut hub = Hub::default();
        let snapshot = hub.read(&state).unwrap();
        let device = |id: &str, utilization: Option<f64>| GpuDevice {
            id: id.into(),
            name: id.into(),
            vendor: "Fixture".into(),
            utilization,
            memory_used: None,
            memory_total: None,
            memory_kind: "unknown".into(),
            temperature: None,
            power_watts: None,
            core_clock_mhz: None,
            cores: None,
            source: "Fixture".into(),
            fan_percent: None,
            engines: Vec::new(),
        };
        let frame = Frame {
            captured_at: 10000,
            sequence: 1,
            processes: vec![],
            stats: snapshot.frame.stats.clone(),
            gpus: vec![
                device("adapter-a", Some(0.0)),
                device("adapter-b", Some(75.0)),
                device("adapter-c", None),
            ],
            extra: Extra::default(),
        };
        let sample = Sample::from_frame(&frame, 3000);
        let mut history = VecDeque::new();
        append(&mut history, sample);
        append(&mut history, Sample::gap(13000, 3000));
        assert_eq!(history[0].gpus["adapter-a"], Some(0.0));
        assert_eq!(history[0].gpus["adapter-b"], Some(75.0));
        assert_eq!(history[0].gpus["adapter-c"], None);
        assert!(history[1].gpus.is_empty());
    }
}
