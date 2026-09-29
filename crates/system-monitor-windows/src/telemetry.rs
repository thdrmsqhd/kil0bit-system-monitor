//! Non-blocking-friendly telemetry sampling primitives backed by sysinfo.
//! Snapshot collection keeps counter state per adapter so switching selection does not
//! turn an unrelated adapter's lifetime byte count into a transfer-rate spike.

use std::collections::HashMap;
use std::time::Instant;
use std::time::Duration;
use sysinfo::{Disks, Networks, System};
use system_monitor_core::{
    bytes_per_second_to_kib, format_network_rate, DiskMetric, SystemMetrics,
};

pub struct TelemetryCollector {
    system: System,
    networks: Networks,
    disks: Disks,
    previous_network_totals: HashMap<String, (u64, u64, Instant)>,
    adapter_totals: HashMap<String, (u64, u64)>,
    disk_counters: Option<super::pdh::CounterGroup>,
    gpu_counters: Option<super::pdh::CounterGroup>,
    last_counter_probe: Instant,
    last_disk_names: Vec<String>,
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
            previous_network_totals: HashMap::new(),
            adapter_totals: HashMap::new(),
            disk_counters: super::pdh::CounterGroup::new("\\PhysicalDisk(*)\\% Disk Time"),
            gpu_counters: super::pdh::CounterGroup::new("\\GPU Engine(*)\\Utilization Percentage"),
            last_counter_probe: Instant::now(),
            last_disk_names: Vec::new(),
        }
    }

    /// Poll one immutable metrics sample. A first network sample has zero rate because there is
    /// no previous counter value to subtract.
    pub fn sample(&mut self, config: &system_monitor_core::AppConfig) -> SystemMetrics {
        let now = Instant::now();
        self.system.refresh_cpu_usage();
        self.system.refresh_memory();
        self.networks.refresh(true);
        self.disks.refresh(true);
        let mut disk_names: Vec<_> = self.disks.iter()
            .map(|disk| disk.mount_point().to_string_lossy().into_owned()).collect();
        disk_names.sort();
        if disk_names != self.last_disk_names || now.duration_since(self.last_counter_probe) >= Duration::from_secs(30) {
            self.disk_counters = super::pdh::CounterGroup::new("\\PhysicalDisk(*)\\% Disk Time");
            self.gpu_counters = super::pdh::CounterGroup::new("\\GPU Engine(*)\\Utilization Percentage");
            self.last_disk_names = disk_names;
            self.last_counter_probe = now;
        }
        let mut up = 0.0;
        let mut down = 0.0;
        self.adapter_totals.clear();
        for (name, network) in &self.networks {
            if !is_eligible_adapter(name) {
                continue;
            }
            let rx = network.total_received();
            let tx = network.total_transmitted();
            if config.network_adapter == "Default" || config.network_adapter == name.as_str() {
                if let Some((old_rx, old_tx, previous)) = self.previous_network_totals.get(name) {
                    let elapsed = now.duration_since(*previous).as_secs_f32();
                    down += bytes_per_second_to_kib(rx.saturating_sub(*old_rx), elapsed);
                    up += bytes_per_second_to_kib(tx.saturating_sub(*old_tx), elapsed);
                }
            }
            self.adapter_totals.insert(name.clone(), (rx, tx));
            self.previous_network_totals
                .insert(name.clone(), (rx, tx, now));
        }
        self.previous_network_totals
            .retain(|name, _| self.adapter_totals.contains_key(name));
        let total_memory = self.system.total_memory();
        let used_memory = total_memory.saturating_sub(self.system.available_memory());
        let ram_percent = if total_memory == 0 {
            0.0
        } else {
            used_memory as f32 / total_memory as f32 * 100.0
        };
        let first_disk = self
            .disks
            .iter()
            .map(|disk| disk.mount_point().to_string_lossy().into_owned())
            .min();
        let selected = |name: &str| {
            (config.selected_disks == "Default" && first_disk.as_deref() == Some(name))
                || config.selected_disks == "All"
                || (config.selected_disks != "None"
                    && config
                        .selected_disks
                        .split(';')
                        .any(|item| item.trim() == name))
        };
        let disk_activity = self
            .disk_counters
            .as_ref()
            .map(|group| group.sample())
            .unwrap_or_default();
        let disk_metrics: Vec<DiskMetric> = self
            .disks
            .iter()
            .filter_map(|disk| {
                let name = disk.mount_point().to_string_lossy().into_owned();
                if !selected(&name) {
                    return None;
                }
                let total = disk.total_space();
                if total == 0 {
                    return None;
                }
                let used = total.saturating_sub(disk.available_space());
                let drive = name.trim_end_matches('\\');
                let activity_percent = disk_activity
                    .iter()
                    .filter(|(instance, _)| instance.contains(drive))
                    .map(|(_, usage)| *usage)
                    .fold(0.0, f32::max);
                Some(DiskMetric {
                    name,
                    space_percent: used as f32 / total as f32 * 100.0,
                    activity_percent,
                })
            })
            .collect();
        let total_space: u128 = self
            .disks
            .iter()
            .filter(|d| selected(&d.mount_point().to_string_lossy()))
            .map(|d| d.total_space() as u128)
            .sum();
        let used_space: u128 = self
            .disks
            .iter()
            .filter(|d| selected(&d.mount_point().to_string_lossy()))
            .map(|d| d.total_space().saturating_sub(d.available_space()) as u128)
            .sum();
        let disk_used_percent = if total_space == 0 {
            0.0
        } else {
            used_space as f32 / total_space as f32 * 100.0
        };
        let gpu = if config.show_gpu || config.show_temp {
            super::gpu::sample_nvidia(config.gpu_index as usize)
                .or_else(|| super::gpu::sample_amd(config.gpu_index as usize))
        } else {
            None
        };
        let gpu_samples = if config.show_gpu || config.show_temp {
            self.gpu_counters.as_ref().map(|group| group.sample()).unwrap_or_default()
        } else { Vec::new() };
        let selected_gpu = gpu_samples.iter().filter(|(name, _)| {
            name.to_ascii_lowercase()
                .split("_phys_").nth(1)
                .and_then(|tail| tail.split('_').next())
                .and_then(|index| index.parse::<u32>().ok()) == Some(config.gpu_index)
        });
        let fallback_gpu = selected_gpu.clone().map(|(_, usage)| *usage).reduce(f32::max);
        let gpu_temperature_c = gpu.as_ref().and_then(|v| v.temperature_c)
            .or_else(|| selected_gpu.clone()
                .find_map(|(name, _)| super::gpu::sample_d3dkmt_temperature(name)));
        let disk_usage_percent = disk_metrics
            .iter()
            .map(|disk| disk.activity_percent)
            .fold(0.0, f32::max);
        SystemMetrics {
            cpu_usage_percent: self.system.global_cpu_usage().clamp(0.0, 100.0),
            ram_percent: ram_percent.clamp(0.0, 100.0),
            net_up_kbps: up,
            net_down_kbps: down,
            net_up_text: format_network_rate(up),
            net_down_text: format_network_rate(down),
            disk_used_percent: disk_used_percent.clamp(0.0, 100.0),
            disk_usage_percent,
            disk_activity_available: !disk_activity.is_empty(),
            disks: disk_metrics,
            gpu_usage_percent: gpu
                .as_ref()
                .map(|v| v.usage_percent)
                .or(fallback_gpu)
                .unwrap_or(0.0),
            gpu_usage_available: gpu.is_some() || fallback_gpu.is_some(),
            gpu_temperature_c,
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

pub(crate) fn is_eligible_adapter(name: &str) -> bool {
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
