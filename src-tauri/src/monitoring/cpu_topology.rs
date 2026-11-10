//! Fob: which logical processors are performance cores and which are
//! efficiency cores, from the efficiency class Windows reports per CPU set.
//! Read once; the per-group load is averaged in the UI from `cpu_usage`.

use serde::Serialize;

/// Read from the CPUID vendor string, only to pick the group names: Intel
/// says P-core / E-core, everyone else gets the generic names.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CpuVendor {
    Intel,
    Amd,
    Other,
}

impl CpuVendor {
    pub fn from_id(vendor_id: &str) -> Self {
        match vendor_id.trim() {
            "GenuineIntel" => Self::Intel,
            "AuthenticAMD" => Self::Amd,
            _ => Self::Other,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CoreKind {
    Performance,
    Efficiency,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct CoreGroup {
    pub kind: CoreKind,
    /// Physical cores; less than `threads.len()` when the cores run two threads.
    pub cores: usize,
    /// Indexes into `SystemStats::cpu_usage`, ascending.
    pub threads: Vec<usize>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum CpuTopology {
    /// Not read (not Windows, the call failed, or it did not match the
    /// per-thread readings). The UI shows no split.
    Unknown,
    /// Every core has the same efficiency class. The UI shows no split.
    Uniform,
    /// Performance group first, then efficiency.
    Hybrid {
        vendor: CpuVendor,
        groups: Vec<CoreGroup>,
    },
}

/// One logical processor as Windows describes it (SYSTEM_CPU_SET_INFORMATION).
#[derive(Clone, Copy, Debug)]
pub struct CpuSet {
    pub group: u16,
    pub logical: u8,
    pub core: u8,
    pub efficiency_class: u8,
}

/// `threads` is the length of `cpu_usage`: sysinfo reads `\Processor(i)` for
/// i in 0..threads, which is logical processor i of processor group 0.
/// Windows gives higher efficiency classes to faster cores; the highest class
/// is the performance group and every lower class is efficiency.
pub fn classify(sets: &[CpuSet], threads: usize, vendor: CpuVendor) -> CpuTopology {
    if threads == 0
        || sets.len() != threads
        || sets
            .iter()
            .any(|s| s.group != 0 || s.logical as usize >= threads)
    {
        return CpuTopology::Unknown;
    }
    let mut seen = vec![false; threads];
    for s in sets {
        if std::mem::replace(&mut seen[s.logical as usize], true) {
            return CpuTopology::Unknown;
        }
    }
    let top = sets.iter().map(|s| s.efficiency_class).max().unwrap_or(0);
    if sets.iter().all(|s| s.efficiency_class == top) {
        return CpuTopology::Uniform;
    }
    let group = |kind: CoreKind| {
        let members: Vec<&CpuSet> = sets
            .iter()
            .filter(|s| (s.efficiency_class == top) == (kind == CoreKind::Performance))
            .collect();
        let mut threads: Vec<usize> = members.iter().map(|s| s.logical as usize).collect();
        threads.sort_unstable();
        let mut cores: Vec<u8> = members.iter().map(|s| s.core).collect();
        cores.sort_unstable();
        cores.dedup();
        CoreGroup {
            kind,
            cores: cores.len(),
            threads,
        }
    };
    CpuTopology::Hybrid {
        vendor,
        groups: vec![group(CoreKind::Performance), group(CoreKind::Efficiency)],
    }
}

/// All CPU sets of type CpuSetInformation, or None when the call fails.
#[cfg(target_os = "windows")]
pub fn read_cpu_sets() -> Option<Vec<CpuSet>> {
    use windows::Win32::System::SystemInformation::{
        CpuSetInformation, GetSystemCpuSetInformation, SYSTEM_CPU_SET_INFORMATION,
    };
    let mut needed = 0u32;
    unsafe {
        let _ = GetSystemCpuSetInformation(None, 0, &mut needed, None, None);
    }
    if needed == 0 {
        return None;
    }
    let mut buf = vec![0u64; (needed as usize).div_ceil(8)];
    let ok = unsafe {
        GetSystemCpuSetInformation(
            Some(buf.as_mut_ptr().cast()),
            needed,
            &mut needed,
            None,
            None,
        )
    };
    if !ok.as_bool() {
        return None;
    }
    let bytes = needed as usize;
    let base = buf.as_ptr().cast::<u8>();
    let full = std::mem::size_of::<SYSTEM_CPU_SET_INFORMATION>();
    let mut sets = Vec::new();
    let mut offset = 0usize;
    // Entries carry their own Size (and Type) in their first 8 bytes.
    while offset + 8 <= bytes {
        let (size, kind) = unsafe {
            (
                base.add(offset).cast::<u32>().read_unaligned() as usize,
                base.add(offset + 4).cast::<i32>().read_unaligned(),
            )
        };
        if size < 8 || offset + size > bytes {
            return None;
        }
        if kind == CpuSetInformation.0 {
            if size < full {
                return None;
            }
            let entry = unsafe {
                base.add(offset)
                    .cast::<SYSTEM_CPU_SET_INFORMATION>()
                    .read_unaligned()
            };
            let set = unsafe { entry.Anonymous.CpuSet };
            sets.push(CpuSet {
                group: set.Group,
                logical: set.LogicalProcessorIndex,
                core: set.CoreIndex,
                efficiency_class: set.EfficiencyClass,
            });
        }
        offset += size;
    }
    Some(sets)
}

/// Fob reads the core layout on Windows only; elsewhere the UI shows no split.
#[cfg(not(target_os = "windows"))]
pub fn read_cpu_sets() -> Option<Vec<CpuSet>> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn set(logical: u8, core: u8, efficiency_class: u8) -> CpuSet {
        CpuSet {
            group: 0,
            logical,
            core,
            efficiency_class,
        }
    }

    /// An Intel hybrid CPU with 8 P-cores (two threads each, 0-15) and 8 E-cores (16-23).
    fn intel_hybrid_8p8e() -> Vec<CpuSet> {
        let p = (0..16).map(|i| set(i, i / 2 * 2, 1));
        let e = (16..24).map(|i| set(i, i, 0));
        p.chain(e).collect()
    }

    fn groups(t: &CpuTopology) -> Vec<(CoreKind, usize, Vec<usize>)> {
        match t {
            CpuTopology::Hybrid { groups, .. } => groups
                .iter()
                .map(|g| (g.kind, g.cores, g.threads.clone()))
                .collect(),
            other => panic!("expected hybrid, got {other:?}"),
        }
    }

    #[test]
    fn intel_hybrid_8p8e_has_8_p_cores_on_16_threads_and_8_e_cores() {
        let t = classify(&intel_hybrid_8p8e(), 24, CpuVendor::Intel);
        assert_eq!(
            groups(&t),
            vec![
                (CoreKind::Performance, 8, (0..16).collect()),
                (CoreKind::Efficiency, 8, (16..24).collect()),
            ]
        );
        assert!(matches!(
            t,
            CpuTopology::Hybrid {
                vendor: CpuVendor::Intel,
                ..
            }
        ));
    }

    #[test]
    fn amd_dense_cores_split_the_same_way_and_keep_their_vendor() {
        // Ryzen AI 9 HX 370: 4 Zen 5 + 8 Zen 5c, two threads on every core,
        // listed out of order to show the order Windows uses does not matter.
        let mut sets: Vec<CpuSet> = (0..24)
            .map(|i| set(i, i / 2 * 2, if i < 8 { 1 } else { 0 }))
            .collect();
        sets.reverse();
        let t = classify(&sets, 24, CpuVendor::from_id("AuthenticAMD"));
        assert_eq!(
            groups(&t),
            vec![
                (CoreKind::Performance, 4, (0..8).collect()),
                (CoreKind::Efficiency, 8, (8..24).collect()),
            ]
        );
        assert!(matches!(
            t,
            CpuTopology::Hybrid {
                vendor: CpuVendor::Amd,
                ..
            }
        ));
    }

    #[test]
    fn one_efficiency_class_is_uniform_whatever_the_vendor() {
        let ryzen: Vec<CpuSet> = (0..16).map(|i| set(i, i / 2 * 2, 0)).collect();
        assert_eq!(classify(&ryzen, 16, CpuVendor::Amd), CpuTopology::Uniform);
        let old_laptop: Vec<CpuSet> = (0..4).map(|i| set(i, i / 2 * 2, 0)).collect();
        assert_eq!(
            classify(&old_laptop, 4, CpuVendor::Intel),
            CpuTopology::Uniform
        );
    }

    #[test]
    fn a_third_lower_class_joins_the_efficiency_group() {
        // Low-power E-cores below the E-cores: still two groups.
        let sets: Vec<CpuSet> = (0..4)
            .map(|i| set(i, i, 2))
            .chain((4..8).map(|i| set(i, i, 1)))
            .chain((8..10).map(|i| set(i, i, 0)))
            .collect();
        assert_eq!(
            groups(&classify(&sets, 10, CpuVendor::Intel)),
            vec![
                (CoreKind::Performance, 4, (0..4).collect()),
                (CoreKind::Efficiency, 6, (4..10).collect()),
            ]
        );
    }

    #[test]
    fn sets_that_do_not_match_the_thread_readings_are_unknown() {
        assert_eq!(classify(&[], 24, CpuVendor::Intel), CpuTopology::Unknown);
        assert_eq!(
            classify(&intel_hybrid_8p8e(), 0, CpuVendor::Intel),
            CpuTopology::Unknown
        );
        // Fewer readings than CPU sets (or the reverse).
        assert_eq!(
            classify(&intel_hybrid_8p8e(), 20, CpuVendor::Intel),
            CpuTopology::Unknown
        );
        assert_eq!(
            classify(&intel_hybrid_8p8e(), 32, CpuVendor::Intel),
            CpuTopology::Unknown
        );
        // A second processor group: sysinfo only reads group 0.
        let mut two_groups = intel_hybrid_8p8e();
        two_groups[23].group = 1;
        assert_eq!(
            classify(&two_groups, 24, CpuVendor::Intel),
            CpuTopology::Unknown
        );
        let mut duplicate = intel_hybrid_8p8e();
        duplicate[23].logical = 0;
        assert_eq!(
            classify(&duplicate, 24, CpuVendor::Intel),
            CpuTopology::Unknown
        );
    }

    #[test]
    fn serializes_as_a_tagged_state() {
        assert_eq!(
            serde_json::to_value(CpuTopology::Unknown).unwrap(),
            serde_json::json!({ "state": "unknown" })
        );
        let t = classify(&intel_hybrid_8p8e(), 24, CpuVendor::Intel);
        let v = serde_json::to_value(&t).unwrap();
        assert_eq!(v["state"], "hybrid");
        assert_eq!(v["vendor"], "intel");
        assert_eq!(v["groups"][0]["kind"], "performance");
        assert_eq!(v["groups"][0]["cores"], 8);
        assert_eq!(v["groups"][1]["threads"][0], 16);
    }
}
