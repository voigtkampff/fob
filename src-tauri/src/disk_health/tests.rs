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
    assert_eq!(parse::query_error(5), NvmeState::Denied);
    assert_eq!(parse::query_error(1), NvmeState::Unsupported);
    assert_eq!(parse::query_error(50), NvmeState::Unsupported);
    assert_eq!(parse::query_error(87), NvmeState::Error { code: 87 });
    assert_eq!(parse::query_error(1117), NvmeState::Error { code: 1117 });
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

fn raw(number: Option<u32>, bus: Bus, nvme: NvmeState) -> RawDisk {
    RawDisk {
        number,
        info: Some(DeviceInfo {
            model: Some("Disk".into()),
            bus,
            removable: false,
        }),
        nvme,
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
