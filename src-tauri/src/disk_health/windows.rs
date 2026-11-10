//! Windows probes. Every handle is opened with no access rights: the three
//! IOCTLs used are FILE_ANY_ACCESS (see the tests), so the I/O manager lets
//! a standard user send them. Whether a driver accepts the NVMe log or ATA
//! statistics query on such a handle is the driver's call; a refusal shows
//! as Denied.

use super::parse::{self, BadReply};
use super::{AtaState, NvmeState, RawDisk, RawVolume, WmiDisk};
use windows::core::{BOOL, BSTR, HSTRING, PCWSTR};
use windows::Win32::Devices::DeviceAndDriverInstallation::{
    CM_Get_Device_Interface_ListW, CM_Get_Device_Interface_List_SizeW,
    CM_GET_DEVICE_INTERFACE_LIST_PRESENT, CR_BUFFER_SMALL, CR_SUCCESS,
};
use windows::Win32::Foundation::{CloseHandle, HANDLE, MAX_PATH};
use windows::Win32::Storage::FileSystem::{
    CreateFileW, FindFirstVolumeW, FindNextVolumeW, FindVolumeClose,
    GetVolumePathNamesForVolumeNameW, FILE_FLAGS_AND_ATTRIBUTES, FILE_SHARE_READ, FILE_SHARE_WRITE,
    IOCTL_VOLUME_GET_VOLUME_DISK_EXTENTS, OPEN_EXISTING,
};
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CoSetProxyBlanket, CoUninitialize, CLSCTX_INPROC_SERVER,
    COINIT_MULTITHREADED, EOAC_NONE, RPC_C_AUTHN_LEVEL_CALL, RPC_C_IMP_LEVEL_IMPERSONATE,
    SAFEARRAY,
};
use windows::Win32::System::Ioctl::{
    GUID_DEVINTERFACE_DISK, IOCTL_STORAGE_GET_DEVICE_NUMBER, IOCTL_STORAGE_QUERY_PROPERTY,
    STORAGE_DEVICE_NUMBER,
};
use windows::Win32::System::Ole::{SafeArrayGetElement, SafeArrayGetLBound, SafeArrayGetUBound};
use windows::Win32::System::Power::GetDevicePowerState;
use windows::Win32::System::Rpc::{RPC_C_AUTHN_WINNT, RPC_C_AUTHZ_NONE};
use windows::Win32::System::Variant::{VariantClear, VARIANT, VT_ARRAY, VT_BSTR, VT_I4, VT_NULL};
use windows::Win32::System::Wmi::{
    IWbemClassObject, IWbemLocator, WbemLocator, WBEM_FLAG_FORWARD_ONLY,
    WBEM_FLAG_RETURN_IMMEDIATELY,
};
use windows::Win32::System::IO::DeviceIoControl;

struct Handle(HANDLE);
impl Drop for Handle {
    fn drop(&mut self) {
        let _ = unsafe { CloseHandle(self.0) };
    }
}

fn open(path: &str) -> Result<Handle, u32> {
    unsafe {
        CreateFileW(
            &HSTRING::from(path),
            0,
            FILE_SHARE_READ | FILE_SHARE_WRITE,
            None,
            OPEN_EXISTING,
            FILE_FLAGS_AND_ATTRIBUTES(0),
            None,
        )
    }
    .map(Handle)
    .map_err(|e| e.code().0 as u32 & 0xFFFF)
}

/// DeviceIoControl into `out`; the Win32 error code on failure.
fn ioctl(h: &Handle, code: u32, input: &[u8], out: &mut [u8]) -> Result<usize, u32> {
    let mut got = 0u32;
    unsafe {
        DeviceIoControl(
            h.0,
            code,
            Some(input.as_ptr().cast()),
            input.len() as u32,
            Some(out.as_mut_ptr().cast()),
            out.len() as u32,
            Some(&mut got),
            None,
        )
    }
    .map(|_| got as usize)
    .map_err(|e| e.code().0 as u32 & 0xFFFF)
}

fn disk_paths() -> Vec<String> {
    unsafe {
        for _ in 0..3 {
            let mut len = 0u32;
            if CM_Get_Device_Interface_List_SizeW(
                &mut len,
                &GUID_DEVINTERFACE_DISK,
                PCWSTR::null(),
                CM_GET_DEVICE_INTERFACE_LIST_PRESENT,
            ) != CR_SUCCESS
            {
                return Vec::new();
            }
            let mut buf = vec![0u16; len as usize];
            match CM_Get_Device_Interface_ListW(
                &GUID_DEVINTERFACE_DISK,
                PCWSTR::null(),
                &mut buf,
                CM_GET_DEVICE_INTERFACE_LIST_PRESENT,
            ) {
                CR_SUCCESS => {
                    return buf
                        .split(|&c| c == 0)
                        .filter(|s| !s.is_empty())
                        .map(String::from_utf16_lossy)
                        .collect()
                }
                // A disk arrived between the two calls.
                CR_BUFFER_SMALL => continue,
                _ => return Vec::new(),
            }
        }
        Vec::new()
    }
}

fn probe_disk(path: &str) -> RawDisk {
    let h = match open(path) {
        Ok(h) => h,
        Err(code) => {
            return RawDisk {
                number: None,
                info: None,
                nvme: parse::refusal(code).into(),
                ata: AtaState::NotAsked,
            }
        }
    };
    let mut num = [0u8; std::mem::size_of::<STORAGE_DEVICE_NUMBER>()];
    let number = ioctl(&h, IOCTL_STORAGE_GET_DEVICE_NUMBER, &[], &mut num)
        .ok()
        .filter(|&n| n >= 8)
        .map(|_| u32::from_le_bytes(num[4..8].try_into().unwrap()));
    // StorageDeviceProperty (0), PropertyStandardQuery.
    let query = [0u8; 12];
    let mut out = vec![0u8; 1024];
    let info = ioctl(&h, IOCTL_STORAGE_QUERY_PROPERTY, &query, &mut out)
        .ok()
        .and_then(|n| parse::device_descriptor(&out[..n]).ok());
    let bus = info.as_ref().map(|i| i.bus).unwrap_or(super::Bus::Unknown);
    let nvme = if bus.skips_nvme_query() {
        NvmeState::NotAsked
    } else {
        let request = parse::health_log_request();
        let mut reply = vec![0u8; request.len()];
        match ioctl(&h, IOCTL_STORAGE_QUERY_PROPERTY, &request, &mut reply) {
            Err(code) => parse::refusal(code).into(),
            Ok(n) => match parse::health_log_reply(&reply[..n]).and_then(parse::health_log) {
                Ok(log) => NvmeState::Ok { log },
                Err(BadReply::Short | BadReply::Header) => NvmeState::BadReply,
            },
        }
    };
    let ata = if bus.asks_ata() {
        ata_stats(&h)
    } else {
        AtaState::NotAsked
    };
    RawDisk {
        number,
        info,
        nvme,
        ata,
    }
}

/// Windows' own power state for the disk. FALSE in `on` means a low-power
/// state; then nothing is sent. TRUE does not prove the drive is spun up,
/// and a failed call is treated as awake.
fn windows_says_asleep(h: &Handle) -> bool {
    let mut on = BOOL(1);
    unsafe { GetDevicePowerState(h.0, &mut on) }.as_bool() && !on.as_bool()
}

enum PageError {
    Refused(u32),
    Bad,
}

fn ata_page(h: &Handle, page: u8) -> Result<Vec<u8>, PageError> {
    let request = parse::ata_page_request(page);
    let mut reply = vec![0u8; request.len()];
    let n =
        ioctl(h, IOCTL_STORAGE_QUERY_PROPERTY, &request, &mut reply).map_err(PageError::Refused)?;
    parse::ata_page_reply(&reply[..n], page)
        .map(<[u8]>::to_vec)
        .map_err(|_| PageError::Bad)
}

/// StorageAccessAlignmentProperty (6): the logical sector size the
/// sector counters are in.
fn logical_sector_bytes(h: &Handle) -> Option<u32> {
    let query = [6u8, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
    let mut out = [0u8; 64];
    let n = ioctl(h, IOCTL_STORAGE_QUERY_PROPERTY, &query, &mut out).ok()?;
    parse::logical_sector_bytes(&out[..n.min(out.len())])
}

/// The page list first, then only the listed pages Fob shows. A page that
/// fails after the list leaves its fields out.
fn ata_stats(h: &Handle) -> AtaState {
    if windows_says_asleep(h) {
        return AtaState::Sleeping;
    }
    let listed = match ata_page(h, parse::PAGE_LIST) {
        Err(PageError::Refused(code)) => return parse::refusal(code).into(),
        Err(PageError::Bad) => return AtaState::BadReply,
        Ok(list) => match parse::supported_pages(&list) {
            Ok(pages) => pages,
            Err(_) => return AtaState::BadReply,
        },
    };
    let read = |page: u8| {
        listed
            .contains(&page)
            .then(|| ata_page(h, page).ok())
            .flatten()
    };
    let general = read(parse::PAGE_GENERAL);
    let temperature = read(parse::PAGE_TEMPERATURE);
    let solid_state = read(parse::PAGE_SOLID_STATE);
    let sector = general.as_ref().and_then(|_| logical_sector_bytes(h));
    let stats = parse::ata_stats(
        general.as_deref(),
        temperature.as_deref(),
        solid_state.as_deref(),
        sector,
    );
    if stats.is_empty() {
        AtaState::Unsupported
    } else {
        AtaState::Ok { stats }
    }
}

fn volumes() -> Vec<RawVolume> {
    let mut out = Vec::new();
    let mut name = [0u16; MAX_PATH as usize + 1];
    let Ok(find) = (unsafe { FindFirstVolumeW(&mut name) }) else {
        return out;
    };
    loop {
        let end = name.iter().position(|&c| c == 0).unwrap_or(name.len());
        let guid = String::from_utf16_lossy(&name[..end]);
        out.push(volume(&guid));
        if unsafe { FindNextVolumeW(find, &mut name) }.is_err() {
            break;
        }
    }
    let _ = unsafe { FindVolumeClose(find) };
    out
}

fn volume(guid: &str) -> RawVolume {
    let mut mounts = Vec::new();
    let mut buf = vec![0u16; 1024];
    let mut len = 0u32;
    if unsafe { GetVolumePathNamesForVolumeNameW(&HSTRING::from(guid), Some(&mut buf), &mut len) }
        .is_ok()
    {
        mounts = buf[..(len as usize).min(buf.len())]
            .split(|&c| c == 0)
            .filter(|s| !s.is_empty())
            .map(String::from_utf16_lossy)
            .collect();
    }
    // A volume can span disks: DISK_EXTENT is 24 bytes after an 8-byte
    // header. 128 extents is far past any real spanned volume.
    let disks = open(guid.trim_end_matches('\\'))
        .ok()
        .map(|h| {
            let mut out = vec![0u8; 8 + 24 * 128];
            match ioctl(&h, IOCTL_VOLUME_GET_VOLUME_DISK_EXTENTS, &[], &mut out) {
                Ok(n) if n >= 8 => {
                    let count = u32::from_le_bytes(out[0..4].try_into().unwrap()) as usize;
                    (0..count)
                        .filter_map(|i| {
                            let at = 8 + i * 24;
                            out.get(at..at + 4)
                                .map(|b| u32::from_le_bytes(b.try_into().unwrap()))
                        })
                        .collect()
                }
                _ => Vec::new(),
            }
        })
        .unwrap_or_default();
    RawVolume { mounts, disks }
}

struct Com(bool);
impl Drop for Com {
    fn drop(&mut self) {
        if self.0 {
            unsafe { CoUninitialize() };
        }
    }
}

fn u32_of(v: &VARIANT) -> Option<u32> {
    unsafe {
        let v = &v.Anonymous.Anonymous;
        (v.vt == VT_I4).then(|| v.Anonymous.lVal as u32)
    }
}

fn get(obj: &IWbemClassObject, name: &str) -> VARIANT {
    let mut v = VARIANT::default();
    let name = HSTRING::from(name);
    let _ = unsafe { obj.Get(PCWSTR(name.as_ptr()), 0, &mut v, None, None) };
    v
}

fn wmi_row(obj: &IWbemClassObject) -> WmiDisk {
    let mut id = get(obj, "DeviceId");
    let mut health = get(obj, "HealthStatus");
    let mut bus = get(obj, "BusType");
    let mut ops = get(obj, "OperationalStatus");
    let device_id = unsafe {
        let v = &id.Anonymous.Anonymous;
        if v.vt == VT_BSTR {
            v.Anonymous.bstrVal.to_string()
        } else {
            String::new()
        }
    };
    let mut operational = Vec::new();
    unsafe {
        let v = &ops.Anonymous.Anonymous;
        // uint16[] arrives as VT_ARRAY | VT_I4.
        if v.vt.0 == VT_ARRAY.0 | VT_I4.0 && v.vt != VT_NULL {
            let sa: *const SAFEARRAY = v.Anonymous.parray;
            if let (Ok(lo), Ok(hi)) = (SafeArrayGetLBound(sa, 1), SafeArrayGetUBound(sa, 1)) {
                for i in lo..=hi.min(lo + 31) {
                    let mut x = 0i32;
                    if SafeArrayGetElement(sa, &i, (&mut x as *mut i32).cast()).is_ok() {
                        operational.push(x as u32);
                    }
                }
            }
        }
    }
    let row = WmiDisk {
        device_id,
        health: u32_of(&health),
        operational,
        bus: u32_of(&bus),
    };
    for v in [&mut id, &mut health, &mut bus, &mut ops] {
        let _ = unsafe { VariantClear(v) };
    }
    row
}

/// MSFT_PhysicalDisk without SerialNumber or FriendlyName.
fn wmi() -> Result<Vec<WmiDisk>, i32> {
    let _com = Com(unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) }.is_ok());
    unsafe {
        let locator: IWbemLocator =
            CoCreateInstance(&WbemLocator, None, CLSCTX_INPROC_SERVER).map_err(|e| e.code().0)?;
        let services = locator
            .ConnectServer(
                &BSTR::from(r"ROOT\Microsoft\Windows\Storage"),
                &BSTR::new(),
                &BSTR::new(),
                &BSTR::new(),
                0,
                &BSTR::new(),
                None,
            )
            .map_err(|e| e.code().0)?;
        CoSetProxyBlanket(
            &services,
            RPC_C_AUTHN_WINNT,
            RPC_C_AUTHZ_NONE,
            PCWSTR::null(),
            RPC_C_AUTHN_LEVEL_CALL,
            RPC_C_IMP_LEVEL_IMPERSONATE,
            None,
            EOAC_NONE,
        )
        .map_err(|e| e.code().0)?;
        let rows = services
            .ExecQuery(
                &BSTR::from("WQL"),
                &BSTR::from(
                    "SELECT DeviceId, HealthStatus, OperationalStatus, BusType FROM MSFT_PhysicalDisk",
                ),
                WBEM_FLAG_FORWARD_ONLY | WBEM_FLAG_RETURN_IMMEDIATELY,
                None,
            )
            .map_err(|e| e.code().0)?;
        let mut out = Vec::new();
        loop {
            let mut objs = [None];
            let mut got = 0u32;
            // 10 s per row at most; a stuck provider ends the list.
            let hr = rows.Next(10_000, &mut objs, &mut got);
            if hr.is_err() {
                return if out.is_empty() { Err(hr.0) } else { Ok(out) };
            }
            match objs[0].take() {
                Some(obj) if got == 1 => out.push(wmi_row(&obj)),
                _ => break,
            }
        }
        Ok(out)
    }
}

pub fn probe() -> (Vec<RawDisk>, Vec<RawVolume>, Result<Vec<WmiDisk>, i32>) {
    let disks = disk_paths().iter().map(|p| probe_disk(p)).collect();
    (disks, volumes(), wmi())
}

#[cfg(test)]
mod tests {
    use super::parse;
    use std::mem::{offset_of, size_of};
    use windows::Win32::Storage::Nvme::NVME_HEALTH_INFO_LOG;
    use windows::Win32::System::Ioctl::*;

    /// Bits 14-15 of an IOCTL code are its required access; 0 is
    /// FILE_ANY_ACCESS, which a handle opened with no rights may send.
    #[test]
    fn every_ioctl_used_needs_no_access_rights() {
        for code in [
            IOCTL_STORAGE_QUERY_PROPERTY,
            IOCTL_STORAGE_GET_DEVICE_NUMBER,
            windows::Win32::Storage::FileSystem::IOCTL_VOLUME_GET_VOLUME_DISK_EXTENTS,
        ] {
            assert_eq!((code >> 14) & 3, 0, "{code:#x}");
        }
    }

    #[test]
    fn hand_offsets_match_the_windows_layouts() {
        assert_eq!(
            offset_of!(STORAGE_PROPERTY_QUERY, AdditionalParameters),
            parse::QUERY_HEADER
        );
        assert_eq!(
            size_of::<STORAGE_PROTOCOL_SPECIFIC_DATA>(),
            parse::PROTOCOL_DATA
        );
        assert_eq!(
            size_of::<STORAGE_PROTOCOL_DATA_DESCRIPTOR>(),
            parse::PROTOCOL_DESCRIPTOR
        );
        assert_eq!(
            offset_of!(STORAGE_PROTOCOL_SPECIFIC_DATA, ProtocolDataOffset),
            16
        );
        assert_eq!(
            offset_of!(STORAGE_PROTOCOL_SPECIFIC_DATA, ProtocolDataLength),
            20
        );
        assert_eq!(size_of::<NVME_HEALTH_INFO_LOG>(), parse::HEALTH_LOG);
        assert_eq!(offset_of!(NVME_HEALTH_INFO_LOG, DataUnitRead), 32);
        assert_eq!(offset_of!(NVME_HEALTH_INFO_LOG, DataUnitWritten), 48);
        assert_eq!(offset_of!(NVME_HEALTH_INFO_LOG, PowerCycle), 112);
        assert_eq!(offset_of!(NVME_HEALTH_INFO_LOG, PowerOnHours), 128);
        assert_eq!(offset_of!(NVME_HEALTH_INFO_LOG, UnsafeShutdowns), 144);
        assert_eq!(offset_of!(NVME_HEALTH_INFO_LOG, MediaErrors), 160);
        assert_eq!(
            offset_of!(NVME_HEALTH_INFO_LOG, ErrorInfoLogEntryCount),
            176
        );
        assert_eq!(
            offset_of!(NVME_HEALTH_INFO_LOG, WarningCompositeTemperatureTime),
            192
        );
        assert_eq!(offset_of!(STORAGE_DEVICE_DESCRIPTOR, RemovableMedia), 10);
        assert_eq!(offset_of!(STORAGE_DEVICE_DESCRIPTOR, ProductIdOffset), 16);
        assert_eq!(offset_of!(STORAGE_DEVICE_DESCRIPTOR, BusType), 28);
        assert_eq!(offset_of!(STORAGE_DEVICE_NUMBER, DeviceNumber), 4);
        assert_eq!(size_of::<DISK_EXTENT>(), 24);
        assert_eq!(offset_of!(VOLUME_DISK_EXTENTS, Extents), 8);
        assert_eq!(StorageDeviceProtocolSpecificProperty.0, 50);
        assert_eq!(ProtocolTypeNvme.0, 3);
        assert_eq!(ProtocolTypeAta.0, 2);
        assert_eq!(AtaDataTypeLogPage.0, 2);
        assert_eq!(
            offset_of!(STORAGE_PROTOCOL_SPECIFIC_DATA, ProtocolDataRequestValue),
            8
        );
        assert_eq!(
            offset_of!(STORAGE_PROTOCOL_SPECIFIC_DATA, ProtocolDataRequestSubValue),
            12
        );
        assert_eq!(StorageAccessAlignmentProperty.0, 6);
        assert_eq!(
            offset_of!(STORAGE_ACCESS_ALIGNMENT_DESCRIPTOR, BytesPerLogicalSector),
            16
        );
        assert_eq!(NVMeDataTypeLogPage.0, 2);
        assert_eq!(
            windows::Win32::Storage::Nvme::NVME_LOG_PAGE_HEALTH_INFO.0,
            2
        );
    }

    /// Runs the real probes on the CI runner: no panic, no serial in the
    /// output, every disk accounted for.
    #[test]
    fn probing_this_machine_does_not_fail_all_disks() {
        let (disks, volumes, wmi) = super::probe();
        println!("disks {disks:?}\nvolumes {volumes:?}\nwmi {wmi:?}");
        assert!(!disks.is_empty(), "the runner has a system disk");
        assert!(disks.iter().any(|d| d.number.is_some()));
        assert!(volumes.iter().any(|v| v.mounts.iter().any(|m| m == r"C:\")));
    }

    /// GetDevicePowerState on the same no-access handles the probe uses:
    /// printed, since a runner's virtual disks say nothing about SATA.
    #[test]
    fn device_power_state_answers_on_a_no_access_handle() {
        for path in super::disk_paths() {
            let h = super::open(&path).expect("open with no access rights");
            let mut on = windows::core::BOOL(0);
            let ok = unsafe { windows::Win32::System::Power::GetDevicePowerState(h.0, &mut on) };
            println!(
                "power ok={} on={} asleep={}",
                ok.as_bool(),
                on.as_bool(),
                super::windows_says_asleep(&h)
            );
        }
    }
}
