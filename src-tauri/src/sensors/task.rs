//! The `fob-sensors` scheduled task through the Task Scheduler COM API
//! (ITaskService) instead of spawning schtasks.exe: no child processes, which
//! behaviour-based antivirus tends to read as "execution / persistence".
#![cfg(target_os = "windows")]

use windows::core::{BSTR, HRESULT};
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CoUninitialize, CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED,
};
use windows::Win32::System::TaskScheduler::{
    ITaskFolder, ITaskService, TaskScheduler, TASK_CREATE_OR_UPDATE, TASK_LOGON_INTERACTIVE_TOKEN,
};
use windows::Win32::System::Variant::VARIANT;

/// HRESULT_FROM_WIN32(ERROR_FILE_NOT_FOUND): no such task.
const NOT_FOUND: HRESULT = HRESULT(0x8007_0002u32 as i32);

#[derive(Debug, PartialEq)]
pub enum Query {
    Xml(String),
    Missing,
    /// Access denied or the service isn't reachable.
    Unknown,
}

struct Com(bool);
impl Com {
    fn init() -> Self {
        // S_OK / S_FALSE need a matching CoUninitialize; RPC_E_CHANGED_MODE
        // (thread already apartment-threaded) is fine to use as is.
        Com(unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) }.is_ok())
    }
}
impl Drop for Com {
    fn drop(&mut self) {
        if self.0 {
            unsafe { CoUninitialize() };
        }
    }
}

fn with_root<T>(
    f: impl FnOnce(&ITaskFolder) -> windows::core::Result<T>,
) -> windows::core::Result<T> {
    let _com = Com::init();
    unsafe {
        let service: ITaskService = CoCreateInstance(&TaskScheduler, None, CLSCTX_INPROC_SERVER)?;
        let none = VARIANT::default();
        service.Connect(&none, &none, &none, &none)?;
        let root = service.GetFolder(&BSTR::from("\\"))?;
        f(&root)
    }
}

pub fn query(name: &str) -> Query {
    match with_root(|root| unsafe { root.GetTask(&BSTR::from(name))?.Xml() }) {
        Ok(xml) => Query::Xml(xml.to_string()),
        Err(e) if e.code() == NOT_FOUND => Query::Missing,
        Err(_) => Query::Unknown,
    }
}

/// Create or replace the task from its XML (current user, interactive token).
pub fn register(name: &str, xml: &str) -> windows::core::Result<()> {
    with_root(|root| unsafe {
        let none = VARIANT::default();
        root.RegisterTask(
            &BSTR::from(name),
            &BSTR::from(xml),
            TASK_CREATE_OR_UPDATE.0,
            &none,
            &none,
            TASK_LOGON_INTERACTIVE_TOKEN,
            &none,
        )
        .map(|_| ())
    })
}

/// Start the task now (no-op if it is already running: IgnoreNew).
pub fn run(name: &str) -> windows::core::Result<()> {
    with_root(|root| unsafe {
        root.GetTask(&BSTR::from(name))?
            .Run(&VARIANT::default())
            .map(|_| ())
    })
}

/// Stop all running instances.
pub fn stop(name: &str) -> windows::core::Result<()> {
    with_root(|root| unsafe { root.GetTask(&BSTR::from(name))?.Stop(0) })
}

/// Delete the task; a task that is already gone counts as deleted.
pub fn delete(name: &str) -> windows::core::Result<()> {
    match with_root(|root| unsafe { root.DeleteTask(&BSTR::from(name), 0) }) {
        Err(e) if e.code() == NOT_FOUND => Ok(()),
        other => other,
    }
}

/// Stop the helper's ETW session if it is still around (the helper was
/// killed by `stop`, so it couldn't stop it itself). Needs admin.
pub fn stop_trace_session(name: &str) {
    use windows::core::HSTRING;
    use windows::Win32::System::Diagnostics::Etw::{
        ControlTraceW, CONTROLTRACE_HANDLE, EVENT_TRACE_CONTROL_STOP, EVENT_TRACE_PROPERTIES,
    };
    // Properties plus room for the logger and log file names it writes back.
    let size = std::mem::size_of::<EVENT_TRACE_PROPERTIES>() + 2 * 1024 * 2;
    let mut buffer = vec![0u64; size.div_ceil(8)];
    let props = buffer.as_mut_ptr() as *mut EVENT_TRACE_PROPERTIES;
    unsafe {
        (*props).Wnode.BufferSize = size as u32;
        (*props).LoggerNameOffset = std::mem::size_of::<EVENT_TRACE_PROPERTIES>() as u32;
        let _ = ControlTraceW(
            CONTROLTRACE_HANDLE::default(),
            &HSTRING::from(name),
            props,
            EVENT_TRACE_CONTROL_STOP,
        );
    }
}
