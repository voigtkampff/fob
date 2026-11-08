//! One sample from a process list plus a metrics reader. OS-independent so the
//! Windows collector and Linux fixtures run the same code.
use crate::model::{Entry, Identity, Obs, Role, Sample};
use crate::tree::select;

pub struct Read {
    pub create: u64,
    pub cpu: u64,
    pub private_bytes: Option<u64>,
    pub working_set: Option<u64>,
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
        let mut o = Obs { id: *id, ppid: *ppid, name: name.clone(), role, cpu: None, private_bytes: None, working_set: None, error: None };
        match r.read(id.pid) {
            Ok(m) if m.create == id.create => {
                o.cpu = Some(m.cpu);
                o.private_bytes = m.private_bytes;
                o.working_set = m.working_set;
                if m.private_bytes.is_none() {
                    o.error = Some("memory counters unreadable".into());
                }
            }
            Ok(_) => o.error = Some("pid reused during sample".into()),
            Err(e) => o.error = Some(e),
        }
        procs.push(o);
    }
    for (pid, name, e) in &sel.errors {
        procs.push(Obs { id: Identity { pid: *pid, create: 0 }, ppid: 0, name: name.clone(), role: Role::Child, cpu: None, private_bytes: None, working_set: None, error: Some(e.clone()) });
    }
    let keep: Vec<Identity> = sel.members.iter().map(|m| m.0).collect();
    (Sample { seq, t_ns, wall, root: sel.root, procs, excluded: sel.excluded, discovery_error: None }, keep)
}
