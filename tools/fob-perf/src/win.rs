//! Windows reads: Toolhelp process list (pid/ppid/name only), then per owned
//! process GetProcessTimes and GetProcessMemoryInfo. No WMI, no command lines.
use crate::model::{Entry, Identity};
use std::collections::HashMap;
use windows_sys::Win32::Foundation::{CloseHandle, FILETIME, HANDLE, INVALID_HANDLE_VALUE, GetLastError, ERROR_NO_MORE_FILES, STILL_ACTIVE};
use windows_sys::Win32::System::Diagnostics::ToolHelp::{CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W, TH32CS_SNAPPROCESS};
use windows_sys::Win32::System::ProcessStatus::{K32GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS, PROCESS_MEMORY_COUNTERS_EX};
use windows_sys::Win32::System::SystemInformation::GetSystemTimeAsFileTime;
use windows_sys::Win32::System::Threading::{GetActiveProcessorCount, GetActiveProcessorGroupCount, GetCurrentProcess, GetExitCodeProcess, GetProcessTimes, OpenProcess, QueryFullProcessImageNameW, PROCESS_QUERY_LIMITED_INFORMATION};

const ALL_GROUPS: u16 = 0xFFFF;

fn ft(f: FILETIME) -> u64 {
    ((f.dwHighDateTime as u64) << 32) | f.dwLowDateTime as u64
}

pub fn now_filetime() -> u64 {
    let mut f = FILETIME { dwLowDateTime: 0, dwHighDateTime: 0 };
    unsafe { GetSystemTimeAsFileTime(&mut f) };
    ft(f)
}

pub fn cores() -> Result<(u32, u32), String> {
    let (cores, groups) = unsafe { (GetActiveProcessorCount(ALL_GROUPS), GetActiveProcessorGroupCount() as u32) };
    if cores == 0 || groups == 0 { return Err("active processor topology unavailable".into()); }
    Ok((cores, groups))
}

pub fn list() -> Result<Vec<Entry>, String> {
    unsafe {
        let snap = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
        if snap == INVALID_HANDLE_VALUE {
            return Err(format!("CreateToolhelp32Snapshot failed: {}", std::io::Error::last_os_error()));
        }
        let mut e: PROCESSENTRY32W = std::mem::zeroed();
        e.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;
        let mut out = vec![];
        let mut ok = Process32FirstW(snap, &mut e);
        while ok != 0 {
            let n = e.szExeFile.iter().position(|&c| c == 0).unwrap_or(e.szExeFile.len());
            out.push(Entry { pid: e.th32ProcessID, ppid: e.th32ParentProcessID, name: String::from_utf16_lossy(&e.szExeFile[..n]) });
            ok = Process32NextW(snap, &mut e);
        }
        let error = GetLastError();
        CloseHandle(snap);
        if error != ERROR_NO_MORE_FILES {
            return Err(format!("process enumeration failed: {}", std::io::Error::from_raw_os_error(error as i32)));
        }
        Ok(out)
    }
}

pub struct Metrics {
    pub create: u64,
    pub cpu: u64,
    pub cpu_t_ns: u64,
    pub private_bytes: Option<u64>,
    pub working_set: Option<u64>,
    pub memory_error: Option<String>,
}

unsafe fn times(h: HANDLE) -> Result<(u64, u64), String> {
    let z = FILETIME { dwLowDateTime: 0, dwHighDateTime: 0 };
    let (mut c, mut x, mut k, mut u) = (z, z, z, z);
    if GetProcessTimes(h, &mut c, &mut x, &mut k, &mut u) == 0 {
        return Err(format!("GetProcessTimes: {}", std::io::Error::last_os_error()));
    }
    let mut code = 0;
    if GetExitCodeProcess(h, &mut code) == 0 {
        return Err(format!("GetExitCodeProcess: {}", std::io::Error::last_os_error()));
    }
    if code != STILL_ACTIVE as u32 { return Err("process exited during read".into()); }
    Ok((ft(c), ft(k) + ft(u)))
}

unsafe fn mem(h: HANDLE) -> Result<(u64, u64), String> {
    let mut m: PROCESS_MEMORY_COUNTERS_EX = std::mem::zeroed();
    let sz = std::mem::size_of::<PROCESS_MEMORY_COUNTERS_EX>() as u32;
    m.cb = sz;
    if K32GetProcessMemoryInfo(h, &mut m as *mut _ as *mut PROCESS_MEMORY_COUNTERS, sz) == 0 {
        return Err(format!("K32GetProcessMemoryInfo: {}", std::io::Error::last_os_error()));
    }
    Ok((m.PrivateUsage as u64, m.WorkingSetSize as u64))
}

/// Keeps one handle per pid. A held handle also stops that pid being reused.
#[derive(Default)]
pub struct Handles(HashMap<u32, HANDLE>);

impl Handles {
    fn get(&mut self, pid: u32) -> Result<HANDLE, String> {
        if let Some(h) = self.0.get(&pid) {
            return Ok(*h);
        }
        let h = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) };
        if h.is_null() {
            return Err(format!("OpenProcess: {}", std::io::Error::last_os_error()));
        }
        self.0.insert(pid, h);
        Ok(h)
    }
    pub fn create_time(&mut self, pid: u32) -> Result<u64, String> {
        let h = self.get(pid)?;
        unsafe { times(h).map(|t| t.0) }
    }
    pub fn metrics(&mut self, pid: u32, clock: &std::time::Instant) -> Result<Metrics, String> {
        let h = self.get(pid)?;
        unsafe {
            let before = clock.elapsed().as_nanos() as u64;
            let (create, cpu) = times(h)?;
            let after = clock.elapsed().as_nanos() as u64;
            let cpu_t_ns = before + (after - before) / 2;
            let m = mem(h);
            Ok(Metrics { create, cpu, cpu_t_ns, private_bytes: m.as_ref().ok().map(|m| m.0), working_set: m.as_ref().ok().map(|m| m.1), memory_error: m.err() })
        }
    }
    /// Drop handles for pids no longer selected.
    pub fn retain(&mut self, keep: &[Identity]) {
        self.0.retain(|pid, h| {
            let k = keep.iter().any(|i| i.pid == *pid);
            if !k {
                unsafe { CloseHandle(*h) };
            }
            k
        });
    }
    pub fn image_path(&mut self, pid: u32) -> Option<String> {
        let h = self.get(pid).ok()?;
        let mut buf = vec![0u16; 32768];
        let mut n = buf.len() as u32;
        (unsafe { QueryFullProcessImageNameW(h, 0, buf.as_mut_ptr(), &mut n) } != 0).then(|| String::from_utf16_lossy(&buf[..n as usize]))
    }
}

pub fn self_usage() -> Option<(f64, f64)> {
    unsafe {
        let h = GetCurrentProcess();
        let (_, cpu) = times(h).ok()?;
        let mut m: PROCESS_MEMORY_COUNTERS = std::mem::zeroed();
        m.cb = std::mem::size_of::<PROCESS_MEMORY_COUNTERS>() as u32;
        if K32GetProcessMemoryInfo(h, &mut m, m.cb) == 0 { return None; }
        Some((cpu as f64 / 1e7, m.PeakWorkingSetSize as f64 / 1048576.0))
    }
}

impl Drop for Handles {
    fn drop(&mut self) {
        for h in self.0.values() { unsafe { CloseHandle(*h); } }
    }
}
