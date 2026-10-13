//! Minimal Win32 layered-window PoT using the system ABI directly.
//!
//! No .NET runtime and no third-party native runtime are involved.

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

// NOTE: This is still incomplete - full restore in progress via automated upload.
// If you see this, the large-file upload needs another path.
pub type Hwnd = *mut c_void;
