//! Pure decoders for the bytes Windows returns. Offsets follow winioctl.h,
//! nvme.h and the ATA Device Statistics log (ACS-4, log 04h); `windows.rs`
//! tests check the Windows ones against the `windows` crate layouts.

use super::{AtaStats, Bus, CriticalWarning, NvmeHealth, Refusal};

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
fn u64_at(b: &[u8], at: usize) -> Option<u64> {
    Some(u64::from_le_bytes(b.get(at..at + 8)?.try_into().ok()?))
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

/// A StorageDeviceProtocolSpecificProperty (50) PropertyStandardQuery with
/// room for `length` bytes of protocol data right after the request.
fn protocol_request(
    protocol: u32,
    data_type: u32,
    value: u32,
    sub_value: u32,
    length: usize,
) -> Vec<u8> {
    let mut b = vec![0u8; QUERY_HEADER + PROTOCOL_DATA + length];
    let put = |b: &mut [u8], at: usize, v: u32| b[at..at + 4].copy_from_slice(&v.to_le_bytes());
    put(&mut b, 0, 50);
    put(&mut b, 4, 0);
    let p = QUERY_HEADER;
    put(&mut b, p, protocol);
    put(&mut b, p + 4, data_type);
    put(&mut b, p + 8, value);
    put(&mut b, p + 12, sub_value);
    put(&mut b, p + 16, PROTOCOL_DATA as u32);
    put(&mut b, p + 20, length as u32);
    b
}

/// The NVMe SMART / Health Information log page (02h): ProtocolTypeNvme (3),
/// NVMeDataTypeLogPage (2).
pub fn health_log_request() -> Vec<u8> {
    protocol_request(3, 2, 2, 0, HEALTH_LOG)
}

/// The `length` bytes of protocol data inside a
/// STORAGE_PROTOCOL_DATA_DESCRIPTOR reply, checked the way Microsoft's
/// sample checks it.
fn protocol_reply(reply: &[u8], length: usize) -> Result<&[u8], BadReply> {
    if reply.len() < PROTOCOL_DESCRIPTOR {
        return Err(BadReply::Short);
    }
    let (version, size) = (u32_at(reply, 0), u32_at(reply, 4));
    let offset = u32_at(reply, 8 + 16).ok_or(BadReply::Short)? as usize;
    let got = u32_at(reply, 8 + 20).ok_or(BadReply::Short)? as usize;
    if version != Some(PROTOCOL_DESCRIPTOR as u32) || size != Some(PROTOCOL_DESCRIPTOR as u32) {
        return Err(BadReply::Header);
    }
    if offset < PROTOCOL_DATA || got < length {
        return Err(BadReply::Header);
    }
    let start = 8usize.checked_add(offset).ok_or(BadReply::Header)?;
    let end = start.checked_add(length).ok_or(BadReply::Header)?;
    reply.get(start..end).ok_or(BadReply::Short)
}

pub fn health_log_reply(reply: &[u8]) -> Result<&[u8], BadReply> {
    protocol_reply(reply, HEALTH_LOG)
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

/// Win32 error from a query, as the state the UI explains.
pub fn refusal(code: u32) -> Refusal {
    const ERROR_INVALID_FUNCTION: u32 = 1;
    const ERROR_ACCESS_DENIED: u32 = 5;
    const ERROR_NOT_SUPPORTED: u32 = 50;
    match code {
        ERROR_ACCESS_DENIED => Refusal::Denied,
        ERROR_INVALID_FUNCTION | ERROR_NOT_SUPPORTED => Refusal::Unsupported,
        code => Refusal::Error(code),
    }
}

/// ATA Device Statistics: log address 04h, one 512-byte page per request.
pub const ATA_PAGE: usize = 512;
const DEVICE_STATISTICS_LOG: u32 = 0x04;
pub const PAGE_LIST: u8 = 0x00;
pub const PAGE_GENERAL: u8 = 0x01;
pub const PAGE_TEMPERATURE: u8 = 0x05;
pub const PAGE_SOLID_STATE: u8 = 0x07;

/// One Device Statistics page: ProtocolTypeAta (2), AtaDataTypeLogPage (2),
/// the log address, then the page number.
pub fn ata_page_request(page: u8) -> Vec<u8> {
    protocol_request(2, 2, DEVICE_STATISTICS_LOG, u32::from(page), ATA_PAGE)
}

/// The page in a reply, if its header names the page asked for. Every page
/// starts with a QWORD: revision in bits 15:0 (never 0), page number in
/// bits 23:16.
pub fn ata_page_reply(reply: &[u8], page: u8) -> Result<&[u8], BadReply> {
    let data = protocol_reply(reply, ATA_PAGE)?;
    let header = u64_at(data, 0).ok_or(BadReply::Short)?;
    if header & 0xFFFF == 0 || (header >> 16) & 0xFF != u64::from(page) {
        return Err(BadReply::Header);
    }
    Ok(data)
}

/// Page 00h: byte 8 is the entry count, the page numbers follow.
pub fn supported_pages(list: &[u8]) -> Result<Vec<u8>, BadReply> {
    let count = *list.get(8).ok_or(BadReply::Short)? as usize;
    list.get(9..9 + count)
        .map(<[u8]>::to_vec)
        .ok_or(BadReply::Short)
}

/// A statistic's 48-bit value, only when the drive sets both Supported
/// (bit 63) and Valid Value (bit 62).
fn statistic(page: Option<&[u8]>, at: usize) -> Option<u64> {
    let q = u64_at(page?, at)?;
    (q >> 63 == 1 && (q >> 62) & 1 == 1).then_some(q & 0xFFFF_FFFF_FFFF)
}

/// Logical sectors as bytes; None when the sector size is unknown or the
/// product does not fit.
fn sectors_to_bytes(sectors: Option<u64>, sector_bytes: Option<u32>) -> Option<f64> {
    sectors?
        .checked_mul(u64::from(sector_bytes?))
        .map(|b| b as f64)
}

/// The statistics Fob shows, from the pages that were read. Offsets are
/// ACS-4 9.5: General (01h) power-on resets 8, power-on hours 16, logical
/// sectors written 24, read 40; Temperature (05h) current 8; Solid State
/// (07h) percentage used 8.
pub fn ata_stats(
    general: Option<&[u8]>,
    temperature: Option<&[u8]>,
    solid_state: Option<&[u8]>,
    sector_bytes: Option<u32>,
) -> AtaStats {
    let low32 = |v: u64| (v & 0xFFFF_FFFF) as u32;
    AtaStats {
        // A signed byte; -128 (80h) is "no valid value".
        temperature_c: statistic(temperature, 8)
            .map(|v| i32::from(v as u8 as i8))
            .filter(|&c| c != -128),
        percent_used: statistic(solid_state, 8).map(|v| v as u8),
        power_on_hours: statistic(general, 16).map(low32),
        power_cycles: statistic(general, 8).map(low32),
        data_written_bytes: sectors_to_bytes(statistic(general, 24), sector_bytes),
        data_read_bytes: sectors_to_bytes(statistic(general, 40), sector_bytes),
    }
}

/// BytesPerLogicalSector from a STORAGE_ACCESS_ALIGNMENT_DESCRIPTOR reply;
/// only sizes a disk can have (a multiple of 512, up to 64 KiB).
pub fn logical_sector_bytes(reply: &[u8]) -> Option<u32> {
    let size = u32_at(reply, 4)? as usize;
    if size < 20 {
        return None;
    }
    let bytes = u32_at(reply, 16)?;
    ((512..=65536).contains(&bytes) && bytes % 512 == 0).then_some(bytes)
}
