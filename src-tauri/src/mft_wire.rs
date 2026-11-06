//! Bounded, versioned metadata-only protocol shared with the elevated worker.
#![cfg_attr(not(windows), allow(dead_code))]
use crate::mft_records::MftLink;
use std::collections::{HashMap, HashSet};
use std::io::{self, Read, Write};

pub const MAGIC: &[u8; 4] = b"FM01";
pub const MAX_FRAME: usize = 4096;
pub const MAX_LINKS: usize = 4_000_000;
pub const MAX_NAMES: usize = 128 * 1024 * 1024;
pub const MAX_STREAM: usize = 768 * 1024 * 1024;
pub const IDLE_SECS: u64 = 15;
pub const TOTAL_SECS: u64 = 180;

fn invalid() -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, "invalid MFT response")
}

pub fn request(root: &str) -> Option<[u8; 8]> {
    let b = root.as_bytes();
    if b.len() != 3 || !b[0].is_ascii_uppercase() || &b[1..] != b":\\" {
        return None;
    }
    Some([b'F', b'M', b'0', b'1', b[0], 0, 0, 0])
}

pub fn frame(out: &mut impl Write, kind: u8, data: &[u8]) -> io::Result<()> {
    if data.len() > MAX_FRAME {
        return Err(invalid());
    }
    out.write_all(&[kind])?;
    out.write_all(&(data.len() as u32).to_le_bytes())?;
    out.write_all(data)
}

#[derive(Default)]
pub struct Links {
    links: Vec<MftLink>,
    names: usize,
    sized: HashSet<u64>,
    dirs: HashMap<u64, u64>,
    bytes: u64,
}
impl Links {
    pub fn push(&mut self, mut link: MftLink) -> io::Result<()> {
        self.names = self
            .names
            .checked_add(link.name.len())
            .ok_or_else(invalid)?;
        if self.links.len() >= MAX_LINKS
            || self.names > MAX_NAMES
            || link.name.is_empty()
            || link.name.len() > 1024
            || link.name == "."
            || link.name == ".."
            || link
                .name
                .chars()
                .any(|c| matches!(c, '\0' | '\\' | '/' | ':'))
            || link.id == link.parent
            || link.id == 5
            || link.size > (1 << 60)
            || (link.dir && (link.size != 0 || self.dirs.insert(link.id, link.parent).is_some()))
        {
            return Err(invalid());
        }
        if (link.dir && self.sized.contains(&link.id))
            || (!link.dir && self.dirs.contains_key(&link.id))
        {
            return Err(invalid());
        }
        if !link.dir && !self.sized.insert(link.id) {
            link.size = 0;
        }
        self.bytes = self.bytes.checked_add(link.size).ok_or_else(invalid)?;
        self.links.push(link);
        Ok(())
    }
    pub fn finish(self, mut check: impl FnMut() -> io::Result<()>) -> io::Result<Vec<MftLink>> {
        // The tree builder recurses. Reject corrupt cycles and excessive depth first.
        let mut depths: HashMap<u64, usize> = HashMap::from([(5, 0)]);
        for &id in self.dirs.keys() {
            check()?;
            let mut chain = Vec::new();
            let mut next = id;
            while !depths.contains_key(&next) {
                if chain.len() >= 512 || chain.contains(&next) {
                    return Err(invalid());
                }
                chain.push(next);
                let Some(parent) = self.dirs.get(&next) else {
                    break;
                };
                next = *parent;
            }
            let mut depth = depths.get(&next).copied().unwrap_or(0);
            for id in chain.into_iter().rev() {
                depth += 1;
                if depth > 512 {
                    return Err(invalid());
                }
                depths.insert(id, depth);
            }
        }
        Ok(self.links)
    }
}

pub fn receive(
    input: &mut impl Read,
    mut pulse: impl FnMut(usize) -> io::Result<()>,
) -> io::Result<Vec<MftLink>> {
    let mut magic = [0; 4];
    input.read_exact(&mut magic)?;
    if &magic != MAGIC {
        return Err(invalid());
    }
    let mut links = Links::default();
    let mut total = 0usize;
    let mut count = 0;
    loop {
        pulse(count)?;
        let mut head = [0; 5];
        input.read_exact(&mut head)?;
        let len = u32::from_le_bytes(head[1..].try_into().unwrap()) as usize;
        total = total.checked_add(5 + len).ok_or_else(invalid)?;
        if len > MAX_FRAME || total > MAX_STREAM {
            return Err(invalid());
        }
        let mut body = vec![0; len];
        input.read_exact(&mut body)?;
        match (head[0], len) {
            (1, _) => {
                let link: MftLink = serde_json::from_slice(&body).map_err(|_| invalid())?;
                if !link.dir {
                    count += 1;
                }
                links.push(link)?;
            }
            (2, 0) => return links.finish(|| pulse(count)),
            (3, 1..=64) => return Err(io::Error::other("MFT helper rejected scan")),
            (4, 0) => {}
            _ => return Err(invalid()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn record(id: u64, parent: u64, name: &str, dir: bool, size: u64) -> MftLink {
        MftLink {
            id,
            parent,
            name: name.into(),
            dir,
            size,
        }
    }
    fn stream(records: &[MftLink]) -> Vec<u8> {
        let mut b = MAGIC.to_vec();
        frame(&mut b, 4, &[]).unwrap();
        for r in records {
            frame(&mut b, 1, &serde_json::to_vec(r).unwrap()).unwrap();
        }
        frame(&mut b, 2, &[]).unwrap();
        b
    }
    #[test]
    fn requests_authorize_only_whole_drive_roots() {
        assert_eq!(request("C:\\"), Some(*b"FM01C\0\0\0"));
        for root in [
            "C:",
            "c:\\",
            "C:\\Users",
            "C:\\..\\",
            "\\\\server\\share",
            "\\\\.\\C:",
            "C:\\x\0",
            "Z:\\\\",
        ] {
            assert!(request(root).is_none(), "{root:?}");
        }
    }
    #[test]
    fn incomplete_version_error_and_oversized_responses_fail() {
        for b in [
            b"FM00".to_vec(),
            b"FM01".to_vec(),
            b"FM01\x09\0\0\0\0".to_vec(),
            b"FM01\x01\x01\x10\0\0".to_vec(),
            b"FM01\x03\x01\0\0\0x".to_vec(),
            b"FM01\x02\x01\0\0\0x".to_vec(),
        ] {
            assert!(receive(&mut b.as_slice(), |_| Ok(())).is_err());
        }
    }
    #[test]
    fn malformed_json_unknown_fields_and_path_names_fail() {
        for data in [
            b"{}".as_slice(),
            br#"{"parent":5,"id":20,"name":"a","dir":false,"size":1,"op":"write"}"#,
            br#"{"parent":5,"id":20,"name":"../a","dir":false,"size":1}"#,
        ] {
            let mut b = MAGIC.to_vec();
            frame(&mut b, 1, data).unwrap();
            frame(&mut b, 2, &[]).unwrap();
            assert!(receive(&mut b.as_slice(), |_| Ok(())).is_err());
        }
    }
    #[test]
    fn corrupt_directory_cycles_and_duplicate_ids_are_rejected() {
        for records in [
            vec![record(20, 21, "a", true, 0), record(21, 20, "b", true, 0)],
            vec![record(20, 5, "a", true, 0), record(20, 5, "b", true, 0)],
        ] {
            assert!(receive(&mut stream(&records).as_slice(), |_| Ok(())).is_err());
        }
    }
    #[test]
    fn excessive_names_sizes_and_conflicting_identities_fail() {
        let too_large = record(20, 5, &"a".repeat(1025), false, 1);
        assert!(receive(&mut stream(&[too_large]).as_slice(), |_| Ok(())).is_err());
        let mut overflow = Vec::new();
        for id in 20..36 {
            overflow.push(record(id, 5, "a", false, 1 << 60));
        }
        assert!(receive(&mut stream(&overflow).as_slice(), |_| Ok(())).is_err());
        for records in [
            vec![record(20, 5, "a", true, 0), record(20, 5, "b", false, 1)],
            vec![record(20, 5, "a", false, 1), record(20, 5, "b", true, 0)],
        ] {
            assert!(receive(&mut stream(&records).as_slice(), |_| Ok(())).is_err());
        }
    }
    #[test]
    fn cancellation_and_timeout_abort_without_a_partial_tree() {
        let b = stream(&[record(20, 5, "a", false, 300_000)]);
        for kind in [io::ErrorKind::Interrupted, io::ErrorKind::TimedOut] {
            let e = receive(&mut b.as_slice(), |_| Err(io::Error::new(kind, "stop"))).unwrap_err();
            assert_eq!(e.kind(), kind);
        }
    }
}
