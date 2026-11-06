#![cfg_attr(not(windows), allow(dead_code))]
//! Build the space-map tree from NTFS MFT records.
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

#[cfg(test)]
use crate::mft_records::allocated;
pub use crate::mft_records::MftLink;

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
        if i % 1024 == 0 && progress.cancel.load(Ordering::Relaxed) {
            return None;
        }
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
        if progress.cancel.load(Ordering::Relaxed) {
            return None;
        }
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
fn read_volume(root: &std::path::Path, progress: &Progress) -> Option<Node> {
    let mut links = crate::mft_wire::Links::default();
    crate::mft_records::read_volume(root, |link| {
        if progress.cancel.load(Ordering::Relaxed) {
            return Err(std::io::Error::other("cancelled"));
        }
        if let Some(link) = link {
            links.push(link)?;
        }
        Ok(())
    })
    .ok()?;
    build_tree(
        5,
        root.to_string_lossy().into_owned(),
        &links
            .finish(|| {
                if progress.cancel.load(Ordering::Relaxed) {
                    Err(std::io::Error::other("cancelled"))
                } else {
                    Ok(())
                }
            })
            .ok()?,
        progress,
    )
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
    fn framed_scan_builds_the_existing_tree_and_counts_hardlinks_once() {
        let records = vec![
            link(5, 20, "folder", true, 0),
            link(20, 21, "a", false, 300_000),
            link(5, 21, "hard", false, 300_000),
        ];
        let mut bytes = crate::mft_wire::MAGIC.to_vec();
        for record in records {
            crate::mft_wire::frame(&mut bytes, 1, &serde_json::to_vec(&record).unwrap()).unwrap();
        }
        crate::mft_wire::frame(&mut bytes, 2, &[]).unwrap();
        let links = crate::mft_wire::receive(&mut bytes.as_slice(), |_| Ok(())).unwrap();
        assert_eq!(links[2].size, 0);
        let tree = build_tree(5, "C:\\".into(), &links, &Progress::default()).unwrap();
        assert_eq!((tree.files, tree.size), (2, 300_000));
        assert_eq!(tree.children[0].children[0].name, "a");
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
