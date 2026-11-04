//! System statistics monitoring
//!
//! This module handles collection and monitoring of system-wide statistics
//! including CPU, memory, network, and disk usage.

use super::{InterfaceDetails, ResourceDetails, SystemIdentity, SystemStats, VolumeDetails};
use std::collections::BTreeMap;
use std::fmt::Debug;
use std::time::Instant;
use sysinfo::{Disk, Disks, Networks, System};

/// Monitors system-wide statistics
#[derive(Debug)]
pub struct SystemMonitor {
    /// Tracks network usage between updates
    last_network_update: Instant,
    last_interfaces: BTreeMap<String, (u64, u64)>,
    /// Fob: per-mount (read, written) byte totals at the last reading.
    last_disk_update: Instant,
    last_disks: BTreeMap<String, (u64, u64)>,
    identity: Option<SystemIdentity>,
}

impl SystemMonitor {
    /// Creates a new system monitor instance
    ///
    /// # Arguments
    ///
    /// * `sys` - System information provider for initial readings
    pub fn new(networks: &Networks) -> Self {
        Self {
            last_network_update: Instant::now(),
            last_interfaces: networks
                .iter()
                .map(|(name, data)| {
                    (
                        name.clone(),
                        (data.total_received(), data.total_transmitted()),
                    )
                })
                .collect(),
            last_disk_update: Instant::now(),
            last_disks: BTreeMap::new(),
            identity: None,
        }
    }

    /// Collects current system statistics
    ///
    /// # Arguments
    ///
    /// * `sys` - System information provider
    pub fn collect_stats(
        &mut self,
        sys: &System,
        networks: &Networks,
        disks: &Disks,
    ) -> SystemStats {
        let (network_rx, network_tx, interfaces) = self.calculate_network_stats(networks);
        let (disk_total, disk_used, disk_free) = self.calculate_disk_stats(disks);
        let disk_rates = self.calculate_disk_rates(disks);
        let counted: Vec<String> = Self::filter_disks(disks)
            .map(|d| d.mount_point().to_string_lossy().into_owned())
            .collect();
        let (disk_read, disk_write) = {
            let rates = || {
                counted
                    .iter()
                    .filter_map(|m| disk_rates.get(m).copied())
                    .filter(|r| r.0.is_some())
            };
            if rates().next().is_none() {
                (None, None)
            } else {
                (
                    Some(rates().filter_map(|r| r.0).sum::<u64>()),
                    Some(rates().filter_map(|r| r.1).sum::<u64>()),
                )
            }
        };
        let load_average = System::load_average();

        SystemStats {
            details: ResourceDetails {
                identity: self
                    .identity
                    .get_or_insert_with(|| SystemIdentity {
                        cpu_model: fake_identity("WINPERF_FAKE_CPU").or_else(|| {
                            sys.cpus()
                                .first()
                                .map(|c| c.brand().trim().to_owned())
                                .filter(|s| !s.is_empty())
                        }),
                        physical_cores: System::physical_core_count(),
                        architecture: std::env::consts::ARCH.into(),
                        platform: std::env::consts::OS.into(),
                        os_version: fake_identity("WINPERF_FAKE_OS")
                            .or_else(System::long_os_version)
                            .or_else(System::name),
                        kernel_version: System::kernel_version(),
                    })
                    .clone(),
                memory_unallocated: sys.free_memory(),
                swap_total: sys.total_swap(),
                swap_used: sys.used_swap(),
                volumes: {
                    let mut volumes = disks
                        .iter()
                        .map(|disk| VolumeDetails {
                            name: disk.name().to_string_lossy().into_owned(),
                            mount: disk.mount_point().to_string_lossy().into_owned(),
                            file_system: disk.file_system().to_string_lossy().into_owned(),
                            total: disk.total_space(),
                            available: disk.available_space(),
                            read_only: disk.is_read_only(),
                            read: disk_rates
                                .get(&*disk.mount_point().to_string_lossy())
                                .and_then(|r| r.0),
                            written: disk_rates
                                .get(&*disk.mount_point().to_string_lossy())
                                .and_then(|r| r.1),
                        })
                        .collect::<Vec<_>>();
                    volumes.sort_by(|a, b| a.mount.cmp(&b.mount));
                    volumes
                },
                interfaces,
            },
            cpu_usage: sys.cpus().iter().map(|cpu| cpu.cpu_usage()).collect(),
            memory_total: sys.total_memory(),
            memory_used: sys.used_memory(),
            memory_free: sys.total_memory() - sys.used_memory(),
            memory_cached: sys.total_memory()
                - (sys.used_memory() + (sys.total_memory() - sys.used_memory())),
            uptime: System::uptime(),
            load_avg: [load_average.one, load_average.five, load_average.fifteen],
            network_rx_bytes: network_rx,
            network_tx_bytes: network_tx,
            disk_total_bytes: disk_total,
            disk_used_bytes: disk_used,
            disk_free_bytes: disk_free,
            disk_read_bytes: disk_read,
            disk_write_bytes: disk_write,
        }
    }

    /// Fob: read/write rates per mount from the I/O totals sysinfo reports
    /// (Windows: IOCTL_DISK_PERFORMANCE). A drive whose totals are both 0
    /// reports no counters (the query failed), so it gets None, not 0 B/s.
    fn calculate_disk_rates(
        &mut self,
        disks: &Disks,
    ) -> BTreeMap<String, (Option<u64>, Option<u64>)> {
        let elapsed = self.last_disk_update.elapsed().as_secs_f64();
        let mut next = BTreeMap::new();
        let mut rates = BTreeMap::new();
        for disk in disks.iter() {
            let usage = disk.usage();
            let totals = (usage.total_read_bytes, usage.total_written_bytes);
            let mount = disk.mount_point().to_string_lossy().into_owned();
            let rate = disk_rates(self.last_disks.get(&mount).copied(), totals, elapsed);
            rates.insert(mount.clone(), rate);
            next.insert(mount, totals);
        }
        self.last_disk_update = Instant::now();
        self.last_disks = next;
        rates
    }

    /// Filters disks based on platform-specific criteria
    #[cfg(not(target_os = "windows"))]
    fn filter_disks(disks: &[Disk]) -> impl Iterator<Item = &Disk> {
        disks
            .iter()
            .filter(|disk| disk.mount_point() == std::path::Path::new("/"))
    }

    /// Windows-specific disk filtering
    #[cfg(target_os = "windows")]
    fn filter_disks(disks: &[Disk]) -> impl Iterator<Item = &Disk> {
        disks.iter()
    }

    /// Calculates network usage rates
    fn calculate_network_stats(
        &mut self,
        networks: &Networks,
    ) -> (Option<u64>, Option<u64>, Vec<InterfaceDetails>) {
        let elapsed = self.last_network_update.elapsed().as_secs_f64();
        let mut interfaces = Vec::new();
        let mut next = BTreeMap::new();
        for (name, data) in networks.iter() {
            let totals = (data.total_received(), data.total_transmitted());
            let (received, sent) =
                interface_rates(self.last_interfaces.get(name).copied(), totals, elapsed);
            interfaces.push(InterfaceDetails {
                name: name.clone(),
                received,
                sent,
                received_total: totals.0,
                sent_total: totals.1,
            });
            next.insert(name.clone(), totals);
        }
        interfaces.sort_by(|a, b| a.name.cmp(&b.name));
        let rx_rate = rate_total(interfaces.iter().map(|i| i.received));
        let tx_rate = rate_total(interfaces.iter().map(|i| i.sent));

        self.last_network_update = Instant::now();
        self.last_interfaces = next;
        (rx_rate, tx_rate, interfaces)
    }

    /// Calculates disk usage statistics and returns `(total, used, free)`.
    fn calculate_disk_stats(&self, disks: &Disks) -> (u64, u64, u64) {
        Self::filter_disks(disks).fold((0, 0, 0), |(total, used, free), disk| {
            (
                total + disk.total_space(),
                used + (disk.total_space() - disk.available_space()),
                free + disk.available_space(),
            )
        })
    }
}

fn rate_total(mut rates: impl Iterator<Item = Option<u64>>) -> Option<u64> {
    let first = rates.next()??;
    rates.try_fold(first, |total, value| Some(total.saturating_add(value?)))
}

fn network_rate(current: u64, previous: u64, elapsed: f64) -> Option<u64> {
    if elapsed <= 0.0 || !elapsed.is_finite() {
        return None;
    }
    current
        .checked_sub(previous)
        .map(|delta| (delta as f64 / elapsed) as u64)
}

fn interface_rates(
    previous: Option<(u64, u64)>,
    current: (u64, u64),
    elapsed: f64,
) -> (Option<u64>, Option<u64>) {
    previous
        .map(|p| {
            (
                network_rate(current.0, p.0, elapsed),
                network_rate(current.1, p.1, elapsed),
            )
        })
        .unwrap_or((None, None))
}

/// Fob: like interface_rates, but (0, 0) totals mean "no counters".
fn disk_rates(
    previous: Option<(u64, u64)>,
    totals: (u64, u64),
    elapsed: f64,
) -> (Option<u64>, Option<u64>) {
    if totals == (0, 0) {
        (None, None)
    } else {
        interface_rates(previous, totals, elapsed)
    }
}

/// Debug builds only (screenshots with real-world long names):
/// WINPERF_FAKE_OS / WINPERF_FAKE_CPU replace the OS and CPU names.
fn fake_identity(var: &str) -> Option<String> {
    if !cfg!(debug_assertions) {
        return None;
    }
    std::env::var(var).ok().filter(|s| !s.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disk_rates_need_two_readings_and_real_counters() {
        assert_eq!(disk_rates(None, (4096, 0), 1.0), (None, None));
        assert_eq!(
            disk_rates(Some((1000, 500)), (5000, 2500), 2.0),
            (Some(2000), Some(1000))
        );
        // A drive whose counters could not be read stays unknown, not idle.
        assert_eq!(disk_rates(Some((0, 0)), (0, 0), 1.0), (None, None));
    }
    use sysinfo::System;

    #[test]
    fn counter_resets_are_unknown_not_underflows() {
        assert_eq!(network_rate(10, 20, 2.0), None);
        assert_eq!(network_rate(20, 20, 2.0), Some(0));
        assert_eq!(network_rate(40, 20, 2.0), Some(10));
        assert_eq!(network_rate(40, 20, 0.0), None);
    }

    #[test]
    fn new_and_reset_interfaces_do_not_invent_traffic() {
        assert_eq!(interface_rates(None, (10000, 20000), 2.0), (None, None));
        assert_eq!(
            interface_rates(Some((100, 200)), (140, 200), 2.0),
            (Some(20), Some(0))
        );
        assert_eq!(
            interface_rates(Some((100, 200)), (40, 220), 2.0),
            (None, Some(10))
        );
        assert_eq!(
            interface_rates(Some((100, 200)), (140, 220), 0.0),
            (None, None)
        );
    }

    #[test]
    fn aggregate_rates_require_complete_current_interface_samples() {
        assert_eq!(
            rate_total([Some(20), Some(0), Some(10)].into_iter()),
            Some(30)
        );
        assert_eq!(rate_total([Some(20), None].into_iter()), None);
        assert_eq!(rate_total([None, Some(20)].into_iter()), None);
        assert_eq!(rate_total([].into_iter()), None);
        // Removing an interface does not subtract its lifetime byte counter.
        assert_eq!(rate_total([Some(20)].into_iter()), Some(20));
    }

    /// Tests creation of system monitor
    #[test]
    fn test_system_monitor_creation() {
        let networks = Networks::new();
        let monitor = SystemMonitor::new(&networks);
        assert!(monitor.last_interfaces.is_empty());
    }

    /// Tests system statistics collection
    #[test]
    fn test_stats_collection() {
        let mut networks = Networks::new();
        let mut monitor = SystemMonitor::new(&networks);
        networks.refresh(true);
        let sys = System::new_all();
        let disks = Disks::new_with_refreshed_list();

        let stats = monitor.collect_stats(&sys, &networks, &disks);
        assert!(!stats.cpu_usage.is_empty());
        assert!(stats.memory_total > 0);
    }
}
