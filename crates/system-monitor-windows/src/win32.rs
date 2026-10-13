//! Minimal Win32 layered-window PoT using the system ABI directly.
//!
//! No .NET runtime and no third-party native runtime are involved.
//!
//! Function bodies live in `win32_part{0,1,2}.rs` and are included into this
//! module so the large implementation can be updated without a single-blob limit.

use std::ffi::c_void;
use std::fmt;
use std::mem::{size_of, zeroed};
use std::ptr::{null, null_mut};
use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};
use std::sync::{Mutex, OnceLock};
use std::sync::mpsc::{self, Sender};
use std::thread::JoinHandle;
use std::time::Duration;
use system_monitor_core::{config_store, AiUsageSnapshot, AppConfig, SystemMetrics};
use zeroize::Zeroize;

pub type Hwnd = *mut c_void;
type Hinstance = *mut c_void;
type Hdc = *mut c_void;
type Hbitmap = *mut c_void;
type Hgdiobj = *mut c_void;
type Lresult = isize;
type WndProc = unsafe extern "system" fn(Hwnd, u32, usize, isize) -> Lresult;

// Full constants/statics/externs and function includes are restored in the
// following commits (win32_part0/1/2). Temporary compile stub:
#[allow(dead_code)]
pub fn run(_pixels: &[u8], _width: i32, _height: i32) -> Result<(), String> {
    Err("win32 body restore in progress — pull latest after part uploads".into())
}
