//! Shared read-only NTFS records for direct and elevated scans.
#![cfg_attr(not(windows), allow(dead_code))]

/// One name of a file or folder, as the MFT lists it.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
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
    logical.div_ceil(cluster).saturating_mul(cluster)
}

#[cfg(windows)]
pub fn is_ntfs(root: &std::path::Path) -> bool {
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
    let letter = text.chars().next()?;
    crate::mft_wire::request(&text)?;
    if !letter.is_ascii_alphabetic() {
        return None;
    }
    Some(format!(r"\\.\{letter}:"))
}

#[cfg(windows)]
pub fn read_volume(
    root: &std::path::Path,
    mut emit: impl FnMut(Option<MftLink>) -> std::io::Result<()>,
) -> std::io::Result<()> {
    use ntfs_reader::{MftScan, NtfsFileNamespace, Volume};
    if !is_ntfs(root) {
        return Err(std::io::Error::other("unsupported"));
    }
    let device = volume_path(root).ok_or_else(|| std::io::Error::other("root"))?;
    let volume = match Volume::new(&device) {
        Ok(v) => v,
        Err(e) => return Err(std::io::Error::other(e.to_string())), // AccessDenied without admin, or not a volume
    };
    let cluster = {
        // Use the same cluster geometry as the directory walk.
        cluster_size(root).max(1)
    };
    let mut scan = match MftScan::new(volume) {
        Ok(s) => s,
        Err(e) => return Err(std::io::Error::other(e.to_string())),
    };
    loop {
        emit(None)?;
        let chunk = match scan
            .next_chunk()
            .map_err(|e| std::io::Error::other(e.to_string()))?
        {
            Some(chunk) => chunk,
            None => break,
        };
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
            let mut first = true;
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
                emit(Some(MftLink {
                    parent: name.parent_number(),
                    id,
                    name: name.to_string(),
                    dir: file.is_directory(),
                    size,
                }))?;
            }
        }
    }
    Ok(())
}

#[cfg(windows)]
fn cluster_size(root: &std::path::Path) -> u64 {
    use windows::Win32::Storage::FileSystem::GetDiskFreeSpaceW;
    let (mut sectors, mut bytes) = (0u32, 0u32);
    let ok = unsafe {
        GetDiskFreeSpaceW(
            &windows::core::HSTRING::from(root.as_os_str()),
            Some(&mut sectors),
            Some(&mut bytes),
            None,
            None,
        )
    };
    if ok.is_ok() {
        u64::from(sectors) * u64::from(bytes)
    } else {
        1
    }
}
