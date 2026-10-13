//! Minimal Win32 layered-window PoT using the system ABI directly.
//!
//! No .NET runtime and no third-party native runtime are involved.
//!
//! Function bodies are split across win32_p*.rs (include!) so large updates
//! stay under per-file upload limits while compiling as one module.

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

const WS_POPUP: u32 = 0x8000_0000;
const WS_CAPTION: isize = 0x00C0_0000;
const WS_EX_TOPMOST: u32 = 0x0000_0008;
const WS_EX_TOOLWINDOW: u32 = 0x0000_0080;
const WS_EX_LAYERED: u32 = 0x0008_0000;
const WM_DESTROY: u32 = 0x0002;
const WM_CLOSE: u32 = 0x0010;
const WM_KEYDOWN: u32 = 0x0100;
const WM_NCHITTEST: u32 = 0x0084;
const WM_NCLBUTTONDOWN: u32 = 0x00A1;
const WM_NCLBUTTONUP: u32 = 0x00A2;
const WM_NCRBUTTONUP: u32 = 0x00A5;
const WM_LBUTTONDOWN: u32 = 0x0201;
const WM_LBUTTONUP: u32 = 0x0202;
const WM_MOUSEMOVE: u32 = 0x0200;
const WM_NCMOUSEMOVE: u32 = 0x00A0;
const WM_RBUTTONUP: u32 = 0x0205;
const WM_MOVE: u32 = 0x0003;
const WM_WINDOWPOSCHANGED: u32 = 0x0047;
const WM_EXITSIZEMOVE: u32 = 0x0232;
const WM_DISPLAYCHANGE: u32 = 0x007E;
const WM_SETTINGCHANGE: u32 = 0x001A;
const WM_DPICHANGED: u32 = 0x02E0;
const WM_COMMAND: u32 = 0x0111;
const WM_TIMER: u32 = 0x0113;
const FADE_TIMER_ID: usize = 2;
const WM_APP_REFRESH: u32 = 0x8001;
const WM_APP_SHOW_SETTINGS: u32 = 0x8002;
const WM_APP_DRAG_DIAGNOSTIC: u32 = 0x8003;
const GWLP_HWNDPARENT: i32 = -8;
const SWP_NOSIZE: u32 = 0x0001;
const SWP_NOZORDER: u32 = 0x0004;
const SWP_NOACTIVATE: u32 = 0x0010;
const HWND_TOPMOST: Hwnd = (-1_isize) as Hwnd;
const HWND_NOTOPMOST: Hwnd = (-2_isize) as Hwnd;
const ABM_NEW: u32 = 0x0000_0000;
const ABM_REMOVE: u32 = 0x0000_0001;
const ABM_WINDOWPOSCHANGED: u32 = 0x0000_0009;
const HTCLIENT: isize = 1;
const HTCAPTION: isize = 2;
const MF_STRING: u32 = 0;
const MF_CHECKED: u32 = 0x0008;
const TPM_RIGHTBUTTON: u32 = 0x0002;
const TPM_RETURNCMD: u32 = 0x0100;
const MENU_TOGGLE_LOCK: u32 = 1;
const MENU_EXIT: u32 = 2;
const MENU_TOGGLE_SNAP: u32 = 3;
const MENU_SETTINGS: u32 = 4;
const MENU_TASK_MANAGER: u32 = 5;
const MENU_TOGGLE_TOPMOST: u32 = 6;
const MENU_ABOUT: u32 = 7;
const MENU_TOGGLE_FULLSCREEN: u32 = 8;
const SETTINGS_TOGGLE_ACCENT: usize = 1001;
const SETTINGS_SAVE_OPENCODE_KEY: usize = 1002;
const SETTINGS_REMOVE_OPENCODE_KEY: usize = 1003;
const SETTINGS_TOGGLE_STARTUP: usize = 1004;
const SETTINGS_AI_ROLLING: usize = 1005;
const SETTINGS_AI_WEEKLY: usize = 1006;
const SETTINGS_AI_MONTHLY: usize = 1007;
const SETTINGS_AI_USED: usize = 1008;
const SETTINGS_AI_INTERVAL: usize = 1009;
const SETTINGS_PORT_FIRST: usize = 1011;
const SETTINGS_PORT_LAST: usize = 1022;
const SETTINGS_DEVICES: usize = 1023;
const DEVICES_SAVE: usize = 1101;
const DEVICES_APPEARANCE: usize = 1102;
const APPEARANCE_SAVE: usize = 1401;
const SETTINGS_DEEPSEEK: usize = 1024;
const DEEPSEEK_SAVE: usize = 1201;
const DEEPSEEK_REMOVE: usize = 1202;
const SETTINGS_MAINTENANCE: usize = 1025;
const MAINTENANCE_RESET_APPEARANCE: usize = 1301;
const MAINTENANCE_RESET_ALL: usize = 1302;
const MAINTENANCE_IMPORT_LEGACY: usize = 1303;
const MAINTENANCE_RESTORE_BACKUP: usize = 1304;
const AI_INTERVALS: [u32; 4] = [60, 300, 900, 3600];
const BM_GETCHECK: u32 = 0x00F0;
const BM_SETCHECK: u32 = 0x00F1;
const BST_CHECKED: usize = 1;
const WS_BORDER: u32 = 0x0080_0000;
const WS_VSCROLL: u32 = 0x0020_0000;
const ES_PASSWORD: u32 = 0x0020;
const WS_OVERLAPPEDWINDOW: u32 = 0x00CF_0000;
const WS_CHILD: u32 = 0x4000_0000;
const WS_VISIBLE: u32 = 0x1000_0000;
const WS_TABSTOP: u32 = 0x0001_0000;
const SW_SHOW: i32 = 5;
const ERROR_CLASS_ALREADY_EXISTS: u32 = 1410;
const ERROR_ALREADY_EXISTS: u32 = 183;
static POSITION_LOCKED: AtomicBool = AtomicBool::new(false);
static DRAG_ORIGIN: Mutex<Option<(Point, Point)>> = Mutex::new(None);
static DRAG_NC_DOWN: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
static DRAG_CLIENT_DOWN: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
static DRAG_CLIENT_MOVE: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
static DRAG_WINDOW_MOVE: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
static SNAP_TO_TASKBAR: AtomicBool = AtomicBool::new(true);
static APPBAR_REGISTERED: AtomicBool = AtomicBool::new(false);
static FREE_X: AtomicI32 = AtomicI32::new(100);
static FREE_Y: AtomicI32 = AtomicI32::new(100);
static ALTERNATE_ACCENT: AtomicBool = AtomicBool::new(false);
static FULLSCREEN_HIDDEN: AtomicBool = AtomicBool::new(false);
static OVERLAY_ALPHA: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(255);
static FULLSCREEN_CANDIDATE: Mutex<(bool, u8)> = Mutex::new((false, 0));
static DPI_SCALE_PERCENT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(100);
static CONFIG_PATH: OnceLock<std::path::PathBuf> = OnceLock::new();
static APP_CONFIG: OnceLock<Mutex<AppConfig>> = OnceLock::new();
static CURRENT_METRICS: OnceLock<Mutex<SystemMetrics>> = OnceLock::new();
static TELEMETRY_WORKER: OnceLock<Mutex<Option<TelemetryWorker>>> = OnceLock::new();
static AI_SNAPSHOT: OnceLock<Mutex<Option<AiUsageSnapshot>>> = OnceLock::new();
static AI_WORKER: OnceLock<Mutex<Option<super::ai_usage::AiUsageWorker>>> = OnceLock::new();
static OVERLAY_HANDLE: std::sync::atomic::AtomicPtr<c_void> =
    std::sync::atomic::AtomicPtr::new(null_mut());
static SETTINGS_HANDLE: std::sync::atomic::AtomicPtr<c_void> =
    std::sync::atomic::AtomicPtr::new(null_mut());
static DEVICES_HANDLE: std::sync::atomic::AtomicPtr<c_void> =
    std::sync::atomic::AtomicPtr::new(null_mut());
static NETWORK_COMBO: std::sync::atomic::AtomicPtr<c_void> =
    std::sync::atomic::AtomicPtr::new(null_mut());
static DISK_LIST: std::sync::atomic::AtomicPtr<c_void> =
    std::sync::atomic::AtomicPtr::new(null_mut());
static GPU_INDEX_EDIT: std::sync::atomic::AtomicPtr<c_void> =
    std::sync::atomic::AtomicPtr::new(null_mut());
static GPU_VENDOR_COMBO: std::sync::atomic::AtomicPtr<c_void> =
    std::sync::atomic::AtomicPtr::new(null_mut());
static GPU_VENDOR_CHOICES: OnceLock<Mutex<Vec<String>>> = OnceLock::new();
static UPDATE_COMBO: std::sync::atomic::AtomicPtr<c_void> =
    std::sync::atomic::AtomicPtr::new(null_mut());
static DEVICE_CHECKS: [std::sync::atomic::AtomicPtr<c_void>; 2] = [
    std::sync::atomic::AtomicPtr::new(null_mut()),
    std::sync::atomic::AtomicPtr::new(null_mut()),
];
static NETWORK_CHOICES: OnceLock<Mutex<Vec<String>>> = OnceLock::new();
static DISK_CHOICES: OnceLock<Mutex<Vec<String>>> = OnceLock::new();
static DEEPSEEK_HANDLE: std::sync::atomic::AtomicPtr<c_void> =
    std::sync::atomic::AtomicPtr::new(null_mut());
static DEEPSEEK_KEY_EDIT: std::sync::atomic::AtomicPtr<c_void> =
    std::sync::atomic::AtomicPtr::new(null_mut());
static MAINTENANCE_HANDLE: std::sync::atomic::AtomicPtr<c_void> =
    std::sync::atomic::AtomicPtr::new(null_mut());
static APPEARANCE_HANDLE: std::sync::atomic::AtomicPtr<c_void> =
    std::sync::atomic::AtomicPtr::new(null_mut());
static PALETTE_HANDLE: std::sync::atomic::AtomicPtr<c_void> =
    std::sync::atomic::AtomicPtr::new(null_mut());
static PALETTE_EDITS: [std::sync::atomic::AtomicPtr<c_void>; 8] = [
    const { std::sync::atomic::AtomicPtr::new(null_mut()) }; 8
];
const APPEARANCE_PALETTE: usize = 1820;
const PALETTE_SAVE: usize = 1821;
static APPEARANCE_EDITS: [std::sync::atomic::AtomicPtr<c_void>; 8] = [
    std::sync::atomic::AtomicPtr::new(null_mut()),
    std::sync::atomic::AtomicPtr::new(null_mut()),
    std::sync::atomic::AtomicPtr::new(null_mut()),
    std::sync::atomic::AtomicPtr::new(null_mut()),
    std::sync::atomic::AtomicPtr::new(null_mut()),
    std::sync::atomic::AtomicPtr::new(null_mut()),
    std::sync::atomic::AtomicPtr::new(null_mut()),
    std::sync::atomic::AtomicPtr::new(null_mut()),
];
static APPEARANCE_CHECKS: [std::sync::atomic::AtomicPtr<c_void>; 3] = [
    std::sync::atomic::AtomicPtr::new(null_mut()),
    std::sync::atomic::AtomicPtr::new(null_mut()),
    std::sync::atomic::AtomicPtr::new(null_mut()),
];
static DEEPSEEK_SNAPSHOT: OnceLock<Mutex<Option<super::ai_usage::DeepSeekBalanceSnapshot>>> = OnceLock::new();
static DEEPSEEK_WORKER: OnceLock<Mutex<Option<super::ai_usage::DeepSeekWorker>>> = OnceLock::new();
static AI_KEY_EDIT: std::sync::atomic::AtomicPtr<c_void> =
    std::sync::atomic::AtomicPtr::new(null_mut());
static AI_KEY_STATUS: std::sync::atomic::AtomicPtr<c_void> =
    std::sync::atomic::AtomicPtr::new(null_mut());
static STARTUP_CHECKBOX: std::sync::atomic::AtomicPtr<c_void> =
    std::sync::atomic::AtomicPtr::new(null_mut());
static AI_OPTION_CONTROLS: [std::sync::atomic::AtomicPtr<c_void>; 5] = [
    std::sync::atomic::AtomicPtr::new(null_mut()),
    std::sync::atomic::AtomicPtr::new(null_mut()),
    std::sync::atomic::AtomicPtr::new(null_mut()),
    std::sync::atomic::AtomicPtr::new(null_mut()),
    std::sync::atomic::AtomicPtr::new(null_mut()),
];
static AI_INTERVAL_COMBO: std::sync::atomic::AtomicPtr<c_void> =
    std::sync::atomic::AtomicPtr::new(null_mut());
static STARTUP_STATUS: std::sync::atomic::AtomicPtr<c_void> =
    std::sync::atomic::AtomicPtr::new(null_mut());
static PORT_OPTION_CONTROLS: [std::sync::atomic::AtomicPtr<c_void>; 12] = [
    std::sync::atomic::AtomicPtr::new(null_mut()),
    std::sync::atomic::AtomicPtr::new(null_mut()),
    std::sync::atomic::AtomicPtr::new(null_mut()),
    std::sync::atomic::AtomicPtr::new(null_mut()),
    std::sync::atomic::AtomicPtr::new(null_mut()),
    std::sync::atomic::AtomicPtr::new(null_mut()),
    std::sync::atomic::AtomicPtr::new(null_mut()),
    std::sync::atomic::AtomicPtr::new(null_mut()),
    std::sync::atomic::AtomicPtr::new(null_mut()),
    std::sync::atomic::AtomicPtr::new(null_mut()),
    std::sync::atomic::AtomicPtr::new(null_mut()),
    std::sync::atomic::AtomicPtr::new(null_mut()),
];
const VK_ESCAPE: usize = 0x1B;
const SW_SHOWNOACTIVATE: i32 = 4;
const DIB_RGB_COLORS: u32 = 0;
const BI_RGB: u32 = 0;
const ULW_ALPHA: u32 = 2;
const AC_SRC_OVER: u8 = 0;
const AC_SRC_ALPHA: u8 = 1;

#[repr(C)]
pub struct WindowClass {
    style: u32,
    window_proc: Option<WndProc>,
    class_extra: i32,
    window_extra: i32,
    instance: Hinstance,
    icon: *mut c_void,
    cursor: *mut c_void,
    background: *mut c_void,
    menu_name: *const u16,
    class_name: *const u16,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct Point {
    x: i32,
    y: i32,
}
#[repr(C)]
struct Size {
    cx: i32,
    cy: i32,
}
#[repr(C)]
struct Rect {
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
}
#[repr(C)]
struct MonitorInfo {
    size: u32,
    monitor: Rect,
    work: Rect,
    flags: u32,
}
#[repr(C)]
struct AppBarData {
    size: u32,
    hwnd: Hwnd,
    callback_message: u32,
    edge: u32,
    rect: Rect,
    lparam: isize,
}
#[repr(C)]
struct BlendFunction {
    operation: u8,
    flags: u8,
    source_constant_alpha: u8,
    alpha_format: u8,
}
#[repr(C)]
struct Message {
    hwnd: Hwnd,
    message: u32,
    wparam: usize,
    lparam: isize,
    time: u32,
    point: Point,
    private: u32,
}
#[repr(C)]
struct BitmapInfoHeader {
    size: u32,
    width: i32,
    height: i32,
    planes: u16,
    bit_count: u16,
    compression: u32,
    size_image: u32,
    x_pixels_per_meter: i32,
    y_pixels_per_meter: i32,
    colors_used: u32,
    colors_important: u32,
}
#[repr(C)]
struct RgbQuad {
    blue: u8,
    green: u8,
    red: u8,
    reserved: u8,
}
#[repr(C)]
struct BitmapInfo {
    header: BitmapInfoHeader,
    colors: [RgbQuad; 1],
}

#[link(name = "kernel32")]
extern "system" {
    fn GetLastError() -> u32;
    fn CloseHandle(handle: *mut c_void) -> i32;
}

#[link(name = "user32")]
extern "system" {
    fn RegisterClassW(class: *const WindowClass) -> u16;
    fn UnregisterClassW(class_name: *const u16, instance: Hinstance) -> i32;
    fn CreateWindowExW(
        ex_style: u32,
        class_name: *const u16,
        window_name: *const u16,
        style: u32,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
        parent: Hwnd,
        menu: *mut c_void,
        instance: Hinstance,
        parameter: *mut c_void,
    ) -> Hwnd;
    fn DefWindowProcW(hwnd: Hwnd, message: u32, wparam: usize, lparam: isize) -> Lresult;
    fn DestroyWindow(hwnd: Hwnd) -> i32;
    fn PostMessageW(hwnd: Hwnd, message: u32, wparam: usize, lparam: isize) -> i32;
    fn UpdateWindow(hwnd: Hwnd) -> i32;
    fn ShowWindow(hwnd: Hwnd, command: i32) -> i32;
    fn GetMessageW(message: *mut Message, hwnd: Hwnd, min: u32, max: u32) -> i32;
    fn TranslateMessage(message: *const Message) -> i32;
    fn DispatchMessageW(message: *const Message) -> Lresult;
    fn PostQuitMessage(exit_code: i32);
    fn CreatePopupMenu() -> *mut c_void;
    fn AppendMenuW(menu: *mut c_void, flags: u32, item_id: usize, text: *const u16) -> i32;
    fn GetCursorPos(point: *mut Point) -> i32;
    fn SetCapture(hwnd: Hwnd) -> Hwnd;
    fn ReleaseCapture() -> i32;
    fn TrackPopupMenu(
        menu: *mut c_void,
        flags: u32,
        x: i32,
        y: i32,
        reserved: i32,
        owner: Hwnd,
        rect: *const c_void,
    ) -> u32;
    fn DestroyMenu(menu: *mut c_void) -> i32;
    fn FindWindowW(class_name: *const u16, window_name: *const u16) -> Hwnd;
    fn GetForegroundWindow() -> Hwnd;
    fn MessageBoxW(hwnd: Hwnd, text: *const u16, caption: *const u16, flags: u32) -> i32;
    fn GetWindowRect(hwnd: Hwnd, rect: *mut Rect) -> i32;
    fn GetWindowLongPtrW(hwnd: Hwnd, index: i32) -> isize;
    fn MonitorFromWindow(hwnd: Hwnd, flags: u32) -> Hwnd;
    fn GetMonitorInfoW(monitor: Hwnd, info: *mut MonitorInfo) -> i32;
    fn GetClassNameW(hwnd: Hwnd, class_name: *mut u16, max_count: i32) -> i32;
    fn SetWindowPos(
        hwnd: Hwnd,
        insert_after: Hwnd,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
        flags: u32,
    ) -> i32;
    fn SetWindowLongPtrW(hwnd: Hwnd, index: i32, value: isize) -> isize;
    fn GetWindowTextLengthW(hwnd: Hwnd) -> i32;
    fn GetWindowTextW(hwnd: Hwnd, text: *mut u16, max_count: i32) -> i32;
    fn SetWindowTextW(hwnd: Hwnd, text: *const u16) -> i32;
    fn SendMessageW(hwnd: Hwnd, message: u32, wparam: usize, lparam: isize) -> Lresult;
    fn GetDC(hwnd: Hwnd) -> Hdc;
    fn ReleaseDC(hwnd: Hwnd, dc: Hdc) -> i32;
    fn UpdateLayeredWindow(
        hwnd: Hwnd,
        destination_dc: Hdc,
        destination: *const Point,
        size: *const Size,
        source_dc: Hdc,
        source: *const Point,
        color_key: u32,
        blend: *const BlendFunction,
        flags: u32,
    ) -> i32;
    fn GetModuleHandleW(module_name: *const u16) -> Hinstance;
    fn SetProcessDpiAwarenessContext(context: *mut c_void) -> i32;
    fn CreateMutexW(attributes: *mut c_void, initial_owner: i32, name: *const u16) -> *mut c_void;
    fn SetTimer(hwnd: Hwnd, timer_id: usize, interval_ms: u32, callback: *mut c_void) -> usize;
    fn KillTimer(hwnd: Hwnd, timer_id: usize) -> i32;
}

#[link(name = "shell32")]
extern "system" {
    fn SHAppBarMessage(message: u32, data: *mut AppBarData) -> usize;
    fn ShellExecuteW(
        hwnd: Hwnd,
        operation: *const u16,
        file: *const u16,
        parameters: *const u16,
        directory: *const u16,
        show: i32,
    ) -> Hinstance;
}

#[link(name = "gdi32")]
extern "system" {
    fn CreateCompatibleDC(dc: Hdc) -> Hdc;
    fn DeleteDC(dc: Hdc) -> i32;
    fn CreateDIBSection(
        dc: Hdc,
        info: *const c_void,
        usage: u32,
        bits: *mut *mut c_void,
        section: *mut c_void,
        offset: u32,
    ) -> Hbitmap;
    fn SelectObject(dc: Hdc, object: Hgdiobj) -> Hgdiobj;
    fn DeleteObject(object: Hgdiobj) -> i32;
}

#[derive(Debug)]
pub struct WinError(&'static str, u32);
struct TelemetryWorker {
    stop: Sender<()>,
    thread: Option<JoinHandle<()>>,
}

impl Drop for TelemetryWorker {
    fn drop(&mut self) {
        let _ = self.stop.send(());
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}
impl fmt::Display for WinError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} (GetLastError={})", self.0, self.1)
    }
}
impl std::error::Error for WinError {}

include!("win32_p0_0.rs");
include!("win32_p0_1.rs");
include!("win32_p0_2.rs");
include!("win32_p1_0.rs");
include!("win32_p1_1.rs");
include!("win32_p1_2.rs");
include!("win32_p2_0.rs");
include!("win32_p2_1.rs");
