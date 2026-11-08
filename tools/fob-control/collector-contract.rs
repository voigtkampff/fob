use super::*;

fn frame(pid: u32, start: u64, name: &str, command: &[&str], environment: &[&str]) -> ProcessData {
    let strings = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
    ProcessData {
        pid,
        name: name.into(),
        cmd: strings(command).into(),
        user_id: Some("1000".into()),
        cpu_usage: 4.825,
        memory: 1234,
        status: ProcessStatus::Run,
        ppid: Some(2),
        environ: strings(environment).into(),
        root: "root".into(),
        virtual_memory: 4321,
        start_time: start,
        run_time: 7,
        disk_usage: sysinfo::DiskUsage {
            read_bytes: 19,
            written_bytes: 23,
            total_read_bytes: 29,
            total_written_bytes: 31,
        },
        session_id: Some(3),
    }
}

#[test]
fn stable_process_has_identical_visible_json_on_repeated_collection() {
    let mut monitor = ProcessMonitor::new();
    let make = || frame(7, 100, "a.exe", &["a.exe", "-x"], &["A=1"]);
    let first = monitor.build_process_info(vec![make()]);
    let next = monitor.build_process_info(vec![make()]);
    assert_eq!(
        serde_json::to_value(&first[0]).unwrap(),
        serde_json::to_value(&next[0]).unwrap()
    );
    let json = serde_json::to_value(&next[0]).unwrap();
    assert_eq!(json["command"], "a.exe -x");
    assert_eq!(json["environ"], serde_json::json!(["A=1"]));
    assert_eq!(json["gpu_usage"], serde_json::Value::Null);
    assert_eq!(json["threads"], serde_json::Value::Null);
    assert_eq!(json["ports"], serde_json::json!([]));
    assert_eq!(
        json,
        serde_json::json!({
            "gpu_usage": null, "pid": 7, "ppid": 2, "name": "a.exe", "cpu_usage": 4.825_f32,
            "memory_usage": 1234, "status": "Running", "user": "1000", "command": "a.exe -x",
            "threads": null, "environ": ["A=1"], "root": "root", "virtual_memory": 4321,
            "start_time": 100, "run_time": 7, "disk_usage": [19, 23], "session_id": 3, "ports": []
        })
    );
}

#[test]
fn dynamic_readings_and_returned_timestamps_are_not_cached() {
    let mut monitor = ProcessMonitor::new();
    monitor.build_process_info(vec![frame(7, 100, "a.exe", &["a.exe"], &["A=1"])]);
    let mut data = frame(7, 100, "a.exe", &["a.exe"], &["A=1"]);
    data.cpu_usage = normalize_cpu(115.8, 24);
    data.memory = 2048;
    data.virtual_memory = 4096;
    data.status = ProcessStatus::Sleep;
    data.run_time = 47;
    data.disk_usage.read_bytes = 71;
    data.disk_usage.written_bytes = 73;
    data.ppid = Some(5);
    data.session_id = None;
    let now = monitor.build_process_info(vec![data]);
    assert!((now[0].cpu_usage - 4.825).abs() < 0.0001);
    assert_eq!(now[0].memory_usage, 2048);
    assert_eq!(now[0].virtual_memory, 4096);
    assert_eq!(now[0].status, "Sleeping");
    assert_eq!(now[0].start_time, 100);
    assert_eq!(now[0].run_time, 47);
    assert_eq!(now[0].disk_usage, (71, 73));
    assert_eq!(now[0].ppid, 5);
    assert_eq!(now[0].session_id, None);
}

#[test]
fn pid_reuse_exposes_the_documented_variant_policy() {
    let mut monitor = ProcessMonitor::new();
    monitor.build_process_info(vec![frame(
        7,
        100,
        "old.exe",
        &["old.exe", "-x"],
        &["OLD=1"],
    )]);
    let now = monitor.build_process_info(vec![frame(7, 200, "new.exe", &["new.exe"], &["NEW=1"])]);
    assert_eq!(
        now[0].name,
        if CONTROL_BASELINE {
            "old.exe"
        } else {
            "new.exe"
        }
    );
    assert_eq!(
        now[0].command,
        if CONTROL_BASELINE {
            "old.exe -x"
        } else {
            "new.exe"
        }
    );
    assert_eq!(&*now[0].environ, &["NEW=1".to_string()]);
    assert_eq!(now[0].start_time, 200);
    assert_eq!(now[0].run_time, 7);
}

#[test]
fn exited_processes_disappear_from_output_and_retention_is_characterized() {
    let mut monitor = ProcessMonitor::new();
    monitor.build_process_info(vec![frame(7, 100, "old.exe", &["old.exe"], &[])]);
    assert!(monitor.build_process_info(vec![]).is_empty());
    let back = monitor.build_process_info(vec![frame(7, 200, "new.exe", &["new.exe"], &[])]);
    assert_eq!(
        back[0].name,
        if CONTROL_BASELINE {
            "old.exe"
        } else {
            "new.exe"
        }
    );
    assert_eq!(monitor.process_cache.len(), 1);
    monitor.build_process_info(vec![]);
    assert_eq!(
        monitor.process_cache.len(),
        if CONTROL_BASELINE { 1 } else { 0 }
    );
}

#[test]
fn late_optional_command_and_environment_follow_the_variant_policy() {
    let mut monitor = ProcessMonitor::new();
    let mut missing = frame(7, 100, "a.exe", &[], &[]);
    missing.user_id = None;
    let first = monitor.build_process_info(vec![missing]);
    assert_eq!(first[0].command, "");
    assert!(first[0].environ.is_empty());
    assert_eq!(first[0].user, "-");
    let later =
        monitor.build_process_info(vec![frame(7, 100, "a.exe", &["a.exe", "-x"], &["A=1"])]);
    assert_eq!(
        later[0].command,
        if CONTROL_BASELINE { "" } else { "a.exe -x" }
    );
    assert_eq!(&*later[0].environ, &["A=1".to_string()]);
    assert_eq!(later[0].user, "-");
    let missing_again = monitor.build_process_info(vec![frame(7, 100, "a.exe", &[], &[])]);
    assert_eq!(missing_again[0].environ.is_empty(), CONTROL_BASELINE);
}

#[test]
fn runtime_uses_the_supplied_collection_timestamp() {
    let mut sys = sysinfo::System::new();
    sys.refresh_processes(sysinfo::ProcessesToUpdate::All, true);
    let current = ProcessMonitor::get_current_time().unwrap();
    let monitor = ProcessMonitor::new();
    let raw = monitor.collect_process_data(&sys, current + 37);
    assert!(!raw.is_empty());
    for data in raw {
        assert_eq!(
            data.run_time,
            if data.start_time > 0 {
                (current + 37).saturating_sub(data.start_time)
            } else {
                0
            }
        );
    }
}

#[test]
fn actual_collector_repeats_or_skips_command_copy_as_declared() {
    let mut sys = sysinfo::System::new();
    sys.refresh_all();
    let mut monitor = ProcessMonitor::new();
    let now = ProcessMonitor::get_current_time().unwrap();
    let first = monitor.collect_process_data(&sys, now);
    let own = first.iter().find(|p| p.pid == std::process::id()).unwrap();
    let encoded = serde_json::to_value(&own.cmd).unwrap();
    assert!(encoded.as_array().is_some_and(|v| !v.is_empty()));
    monitor.build_process_info(first);
    let next = monitor.collect_process_data(&sys, now + 3);
    let own = next.iter().find(|p| p.pid == std::process::id()).unwrap();
    assert_eq!(
        serde_json::to_value(&own.cmd).unwrap().is_null(),
        !CONTROL_BASELINE
    );
}
