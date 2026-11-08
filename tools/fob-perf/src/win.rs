//! Windows reads: Toolhelp process list (pid/ppid/name only), then per owned
//! process GetProcessTimes and GetProcessMemoryInfo. No WMI, no command lines.
use crate::model::{Entry, Identity};
use std::collections::HashMap;
use windows_sys::Win32::Foundation::{CloseHandle, FILETIME, HANDLE, INVALID_HANDLE_VALUE};
use windows_sys::Win32::System::Diagnostics::ToolHelp::{CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W, TH32CS_SNAPPROCESS};
use windows_sys::Win32::System::ProcessStatus::{K32GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS, PROCESS_MEMORY_COUNTERS_EX};
use windows_sys::Win32::System::SystemInformation::GetSystemTimeAsFileTime;
use windows_sys::Win32::System::Threading::{GetActiveProcessorCount, GetActiveProcessorGroupCount, GetCurrentProcess, GetProcessTimes, OpenProcess, QueryFullProcessImageNameW, PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_VM_READ};

const ALL_GROUPS: u16 = 0xFFFF;

fn ft(f: FILETIME) -> u64 {
    ((f.dwHighDateTime as u64) << 32) | f.dwLowDateTime as u64
}

pub fn now_filetime() -> u64 {
    let mut f = FILETIME { dwLowDateTime: 0, dwHighDateTime: 0 };
    unsafe { GetSystemTimeAsFileTime(&mut f) };
    ft(f)
}

pub fn cores() -> (u32, u32) {
    unsafe { (GetActiveProcessorCount(ALL_GROUPS), GetActiveProcessorGroupCount() as u32) }
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
        CloseHandle(snap);
        Ok(out)
    }
}

pub struct Metrics {
    pub create: u64,
    pub cpu: u64,
    pub private_bytes: Option<u64>,
    pub working_set: Option<u64>,
}

unsafe fn times(h: HANDLE) -> Result<(u64, u64), String> {
    let z = FILETIME { dwLowDateTime: 0, dwHighDateTime: 0 };
    let (mut c, mut x, mut k, mut u) = (z, z, z, z);
    if GetProcessTimes(h, &mut c, &mut x, &mut k, &mut u) == 0 {
        return Err(format!("GetProcessTimes: {}", std::io::Error::last_os_error()));
    }
    Ok((ft(c), ft(k) + ft(u)))
}

unsafe fn mem(h: HANDLE) -> Option<(u64, u64)> {
    let mut m: PROCESS_MEMORY_COUNTERS_EX = std::mem::zeroed();
    let sz = std::mem::size_of::<PROCESS_MEMORY_COUNTERS_EX>() as u32;
    m.cb = sz;
    (K32GetProcessMemoryInfo(h, &mut m as *mut _ as *mut PROCESS_MEMORY_COUNTERS, sz) != 0).then(|| (m.PrivateUsage as u64, m.WorkingSetSize as u64))
}

/// Keeps one handle per pid. A held handle also stops that pid being reused.
#[derive(Default)]
pub struct Handles(HashMap<u32, HANDLE>);

impl Handles {
    fn get(&mut self, pid: u32) -> Result<HANDLE, String> {
        if let Some(h) = self.0.get(&pid) {
            return Ok(*h);
        }
        let h = unsafe {
            let h = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_VM_READ, 0, pid);
            if h.is_null() { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) } else { h }
        };
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
    pub fn metrics(&mut self, pid: u32) -> Result<Metrics, String> {
        let h = self.get(pid)?;
        unsafe {
            let (create, cpu) = times(h)?;
            let m = mem(h);
            Ok(Metrics { create, cpu, private_bytes: m.map(|m| m.0), working_set: m.map(|m| m.1) })
        }
    }
    /// Drop handles for pids no longer of interest (or whose identity changed).
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
        K32GetProcessMemoryInfo(h, &mut m, m.cb);
        Some((cpu as f64 / 1e7, m.PeakWorkingSetSize as f64 / 1048576.0))
    }
}
