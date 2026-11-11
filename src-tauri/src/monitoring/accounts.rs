//! Readable owner names for processes.
//!
//! On Windows sysinfo reports a process's owner as a SID string such as
//! `S-1-5-18`. Task Manager shows the account name instead: SYSTEM, LOCAL
//! SERVICE, NETWORK SERVICE or the user's name. A lookup can be slow (a
//! domain account asks the domain controller), so it never runs on the
//! refresh path: the first time a SID is seen it is queued for one
//! background worker and the SID itself is shown until the name arrives.
//! Each SID is looked up once and the answer kept for the rest of the run,
//! failures included, so repeated owners and a churn of short-lived
//! processes cost one map lookup each. A SID that maps to no account, or
//! whose lookup fails, keeps its SID text. Nothing about a lookup is logged.

use std::collections::HashMap;
use std::sync::mpsc::{sync_channel, SyncSender};
use std::sync::{Arc, Mutex, PoisonError};

/// Why a SID shows as its own text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unresolved {
    /// No account has this SID (a deleted user, a foreign domain).
    NoAccount,
    /// The SID was malformed or the lookup itself failed.
    Failed,
}

pub type Lookup = fn(&str) -> Result<String, Unresolved>;

/// SIDs waiting for the worker; a SID that finds the queue full is tried
/// again on a later refresh.
const QUEUE: usize = 64;

/// SID text to what the owner column shows; `None` while the lookup waits.
type Shown = Arc<Mutex<HashMap<String, Option<String>>>>;

#[derive(Debug)]
pub struct AccountNames {
    shown: Shown,
    /// Started with the first lookup; `None` until then or if the worker
    /// thread could not start (owners then stay SIDs).
    queue: Option<SyncSender<String>>,
    started: bool,
    lookup: Lookup,
}

impl AccountNames {
    pub fn new(lookup: Lookup) -> Self {
        Self {
            shown: Arc::default(),
            queue: None,
            started: false,
            lookup,
        }
    }

    /// What the owner column shows for `sid` now. Never waits for a lookup.
    pub fn name(&mut self, sid: &str) -> String {
        let shown = Arc::clone(&self.shown);
        let mut shown = shown.lock().unwrap_or_else(PoisonError::into_inner);
        match shown.get(sid) {
            Some(Some(name)) => return name.clone(),
            Some(None) => return sid.to_string(),
            None => {}
        }
        if self
            .queue()
            .is_some_and(|queue| queue.try_send(sid.to_string()).is_ok())
        {
            shown.insert(sid.to_string(), None);
        }
        sid.to_string()
    }

    fn queue(&mut self) -> Option<&SyncSender<String>> {
        if !self.started {
            self.started = true;
            let (send, receive) = sync_channel::<String>(QUEUE);
            let shown = Arc::clone(&self.shown);
            let lookup = self.lookup;
            self.queue = std::thread::Builder::new()
                .name("fob-accounts".into())
                .spawn(move || {
                    for sid in receive {
                        let name = lookup(&sid)
                            .ok()
                            .filter(|n| !n.is_empty())
                            .unwrap_or_else(|| sid.clone());
                        shown
                            .lock()
                            .unwrap_or_else(PoisonError::into_inner)
                            .insert(sid, Some(name));
                    }
                })
                .ok()
                .map(|_| send);
        }
        self.queue.as_ref()
    }
}

impl Default for AccountNames {
    fn default() -> Self {
        Self::new(lookup)
    }
}

/// The account name of `sid`, without its domain, as Task Manager shows it.
#[cfg(windows)]
pub fn lookup(sid: &str) -> Result<String, Unresolved> {
    use std::ffi::c_void;
    type Raw = *mut c_void;
    const ERROR_NONE_MAPPED: i32 = 1332;

    let text: Vec<u16> = sid.encode_utf16().chain(Some(0)).collect();
    let mut raw: Raw = std::ptr::null_mut();
    if unsafe { ConvertStringSidToSidW(text.as_ptr(), &mut raw) } == 0 {
        return Err(Unresolved::Failed);
    }
    let mut name = [0u16; 256];
    let mut name_len = name.len() as u32;
    let mut domain = [0u16; 256];
    let mut domain_len = domain.len() as u32;
    let mut kind = 0u32;
    let found = unsafe {
        LookupAccountSidW(
            std::ptr::null(),
            raw,
            name.as_mut_ptr(),
            &mut name_len,
            domain.as_mut_ptr(),
            &mut domain_len,
            &mut kind,
        )
    } != 0;
    let error = std::io::Error::last_os_error().raw_os_error();
    unsafe { LocalFree(raw) };
    if found {
        let len = (name_len as usize).min(name.len());
        return Ok(String::from_utf16_lossy(&name[..len]));
    }
    Err(if error == Some(ERROR_NONE_MAPPED) {
        Unresolved::NoAccount
    } else {
        Unresolved::Failed
    })
}

/// Elsewhere sysinfo already reports a plain user id; it is shown as is.
#[cfg(not(windows))]
pub fn lookup(_id: &str) -> Result<String, Unresolved> {
    Err(Unresolved::NoAccount)
}

#[cfg(windows)]
#[link(name = "advapi32")]
unsafe extern "system" {
    fn ConvertStringSidToSidW(text: *const u16, sid: *mut *mut std::ffi::c_void) -> i32;
    fn LookupAccountSidW(
        system: *const u16,
        sid: *mut std::ffi::c_void,
        name: *mut u16,
        name_len: *mut u32,
        domain: *mut u16,
        domain_len: *mut u32,
        kind: *mut u32,
    ) -> i32;
}

#[cfg(windows)]
#[link(name = "kernel32")]
unsafe extern "system" {
    fn LocalFree(data: *mut std::ffi::c_void) -> *mut std::ffi::c_void;
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::{Duration, Instant};

    /// Generic accounts only: well-known SIDs and a made-up user.
    fn answer(sid: &str) -> Result<String, Unresolved> {
        match sid {
            "S-1-5-18" => Ok("SYSTEM".into()),
            "S-1-5-19" => Ok("LOCAL SERVICE".into()),
            "S-1-5-20" => Ok("NETWORK SERVICE".into()),
            "S-1-5-21-1000000001-1000000002-1000000003-1001" => Ok("sample-user".into()),
            "S-1-5-21-1000000001-1000000002-1000000003-1002" => Ok(String::new()),
            "not-a-sid" => Err(Unresolved::Failed),
            _ => Err(Unresolved::NoAccount),
        }
    }

    /// What `sid` shows once its lookup has finished.
    pub(crate) fn settled(names: &mut AccountNames, sid: &str) -> String {
        let until = Instant::now() + Duration::from_secs(5);
        loop {
            let pending = names
                .shown
                .lock()
                .unwrap()
                .get(sid)
                .is_none_or(Option::is_none);
            if !pending || Instant::now() > until {
                return names.name(sid);
            }
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    #[test]
    fn system_accounts_and_users_show_their_names() {
        let mut names = AccountNames::new(answer);
        for (sid, name) in [
            ("S-1-5-18", "SYSTEM"),
            ("S-1-5-19", "LOCAL SERVICE"),
            ("S-1-5-20", "NETWORK SERVICE"),
            (
                "S-1-5-21-1000000001-1000000002-1000000003-1001",
                "sample-user",
            ),
        ] {
            names.name(sid);
            assert_eq!(settled(&mut names, sid), name);
        }
    }

    #[test]
    fn a_sid_without_a_name_shows_the_sid() {
        let mut names = AccountNames::new(answer);
        for sid in [
            "S-1-5-21-1000000001-1000000002-1000000003-1999",
            "S-1-5-21-1000000001-1000000002-1000000003-1002",
            "not-a-sid",
        ] {
            names.name(sid);
            assert_eq!(settled(&mut names, sid), sid);
        }
    }

    #[test]
    fn a_slow_lookup_never_holds_up_a_refresh() {
        static CALLS: AtomicUsize = AtomicUsize::new(0);
        fn slow(sid: &str) -> Result<String, Unresolved> {
            CALLS.fetch_add(1, Ordering::SeqCst);
            std::thread::sleep(Duration::from_millis(400));
            answer(sid)
        }
        let mut names = AccountNames::new(slow);
        let started = Instant::now();
        for _ in 0..100 {
            assert_eq!(names.name("S-1-5-18"), "S-1-5-18");
        }
        assert!(
            started.elapsed() < Duration::from_millis(200),
            "refreshes waited {:?}",
            started.elapsed()
        );
        assert_eq!(settled(&mut names, "S-1-5-18"), "SYSTEM");
        assert_eq!(CALLS.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn each_sid_is_looked_up_once_even_when_it_fails() {
        static CALLS: AtomicUsize = AtomicUsize::new(0);
        fn counted(sid: &str) -> Result<String, Unresolved> {
            CALLS.fetch_add(1, Ordering::SeqCst);
            answer(sid)
        }
        let mut names = AccountNames::new(counted);
        let gone = "S-1-5-21-1000000001-1000000002-1000000003-1999";
        for _ in 0..50 {
            names.name("S-1-5-18");
            names.name(gone);
        }
        settled(&mut names, "S-1-5-18");
        settled(&mut names, gone);
        for _ in 0..50 {
            names.name("S-1-5-18");
            names.name(gone);
        }
        assert_eq!(CALLS.load(Ordering::SeqCst), 2);
    }

    #[cfg(windows)]
    #[test]
    fn windows_names_the_well_known_accounts() {
        for sid in ["S-1-5-18", "S-1-5-19", "S-1-5-20"] {
            let name = lookup(sid).expect("well-known SID resolves");
            assert!(
                !name.is_empty() && !name.starts_with("S-1-"),
                "{sid} -> {name}"
            );
        }
        assert_eq!(lookup("not-a-sid"), Err(Unresolved::Failed));
        assert_eq!(
            lookup("S-1-5-21-1000000001-1000000002-1000000003-1999"),
            Err(Unresolved::NoAccount)
        );
    }

    #[cfg(not(windows))]
    #[test]
    fn elsewhere_the_user_id_is_shown_as_is() {
        let mut names = AccountNames::default();
        names.name("1000");
        assert_eq!(settled(&mut names, "1000"), "1000");
    }
}
