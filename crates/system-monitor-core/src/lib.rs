//! Platform-independent contracts for the Rust system monitor port.
//!
//! Field meanings and units follow `docs/rust-port/function-task-breakdown.md`.

/// Per-drive metrics corresponding to the original `SystemMetrics.Disks` item.
#[derive(Clone, Debug, PartialEq)]
pub struct DiskMetric {
    /// Original physical disk instance name, for example `"0 C:"`.
    pub name: String,
    /// Used space in percent, in the inclusive range 0–100 when available.
    pub space_percent: f32,
    /// Disk activity in percent, capped at 100.
    pub activity_percent: f32,
}

/// Immutable telemetry snapshot; rates retain the original KiB/s semantics.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SystemMetrics {
    pub cpu_usage_percent: f32,
    pub ram_percent: f32,
    pub gpu_usage_percent: f32,
    /// `None` means the selected device/API has no temperature reading.
    pub gpu_temperature_c: Option<f32>,
    pub net_up_kbps: f32,
    pub net_down_kbps: f32,
    pub net_up_text: String,
    pub net_down_text: String,
    /// Maximum activity among selected physical disk instances.
    pub disk_usage_percent: f32,
    /// Capacity-weighted used-space percent across ready selected drives.
    pub disk_used_percent: f32,
    pub disks: Vec<DiskMetric>,
}
