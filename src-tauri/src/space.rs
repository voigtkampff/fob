//! Fob: disk space map, v1 — a plain, read-only directory walk.
//!
//! A scan runs only when asked (`space_scan`), on its own thread at
//! background CPU and I/O priority, so the UI and the sampler never wait on
//! it. Junctions, symlinks and other directory reparse points are not
//! followed (no loops, no double counting); unreadable folders are skipped
//! and counted. Small items are folded into their folder's `other` total so
//! the tree stays small enough to cache and to send to the page in one piece.
//!
//! Fob keeps no tree in memory: a finished scan is written to its cache file
//! and dropped, and `space_result` hands the page that file's bytes as they
//! are (a big C:\\ tree is ~90 MB as Rust nodes, ~25 MB as JSON).
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
    node.children.shrink_to_fit();
}

/// One directory entry as listed (links already dropped).
struct Entry {
    name: String,
    dir: bool,
    /// Allocated bytes (files).
    size: u64,
}

struct Walker<'a> {
    progress: &'a Progress,
    /// Allocation unit; file sizes round up to it (Windows).
    #[cfg_attr(not(windows), allow(dead_code))]
    cluster: u64,
    /// Subfolders are walked on the current rayon pool (one per thread).
    parallel: bool,
    /// The scanned volume; other file systems mounted inside are not entered.
    #[cfg(unix)]
    device: u64,
    /// Hard-linked files already counted (device, inode).
    #[cfg(unix)]
    seen: Mutex<std::collections::HashSet<(u64, u64)>>,
}

impl Walker<'_> {
    fn cancelled(&self) -> bool {
        self.progress.cancel.load(Ordering::Relaxed)
    }

    /// The folder's files and subfolders. Links are never followed:
    /// symlinks, junctions, mount points and any other directory reparse
    /// point (Windows), symlinks and other devices' folders (Unix).
    #[cfg(unix)]
    fn list(&self, path: &Path) -> std::io::Result<Vec<Entry>> {
        use std::os::unix::fs::MetadataExt;
        let mut out = Vec::new();
        for entry in fs::read_dir(path)?.flatten() {
            let Ok(meta) = entry.metadata() else { continue };
            if meta.file_type().is_symlink() {
                continue;
            }
            let name = entry.file_name().to_string_lossy().into_owned();
            if meta.is_dir() {
                if meta.dev() == self.device {
                    out.push(Entry {
                        name,
                        dir: true,
                        size: 0,
                    });
                }
            } else if meta.is_file() {
                let first = meta.nlink() <= 1
                    || self
                        .seen
                        .lock()
                        .is_ok_and(|mut s| s.insert((meta.dev(), meta.ino())));
                let size = if first { meta.blocks() * 512 } else { 0 };
                out.push(Entry {
                    name,
                    dir: false,
                    size,
                });
            }
        }
        Ok(out)
    }

    /// FindFirstFileExW with the basic info level and large fetches (fewer
    /// kernel round trips), without the large-fetch flag where a file
    /// system refuses it.
    #[cfg(windows)]
    fn list(&self, path: &Path) -> std::io::Result<Vec<Entry>> {
        use windows::Win32::Foundation::ERROR_INVALID_PARAMETER;
        use windows::Win32::Storage::FileSystem::{
            FindClose, FindExInfoBasic, FindExSearchNameMatch, FindFirstFileExW, FindNextFileW,
            FIND_FIRST_EX_FLAGS, FIND_FIRST_EX_LARGE_FETCH, WIN32_FIND_DATAW,
        };
        const REPARSE: u32 = 0x400;
        const DIRECTORY: u32 = 0x10;
        const NAME_SURROGATE: u32 = 0x2000_0000;
        let pattern = windows::core::HSTRING::from(find_pattern(path).as_os_str());
        let mut data = WIN32_FIND_DATAW::default();
        let mut open = |flags| unsafe {
            FindFirstFileExW(
                &pattern,
                FindExInfoBasic,
                &mut data as *mut _ as *mut _,
                FindExSearchNameMatch,
                None,
                flags,
            )
        };
        let handle = match open(FIND_FIRST_EX_LARGE_FETCH) {
            Ok(h) => h,
            Err(e) if e.code() == ERROR_INVALID_PARAMETER.to_hresult() => {
                open(FIND_FIRST_EX_FLAGS(0)).map_err(|e| std::io::Error::other(e.to_string()))?
            }
            Err(e) => return Err(std::io::Error::other(e.to_string())),
        };
        let mut out = Vec::new();
        loop {
            let len = data
                .cFileName
                .iter()
                .position(|&c| c == 0)
                .unwrap_or(data.cFileName.len());
            let name = String::from_utf16_lossy(&data.cFileName[..len]);
            let attrs = data.dwFileAttributes;
            let dir = attrs & DIRECTORY != 0;
            let reparse = attrs & REPARSE != 0;
            // Folders: no reparse point at all. Files: only name-surrogate
            // tags are links (symlinks); cloud placeholders are kept.
            let link = reparse && (dir || data.dwReserved0 & NAME_SURROGATE != 0);
            if name != "." && name != ".." && !link {
                let size = if dir {
                    0
                } else {
                    let len = (u64::from(data.nFileSizeHigh) << 32) | u64::from(data.nFileSizeLow);
                    self.allocated(&path.join(&name), attrs, len)
                };
                out.push(Entry { name, dir, size });
            }
            if unsafe { FindNextFileW(handle, &mut data) }.is_err() {
                break;
            }
        }
        unsafe {
            let _ = FindClose(handle);
        }
        Ok(out)
    }

    /// Bytes the file takes on disk.
    #[cfg(windows)]
    fn allocated(&self, path: &Path, attrs: u32, len: u64) -> u64 {
        // Compressed, sparse and cloud placeholder files: ask the file
        // system; others take their size rounded up to whole clusters.
        const SPECIAL: u32 = 0x800 | 0x200 | 0x400 | 0x1000 | 0x40_0000;
        let size = if attrs & SPECIAL != 0 {
            compressed_size(path).unwrap_or(len)
        } else {
            len
        };
        if self.cluster > 1 {
            size.div_ceil(self.cluster) * self.cluster
        } else {
            size
        }
    }

    /// Walks one folder; `None` once the scan is cancelled.
    fn folder(&self, path: &Path, name: String) -> Option<Node> {
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
        let entries = match self.list(path) {
            Ok(entries) => entries,
            Err(_) => {
                self.progress.skipped.fetch_add(1, Ordering::Relaxed);
                node.denied = true;
                return Some(node);
            }
        };
        let mut kids = Vec::with_capacity(entries.len());
        let mut dirs = Vec::new();
        for e in entries {
            if e.dir {
                dirs.push(e.name);
            } else {
                self.progress.files.fetch_add(1, Ordering::Relaxed);
                self.progress.bytes.fetch_add(e.size, Ordering::Relaxed);
                kids.push(Node {
                    name: e.name,
                    size: e.size,
                    files: 1,
                    ..Node::default()
                });
            }
        }
        let sub = |name: &String| self.folder(&path.join(name), name.clone());
        let subs: Option<Vec<Node>> = if self.parallel && dirs.len() > 1 {
            use rayon::prelude::*;
            dirs.par_iter().map(sub).collect()
        } else {
            dirs.iter().map(sub).collect()
        };
        kids.extend(subs?);
        node.size = kids.iter().map(|k| k.size).sum();
        node.files = kids.iter().map(|k| k.files).sum();
        // fold sorts by size, then name: the result does not depend on the
        // order folders finished in.
        fold(&mut node, kids);
        Some(node)
    }
}

/// `\\?\C:\path\*`: no MAX_PATH limit.
#[cfg(windows)]
fn find_pattern(path: &Path) -> PathBuf {
    let text = path.to_string_lossy();
    let base = if text.starts_with(r"\\?\") {
        text.into_owned()
    } else if let Some(unc) = text.strip_prefix(r"\\") {
        format!(r"\\?\UNC\{unc}")
    } else {
        format!(r"\\?\{text}")
    };
    PathBuf::from(base).join("*")
}

/// Walks `root` on the calling thread; `None` if `progress.cancel` was set
/// before it finished.
pub fn walk(root: &Path, progress: &Progress) -> Option<Node> {
    walk_with(root, progress, false)
}

/// Walks `root`; with `parallel`, subfolders go to the current rayon pool.
fn walk_with(root: &Path, progress: &Progress, parallel: bool) -> Option<Node> {
    let meta = fs::metadata(root).ok()?;
    let walker = Walker {
        progress,
        cluster: cluster_size(root),
        parallel,
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

/// What the volume is, for how many threads walk it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum DriveKind {
    Ssd,
    Hdd,
    /// Removable, network, or the seek-penalty query gave no answer.
    Unknown,
}

/// SSD: many threads hide per-folder latency. HDD: one thread, because
/// parallel reads make the heads seek. Unknown: a cautious two.
pub fn scan_threads(kind: DriveKind, logical: usize) -> usize {
    match kind {
        DriveKind::Ssd => (logical * 2).clamp(2, 16),
        DriveKind::Hdd => 1,
        DriveKind::Unknown => 2,
    }
}

/// IOCTL_STORAGE_QUERY_PROPERTY / StorageDeviceSeekPenaltyProperty on
/// `\\.\X:`, opened with no access rights (no admin needed). No seek
/// penalty means solid state.
pub fn drive_kind(_root: &str) -> DriveKind {
    #[cfg(windows)]
    unsafe {
        use windows::core::HSTRING;
        use windows::Win32::Foundation::CloseHandle;
        use windows::Win32::Storage::FileSystem::{
            CreateFileW, GetDriveTypeW, FILE_FLAGS_AND_ATTRIBUTES, FILE_SHARE_READ,
            FILE_SHARE_WRITE, OPEN_EXISTING,
        };
        use windows::Win32::System::Ioctl::{
            PropertyStandardQuery, StorageDeviceSeekPenaltyProperty,
            DEVICE_SEEK_PENALTY_DESCRIPTOR, IOCTL_STORAGE_QUERY_PROPERTY, STORAGE_PROPERTY_QUERY,
        };
        use windows::Win32::System::IO::DeviceIoControl;
        const DRIVE_FIXED: u32 = 3;
        if GetDriveTypeW(&HSTRING::from(_root)) != DRIVE_FIXED {
            return DriveKind::Unknown;
        }
        let letter = _root.trim_end_matches('\\');
        let Ok(handle) = CreateFileW(
            &HSTRING::from(format!(r"\\.\{letter}")),
            0,
            FILE_SHARE_READ | FILE_SHARE_WRITE,
            None,
            OPEN_EXISTING,
            FILE_FLAGS_AND_ATTRIBUTES(0),
            None,
        ) else {
            return DriveKind::Unknown;
        };
        let query = STORAGE_PROPERTY_QUERY {
            PropertyId: StorageDeviceSeekPenaltyProperty,
            QueryType: PropertyStandardQuery,
            ..Default::default()
        };
        let mut desc = DEVICE_SEEK_PENALTY_DESCRIPTOR::default();
        let mut got = 0u32;
        let ok = DeviceIoControl(
            handle,
            IOCTL_STORAGE_QUERY_PROPERTY,
            Some(&query as *const _ as *const _),
            std::mem::size_of_val(&query) as u32,
            Some(&mut desc as *mut _ as *mut _),
            std::mem::size_of_val(&desc) as u32,
            Some(&mut got),
            None,
        )
        .is_ok();
        let _ = CloseHandle(handle);
        if !ok || (got as usize) < std::mem::size_of_val(&desc) {
            return DriveKind::Unknown;
        }
        if desc.IncursSeekPenalty {
            DriveKind::Hdd
        } else {
            DriveKind::Ssd
        }
    }
    #[cfg(not(windows))]
    DriveKind::Unknown
}

/// Walks `root` on a pool sized for its drive; every worker runs at
/// background priority. Returns the tree and the thread count used.
pub fn walk_drive(root: &Path, progress: &Progress, kind: DriveKind) -> (Option<Node>, usize) {
    let logical = std::thread::available_parallelism().map_or(4, |n| n.get());
    let threads = std::env::var("WINPERF_SPACE_THREADS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or_else(|| scan_threads(kind, logical))
        .max(1);
    if threads == 1 {
        return (walk(root, progress), 1);
    }
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(threads)
        .thread_name(|i| format!("fob-space-{i}"))
        .stack_size(16 << 20) // deep folder trees recurse
        .start_handler(|_| background_priority())
        .build();
    match pool {
        Ok(pool) => (pool.install(|| walk_with(root, progress, true)), threads),
        Err(_) => (walk(root, progress), 1),
    }
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
/// Streams the scan to its cache file (no second copy in memory).
pub fn write_cache(dir: &Path, scan: &Scan) -> std::io::Result<()> {
    use std::io::Write;
    fs::create_dir_all(dir)?;
    let path = cache_file(dir, &scan.root);
    let temp = path.with_extension("json.tmp");
    let mut out = std::io::BufWriter::new(fs::File::create(&temp)?);
    serde_json::to_writer(&mut out, scan)?;
    out.flush()?;
    drop(out);
    fs::rename(temp, path)
}
/// The cached scan of `root` as JSON bytes, checked but not turned into nodes.
pub fn read_cache(dir: &Path, root: &str) -> Option<Vec<u8>> {
    #[derive(Deserialize)]
    struct Head {
        root: String,
    }
    let bytes = fs::read(cache_file(dir, root)).ok()?;
    let head: Head = serde_json::from_slice(&bytes).ok()?;
    (head.root == root).then_some(bytes)
}
/// Gives freed scan memory back to the system.
fn release_memory() {
    #[cfg(windows)]
    unsafe {
        use windows::Win32::System::Memory::{GetProcessHeap, HeapCompact, HEAP_FLAGS};
        if let Ok(heap) = GetProcessHeap() {
            HeapCompact(heap, HEAP_FLAGS(0));
        }
    }
    #[cfg(all(target_os = "linux", target_env = "gnu"))]
    {
        extern "C" {
            fn malloc_trim(pad: usize) -> i32;
        }
        unsafe { malloc_trim(0) };
    }
}
/// Fob's own memory (resident / working set), MB.
fn own_memory_mb() -> u64 {
    let Ok(pid) = sysinfo::get_current_pid() else {
        return 0;
    };
    let mut sys = sysinfo::System::new();
    sys.refresh_processes(sysinfo::ProcessesToUpdate::Some(&[pid]), true);
    sys.process(pid).map_or(0, |p| p.memory() >> 20)
}
fn count_nodes(node: &Node) -> u64 {
    1 + node.children.iter().map(count_nodes).sum::<u64>()
}

struct Running {
    root: String,
    progress: Arc<Progress>,
    started: Instant,
}
#[derive(Default)]
pub struct SpaceState {
    running: Mutex<Option<Running>>,
    /// Scans whose cache file could not be written, as JSON (rare).
    unsaved: Mutex<HashMap<String, Arc<Vec<u8>>>>,
    /// The last scan that ended: (root, stopped before it finished).
    ended: Mutex<Option<(String, bool)>>,
    /// The last "other" listing (folder path, items), for its later pages.
    folded: Mutex<Option<(String, Arc<Vec<Folded>>)>>,
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
            let before_mb = own_memory_mb();
            let started = Instant::now();
            let kind = drive_kind(&root);
            let (tree, threads) = walk_drive(Path::new(&root), &progress, kind);
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
                let nodes = count_nodes(&scan.tree);
                let peak_mb = own_memory_mb();
                let saved = write_cache(&dir, &scan);
                if let Ok(mut unsaved) = state.unsaved.lock() {
                    unsaved.remove(&root);
                    if saved.is_err() {
                        if let Ok(bytes) = serde_json::to_vec(&scan) {
                            unsaved.insert(root.clone(), Arc::new(bytes));
                        }
                    }
                }
                let files = scan.files;
                drop(scan);
                release_memory();
                log_scan(
                    &app,
                    &format!(
                        "{root}: {kind:?}, {threads} threads, {files} files, {nodes} nodes kept, {} s, cache {}; \
                         fob memory {before_mb} MB before, {peak_mb} MB with the tree, \
                         {} MB after",
                        (started.elapsed().as_secs_f64() * 10.0).round() / 10.0,
                        if saved.is_ok() {
                            "written"
                        } else {
                            "NOT written"
                        },
                        own_memory_mb()
                    ),
                );
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
/// One line per finished scan in the app log folder's space.log.
fn log_scan(app: &AppHandle, line: &str) {
    use std::io::Write;
    let Ok(dir) = app.path().app_log_dir() else {
        return;
    };
    let _ = fs::create_dir_all(&dir);
    let path = dir.join("space.log");
    if fs::metadata(&path).is_ok_and(|m| m.len() > 256 * 1024) {
        let _ = fs::remove_file(&path);
    }
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    if let Ok(mut file) = fs::OpenOptions::new().create(true).append(true).open(path) {
        let _ = writeln!(file, "{stamp} {line}");
    }
}
/// The last scan of `root` as `Scan` JSON bytes (empty: none yet), read
/// from its cache file each time so no tree stays in Fob's memory.
#[tauri::command]
pub async fn space_result(app: AppHandle, root: String) -> Result<tauri::ipc::Response, String> {
    let unsaved = app
        .state::<SpaceState>()
        .unsaved
        .lock()
        .map_err(|e| e.to_string())?
        .get(&root)
        .cloned();
    let bytes = match unsaved {
        Some(bytes) => bytes.to_vec(),
        None => read_cache(&cache_dir(&app)?, &root).unwrap_or_default(),
    };
    Ok(tauri::ipc::Response::new(bytes))
}
/// An item of a folder's "other" block, as the page lists it.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Folded {
    pub n: String,
    pub s: u64,
    pub f: u64,
    #[serde(default)]
    pub d: bool,
    /// In the scan's tree (a folder here can open in the map); false: the
    /// scan folded it away and its size was read now from the disk.
    #[serde(default)]
    pub t: bool,
}

#[derive(Serialize)]
pub struct FoldedPage {
    pub total: usize,
    pub items: Vec<Folded>,
}

/// Everything in `path` except the `shown` map blocks: the `known` items
/// (merged on the page, sizes from the scan) as they are, the rest (folded
/// by the scan, names not cached) listed and measured now. Largest first.
pub fn folded_items(path: &Path, shown: &[String], known: Vec<Folded>) -> Vec<Folded> {
    use std::collections::{HashMap, HashSet};
    let shown: HashSet<&str> = shown.iter().map(String::as_str).collect();
    let mut known: HashMap<String, Folded> = known.into_iter().map(|k| (k.n.clone(), k)).collect();
    let progress = Progress::default();
    let Ok(meta) = fs::metadata(path) else {
        return Vec::new();
    };
    let walker = Walker {
        progress: &progress,
        cluster: cluster_size(path),
        parallel: false,
        #[cfg(unix)]
        device: {
            use std::os::unix::fs::MetadataExt;
            meta.dev()
        },
        #[cfg(unix)]
        seen: Default::default(),
    };
    let _ = &meta;
    let mut out: Vec<Folded> = Vec::new();
    for e in walker.list(path).unwrap_or_default() {
        if shown.contains(e.name.as_str()) {
            continue;
        }
        if let Some(k) = known.remove(&e.name) {
            out.push(Folded { t: true, ..k });
        } else if e.dir {
            let node = walker
                .folder(&path.join(&e.name), e.name.clone())
                .unwrap_or_default();
            out.push(Folded {
                n: e.name,
                s: node.size,
                f: node.files,
                d: true,
                t: false,
            });
        } else {
            out.push(Folded {
                n: e.name,
                s: e.size,
                f: 1,
                d: false,
                t: false,
            });
        }
    }
    // Known items that vanished since the scan are still shown as scanned.
    out.extend(known.into_values().map(|k| Folded { t: true, ..k }));
    out.sort_by(|a, b| b.s.cmp(&a.s).then_with(|| a.n.cmp(&b.n)));
    out
}

/// One page of `folded_items`. Offset 0 lists the folder again; later pages
/// come from that listing (one folder at a time is kept).
#[tauri::command]
pub async fn space_folded(
    app: AppHandle,
    path: String,
    shown: Vec<String>,
    known: Vec<Folded>,
    offset: usize,
    limit: usize,
) -> Result<FoldedPage, String> {
    let target = PathBuf::from(&path);
    if !target.is_absolute() || !drives().iter().any(|d| path.starts_with(d.root.as_str())) {
        return Err("Not inside a drive Fob scans".into());
    }
    let state = app.state::<SpaceState>();
    let cached = state
        .folded
        .lock()
        .ok()
        .and_then(|c| c.clone())
        .filter(|(p, _)| offset > 0 && *p == path)
        .map(|(_, items)| items);
    let items = match cached {
        Some(items) => items,
        None => {
            let items = tauri::async_runtime::spawn_blocking(move || {
                background_priority();
                folded_items(&target, &shown, known)
            })
            .await
            .map_err(|e| e.to_string())?;
            let items = Arc::new(items);
            if let Ok(mut c) = state.folded.lock() {
                *c = Some((path, items.clone()));
            }
            items
        }
    };
    let end = offset.saturating_add(limit.min(1000)).min(items.len());
    Ok(FoldedPage {
        total: items.len(),
        items: items.get(offset.min(end)..end).unwrap_or(&[]).to_vec(),
    })
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
        // Which of the two hard links carries the bytes depends on listing
        // order; the total counts them once either way.
        assert_eq!(
            tree.size,
            tree.children.iter().map(|c| c.size).sum::<u64>() + tree.other
        );
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
    fn a_parallel_walk_equals_the_serial_walk() {
        let root = temp("parallel");
        let mut seed = 0x9e37_79b9_7f4a_7c15u64;
        let mut rnd = move |n: u64| {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            seed % n
        };
        // ~60 folders up to 4 deep, 0-30 files each, sizes 0..2 MB, some
        // folders far over KEEP_MAX so folding is exercised.
        let mut dirs = vec![root.clone()];
        for i in 0..60 {
            let parent = dirs[rnd(dirs.len() as u64) as usize].clone();
            if parent.components().count() - root.components().count() >= 4 {
                continue;
            }
            let dir = parent.join(format!("d{i}"));
            fs::create_dir_all(&dir).unwrap();
            let files = if i % 17 == 0 {
                KEEP_MAX as u64 + 40
            } else {
                rnd(30)
            };
            for f in 0..files {
                file(&dir.join(format!("f{f}.bin")), rnd(2_000_000) as usize);
            }
            dirs.push(dir);
        }
        let serial = walk(&root, &Progress::default()).unwrap();
        for threads in [2, 4, 16] {
            let pool = rayon::ThreadPoolBuilder::new()
                .num_threads(threads)
                .build()
                .unwrap();
            let progress = Progress::default();
            let parallel = pool.install(|| walk_with(&root, &progress, true)).unwrap();
            assert_eq!(
                parallel, serial,
                "{threads} threads differ from the serial walk"
            );
            assert_eq!(progress.files.load(Ordering::Relaxed), serial.files);
        }
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn the_other_list_holds_everything_but_the_shown_blocks() {
        let root = temp("folded");
        fs::create_dir_all(root.join("shown")).unwrap();
        fs::create_dir_all(root.join("merged")).unwrap();
        fs::create_dir_all(root.join("tiny/inner")).unwrap();
        file(&root.join("shown/a.bin"), 900_000);
        file(&root.join("tiny/inner/x.bin"), 5000);
        file(&root.join("small.txt"), 100);
        let merged = Folded {
            n: "merged".into(),
            s: 777,
            f: 3,
            d: true,
            t: false,
        };
        let items = folded_items(&root, &["shown".into()], vec![merged]);
        let names: Vec<&str> = items.iter().map(|i| i.n.as_str()).collect();
        assert!(!names.contains(&"shown"));
        let m = items.iter().find(|i| i.n == "merged").unwrap();
        assert!(
            m.t && m.d && m.s == 777,
            "merged items keep their scanned size"
        );
        let tiny = items.iter().find(|i| i.n == "tiny").unwrap();
        assert!(
            !tiny.t && tiny.d && tiny.f == 1 && tiny.s >= 5000,
            "folded folders are measured"
        );
        assert!(items.iter().any(|i| i.n == "small.txt" && !i.d));
        assert!(items.windows(2).all(|w| w[0].s >= w[1].s), "largest first");
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn threads_follow_the_drive_kind() {
        assert_eq!(scan_threads(DriveKind::Ssd, 24), 16);
        assert_eq!(scan_threads(DriveKind::Ssd, 4), 8);
        assert_eq!(scan_threads(DriveKind::Ssd, 1), 2);
        assert_eq!(scan_threads(DriveKind::Hdd, 24), 1);
        assert_eq!(scan_threads(DriveKind::Unknown, 24), 2);
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
        let bytes = read_cache(&dir.join("cache"), &scan.root).unwrap();
        assert_eq!(serde_json::from_slice::<Scan>(&bytes).unwrap(), scan);
        assert_eq!(read_cache(&dir.join("cache"), "/elsewhere"), None);
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn the_scan_thread_runs_at_background_priority() {
        let src = include_str!("space.rs");
        let start = src.find("pub fn start(").unwrap();
        let body = &src[start..src[start..].find("\n}\n").unwrap() + start];
        assert!(body.contains("background_priority();"));
        let pool = &src[src.find("pub fn walk_drive(").unwrap()..];
        assert!(pool[..pool.find("\n}\n").unwrap()]
            .contains("start_handler(|_| background_priority())"));
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

/// Memory of a scan: WINPERF_SPACE_BENCH=/path cargo test --release
/// space_memory -- --ignored --nocapture
#[cfg(all(test, target_os = "linux"))]
#[test]
#[ignore]
fn space_memory() {
    fn rss() -> String {
        let s = fs::read_to_string("/proc/self/status").unwrap();
        s.lines()
            .filter(|l| l.starts_with("VmRSS") || l.starts_with("VmHWM"))
            .map(|l| l.split_whitespace().nth(1).unwrap().parse::<u64>().unwrap() / 1024)
            .map(|mb| format!("{mb} MB"))
            .collect::<Vec<_>>()
            .into_iter()
            .rev() // VmHWM comes first in the file
            .collect::<Vec<_>>()
            .join(" / peak ")
    }
    fn count(n: &Node) -> (u64, u64) {
        n.children
            .iter()
            .fold((1, n.name.capacity() as u64), |(c, b), k| {
                let (kc, kb) = count(k);
                (c + kc, b + kb)
            })
    }
    let root = std::env::var("WINPERF_SPACE_BENCH").expect("WINPERF_SPACE_BENCH");
    println!("start: {}", rss());
    let progress = Progress::default();
    let tree = if root == "synthetic" {
        // A C:\-like tree built through `fold` without touching the disk:
        // 1.69M files in ~340k folders, sizes log-normal around 8 KB.
        let mut seed = 0x2545_f491_4f6c_dd1du64;
        let mut rnd = move || {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            (seed >> 11) as f64 / (1u64 << 53) as f64
        };
        fn grow(rnd: &mut dyn FnMut() -> f64, depth: u32, left: &mut i64, p: &Progress) -> Node {
            let mut kids = Vec::new();
            let files = (rnd() * 10.0) as i64;
            for i in 0..files.min(*left) {
                let normal =
                    (-2.0 * rnd().max(1e-12).ln()).sqrt() * (std::f64::consts::TAU * rnd()).cos();
                let size = ((8192.0 * (2.3 * normal).exp()) as u64).div_ceil(4096) * 4096;
                p.files.fetch_add(1, Ordering::Relaxed);
                kids.push(Node {
                    name: format!("Microsoft.Windows.Component_{depth}_{i:04}.dll"),
                    size,
                    files: 1,
                    ..Node::default()
                });
            }
            *left -= files;
            let dirs = if depth > 11 { 0 } else { (rnd() * 6.1) as u32 };
            for i in 0..dirs {
                if *left <= 0 {
                    break;
                }
                let mut kid = grow(rnd, depth + 1, left, p);
                kid.name = format!("amd64_microsoft-windows-folder_{depth}_{i}");
                kids.push(kid);
            }
            let mut node = Node {
                dir: true,
                ..Node::default()
            };
            node.size = kids.iter().map(|k| k.size).sum();
            node.files = kids.iter().map(|k| k.files).sum();
            fold(&mut node, kids);
            node
        }
        let mut left = 1_690_000i64;
        let mut top = Vec::new();
        for i in 0..12 {
            let mut kid = grow(&mut rnd, 1, &mut left, &progress);
            kid.name = format!("Top folder {i}");
            top.push(kid);
        }
        let mut node = Node {
            name: "C:\\".into(),
            dir: true,
            ..Node::default()
        };
        node.size = top.iter().map(|k| k.size).sum();
        node.files = top.iter().map(|k| k.files).sum();
        fold(&mut node, top);
        node
    } else {
        walk(Path::new(&root), &progress).unwrap()
    };
    let (nodes, names) = count(&tree);
    println!(
        "walked {} files, {:.1} GB: {} nodes kept, names {} KB, Node {} B; rss {}",
        progress.files.load(Ordering::Relaxed),
        tree.size as f64 / 1e9,
        nodes,
        names / 1024,
        std::mem::size_of::<Node>(),
        rss()
    );
    // What `start` does with a finished tree.
    let dir = std::env::temp_dir().join(format!("fob-space-memory-{}", std::process::id()));
    let scan = Scan {
        root: root.clone(),
        sep: "/".into(),
        scanned_at: 0,
        elapsed_ms: 0,
        files: tree.files,
        skipped: 0,
        tree,
    };
    write_cache(&dir, &scan).unwrap();
    let json = fs::metadata(cache_file(&dir, &root)).unwrap().len();
    println!("cache written, {} KB; rss {}", json / 1024, rss());
    drop(scan);
    release_memory();
    println!("tree dropped: rss {}", rss());
    let bytes = read_cache(&dir, &root).unwrap();
    println!(
        "space_result bytes {} KB; rss {}",
        bytes.len() / 1024,
        rss()
    );
    drop(bytes);
    release_memory();
    println!("after: rss {}", rss());
    let _ = fs::remove_dir_all(dir);
}
