#![cfg_attr(not(windows), allow(dead_code))]
//! Fob 0.8.0: build the space-map tree from NTFS MFT records.
//!
//! The Windows reader (`try_scan`) lives behind `cfg(windows)`. The tree
//! builder is pure and tested with fixtures on every OS. Hard links share one
//! file id: the first name carries the size, the rest carry 0 (same as the
//! directory walk). Junctions and other directory reparse points are dropped,
//! matching the walk. Size is allocated bytes (logical size rounded up to the
//! volume's cluster size), matching non-special files in the walk.

use crate::space::{fold, Node, Progress};
use std::collections::HashMap;
use std::sync::atomic::Ordering;

/// One name of a file or folder, as the MFT lists it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MftLink {
    /// Parent directory's MFT record number.
    pub parent: u64,
    /// This file's MFT record number (shared by hard links).
    pub id: u64,
    pub name: String,
    pub dir: bool,
    /// Allocated bytes; 0 for an extra hard-link name.
    pub size: u64,
}

/// Round a logical size up to whole clusters (walk does the same for ordinary files).
pub fn allocated(logical: u64, cluster: u64) -> u64 {
    if cluster <= 1 {
        return logical;
    }
    logical.div_ceil(cluster) * cluster
}

/// Builds the same `Node` tree the directory walk produces, from flat MFT links.
/// `root_id` is the volume root's record number; `root_name` is the drive label
/// shown on the page (e.g. `C:\`). Returns `None` if cancelled.
pub fn build_tree(
    root_id: u64,
    root_name: String,
    links: &[MftLink],
    progress: &Progress,
) -> Option<Node> {
    let mut by_parent: HashMap<u64, Vec<usize>> = HashMap::new();
    for (i, link) in links.iter().enumerate() {
        by_parent.entry(link.parent).or_default().push(i);
    }
    build_dir(root_id, root_name, &by_parent, links, progress)
}

fn build_dir(
    id: u64,
    name: String,
    by_parent: &HashMap<u64, Vec<usize>>,
    links: &[MftLink],
    progress: &Progress,
) -> Option<Node> {
    if progress.cancel.load(Ordering::Relaxed) {
        return None;
    }
    if let Ok(mut current) = progress.current.try_lock() {
        current.clear();
        current.push_str(&name);
    }
    let mut node = Node {
        name,
        dir: true,
        ..Node::default()
    };
    let mut kids = Vec::new();
    let mut files = 0u64;
    let mut bytes = 0u64;
    for &i in by_parent.get(&id).map(|v| v.as_slice()).unwrap_or(&[]) {
        let link = &links[i];
        if link.dir {
            let child = build_dir(link.id, link.name.clone(), by_parent, links, progress)?;
            kids.push(child);
        } else {
            files += 1;
            bytes += link.size;
            kids.push(Node {
                name: link.name.clone(),
                size: link.size,
                files: 1,
                ..Node::default()
            });
        }
    }
    progress.files.fetch_add(files, Ordering::Relaxed);
    progress.bytes.fetch_add(bytes, Ordering::Relaxed);
    progress.dirs.fetch_add(1, Ordering::Relaxed);
    node.size = kids.iter().map(|k| k.size).sum();
    node.files = kids.iter().map(|k| k.files).sum();
    fold(&mut node, kids);
    Some(node)
}

/// Tries an MFT scan of `root` (e.g. `C:\`). `None` means fall back to the walk:
/// not Windows, not NTFS, no elevation, or the reader failed.
pub fn try_scan(root: &std::path::Path, progress: &Progress) -> Option<Node> {
    #[cfg(windows)]
    {
        return read_volume(root, progress);
    }
    #[cfg(not(windows))]
    {
        let _ = (root, progress);
        None
    }
}

#[cfg(windows)]
fn is_ntfs(root: &std::path::Path) -> bool {
    use windows::Win32::Storage::FileSystem::GetVolumeInformationW;
    let path = windows::core::HSTRING::from(root.as_os_str());
    let mut fs = [0u16; 32];
    let ok = unsafe { GetVolumeInformationW(&path, None, None, None, None, Some(&mut fs)) };
    if ok.is_err() {
        return false;
    }
    let end = fs.iter().position(|&c| c == 0).unwrap_or(fs.len());
    String::from_utf16_lossy(&fs[..end]).eq_ignore_ascii_case("NTFS")
}

#[cfg(windows)]
fn volume_path(root: &std::path::Path) -> Option<String> {
    // ntfs-reader wants `\\.\C:` with no trailing slash.
    let text = root.to_string_lossy();
    let letter = text.chars().next()?.to_ascii_uppercase();
    if !letter.is_ascii_alphabetic() {
        return None;
    }
    Some(format!(r"\\.\{letter}:"))
}

#[cfg(windows)]
fn read_volume(root: &std::path::Path, progress: &Progress) -> Option<Node> {
    use ntfs_reader::{MftScan, NtfsFileNamespace, Volume, ROOT_RECORD};
    if !is_ntfs(root) {
        return None;
    }
    let device = volume_path(root)?;
    let volume = match Volume::new(&device) {
        Ok(v) => v,
        Err(_) => return None, // AccessDenied without admin, or not a volume
    };
    let cluster = {
        // Boot sector geometry: prefer the volume's cluster size when exposed.
        // Fall back to GetDiskFreeSpaceW via space::cluster_size.
        crate::space::cluster_size(root).max(1)
    };
    let mut scan = match MftScan::new(volume) {
        Ok(s) => s,
        Err(_) => return None,
    };
    let mut links: Vec<MftLink> = Vec::new();
    let mut sized: std::collections::HashSet<u64> = std::collections::HashSet::new();
    while let Ok(Some(chunk)) = scan.next_chunk() {
        if progress.cancel.load(Ordering::Relaxed) {
            return None;
        }
        for file in chunk.files() {
            if file.is_directory() {
                // Directory reparse points (junctions, mount points): the walk
                // skips them; children live under the real target on this volume.
                let reparse = file
                    .standard_information()
                    .map(|s| s.file_attributes() & 0x400 != 0)
                    .unwrap_or(false)
                    || file.hard_links().any(|n| n.is_reparse_point());
                if reparse {
                    continue;
                }
            }
            let size_once = if file.is_directory() {
                0
            } else {
                let logical = file
                    .data_streams()
                    .find(|s| s.name.is_none())
                    .map(|s| s.size)
                    .unwrap_or(0);
                allocated(logical, cluster)
            };
            let id = file.number();
            let mut first = sized.insert(id);
            for name in file.hard_links() {
                // Skip pure DOS 8.3 aliases; hard_links() already drops most,
                // but keep a namespace guard for Win32AndDos-only oddities.
                if matches!(name.namespace(), Some(NtfsFileNamespace::Dos)) {
                    continue;
                }
                let size = if file.is_directory() {
                    0
                } else if first {
                    first = false;
                    size_once
                } else {
                    0
                };
                links.push(MftLink {
                    parent: name.parent_number(),
                    id,
                    name: name.to_string(),
                    dir: file.is_directory(),
                    size,
                });
            }
        }
    }
    let root_name = root.to_string_lossy().into_owned();
    build_tree(ROOT_RECORD, root_name, &links, progress)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::space::Progress;

    fn link(parent: u64, id: u64, name: &str, dir: bool, size: u64) -> MftLink {
        MftLink {
            parent,
            id,
            name: name.into(),
            dir,
            size,
        }
    }

    #[test]
    fn allocated_rounds_up_to_the_cluster() {
        assert_eq!(allocated(1, 4096), 4096);
        assert_eq!(allocated(4096, 4096), 4096);
        assert_eq!(allocated(4097, 4096), 8192);
        assert_eq!(allocated(100, 1), 100);
    }

    #[test]
    fn fixture_cases_build_the_expected_tree() {
        #[derive(serde::Deserialize)]
        struct Link {
            parent: u64,
            id: u64,
            name: String,
            dir: bool,
            size: u64,
        }
        #[derive(serde::Deserialize)]
        struct Case {
            name: String,
            links: Vec<Link>,
            files: u64,
            size: u64,
        }
        #[derive(serde::Deserialize)]
        struct Fixture {
            #[serde(rename = "rootId")]
            root_id: u64,
            #[serde(rename = "rootName")]
            root_name: String,
            cases: Vec<Case>,
        }
        let fix: Fixture = serde_json::from_str(include_str!("../../tests/fixtures/mft-tree.json"))
            .expect("mft-tree.json");
        for case in fix.cases {
            let links: Vec<MftLink> = case
                .links
                .into_iter()
                .map(|l| MftLink {
                    parent: l.parent,
                    id: l.id,
                    name: l.name,
                    dir: l.dir,
                    size: l.size,
                })
                .collect();
            let progress = Progress::default();
            let tree = build_tree(fix.root_id, fix.root_name.clone(), &links, &progress)
                .unwrap_or_else(|| panic!("{} cancelled", case.name));
            assert_eq!(tree.files, case.files, "{}", case.name);
            assert_eq!(tree.size, case.size, "{}", case.name);
            if case.name == "hardlink-once" {
                assert_eq!(progress.bytes.load(Ordering::Relaxed), case.size);
            }
        }
    }

    #[test]
    fn no_debug_log_files_are_written() {
        let space = include_str!("space.rs");
        let companion = include_str!("companion.rs");
        assert!(!space.contains(concat!("space", ".log")));
        assert!(!space.contains("log_scan"));
        assert!(!companion.contains(concat!("mini-placement", ".txt")));
        assert!(!companion.contains(concat!("mini-placement", ".log")));
        assert!(!companion.contains("fs::write(dir.join"));
    }

    #[test]
    fn cancel_stops_the_build() {
        let links = vec![link(5, 10, "a", false, 1)];
        let progress = Progress::default();
        progress.cancel.store(true, Ordering::Relaxed);
        assert!(build_tree(5, r"C:\".into(), &links, &progress).is_none());
    }
}
