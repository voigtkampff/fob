//! winperf: per-adapter GPU utilization from the Windows PDH counter
//! `\GPU Engine(*)\Utilization Percentage`. Pure parsing/aggregation lives here so
//! it is unit-tested on every platform; the PDH query itself is in `windows.rs`.
//!
//! Instance names look like
//! `pid_1234_luid_0x00000000_0x0000D1A6_phys_0_eng_0_engtype_3D`.
//! Like Task Manager, an adapter's utilization is its busiest engine, where each
//! engine's value is the sum over all processes using it.
use std::collections::BTreeMap;

#[derive(Debug, PartialEq, Eq)]
pub struct EngineInstance<'a> {
    pub pid: u32,
    /// Same format as the DXGI inventory id: `luid:{high:x}:{low:x}`.
    pub adapter: String,
    pub phys: u32,
    pub engine: u32,
    pub engine_type: &'a str,
}

fn hex(value: &str) -> Option<u32> {
    let digits = value
        .strip_prefix("0x")
        .or_else(|| value.strip_prefix("0X"))?;
    u32::from_str_radix(digits, 16).ok()
}

pub fn parse_instance(name: &str) -> Option<EngineInstance<'_>> {
    let rest = name.strip_prefix("pid_")?;
    let (pid, rest) = rest.split_once("_luid_")?;
    let (high, rest) = rest.split_once('_')?;
    let (low, rest) = rest.split_once("_phys_")?;
    let (phys, rest) = rest.split_once("_eng_")?;
    let (engine, engine_type) = rest.split_once("_engtype_")?;
    Some(EngineInstance {
        pid: pid.parse().ok()?,
        adapter: format!("luid:{:x}:{:x}", hex(high)?, hex(low)?),
        phys: phys.parse().ok()?,
        engine: engine.parse().ok()?,
        engine_type,
    })
}

/// Map of adapter id -> utilization percent (0..=100). Unparseable instances and
/// non-finite values are ignored.
pub fn adapter_utilization<'a>(
    samples: impl IntoIterator<Item = (&'a str, f64)>,
) -> BTreeMap<String, f64> {
    let mut engines: BTreeMap<(String, u32, u32), f64> = BTreeMap::new();
    for (name, value) in samples {
        let Some(instance) = parse_instance(name) else {
            continue;
        };
        if !value.is_finite() || value < 0.0 {
            continue;
        }
        *engines
            .entry((instance.adapter, instance.phys, instance.engine))
            .or_default() += value;
    }
    let mut adapters: BTreeMap<String, f64> = BTreeMap::new();
    for ((adapter, _, _), busy) in engines {
        let entry = adapters.entry(adapter).or_default();
        *entry = entry.max(busy.min(100.0));
    }
    adapters
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_instance_names() {
        let i =
            parse_instance("pid_1234_luid_0x00000000_0x0000D1A6_phys_0_eng_3_engtype_3D").unwrap();
        assert_eq!(i.pid, 1234);
        assert_eq!(i.adapter, "luid:0:d1a6");
        assert_eq!((i.phys, i.engine, i.engine_type), (0, 3, "3D"));
        let i = parse_instance("pid_8_luid_0xFFFFFFFF_0x00012345_phys_1_eng_12_engtype_Compute_0")
            .unwrap();
        assert_eq!(i.adapter, "luid:ffffffff:12345");
        assert_eq!(i.engine_type, "Compute_0");
        for bad in [
            "",
            "_Total",
            "pid_x_luid_0x0_0x1_phys_0_eng_0_engtype_3D",
            "pid_1_luid_00_0x1_phys_0_eng_0_engtype_3D",
            "pid_1_luid_0x0_0x1_phys_0_eng_0",
        ] {
            assert!(parse_instance(bad).is_none(), "{bad}");
        }
    }

    #[test]
    fn busiest_engine_wins_and_processes_add_up() {
        let samples = [
            ("pid_1_luid_0x0_0xA_phys_0_eng_0_engtype_3D", 20.0),
            ("pid_2_luid_0x0_0xA_phys_0_eng_0_engtype_3D", 15.0),
            ("pid_2_luid_0x0_0xA_phys_0_eng_1_engtype_Copy", 30.0),
            ("pid_3_luid_0x0_0xB_phys_0_eng_0_engtype_VideoDecode", 7.5),
            ("garbage", 99.0),
            (
                "pid_4_luid_0x0_0xB_phys_0_eng_0_engtype_VideoDecode",
                f64::NAN,
            ),
        ];
        let u = adapter_utilization(samples);
        assert_eq!(u.len(), 2);
        assert_eq!(u["luid:0:a"], 35.0);
        assert_eq!(u["luid:0:b"], 7.5);
    }

    #[test]
    fn engine_sum_is_capped_and_idle_adapters_report_zero() {
        let u = adapter_utilization([
            ("pid_1_luid_0x0_0xA_phys_0_eng_0_engtype_3D", 80.0),
            ("pid_2_luid_0x0_0xA_phys_0_eng_0_engtype_3D", 70.0),
            ("pid_2_luid_0x0_0xC_phys_0_eng_0_engtype_3D", 0.0),
        ]);
        assert_eq!(u["luid:0:a"], 100.0);
        assert_eq!(u["luid:0:c"], 0.0);
        assert!(adapter_utilization([]).is_empty());
    }
}
