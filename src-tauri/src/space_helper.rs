//! Authenticated, nonblocking client for the installed elevated helper.
#![cfg_attr(not(windows), allow(dead_code))]
use crate::space::{Node, Progress};
use std::io::{self, Read};
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

struct Input<'a, R> {
    pipe: R,
    progress: &'a Progress,
    started: Instant,
    received: Instant,
    idle: Duration,
    total: Duration,
}
impl<R: Read> Read for Input<'_, R> {
    fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
        loop {
            if self.progress.cancel.load(Ordering::Relaxed) {
                return Err(io::Error::other("cancelled"));
            }
            if self.started.elapsed() > self.total || self.received.elapsed() > self.idle {
                return Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    "MFT helper timed out",
                ));
            }
            match self.pipe.read(out) {
                Ok(n) => {
                    self.received = Instant::now();
                    return Ok(n);
                }
                Err(e)
                    if e.raw_os_error() == Some(232) || e.kind() == io::ErrorKind::WouldBlock =>
                {
                    std::thread::sleep(Duration::from_millis(20));
                }
                Err(e) => return Err(e),
            }
        }
    }
}

pub fn try_scan(root: &std::path::Path, progress: &Progress) -> Option<Node> {
    #[cfg(windows)]
    {
        win::scan(root, progress).ok()
    }
    #[cfg(not(windows))]
    {
        let _ = (root, progress);
        None
    }
}

#[cfg(windows)]
mod win {
    use super::*;
    use crate::{mft_wire, sensors::setup};
    use std::ffi::c_void;
    use std::io::{self, Write};
    use std::os::windows::io::{AsRawHandle, FromRawHandle};
    use std::sync::atomic::Ordering;
    use std::time::{Duration, Instant};
    type Raw = *mut c_void;
    struct Handle(Raw);
    impl Drop for Handle {
        fn drop(&mut self) {
            unsafe {
                CloseHandle(self.0);
            }
        }
    }
    fn failed() -> io::Error {
        io::Error::other("MFT helper unavailable or incompatible")
    }
    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(Some(0)).collect()
    }
    fn token(process: Raw) -> io::Result<Handle> {
        let mut t = std::ptr::null_mut();
        if unsafe { OpenProcessToken(process, 8, &mut t) } == 0 {
            return Err(failed());
        }
        Ok(Handle(t))
    }
    fn user(token: &Handle) -> io::Result<Vec<usize>> {
        let mut needed = 0;
        unsafe {
            GetTokenInformation(token.0, 1, std::ptr::null_mut(), 0, &mut needed);
        }
        if needed == 0 || needed > 65536 {
            return Err(failed());
        }
        // TokenUser starts with an aligned SID pointer.
        let mut data = vec![0usize; (needed as usize).div_ceil(std::mem::size_of::<usize>())];
        if unsafe { GetTokenInformation(token.0, 1, data.as_mut_ptr().cast(), needed, &mut needed) }
            == 0
        {
            return Err(failed());
        }
        Ok(data)
    }
    fn sid_text(user: &[usize]) -> io::Result<String> {
        let mut text = std::ptr::null_mut();
        if unsafe { ConvertSidToStringSidW(user[0] as Raw, &mut text) } == 0 {
            return Err(failed());
        }
        let mut len = 0;
        unsafe {
            while *text.add(len) != 0 {
                len += 1;
            }
            let s = String::from_utf16_lossy(std::slice::from_raw_parts(text, len));
            LocalFree(text.cast());
            Ok(s)
        }
    }
    fn authenticate(pipe: &std::fs::File, own: &[usize]) -> io::Result<()> {
        let mut pid = 0;
        if unsafe { GetNamedPipeServerProcessId(pipe.as_raw_handle(), &mut pid) } == 0 {
            return Err(failed());
        }
        let process = Handle(unsafe { OpenProcess(0x1000, 0, pid) });
        if process.0.is_null() {
            return Err(failed());
        }
        let mut path = vec![0u16; 32768];
        let mut len = path.len() as u32;
        if unsafe { QueryFullProcessImageNameW(process.0, 0, path.as_mut_ptr(), &mut len) } == 0 {
            return Err(failed());
        }
        let path = String::from_utf16_lossy(&path[..len as usize]);
        if !path.eq_ignore_ascii_case(&setup::installed_helper().to_string_lossy()) {
            return Err(failed());
        }
        let token = token(process.0)?;
        let server_user = user(&token)?;
        if unsafe { EqualSid(own[0] as Raw, server_user[0] as Raw) } == 0 {
            return Err(failed());
        }
        let (mut elevated, mut size) = (0u32, 0u32);
        if unsafe {
            GetTokenInformation(
                token.0,
                20,
                (&mut elevated as *mut u32).cast(),
                4,
                &mut size,
            )
        } == 0
            || elevated == 0
        {
            return Err(failed());
        }
        let (mut server_session, mut own_session) = (0, 0);
        if unsafe { ProcessIdToSessionId(pid, &mut server_session) } == 0
            || unsafe { ProcessIdToSessionId(std::process::id(), &mut own_session) } == 0
            || server_session != own_session
        {
            return Err(failed());
        }
        Ok(())
    }
    pub fn scan(root: &std::path::Path, progress: &Progress) -> io::Result<Node> {
        let request = mft_wire::request(&root.to_string_lossy()).ok_or_else(failed)?;
        let dir = setup::installed_helper()
            .parent()
            .ok_or_else(failed)?
            .to_path_buf();
        // Reject an old/mixed installation before connecting. No setup or UAC here.
        setup::verify(&dir, setup::MANIFEST, "sensors/").map_err(|_| failed())?;
        let own = user(&token(unsafe { GetCurrentProcess() })?)?;
        let path = wide(&format!(r"\\.\pipe\fob-space-v1-{}", sid_text(&own)?));
        let raw = unsafe {
            CreateFileW(
                path.as_ptr(),
                0x00100183,
                0,
                std::ptr::null(),
                3,
                0x00110000,
                std::ptr::null_mut(),
            )
        };
        if raw as isize == -1 {
            return Err(failed());
        }
        let mut pipe = unsafe { std::fs::File::from_raw_handle(raw) };
        authenticate(&pipe, &own)?;
        let mode = 1u32; // PIPE_NOWAIT: cancellation never waits on a blocking read.
        if unsafe {
            SetNamedPipeHandleState(
                pipe.as_raw_handle(),
                &mode,
                std::ptr::null(),
                std::ptr::null(),
            )
        } == 0
        {
            return Err(failed());
        }
        pipe.write_all(&request)?;
        let now = Instant::now();
        let mut input = Input {
            pipe,
            progress,
            started: now,
            received: now,
            idle: Duration::from_secs(mft_wire::IDLE_SECS),
            total: Duration::from_secs(mft_wire::TOTAL_SECS),
        };
        if let Ok(mut current) = progress.current.lock() {
            *current = format!("{} [MFT]", root.display());
        }
        let links = mft_wire::receive(
            &mut std::io::BufReader::with_capacity(65536, &mut input),
            |count| {
                if progress.cancel.load(Ordering::Relaxed) {
                    return Err(io::Error::other("cancelled"));
                }
                progress.files.store(count as u64, Ordering::Relaxed);
                Ok(())
            },
        );
        // Closing the duplex pipe cancels the server's worker on every exit path.
        drop(input);
        progress.files.store(0, Ordering::Relaxed);
        let links = links?;
        crate::space_mft::build_tree(5, root.to_string_lossy().into_owned(), &links, progress)
            .ok_or_else(failed)
    }

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn CloseHandle(h: Raw) -> i32;
        fn GetCurrentProcess() -> Raw;
        fn OpenProcess(access: u32, inherit: i32, pid: u32) -> Raw;
        fn QueryFullProcessImageNameW(
            process: Raw,
            flags: u32,
            path: *mut u16,
            size: *mut u32,
        ) -> i32;
        fn ProcessIdToSessionId(pid: u32, session: *mut u32) -> i32;
        fn GetNamedPipeServerProcessId(pipe: Raw, pid: *mut u32) -> i32;
        fn CreateFileW(
            name: *const u16,
            access: u32,
            share: u32,
            security: *const c_void,
            disposition: u32,
            flags: u32,
            template: Raw,
        ) -> Raw;
        fn SetNamedPipeHandleState(
            pipe: Raw,
            mode: *const u32,
            max: *const u32,
            timeout: *const u32,
        ) -> i32;
        fn LocalFree(data: Raw) -> Raw;
    }
    #[link(name = "advapi32")]
    unsafe extern "system" {
        fn OpenProcessToken(process: Raw, access: u32, token: *mut Raw) -> i32;
        fn GetTokenInformation(
            token: Raw,
            class: u32,
            data: Raw,
            len: u32,
            needed: *mut u32,
        ) -> i32;
        fn EqualSid(a: Raw, b: Raw) -> i32;
        fn ConvertSidToStringSidW(sid: Raw, text: *mut *mut u16) -> i32;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Pending;
    impl Read for Pending {
        fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
            Err(io::Error::from(io::ErrorKind::WouldBlock))
        }
    }
    fn pending(progress: &Progress) -> Input<'_, Pending> {
        let now = Instant::now();
        Input {
            pipe: Pending,
            progress,
            started: now,
            received: now,
            idle: Duration::from_millis(1),
            total: Duration::from_secs(1),
        }
    }
    #[test]
    fn stalled_server_times_out_and_cancel_does_not_wait_for_data() {
        let p = Progress::default();
        assert_eq!(
            pending(&p).read(&mut [0]).unwrap_err().kind(),
            io::ErrorKind::TimedOut
        );
        p.cancel.store(true, Ordering::Relaxed);
        assert!(pending(&p).read(&mut [0]).is_err());
    }
    #[test]
    fn total_deadline_bounds_a_server_that_keeps_responding() {
        let p = Progress::default();
        let mut input = pending(&p);
        input.started = Instant::now() - Duration::from_secs(2);
        assert_eq!(
            input.read(&mut [0]).unwrap_err().kind(),
            io::ErrorKind::TimedOut
        );
    }
    #[test]
    fn cancellation_while_waiting_is_observed_without_a_reader_thread() {
        let p = Progress::default();
        std::thread::scope(|scope| {
            let job = scope.spawn(|| {
                let mut input = pending(&p);
                input.idle = Duration::from_secs(1);
                input.read(&mut [0])
            });
            p.cancel.store(true, Ordering::Relaxed);
            assert!(job.join().unwrap().is_err());
        });
    }
    #[cfg(not(windows))]
    #[test]
    fn unavailable_helper_never_prevents_existing_walk() {
        let root = std::env::temp_dir().join(format!("fob-old-helper-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join("test"), vec![0u8; 300_000]).unwrap();
        let (tree, threads, method) = crate::space::scan_root(
            &root,
            &Progress::default(),
            crate::space::DriveKind::Unknown,
        );
        assert_eq!(method, "walk");
        assert_eq!(threads, 1);
        assert_eq!(tree.unwrap().files, 1);
        std::fs::remove_dir_all(root).unwrap();
    }
}
