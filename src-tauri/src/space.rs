//! Fob: disk space map, v1 — a plain, read-only directory walk.
//!
//! A scan runs only when asked (`space_scan`), on its own thread at
//! background CPU and I/O priority, so the UI and the sampler never wait on
//! it. Junctions, symlinks and other directory reparse points are not
//! followed (no loops, no double counting); unreadable folders are skipped
//! and counted. Small items are folded into their folder's `other` total so
//! the tree stays small enough to cache and to send to the page in one piece.
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Instant, SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Manager};

/// Items a folder keeps individually beyond those that are at least
/// 1/KEEP_SHARE of it; the rest go into its `other` total.
const KEEP_MAX: usize = 200;
const KEEP_SHARE: u64 = 2000;
/// Items smaller than this go into `other`.
const KEEP_MIN_BYTES: u64 = 256 * 1024;

fn is_zero(v: &u64) -> bool {
    *v == 0
}
fn is_false(v: &bool) -> bool {
    !*v
}

/// A folder or file. Short field names keep the cache and the IPC payload small.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct Node {
    #[serde(rename = "n")]
    pub name: String,
    /// Allocated bytes, this item and everything under it.
    #[serde(rename = "s")]
    pub size: u64,
    /// Files under it (1 for a file).
    #[serde(rename = "f")]
    pub files: u64,
    #[serde(rename = "d", default, skip_serializing_if = "is_false")]
    pub dir: bool,
    /// Kept items, largest first.
    #[serde(rename = "c", default, skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<Node>,
    /// Bytes of the items folded away (too small or too many).
    #[serde(rename = "o", default, skip_serializing_if = "is_zero")]
    pub other: u64,
    /// How many items were folded away.
    #[serde(rename = "oc", default, skip_serializing_if = "is_zero")]
    pub other_count: u64,
    /// The folder could not be read (access denied, or vanished).
    #[serde(rename = "x", default, skip_serializing_if = "is_false")]
    pub denied: bool,
}

/// Live counters of a running scan, shared with `space_status`.
#[derive(Default)]
pub struct Progress {
    pub files: AtomicU64,
    pub bytes: AtomicU64,
    pub skipped: AtomicU64,
    pub cancel: AtomicBool,
    pub current: Mutex<String>,
}

/// Keeps the largest items of a folder and folds the rest into `other`:
/// items under KEEP_MIN_BYTES, and past the first KEEP_MAX those under
/// 1/KEEP_SHARE of the folder (so many big files are never hidden).
fn fold(node: &mut Node, mut kids: Vec<Node>) {
    kids.sort_by(|a, b| b.size.cmp(&a.size).then_with(|| a.name.cmp(&b.name)));
    let total: u64 = kids.iter().map(|k| k.size).sum();
    for (i, kid) in kids.into_iter().enumerate() {
        let keep = i < KEEP_MAX || kid.size.saturating_mul(KEEP_SHARE) >= total;
        if keep && kid.size >= KEEP_MIN_BYTES {
            node.children.push(kid);
        } else {
            node.other += kid.size;
            node.other_count += 1;
        }
    }
}

struct Walker<'a> {
    progress: &'a Progress,
    /// Allocation unit; file sizes round up to it (Windows).
    #[cfg_attr(not(windows), allow(dead_code))]
    cluster: u64,
    /// The scanned volume; other file systems mounted inside are not entered.
    #[cfg(unix)]
    device: u64,
    /// Hard-linked files already counted (device, inode).
    #[cfg(unix)]
    seen: std::collections::HashSet<(u64, u64)>,
}

impl Walker<'_> {
    fn cancelled(&self) -> bool {
        self.progress.cancel.load(Ordering::Relaxed)
    }

    /// Links are never followed: symlinks, junctions, mount points and any
    /// other directory reparse point (Windows), symlinks (Unix).
    fn is_link(meta: &fs::Metadata) -> bool {
        if meta.file_type().is_symlink() {
            return true;
        }
        #[cfg(windows)]
        {
            use std::os::windows::fs::MetadataExt;
            const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
            if meta.is_dir() && meta.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
                return true;
            }
        }
        false
    }

    /// Bytes the file takes on disk.
    fn allocated(&mut self, _path: &Path, meta: &fs::Metadata) -> u64 {
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            if meta.nlink() > 1 && !self.seen.insert((meta.dev(), meta.ino())) {
                return 0;
            }
            meta.blocks() * 512
        }
        #[cfg(windows)]
        {
            use std::os::windows::fs::MetadataExt;
            // Compressed, sparse and cloud placeholder files: ask the file
            // system; others take their size rounded up to whole clusters.
            const SPECIAL: u32 = 0x800 | 0x200 | 0x400 | 0x1000 | 0x40_0000;
            let size = if meta.file_attributes() & SPECIAL != 0 {
                compressed_size(_path).unwrap_or(meta.file_size())
            } else {
                meta.file_size()
            };
            if self.cluster > 1 {
                size.div_ceil(self.cluster) * self.cluster
            } else {
                size
            }
        }
        #[cfg(not(any(unix, windows)))]
        meta.len()
    }

    /// Walks one folder; `None` once the scan is cancelled.
    fn folder(&mut self, path: &Path, name: String) -> Option<Node> {
        if self.cancelled() {
            return None;
        }
        if let Ok(mut current) = self.progress.current.lock() {
            current.clear();
            current.push_str(&path.to_string_lossy());
        }
        let mut node = Node {
            name,
            dir: true,
            ..Node::default()
        };
        let entries = match fs::read_dir(path) {
            Ok(entries) => entries,
            Err(_) => {
                self.progress.skipped.fetch_add(1, Ordering::Relaxed);
                node.denied = true;
                return Some(node);
            }
        };
        let mut kids = Vec::new();
        for entry in entries.flatten() {
            if self.cancelled() {
                return None;
            }
            // DirEntry::metadata does not follow links (Unix: lstat;
            // Windows: the directory listing's own data).
            let Ok(meta) = entry.metadata() else { continue };
            if Self::is_link(&meta) {
                continue;
            }
            let name = entry.file_name().to_string_lossy().into_owned();
            if meta.is_dir() {
                #[cfg(unix)]
                {
                    use std::os::unix::fs::MetadataExt;
                    if meta.dev() != self.device {
                        continue;
                    }
                }
                kids.push(self.folder(&entry.path(), name)?);
            } else if meta.is_file() {
                let size = self.allocated(&entry.path(), &meta);
                self.progress.files.fetch_add(1, Ordering::Relaxed);
                self.progress.bytes.fetch_add(size, Ordering::Relaxed);
                kids.push(Node {
                    name,
                    size,
                    files: 1,
                    ..Node::default()
                });
            }
        }
        node.size = kids.iter().map(|k| k.size).sum();
        node.files = kids.iter().map(|k| k.files).sum();
        fold(&mut node, kids);
        Some(node)
    }
}

/// Walks `root`; `None` if `progress.cancel` was set before it finished.
pub fn walk(root: &Path, progress: &Progress) -> Option<Node> {
    let meta = fs::metadata(root).ok()?;
    let mut walker = Walker {
        progress,
        cluster: cluster_size(root),
        #[cfg(unix)]
        device: {
            use std::os::unix::fs::MetadataExt;
            meta.dev()
        },
        #[cfg(unix)]
        seen: Default::default(),
    };
    let _ = &meta;
    walker.folder(root, root.to_string_lossy().into_owned())
}

#[cfg(windows)]
fn wide(path: &Path) -> windows::core::HSTRING {
    windows::core::HSTRING::from(path.as_os_str())
}
#[cfg(windows)]
fn compressed_size(path: &Path) -> Option<u64> {
    use windows::Win32::Storage::FileSystem::GetCompressedFileSizeW;
    let mut high = 0u32;
    let low = unsafe { GetCompressedFileSizeW(&wide(path), Some(&mut high)) };
    (low != u32::MAX || high != 0).then(|| (u64::from(high) << 32) | u64::from(low))
}
fn cluster_size(_root: &Path) -> u64 {
    #[cfg(windows)]
    {
        use windows::Win32::Storage::FileSystem::GetDiskFreeSpaceW;
        let (mut sectors, mut bytes) = (0u32, 0u32);
        if unsafe {
            GetDiskFreeSpaceW(
                &wide(_root),
                Some(&mut sectors),
                Some(&mut bytes),
                None,
                None,
            )
        }
        .is_ok()
        {
            return u64::from(sectors) * u64::from(bytes);
        }
    }
    1
}

/// Lowers this thread's CPU priority and, on Windows, its I/O and memory
/// priority as well (THREAD_MODE_BACKGROUND_BEGIN).
fn background_priority() {
    #[cfg(windows)]
    unsafe {
        use windows::Win32::System::Threading::{
            GetCurrentThread, SetThreadPriority, THREAD_MODE_BACKGROUND_BEGIN,
        };
        let _ = SetThreadPriority(GetCurrentThread(), THREAD_MODE_BACKGROUND_BEGIN);
    }
    #[cfg(target_os = "linux")]
    {
        // Linux nice values are per thread.
        extern "C" {
            fn nice(inc: i32) -> i32;
        }
        unsafe {
            nice(19);
        }
    }
}

/// A finished scan, as shown and cached.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Scan {
    pub root: String,
    /// Path separator for building full paths on the page.
    pub sep: String,
    /// Unix seconds.
    pub scanned_at: u64,
    pub elapsed_ms: u64,
    pub files: u64,
    pub skipped: u64,
    pub tree: Node,
}

/// A drive the page offers.
#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct Drive {
    pub root: String,
    pub total: u64,
    pub free: u64,
}

/// Fixed drives (Windows). Elsewhere (development) the folders listed in
/// WINPERF_SPACE_ROOTS, separated by ':', or the home folder.
pub fn drives() -> Vec<Drive> {
    #[cfg(windows)]
    {
        use windows::Win32::Storage::FileSystem::{
            GetDiskFreeSpaceExW, GetDriveTypeW, GetLogicalDrives,
        };
        const DRIVE_FIXED: u32 = 3;
        let mask = unsafe { GetLogicalDrives() };
        return (0..26u8)
            .filter(|i| mask & (1 << i) != 0)
            .map(|i| format!("{}:\\", (b'A' + i) as char))
            .filter(|root| unsafe { GetDriveTypeW(&windows::core::HSTRING::from(root.as_str())) } == DRIVE_FIXED)
            .map(|root| {
                let (mut free, mut total) = (0u64, 0u64);
                let _ = unsafe {
                    GetDiskFreeSpaceExW(
                        &windows::core::HSTRING::from(root.as_str()),
                        None,
                        Some(&mut total),
                        Some(&mut free),
                    )
                };
                Drive { root, total, free }
            })
            .collect();
    }
    #[cfg(not(windows))]
    {
        let roots = std::env::var("WINPERF_SPACE_ROOTS")
            .ok()
            .or_else(|| std::env::var("HOME").ok())
            .unwrap_or_else(|| "/".into());
        roots
            .split(':')
            .filter(|r| !r.is_empty())
            .map(|root| Drive {
                root: root.to_string(),
                total: 0,
                free: 0,
            })
            .collect()
    }
}

fn cache_file(dir: &Path, root: &str) -> PathBuf {
    let key: String = root
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect();
    dir.join(format!("space-{}.json", key.trim_matches('_')))
}
pub fn write_cache(dir: &Path, scan: &Scan) -> std::io::Result<()> {
    fs::create_dir_all(dir)?;
    let path = cache_file(dir, &scan.root);
    let temp = path.with_extension("json.tmp");
    fs::write(&temp, serde_json::to_vec(scan)?)?;
    fs::rename(temp, path)
}
pub fn read_cache(dir: &Path, root: &str) -> Option<Scan> {
    let scan: Scan = serde_json::from_slice(&fs::read(cache_file(dir, root)).ok()?).ok()?;
    (scan.root == root).then_some(scan)
}

struct Running {
    root: String,
    progress: Arc<Progress>,
    started: Instant,
}
#[derive(Default)]
pub struct SpaceState {
    running: Mutex<Option<Running>>,
    results: Mutex<HashMap<String, Arc<Scan>>>,
    /// The last scan that ended: (root, stopped before it finished).
    ended: Mutex<Option<(String, bool)>>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    pub root: Option<String>,
    pub files: u64,
    pub bytes: u64,
    pub skipped: u64,
    pub current: String,
    pub elapsed_ms: u64,
    /// The last scan that ended and whether it was stopped.
    pub ended: Option<(String, bool)>,
}

fn cache_dir(app: &AppHandle) -> Result<PathBuf, String> {
    app.path().app_data_dir().map_err(|e| e.to_string())
}

/// Starts a scan of one of `drives()` (stopping any scan already running).
pub fn start(app: AppHandle, root: String) -> Result<(), String> {
    if !drives().iter().any(|d| d.root == root) {
        return Err("Not a drive Fob can scan".into());
    }
    let state = app.state::<SpaceState>();
    let progress = Arc::new(Progress::default());
    {
        let mut running = state.running.lock().map_err(|e| e.to_string())?;
        if let Some(old) = running.take() {
            old.progress.cancel.store(true, Ordering::Relaxed);
        }
        *running = Some(Running {
            root: root.clone(),
            progress: progress.clone(),
            started: Instant::now(),
        });
    }
    let dir = cache_dir(&app)?;
    std::thread::Builder::new()
        .name("fob-space".into())
        .stack_size(16 << 20) // deep folder trees recurse
        .spawn(move || {
            background_priority();
            let started = Instant::now();
            let tree = walk(Path::new(&root), &progress);
            let state = app.state::<SpaceState>();
            if let Ok(mut running) = state.running.lock() {
                if running
                    .as_ref()
                    .is_some_and(|r| Arc::ptr_eq(&r.progress, &progress))
                {
                    *running = None;
                }
            }
            let stopped = tree.is_none();
            if let Some(tree) = tree {
                let scan = Scan {
                    sep: std::path::MAIN_SEPARATOR.to_string(),
                    scanned_at: SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .map(|d| d.as_secs())
                        .unwrap_or(0),
                    elapsed_ms: started.elapsed().as_millis() as u64,
                    files: progress.files.load(Ordering::Relaxed),
                    skipped: progress.skipped.load(Ordering::Relaxed),
                    root: root.clone(),
                    tree,
                };
                let _ = write_cache(&dir, &scan);
                if let Ok(mut results) = state.results.lock() {
                    results.insert(root.clone(), Arc::new(scan));
                }
            }
            if let Ok(mut ended) = state.ended.lock() {
                *ended = Some((root, stopped));
            };
        })
        .map(|_| ())
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn space_drives() -> Vec<Drive> {
    drives()
}
#[tauri::command]
pub async fn space_scan(app: AppHandle, root: String) -> Result<(), String> {
    start(app, root)
}
#[tauri::command]
pub async fn space_stop(app: AppHandle) {
    if let Ok(running) = app.state::<SpaceState>().running.lock() {
        if let Some(r) = running.as_ref() {
            r.progress.cancel.store(true, Ordering::Relaxed);
        }
    }
}
#[tauri::command]
pub async fn space_status(app: AppHandle) -> Status {
    let state = app.state::<SpaceState>();
    let ended = state.ended.lock().ok().and_then(|e| e.clone());
    let running = state.running.lock();
    let status = match running.as_ref().ok().and_then(|r| r.as_ref()) {
        Some(r) => Status {
            root: Some(r.root.clone()),
            files: r.progress.files.load(Ordering::Relaxed),
            bytes: r.progress.bytes.load(Ordering::Relaxed),
            skipped: r.progress.skipped.load(Ordering::Relaxed),
            current: r
                .progress
                .current
                .lock()
                .map(|c| c.clone())
                .unwrap_or_default(),
            elapsed_ms: r.started.elapsed().as_millis() as u64,
            ended,
        },
        None => Status {
            root: None,
            files: 0,
            bytes: 0,
            skipped: 0,
            current: String::new(),
            elapsed_ms: 0,
            ended,
        },
    };
    status
}
/// The last scan of `root`: this session's, else the cached one.
#[tauri::command]
pub async fn space_result(app: AppHandle, root: String) -> Result<Option<Arc<Scan>>, String> {
    let state = app.state::<SpaceState>();
    if let Some(scan) = state.results.lock().map_err(|e| e.to_string())?.get(&root) {
        return Ok(Some(scan.clone()));
    }
    let Some(scan) = read_cache(&cache_dir(&app)?, &root) else {
        return Ok(None);
    };
    let scan = Arc::new(scan);
    if let Ok(mut results) = state.results.lock() {
        results.insert(root, scan.clone());
    }
    Ok(Some(scan))
}
/// Shows `path` selected in File Explorer (explorer.exe /select, through
/// ShellExecute; no shell in between).
#[tauri::command]
pub async fn space_reveal(path: String) -> Result<(), String> {
    let target = Path::new(&path);
    if path.contains('"') || !target.is_absolute() || fs::symlink_metadata(target).is_err() {
        return Err("This item no longer exists".into());
    }
    #[cfg(windows)]
    {
        use windows::core::{w, HSTRING, PCWSTR};
        use windows::Win32::UI::Shell::ShellExecuteW;
        use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
        let params = HSTRING::from(format!("/select,\"{path}\""));
        let result = unsafe {
            ShellExecuteW(
                None,
                w!("open"),
                w!("explorer.exe"),
                &params,
                PCWSTR::null(),
                SW_SHOWNORMAL,
            )
        };
        if result.0 as isize <= 32 {
            return Err("File Explorer did not open".into());
        }
        Ok(())
    }
    #[cfg(not(windows))]
    Err("Open in Explorer is available on Windows".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("fob-space-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }
    fn file(path: &Path, bytes: usize) {
        fs::write(path, vec![7u8; bytes]).unwrap();
    }
    fn child<'a>(node: &'a Node, name: &str) -> &'a Node {
        node.children.iter().find(|c| c.name == name).unwrap()
    }

    #[test]
    fn many_big_files_are_all_kept() {
        let mut node = Node::default();
        let kids = (0..1000)
            .map(|i| Node {
                name: format!("{i:04}"),
                size: 80 << 20,
                files: 1,
                ..Node::default()
            })
            .collect();
        fold(&mut node, kids);
        assert_eq!(node.children.len(), 1000);
        assert_eq!(node.other, 0);
    }

    #[test]
    fn small_and_surplus_items_fold_into_other() {
        let mut node = Node::default();
        let mut kids: Vec<Node> = (0..KEEP_MAX + 10)
            .map(|i| Node {
                name: format!("{i:04}"),
                size: KEEP_MIN_BYTES + i as u64,
                files: 1,
                ..Node::default()
            })
            .collect();
        kids.push(Node {
            name: "tiny".into(),
            size: 10,
            files: 1,
            ..Node::default()
        });
        // One huge item: the others are now under 1/KEEP_SHARE of the folder.
        kids.push(Node {
            name: "huge".into(),
            size: 10 << 30,
            files: 1,
            ..Node::default()
        });
        let total: u64 = kids.iter().map(|k| k.size).sum();
        fold(&mut node, kids);
        assert_eq!(node.children.len(), KEEP_MAX);
        assert_eq!(node.other_count, 12);
        assert_eq!(node.children[0].name, "huge");
        assert!(node.children.windows(2).all(|w| w[0].size >= w[1].size));
        assert_eq!(
            node.children.iter().map(|c| c.size).sum::<u64>() + node.other,
            total
        );
        assert!(node.children.iter().all(|c| c.name != "tiny"));
    }

    #[cfg(unix)]
    #[test]
    fn the_walk_sums_sizes_and_never_follows_links() {
        let root = temp("links");
        fs::create_dir_all(root.join("big/inner")).unwrap();
        file(&root.join("big/inner/a.bin"), 600_000);
        file(&root.join("big/b.bin"), 400_000);
        file(&root.join("small.txt"), 100);
        // A loop back to the root, a link to the big folder, a file link,
        // and a hard link (counted once).
        std::os::unix::fs::symlink(&root, root.join("big/loop")).unwrap();
        std::os::unix::fs::symlink(root.join("big"), root.join("big-again")).unwrap();
        std::os::unix::fs::symlink(root.join("big/b.bin"), root.join("b-link")).unwrap();
        fs::hard_link(root.join("big/b.bin"), root.join("b-hard")).unwrap();
        let progress = Progress::default();
        let tree = walk(&root, &progress).unwrap();
        let big = child(&tree, "big");
        assert!(big.dir);
        assert_eq!(big.files, 2);
        assert!(child(big, "inner").size >= 600_000);
        assert!(tree
            .children
            .iter()
            .all(|c| c.name != "big-again" && c.name != "b-link"));
        assert!(big.children.iter().all(|c| c.name != "loop"));
        // a.bin, b.bin, small.txt and the hard link entry (0 extra bytes).
        assert_eq!(tree.files, 4);
        assert_eq!(progress.files.load(Ordering::Relaxed), 4);
        assert_eq!(tree.size, big.size + tree.other + 0);
        assert!(
            tree.size < 1_100_000,
            "hard link counted twice: {}",
            tree.size
        );
        assert_eq!(progress.bytes.load(Ordering::Relaxed), tree.size);
        let _ = fs::remove_dir_all(root);
    }

    #[cfg(unix)]
    #[test]
    fn unreadable_folders_are_skipped_and_counted() {
        use std::os::unix::fs::PermissionsExt;
        let root = temp("denied");
        fs::create_dir_all(root.join("locked")).unwrap();
        file(&root.join("locked/x.bin"), 1000);
        file(&root.join("ok.bin"), 300_000);
        fs::set_permissions(root.join("locked"), fs::Permissions::from_mode(0o000)).unwrap();
        let progress = Progress::default();
        let tree = walk(&root, &progress);
        fs::set_permissions(root.join("locked"), fs::Permissions::from_mode(0o755)).unwrap();
        let tree = tree.unwrap();
        // Root bypasses permissions; the test only means something otherwise.
        if fs::read_dir(Path::new("/root")).is_err() {
            assert_eq!(progress.skipped.load(Ordering::Relaxed), 1);
            assert_eq!(tree.files, 1);
        }
        assert!(child(&tree, "ok.bin").size >= 300_000);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn a_stopped_walk_returns_nothing_and_stops_early() {
        let root = temp("cancel");
        for d in 0..200 {
            let dir = root.join(format!("d{d}"));
            fs::create_dir_all(&dir).unwrap();
            for f in 0..20 {
                file(&dir.join(format!("f{f}")), 10);
            }
        }
        let stopped = Progress::default();
        stopped.cancel.store(true, Ordering::Relaxed);
        assert!(walk(&root, &stopped).is_none());
        assert_eq!(stopped.files.load(Ordering::Relaxed), 0);
        // Stopped part-way: from another thread once files are counted.
        let progress = Arc::new(Progress::default());
        let p = progress.clone();
        let r = root.clone();
        let scan = std::thread::spawn(move || walk(&r, &p));
        while progress.files.load(Ordering::Relaxed) == 0 {
            std::thread::yield_now();
        }
        progress.cancel.store(true, Ordering::Relaxed);
        let result = scan.join().unwrap();
        let counted = progress.files.load(Ordering::Relaxed);
        assert!(result.is_none(), "the walk ignored the stop");
        assert!(counted < 4000, "stopped only after {counted} files");
        // Unstopped, the same tree is complete.
        let full = Progress::default();
        assert_eq!(walk(&root, &full).unwrap().files, 4000);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn a_cached_scan_reads_back_identical() {
        let dir = temp("cache");
        let root = dir.join("data");
        fs::create_dir_all(root.join("sub")).unwrap();
        file(&root.join("sub/a.bin"), 500_000);
        let tree = walk(&root, &Progress::default()).unwrap();
        let scan = Scan {
            root: root.to_string_lossy().into_owned(),
            sep: "/".into(),
            scanned_at: 1_762_000_000,
            elapsed_ms: 12,
            files: 1,
            skipped: 0,
            tree,
        };
        write_cache(&dir.join("cache"), &scan).unwrap();
        assert_eq!(
            read_cache(&dir.join("cache"), &scan.root),
            Some(scan.clone())
        );
        assert_eq!(read_cache(&dir.join("cache"), "/elsewhere"), None);
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn the_scan_thread_runs_at_background_priority() {
        let src = include_str!("space.rs");
        let start = src.find("pub fn start(").unwrap();
        let body = &src[start..src[start..].find("\n}\n").unwrap() + start];
        assert!(body.contains("background_priority();"));
        assert!(src.contains("THREAD_MODE_BACKGROUND_BEGIN"));
    }
}

/// Scan speed on a real folder: WINPERF_SPACE_BENCH=/path cargo test --release
/// space_speed -- --ignored --nocapture
#[cfg(test)]
#[test]
#[ignore]
fn space_speed() {
    let root = std::env::var("WINPERF_SPACE_BENCH").expect("WINPERF_SPACE_BENCH");
    let progress = Progress::default();
    let start = Instant::now();
    let tree = walk(Path::new(&root), &progress).unwrap();
    let secs = start.elapsed().as_secs_f64();
    let files = progress.files.load(Ordering::Relaxed);
    let json = serde_json::to_vec(&tree).unwrap().len();
    println!(
        "{root}: {files} files, {:.1} GB in {secs:.2} s = {:.0} files/s; skipped {}; cache {} KB",
        tree.size as f64 / 1e9,
        files as f64 / secs,
        progress.skipped.load(Ordering::Relaxed),
        json / 1024
    );
}
