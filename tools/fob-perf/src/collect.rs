//! One sample from a process list plus a metrics reader. OS-independent so the
//! Windows collector and Linux fixtures run the same code.
use crate::model::{Entry, Identity, Obs, Role, Sample};
use crate::tree::select;

pub struct Read {
    pub create: u64,
    pub cpu: u64,
    pub cpu_t_ns: Option<u64>,
    pub private_bytes: Option<u64>,
    pub working_set: Option<u64>,
    pub memory_error: Option<String>,
}

pub trait Reader {
    fn create_time(&mut self, pid: u32) -> Result<u64, String>;
    fn read(&mut self, pid: u32) -> Result<Read, String>;
}

pub fn sample(seq: u64, t_ns: u64, wall: u64, root: Identity, entries: Result<Vec<Entry>, String>, r: &mut dyn Reader) -> (Sample, Vec<Identity>) {
    let entries = match entries {
        Ok(e) => e,
        Err(e) => {
            let s = Sample { seq, t_ns, wall, root: crate::model::RootState::Unreadable, procs: vec![], excluded: vec![], discovery_error: Some(e) };
            return (s, vec![root]);
        }
    };
    let sel = select(root, &entries, |pid| r.create_time(pid));
    let mut procs = vec![];
    for (id, ppid, name, is_root) in &sel.members {
        let role = if *is_root { Role::Root } else { Role::Child };
        let mut o = Obs { id: *id, ppid: *ppid, name: name.clone(), role, cpu: None, cpu_t_ns: t_ns, private_bytes: None, working_set: None, error: None };
        match r.read(id.pid) {
            Ok(m) if m.create == id.create => {
                o.cpu = Some(m.cpu);
                o.cpu_t_ns = m.cpu_t_ns.unwrap_or(t_ns);
                o.private_bytes = m.private_bytes;
                o.working_set = m.working_set;
                if m.private_bytes.is_none() || m.working_set.is_none() {
                    o.error = Some(m.memory_error.unwrap_or_else(|| "memory counters unreadable".into()));
                }
            }
            Ok(_) => o.error = Some("pid reused during sample".into()),
            Err(e) => o.error = Some(e),
        }
        procs.push(o);
    }
    for (pid, name, e) in &sel.errors {
        procs.push(Obs { id: if *pid == root.pid { root } else { Identity { pid: *pid, create: 0 } }, ppid: 0, name: name.clone(), role: if *pid == root.pid { Role::Root } else { Role::Child }, cpu: None, cpu_t_ns: t_ns, private_bytes: None, working_set: None, error: Some(e.clone()) });
    }
    let keep: Vec<Identity> = sel.members.iter().map(|m| m.0).collect();
    let state = if procs.iter().any(|o| o.role == Role::Root && o.cpu.is_none()) {
        crate::model::RootState::Unreadable
    } else { sel.root };
    (Sample { seq, t_ns, wall, root: state, procs, excluded: sel.excluded, discovery_error: None }, keep)
}

/// Smoke must validate discovery and root state, not just visible error rows.
pub fn smoke_ready(s: &Sample) -> bool {
    s.root == crate::model::RootState::Alive && s.discovery_error.is_none()
        && s.procs.iter().any(|o| o.role == Role::Root)
        && s.procs.iter().all(|o| o.error.is_none() && o.cpu.is_some()
            && o.private_bytes.is_some() && o.working_set.is_some())
}
