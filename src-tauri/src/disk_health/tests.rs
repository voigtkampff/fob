use super::parse::{self, BadReply, DeviceInfo};
use super::*;
use std::sync::atomic::{AtomicUsize, Ordering};

fn log() -> Vec<u8> {
    let mut b = vec![0u8; parse::HEALTH_LOG];
    b[0] = 0b0000_0101; // spare below threshold, reliability degraded
    b[1..3].copy_from_slice(&318u16.to_le_bytes()); // 318 K = 45 °C
    b[3] = 87;
    b[4] = 10;
    b[5] = 12;
    b[32..48].copy_from_slice(&2_000_000u128.to_le_bytes());
    b[48..64].copy_from_slice(&u128::MAX.to_le_bytes());
    b[112..128].copy_from_slice(&1_234u128.to_le_bytes());
    b[128..144].copy_from_slice(&9_876u128.to_le_bytes());
    b[144..160].copy_from_slice(&56u128.to_le_bytes());
    b[176..192].copy_from_slice(&3u128.to_le_bytes());
    b[192..196].copy_from_slice(&7u32.to_le_bytes());
    b
}

fn reply(log: &[u8], offset: u32, length: u32) -> Vec<u8> {
    let mut b = vec![0u8; parse::PROTOCOL_DESCRIPTOR];
    b[0..4].copy_from_slice(&48u32.to_le_bytes());
    b[4..8].copy_from_slice(&48u32.to_le_bytes());
    b[8 + 16..8 + 20].copy_from_slice(&offset.to_le_bytes());
    b[8 + 20..8 + 24].copy_from_slice(&length.to_le_bytes());
    b.extend_from_slice(log);
    b
}

#[test]
fn health_log_fields_decode_with_units() {
    let h = parse::health_log(&log()).unwrap();
    assert_eq!(h.temperature_c, Some(45));
    assert_eq!((h.available_spare, h.spare_threshold), (Some(87), Some(10)));
    assert_eq!(h.percent_used, 12);
    assert!(h.critical_warning.spare_low && h.critical_warning.reliability);
    assert!(!h.critical_warning.temperature && !h.critical_warning.read_only);
    assert_eq!(h.data_read_bytes, 2_000_000.0 * 512_000.0);
    assert_eq!((h.power_cycles, h.power_on_hours), (1_234.0, 9_876.0));
    assert_eq!((h.unsafe_shutdowns, h.media_errors), (56.0, 0.0));
    assert_eq!(h.error_log_entries, 3.0);
    assert_eq!(h.warning_temp_minutes, 7);
}

#[test]
fn a_full_128_bit_counter_does_not_wrap() {
    let h = parse::health_log(&log()).unwrap();
    assert!(h.data_written_bytes.is_finite());
    assert!(h.data_written_bytes > 1e43, "{}", h.data_written_bytes);
}

#[test]
fn unreported_temperature_and_out_of_range_spare_are_absent_not_zero() {
    let mut b = log();
    b[1] = 0;
    b[2] = 0;
    b[3] = 255;
    b[4] = 101;
    let h = parse::health_log(&b).unwrap();
    assert_eq!(h.temperature_c, None);
    assert_eq!((h.available_spare, h.spare_threshold), (None, None));
    b[1..3].copy_from_slice(&263u16.to_le_bytes());
    assert_eq!(parse::health_log(&b).unwrap().temperature_c, Some(-10));
}

#[test]
fn short_or_malformed_replies_are_rejected() {
    assert_eq!(parse::health_log(&[0u8; 511]), Err(BadReply::Short));
    let good = reply(&log(), 40, 512);
    assert_eq!(parse::health_log_reply(&good).unwrap(), &log()[..]);
    assert_eq!(parse::health_log_reply(&good[..100]), Err(BadReply::Short));
    assert_eq!(parse::health_log_reply(&[0u8; 20]), Err(BadReply::Short));
    assert_eq!(
        parse::health_log_reply(&reply(&log(), 40, 256)),
        Err(BadReply::Header)
    );
    assert_eq!(
        parse::health_log_reply(&reply(&log(), 8, 512)),
        Err(BadReply::Header)
    );
    assert_eq!(
        parse::health_log_reply(&reply(&log(), u32::MAX, 512)),
        Err(BadReply::Short)
    );
    let mut bad = good.clone();
    bad[0] = 40;
    assert_eq!(parse::health_log_reply(&bad), Err(BadReply::Header));
}

#[test]
fn the_request_asks_for_the_health_log_page() {
    let r = parse::health_log_request();
    let at = |i: usize| u32::from_le_bytes(r[i..i + 4].try_into().unwrap());
    assert_eq!(r.len(), 8 + 40 + 512);
    assert_eq!((at(0), at(4)), (50, 0));
    assert_eq!((at(8), at(12), at(16)), (3, 2, 2));
    assert_eq!((at(8 + 16), at(8 + 20)), (40, 512));
}

fn descriptor(model: &[u8], bus: u32, removable: bool) -> Vec<u8> {
    let mut b = vec![0u8; 40];
    b[10] = removable as u8;
    b[16..20].copy_from_slice(&40u32.to_le_bytes());
    // A serial offset pointing at text that must never be read.
    b[24..28].copy_from_slice(&(40 + model.len() as u32 + 1).to_le_bytes());
    b[28..32].copy_from_slice(&bus.to_le_bytes());
    b.extend_from_slice(model);
    b.push(0);
    b.extend_from_slice(b"SERIAL123\0");
    let size = b.len() as u32;
    b[4..8].copy_from_slice(&size.to_le_bytes());
    b
}

#[test]
fn device_descriptor_reads_model_and_bus_never_the_serial() {
    let d = parse::device_descriptor(&descriptor(b"  NVMe SSD 1TB  ", 17, false)).unwrap();
    assert_eq!(d.model.as_deref(), Some("NVMe SSD 1TB"));
    assert_eq!((d.bus, d.removable), (Bus::Nvme, false));
    assert!(!format!("{d:?}").contains("SERIAL"));
    let usb = parse::device_descriptor(&descriptor(b"", 7, true)).unwrap();
    assert_eq!((usb.model, usb.bus, usb.removable), (None, Bus::Usb, true));
    assert_eq!(parse::device_descriptor(&[0u8; 20]), Err(BadReply::Short));
    let mut b = descriptor(b"X", 99, false);
    b[16..20].copy_from_slice(&5000u32.to_le_bytes());
    assert_eq!(parse::device_descriptor(&b).unwrap().model, None);
    assert_eq!(parse::device_descriptor(&b).unwrap().bus, Bus::Other);
}

#[test]
fn errors_say_denied_unsupported_or_the_code() {
    let nvme = |c| NvmeState::from(parse::refusal(c));
    assert_eq!(nvme(5), NvmeState::Denied);
    assert_eq!(nvme(1), NvmeState::Unsupported);
    assert_eq!(nvme(50), NvmeState::Unsupported);
    assert_eq!(nvme(87), NvmeState::Error { code: 87 });
    assert_eq!(nvme(1117), NvmeState::Error { code: 1117 });
    let ata = |c| AtaState::from(parse::refusal(c));
    assert_eq!(ata(5), AtaState::Denied);
    assert_eq!(ata(50), AtaState::Unsupported);
    assert_eq!(ata(1117), AtaState::Error { code: 1117 });
}

#[test]
fn only_buses_that_cannot_carry_nvme_skip_the_query() {
    for bus in [Bus::Sata, Bus::Ata, Bus::Virtual, Bus::Spaces] {
        assert!(bus.skips_nvme_query(), "{bus:?}");
    }
    for bus in [
        Bus::Nvme,
        Bus::Usb,
        Bus::Raid,
        Bus::Scsi,
        Bus::Sas,
        Bus::Unknown,
    ] {
        assert!(!bus.skips_nvme_query(), "{bus:?}");
    }
}

#[test]
fn only_sata_and_ata_disks_are_asked_for_ata_statistics() {
    for bus in [Bus::Sata, Bus::Ata] {
        assert!(bus.asks_ata(), "{bus:?}");
        assert!(bus.skips_nvme_query(), "{bus:?}");
    }
    for bus in [
        Bus::Nvme,
        Bus::Usb,
        Bus::Raid,
        Bus::Scsi,
        Bus::Sas,
        Bus::Atapi,
        Bus::Virtual,
        Bus::Spaces,
        Bus::Unknown,
        Bus::Other,
    ] {
        assert!(!bus.asks_ata(), "{bus:?}");
    }
}

fn raw(number: Option<u32>, bus: Bus, nvme: NvmeState) -> RawDisk {
    RawDisk {
        number,
        info: Some(DeviceInfo {
            model: Some("Disk".into()),
            bus,
            removable: false,
        }),
        nvme,
        ata: AtaState::NotAsked,
    }
}
fn wmi(id: &str, health: u32, ops: &[u32], bus: u32) -> WmiDisk {
    WmiDisk {
        device_id: id.into(),
        health: Some(health),
        operational: ops.to_vec(),
        bus: Some(bus),
    }
}
fn disks(r: Report) -> Vec<PhysicalDisk> {
    match r {
        Report::Ok { disks, .. } => disks,
        other => panic!("{other:?}"),
    }
}

#[test]
fn volumes_and_windows_status_join_by_disk_number() {
    let raw = vec![
        raw(Some(2), Bus::Sata, NvmeState::NotAsked),
        raw(Some(0), Bus::Nvme, NvmeState::Unsupported),
        raw(None, Bus::Usb, NvmeState::Error { code: 31 }),
        raw(Some(5), Bus::Nvme, NvmeState::Denied),
    ];
    let volumes = vec![
        RawVolume {
            mounts: vec![r"D:\".into()],
            disks: vec![0],
        },
        RawVolume {
            mounts: vec![r"C:\".into()],
            disks: vec![0],
        },
        // EFI partition: on disk 0, no mount point.
        RawVolume {
            mounts: vec![],
            disks: vec![0],
        },
        // Spanned over disks 2 and 5.
        RawVolume {
            mounts: vec![r"E:\".into(), r"C:\Data\".into()],
            disks: vec![2, 5],
        },
    ];
    let rows = Ok(vec![
        wmi("0", 0, &[2], 17),
        wmi("2", 1, &[2, 5], 11),
        // DeviceId 5 but a SATA bus: not disk 5 (NVMe), so not used.
        wmi("5", 2, &[6], 11),
        wmi("{pool-member}", 2, &[6], 16),
    ]);
    let d = disks(assemble(raw, &volumes, &rows, 1));
    assert_eq!(
        d.iter().map(|d| d.number).collect::<Vec<_>>(),
        [Some(0), Some(2), Some(5), None]
    );
    assert_eq!(d[0].volumes, [r"C:\", r"D:\"]);
    assert_eq!(d[1].volumes, [r"C:\Data\", r"E:\"]);
    assert_eq!(d[2].volumes, [r"C:\Data\", r"E:\"]);
    assert!(d[3].volumes.is_empty());
    assert_eq!(
        d[0].windows,
        WindowsStatus::Ok {
            health: Health::Healthy,
            operational: vec![Operational::Ok]
        }
    );
    assert_eq!(
        d[1].windows,
        WindowsStatus::Ok {
            health: Health::Warning,
            operational: vec![Operational::Ok, Operational::PredictiveFailure]
        }
    );
    assert_eq!(d[2].windows, WindowsStatus::NotReported);
    assert_eq!(d[3].windows, WindowsStatus::NotReported);
    assert_eq!(d[3].nvme, NvmeState::Error { code: 31 });
}

#[test]
fn duplicate_wmi_rows_and_failed_wmi_are_not_guessed() {
    let rows = Ok(vec![wmi("0", 0, &[2], 17), wmi("0", 2, &[6], 17)]);
    let d = disks(assemble(
        vec![raw(Some(0), Bus::Nvme, NvmeState::NotAsked)],
        &[],
        &rows,
        1,
    ));
    assert_eq!(d[0].windows, WindowsStatus::NotReported);
    let d = disks(assemble(
        vec![raw(Some(0), Bus::Nvme, NvmeState::NotAsked)],
        &[],
        &Err(-2147217405),
        1,
    ));
    assert_eq!(d[0].windows, WindowsStatus::Failed { code: -2147217405 });
    let unknown = Ok(vec![WmiDisk {
        device_id: "0".into(),
        health: None,
        operational: vec![],
        bus: None,
    }]);
    let d = disks(assemble(
        vec![raw(Some(0), Bus::Nvme, NvmeState::NotAsked)],
        &[],
        &unknown,
        1,
    ));
    assert_eq!(
        d[0].windows,
        WindowsStatus::Ok {
            health: Health::Unknown,
            operational: vec![]
        }
    );
    assert_eq!(Health::from_wmi(Some(5)), Health::Unknown);
    assert_eq!(
        Operational::from_wmi(0xD00D),
        Operational::Other { code: 0xD00D }
    );
}

#[test]
fn a_disk_whose_probe_failed_stays_in_the_list() {
    let failed = RawDisk {
        number: Some(1),
        info: None,
        nvme: NvmeState::Error { code: 21 },
        ata: AtaState::NotAsked,
    };
    let d = disks(assemble(
        vec![failed, raw(Some(0), Bus::Nvme, NvmeState::NotAsked)],
        &[],
        &Ok(vec![]),
        1,
    ));
    assert_eq!(d.len(), 2);
    assert_eq!((d[1].bus, d[1].model.clone()), (Bus::Unknown, None));
}

#[test]
fn the_report_serializes_tagged_for_the_ui() {
    let d = disks(assemble(
        vec![raw(
            Some(0),
            Bus::Nvme,
            NvmeState::Ok {
                log: parse::health_log(&log()).unwrap(),
            },
        )],
        &[],
        &Ok(vec![]),
        42,
    ));
    let json = serde_json::to_value(Report::Ok {
        checked_at_ms: 42,
        disks: d,
    })
    .unwrap();
    assert_eq!(json["kind"], "ok");
    assert_eq!(json["disks"][0]["bus"], "nvme");
    assert_eq!(json["disks"][0]["nvme"]["kind"], "ok");
    assert_eq!(json["disks"][0]["nvme"]["log"]["temperature_c"], 45);
    assert_eq!(json["disks"][0]["windows"]["kind"], "not_reported");
    assert_eq!(
        serde_json::to_value(Report::Pending).unwrap()["kind"],
        "pending"
    );
}

fn report(n: u64) -> Report {
    Report::Ok {
        checked_at_ms: n,
        disks: vec![],
    }
}

#[test]
fn concurrent_requests_share_one_scan_and_reuse_it_until_the_ttl() {
    let state = DiskHealthState::default();
    let scans = Arc::new(AtomicUsize::new(0));
    let threads: Vec<_> = (0..8)
        .map(|_| {
            let (state, scans) = (state.clone(), scans.clone());
            std::thread::spawn(move || {
                state.get(TTL, Duration::from_secs(5), move || {
                    scans.fetch_add(1, Ordering::SeqCst);
                    std::thread::sleep(Duration::from_millis(200));
                    report(1)
                })
            })
        })
        .collect();
    for t in threads {
        assert_eq!(t.join().unwrap(), report(1));
    }
    assert_eq!(scans.load(Ordering::SeqCst), 1);
    let again = state.get(TTL, Duration::from_secs(1), || report(2));
    assert_eq!(
        again,
        report(1),
        "inside the TTL the cached report is reused"
    );
    let expired = state.get(Duration::ZERO, Duration::from_secs(1), || report(3));
    assert_eq!(expired, report(3), "past the TTL a new scan runs");
}

#[test]
fn a_slow_scan_returns_pending_and_is_not_started_twice() {
    let state = DiskHealthState::default();
    let scans = Arc::new(AtomicUsize::new(0));
    let s = scans.clone();
    let first = state.get(TTL, Duration::from_millis(50), move || {
        s.fetch_add(1, Ordering::SeqCst);
        std::thread::sleep(Duration::from_millis(400));
        report(7)
    });
    assert_eq!(first, Report::Pending);
    let s = scans.clone();
    let second = state.get(TTL, Duration::from_secs(5), move || {
        s.fetch_add(1, Ordering::SeqCst);
        report(8)
    });
    assert_eq!(second, report(7));
    assert_eq!(scans.load(Ordering::SeqCst), 1);
}

const SUPPORTED: u64 = 1 << 63;
const VALID: u64 = 1 << 62;

/// A Device Statistics page with its header and QWORD statistics.
fn page(number: u8, stats: &[(usize, u64)]) -> Vec<u8> {
    let mut p = vec![0u8; parse::ATA_PAGE];
    p[..8].copy_from_slice(&(1u64 | u64::from(number) << 16).to_le_bytes());
    for &(at, q) in stats {
        p[at..at + 8].copy_from_slice(&q.to_le_bytes());
    }
    p
}
fn ok(v: u64) -> u64 {
    SUPPORTED | VALID | v
}

#[test]
fn ata_page_request_asks_for_one_device_statistics_page() {
    let r = parse::ata_page_request(parse::PAGE_TEMPERATURE);
    let u = |at: usize| u32::from_le_bytes(r[at..at + 4].try_into().unwrap());
    assert_eq!(
        r.len(),
        parse::QUERY_HEADER + parse::PROTOCOL_DATA + parse::ATA_PAGE
    );
    assert_eq!((u(0), u(4)), (50, 0));
    assert_eq!((u(8), u(12), u(16), u(20)), (2, 2, 0x04, 5));
    assert_eq!((u(24), u(28)), (parse::PROTOCOL_DATA as u32, 512));
}

#[test]
fn ata_statistics_decode_with_units() {
    let general = page(
        1,
        &[
            (8, ok(1234)),
            (16, ok(0xFFFF_0000_5678)),
            (24, ok(2_000_000)),
            (40, ok(1_000_000)),
        ],
    );
    let temperature = page(5, &[(8, ok(41))]);
    let solid = page(7, &[(8, ok(3))]);
    let s = parse::ata_stats(Some(&general), Some(&temperature), Some(&solid), Some(4096));
    assert_eq!(s.power_cycles, Some(1234));
    // Power-on hours are bits 31:0 of the value.
    assert_eq!(s.power_on_hours, Some(0x5678));
    assert_eq!(s.data_written_bytes, Some(2_000_000.0 * 4096.0));
    assert_eq!(s.data_read_bytes, Some(1_000_000.0 * 4096.0));
    assert_eq!(s.temperature_c, Some(41));
    assert_eq!(s.percent_used, Some(3));
}

#[test]
fn ata_statistics_need_supported_and_valid_bits() {
    let general = page(
        1,
        &[(8, SUPPORTED | 7), (16, VALID | 7), (24, 7), (40, ok(9))],
    );
    let s = parse::ata_stats(Some(&general), None, None, Some(512));
    assert_eq!(s.power_cycles, None);
    assert_eq!(s.power_on_hours, None);
    assert_eq!(s.data_written_bytes, None);
    assert_eq!(s.data_read_bytes, Some(9.0 * 512.0));
    assert_eq!(s.temperature_c, None);
    assert_eq!(s.percent_used, None);
}

#[test]
fn ata_temperature_is_signed_and_80h_is_no_value() {
    let t = |q| parse::ata_stats(None, Some(&page(5, &[(8, q)])), None, None).temperature_c;
    assert_eq!(t(ok(0xF6)), Some(-10));
    assert_eq!(t(ok(0x80)), None);
    assert_eq!(t(ok(0)), Some(0));
    // Bits above the byte are reserved.
    assert_eq!(t(ok(0xAB00_0000_0028)), Some(40));
}

#[test]
fn ata_life_used_can_pass_100_and_zero_is_a_reading() {
    let u = |v| parse::ata_stats(None, None, Some(&page(7, &[(8, ok(v))])), None).percent_used;
    assert_eq!(u(0), Some(0));
    assert_eq!(u(255), Some(255));
}

#[test]
fn ata_byte_counts_need_a_sector_size_and_must_not_overflow() {
    let general = page(1, &[(24, ok(0xFFFF_FFFF_FFFF)), (40, ok(10))]);
    let s = parse::ata_stats(Some(&general), None, None, None);
    assert_eq!((s.data_written_bytes, s.data_read_bytes), (None, None));
    let s = parse::ata_stats(Some(&general), None, None, Some(65536));
    // 2^48 - 1 sectors of 64 KiB still fit in u64.
    assert_eq!(
        s.data_written_bytes,
        Some(((1u64 << 48) - 1) as f64 * 65536.0)
    );
    assert!(parse::ata_stats(None, None, None, Some(512)).is_empty());
}

#[test]
fn ata_short_pages_leave_fields_out() {
    let s = parse::ata_stats(Some(&[0u8; 20]), Some(&[0u8; 4]), Some(&[]), Some(512));
    assert!(s.is_empty());
}

#[test]
fn ata_page_replies_are_checked_before_use() {
    let good = page(5, &[(8, ok(30))]);
    let r = reply(&good, parse::PROTOCOL_DATA as u32, 512);
    assert_eq!(parse::ata_page_reply(&r, 5).unwrap(), &good[..]);
    // Another page than the one asked for.
    assert_eq!(parse::ata_page_reply(&r, 7), Err(BadReply::Header));
    // Revision 0.
    let mut zero = good.clone();
    zero[0] = 0;
    let r0 = reply(&zero, parse::PROTOCOL_DATA as u32, 512);
    assert_eq!(parse::ata_page_reply(&r0, 5), Err(BadReply::Header));
    // Truncated data, short length, offset inside the header, huge offset.
    assert_eq!(
        parse::ata_page_reply(&r[..r.len() - 1], 5),
        Err(BadReply::Short)
    );
    let short = reply(&good, parse::PROTOCOL_DATA as u32, 511);
    assert_eq!(parse::ata_page_reply(&short, 5), Err(BadReply::Header));
    let inside = reply(&good, 8, 512);
    assert_eq!(parse::ata_page_reply(&inside, 5), Err(BadReply::Header));
    let far = reply(&good, u32::MAX, 512);
    assert!(parse::ata_page_reply(&far, 5).is_err());
    assert_eq!(parse::ata_page_reply(&[0u8; 10], 5), Err(BadReply::Short));
}

#[test]
fn the_page_list_is_bounded_by_its_count() {
    let mut list = page(0, &[]);
    list[8] = 4;
    list[9..13].copy_from_slice(&[0, 1, 5, 7]);
    assert_eq!(parse::supported_pages(&list).unwrap(), vec![0, 1, 5, 7]);
    let mut list = vec![0u8; 12];
    list[8] = 200;
    assert_eq!(parse::supported_pages(&list), Err(BadReply::Short));
    assert_eq!(parse::supported_pages(&[0u8; 4]), Err(BadReply::Short));
}

#[test]
fn logical_sector_size_comes_only_from_a_sane_descriptor() {
    let desc = |size: u32, bytes: u32| {
        let mut b = vec![0u8; 28];
        b[0..4].copy_from_slice(&28u32.to_le_bytes());
        b[4..8].copy_from_slice(&size.to_le_bytes());
        b[16..20].copy_from_slice(&bytes.to_le_bytes());
        b
    };
    assert_eq!(parse::logical_sector_bytes(&desc(28, 512)), Some(512));
    assert_eq!(parse::logical_sector_bytes(&desc(28, 4096)), Some(4096));
    for bad in [0, 256, 520, 131072] {
        assert_eq!(parse::logical_sector_bytes(&desc(28, bad)), None, "{bad}");
    }
    assert_eq!(parse::logical_sector_bytes(&desc(16, 512)), None);
    assert_eq!(parse::logical_sector_bytes(&desc(28, 512)[..18]), None);
}

#[test]
fn ata_states_serialize_tagged_and_absent_fields_are_null() {
    let mut d = raw(Some(1), Bus::Sata, NvmeState::NotAsked);
    d.ata = AtaState::Ok {
        stats: AtaStats {
            temperature_c: Some(35),
            ..AtaStats::default()
        },
    };
    let mut asleep = raw(Some(2), Bus::Sata, NvmeState::NotAsked);
    asleep.ata = AtaState::Sleeping;
    let json = serde_json::to_value(assemble(vec![d, asleep], &[], &Ok(vec![]), 1)).unwrap();
    let ata = &json["disks"][0]["ata"];
    assert_eq!(ata["kind"], "ok");
    assert_eq!(ata["stats"]["temperature_c"], 35);
    assert!(ata["stats"]["percent_used"].is_null());
    assert!(ata["stats"]["data_written_bytes"].is_null());
    assert_eq!(json["disks"][1]["ata"]["kind"], "sleeping");
    assert_eq!(
        serde_json::to_value(AtaState::Error { code: 87 }).unwrap(),
        serde_json::json!({"kind": "error", "code": 87})
    );
}
