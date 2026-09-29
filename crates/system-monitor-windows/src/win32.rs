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
const WM_LBUTTONDOWN: u32 = 0x0201;
const WM_LBUTTONUP: u32 = 0x0202;
const WM_MOUSEMOVE: u32 = 0x0200;
const WM_RBUTTONUP: u32 = 0x0205;
const WM_MOVE: u32 = 0x0003;
const WM_WINDOWPOSCHANGED: u32 = 0x0047;
const WM_EXITSIZEMOVE: u32 = 0x0232;
const WM_DISPLAYCHANGE: u32 = 0x007E;
const WM_SETTINGCHANGE: u32 = 0x001A;
const WM_DPICHANGED: u32 = 0x02E0;
const WM_COMMAND: u32 = 0x0111;
const WM_APP_REFRESH: u32 = 0x8001;
const WM_APP_SHOW_SETTINGS: u32 = 0x8002;
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
const SETTINGS_DEEPSEEK: usize = 1024;
const DEEPSEEK_SAVE: usize = 1201;
const DEEPSEEK_REMOVE: usize = 1202;
const SETTINGS_MAINTENANCE: usize = 1025;
const MAINTENANCE_RESET_APPEARANCE: usize = 1301;
const MAINTENANCE_RESET_ALL: usize = 1302;
const MAINTENANCE_IMPORT_LEGACY: usize = 1303;
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
static SNAP_TO_TASKBAR: AtomicBool = AtomicBool::new(true);
static APPBAR_REGISTERED: AtomicBool = AtomicBool::new(false);
static FREE_X: AtomicI32 = AtomicI32::new(100);
static FREE_Y: AtomicI32 = AtomicI32::new(100);
static ALTERNATE_ACCENT: AtomicBool = AtomicBool::new(false);
static FULLSCREEN_HIDDEN: AtomicBool = AtomicBool::new(false);
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
static NETWORK_CHOICES: OnceLock<Mutex<Vec<String>>> = OnceLock::new();
static DISK_CHOICES: OnceLock<Mutex<Vec<String>>> = OnceLock::new();
static DEEPSEEK_HANDLE: std::sync::atomic::AtomicPtr<c_void> =
    std::sync::atomic::AtomicPtr::new(null_mut());
static DEEPSEEK_KEY_EDIT: std::sync::atomic::AtomicPtr<c_void> =
    std::sync::atomic::AtomicPtr::new(null_mut());
static MAINTENANCE_HANDLE: std::sync::atomic::AtomicPtr<c_void> =
    std::sync::atomic::AtomicPtr::new(null_mut());
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
        info: *const BitmapInfo,
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

unsafe extern "system" fn window_proc(
    hwnd: Hwnd,
    message: u32,
    wparam: usize,
    lparam: isize,
) -> Lresult {
    match message {
        WM_NCHITTEST if POSITION_LOCKED.load(Ordering::Relaxed) => HTCLIENT,
        WM_NCHITTEST => HTCAPTION,
        WM_NCLBUTTONDOWN | WM_LBUTTONDOWN if !POSITION_LOCKED.load(Ordering::Relaxed) => {
            let mut cursor = Point { x: 0, y: 0 };
            let mut rect = Rect { left: 0, top: 0, right: 0, bottom: 0 };
            if GetCursorPos(&mut cursor) != 0 && GetWindowRect(hwnd, &mut rect) != 0 {
                if let Ok(mut drag) = DRAG_ORIGIN.lock() {
                    *drag = Some((cursor, Point { x: rect.left, y: rect.top }));
                }
                SetCapture(hwnd);
            }
            0
        }
        WM_MOUSEMOVE => {
            if let Ok(drag) = DRAG_ORIGIN.lock() {
                if let Some((cursor_start, window_start)) = *drag {
                    let mut cursor = Point { x: 0, y: 0 };
                    if GetCursorPos(&mut cursor) != 0 {
                        SetWindowPos(hwnd, null_mut(), window_start.x + cursor.x - cursor_start.x,
                            window_start.y + cursor.y - cursor_start.y, 0, 0,
                            SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE);
                    }
                }
            }
            0
        }
        WM_NCLBUTTONUP | WM_LBUTTONUP => {
            if let Ok(mut drag) = DRAG_ORIGIN.lock() {
                if drag.take().is_some() {
                    ReleaseCapture();
                    save_position();
                }
            }
            0
        }
        WM_CLOSE => {
            DestroyWindow(hwnd);
            0
        }
        WM_APP_REFRESH => {
            update_fullscreen_visibility(hwnd);
            refresh_overlay(hwnd);
            0
        }
        WM_APP_SHOW_SETTINGS => {
            open_settings_window(hwnd);
            0
        }
        WM_DESTROY => {
            if let Some(worker) = TELEMETRY_WORKER.get()
                .and_then(|v| v.lock().ok())
                .and_then(|mut v| v.take()) {
                drop(worker);
            }
            if let Some(worker) = AI_WORKER
                .get()
                .and_then(|v| v.lock().ok())
                .and_then(|mut v| v.take())
            {
                drop(worker);
            }
            if let Some(worker) = DEEPSEEK_WORKER.get()
                .and_then(|v| v.lock().ok()).and_then(|mut v| v.take()) { drop(worker); }
            remove_appbar(hwnd);
            OVERLAY_HANDLE.store(null_mut(), Ordering::Relaxed);
            let settings = SETTINGS_HANDLE.swap(null_mut(), Ordering::Relaxed);
            if !settings.is_null() {
                DestroyWindow(settings);
            }
            PostQuitMessage(0);
            0
        }
        WM_MOVE => {
            let mut rect = Rect {
                left: 0,
                top: 0,
                right: 0,
                bottom: 0,
            };
            if GetWindowRect(hwnd, &mut rect) != 0 {
                FREE_X.store(rect.left, Ordering::Relaxed);
                if !SNAP_TO_TASKBAR.load(Ordering::Relaxed) {
                    FREE_Y.store(rect.top, Ordering::Relaxed);
                }
            }
            DefWindowProcW(hwnd, message, wparam, lparam)
        }
        WM_EXITSIZEMOVE => {
            save_position();
            DefWindowProcW(hwnd, message, wparam, lparam)
        }
        WM_WINDOWPOSCHANGED => {
            if APPBAR_REGISTERED.load(Ordering::Relaxed) {
                let mut data = AppBarData {
                    size: size_of::<AppBarData>() as u32,
                    hwnd,
                    callback_message: 0,
                    edge: 0,
                    rect: Rect {
                        left: 0,
                        top: 0,
                        right: 0,
                        bottom: 0,
                    },
                    lparam: 0,
                };
                SHAppBarMessage(ABM_WINDOWPOSCHANGED, &mut data);
            }
            DefWindowProcW(hwnd, message, wparam, lparam)
        }
        WM_DPICHANGED => {
            let dpi = (wparam as u32) & 0xffff;
            if dpi > 0 {
                DPI_SCALE_PERCENT.store((dpi * 100 + 48) / 96, Ordering::Relaxed);
            }
            let (scaled, width, height) = render_current_bitmap();
            if lparam != 0 {
                let suggested = &*(lparam as *const Rect);
                SetWindowPos(
                    hwnd,
                    null_mut(),
                    suggested.left,
                    suggested.top,
                    width,
                    height,
                    SWP_NOZORDER | SWP_NOACTIVATE,
                );
            }
            if SNAP_TO_TASKBAR.load(Ordering::Relaxed) {
                align_to_taskbar(hwnd, height);
            }
            let _ = present_bitmap(hwnd, &scaled, width, height);
            0
        }
        WM_DISPLAYCHANGE | WM_SETTINGCHANGE => {
            if SNAP_TO_TASKBAR.load(Ordering::Relaxed) {
                let (_, _, height) = render_current_bitmap();
                align_to_taskbar(hwnd, height);
            }
            DefWindowProcW(hwnd, message, wparam, lparam)
        }
        WM_KEYDOWN if wparam == VK_ESCAPE => {
            DestroyWindow(hwnd);
            0
        }
        WM_RBUTTONUP => {
            show_context_menu(hwnd);
            0
        }
        _ => DefWindowProcW(hwnd, message, wparam, lparam),
    }
}

unsafe extern "system" fn settings_window_proc(
    hwnd: Hwnd,
    message: u32,
    wparam: usize,
    lparam: isize,
) -> Lresult {
    match message {
        WM_COMMAND if (wparam & 0xffff) == SETTINGS_TOGGLE_ACCENT => {
            let enabled = !ALTERNATE_ACCENT.load(Ordering::Relaxed);
            ALTERNATE_ACCENT.store(enabled, Ordering::Relaxed);
            let overlay = OVERLAY_HANDLE.load(Ordering::Relaxed);
            if !overlay.is_null() {
                PostMessageW(overlay, WM_APP_REFRESH, 0, 0);
            }
            0
        }
        WM_COMMAND if (wparam & 0xffff) == SETTINGS_SAVE_OPENCODE_KEY => {
            save_opencode_key_from_settings();
            0
        }
        WM_COMMAND if (wparam & 0xffff) == SETTINGS_REMOVE_OPENCODE_KEY => {
            let result =
                crate::secret_store::SecretStore::opencode().and_then(|store| store.remove());
            if result.is_ok() {
                if let Ok(mut config) = config_lock().lock() {
                    config.opencode_enabled = false;
                    let _ = config_store::save(&config_path(), &config);
                }
                if let Some(worker) = AI_WORKER
                    .get()
                    .and_then(|v| v.lock().ok())
                    .and_then(|mut v| v.take())
                {
                    drop(worker);
                }
                if let Some(snapshot) = AI_SNAPSHOT.get() {
                    if let Ok(mut value) = snapshot.lock() {
                        *value = None;
                    }
                }
                set_ai_key_status("OpenCode key removed");
            } else {
                set_ai_key_status("Could not remove OpenCode key");
            }
            let overlay = OVERLAY_HANDLE.load(Ordering::Relaxed);
            if !overlay.is_null() {
                PostMessageW(overlay, WM_APP_REFRESH, 0, 0);
            }
            0
        }
        WM_COMMAND if (wparam & 0xffff) == SETTINGS_TOGGLE_STARTUP => {
            let checkbox = STARTUP_CHECKBOX.load(Ordering::Relaxed);
            if !checkbox.is_null() {
                let enabled = SendMessageW(checkbox, BM_GETCHECK, 0, 0) == BST_CHECKED as isize;
                if let Ok(mut config) = config_lock().lock() {
                    config.launch_on_startup = enabled;
                    let saved = config_store::save(&config_path(), &config).is_ok();
                    let registered = set_startup_registration(enabled).is_ok();
                    set_startup_status(if saved && registered {
                        "Startup setting saved"
                    } else {
                        "Could not update startup setting"
                    });
                }
            }
            0
        }
        WM_COMMAND
            if ((SETTINGS_AI_ROLLING..=SETTINGS_AI_INTERVAL).contains(&(wparam & 0xffff))
                || (wparam & 0xffff) == 1010) =>
        {
            save_ai_options_from_settings();
            0
        }
        WM_COMMAND if (SETTINGS_PORT_FIRST..=SETTINGS_PORT_LAST).contains(&(wparam & 0xffff)) => {
            save_port_options_from_settings(hwnd);
            0
        }
        WM_COMMAND if (wparam & 0xffff) == SETTINGS_DEVICES => {
            open_devices_window(hwnd);
            0
        }
        WM_COMMAND if (wparam & 0xffff) == SETTINGS_DEEPSEEK => {
            open_deepseek_window(hwnd);
            0
        }
        WM_COMMAND if (wparam & 0xffff) == SETTINGS_MAINTENANCE => {
            open_maintenance_window(hwnd);
            0
        }
        WM_CLOSE => {
            DestroyWindow(hwnd);
            0
        }
        WM_DESTROY => {
            let devices = DEVICES_HANDLE.swap(null_mut(), Ordering::Relaxed);
            if !devices.is_null() { DestroyWindow(devices); }
            let deepseek = DEEPSEEK_HANDLE.swap(null_mut(), Ordering::Relaxed);
            if !deepseek.is_null() { DestroyWindow(deepseek); }
            let maintenance = MAINTENANCE_HANDLE.swap(null_mut(), Ordering::Relaxed);
            if !maintenance.is_null() { DestroyWindow(maintenance); }
            SETTINGS_HANDLE
                .compare_exchange(hwnd, null_mut(), Ordering::Relaxed, Ordering::Relaxed)
                .ok();
            AI_KEY_EDIT.store(null_mut(), Ordering::Relaxed);
            AI_KEY_STATUS.store(null_mut(), Ordering::Relaxed);
            STARTUP_CHECKBOX.store(null_mut(), Ordering::Relaxed);
            STARTUP_STATUS.store(null_mut(), Ordering::Relaxed);
            for control in &AI_OPTION_CONTROLS {
                control.store(null_mut(), Ordering::Relaxed);
            }
            AI_INTERVAL_COMBO.store(null_mut(), Ordering::Relaxed);
            for control in &PORT_OPTION_CONTROLS {
                control.store(null_mut(), Ordering::Relaxed);
            }
            0
        }
        _ => DefWindowProcW(hwnd, message, wparam, lparam),
    }
}

unsafe fn open_settings_window(owner: Hwnd) {
    let current = SETTINGS_HANDLE.load(Ordering::Relaxed);
    if !current.is_null() {
        ShowWindow(current, SW_SHOW);
        return;
    }
    let instance = GetModuleHandleW(null());
    let class_name: Vec<u16> = "Kil0bitRustSettingsPoT\0".encode_utf16().collect();
    let title: Vec<u16> = "Kil0bit System Monitor Settings\0".encode_utf16().collect();
    let class = WindowClass {
        style: 0,
        window_proc: Some(settings_window_proc),
        class_extra: 0,
        window_extra: 0,
        instance,
        icon: null_mut(),
        cursor: null_mut(),
        background: null_mut(),
        menu_name: null(),
        class_name: class_name.as_ptr(),
    };
    if RegisterClassW(&class) == 0 && GetLastError() != ERROR_CLASS_ALREADY_EXISTS {
        return;
    }
    let window = CreateWindowExW(
        WS_EX_TOOLWINDOW,
        class_name.as_ptr(),
        title.as_ptr(),
        WS_OVERLAPPEDWINDOW,
        300,
        100,
        560,
        760,
        owner,
        null_mut(),
        instance,
        null_mut(),
    );
    if window.is_null() {
        return;
    }
    SETTINGS_HANDLE.store(window, Ordering::Relaxed);
    let button_class: Vec<u16> = "BUTTON\0".encode_utf16().collect();
    let button_text: Vec<u16> = "Toggle overlay accent\0".encode_utf16().collect();
    CreateWindowExW(
        0,
        button_class.as_ptr(),
        button_text.as_ptr(),
        WS_CHILD | WS_VISIBLE | WS_TABSTOP,
        24,
        24,
        240,
        36,
        window,
        SETTINGS_TOGGLE_ACCENT as *mut c_void,
        instance,
        null_mut(),
    );
    let devices_text: Vec<u16> = "Network, disks and GPU...\0".encode_utf16().collect();
    CreateWindowExW(0, button_class.as_ptr(), devices_text.as_ptr(),
        WS_CHILD | WS_VISIBLE | WS_TABSTOP, 280, 24, 240, 36, window,
        SETTINGS_DEVICES as *mut c_void, instance, null_mut());
    let label_class: Vec<u16> = "STATIC\0".encode_utf16().collect();
    let ai_label: Vec<u16> = "OpenCode Go API key (stored with Windows DPAPI):\0"
        .encode_utf16()
        .collect();
    CreateWindowExW(
        0,
        label_class.as_ptr(),
        ai_label.as_ptr(),
        WS_CHILD | WS_VISIBLE,
        24,
        78,
        450,
        24,
        window,
        null_mut(),
        instance,
        null_mut(),
    );
    let edit_class: Vec<u16> = "EDIT\0".encode_utf16().collect();
    let empty: Vec<u16> = "\0".encode_utf16().collect();
    let edit = CreateWindowExW(
        WS_BORDER,
        edit_class.as_ptr(),
        empty.as_ptr(),
        WS_CHILD | WS_VISIBLE | WS_TABSTOP | ES_PASSWORD,
        24,
        106,
        450,
        28,
        window,
        null_mut(),
        instance,
        null_mut(),
    );
    AI_KEY_EDIT.store(edit, Ordering::Relaxed);
    let save_text: Vec<u16> = "Save key\0".encode_utf16().collect();
    CreateWindowExW(
        0,
        button_class.as_ptr(),
        save_text.as_ptr(),
        WS_CHILD | WS_VISIBLE | WS_TABSTOP,
        24,
        146,
        120,
        32,
        window,
        SETTINGS_SAVE_OPENCODE_KEY as *mut c_void,
        instance,
        null_mut(),
    );
    let remove_text: Vec<u16> = "Remove key\0".encode_utf16().collect();
    CreateWindowExW(
        0,
        button_class.as_ptr(),
        remove_text.as_ptr(),
        WS_CHILD | WS_VISIBLE | WS_TABSTOP,
        154,
        146,
        120,
        32,
        window,
        SETTINGS_REMOVE_OPENCODE_KEY as *mut c_void,
        instance,
        null_mut(),
    );
    let status_text: Vec<u16> =
        if crate::secret_store::SecretStore::opencode().is_ok_and(|store| store.exists()) {
            "OpenCode key is saved\0".encode_utf16().collect()
        } else {
            "OpenCode key is not saved\0".encode_utf16().collect()
        };
    let status = CreateWindowExW(
        0,
        label_class.as_ptr(),
        status_text.as_ptr(),
        WS_CHILD | WS_VISIBLE,
        24,
        190,
        450,
        24,
        window,
        null_mut(),
        instance,
        null_mut(),
    );
    AI_KEY_STATUS.store(status, Ordering::Relaxed);
    let config = config_lock().lock().map(|c| c.clone()).unwrap_or_default();
    let options = [
        (
            SETTINGS_AI_ROLLING,
            "Show 5 hour window",
            config.opencode_show_rolling,
        ),
        (
            SETTINGS_AI_WEEKLY,
            "Show weekly window",
            config.opencode_show_weekly,
        ),
        (
            SETTINGS_AI_MONTHLY,
            "Show monthly window",
            config.opencode_show_monthly,
        ),
        (
            SETTINGS_AI_USED,
            "Show used percent (unchecked = remaining)",
            config.ai_show_used_percent,
        ),
        (
            1010,
            "Use Codex CLI quota (read-only auth.json)",
            config.codex_enabled,
        ),
    ];
    for (index, (id, caption, checked)) in options.iter().enumerate() {
        let text: Vec<u16> = format!("{caption}\0").encode_utf16().collect();
        let control = CreateWindowExW(
            0,
            button_class.as_ptr(),
            text.as_ptr(),
            WS_CHILD | WS_VISIBLE | WS_TABSTOP | 0x0003,
            24 + (index as i32 % 2) * 260,
            226 + (index as i32 / 2) * 32,
            250,
            26,
            window,
            *id as *mut c_void,
            instance,
            null_mut(),
        );
        AI_OPTION_CONTROLS[index].store(control, Ordering::Relaxed);
        if *checked {
            SendMessageW(control, BM_SETCHECK, BST_CHECKED, 0);
        }
    }
    let interval_label: Vec<u16> = "Poll interval:\0".encode_utf16().collect();
    CreateWindowExW(
        0,
        label_class.as_ptr(),
        interval_label.as_ptr(),
        WS_CHILD | WS_VISIBLE,
        24,
        328,
        110,
        24,
        window,
        null_mut(),
        instance,
        null_mut(),
    );
    let combo_class: Vec<u16> = "COMBOBOX\0".encode_utf16().collect();
    let combo = CreateWindowExW(
        WS_VSCROLL,
        combo_class.as_ptr(),
        empty.as_ptr(),
        WS_CHILD | WS_VISIBLE | WS_TABSTOP | 0x0003,
        140,
        324,
        180,
        120,
        window,
        SETTINGS_AI_INTERVAL as *mut c_void,
        instance,
        null_mut(),
    );
    for caption in ["1 minute", "5 minutes", "15 minutes", "60 minutes"] {
        let item: Vec<u16> = caption.encode_utf16().chain(Some(0)).collect();
        SendMessageW(combo, 0x0143, 0, item.as_ptr() as isize);
    }
    let interval_index = AI_INTERVALS
        .iter()
        .position(|value| *value == config.ai_poll_interval_seconds)
        .unwrap_or(1);
    SendMessageW(combo, 0x014E, interval_index, 0);
    AI_INTERVAL_COMBO.store(combo, Ordering::Relaxed);
    devices_child(window, "BUTTON", "DeepSeek balance...", SETTINGS_DEEPSEEK,
        350, 324, 170, 32, WS_TABSTOP);
    let startup_text: Vec<u16> = "Launch Rust monitor when I sign in\0"
        .encode_utf16()
        .collect();
    let startup = CreateWindowExW(
        0,
        button_class.as_ptr(),
        startup_text.as_ptr(),
        WS_CHILD | WS_VISIBLE | WS_TABSTOP | 0x0003,
        24,
        376,
        340,
        26,
        window,
        SETTINGS_TOGGLE_STARTUP as *mut c_void,
        instance,
        null_mut(),
    );
    STARTUP_CHECKBOX.store(startup, Ordering::Relaxed);
    devices_child(window, "BUTTON", "Reset / import...", SETTINGS_MAINTENANCE,
        375, 376, 145, 28, WS_TABSTOP);
    if config_lock()
        .lock()
        .map(|config| config.launch_on_startup)
        .unwrap_or(false)
    {
        SendMessageW(startup, BM_SETCHECK, BST_CHECKED, 0);
    }
    let startup_status_text: Vec<u16> = "Startup status\0".encode_utf16().collect();
    let startup_status = CreateWindowExW(
        0,
        label_class.as_ptr(),
        startup_status_text.as_ptr(),
        WS_CHILD | WS_VISIBLE,
        24,
        412,
        450,
        22,
        window,
        null_mut(),
        instance,
        null_mut(),
    );
    STARTUP_STATUS.store(startup_status, Ordering::Relaxed);
    let section_text: Vec<u16> = "Telemetry and overlay options"
        .encode_utf16()
        .chain(Some(0))
        .collect();
    CreateWindowExW(
        0,
        label_class.as_ptr(),
        section_text.as_ptr(),
        WS_CHILD | WS_VISIBLE,
        24,
        450,
        500,
        24,
        window,
        null_mut(),
        instance,
        null_mut(),
    );
    let port_options = [
        ("CPU usage", config.show_cpu),
        ("RAM usage", config.show_ram),
        ("GPU load", config.show_gpu),
        ("GPU temperature", config.show_temp),
        ("Disk used space", config.show_disk),
        ("Network upload", config.show_net_up),
        ("Network download", config.show_net_down),
        ("Show overlay", config.show_overlay),
        ("Lock position", config.lock_position),
        ("Snap to taskbar", config.stick_to_taskbar),
        ("Keep on top", config.always_on_top),
        ("Compact display", config.display_style == "Compact"),
    ];
    for (index, (caption, checked)) in port_options.iter().enumerate() {
        let text: Vec<u16> = format!("{caption}\0").encode_utf16().collect();
        let y = if index < 7 {
            478 + (index as i32 / 2) * 28
        } else if index == 11 {
            562
        } else {
            594 + ((index - 7) as i32) * 28
        };
        let x = if index < 7 {
            24 + (index as i32 % 2) * 270
        } else if index == 11 {
            294
        } else {
            24
        };
        let control = CreateWindowExW(
            0,
            button_class.as_ptr(),
            text.as_ptr(),
            WS_CHILD | WS_VISIBLE | WS_TABSTOP | 0x0003,
            x,
            y,
            260,
            24,
            window,
            (SETTINGS_PORT_FIRST + index) as *mut c_void,
            instance,
            null_mut(),
        );
        PORT_OPTION_CONTROLS[index].store(control, Ordering::Relaxed);
        if *checked {
            SendMessageW(control, BM_SETCHECK, BST_CHECKED, 0);
        }
    }
    ShowWindow(window, SW_SHOW);
    UpdateWindow(window);
}

unsafe extern "system" fn maintenance_window_proc(
    hwnd: Hwnd, message: u32, wparam: usize, lparam: isize,
) -> Lresult {
    match message {
        WM_COMMAND if (wparam & 0xffff) == MAINTENANCE_RESET_APPEARANCE => {
            if confirm_maintenance(hwnd, "Reset appearance settings?") {
                let defaults = AppConfig::default();
                if let Ok(mut config) = config_lock().lock() {
                    config.display_style = defaults.display_style;
                    config.font_family = defaults.font_family;
                    config.accent_color_hex = defaults.accent_color_hex;
                    config.label_color_hex = defaults.label_color_hex;
                    config.background_color_hex = defaults.background_color_hex;
                    config.pod_color_hex = defaults.pod_color_hex;
                    config.scale_factor = defaults.scale_factor;
                    config.column_spacing = defaults.column_spacing;
                    config.is_text_bold = defaults.is_text_bold;
                    config.show_pods = defaults.show_pods;
                    config.show_background = defaults.show_background;
                    config.theme = defaults.theme;
                    config.net_label_color_hex = None;
                    config.cpu_ram_label_color_hex = None;
                    config.gpu_label_color_hex = None;
                    config.disk_label_color_hex = None;
                    config.net_accent_color_hex = None;
                    config.cpu_ram_accent_color_hex = None;
                    config.gpu_accent_color_hex = None;
                    config.disk_accent_color_hex = None;
                    let _ = config_store::save(&config_path(), &config);
                }
                maintenance_refresh();
            }
            0
        }
        WM_COMMAND if (wparam & 0xffff) == MAINTENANCE_RESET_ALL => {
            if confirm_maintenance(hwnd, "Reset all Rust monitor settings?") {
                if let Ok(mut config) = config_lock().lock() {
                    *config = AppConfig::default();
                    let _ = config_store::save(&config_path(), &config);
                }
                load_config();
                if let Some(worker) = AI_WORKER.get()
                    .and_then(|v| v.lock().ok()).and_then(|mut v| v.take()) { drop(worker); }
                if let Some(worker) = DEEPSEEK_WORKER.get()
                    .and_then(|v| v.lock().ok()).and_then(|mut v| v.take()) { drop(worker); }
                if let Some(value) = AI_SNAPSHOT.get() {
                    if let Ok(mut value) = value.lock() { *value = None; }
                }
                if let Some(value) = DEEPSEEK_SNAPSHOT.get() {
                    if let Ok(mut value) = value.lock() { *value = None; }
                }
                maintenance_refresh();
            }
            0
        }
        WM_COMMAND if (wparam & 0xffff) == MAINTENANCE_IMPORT_LEGACY => {
            let legacy = std::env::var_os("APPDATA")
                .map(std::path::PathBuf::from)
                .map(|root| root.join("kil0bit-system-monitor").join("config.json"));
            let imported = legacy.as_deref().and_then(|path| config_store::import_legacy(path).ok());
            let result = if let Some(config) = imported {
                if let Ok(mut current) = config_lock().lock() {
                    *current = config;
                    let _ = config_store::save(&config_path(), &current);
                }
                load_config();
                start_ai_worker(OVERLAY_HANDLE.load(Ordering::Relaxed));
                start_deepseek_worker(OVERLAY_HANDLE.load(Ordering::Relaxed));
                maintenance_refresh();
                "Legacy settings imported; the original file was not modified."
            } else { "Could not read a valid legacy settings file." };
            let wide: Vec<u16> = result.encode_utf16().chain(Some(0)).collect();
            let title: Vec<u16> = "Import legacy config\0".encode_utf16().collect();
            MessageBoxW(hwnd, wide.as_ptr(), title.as_ptr(), 0);
            0
        }
        WM_CLOSE => { DestroyWindow(hwnd); 0 }
        WM_DESTROY => { MAINTENANCE_HANDLE.store(null_mut(), Ordering::Relaxed); 0 }
        _ => DefWindowProcW(hwnd, message, wparam, lparam),
    }
}

unsafe fn confirm_maintenance(hwnd: Hwnd, question: &str) -> bool {
    let wide: Vec<u16> = question.encode_utf16().chain(Some(0)).collect();
    let title: Vec<u16> = "Confirm settings change\0".encode_utf16().collect();
    MessageBoxW(hwnd, wide.as_ptr(), title.as_ptr(), 4 | 0x20) == 6
}

unsafe fn maintenance_refresh() {
    let overlay = OVERLAY_HANDLE.load(Ordering::Relaxed);
    if !overlay.is_null() { PostMessageW(overlay, WM_APP_REFRESH, 0, 0); }
    let settings = SETTINGS_HANDLE.load(Ordering::Relaxed);
    if !settings.is_null() {
        DestroyWindow(settings);
        if !overlay.is_null() { open_settings_window(overlay); }
    }
}

unsafe fn open_maintenance_window(owner: Hwnd) {
    let existing = MAINTENANCE_HANDLE.load(Ordering::Relaxed);
    if !existing.is_null() { ShowWindow(existing, SW_SHOW); return; }
    let class_name: Vec<u16> = "Kil0bitRustMaintenance\0".encode_utf16().collect();
    let title: Vec<u16> = "Reset and import\0".encode_utf16().collect();
    let instance = GetModuleHandleW(null());
    let class = WindowClass {
        style: 0, window_proc: Some(maintenance_window_proc), class_extra: 0,
        window_extra: 0, instance, icon: null_mut(), cursor: null_mut(),
        background: null_mut(), menu_name: null(), class_name: class_name.as_ptr(),
    };
    if RegisterClassW(&class) == 0 && GetLastError() != ERROR_CLASS_ALREADY_EXISTS { return; }
    let window = CreateWindowExW(WS_EX_TOOLWINDOW, class_name.as_ptr(), title.as_ptr(),
        WS_OVERLAPPEDWINDOW, 430, 230, 460, 300, owner, null_mut(), instance, null_mut());
    if window.is_null() { return; }
    MAINTENANCE_HANDLE.store(window, Ordering::Relaxed);
    devices_child(window, "BUTTON", "Reset appearance", MAINTENANCE_RESET_APPEARANCE,
        25, 25, 385, 40, WS_TABSTOP);
    devices_child(window, "BUTTON", "Reset all Rust settings", MAINTENANCE_RESET_ALL,
        25, 80, 385, 40, WS_TABSTOP);
    devices_child(window, "BUTTON", "Import legacy settings (read-only)", MAINTENANCE_IMPORT_LEGACY,
        25, 135, 385, 40, WS_TABSTOP);
    ShowWindow(window, SW_SHOW);
    UpdateWindow(window);
}

unsafe extern "system" fn devices_window_proc(
    hwnd: Hwnd, message: u32, wparam: usize, lparam: isize,
) -> Lresult {
    match message {
        WM_COMMAND if (wparam & 0xffff) == DEVICES_SAVE => {
            let combo = NETWORK_COMBO.load(Ordering::Relaxed);
            let list = DISK_LIST.load(Ordering::Relaxed);
            let edit = GPU_INDEX_EDIT.load(Ordering::Relaxed);
            let network_index = SendMessageW(combo, 0x0147, 0, 0).max(0) as usize;
            let network = NETWORK_CHOICES.get()
                .and_then(|choices| choices.lock().ok())
                .and_then(|choices| choices.get(network_index).cloned())
                .unwrap_or_else(|| "Default".into());
            let mut disks = Vec::new();
            if let Some(choices) = DISK_CHOICES.get().and_then(|v| v.lock().ok()) {
                for (index, name) in choices.iter().enumerate() {
                    if SendMessageW(list, 0x0187, index, 0) > 0 {
                        disks.push(name.clone());
                    }
                }
            }
            let mut text = [0_u16; 12];
            let length = GetWindowTextW(edit, text.as_mut_ptr(), text.len() as i32).max(0) as usize;
            let gpu_index = String::from_utf16_lossy(&text[..length]).trim().parse::<u32>()
                .unwrap_or(0).min(15);
            if let Ok(mut config) = config_lock().lock() {
                config.network_adapter = network;
                config.selected_disks = if disks.is_empty() { "None".into() }
                    else { disks.join(";") };
                config.gpu_index = gpu_index;
                let _ = config_store::save(&config_path(), &config);
            }
            let overlay = OVERLAY_HANDLE.load(Ordering::Relaxed);
            if !overlay.is_null() { PostMessageW(overlay, WM_APP_REFRESH, 0, 0); }
            DestroyWindow(hwnd);
            0
        }
        WM_CLOSE => { DestroyWindow(hwnd); 0 }
        WM_DESTROY => {
            DEVICES_HANDLE.store(null_mut(), Ordering::Relaxed);
            NETWORK_COMBO.store(null_mut(), Ordering::Relaxed);
            DISK_LIST.store(null_mut(), Ordering::Relaxed);
            GPU_INDEX_EDIT.store(null_mut(), Ordering::Relaxed);
            0
        }
        _ => DefWindowProcW(hwnd, message, wparam, lparam),
    }
}

#[allow(clippy::too_many_arguments)]
unsafe fn devices_child(parent: Hwnd, class: &str, caption: &str, id: usize,
    x: i32, y: i32, width: i32, height: i32, style: u32) -> Hwnd {
    let class: Vec<u16> = class.encode_utf16().chain(Some(0)).collect();
    let caption: Vec<u16> = caption.encode_utf16().chain(Some(0)).collect();
    CreateWindowExW(0, class.as_ptr(), caption.as_ptr(), WS_CHILD | WS_VISIBLE | style,
        x, y, width, height, parent, id as *mut c_void, GetModuleHandleW(null()), null_mut())
}

unsafe fn open_devices_window(owner: Hwnd) {
    let existing = DEVICES_HANDLE.load(Ordering::Relaxed);
    if !existing.is_null() { ShowWindow(existing, SW_SHOW); return; }
    let class_name: Vec<u16> = "Kil0bitRustDevices\0".encode_utf16().collect();
    let title: Vec<u16> = "Monitoring devices\0".encode_utf16().collect();
    let instance = GetModuleHandleW(null());
    let class = WindowClass {
        style: 0, window_proc: Some(devices_window_proc), class_extra: 0,
        window_extra: 0, instance, icon: null_mut(), cursor: null_mut(),
        background: null_mut(), menu_name: null(), class_name: class_name.as_ptr(),
    };
    if RegisterClassW(&class) == 0 && GetLastError() != ERROR_CLASS_ALREADY_EXISTS { return; }
    let window = CreateWindowExW(WS_EX_TOOLWINDOW, class_name.as_ptr(), title.as_ptr(),
        WS_OVERLAPPEDWINDOW, 360, 160, 520, 470, owner, null_mut(), instance, null_mut());
    if window.is_null() { return; }
    DEVICES_HANDLE.store(window, Ordering::Relaxed);
    let config = config_lock().lock().map(|c| c.clone()).unwrap_or_default();
    devices_child(window, "STATIC", "Network adapter", 0, 20, 20, 470, 24, 0);
    let combo = devices_child(window, "COMBOBOX", "", 0, 20, 50, 465, 180,
        WS_VSCROLL | WS_TABSTOP | 0x0003);
    NETWORK_COMBO.store(combo, Ordering::Relaxed);
    let mut networks = vec!["Default".to_owned()];
    networks.extend(sysinfo::Networks::new_with_refreshed_list().keys()
        .filter(|name| !name.contains("Loopback")).cloned());
    networks.sort();
    networks.dedup();
    let selected_network = networks.iter().position(|name| *name == config.network_adapter)
        .unwrap_or_else(|| networks.iter().position(|name| name == "Default").unwrap_or(0));
    for name in &networks {
        let wide: Vec<u16> = name.encode_utf16().chain(Some(0)).collect();
        SendMessageW(combo, 0x0143, 0, wide.as_ptr() as isize);
    }
    SendMessageW(combo, 0x014E, selected_network, 0);
    *NETWORK_CHOICES.get_or_init(|| Mutex::new(Vec::new())).lock().unwrap() = networks;

    devices_child(window, "STATIC", "Disk volumes (Ctrl/Shift to select multiple)",
        0, 20, 95, 470, 24, 0);
    let list = devices_child(window, "LISTBOX", "", 0, 20, 125, 465, 140,
        WS_BORDER | WS_VSCROLL | WS_TABSTOP | 0x0008);
    DISK_LIST.store(list, Ordering::Relaxed);
    let mut disks: Vec<String> = sysinfo::Disks::new_with_refreshed_list().iter()
        .map(|disk| disk.mount_point().to_string_lossy().into_owned()).collect();
    disks.sort();
    disks.dedup();
    for (index, name) in disks.iter().enumerate() {
        let wide: Vec<u16> = name.encode_utf16().chain(Some(0)).collect();
        SendMessageW(list, 0x0180, 0, wide.as_ptr() as isize);
        let selected = config.selected_disks == "All"
            || (config.selected_disks == "Default" && index == 0)
            || config.selected_disks.split(';').any(|item| item == name);
        if selected { SendMessageW(list, 0x0185, 1, index as isize); }
    }
    *DISK_CHOICES.get_or_init(|| Mutex::new(Vec::new())).lock().unwrap() = disks;
    devices_child(window, "STATIC", "NVIDIA GPU index (0 = first device)",
        0, 20, 285, 320, 24, 0);
    let edit = devices_child(window, "EDIT", &config.gpu_index.to_string(), 0,
        345, 283, 95, 28, WS_BORDER | WS_TABSTOP);
    GPU_INDEX_EDIT.store(edit, Ordering::Relaxed);
    devices_child(window, "BUTTON", "Save selection", DEVICES_SAVE,
        20, 350, 175, 36, WS_TABSTOP);
    ShowWindow(window, SW_SHOW);
    UpdateWindow(window);
}

unsafe extern "system" fn deepseek_window_proc(
    hwnd: Hwnd, message: u32, wparam: usize, lparam: isize,
) -> Lresult {
    match message {
        WM_COMMAND if (wparam & 0xffff) == DEEPSEEK_SAVE => {
            let edit = DEEPSEEK_KEY_EDIT.load(Ordering::Relaxed);
            let length = GetWindowTextLengthW(edit).clamp(0, 4096) as usize;
            let mut buffer = vec![0_u16; length + 1];
            let actual = GetWindowTextW(edit, buffer.as_mut_ptr(), buffer.len() as i32).max(0) as usize;
            let mut key = String::from_utf16_lossy(&buffer[..actual]);
            if !key.trim().is_empty() {
                let result = crate::secret_store::SecretStore::deepseek()
                    .and_then(|store| store.save(&key));
                if result.is_ok() {
                    if let Ok(mut config) = config_lock().lock() {
                        config.deepseek_enabled = true;
                        let _ = config_store::save(&config_path(), &config);
                    }
                    restart_deepseek_worker(OVERLAY_HANDLE.load(Ordering::Relaxed));
                }
            }
            key.zeroize();
            buffer.zeroize();
            let blank = [0_u16];
            SetWindowTextW(edit, blank.as_ptr());
            DestroyWindow(hwnd);
            0
        }
        WM_COMMAND if (wparam & 0xffff) == DEEPSEEK_REMOVE => {
            let _ = crate::secret_store::SecretStore::deepseek().and_then(|store| store.remove());
            if let Ok(mut config) = config_lock().lock() {
                config.deepseek_enabled = false;
                let _ = config_store::save(&config_path(), &config);
            }
            if let Some(worker) = DEEPSEEK_WORKER.get()
                .and_then(|v| v.lock().ok()).and_then(|mut v| v.take()) { drop(worker); }
            if let Some(snapshot) = DEEPSEEK_SNAPSHOT.get() {
                if let Ok(mut value) = snapshot.lock() { *value = None; }
            }
            let overlay = OVERLAY_HANDLE.load(Ordering::Relaxed);
            if !overlay.is_null() { PostMessageW(overlay, WM_APP_REFRESH, 0, 0); }
            DestroyWindow(hwnd);
            0
        }
        WM_CLOSE => { DestroyWindow(hwnd); 0 }
        WM_DESTROY => {
            DEEPSEEK_HANDLE.store(null_mut(), Ordering::Relaxed);
            DEEPSEEK_KEY_EDIT.store(null_mut(), Ordering::Relaxed);
            0
        }
        _ => DefWindowProcW(hwnd, message, wparam, lparam),
    }
}

unsafe fn open_deepseek_window(owner: Hwnd) {
    let existing = DEEPSEEK_HANDLE.load(Ordering::Relaxed);
    if !existing.is_null() { ShowWindow(existing, SW_SHOW); return; }
    let class_name: Vec<u16> = "Kil0bitRustDeepSeek\0".encode_utf16().collect();
    let title: Vec<u16> = "DeepSeek balance\0".encode_utf16().collect();
    let instance = GetModuleHandleW(null());
    let class = WindowClass {
        style: 0, window_proc: Some(deepseek_window_proc), class_extra: 0,
        window_extra: 0, instance, icon: null_mut(), cursor: null_mut(),
        background: null_mut(), menu_name: null(), class_name: class_name.as_ptr(),
    };
    if RegisterClassW(&class) == 0 && GetLastError() != ERROR_CLASS_ALREADY_EXISTS { return; }
    let window = CreateWindowExW(WS_EX_TOOLWINDOW, class_name.as_ptr(), title.as_ptr(),
        WS_OVERLAPPEDWINDOW, 420, 230, 460, 235, owner, null_mut(), instance, null_mut());
    if window.is_null() { return; }
    DEEPSEEK_HANDLE.store(window, Ordering::Relaxed);
    let status = if crate::secret_store::SecretStore::deepseek().is_ok_and(|s| s.exists()) {
        "API key saved for this Windows user" } else { "Enter a DeepSeek API key" };
    devices_child(window, "STATIC", status, 0, 20, 20, 410, 25, 0);
    let edit = devices_child(window, "EDIT", "", 0, 20, 55, 410, 28,
        WS_BORDER | WS_TABSTOP | ES_PASSWORD);
    DEEPSEEK_KEY_EDIT.store(edit, Ordering::Relaxed);
    devices_child(window, "BUTTON", "Save key", DEEPSEEK_SAVE,
        20, 105, 150, 34, WS_TABSTOP);
    devices_child(window, "BUTTON", "Remove key", DEEPSEEK_REMOVE,
        190, 105, 150, 34, WS_TABSTOP);
    ShowWindow(window, SW_SHOW);
    UpdateWindow(window);
}

fn start_deepseek_worker(hwnd: Hwnd) {
    let config = config_lock().lock().map(|c| c.clone()).unwrap_or_default();
    if hwnd.is_null() || !config.deepseek_enabled { return; }
    let Ok(store) = crate::secret_store::SecretStore::deepseek() else { return; };
    if !store.exists() { return; }
    let handle = hwnd as usize;
    let worker = super::ai_usage::DeepSeekWorker::start(
        config.ai_poll_interval_seconds, store, move |event| {
            if let Ok(snapshot) = event {
                if let Some(state) = DEEPSEEK_SNAPSHOT.get() {
                    if let Ok(mut current) = state.lock() { *current = Some(snapshot); }
                }
                unsafe { PostMessageW(handle as Hwnd, WM_APP_REFRESH, 0, 0); }
            }
        });
    if let Some(slot) = DEEPSEEK_WORKER.get() {
        if let Ok(mut slot) = slot.lock() { *slot = Some(worker); }
    }
}

fn restart_deepseek_worker(hwnd: Hwnd) {
    if let Some(worker) = DEEPSEEK_WORKER.get()
        .and_then(|v| v.lock().ok()).and_then(|mut v| v.take()) { drop(worker); }
    start_deepseek_worker(hwnd);
}

unsafe fn save_opencode_key_from_settings() {
    let edit = AI_KEY_EDIT.load(Ordering::Relaxed);
    if edit.is_null() {
        return;
    }
    let length = GetWindowTextLengthW(edit).clamp(0, 4096) as usize;
    let mut buffer = vec![0_u16; length + 1];
    let actual = GetWindowTextW(edit, buffer.as_mut_ptr(), buffer.len() as i32).max(0) as usize;
    let mut key = String::from_utf16_lossy(&buffer[..actual]);
    if key.trim().is_empty() {
        key.zeroize();
        buffer.zeroize();
        set_ai_key_status("Enter an OpenCode Go API key first");
        return;
    }
    let result = crate::secret_store::SecretStore::opencode().and_then(|store| store.save(&key));
    key.zeroize();
    buffer.zeroize();
    let empty: Vec<u16> = "\0".encode_utf16().collect();
    SetWindowTextW(edit, empty.as_ptr());
    match result {
        Ok(()) => {
            if let Ok(mut config) = config_lock().lock() {
                config.opencode_enabled = true;
                let _ = config_store::save(&config_path(), &config);
            }
            set_ai_key_status("OpenCode key saved for this Windows user");
            start_ai_worker(OVERLAY_HANDLE.load(Ordering::Relaxed));
        }
        Err(_) => set_ai_key_status("Could not save OpenCode key"),
    }
}

fn start_ai_worker(hwnd: Hwnd) {
    if hwnd.is_null() {
        return;
    }
    let config = config_lock().lock().map(|c| c.clone()).unwrap_or_default();
    if !config.opencode_enabled && !config.codex_enabled {
        return;
    }
    let store = if config.codex_enabled {
        None
    } else {
        let Ok(store) = crate::secret_store::SecretStore::opencode() else {
            return;
        };
        if !store.exists() {
            return;
        }
        Some(store)
    };
    if config.codex_enabled && super::ai_usage::codex_auth_path().is_err() {
        return;
    }
    let Ok(mut slot) = AI_WORKER.get_or_init(|| Mutex::new(None)).lock() else {
        return;
    };
    if let Some(worker) = slot.as_ref() {
        worker.wake();
        return;
    }
    let hwnd_value = hwnd as usize;
    let callback = move |event: super::ai_usage::ProviderEvent| {
        if let Some(snapshot) = AI_SNAPSHOT.get() {
            if let Ok(mut value) = snapshot.lock() {
                *value = event.snapshot;
            }
        }
        unsafe {
            PostMessageW(hwnd_value as Hwnd, WM_APP_REFRESH, 0, 0);
        }
    };
    let worker = if config.codex_enabled {
        super::ai_usage::AiUsageWorker::start_codex(config.ai_poll_interval_seconds, callback)
    } else {
        let Some(store) = store else {
            return;
        };
        super::ai_usage::AiUsageWorker::start(config.ai_poll_interval_seconds, store, callback)
    };
    *slot = Some(worker);
}

fn save_ai_options_from_settings() {
    let checked: Vec<bool> = AI_OPTION_CONTROLS
        .iter()
        .map(|control| {
            let handle = control.load(Ordering::Relaxed);
            !handle.is_null()
                && unsafe { SendMessageW(handle, BM_GETCHECK, 0, 0) } == BST_CHECKED as isize
        })
        .collect();
    let combo = AI_INTERVAL_COMBO.load(Ordering::Relaxed);
    let selected = if combo.is_null() {
        -1
    } else {
        unsafe { SendMessageW(combo, 0x0147, 0, 0) as i32 }
    };
    let interval = AI_INTERVALS
        .get(selected.max(0) as usize)
        .copied()
        .unwrap_or(300);
    let mut provider_changed = false;
    if let Ok(mut config) = config_lock().lock() {
        if checked.len() == 5 {
            config.opencode_show_rolling = checked[0];
            config.opencode_show_weekly = checked[1];
            config.opencode_show_monthly = checked[2];
            config.ai_show_used_percent = checked[3];
            provider_changed = config.codex_enabled != checked[4];
            config.codex_enabled = checked[4];
            if config.codex_enabled {
                config.opencode_enabled = false;
            } else if provider_changed
                && crate::secret_store::SecretStore::opencode().is_ok_and(|store| store.exists())
            {
                config.opencode_enabled = true;
            }
        }
        config.ai_poll_interval_seconds = interval;
        let _ = config_store::save(&config_path(), &config);
    }
    if provider_changed {
        if let Some(worker) = AI_WORKER
            .get()
            .and_then(|v| v.lock().ok())
            .and_then(|mut v| v.take())
        {
            drop(worker);
        }
        if let Some(snapshot) = AI_SNAPSHOT.get() {
            if let Ok(mut value) = snapshot.lock() {
                *value = None;
            }
        }
        start_ai_worker(OVERLAY_HANDLE.load(Ordering::Relaxed));
    }
    if let Some(workers) = AI_WORKER.get() {
        if let Ok(slot) = workers.lock() {
            if let Some(worker) = slot.as_ref() {
                worker.set_interval(interval);
            }
        }
    }
    let overlay = OVERLAY_HANDLE.load(Ordering::Relaxed);
    if !overlay.is_null() {
        unsafe {
            PostMessageW(overlay, WM_APP_REFRESH, 0, 0);
        }
    }
}

unsafe fn save_port_options_from_settings(_settings_hwnd: Hwnd) {
    let checked: Vec<bool> = PORT_OPTION_CONTROLS
        .iter()
        .map(|control| {
            let handle = control.load(Ordering::Relaxed);
            !handle.is_null() && SendMessageW(handle, BM_GETCHECK, 0, 0) == BST_CHECKED as isize
        })
        .collect();
    if checked.len() != 12 {
        return;
    }
    let (old_snap, old_topmost) = config_lock()
        .lock()
        .map(|c| (c.stick_to_taskbar, c.always_on_top))
        .unwrap_or((true, true));
    if let Ok(mut config) = config_lock().lock() {
        config.show_cpu = checked[0];
        config.show_ram = checked[1];
        config.show_gpu = checked[2];
        config.show_temp = checked[3];
        config.show_disk = checked[4];
        config.show_net_up = checked[5];
        config.show_net_down = checked[6];
        config.show_overlay = checked[7];
        config.lock_position = checked[8];
        config.stick_to_taskbar = checked[9];
        config.always_on_top = checked[10];
        config.display_style = if checked[11] { "Compact" } else { "Text" }.into();
        let _ = config_store::save(&config_path(), &config);
    }
    POSITION_LOCKED.store(checked[8], Ordering::Relaxed);
    let overlay = OVERLAY_HANDLE.load(Ordering::Relaxed);
    if overlay.is_null() {
        return;
    }
    if old_snap != checked[9] {
        if checked[9] {
            attach_to_taskbar(overlay, 52);
        } else {
            detach_from_taskbar(overlay);
        }
    }
    if old_topmost != checked[10] {
        SetWindowPos(
            overlay,
            if checked[10] {
                HWND_TOPMOST
            } else {
                HWND_NOTOPMOST
            },
            0,
            0,
            0,
            0,
            SWP_NOSIZE | SWP_NOACTIVATE | 0x0002,
        );
    }
    if checked[7] {
        ShowWindow(overlay, SW_SHOWNOACTIVATE);
        refresh_overlay(overlay);
    } else {
        ShowWindow(overlay, 0);
    }
}

fn set_ai_key_status(text: &str) {
    let status = AI_KEY_STATUS.load(Ordering::Relaxed);
    if status.is_null() {
        return;
    }
    let wide: Vec<u16> = text.encode_utf16().chain(Some(0)).collect();
    unsafe {
        SetWindowTextW(status, wide.as_ptr());
    }
}

fn set_startup_status(text: &str) {
    let status = STARTUP_STATUS.load(Ordering::Relaxed);
    if status.is_null() {
        return;
    }
    let wide: Vec<u16> = text.encode_utf16().chain(Some(0)).collect();
    unsafe {
        SetWindowTextW(status, wide.as_ptr());
    }
}

unsafe fn taskbar_window() -> Hwnd {
    let class_name: Vec<u16> = "Shell_TrayWnd\0".encode_utf16().collect();
    FindWindowW(class_name.as_ptr(), null())
}

unsafe fn align_to_taskbar(hwnd: Hwnd, height: i32) {
    let taskbar = taskbar_window();
    if taskbar.is_null() {
        return;
    }
    let mut rect = Rect {
        left: 0,
        top: 0,
        right: 0,
        bottom: 0,
    };
    if GetWindowRect(taskbar, &mut rect) == 0 {
        return;
    }
    let y = rect.top + ((rect.bottom - rect.top) - height) / 2;
    SetWindowPos(
        hwnd,
        null_mut(),
        FREE_X.load(Ordering::Relaxed),
        y,
        0,
        0,
        SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE,
    );
}

unsafe fn attach_to_taskbar(hwnd: Hwnd, height: i32) {
    let taskbar = taskbar_window();
    if taskbar.is_null() {
        SNAP_TO_TASKBAR.store(false, Ordering::Relaxed);
        return;
    }
    SetWindowLongPtrW(hwnd, GWLP_HWNDPARENT, taskbar as isize);
    if !APPBAR_REGISTERED.load(Ordering::Relaxed) {
        let mut data = AppBarData {
            size: size_of::<AppBarData>() as u32,
            hwnd,
            callback_message: 0,
            edge: 0,
            rect: Rect {
                left: 0,
                top: 0,
                right: 0,
                bottom: 0,
            },
            lparam: 0,
        };
        let registered = SHAppBarMessage(ABM_NEW, &mut data) != 0;
        APPBAR_REGISTERED.store(registered, Ordering::Relaxed);
    }
    SNAP_TO_TASKBAR.store(true, Ordering::Relaxed);
    align_to_taskbar(hwnd, height);
}

unsafe fn detach_from_taskbar(hwnd: Hwnd) {
    SetWindowLongPtrW(hwnd, GWLP_HWNDPARENT, 0);
    remove_appbar(hwnd);
    SNAP_TO_TASKBAR.store(false, Ordering::Relaxed);
    SetWindowPos(
        hwnd,
        null_mut(),
        FREE_X.load(Ordering::Relaxed),
        FREE_Y.load(Ordering::Relaxed),
        0,
        0,
        SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE,
    );
}

unsafe fn remove_appbar(hwnd: Hwnd) {
    if APPBAR_REGISTERED.swap(false, Ordering::Relaxed) {
        let mut data = AppBarData {
            size: size_of::<AppBarData>() as u32,
            hwnd,
            callback_message: 0,
            edge: 0,
            rect: Rect {
                left: 0,
                top: 0,
                right: 0,
                bottom: 0,
            },
            lparam: 0,
        };
        SHAppBarMessage(ABM_REMOVE, &mut data);
    }
}

unsafe fn show_context_menu(hwnd: Hwnd) {
    let menu = CreatePopupMenu();
    if menu.is_null() {
        return;
    }
    let locked = POSITION_LOCKED.load(Ordering::Relaxed);
    let lock_text: Vec<u16> = if locked {
        "Unlock Position\0".encode_utf16().collect()
    } else {
        "Lock Position\0".encode_utf16().collect()
    };
    let exit_text: Vec<u16> = "Exit Monitor\0".encode_utf16().collect();
    AppendMenuW(
        menu,
        MF_STRING | if locked { MF_CHECKED } else { 0 },
        MENU_TOGGLE_LOCK as usize,
        lock_text.as_ptr(),
    );
    AppendMenuW(menu, MF_STRING, MENU_EXIT as usize, exit_text.as_ptr());
    let snap_text: Vec<u16> = if SNAP_TO_TASKBAR.load(Ordering::Relaxed) {
        "Free Position\0".encode_utf16().collect()
    } else {
        "Snap to Taskbar\0".encode_utf16().collect()
    };
    AppendMenuW(
        menu,
        MF_STRING
            | if SNAP_TO_TASKBAR.load(Ordering::Relaxed) {
                MF_CHECKED
            } else {
                0
            },
        MENU_TOGGLE_SNAP as usize,
        snap_text.as_ptr(),
    );
    let settings_text: Vec<u16> = "Settings\0".encode_utf16().collect();
    AppendMenuW(
        menu,
        MF_STRING,
        MENU_SETTINGS as usize,
        settings_text.as_ptr(),
    );
    let config = config_lock().lock().map(|c| c.clone()).unwrap_or_default();
    let topmost_text: Vec<u16> = if config.always_on_top {
        "Disable Keep on Top\0"
    } else {
        "Keep on Top\0"
    }
    .encode_utf16()
    .collect();
    AppendMenuW(
        menu,
        MF_STRING | if config.always_on_top { MF_CHECKED } else { 0 },
        MENU_TOGGLE_TOPMOST as usize,
        topmost_text.as_ptr(),
    );
    let task_text: Vec<u16> = "Task Manager\0".encode_utf16().collect();
    AppendMenuW(
        menu,
        MF_STRING,
        MENU_TASK_MANAGER as usize,
        task_text.as_ptr(),
    );
    let about_text: Vec<u16> = "About\0".encode_utf16().collect();
    AppendMenuW(menu, MF_STRING, MENU_ABOUT as usize, about_text.as_ptr());
    let mut point = Point { x: 0, y: 0 };
    if GetCursorPos(&mut point) != 0 {
        let selected = TrackPopupMenu(
            menu,
            TPM_RIGHTBUTTON | TPM_RETURNCMD,
            point.x,
            point.y,
            0,
            hwnd,
            null(),
        );
        match selected {
            MENU_TOGGLE_LOCK => {
                POSITION_LOCKED.store(!locked, Ordering::Relaxed);
                save_config_flags();
            }
            MENU_EXIT => {
                DestroyWindow(hwnd);
            }
            MENU_TOGGLE_SNAP => {
                if SNAP_TO_TASKBAR.load(Ordering::Relaxed) {
                    detach_from_taskbar(hwnd);
                } else {
                    attach_to_taskbar(hwnd, 52);
                }
                save_position();
            }
            MENU_SETTINGS => open_settings_window(hwnd),
            MENU_TASK_MANAGER => {
                let operation: Vec<u16> = "open\0".encode_utf16().collect();
                let executable: Vec<u16> = "taskmgr.exe\0".encode_utf16().collect();
                ShellExecuteW(
                    hwnd,
                    operation.as_ptr(),
                    executable.as_ptr(),
                    null(),
                    null(),
                    SW_SHOW,
                );
            }
            MENU_TOGGLE_TOPMOST => {
                if let Ok(mut config) = config_lock().lock() {
                    config.always_on_top = !config.always_on_top;
                    let insert_after = if config.always_on_top {
                        HWND_TOPMOST
                    } else {
                        HWND_NOTOPMOST
                    };
                    SetWindowPos(
                        hwnd,
                        insert_after,
                        0,
                        0,
                        0,
                        0,
                        SWP_NOSIZE | SWP_NOACTIVATE | 0x0002,
                    );
                    let _ = config_store::save(&config_path(), &config);
                }
            }
            MENU_ABOUT => {
                let text: Vec<u16> =
                    "Kil0bit System Monitor Rust port\nA native Windows telemetry overlay.\0"
                        .encode_utf16()
                        .collect();
                let caption: Vec<u16> = "About Kil0bit System Monitor\0".encode_utf16().collect();
                MessageBoxW(hwnd, text.as_ptr(), caption.as_ptr(), 0);
            }
            _ => {}
        }
    }
    DestroyMenu(menu);
}

/// Registers the window class, creates a layered popup, presents one sample frame, and runs its message loop.
pub fn run(_pixels: &[u8], _width: i32, _height: i32) -> Result<(), WinError> {
    let Some(_instance_lock) = SingleInstanceLock::acquire()? else {
        return Ok(());
    };
    unsafe {
        load_config();
        let mut telemetry = super::telemetry::TelemetryCollector::new();
        let initial_config = config_lock().lock().map(|c| c.clone()).unwrap_or_default();
        let first_sample = telemetry.sample(&initial_config);
        let _ = CURRENT_METRICS.set(Mutex::new(first_sample));
        let (initial_pixels, initial_width, initial_height) = render_current_bitmap();
        // Per-monitor notifications are required before creating the overlay HWND.
        SetProcessDpiAwarenessContext((-4isize) as *mut c_void);
        let instance = GetModuleHandleW(null());
        if instance.is_null() {
            return Err(last_error("GetModuleHandleW"));
        }
        let class_name: Vec<u16> = "Kil0bitRustOverlayPoT\0".encode_utf16().collect();
        let window_name: Vec<u16> = "Kil0bit System Monitor Rust — right-click for menu\0"
            .encode_utf16()
            .collect();
        let class = WindowClass {
            style: 0,
            window_proc: Some(window_proc),
            class_extra: 0,
            window_extra: 0,
            instance,
            icon: null_mut(),
            cursor: null_mut(),
            background: null_mut(),
            menu_name: null(),
            class_name: class_name.as_ptr(),
        };
        if RegisterClassW(&class) == 0 {
            return Err(last_error("RegisterClassW"));
        }

        let always_on_top = config_lock()
            .lock()
            .map(|c| c.always_on_top)
            .unwrap_or(true);
        let hwnd = CreateWindowExW(
            WS_EX_LAYERED | WS_EX_TOOLWINDOW | if always_on_top { WS_EX_TOPMOST } else { 0 },
            class_name.as_ptr(),
            window_name.as_ptr(),
            WS_POPUP,
            FREE_X.load(Ordering::Relaxed),
            FREE_Y.load(Ordering::Relaxed),
            initial_width,
            initial_height,
            null_mut(),
            null_mut(),
            instance,
            null_mut(),
        );
        if hwnd.is_null() {
            UnregisterClassW(class_name.as_ptr(), instance);
            return Err(last_error("CreateWindowExW"));
        }

        OVERLAY_HANDLE.store(hwnd, Ordering::Relaxed);
        let _ = AI_SNAPSHOT.set(Mutex::new(None));
        let _ = AI_WORKER.set(Mutex::new(None));
        let _ = DEEPSEEK_SNAPSHOT.set(Mutex::new(None));
        let _ = DEEPSEEK_WORKER.set(Mutex::new(None));
        ALTERNATE_ACCENT.store(false, Ordering::Relaxed);
        DPI_SCALE_PERCENT.store(100, Ordering::Relaxed);
        let surface_result = present_bitmap(hwnd, &initial_pixels, initial_width, initial_height);
        if surface_result.is_err() {
            DestroyWindow(hwnd);
            UnregisterClassW(class_name.as_ptr(), instance);
            return surface_result;
        }
        if SNAP_TO_TASKBAR.load(Ordering::Relaxed) {
            attach_to_taskbar(hwnd, initial_height);
        }
        if config_lock().lock().map(|c| c.show_overlay).unwrap_or(true) {
            ShowWindow(hwnd, SW_SHOWNOACTIVATE);
        } else {
            ShowWindow(hwnd, 0);
        }
        start_ai_worker(hwnd);
        start_deepseek_worker(hwnd);
        start_telemetry_worker(hwnd, telemetry);
        if !std::env::args().any(|arg| arg == "--startup") {
            open_settings_window(hwnd);
        }

        let mut message: Message = zeroed();
        loop {
            let result = GetMessageW(&mut message, null_mut(), 0, 0);
            if result == 0 {
                break;
            }
            if result == -1 {
                DestroyWindow(hwnd);
                UnregisterClassW(class_name.as_ptr(), instance);
                return Err(last_error("GetMessageW"));
            }
            TranslateMessage(&message);
            DispatchMessageW(&message);
        }
        UnregisterClassW(class_name.as_ptr(), instance);
        Ok(())
    }
}

struct SingleInstanceLock(*mut c_void);

impl SingleInstanceLock {
    fn acquire() -> Result<Option<Self>, WinError> {
        let name: Vec<u16> = "Local\\Kil0bitSystemMonitorRust-SingleInstance\0"
            .encode_utf16()
            .collect();
        let handle = unsafe { CreateMutexW(null_mut(), 0, name.as_ptr()) };
        if handle.is_null() {
            return Err(unsafe { last_error("CreateMutexW") });
        }
        if unsafe { GetLastError() } == ERROR_ALREADY_EXISTS {
            let class: Vec<u16> = "Kil0bitRustOverlayPoT\0".encode_utf16().collect();
            let existing = unsafe { FindWindowW(class.as_ptr(), null()) };
            if !existing.is_null() {
                unsafe {
                    PostMessageW(existing, WM_APP_SHOW_SETTINGS, 0, 0);
                }
            }
            unsafe {
                CloseHandle(handle);
            }
            Ok(None)
        } else {
            Ok(Some(Self(handle)))
        }
    }
}

impl Drop for SingleInstanceLock {
    fn drop(&mut self) {
        unsafe {
            CloseHandle(self.0);
        }
    }
}

fn config_path() -> std::path::PathBuf {
    CONFIG_PATH
        .get_or_init(|| {
            let root = std::env::var_os("APPDATA")
                .map(std::path::PathBuf::from)
                .unwrap_or_else(std::env::temp_dir);
            root.join("Kil0bitSystemMonitorRust").join("config.json")
        })
        .clone()
}

fn config_lock() -> &'static Mutex<AppConfig> {
    APP_CONFIG.get_or_init(|| Mutex::new(AppConfig::default()))
}

fn load_config() {
    let config = config_store::load(&config_path());
    FREE_X.store(config.x as i32, Ordering::Relaxed);
    FREE_Y.store(config.y as i32, Ordering::Relaxed);
    POSITION_LOCKED.store(config.lock_position, Ordering::Relaxed);
    SNAP_TO_TASKBAR.store(config.stick_to_taskbar, Ordering::Relaxed);
    let _ = set_startup_registration(config.launch_on_startup);
    if let Ok(mut current) = config_lock().lock() {
        *current = config;
    }
}

fn set_startup_registration(enabled: bool) -> std::io::Result<()> {
    #[link(name = "advapi32")]
    extern "system" {
        fn RegCreateKeyExW(
            key: *mut c_void,
            subkey: *const u16,
            reserved: u32,
            class: *mut u16,
            options: u32,
            access: u32,
            security: *mut c_void,
            result: *mut *mut c_void,
            disposition: *mut u32,
        ) -> i32;
        fn RegSetValueExW(
            key: *mut c_void,
            name: *const u16,
            reserved: u32,
            value_type: u32,
            data: *const u8,
            data_size: u32,
        ) -> i32;
        fn RegDeleteValueW(key: *mut c_void, name: *const u16) -> i32;
        fn RegCloseKey(key: *mut c_void) -> i32;
    }
    const HKEY_CURRENT_USER: *mut c_void = (-2147483647_isize) as *mut c_void;
    const KEY_SET_VALUE: u32 = 0x0002;
    const REG_SZ: u32 = 1;
    const ERROR_FILE_NOT_FOUND: i32 = 2;
    let subkey: Vec<u16> = "Software\\Microsoft\\Windows\\CurrentVersion\\Run\0"
        .encode_utf16()
        .collect();
    let value_name: Vec<u16> = "Kil0bitSystemMonitorRust\0".encode_utf16().collect();
    let mut key = null_mut();
    let mut disposition = 0_u32;
    let open_status = unsafe {
        RegCreateKeyExW(
            HKEY_CURRENT_USER,
            subkey.as_ptr(),
            0,
            null_mut(),
            0,
            KEY_SET_VALUE,
            null_mut(),
            &mut key,
            &mut disposition,
        )
    };
    if open_status != 0 {
        return Err(std::io::Error::from_raw_os_error(open_status));
    }
    let status = if enabled {
        match std::env::current_exe() {
            Ok(path) => {
                let command = format!("\"{}\" --startup", path.display());
                let data: Vec<u16> = command.encode_utf16().chain(Some(0)).collect();
                unsafe {
                    RegSetValueExW(
                        key,
                        value_name.as_ptr(),
                        0,
                        REG_SZ,
                        data.as_ptr().cast(),
                        (data.len() * size_of::<u16>()) as u32,
                    )
                }
            }
            Err(error) => {
                unsafe {
                    RegCloseKey(key);
                }
                return Err(error);
            }
        }
    } else {
        let code = unsafe { RegDeleteValueW(key, value_name.as_ptr()) };
        if code == ERROR_FILE_NOT_FOUND {
            0
        } else {
            code
        }
    };
    unsafe {
        RegCloseKey(key);
    }
    if status == 0 {
        Ok(())
    } else {
        Err(std::io::Error::from_raw_os_error(status))
    }
}

fn save_config_flags() {
    if let Ok(mut config) = config_lock().lock() {
        config.lock_position = POSITION_LOCKED.load(Ordering::Relaxed);
        config.stick_to_taskbar = SNAP_TO_TASKBAR.load(Ordering::Relaxed);
        config.x = FREE_X.load(Ordering::Relaxed) as f64;
        config.y = FREE_Y.load(Ordering::Relaxed) as f64;
        let _ = config_store::save(&config_path(), &config);
    }
}

fn save_position() {
    if let Ok(mut config) = config_lock().lock() {
        config.x = FREE_X.load(Ordering::Relaxed) as f64;
        if !SNAP_TO_TASKBAR.load(Ordering::Relaxed) {
            config.y = FREE_Y.load(Ordering::Relaxed) as f64;
        }
        config.stick_to_taskbar = SNAP_TO_TASKBAR.load(Ordering::Relaxed);
        let _ = config_store::save(&config_path(), &config);
    }
}

fn render_current_bitmap() -> (Vec<u8>, i32, i32) {
    let metrics = CURRENT_METRICS
        .get()
        .and_then(|state| state.lock().ok().map(|value| value.clone()))
        .unwrap_or_default();
    let config = config_lock()
        .lock()
        .map(|value| value.clone())
        .unwrap_or_default();
    let (base, width, height) = super::bitmap::build_metrics_bitmap(
        &metrics,
        &config,
        AI_SNAPSHOT
            .get()
            .and_then(|v| v.lock().ok())
            .and_then(|v| v.clone())
            .as_ref(),
        DEEPSEEK_SNAPSHOT.get().and_then(|v| v.lock().ok())
            .and_then(|v| v.clone()).as_ref(),
        ALTERNATE_ACCENT.load(Ordering::Relaxed),
    );
    super::bitmap::scale_surface(
        &base,
        width,
        height,
        (DPI_SCALE_PERCENT.load(Ordering::Relaxed) as f64 * config.scale_factor)
            .round().clamp(25.0, 400.0) as u32,
    )
}

fn start_telemetry_worker(hwnd: Hwnd, mut collector: super::telemetry::TelemetryCollector) {
    let (stop, stopped) = mpsc::channel();
    let handle = hwnd as usize;
    let thread = std::thread::spawn(move || loop {
        let config = config_lock().lock().map(|c| c.clone()).unwrap_or_default();
        if stopped.recv_timeout(Duration::from_millis(config.update_interval_ms as u64)).is_ok() {
            break;
        }
        let sample = collector.sample(&config);
        if let Some(state) = CURRENT_METRICS.get() {
            if let Ok(mut current) = state.lock() {
                *current = sample;
            }
        }
        unsafe { PostMessageW(handle as Hwnd, WM_APP_REFRESH, 0, 0); }
    });
    let _ = TELEMETRY_WORKER.set(Mutex::new(Some(TelemetryWorker { stop, thread: Some(thread) })));
}

unsafe fn refresh_overlay(hwnd: Hwnd) {
    let visible = config_lock()
        .lock()
        .map(|config| config.show_overlay)
        .unwrap_or(true);
    if !visible || FULLSCREEN_HIDDEN.load(Ordering::Relaxed) {
        ShowWindow(hwnd, 0);
        return;
    }
    let (pixels, width, height) = render_current_bitmap();
    let _ = present_bitmap(hwnd, &pixels, width, height);
}

unsafe fn update_fullscreen_visibility(hwnd: Hwnd) {
    let (enabled, app_visible) = config_lock()
        .lock()
        .map(|c| (c.hide_on_fullscreen, c.show_overlay))
        .unwrap_or((false, true));
    let foreground = GetForegroundWindow();
    let mut hidden = false;
    if enabled
        && app_visible
        && !foreground.is_null()
        && foreground != hwnd
        && foreground != SETTINGS_HANDLE.load(Ordering::Relaxed)
    {
        let mut class_name = [0_u16; 128];
        let length = GetClassNameW(foreground, class_name.as_mut_ptr(), class_name.len() as i32)
            .max(0) as usize;
        let class = String::from_utf16_lossy(&class_name[..length]);
        let shell_exempt = [
            "Shell_TrayWnd",
            "Progman",
            "WorkerW",
            "MultitaskingViewFrame",
            "XamlExplorerHostIslandWindow",
        ]
        .iter()
        .any(|candidate| candidate.eq_ignore_ascii_case(&class));
        if !shell_exempt {
            let mut rect = Rect {
                left: 0,
                top: 0,
                right: 0,
                bottom: 0,
            };
            let monitor = MonitorFromWindow(foreground, 2);
            let mut info = MonitorInfo {
                size: size_of::<MonitorInfo>() as u32,
                monitor: Rect { left: 0, top: 0, right: 0, bottom: 0 },
                work: Rect { left: 0, top: 0, right: 0, bottom: 0 },
                flags: 0,
            };
            hidden = !monitor.is_null()
                && GetMonitorInfoW(monitor, &mut info) != 0
                && GetWindowRect(foreground, &mut rect) != 0
                && GetWindowLongPtrW(foreground, -16) & WS_CAPTION == 0
                && rect.left <= info.monitor.left && rect.top <= info.monitor.top
                && rect.right >= info.monitor.right && rect.bottom >= info.monitor.bottom;
        }
    }
    if let Ok(mut candidate) = FULLSCREEN_CANDIDATE.lock() {
        if candidate.0 != hidden {
            *candidate = (hidden, 1);
            return;
        }
        candidate.1 = candidate.1.saturating_add(1);
        if candidate.1 < 2 { return; }
    }
    let previous = FULLSCREEN_HIDDEN.swap(hidden, Ordering::Relaxed);
    if previous != hidden {
        if hidden {
            ShowWindow(hwnd, 0);
        } else if app_visible {
            ShowWindow(hwnd, SW_SHOWNOACTIVATE);
        }
        if !hidden && app_visible {
            refresh_overlay(hwnd);
        }
    }
}

unsafe fn present_bitmap(
    hwnd: Hwnd,
    pixels: &[u8],
    width: i32,
    height: i32,
) -> Result<(), WinError> {
    if width <= 0 || height <= 0 || pixels.len() != width as usize * height as usize * 4 {
        return Err(WinError("Invalid premultiplied BGRA surface", 0));
    }
    let screen_dc = GetDC(null_mut());
    if screen_dc.is_null() {
        return Err(last_error("GetDC"));
    }
    let memory_dc = CreateCompatibleDC(screen_dc);
    if memory_dc.is_null() {
        ReleaseDC(null_mut(), screen_dc);
        return Err(last_error("CreateCompatibleDC"));
    }
    let info = BitmapInfo {
        header: BitmapInfoHeader {
            size: size_of::<BitmapInfoHeader>() as u32,
            width,
            height: -height,
            planes: 1,
            bit_count: 32,
            compression: BI_RGB,
            size_image: (width * height * 4) as u32,
            x_pixels_per_meter: 0,
            y_pixels_per_meter: 0,
            colors_used: 0,
            colors_important: 0,
        },
        colors: [RgbQuad {
            blue: 0,
            green: 0,
            red: 0,
            reserved: 0,
        }],
    };
    let mut bits = null_mut();
    let bitmap = CreateDIBSection(screen_dc, &info, DIB_RGB_COLORS, &mut bits, null_mut(), 0);
    if bitmap.is_null() || bits.is_null() {
        DeleteDC(memory_dc);
        ReleaseDC(null_mut(), screen_dc);
        return Err(last_error("CreateDIBSection"));
    }
    std::ptr::copy_nonoverlapping(pixels.as_ptr(), bits.cast::<u8>(), pixels.len());
    let old_bitmap = SelectObject(memory_dc, bitmap);
    let mut window_rect = Rect {
        left: 100,
        top: 100,
        right: 100 + width,
        bottom: 100 + height,
    };
    GetWindowRect(hwnd, &mut window_rect);
    let destination = Point {
        x: window_rect.left,
        y: window_rect.top,
    };
    let source = Point { x: 0, y: 0 };
    let size = Size {
        cx: width,
        cy: height,
    };
    let blend = BlendFunction {
        operation: AC_SRC_OVER,
        flags: 0,
        source_constant_alpha: 255,
        alpha_format: AC_SRC_ALPHA,
    };
    let success = UpdateLayeredWindow(
        hwnd,
        screen_dc,
        &destination,
        &size,
        memory_dc,
        &source,
        0,
        &blend,
        ULW_ALPHA,
    );
    SelectObject(memory_dc, old_bitmap);
    DeleteObject(bitmap);
    DeleteDC(memory_dc);
    ReleaseDC(null_mut(), screen_dc);
    if success == 0 {
        return Err(last_error("UpdateLayeredWindow"));
    }
    Ok(())
}

unsafe fn last_error(operation: &'static str) -> WinError {
    WinError(operation, GetLastError())
}
