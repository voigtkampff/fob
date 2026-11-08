use fob_perf::{compare, model::*, store};
use std::collections::HashMap;
use std::path::PathBuf;
use std::process::ExitCode;

const USAGE: &str = "fob-perf: read-only Fob CPU/memory sampler
Run in PowerShell from the extracted directory. Double-clicking may close immediately.
App version, refresh, mode and helper state are declarations, not auto-detected.

  .\\fob-perf.exe smoke [--pid N]
      Find the running Fob and list measured and excluded processes. Writes nothing
  .\\fob-perf.exe sample --out DIR --label A|B --app-version V --mode tray|mini|statusbar|processes
                  --refresh-ms N --helper STATE [--settle-s 120] [--steady-s 600]
                  [--interval-ms 1000] [--setting KEY=VALUE]... [--pid N]
      Attach to the running Fob, sample, then write a new result directory DIR
  .\\fob-perf.exe analyze RUN_DIR --out NEW_DIR
      Recompute reports from a run's raw data into a new directory
  .\\fob-perf.exe compare --a LABEL --b LABEL [--out FILE] RUN_DIR...
      Compare complete runs. Rejects incompatible runs. Prints inconclusive
      until each side has at least 5 interleaved complete runs per mode
";

struct Args {
    flags: HashMap<String, Vec<String>>,
    pos: Vec<String>,
}

fn parse(v: &[String], known: &[&str]) -> Result<Args, String> {
    let mut a = Args { flags: HashMap::new(), pos: vec![] };
    let mut i = 0;
    while i < v.len() {
        if let Some(k) = v[i].strip_prefix("--") {
            if !known.contains(&k) {
                return Err(format!("unknown option --{k}"));
            }
            let val = v.get(i + 1).ok_or(format!("--{k} needs a value"))?;
            if val.starts_with("--") { return Err(format!("--{k} needs a value")); }
            if k != "setting" && a.flags.contains_key(k) { return Err(format!("duplicate --{k}")); }
            a.flags.entry(k.into()).or_default().push(val.clone());
            i += 2;
        } else {
            a.pos.push(v[i].clone());
            i += 1;
        }
    }
    Ok(a)
}

impl Args {
    fn one(&self, k: &str) -> Option<&str> {
        self.flags.get(k).and_then(|v| v.last()).map(|s| s.as_str())
    }
    fn req(&self, k: &str) -> Result<&str, String> {
        self.one(k).ok_or(format!("missing --{k}"))
    }
    fn num(&self, k: &str, d: u64) -> Result<u64, String> {
        self.one(k).map_or(Ok(d), |s| s.parse().map_err(|_| format!("--{k} must be a whole number")))
    }
}

fn run(v: Vec<String>) -> Result<(), String> {
    if v.len() == 2 && v[1] == "--help" { print!("{USAGE}"); return Ok(()); }
    match v.first().map(|s| s.as_str()) {
        Some("compare") => {
            let a = parse(&v[1..], &["a", "b", "out"])?;
            if a.pos.is_empty() {
                return Err("compare needs run directories".into());
            }
            let runs = a.pos.iter().map(|d| store::read_summary(&PathBuf::from(d))).collect::<Result<Vec<_>, _>>()?;
            let r = compare::compare(&runs, a.req("a")?, a.req("b")?)?;
            print!("{}", compare::render(&r));
            if let Some(out) = a.one("out") {
                let json = serde_json::to_string_pretty(&r).unwrap();
                std::fs::OpenOptions::new().write(true).create_new(true).open(out).and_then(|mut f| std::io::Write::write_all(&mut f, json.as_bytes())).map_err(|e| format!("{out}: {e}"))?;
            }
            Ok(())
        }
        Some("analyze") => {
            let a = parse(&v[1..], &["out"])?;
            if a.pos.len() != 1 { return Err("analyze needs exactly one RUN_DIR".into()); }
            let src = &a.pos[0];
            let out = PathBuf::from(a.req("out")?);
            let (meta, samples, sampler) = store::read_run(&PathBuf::from(src))?;
            store::create_new_dir(&out)?;
            store::write_meta(&out, &meta)?;
            let mut from = std::fs::File::open(PathBuf::from(src).join("samples.jsonl")).map_err(|e| e.to_string())?;
            let mut to = std::fs::OpenOptions::new().write(true).create_new(true).open(out.join("samples.jsonl")).map_err(|e| e.to_string())?;
            std::io::copy(&mut from, &mut to).map_err(|e| e.to_string())?;
            let s = store::write_reports(&out, meta, &samples, sampler)?;
            print!("{}", fob_perf::analyze::render(&s));
            Ok(())
        }
        Some("smoke") => {
            let a = parse(&v[1..], &["pid"])?;
            if !a.pos.is_empty() { return Err("smoke accepts no positional arguments".into()); }
            platform::smoke(a.one("pid").map(|p| p.parse::<u32>().map_err(|_| "--pid must be a number".to_string())).transpose()?)
        }
        Some("sample") => {
            let a = parse(&v[1..], &["out", "label", "app-version", "mode", "refresh-ms", "helper", "settle-s", "steady-s", "interval-ms", "setting", "pid"])?;
            if !a.pos.is_empty() { return Err("sample accepts no positional arguments".into()); }
            let mode = a.req("mode")?;
            if !MODES.contains(&mode) {
                return Err(format!("--mode must be one of {}", MODES.join(", ")));
            }
            let label = a.req("label")?;
            if label.is_empty() || !label.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.') {
                return Err("--label may use letters, digits, - _ .".into());
            }
            let interval = a.num("interval-ms", 1000)?;
            if !(250..=10_000).contains(&interval) {
                return Err("--interval-ms must be 250–10000".into());
            }
            let refresh: u32 = a.req("refresh-ms")?.parse().map_err(|_| "--refresh-ms must be a whole number")?;
            if refresh == 0 { return Err("--refresh-ms must be positive".into()); }
            let settings = a.flags.get("setting").cloned().unwrap_or_default();
            let mut setting_keys = std::collections::HashSet::new();
            for s in &settings {
                let (k, value) = s.split_once('=').ok_or("--setting must look like KEY=VALUE")?;
                if k.trim().is_empty() || value.trim().is_empty() || !setting_keys.insert(k) {
                    return Err("--setting needs nonempty unique KEY=VALUE".into());
                }
            }
            let cfg = platform::Config {
                out: PathBuf::from(a.req("out")?),
                label: label.into(),
                app_version: a.req("app-version")?.into(),
                mode: mode.into(),
                refresh_ms: refresh,
                helper: a.req("helper")?.into(),
                settle_s: a.num("settle-s", 120)?,
                steady_s: a.num("steady-s", 600)?,
                interval_ms: interval,
                settings,
                pid: a.one("pid").map(|p| p.parse::<u32>().map_err(|_| "--pid must be a number".to_string())).transpose()?,
            };
            if cfg.app_version.trim().is_empty() || cfg.helper.trim().is_empty() { return Err("version and helper state must not be empty".into()); }
            if cfg.settle_s.checked_add(cfg.steady_s).is_none_or(|s| s > 604800) { return Err("total duration must not exceed 7 days".into()); }
            if cfg.steady_s < 10 {
                return Err("--steady-s must be at least 10".into());
            }
            if cfg.out.exists() {
                return Err(format!("{} already exists; choose a new directory", cfg.out.display()));
            }
            platform::sample(cfg)
        }
        Some("-h") | Some("--help") | Some("help") => {
            print!("{USAGE}");
            Ok(())
        }
        _ => Err(format!("unknown command\n\n{USAGE}")),
    }
}

fn main() -> ExitCode {
    match run(std::env::args().skip(1).collect()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::from(2)
        }
    }
}

#[allow(dead_code)]
mod platform {
    use std::path::PathBuf;
    pub struct Config {
        pub out: PathBuf,
        pub label: String,
        pub app_version: String,
        pub mode: String,
        pub refresh_ms: u32,
        pub helper: String,
        pub settle_s: u64,
        pub steady_s: u64,
        pub interval_ms: u64,
        pub settings: Vec<String>,
        pub pid: Option<u32>,
    }

    #[cfg(not(windows))]
    pub fn smoke(_: Option<u32>) -> Result<(), String> {
        Err("smoke and sample run only on Windows".into())
    }
    #[cfg(not(windows))]
    pub fn sample(_: Config) -> Result<(), String> {
        Err("smoke and sample run only on Windows".into())
    }

    #[cfg(windows)]
    pub use super::winrun::{sample, smoke};
}

#[cfg(windows)]
mod winrun {
    use super::platform::Config;
    use fob_perf::collect::{self, Read, Reader};
    use fob_perf::model::*;
    use fob_perf::{store, tree, win};
    use sha2::{Digest, Sha256};
    use std::io::Write;
    use std::time::{Duration, Instant};

    struct R<'a>(&'a mut win::Handles, &'a Instant);
    impl Reader for R<'_> {
        fn create_time(&mut self, pid: u32) -> Result<u64, String> {
            self.0.create_time(pid)
        }
        fn read(&mut self, pid: u32) -> Result<Read, String> {
            self.0.metrics(pid, self.1).map(|m| Read { create: m.create, cpu: m.cpu, cpu_t_ns: Some(m.cpu_t_ns), private_bytes: m.private_bytes, working_set: m.working_set, memory_error: m.memory_error })
        }
    }

    fn pick_root(pid: Option<u32>, h: &mut win::Handles) -> Result<Identity, String> {
        let entries = win::list()?;
        let roots = tree::find_roots(&entries, "fob.exe");
        let pid = match (pid, roots.as_slice()) {
            (Some(p), _) if roots.contains(&p) => p,
            (Some(p), _) => return Err(format!("pid {p} is not a running top-level fob.exe; found {roots:?}")),
            (None, [p]) => *p,
            (None, []) => return Err("no running fob.exe found; start Fob first".into()),
            (None, many) => return Err(format!("several fob.exe running {many:?}; close extra ones or pass --pid")),
        };
        let create = h.create_time(pid)?;
        let path = h.image_path(pid).ok_or("cannot read Fob executable path")?;
        if !path.rsplit('\\').next().is_some_and(|n| n.eq_ignore_ascii_case("fob.exe")) {
            return Err("selected PID image is not fob.exe".into());
        }
        Ok(Identity { pid, create })
    }

    pub fn smoke(pid: Option<u32>) -> Result<(), String> {
        let mut h = win::Handles::default();
        let root = pick_root(pid, &mut h)?;
        let (cores, groups) = win::cores()?;
        let clock = Instant::now();
        let (s, _) = collect::sample(0, 0, win::now_filetime(), root, win::list(), &mut R(&mut h, &clock));
        println!("Fob pid {}，逻辑处理器 {cores}（{groups} 组）", root.pid);
        println!("范围：{SCOPE}");
        for o in &s.procs {
            let mb = |v: Option<u64>| v.map_or("不可读".into(), |b| format!("{:.1} MB", b as f64 / 1048576.0));
            println!("  计入 {:?} {} pid {}  私有提交 {}  工作集 {}{}", o.role, o.name, o.id.pid, mb(o.private_bytes), mb(o.working_set), o.error.as_ref().map_or(String::new(), |e| format!("  错误：{e}")));
        }
        for x in &s.excluded {
            println!("  排除 {} pid {}（{}）", x.name, x.pid, x.reason);
        }
        let ready = collect::smoke_ready(&s);
        if let Some(e) = &s.discovery_error { eprintln!("发现失败：{e}"); }
        println!("根进程状态：{:?}", s.root);
        println!("{}", if ready { "可以开始采样" } else { "有进程读不到，先解决再采样" });
        if ready { Ok(()) } else { Err("discovery, root or process metrics are incomplete".into()) }
    }

    fn sha(path: &str) -> Option<(String, u64)> {
        use std::io::Read;
        let mut file = std::fs::File::open(path).ok()?;
        let mut digest = Sha256::new();
        let mut buf = [0u8; 65536];
        let mut size = 0;
        loop {
            let n = file.read(&mut buf).ok()?;
            if n == 0 { break; }
            digest.update(&buf[..n]);
            size += n as u64;
        }
        Some((format!("{:x}", digest.finalize()), size))
    }

    pub fn sample(c: Config) -> Result<(), String> {
        let mut h = win::Handles::default();
        let root = pick_root(c.pid, &mut h)?;
        let (cores, groups) = win::cores()?;
        let path = h.image_path(root.pid);
        let hash = path.as_deref().and_then(sha);
        let wall = win::now_filetime();
        let machine = {
            let computer = std::env::var("COMPUTERNAME").map_err(|_| "machine name unavailable for fingerprint")?;
            let processor = std::env::var("PROCESSOR_IDENTIFIER").map_err(|_| "CPU identifier unavailable for fingerprint")?;
            if computer.is_empty() || processor.is_empty() { return Err("machine fingerprint source is empty".into()); }
            let raw = format!("{computer}|{processor}|{cores}");
            format!("{:x}", Sha256::digest(raw.as_bytes()))[..16].to_string()
        };
        // FILETIME -> unix seconds for a sortable UTC stamp.
        let unix = (wall / 10_000_000).saturating_sub(11_644_473_600);
        let mut meta = RunMeta {
            schema: SCHEMA,
            run_id: format!("{}-{}-{unix}", c.label, c.mode),
            tool_version: env!("CARGO_PKG_VERSION").into(),
            label: c.label,
            app_version: c.app_version,
            mode: c.mode,
            refresh_ms: c.refresh_ms,
            helper: c.helper,
            settings: c.settings,
            interval_ms: c.interval_ms,
            settle_s: c.settle_s,
            steady_s: c.steady_s,
            logical_cores: cores,
            processor_groups: groups,
            machine,
            started_utc: format!("unix:{unix:012}"),
            root,
            root_exe_name: path.as_deref().and_then(|p| p.rsplit('\\').next()).unwrap_or("fob.exe").into(),
            root_exe_sha256: hash.as_ref().map(|h| h.0.clone()),
            root_exe_bytes: hash.as_ref().map(|h| h.1),
            root_age_at_attach_s: wall.saturating_sub(root.create) as f64 / 1e7,
            scope: SCOPE.into(),
        };
        store::create_new_dir(&c.out)?;
        let mut raw = std::fs::OpenOptions::new().write(true).create_new(true).open(c.out.join("samples.jsonl")).map_err(|e| e.to_string())?;
        let total = Duration::from_secs(meta.settle_s + meta.steady_s);
        let step = Duration::from_millis(meta.interval_ms);
        let t0 = Instant::now();
        let mut samples = vec![];
        let mut seq = 0u64;
        eprintln!("采样 {} 秒（等待 {} 秒 + 稳定段 {} 秒），按 Ctrl+C 会中止且结果无效", total.as_secs(), meta.settle_s, meta.steady_s);
        loop {
            let t = t0.elapsed();
            let (s, keep) = collect::sample(seq, t.as_nanos() as u64, win::now_filetime(), root, win::list(), &mut R(&mut h, &t0));
            h.retain(&keep);
            writeln!(raw, "{}", serde_json::to_string(&s).unwrap()).map_err(|e| e.to_string())?;
            let stop = s.root != RootState::Alive;
            samples.push(s);
            if stop {
                eprintln!("Fob 已退出或读不到，停止采样");
                break;
            }
            if t >= total {
                break;
            }
            seq += 1;
            let elapsed = t0.elapsed();
            let grid = step * ((elapsed.as_millis() / meta.interval_ms as u128 + 1) as u32);
            let settle = Duration::from_secs(meta.settle_s);
            let next = if elapsed < settle { grid.min(settle).min(total) } else { grid.min(total) };
            if let Some(d) = next.checked_sub(t0.elapsed()) {
                std::thread::sleep(d);
            }
        }
        raw.flush().map_err(|e| format!("samples.jsonl flush: {e}"))?;
        if path.as_deref().and_then(sha) != hash {
            meta.root_exe_sha256 = None;
            meta.root_exe_bytes = None;
        }
        store::write_meta(&c.out, &meta)?;
        let usage = win::self_usage();
        let s = store::write_reports(&c.out, meta, &samples, usage)?;
        print!("{}", fob_perf::analyze::render(&s));
        if s.quality != "complete" { return Err(format!("sampling {}: reports saved, not eligible for A/B comparison", s.quality)); }
        Ok(())
    }
}
