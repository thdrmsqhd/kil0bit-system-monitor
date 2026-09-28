#![cfg_attr(windows, windows_subsystem = "windows")]
#![cfg_attr(all(not(windows), feature = "win32-api-check"), allow(dead_code))]

pub mod ai_usage;
mod bitmap;
pub mod secret_store;
pub mod telemetry;

#[cfg(any(windows, feature = "win32-api-check"))]
mod win32;

#[cfg(windows)]
fn main() {
    let mut telemetry = telemetry::TelemetryCollector::new();
    let _initial_sample = telemetry.sample();
    if let Err(error) = win32::run(
        &bitmap::build_demo_bitmap(false),
        bitmap::WIDTH,
        bitmap::HEIGHT,
    ) {
        eprintln!("Win32 overlay PoT failed: {error}");
        std::process::exit(1);
    }
}

#[cfg(not(windows))]
fn main() {
    let _demo = bitmap::build_demo_bitmap(false);
    let _scaled_demo = bitmap::scale_bitmap(&_demo, 100);
    let mut telemetry = telemetry::TelemetryCollector::new();
    let _sample = telemetry.sample();
    #[cfg(feature = "win32-api-check")]
    let _ = std::mem::size_of::<win32::WindowClass>();
    eprintln!("system-monitor-windows must be built and run on Windows");
}
