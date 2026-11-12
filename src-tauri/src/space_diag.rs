//! Windows-only diagnostics for the space scan (run explicitly, never in CI by
//! default): does a raw MFT read show folders that were just deleted, moved or
//! recycled, and what do delete-pending names look like to the walk, the MFT
//! and Open? Prints `DIAG` lines; asserts only that each run was valid.
//!
//! cargo test --manifest-path src-tauri/Cargo.toml space::diag -- --ignored --nocapture --test-threads=1
//! Env: FOB_DIAG_DIR (base folder, default the temp folder), FOB_DIAG_ITERS
//! (default 40), FOB_DIAG_RECYCLE (0 turns the Recycle Bin probe off).
use super::{Progress, Walker};
use crate::mft_records::{read_volume, MftLink};
use std::fs;
use std::os::windows::fs::OpenOptionsExt;
use std::os::windows::io::AsRawHandle;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

const PREFIX: &str = "fobdiag";
const BACKUP_SEMANTICS: u32 = 0x0200_0000;
const SHARE_ALL: u32 = 0x7;
const DELETE: u32 = 0x0001_0000;
const READ_ATTRIBUTES: u32 = 0x80;

fn base_dir(tag: &str) -> PathBuf {
    let root = std::env::var_os("FOB_DIAG_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    let dir = root.join(format!("{PREFIX}_{tag}_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

/// `C:\` for any path on C:.
fn volume_root(path: &Path) -> PathBuf {
    let text = fs::canonicalize(path)
        .unwrap()
        .to_string_lossy()
        .into_owned();
    let text = text.trim_start_matches(r"\\?\");
    let letter = text.chars().next().unwrap().to_ascii_uppercase();
    PathBuf::from(format!("{letter}:\\"))
}

/// FlushFileBuffers on the volume (what `Write-VolumeCache` does). Needs admin.
fn flush_volume(root: &Path) -> Duration {
    let letter = root.to_string_lossy().chars().next().unwrap();
    let started = Instant::now();
    let volume = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(format!(r"\\.\{letter}:"))
        .expect("open volume for flush (admin?)");
    volume.sync_all().expect("FlushFileBuffers on the volume");
    started.elapsed()
}

struct Read {
    /// Our links only (names starting with PREFIX), plus every link of `ids`.
    ours: Vec<MftLink>,
    total: u64,
    took: Duration,
}

/// The production MFT read, unchanged; keeps only what the diagnostics ask about.
fn read_mft(root: &Path, ids: &[u64]) -> Read {
    let started = Instant::now();
    let (mut ours, mut total) = (Vec::new(), 0u64);
    read_volume(root, |link| {
        if let Some(link) = link {
            total += 1;
            if link.name.starts_with(PREFIX) || ids.contains(&link.id) {
                ours.push(link);
            }
        }
        Ok(())
    })
    .expect("read_volume (admin and NTFS required)");
    Read {
        ours,
        total,
        took: started.elapsed(),
    }
}

fn seen(read: &Read, name: &str) -> bool {
    read.ours.iter().any(|l| l.name == name)
}

fn file_with(path: &Path) {
    fs::write(path, vec![7u8; 4096]).unwrap();
}

fn tree(dir: &Path) {
    fs::create_dir_all(dir.join("sub")).unwrap();
    file_with(&dir.join("a.bin"));
    file_with(&dir.join("sub").join("b.bin"));
}

/// Deletes the way Explorer's permanent delete and uninstallers do: DeleteFileW
/// and RemoveDirectoryW, depth first.
fn delete_tree(dir: &Path) {
    for entry in fs::read_dir(dir).unwrap().flatten() {
        let path = entry.path();
        if entry.file_type().unwrap().is_dir() {
            delete_tree(&path);
        } else {
            fs::remove_file(&path).unwrap();
        }
    }
    fs::remove_dir(dir).unwrap();
}

/// SHFileOperationW with FOF_ALLOWUNDO: moves `path` to the Recycle Bin.
fn recycle(path: &Path) -> Result<(), String> {
    use windows::core::PCWSTR;
    use windows::Win32::UI::Shell::{
        SHFileOperationW, FOF_ALLOWUNDO, FOF_NOCONFIRMATION, FOF_NOERRORUI, FOF_SILENT, FO_DELETE,
        SHFILEOPSTRUCTW,
    };
    let mut from: Vec<u16> = path.as_os_str().encode_wide_vec();
    from.extend([0, 0]);
    let mut op = SHFILEOPSTRUCTW {
        wFunc: FO_DELETE,
        pFrom: PCWSTR(from.as_ptr()),
        fFlags: (FOF_ALLOWUNDO.0 | FOF_NOCONFIRMATION.0 | FOF_NOERRORUI.0 | FOF_SILENT.0) as u16,
        ..Default::default()
    };
    let code = unsafe { SHFileOperationW(&mut op) };
    if code != 0 || op.fAnyOperationsAborted.as_bool() {
        return Err(format!(
            "SHFileOperationW {code:#x} aborted={}",
            op.fAnyOperationsAborted.as_bool()
        ));
    }
    if fs::symlink_metadata(path).is_ok() {
        return Err("still exists".into());
    }
    Ok(())
}

trait Wide {
    fn encode_wide_vec(&self) -> Vec<u16>;
}
impl Wide for std::ffi::OsStr {
    fn encode_wide_vec(&self) -> Vec<u16> {
        use std::os::windows::ffi::OsStrExt;
        self.encode_wide().collect()
    }
}

/// Recycle once on a worker thread; a shell that hangs or refuses turns it off.
fn recycle_works(base: &Path) -> Result<(), String> {
    if std::env::var("FOB_DIAG_RECYCLE").as_deref() == Ok("0") {
        return Err("off (FOB_DIAG_RECYCLE=0)".into());
    }
    let probe = base.join(format!("{PREFIX}_probe"));
    tree(&probe);
    let (tx, rx) = std::sync::mpsc::channel();
    let target = probe.clone();
    std::thread::spawn(move || {
        let _ = tx.send(recycle(&target));
    });
    rx.recv_timeout(Duration::from_secs(30))
        .unwrap_or_else(|_| Err("timed out after 30 s".into()))
}

fn gone(path: &Path) -> bool {
    matches!(fs::symlink_metadata(path), Err(e) if e.kind() == std::io::ErrorKind::NotFound)
}

#[derive(Default)]
struct Tally {
    runs: u32,
    invalid: u32,
    deleted: u32,
    moved_old: u32,
    moved_new_missing: u32,
    recycled: u32,
    recycle_runs: u32,
    any: u32,
    read_ms: Vec<u128>,
    flush_ms: Vec<u128>,
}

impl Tally {
    fn line(&self, variant: &str) -> String {
        let med = |v: &[u128]| {
            let mut v = v.to_vec();
            v.sort_unstable();
            v.get(v.len() / 2).copied().unwrap_or(0)
        };
        let range = |v: &[u128]| {
            format!(
                "{}..{}",
                v.iter().min().copied().unwrap_or(0),
                v.iter().max().copied().unwrap_or(0)
            )
        };
        format!(
            "DIAG summary variant={variant} runs={} invalid={} stale_any={} deleted_seen={} moved_old_seen={} moved_new_missing={} recycled_seen={}/{} read_ms_median={} read_ms_range={} flush_ms_median={} flush_ms_range={}",
            self.runs, self.invalid, self.any, self.deleted, self.moved_old, self.moved_new_missing,
            self.recycled, self.recycle_runs, med(&self.read_ms), range(&self.read_ms),
            med(&self.flush_ms), range(&self.flush_ms)
        )
    }
}

/// Deleted / moved / recycled folders, read back at once with and without a
/// volume flush first, alternating, N times.
#[test]
#[ignore = "Windows diagnostic: needs admin, reads the whole MFT many times"]
fn diag_stale_mft_after_delete_move_recycle() {
    let base = base_dir("stale");
    let root = volume_root(&base);
    let iters: u32 = std::env::var("FOB_DIAG_ITERS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(40);
    let recycle_ok = recycle_works(&base);
    println!(
        "DIAG setup base_volume={} iters={iters} recycle={:?}",
        root.display(),
        recycle_ok
    );
    let moved = base.join(format!("{PREFIX}_moved"));
    fs::create_dir_all(&moved).unwrap();
    let (mut plain, mut flushed) = (Tally::default(), Tally::default());
    let mut clear_after_ms: Vec<String> = Vec::new();
    for k in 0..iters {
        let flush = k % 2 == 1;
        let it = base.join(format!("{PREFIX}_it{k}"));
        let names = [
            format!("{PREFIX}_keep{k}"),
            format!("{PREFIX}_del{k}"),
            format!("{PREFIX}_mv{k}"),
            format!("{PREFIX}_mvd{k}"),
            format!("{PREFIX}_rc{k}"),
        ];
        let [keep, del, mv, mvd, rc] = &names;
        for n in [keep, del, mv, rc] {
            tree(&it.join(n));
        }
        // The new folders must be on disk before the change under test.
        flush_volume(&root);
        delete_tree(&it.join(del));
        fs::rename(it.join(mv), moved.join(mvd)).unwrap();
        let recycled = recycle_ok.is_ok() && recycle(&it.join(rc)).is_ok();
        assert!(
            gone(&it.join(del)) && gone(&it.join(mv)),
            "changes did not happen"
        );
        let tally = if flush { &mut flushed } else { &mut plain };
        if flush {
            tally.flush_ms.push(flush_volume(&root).as_millis());
        }
        let read = read_mft(&root, &[]);
        tally.runs += 1;
        tally.read_ms.push(read.took.as_millis());
        let valid = seen(&read, keep);
        let (d, mo, mn, r) = (
            seen(&read, del),
            seen(&read, mv),
            !seen(&read, mvd),
            recycled && seen(&read, rc),
        );
        if !valid {
            tally.invalid += 1;
        } else {
            tally.deleted += u32::from(d);
            tally.moved_old += u32::from(mo);
            tally.moved_new_missing += u32::from(mn);
            tally.recycle_runs += u32::from(recycled);
            tally.recycled += u32::from(r);
            tally.any += u32::from(d || mo || mn || r);
        }
        println!(
            "DIAG iter={k} flush={flush} valid={valid} deleted_seen={d} moved_old_seen={mo} moved_new_missing={mn} recycled={recycled} recycled_seen={r} links={} read_ms={}",
            read.total,
            read.took.as_millis()
        );
        // How long a stale entry lasts without any flush (first three only).
        if !flush && valid && (d || mo || r) && clear_after_ms.len() < 3 {
            let started = Instant::now();
            let mut result = "still stale after 180 s".to_string();
            while started.elapsed() < Duration::from_secs(180) {
                std::thread::sleep(Duration::from_secs(5));
                let again = read_mft(&root, &[]);
                if !seen(&again, del) && !seen(&again, mv) && !(recycled && seen(&again, rc)) {
                    result = format!("clear at {} ms", started.elapsed().as_millis());
                    break;
                }
            }
            println!("DIAG iter={k} stale_lifetime {result}");
            clear_after_ms.push(result);
        }
        let _ = fs::remove_dir_all(&it);
        let _ = fs::remove_dir_all(moved.join(mvd));
    }
    println!("{}", plain.line("no_flush"));
    println!("{}", flushed.line("flush"));
    let _ = fs::remove_dir_all(&base);
    assert!(plain.runs + flushed.runs == iters);
    assert_eq!(
        flushed.invalid, 0,
        "a flushed read missed a folder kept on disk"
    );
}

fn file_index(file: &fs::File) -> u64 {
    use windows::Win32::Foundation::HANDLE;
    use windows::Win32::Storage::FileSystem::{
        GetFileInformationByHandle, BY_HANDLE_FILE_INFORMATION,
    };
    let mut info = BY_HANDLE_FILE_INFORMATION::default();
    unsafe { GetFileInformationByHandle(HANDLE(file.as_raw_handle()), &mut info) }.unwrap();
    ((u64::from(info.nFileIndexHigh) << 32) | u64::from(info.nFileIndexLow)) & 0x0000_FFFF_FFFF_FFFF
}

/// A handle that lets others delete the item while it stays open.
fn hold(path: &Path, dir: bool) -> fs::File {
    fs::OpenOptions::new()
        .access_mode(READ_ATTRIBUTES)
        .share_mode(SHARE_ALL)
        .custom_flags(if dir { BACKUP_SEMANTICS } else { 0 })
        .open(path)
        .unwrap()
}

/// Classic (non-POSIX) delete: FileDispositionInfo through a DELETE handle.
fn legacy_delete(path: &Path, dir: bool) -> Result<(), String> {
    use windows::Win32::Foundation::HANDLE;
    use windows::Win32::Storage::FileSystem::{
        FileDispositionInfo, SetFileInformationByHandle, FILE_DISPOSITION_INFO,
    };
    let file = fs::OpenOptions::new()
        .access_mode(DELETE)
        .share_mode(SHARE_ALL)
        .custom_flags(if dir { BACKUP_SEMANTICS } else { 0 })
        .open(path)
        .map_err(|e| e.to_string())?;
    let info = FILE_DISPOSITION_INFO { DeleteFile: true };
    unsafe {
        SetFileInformationByHandle(
            HANDLE(file.as_raw_handle()),
            FileDispositionInfo,
            &info as *const _ as *const _,
            std::mem::size_of::<FILE_DISPOSITION_INFO>() as u32,
        )
    }
    .map_err(|e| e.to_string())
}

fn open_check(path: &Path) -> String {
    // Same test as space_reveal.
    match fs::symlink_metadata(path) {
        Ok(_) => "exists".into(),
        Err(e) => format!(
            "err os={:?} kind={:?} => reveal says no longer exists",
            e.raw_os_error(),
            e.kind()
        ),
    }
}

/// Delete-pending names: deleted while another handle (opened with
/// FILE_SHARE_DELETE) stays open; seen by the walk, the MFT and Open?
#[test]
#[ignore = "Windows diagnostic: needs admin, reads the whole MFT"]
fn diag_delete_pending_names() {
    let base = base_dir("pending");
    let root = volume_root(&base);
    let progress = Progress::default();
    let walker = Walker {
        progress: &progress,
        cluster: 4096,
        parallel: false,
    };
    let base_id = file_index(&hold(&base, true));
    // (name, is dir, how)
    let cases = [
        (format!("{PREFIX}_dir_api"), true, "RemoveDirectoryW"),
        (format!("{PREFIX}_dir_legacy"), true, "FileDispositionInfo"),
        (format!("{PREFIX}_file_api.bin"), false, "DeleteFileW"),
        (
            format!("{PREFIX}_file_legacy.bin"),
            false,
            "FileDispositionInfo",
        ),
    ];
    let mut holders = Vec::new();
    let mut ids = Vec::new();
    for (name, dir, how) in &cases {
        let path = base.join(name);
        if *dir {
            fs::create_dir(&path).unwrap();
        } else {
            file_with(&path);
        }
        let held = hold(&path, *dir);
        ids.push(file_index(&held));
        let result = match (*dir, *how) {
            (true, "RemoveDirectoryW") => fs::remove_dir(&path).map_err(|e| e.to_string()),
            (false, "DeleteFileW") => fs::remove_file(&path).map_err(|e| e.to_string()),
            _ => legacy_delete(&path, *dir),
        };
        println!("DIAG pending case={name} how={how} delete_result={result:?}");
        holders.push(held);
    }
    let report = |phase: &str| {
        let flush_ms = flush_volume(&root).as_millis();
        let listed: Vec<String> = walker
            .list(&base)
            .unwrap()
            .into_iter()
            .map(|e| e.name)
            .collect();
        let read = read_mft(&root, &ids);
        for ((name, _, how), id) in cases.iter().zip(&ids) {
            let under_base = read
                .ours
                .iter()
                .any(|l| l.parent == base_id && &l.name == name);
            let elsewhere: Vec<String> = read
                .ours
                .iter()
                .filter(|l| l.id == *id && !(l.parent == base_id && &l.name == name))
                .map(|l| format!("parent={} name={}", l.parent, l.name))
                .collect();
            println!(
                "DIAG pending phase={phase} case={name} how={how} walk_lists={} mft_under_base={under_base} mft_same_id_elsewhere={elsewhere:?} open={}",
                listed.contains(name),
                open_check(&base.join(name)),
            );
        }
        println!(
            "DIAG pending phase={phase} flush_ms={flush_ms} read_ms={} links={}",
            read.took.as_millis(),
            read.total
        );
        (listed, read)
    };
    report("held");
    drop(holders);
    let (listed, read) = report("closed");
    for (name, _, _) in &cases {
        assert!(
            !listed.contains(name),
            "{name} still listed after the last handle closed"
        );
        assert!(
            !read
                .ours
                .iter()
                .any(|l| l.parent == base_id && &l.name == name),
            "{name} still in the flushed MFT after the last handle closed"
        );
    }
    let _ = fs::remove_dir_all(&base);
}
