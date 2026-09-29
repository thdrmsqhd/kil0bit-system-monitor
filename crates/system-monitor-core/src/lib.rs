//! Platform-independent contracts for the Rust system monitor port.
//!
//! Field meanings and units follow `docs/rust-port/function-task-breakdown.md`.

use serde::{Deserialize, Serialize};

/// Versioned, Rust-owned settings. Field defaults map to the source `AppConfig`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "PascalCase")]
pub struct AppConfig {
    pub schema_version: u32,
    pub show_overlay: bool,
    pub lock_position: bool,
    pub launch_on_startup: bool,
    pub show_cpu: bool,
    pub show_ram: bool,
    pub show_gpu: bool,
    pub show_temp: bool,
    pub show_disk: bool,
    pub show_disk_speed: bool,
    pub show_net_up: bool,
    pub show_net_down: bool,
    pub network_adapter: String,
    pub gpu_adapter: String,
    pub selected_disks: String,
    pub display_style: String,
    pub font_family: String,
    pub accent_color_hex: String,
    pub label_color_hex: String,
    pub x: f64,
    pub y: f64,
    pub hide_on_fullscreen: bool,
    pub stick_to_taskbar: bool,
    pub show_background: bool,
    pub background_color_hex: String,
    pub scale_factor: f64,
    pub is_text_bold: bool,
    pub column_spacing: u8,
    pub theme: String,
    #[serde(rename = "UpdateInterval")]
    pub update_interval_ms: u32,
    pub gpu_index: u32,
    pub show_pods: bool,
    pub pod_color_hex: String,
    pub always_on_top: bool,
    pub opencode_enabled: bool,
    pub codex_enabled: bool,
    pub deepseek_enabled: bool,
    pub opencode_show_rolling: bool,
    pub opencode_show_weekly: bool,
    pub opencode_show_monthly: bool,
    pub ai_show_used_percent: bool,
    pub ai_poll_interval_seconds: u32,
    pub ai_warning_threshold_percent: u8,
    pub net_label_color_hex: Option<String>,
    pub cpu_ram_label_color_hex: Option<String>,
    pub gpu_label_color_hex: Option<String>,
    pub disk_label_color_hex: Option<String>,
    pub net_accent_color_hex: Option<String>,
    pub cpu_ram_accent_color_hex: Option<String>,
    pub gpu_accent_color_hex: Option<String>,
    pub disk_accent_color_hex: Option<String>,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            schema_version: 1,
            show_overlay: true,
            lock_position: false,
            launch_on_startup: false,
            show_cpu: true,
            show_ram: true,
            show_gpu: true,
            show_temp: true,
            show_disk: true,
            show_disk_speed: true,
            show_net_up: true,
            show_net_down: true,
            network_adapter: "Default".into(),
            gpu_adapter: "Default".into(),
            selected_disks: "Default".into(),
            display_style: "Text".into(),
            font_family: "Segoe UI".into(),
            accent_color_hex: "#FFFFFF".into(),
            label_color_hex: "#00CCFF".into(),
            x: 100.0,
            y: 100.0,
            hide_on_fullscreen: true,
            stick_to_taskbar: true,
            show_background: false,
            background_color_hex: "#B4141414".into(),
            scale_factor: 1.0,
            is_text_bold: true,
            column_spacing: 6,
            theme: "Default".into(),
            update_interval_ms: 1000,
            gpu_index: 0,
            show_pods: true,
            pod_color_hex: "#0FFFFFFF".into(),
            always_on_top: true,
            opencode_enabled: false,
            codex_enabled: false,
            deepseek_enabled: false,
            opencode_show_rolling: true,
            opencode_show_weekly: true,
            opencode_show_monthly: true,
            ai_show_used_percent: false,
            ai_poll_interval_seconds: 300,
            ai_warning_threshold_percent: 80,
            net_label_color_hex: None,
            cpu_ram_label_color_hex: None,
            gpu_label_color_hex: None,
            disk_label_color_hex: None,
            net_accent_color_hex: None,
            cpu_ram_accent_color_hex: None,
            gpu_accent_color_hex: None,
            disk_accent_color_hex: None,
        }
    }
}

impl AppConfig {
    /// Clamp persisted values to the original settings' supported ranges.
    pub fn normalize(&mut self) {
        self.schema_version = 1;
        self.scale_factor = if self.scale_factor.is_finite() {
            self.scale_factor.clamp(0.5, 2.0)
        } else {
            1.0
        };
        self.column_spacing = self.column_spacing.min(20);
        self.update_interval_ms = [500_u32, 1000, 2000, 5000]
            .into_iter()
            .min_by_key(|candidate| candidate.abs_diff(self.update_interval_ms))
            .unwrap_or(1000);
        if !self.x.is_finite() {
            self.x = 100.0;
        }
        if !self.y.is_finite() {
            self.y = 100.0;
        }
        if self.display_style != "Compact" {
            self.display_style = "Text".into();
        }
        self.ai_poll_interval_seconds = self.ai_poll_interval_seconds.clamp(60, 3600);
        self.ai_warning_threshold_percent = self.ai_warning_threshold_percent.min(100);
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AiUsageWindow {
    pub used_percent: u8,
    pub reset_at_epoch_seconds: Option<u64>,
}

impl AiUsageWindow {
    pub fn remaining_percent(&self) -> u8 {
        100 - self.used_percent
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct AiUsageSnapshot {
    pub rolling: Option<AiUsageWindow>,
    pub weekly: Option<AiUsageWindow>,
    pub monthly: Option<AiUsageWindow>,
    pub updated_at_epoch_seconds: u64,
    pub stale: bool,
}

/// Exponential retry, capped at one hour and never below the configured normal interval.
pub fn ai_retry_delay_seconds(configured_seconds: u32, consecutive_failures: u32) -> u32 {
    let base = configured_seconds.clamp(60, 3600);
    let exponent = consecutive_failures.saturating_sub(1).min(10);
    base.saturating_mul(1_u32 << exponent).min(3600)
}

/// Safe persistence for the Rust-specific profile; never writes the legacy path.
pub mod config_store {
    use super::AppConfig;
    use std::fs;
    use std::io::{self, Write};
    use std::path::{Path, PathBuf};

    pub fn load(path: &Path) -> AppConfig {
        fs::read(path)
            .ok()
            .and_then(|bytes| serde_json::from_slice::<AppConfig>(&bytes).ok())
            .map(|mut config| {
                config.normalize();
                config
            })
            .unwrap_or_default()
    }

    pub fn save(path: &Path, config: &AppConfig) -> io::Result<()> {
        let mut normalized = config.clone();
        normalized.normalize();
        let bytes = serde_json::to_vec_pretty(&normalized)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
        let parent = path.parent().ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidInput, "config path has no parent")
        })?;
        fs::create_dir_all(parent)?;
        let temp = temp_path(path);
        let mut file = fs::File::create(&temp)?;
        file.write_all(&bytes)?;
        file.sync_all()?;
        drop(file);
        if let Ok(previous) = fs::read(path) {
            let suffix = if serde_json::from_slice::<AppConfig>(&previous).is_ok() {
                "bak"
            } else {
                "corrupt.bak"
            };
            fs::copy(path, backup_path(path, suffix))?;
        }
        replace(&temp, path)?;
        Ok(())
    }

    /// Restore the most recent valid Rust-owned config; the original app is never accessed.
    pub fn restore_backup(path: &Path) -> io::Result<AppConfig> {
        let backup = backup_path(path, "bak");
        let bytes = fs::read(&backup)?;
        let mut config: AppConfig = serde_json::from_slice(&bytes)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
        config.normalize();
        save(path, &config)?;
        Ok(config)
    }

    /// Read a legacy config once, preserving its bytes and tolerating unknown fields.
    pub fn import_legacy(path: &Path) -> io::Result<AppConfig> {
        import_legacy_with_report(path).map(|(config, _)| config)
    }

    /// Returns unsupported source keys so the user can review what was not imported.
    pub fn import_legacy_with_report(path: &Path) -> io::Result<(AppConfig, Vec<String>)> {
        let bytes = fs::read(path)?;
        let source: serde_json::Value = serde_json::from_slice(&bytes)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
        let known = serde_json::to_value(AppConfig::default())
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
        let mut unsupported: Vec<String> = source.as_object().into_iter().flat_map(|map| map.keys())
            .filter(|key| !known.as_object().is_some_and(|map| map.contains_key(*key)))
            .cloned().collect();
        unsupported.sort();
        let mut config: AppConfig = serde_json::from_slice(&bytes)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
        config.normalize();
        Ok((config, unsupported))
    }

    fn temp_path(path: &Path) -> PathBuf {
        let mut name = path.as_os_str().to_owned();
        name.push(".tmp");
        PathBuf::from(name)
    }

    fn backup_path(path: &Path, suffix: &str) -> PathBuf {
        let mut name = path.as_os_str().to_owned();
        name.push(format!(".{suffix}"));
        PathBuf::from(name)
    }

    #[cfg(windows)]
    fn replace(temp: &Path, path: &Path) -> io::Result<()> {
        use std::os::windows::ffi::OsStrExt;
        #[link(name = "kernel32")]
        extern "system" {
            fn MoveFileExW(existing: *const u16, new: *const u16, flags: u32) -> i32;
        }
        const MOVEFILE_REPLACE_EXISTING: u32 = 0x1;
        const MOVEFILE_WRITE_THROUGH: u32 = 0x8;
        let existing: Vec<u16> = temp.as_os_str().encode_wide().chain(Some(0)).collect();
        let new: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
        if unsafe {
            MoveFileExW(
                existing.as_ptr(),
                new.as_ptr(),
                MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
            )
        } != 0
        {
            Ok(())
        } else {
            Err(io::Error::last_os_error())
        }
    }
    #[cfg(not(windows))]
    fn replace(temp: &Path, path: &Path) -> io::Result<()> {
        fs::rename(temp, path)
    }
}

#[cfg(test)]
mod config_tests {
    use super::{config_store, AppConfig};
    use std::path::PathBuf;

    fn temp_dir(label: &str) -> PathBuf {
        let path =
            std::env::temp_dir().join(format!("kil0bit-rust-{label}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).unwrap();
        path
    }

    #[test]
    fn defaults_match_catalog_and_normalize_ranges() {
        let mut config = AppConfig::default();
        assert_eq!(config.update_interval_ms, 1000);
        assert_eq!(config.display_style, "Text");
        config.scale_factor = f64::NAN;
        config.column_spacing = 255;
        config.update_interval_ms = 3300;
        config.normalize();
        assert_eq!(config.scale_factor, 1.0);
        assert_eq!(config.column_spacing, 20);
        assert_eq!(config.update_interval_ms, 2000);
    }

    #[test]
    fn save_and_load_round_trip_preserves_settings() {
        let dir = temp_dir("config-roundtrip");
        let path = dir.join("settings.json");
        let config = AppConfig {
            show_cpu: false,
            x: 521.0,
            ..AppConfig::default()
        };
        config_store::save(&path, &config).unwrap();
        assert_eq!(config_store::load(&path), config);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn legacy_import_is_read_only_and_ignores_unknown_fields() {
        let dir = temp_dir("legacy-import");
        let path = dir.join("config.json");
        let bytes =
            br#"{"ShowCpu":false,"UpdateInterval":2000,"ScaleFactor":1.5,"LegacyField":"keep"}"#;
        std::fs::write(&path, bytes).unwrap();
        let config = config_store::import_legacy(&path).unwrap();
        assert!(!config.show_cpu);
        assert_eq!(config.update_interval_ms, 2000);
        assert_eq!(config.scale_factor, 1.5);
        assert_eq!(std::fs::read(&path).unwrap(), bytes);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn malformed_config_uses_defaults_without_overwriting_source() {
        let dir = temp_dir("config-corrupt");
        let path = dir.join("settings.json");
        let bytes = b"{invalid";
        std::fs::write(&path, bytes).unwrap();
        assert_eq!(config_store::load(&path), AppConfig::default());
        assert_eq!(std::fs::read(&path).unwrap(), bytes);
        std::fs::remove_dir_all(dir).unwrap();
    }
}

#[cfg(test)]
mod network_contract_tests {
    use super::{bytes_per_second_to_kib, format_network_rate};

    #[test]
    fn network_format_matches_original_unit_boundaries() {
        assert_eq!(format_network_rate(99.9), "99.9 KB/s");
        assert_eq!(format_network_rate(100.0), "100 KB/s");
        assert_eq!(format_network_rate(1024.0), "1.0 MB/s");
        assert_eq!(format_network_rate(1024.0 * 100.0), "100 MB/s");
        assert_eq!(format_network_rate(1024.0 * 1024.0), "1.0 GB/s");
    }

    #[test]
    fn byte_counter_deltas_use_kib_per_second_and_reject_invalid_intervals() {
        assert_eq!(bytes_per_second_to_kib(2048, 2.0), 1.0);
        assert_eq!(bytes_per_second_to_kib(2048, 0.0), 0.0);
        assert_eq!(bytes_per_second_to_kib(2048, f32::NAN), 0.0);
    }
}

#[cfg(test)]
mod ai_usage_tests {
    use super::ai_retry_delay_seconds;

    #[test]
    fn retry_delay_uses_minimum_interval_and_caps_growth() {
        assert_eq!(ai_retry_delay_seconds(20, 0), 60);
        assert_eq!(ai_retry_delay_seconds(300, 1), 300);
        assert_eq!(ai_retry_delay_seconds(300, 2), 600);
        assert_eq!(ai_retry_delay_seconds(300, 3), 1200);
        assert_eq!(ai_retry_delay_seconds(300, 99), 3600);
    }
}

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
    pub gpu_usage_available: bool,
    /// `None` means the selected device/API has no temperature reading.
    pub gpu_temperature_c: Option<f32>,
    pub net_up_kbps: f32,
    pub net_down_kbps: f32,
    pub net_up_text: String,
    pub net_down_text: String,
    /// Maximum activity among selected physical disk instances.
    pub disk_usage_percent: f32,
    pub disk_activity_available: bool,
    /// Capacity-weighted used-space percent across ready selected drives.
    pub disk_used_percent: f32,
    pub disks: Vec<DiskMetric>,
}

/// Matches the original `FormatNet` thresholds; values are KiB/s despite the legacy field name.
pub fn format_network_rate(kib_per_second: f32) -> String {
    let value = kib_per_second.max(0.0);
    if value >= 1024.0 * 1024.0 {
        format!("{:.1} GB/s", value / 1024.0 / 1024.0)
    } else if value >= 1024.0 {
        let mb = value / 1024.0;
        if mb >= 100.0 {
            format!("{mb:.0} MB/s")
        } else {
            format!("{mb:.1} MB/s")
        }
    } else if value >= 100.0 {
        format!("{value:.0} KB/s")
    } else {
        format!("{value:.1} KB/s")
    }
}

/// Convert counter deltas to the original KiB/s unit, protecting reconnect/reset cases.
pub fn bytes_per_second_to_kib(delta_bytes: u64, elapsed_seconds: f32) -> f32 {
    if !elapsed_seconds.is_finite() || elapsed_seconds <= 0.0 {
        return 0.0;
    }
    delta_bytes as f32 / 1024.0 / elapsed_seconds
}
