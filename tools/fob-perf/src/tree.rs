//! Which processes belong to the Fob being measured.
use crate::model::{Entry, Excluded, Identity, RootState, HELPERS};
use std::collections::{BTreeMap, HashMap, HashSet};

pub struct Selection {
    pub root: RootState,
    /// (identity, parent pid, name, is_root)
    pub members: Vec<(Identity, u32, String, bool)>,
    pub excluded: Vec<Excluded>,
    pub errors: Vec<(u32, String, String)>,
}

/// Fob roots: fob.exe whose parent is not another fob.exe.
pub fn find_roots(entries: &[Entry], root_name: &str) -> Vec<u32> {
    let by_pid: HashMap<u32, &Entry> = entries.iter().map(|e| (e.pid, e)).collect();
    entries
        .iter()
        .filter(|e| e.name.eq_ignore_ascii_case(root_name))
        .filter(|e| by_pid.get(&e.ppid).map_or(true, |p| !p.name.eq_ignore_ascii_case(root_name)))
        .map(|e| e.pid)
        .collect()
}

/// Walk descendants of `root`. A child counts only if its creation time is not
/// earlier than its parent's (a stale ppid pointing at a reused pid fails this).
/// `create_of` returns the creation time or an error string (e.g. access denied).
pub fn select(
    root: Identity,
    entries: &[Entry],
    mut create_of: impl FnMut(u32) -> Result<u64, String>,
) -> Selection {
    let mut sel = Selection { root: RootState::Alive, members: vec![], excluded: vec![], errors: vec![] };
    let Some(r) = entries.iter().find(|e| e.pid == root.pid) else {
        sel.root = RootState::Exited;
        return sel;
    };
    if !r.name.eq_ignore_ascii_case("fob.exe") {
        sel.root = RootState::PidReused;
        return sel;
    }
    match create_of(root.pid) {
        Ok(c) if c == root.create => {}
        Ok(_) => {
            sel.root = RootState::PidReused;
            return sel;
        }
        Err(e) => {
            sel.root = RootState::Unreadable;
            sel.errors.push((root.pid, r.name.clone(), e));
            return sel;
        }
    }
    sel.members.push((root, r.ppid, r.name.clone(), true));
    let mut kids: BTreeMap<u32, Vec<&Entry>> = BTreeMap::new();
    for e in entries {
        if e.pid != e.ppid {
            kids.entry(e.ppid).or_default().push(e);
        }
    }
    let mut stack = vec![root];
    let mut seen = HashSet::from([root.pid]);
    while let Some(parent) = stack.pop() {
        for k in kids.get(&parent.pid).into_iter().flatten() {
            if !seen.insert(k.pid) {
                continue;
            }
            if HELPERS.iter().any(|h| k.name.eq_ignore_ascii_case(h)) {
                sel.excluded.push(Excluded { pid: k.pid, name: k.name.clone(), reason: "helper".into() });
                continue;
            }
            if !k.name.eq_ignore_ascii_case("msedgewebview2.exe") {
                continue;
            }
            match create_of(k.pid) {
                Ok(c) if c >= parent.create => {
                    let id = Identity { pid: k.pid, create: c };
                    sel.members.push((id, k.ppid, k.name.clone(), false));
                    stack.push(id);
                }
                Ok(_) => sel.excluded.push(Excluded { pid: k.pid, name: k.name.clone(), reason: "older_than_parent".into() }),
                Err(e) => sel.errors.push((k.pid, k.name.clone(), e)),
            }
        }
    }
    // Helpers run as a separate elevated task, not as children: report them.
    for e in entries {
        if HELPERS.iter().any(|h| e.name.eq_ignore_ascii_case(h)) && !sel.excluded.iter().any(|x| x.pid == e.pid) {
            sel.excluded.push(Excluded { pid: e.pid, name: e.name.clone(), reason: "helper".into() });
        }
    }
    sel
}
