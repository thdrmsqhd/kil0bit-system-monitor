#![cfg_attr(windows, windows_subsystem = "windows")]
#![cfg_attr(all(not(windows), feature = "win32-api-check"), allow(dead_code))]

mod bitmap;

#[cfg(any(windows, feature = "win32-api-check"))]
mod win32;

#[cfg(windows)]
fn main() {
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
    #[cfg(feature = "win32-api-check")]
    let _ = std::mem::size_of::<win32::WindowClass>();
    eprintln!("system-monitor-windows must be built and run on Windows");
}
