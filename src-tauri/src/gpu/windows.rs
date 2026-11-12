use super::{engine_counters, percent, GpuDevice};
use std::collections::BTreeMap;
use windows::core::{w, PCWSTR};
use windows::Win32::Graphics::Dxgi::{
    CreateDXGIFactory1, IDXGIFactory1, DXGI_ADAPTER_FLAG_SOFTWARE,
};
use windows::Win32::System::Performance::{
    PdhAddEnglishCounterW, PdhCloseQuery, PdhCollectQueryData, PdhGetFormattedCounterArrayW,
    PdhOpenQueryW, PDH_FMT_COUNTERVALUE_ITEM_W, PDH_FMT_DOUBLE, PDH_HCOUNTER, PDH_HQUERY,
};

pub fn collect(busy: &engine_counters::EngineSnapshot) -> Vec<GpuDevice> {
    let mut result = Vec::new();
    // Enumeration only. DXGI capacity is not used as a utilization counter.
    unsafe {
        let Ok(factory) = CreateDXGIFactory1::<IDXGIFactory1>() else {
            return result;
        };
        for index in 0..32 {
            let Ok(adapter) = factory.EnumAdapters1(index) else {
                break;
            };
            let Ok(desc) = adapter.GetDesc1() else {
                continue;
            };
            if desc.Flags & DXGI_ADAPTER_FLAG_SOFTWARE.0 as u32 != 0 {
                continue;
            }
            let end = desc
                .Description
                .iter()
                .position(|v| *v == 0)
                .unwrap_or(desc.Description.len());
            let id = format!(
                "luid:{:x}:{:x}",
                desc.AdapterLuid.HighPart, desc.AdapterLuid.LowPart
            );
            // winperf: utilization from PDH GPU engine counters (None if PDH failed).
            let utilization = busy_for(&busy.adapters, &id);
            let engines = busy.engines.get(&id).cloned().unwrap_or_default();
            result.push(GpuDevice {
                id,
                name: String::from_utf16_lossy(&desc.Description[..end]),
                vendor: match desc.VendorId {
                    0x1002 => "AMD",
                    0x8086 => "Intel",
                    0x10de => "NVIDIA",
                    _ => "Unknown",
                }
                .into(),
                utilization,
                memory_used: None,
                memory_total: (desc.DedicatedVideoMemory > 0)
                    .then_some(desc.DedicatedVideoMemory as u64),
                memory_kind: if desc.DedicatedVideoMemory > 0 {
                    "dedicated"
                } else {
                    "shared"
                }
                .into(),
                temperature: None,
                power_watts: None,
                core_clock_mhz: None,
                cores: None,
                source: if utilization.is_some() {
                    super::SOURCE_DXGI_PDH
                } else {
                    super::SOURCE_DXGI
                }
                .into(),
                fan_percent: None,
                engines,
            });
            if result.len() == 8 {
                break;
            }
        }
    }
    result
}

/// An adapter with no engine instances is idle (0%) once PDH has produced a
/// sample at all; if PDH is unavailable the map is empty and nothing is claimed.
fn busy_for(busy: &BTreeMap<String, f64>, id: &str) -> Option<f64> {
    if busy.is_empty() {
        return None;
    }
    percent(busy.get(id).copied().unwrap_or(0.0))
}

/// winperf: `\GPU Engine(*)\Utilization Percentage` query, kept open across
/// samples because the counter is a rate (needs two collections). No admin needed.
#[derive(Debug, Default)]
pub struct EngineCounters(Option<Query>);

#[derive(Debug)]
struct Query {
    query: PDH_HQUERY,
    counter: PDH_HCOUNTER,
    /// `\GPU Process Memory(*)\Dedicated Usage`; None if Windows lacks it.
    memory: Option<PDH_HCOUNTER>,
}
// PDH handles are plain handles; access is serialized by the GpuMonitor mutex.
unsafe impl Send for Query {}

impl Query {
    fn open() -> Option<Self> {
        unsafe {
            let mut query = PDH_HQUERY::default();
            if PdhOpenQueryW(PCWSTR::null(), 0, &mut query) != 0 {
                return None;
            }
            let mut counter = PDH_HCOUNTER::default();
            if PdhAddEnglishCounterW(
                query,
                w!(r"\GPU Engine(*)\Utilization Percentage"),
                0,
                &mut counter,
            ) != 0
            {
                PdhCloseQuery(query);
                return None;
            }
            // Same query, so memory and engine readings come from one
            // collection; no extra process walk.
            let mut memory = PDH_HCOUNTER::default();
            let memory = (PdhAddEnglishCounterW(
                query,
                w!(r"\GPU Process Memory(*)\Dedicated Usage"),
                0,
                &mut memory,
            ) == 0)
                .then_some(memory);
            PdhCollectQueryData(query); // first sample; rates need two
            Some(Self {
                query,
                counter,
                memory,
            })
        }
    }
    fn sample(&mut self) -> Option<engine_counters::EngineSnapshot> {
        unsafe {
            if PdhCollectQueryData(self.query) != 0 {
                return None;
            }
            let engines = counter_values(self.counter)?;
            let mut snapshot =
                engine_counters::engine_snapshot(engines.iter().map(|(n, v)| (n.as_str(), *v)));
            snapshot.process_memory = self
                .memory
                .and_then(|memory| counter_values(memory))
                .filter(|values| !values.is_empty())
                .map(|values| {
                    engine_counters::process_memory(values.iter().map(|(n, v)| (n.as_str(), *v)))
                });
            Some(snapshot)
        }
    }
}

/// Every instance of one counter with a valid value; None if PDH fails.
unsafe fn counter_values(counter: PDH_HCOUNTER) -> Option<Vec<(String, f64)>> {
    let (mut size, mut count) = (0u32, 0u32);
    PdhGetFormattedCounterArrayW(counter, PDH_FMT_DOUBLE, &mut size, &mut count, None);
    if size == 0 {
        return Some(Vec::new());
    }
    // u64 buffer keeps the item array 8-byte aligned.
    let mut buf = vec![0u64; (size as usize).div_ceil(8)];
    let items = buf.as_mut_ptr() as *mut PDH_FMT_COUNTERVALUE_ITEM_W;
    if PdhGetFormattedCounterArrayW(counter, PDH_FMT_DOUBLE, &mut size, &mut count, Some(items))
        != 0
    {
        return None;
    }
    let items = std::slice::from_raw_parts(items, count as usize);
    Some(
        items
            .iter()
            .filter(|i| engine_counters::usable_status(i.FmtValue.CStatus))
            .map(|i| {
                (
                    i.szName.to_string().unwrap_or_default(),
                    i.FmtValue.Anonymous.doubleValue,
                )
            })
            .collect(),
    )
}

impl Drop for Query {
    fn drop(&mut self) {
        unsafe {
            PdhCloseQuery(self.query);
        }
    }
}

impl EngineCounters {
    /// None on the first call and when PDH is unavailable; a failed query is
    /// reopened on the next call.
    pub fn sample(&mut self) -> Option<engine_counters::EngineSnapshot> {
        if self.0.is_none() {
            self.0 = Query::open();
            // Fresh query: first rate sample comes on the next collection.
            return None;
        }
        let snapshot = self.0.as_mut().and_then(Query::sample);
        if snapshot.is_none() {
            self.0 = None;
        }
        snapshot
    }
}
