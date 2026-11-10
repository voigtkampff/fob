//! Pure decoders for the bytes Windows returns. Offsets follow winioctl.h and
//! nvme.h; `windows.rs` tests check them against the `windows` crate layouts.

use super::{Bus, CriticalWarning, NvmeHealth, NvmeState};

/// FIELD_OFFSET(STORAGE_PROPERTY_QUERY, AdditionalParameters).
pub const QUERY_HEADER: usize = 8;
/// sizeof(STORAGE_PROTOCOL_SPECIFIC_DATA): ten ULONGs.
pub const PROTOCOL_DATA: usize = 40;
/// sizeof(STORAGE_PROTOCOL_DATA_DESCRIPTOR): Version, Size, then the data.
pub const PROTOCOL_DESCRIPTOR: usize = 8 + PROTOCOL_DATA;
/// sizeof(NVME_HEALTH_INFO_LOG).
pub const HEALTH_LOG: usize = 512;
/// One NVMe data unit is 1000 units of 512 bytes.
const DATA_UNIT_BYTES: f64 = 512_000.0;
/// Kelvin to degrees Celsius, as in Microsoft's NVMe sample.
const KELVIN: i32 = 273;

fn u32_at(b: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(b.get(at..at + 4)?.try_into().ok()?))
}
fn u128_at(b: &[u8], at: usize) -> Option<u128> {
    Some(u128::from_le_bytes(b.get(at..at + 16)?.try_into().ok()?))
}
/// NVMe counters are 128-bit. f64 holds any of them without wrapping; past
/// 2^53 it rounds, which no display here can show anyway.
fn counter(b: &[u8], at: usize) -> Option<f64> {
    u128_at(b, at).map(|v| v as f64)
}

/// Why a reply could not be used. Kept apart from Windows error codes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BadReply {
    Short,
    Header,
}

/// The request for the NVMe SMART / Health Information log page (02h).
/// StorageDeviceProtocolSpecificProperty (50), PropertyStandardQuery (0),
/// ProtocolTypeNvme (3), NVMeDataTypeLogPage (2).
pub fn health_log_request() -> Vec<u8> {
    let mut b = vec![0u8; QUERY_HEADER + PROTOCOL_DATA + HEALTH_LOG];
    let put = |b: &mut [u8], at: usize, v: u32| b[at..at + 4].copy_from_slice(&v.to_le_bytes());
    put(&mut b, 0, 50);
    put(&mut b, 4, 0);
    let p = QUERY_HEADER;
    put(&mut b, p, 3);
    put(&mut b, p + 4, 2);
    put(&mut b, p + 8, 2);
    put(&mut b, p + 16, PROTOCOL_DATA as u32);
    put(&mut b, p + 20, HEALTH_LOG as u32);
    b
}

/// The log page inside a STORAGE_PROTOCOL_DATA_DESCRIPTOR reply, checked the
/// way Microsoft's sample checks it.
pub fn health_log_reply(reply: &[u8]) -> Result<&[u8], BadReply> {
    if reply.len() < PROTOCOL_DESCRIPTOR {
        return Err(BadReply::Short);
    }
    let (version, size) = (u32_at(reply, 0), u32_at(reply, 4));
    let offset = u32_at(reply, 8 + 16).ok_or(BadReply::Short)? as usize;
    let length = u32_at(reply, 8 + 20).ok_or(BadReply::Short)? as usize;
    if version != Some(PROTOCOL_DESCRIPTOR as u32) || size != Some(PROTOCOL_DESCRIPTOR as u32) {
        return Err(BadReply::Header);
    }
    if offset < PROTOCOL_DATA || length < HEALTH_LOG {
        return Err(BadReply::Header);
    }
    let start = 8usize.checked_add(offset).ok_or(BadReply::Header)?;
    reply.get(start..start + HEALTH_LOG).ok_or(BadReply::Short)
}

/// NVMe base specification, SMART / Health Information (Log Page 02h).
pub fn health_log(log: &[u8]) -> Result<NvmeHealth, BadReply> {
    if log.len() < HEALTH_LOG {
        return Err(BadReply::Short);
    }
    let kelvin = u16::from_le_bytes([log[1], log[2]]);
    let percent = |v: u8| (v <= 100).then_some(v);
    let c = |at| counter(log, at).ok_or(BadReply::Short);
    Ok(NvmeHealth {
        critical_warning: CriticalWarning::from_bits(log[0]),
        // 0 K means the controller reports no temperature.
        temperature_c: (kelvin > 0).then(|| i32::from(kelvin) - KELVIN),
        available_spare: percent(log[3]),
        spare_threshold: percent(log[4]),
        // Vendor estimate of life used; 255 means 255% or more.
        percent_used: log[5],
        data_read_bytes: c(32)? * DATA_UNIT_BYTES,
        data_written_bytes: c(48)? * DATA_UNIT_BYTES,
        power_cycles: c(112)?,
        power_on_hours: c(128)?,
        unsafe_shutdowns: c(144)?,
        media_errors: c(160)?,
        error_log_entries: c(176)?,
        warning_temp_minutes: u32_at(log, 192).ok_or(BadReply::Short)?,
        critical_temp_minutes: u32_at(log, 196).ok_or(BadReply::Short)?,
    })
}

/// What a disk's STORAGE_DEVICE_DESCRIPTOR tells us. The serial number
/// offset is never read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceInfo {
    pub model: Option<String>,
    pub bus: Bus,
    pub removable: bool,
}

fn ascii_at(b: &[u8], offset: u32) -> Option<String> {
    let offset = offset as usize;
    if offset == 0 || offset >= b.len() {
        return None;
    }
    let raw = &b[offset..];
    let end = raw.iter().position(|&c| c == 0).unwrap_or(raw.len());
    let text = String::from_utf8_lossy(&raw[..end]).trim().to_owned();
    (!text.is_empty()).then_some(text)
}

pub fn device_descriptor(reply: &[u8]) -> Result<DeviceInfo, BadReply> {
    // Version, Size, four bytes of flags, four offsets, BusType.
    if reply.len() < 32 {
        return Err(BadReply::Short);
    }
    let size = u32_at(reply, 4).ok_or(BadReply::Short)? as usize;
    if size < 32 {
        return Err(BadReply::Header);
    }
    let reply = &reply[..size.min(reply.len())];
    Ok(DeviceInfo {
        model: ascii_at(reply, u32_at(reply, 16).unwrap_or(0)),
        bus: Bus::from_storage_bus(u32_at(reply, 28).unwrap_or(0)),
        removable: reply[10] != 0,
    })
}

/// Win32 error from the NVMe log query, as the state the UI explains.
pub fn query_error(code: u32) -> NvmeState {
    const ERROR_INVALID_FUNCTION: u32 = 1;
    const ERROR_ACCESS_DENIED: u32 = 5;
    const ERROR_NOT_SUPPORTED: u32 = 50;
    match code {
        ERROR_ACCESS_DENIED => NvmeState::Denied,
        ERROR_INVALID_FUNCTION | ERROR_NOT_SUPPORTED => NvmeState::Unsupported,
        code => NvmeState::Error { code },
    }
}
