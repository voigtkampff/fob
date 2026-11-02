//! LibreHardwareMonitor's built-in web server (Options -> Remote Web Server,
//! default port 8085): `GET /data.json` returns the sensor tree. Schema per
//! LibreHardwareMonitor.Windows.Forms/Utilities/HttpServer.cs
//! (`GenerateJsonForNode`): every node has `id`, `Text`, `Min`, `Value`, `Max`,
//! `ImageURL`, `Children`; hardware nodes add `HardwareId`; sensor nodes add
//! `SensorId`, `Type` and the unformatted `RawMin`/`RawValue`/`RawMax`.
//! Older releases only have the formatted `Value` (e.g. "52,0 °C") and no `Type`.
#![cfg_attr(not(target_os = "windows"), allow(dead_code))]
use super::{Sensor, SensorKind, SensorSource};
use serde_json::Value;
use std::{
    io::{Read, Write},
    net::{SocketAddr, TcpStream},
    time::{Duration, Instant},
};

fn kind_of(node: &Value) -> Option<SensorKind> {
    let kind = node["Type"].as_str().map(str::to_owned).or_else(|| {
        // "/lpc/example/0/fan/1" -> "fan"
        let id = node["SensorId"].as_str()?;
        let parts: Vec<&str> = id.trim_matches('/').split('/').collect();
        parts.len().checked_sub(2).map(|i| parts[i].to_owned())
    })?;
    match kind.to_ascii_lowercase().as_str() {
        "temperature" => Some(SensorKind::Temperature),
        "fan" => Some(SensorKind::Fan),
        "control" => Some(SensorKind::Control),
        _ => None,
    }
}

/// "52,0 °C" / "1349 RPM" / "45.0 %" -> number. Formatted values never carry
/// thousands separators (LHM uses F0/F1 formats), so ',' is a decimal comma.
fn number_in(text: &str) -> Option<f64> {
    let digits: String = text
        .trim()
        .chars()
        .take_while(|c| c.is_ascii_digit() || matches!(c, '-' | '.' | ','))
        .map(|c| if c == ',' { '.' } else { c })
        .collect();
    digits.parse().ok()
}

fn value_of(node: &Value) -> Option<f64> {
    let raw = match &node["RawValue"] {
        Value::Number(n) => n.as_f64(),
        _ => None,
    };
    raw.or_else(|| node["Value"].as_str().and_then(number_in))
        .filter(|v| v.is_finite())
}

fn walk(node: &Value, path: &mut Vec<String>, hardware: Option<&str>, out: &mut Vec<Sensor>) {
    let text = node["Text"].as_str().unwrap_or_default().to_owned();
    if node.get("SensorId").is_some() {
        if let (Some(kind), Some(value)) = (kind_of(node), value_of(node)) {
            // Fallback for old versions without HardwareId: the node above the
            // "Temperatures"/"Fans" group is the hardware.
            let fallback = path.len().checked_sub(2).map(|i| path[i].as_str());
            out.push(Sensor {
                hardware: hardware.or(fallback).unwrap_or("Unknown").to_owned(),
                id: node["SensorId"].as_str().unwrap_or_default().to_owned(),
                name: text,
                kind,
                value,
                source: "LibreHardwareMonitor".into(),
            });
        }
        return;
    }
    let here = node
        .get("HardwareId")
        .is_some()
        .then_some(text.as_str())
        .or(hardware)
        .map(str::to_owned);
    path.push(text);
    if let Some(children) = node["Children"].as_array() {
        for child in children {
            walk(child, path, here.as_deref(), out);
        }
    }
    path.pop();
}

/// Temperatures, fan RPMs and fan duty (%) from a data.json document.
pub fn parse_data_json(json: &str) -> Result<Vec<Sensor>, String> {
    let root: Value = serde_json::from_str(json).map_err(|e| e.to_string())?;
    if !root.is_object() || !root["Children"].is_array() {
        return Err("not a LibreHardwareMonitor data.json".into());
    }
    let mut out = Vec::new();
    walk(&root, &mut Vec::new(), None, &mut out);
    Ok(out)
}

/// Body of an HTTP/1.x response (Content-Length or chunked; no compression,
/// since we never send Accept-Encoding).
pub fn http_body(response: &[u8]) -> Result<Vec<u8>, String> {
    let split = response
        .windows(4)
        .position(|w| w == b"\r\n\r\n")
        .ok_or("incomplete HTTP response")?;
    let head = String::from_utf8_lossy(&response[..split]).to_ascii_lowercase();
    let mut lines = head.lines();
    let status = lines.next().unwrap_or_default();
    if !status.split_whitespace().nth(1).is_some_and(|c| c == "200") {
        return Err(format!("HTTP status: {status}"));
    }
    let body = &response[split + 4..];
    let chunked = lines.any(|l| l.starts_with("transfer-encoding:") && l.contains("chunked"));
    if !chunked {
        return Ok(body.to_vec());
    }
    let mut out = Vec::new();
    let mut rest = body;
    loop {
        let end = rest
            .windows(2)
            .position(|w| w == b"\r\n")
            .ok_or("bad chunk")?;
        let size_text = String::from_utf8_lossy(&rest[..end]);
        let size = usize::from_str_radix(size_text.split(';').next().unwrap_or("").trim(), 16)
            .map_err(|_| "bad chunk size")?;
        rest = &rest[end + 2..];
        if size == 0 {
            return Ok(out);
        }
        if rest.len() < size {
            return Err("truncated chunk".into());
        }
        out.extend_from_slice(&rest[..size]);
        rest = rest.get(size + 2..).unwrap_or_default();
    }
}

/// Reads `http://127.0.0.1:8085/data.json`. Short timeouts; after a failure
/// it waits before trying again, so a missing server costs nothing.
#[derive(Debug)]
pub struct LhmHttp {
    addr: SocketAddr,
    retry_at: Option<Instant>,
}

impl Default for LhmHttp {
    fn default() -> Self {
        Self {
            addr: SocketAddr::from(([127, 0, 0, 1], 8085)),
            retry_at: None,
        }
    }
}

impl LhmHttp {
    fn fetch(&self) -> Result<Vec<Sensor>, String> {
        let mut stream = TcpStream::connect_timeout(&self.addr, Duration::from_millis(200))
            .map_err(|e| e.to_string())?;
        stream
            .set_read_timeout(Some(Duration::from_millis(800)))
            .map_err(|e| e.to_string())?;
        stream
            .write_all(b"GET /data.json HTTP/1.0\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n")
            .map_err(|e| e.to_string())?;
        let mut response = Vec::new();
        stream
            .take(16 * 1024 * 1024)
            .read_to_end(&mut response)
            .map_err(|e| e.to_string())?;
        let body = http_body(&response)?;
        parse_data_json(&String::from_utf8_lossy(&body))
    }
}

impl SensorSource for LhmHttp {
    fn name(&self) -> &'static str {
        "LibreHardwareMonitor"
    }
    fn read(&mut self) -> Option<Vec<Sensor>> {
        if self.retry_at.is_some_and(|t| Instant::now() < t) {
            return None;
        }
        match self.fetch() {
            Ok(sensors) => {
                self.retry_at = None;
                Some(sensors)
            }
            Err(_) => {
                self.retry_at = Some(Instant::now() + Duration::from_secs(15));
                None
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const SAMPLE: &str = include_str!("../../tests/fixtures/lhm-data.json");

    #[test]
    fn parses_realistic_data_json() {
        let sensors = parse_data_json(SAMPLE).unwrap();
        let find = |name: &str| sensors.iter().find(|s| s.name == name).unwrap();
        let pkg = find("CPU Package");
        assert_eq!(pkg.hardware, "Example Hybrid CPU");
        assert_eq!((pkg.kind, pkg.value), (SensorKind::Temperature, 52.0));
        let fan = find("Fan #2");
        assert_eq!(fan.hardware, "Example Chip");
        assert_eq!((fan.kind, fan.value), (SensorKind::Fan, 1349.0));
        assert_eq!(find("Fan #2 duty").kind, SensorKind::Control);
        let gpu = find("GPU Core");
        assert_eq!(gpu.hardware, "NVIDIA Example GPU");
        assert_eq!(gpu.value, 48.5);
        // Loads/clocks/voltages are not temperatures or fans; NaN and null are dropped.
        assert!(sensors
            .iter()
            .all(|s| s.name != "CPU Total" && s.name != "Bus Speed"));
        assert!(sensors
            .iter()
            .all(|s| s.name != "Fan #7" && s.name != "Temperature #6"));
        assert_eq!(sensors.len(), 8);
    }

    #[test]
    fn old_versions_without_raw_values_or_types() {
        let json = r#"{"id":0,"Text":"Sensor","Min":"Min","Value":"Value","Max":"Max","ImageURL":"","Children":[
          {"id":1,"Text":"PC","Min":"","Value":"","Max":"","ImageURL":"images_icon/computer.png","Children":[
            {"id":2,"Text":"Example Hybrid CPU","Min":"","Value":"","Max":"","ImageURL":"images_icon/cpu.png","Children":[
              {"id":3,"Text":"Temperatures","Min":"","Value":"","Max":"","ImageURL":"images_icon/temperature.png","Children":[
                {"id":4,"Text":"CPU Package","Min":"30,0 °C","Value":"61,5 °C","Max":"88,0 °C","SensorId":"/intelcpu/0/temperature/24","ImageURL":"images/transparent.png","Children":[]}
              ]},
              {"id":5,"Text":"Fans","Min":"","Value":"","Max":"","ImageURL":"images_icon/fan.png","Children":[
                {"id":6,"Text":"Fan #1","Min":"0 RPM","Value":"880 RPM","Max":"1200 RPM","SensorId":"/lpc/example/0/fan/0","ImageURL":"images/transparent.png","Children":[]}
              ]}
            ]}
          ]}
        ]}"#;
        let s = parse_data_json(json).unwrap();
        assert_eq!(s.len(), 2);
        assert_eq!(s[0].hardware, "Example Hybrid CPU");
        assert_eq!((s[0].kind, s[0].value), (SensorKind::Temperature, 61.5));
        assert_eq!((s[1].kind, s[1].value), (SensorKind::Fan, 880.0));
    }

    #[test]
    fn rejects_non_lhm_json() {
        assert!(parse_data_json("[]").is_err());
        assert!(parse_data_json("{\"hello\":1}").is_err());
        assert!(parse_data_json("<html>").is_err());
    }

    #[test]
    fn http_bodies() {
        let ok =
            b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 2\r\n\r\n{}";
        assert_eq!(http_body(ok).unwrap(), b"{}");
        let chunked =
            b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n4\r\n{\"a\"\r\n3;x=y\r\n:1}\r\n0\r\n\r\n";
        assert_eq!(http_body(chunked).unwrap(), b"{\"a\":1}");
        assert!(http_body(b"HTTP/1.1 404 Not Found\r\n\r\n").is_err());
        assert!(http_body(b"HTTP/1.1 200 OK\r\n").is_err());
    }

    #[test]
    fn unreachable_server_backs_off() {
        // Port 9 (discard) on loopback is closed on CI/dev boxes.
        let mut src = LhmHttp {
            addr: SocketAddr::from(([127, 0, 0, 1], 9)),
            retry_at: None,
        };
        assert!(src.read().is_none());
        assert!(src.retry_at.is_some());
        assert!(src.read().is_none());
    }
}
