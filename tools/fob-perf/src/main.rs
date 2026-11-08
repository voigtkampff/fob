use fob_perf::{compare, model::*, store};
use std::collections::HashMap;
use std::path::PathBuf;
use std::process::ExitCode;

const USAGE: &str = "fob-perf: read-only Fob CPU/memory sampler

  fob-perf smoke [--pid N]
      Find the running Fob and list measured and excluded processes. Writes nothing
  fob-perf sample --out DIR --label A|B --app-version V --mode tray|mini|statusbar|processes
                  --refresh-ms N --helper STATE [--settle-s 120] [--steady-s 600]
                  [--interval-ms 1000] [--setting KEY=VALUE]... [--pid N]
      Attach to the running Fob, sample, then write a new result directory DIR
  fob-perf analyze RUN_DIR --out NEW_DIR
      Recompute reports from a run's raw data into a new directory
  fob-perf compare --a LABEL --b LABEL [--out FILE] RUN_DIR...
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
            let src = a.pos.first().ok_or("analyze needs RUN_DIR")?;
            let out = PathBuf::from(a.req("out")?);
            let (meta, samples, sampler) = store::read_run(&PathBuf::from(src))?;
            store::create_new_dir(&out)?;
            store::write_meta(&out, &meta)?;
            std::fs::copy(PathBuf::from(src).join("samples.jsonl"), out.join("samples.jsonl")).map_err(|e| e.to_string())?;
            let s = store::write_reports(&out, meta, &samples, sampler)?;
            print!("{}", fob_perf::analyze::render(&s));
            Ok(())
        }
        Some("smoke") => {
            let a = parse(&v[1..], &["pid"])?;
            platform::smoke(a.one("pid").map(|p| p.parse::<u32>().map_err(|_| "--pid must be a number".to_string())).transpose()?)
        }
        Some("sample") => {
            let a = parse(&v[1..], &["out", "label", "app-version", "mode", "refresh-ms", "helper", "settle-s", "steady-s", "interval-ms", "setting", "pid"])?;
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
            let settings = a.flags.get("setting").cloned().unwrap_or_default();
            if settings.iter().any(|s| !s.contains('=')) {
                return Err("--setting must look like KEY=VALUE".into());
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

    struct R<'a>(&'a mut win::Handles);
    impl Reader for R<'_> {
        fn create_time(&mut self, pid: u32) -> Result<u64, String> {
            self.0.create_time(pid)
        }
        fn read(&mut self, pid: u32) -> Result<Read, String> {
            self.0.metrics(pid).map(|m| Read { create: m.create, cpu: m.cpu, private_bytes: m.private_bytes, working_set: m.working_set })
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
        Ok(Identity { pid, create: h.create_time(pid)? })
    }

    pub fn smoke(pid: Option<u32>) -> Result<(), String> {
        let mut h = win::Handles::default();
        let root = pick_root(pid, &mut h)?;
        let (cores, groups) = win::cores();
        let (s, _) = collect::sample(0, 0, win::now_filetime(), root, win::list(), &mut R(&mut h));
        println!("Fob pid {}，逻辑处理器 {cores}（{groups} 组）", root.pid);
        println!("范围：{SCOPE}");
        for o in &s.procs {
            let mb = |v: Option<u64>| v.map_or("不可读".into(), |b| format!("{:.1} MB", b as f64 / 1048576.0));
            println!("  计入 {:?} {} pid {}  私有提交 {}  工作集 {}{}", o.role, o.name, o.id.pid, mb(o.private_bytes), mb(o.working_set), o.error.as_ref().map_or(String::new(), |e| format!("  错误：{e}")));
        }
        for x in &s.excluded {
            println!("  排除 {} pid {}（{}）", x.name, x.pid, x.reason);
        }
        let bad = s.procs.iter().filter(|o| o.error.is_some()).count();
        println!("{}", if bad == 0 { "可以开始采样" } else { "有进程读不到，先解决再采样" });
        if bad == 0 { Ok(()) } else { Err(format!("{bad} processes unreadable")) }
    }

    fn sha(path: &str) -> Option<(String, u64)> {
        let data = std::fs::read(path).ok()?;
        Some((format!("{:x}", Sha256::digest(&data)), data.len() as u64))
    }

    pub fn sample(c: Config) -> Result<(), String> {
        let mut h = win::Handles::default();
        let root = pick_root(c.pid, &mut h)?;
        let (cores, groups) = win::cores();
        let path = h.image_path(root.pid);
        let hash = path.as_deref().and_then(sha);
        let wall = win::now_filetime();
        let machine = {
            let raw = format!("{}|{}|{cores}", std::env::var("COMPUTERNAME").unwrap_or_default(), std::env::var("PROCESSOR_IDENTIFIER").unwrap_or_default());
            format!("{:x}", Sha256::digest(raw.as_bytes()))[..16].to_string()
        };
        // FILETIME -> unix seconds for a sortable UTC stamp.
        let unix = (wall / 10_000_000).saturating_sub(11_644_473_600);
        let meta = RunMeta {
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
            root_exe_bytes: hash.map(|h| h.1),
            root_age_at_attach_s: wall.saturating_sub(root.create) as f64 / 1e7,
            scope: SCOPE.into(),
        };
        store::create_new_dir(&c.out)?;
        store::write_meta(&c.out, &meta)?;
        let mut raw = std::fs::OpenOptions::new().write(true).create_new(true).open(c.out.join("samples.jsonl")).map_err(|e| e.to_string())?;
        let total = Duration::from_secs(meta.settle_s + meta.steady_s);
        let step = Duration::from_millis(meta.interval_ms);
        let t0 = Instant::now();
        let mut samples = vec![];
        let mut seq = 0u64;
        eprintln!("采样 {} 秒（等待 {} 秒 + 稳定段 {} 秒），按 Ctrl+C 会中止且结果无效", total.as_secs(), meta.settle_s, meta.steady_s);
        loop {
            let t = t0.elapsed();
            let (s, keep) = collect::sample(seq, t.as_nanos() as u64, win::now_filetime(), root, win::list(), &mut R(&mut h));
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
            let next = step * seq as u32;
            if let Some(d) = next.checked_sub(t0.elapsed()) {
                std::thread::sleep(d);
            }
        }
        raw.flush().ok();
        let usage = win::self_usage();
        if let Some(u) = usage {
            std::fs::write(c.out.join("sampler.json"), serde_json::to_string(&u).unwrap()).ok();
        }
        let s = store::write_reports(&c.out, meta, &samples, usage)?;
        print!("{}", fob_perf::analyze::render(&s));
        Ok(())
    }
}
