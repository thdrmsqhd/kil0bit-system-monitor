//! Non-blocking-friendly telemetry sampling primitives backed by sysinfo.
//! GPU readings remain explicitly unavailable until vendor providers are added.

use std::collections::HashMap;
use std::time::Instant;
use sysinfo::{Disks, Networks, System};
use system_monitor_core::{
    bytes_per_second_to_kib, format_network_rate, DiskMetric, SystemMetrics,
};

pub struct TelemetryCollector {
    system: System,
    networks: Networks,
    disks: Disks,
    previous_network_totals: Option<(u64, u64, Instant)>,
    adapter_totals: HashMap<String, (u64, u64)>,
}

impl TelemetryCollector {
    pub fn new() -> Self {
        let mut system = System::new();
        system.refresh_memory();
        system.refresh_cpu_usage();
        let networks = Networks::new_with_refreshed_list();
        let disks = Disks::new_with_refreshed_list();
        Self {
            system,
            networks,
            disks,
            previous_network_totals: None,
            adapter_totals: HashMap::new(),
        }
    }

    /// Poll one immutable metrics sample. A first network sample has zero rate because there is
    /// no previous counter value to subtract.
    pub fn sample(&mut self) -> SystemMetrics {
        let now = Instant::now();
        self.system.refresh_cpu_usage();
        self.system.refresh_memory();
        self.networks.refresh(true);
        self.disks.refresh(true);
        let mut received = 0_u64;
        let mut transmitted = 0_u64;
        self.adapter_totals.clear();
        for (name, network) in &self.networks {
            if !is_eligible_adapter(name) {
                continue;
            }
            let rx = network.total_received();
            let tx = network.total_transmitted();
            received = received.saturating_add(rx);
            transmitted = transmitted.saturating_add(tx);
            self.adapter_totals.insert(name.clone(), (rx, tx));
        }
        let (up, down) = self
            .previous_network_totals
            .map(|(old_rx, old_tx, previous)| {
                let elapsed = now.duration_since(previous).as_secs_f32();
                (
                    bytes_per_second_to_kib(transmitted.saturating_sub(old_tx), elapsed),
                    bytes_per_second_to_kib(received.saturating_sub(old_rx), elapsed),
                )
            })
            .unwrap_or((0.0, 0.0));
        self.previous_network_totals = Some((received, transmitted, now));
        let total_memory = self.system.total_memory();
        let used_memory = total_memory.saturating_sub(self.system.available_memory());
        let ram_percent = if total_memory == 0 {
            0.0
        } else {
            used_memory as f32 / total_memory as f32 * 100.0
        };
        let disk_metrics: Vec<DiskMetric> = self
            .disks
            .iter()
            .filter_map(|disk| {
                let total = disk.total_space();
                if total == 0 {
                    return None;
                }
                let used = total.saturating_sub(disk.available_space());
                Some(DiskMetric {
                    name: disk.mount_point().to_string_lossy().into_owned(),
                    space_percent: used as f32 / total as f32 * 100.0,
                    activity_percent: 0.0,
                })
            })
            .collect();
        let total_space: u128 = self.disks.iter().map(|d| d.total_space() as u128).sum();
        let used_space: u128 = self
            .disks
            .iter()
            .map(|d| d.total_space().saturating_sub(d.available_space()) as u128)
            .sum();
        let disk_used_percent = if total_space == 0 {
            0.0
        } else {
            used_space as f32 / total_space as f32 * 100.0
        };
        SystemMetrics {
            cpu_usage_percent: self.system.global_cpu_usage().clamp(0.0, 100.0),
            ram_percent: ram_percent.clamp(0.0, 100.0),
            net_up_kbps: up,
            net_down_kbps: down,
            net_up_text: format_network_rate(up),
            net_down_text: format_network_rate(down),
            disk_used_percent: disk_used_percent.clamp(0.0, 100.0),
            disks: disk_metrics,
            gpu_temperature_c: None,
            ..SystemMetrics::default()
        }
    }

    pub fn adapter_names(&self) -> Vec<String> {
        let mut names: Vec<_> = self.adapter_totals.keys().cloned().collect();
        names.sort();
        names
    }

    pub fn network_totals(&self) -> &HashMap<String, (u64, u64)> {
        &self.adapter_totals
    }

    pub fn disks(&self) -> &Disks {
        &self.disks
    }
}

impl Default for TelemetryCollector {
    fn default() -> Self {
        Self::new()
    }
}

fn is_eligible_adapter(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    !(lower.contains("loopback") || lower.contains("virtual") || lower.contains("pseudo"))
}

#[cfg(test)]
mod tests {
    use super::is_eligible_adapter;

    #[test]
    fn excludes_loopback_and_pseudo_interfaces() {
        assert!(!is_eligible_adapter("Loopback Pseudo-Interface 1"));
        assert!(!is_eligible_adapter("Virtual Ethernet"));
        assert!(is_eligible_adapter("Wi-Fi"));
        assert!(is_eligible_adapter("Ethernet 2"));
    }
}
