//! Readable owner names for processes.
//!
//! On Windows sysinfo reports a process's owner as a SID string such as
//! `S-1-5-18`. Task Manager shows the account name instead: SYSTEM, LOCAL
//! SERVICE, NETWORK SERVICE or the user's name. Each distinct SID is looked
//! up once, the first time a process with that owner appears, and the answer
//! is kept for the rest of the run, so a refresh never waits on a lookup for
//! an owner it has seen and a churn of short-lived processes costs nothing.
//! A SID that maps to no account, or whose lookup fails, keeps its SID text.
//! Nothing about a lookup is logged.

use std::collections::HashMap;

/// Why a SID shows as its own text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unresolved {
    /// No account has this SID (a deleted user, a foreign domain).
    NoAccount,
    /// The SID was malformed or the lookup itself failed.
    Failed,
}

pub type Lookup = fn(&str) -> Result<String, Unresolved>;

#[derive(Debug)]
pub struct AccountNames {
    /// SID text to what the owner column shows, failures included.
    shown: HashMap<String, String>,
    lookup: Lookup,
}

impl AccountNames {
    pub fn new(lookup: Lookup) -> Self {
        Self {
            shown: HashMap::new(),
            lookup,
        }
    }

    /// What the owner column shows for `sid`.
    pub fn name(&mut self, sid: &str) -> String {
        if let Some(shown) = self.shown.get(sid) {
            return shown.clone();
        }
        let shown = (self.lookup)(sid)
            .ok()
            .filter(|n| !n.is_empty())
            .unwrap_or_else(|| sid.to_string());
        self.shown.insert(sid.to_string(), shown.clone());
        shown
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
    use std::cell::Cell;

    thread_local! {
        static CALLS: Cell<usize> = const { Cell::new(0) };
    }

    /// Generic accounts only: well-known SIDs and a made-up user.
    fn fake(sid: &str) -> Result<String, Unresolved> {
        CALLS.with(|c| c.set(c.get() + 1));
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

    #[test]
    fn system_accounts_and_users_show_their_names() {
        let mut names = AccountNames::new(fake);
        assert_eq!(names.name("S-1-5-18"), "SYSTEM");
        assert_eq!(names.name("S-1-5-19"), "LOCAL SERVICE");
        assert_eq!(names.name("S-1-5-20"), "NETWORK SERVICE");
        assert_eq!(
            names.name("S-1-5-21-1000000001-1000000002-1000000003-1001"),
            "sample-user"
        );
    }

    #[test]
    fn a_sid_without_a_name_shows_the_sid() {
        let mut names = AccountNames::new(fake);
        let gone = "S-1-5-21-1000000001-1000000002-1000000003-1999";
        assert_eq!(names.name(gone), gone);
        let blank = "S-1-5-21-1000000001-1000000002-1000000003-1002";
        assert_eq!(names.name(blank), blank);
        assert_eq!(names.name("not-a-sid"), "not-a-sid");
    }

    #[test]
    fn each_sid_is_looked_up_once_even_when_it_fails() {
        CALLS.with(|c| c.set(0));
        let mut names = AccountNames::new(fake);
        for _ in 0..50 {
            names.name("S-1-5-18");
            names.name("S-1-5-21-1000000001-1000000002-1000000003-1999");
        }
        assert_eq!(CALLS.with(Cell::get), 2);
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
        assert_eq!(names.name("1000"), "1000");
    }
}
