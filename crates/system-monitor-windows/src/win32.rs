//! Minimal Win32 layered-window PoT using the system ABI directly.
//!
//! No .NET runtime and no third-party native runtime are involved.

use std::ffi::c_void;
use std::fmt;
use std::mem::{size_of, zeroed};
use std::ptr::{null, null_mut};
use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};
use std::sync::{Mutex, OnceLock};
use system_monitor_core::{config_store, AppConfig};
use zeroize::Zeroize;

pub type Hwnd = *mut c_void;
type Hinstance = *mut c_void;
type Hdc = *mut c_void;
type Hbitmap = *mut c_void;
type Hgdiobj = *mut c_void;
type Lresult = isize;
type WndProc = unsafe extern "system" fn(Hwnd, u32, usize, isize) -> Lresult;

const WS_POPUP: u32 = 0x8000_0000;
const WS_EX_TOPMOST: u32 = 0x0000_0008;
const WS_EX_TOOLWINDOW: u32 = 0x0000_0080;
const WS_EX_LAYERED: u32 = 0x0008_0000;
const WM_DESTROY: u32 = 0x0002;
const WM_CLOSE: u32 = 0x0010;
const WM_KEYDOWN: u32 = 0x0100;
const WM_NCHITTEST: u32 = 0x0084;
const WM_RBUTTONUP: u32 = 0x0205;
const WM_MOVE: u32 = 0x0003;
const WM_WINDOWPOSCHANGED: u32 = 0x0047;
const WM_EXITSIZEMOVE: u32 = 0x0232;
const WM_DISPLAYCHANGE: u32 = 0x007E;
const WM_SETTINGCHANGE: u32 = 0x001A;
const WM_DPICHANGED: u32 = 0x02E0;
const WM_COMMAND: u32 = 0x0111;
const WM_APP_REFRESH: u32 = 0x8001;
const GWLP_HWNDPARENT: i32 = -8;
const SWP_NOSIZE: u32 = 0x0001;
const SWP_NOZORDER: u32 = 0x0004;
const SWP_NOACTIVATE: u32 = 0x0010;
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
const SETTINGS_TOGGLE_ACCENT: usize = 1001;
const SETTINGS_SAVE_OPENCODE_KEY: usize = 1002;
const SETTINGS_REMOVE_OPENCODE_KEY: usize = 1003;
const WS_BORDER: u32 = 0x0080_0000;
const ES_PASSWORD: u32 = 0x0020;
const WS_OVERLAPPEDWINDOW: u32 = 0x00CF_0000;
const WS_CHILD: u32 = 0x4000_0000;
const WS_VISIBLE: u32 = 0x1000_0000;
const WS_TABSTOP: u32 = 0x0001_0000;
const SW_SHOW: i32 = 5;
const ERROR_CLASS_ALREADY_EXISTS: u32 = 1410;
static POSITION_LOCKED: AtomicBool = AtomicBool::new(false);
static SNAP_TO_TASKBAR: AtomicBool = AtomicBool::new(true);
static APPBAR_REGISTERED: AtomicBool = AtomicBool::new(false);
static FREE_X: AtomicI32 = AtomicI32::new(100);
static FREE_Y: AtomicI32 = AtomicI32::new(100);
static ALTERNATE_ACCENT: AtomicBool = AtomicBool::new(false);
static DPI_SCALE_PERCENT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(100);
static CONFIG_PATH: OnceLock<std::path::PathBuf> = OnceLock::new();
static APP_CONFIG: OnceLock<Mutex<AppConfig>> = OnceLock::new();
static OVERLAY_HANDLE: std::sync::atomic::AtomicPtr<c_void> =
    std::sync::atomic::AtomicPtr::new(null_mut());
static SETTINGS_HANDLE: std::sync::atomic::AtomicPtr<c_void> =
    std::sync::atomic::AtomicPtr::new(null_mut());
static AI_KEY_EDIT: std::sync::atomic::AtomicPtr<c_void> =
    std::sync::atomic::AtomicPtr::new(null_mut());
static AI_KEY_STATUS: std::sync::atomic::AtomicPtr<c_void> =
    std::sync::atomic::AtomicPtr::new(null_mut());
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
    fn GetWindowRect(hwnd: Hwnd, rect: *mut Rect) -> i32;
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
}

#[link(name = "shell32")]
extern "system" {
    fn SHAppBarMessage(message: u32, data: *mut AppBarData) -> usize;
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
        WM_CLOSE => {
            DestroyWindow(hwnd);
            0
        }
        WM_APP_REFRESH => {
            refresh_overlay(hwnd);
            0
        }
        WM_DESTROY => {
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
            let status = match crate::secret_store::SecretStore::opencode()
                .and_then(|store| store.remove())
            {
                Ok(()) => "OpenCode key removed",
                Err(_) => "Could not remove OpenCode key",
            };
            set_ai_key_status(status);
            0
        }
        WM_CLOSE => {
            DestroyWindow(hwnd);
            0
        }
        WM_DESTROY => {
            SETTINGS_HANDLE
                .compare_exchange(hwnd, null_mut(), Ordering::Relaxed, Ordering::Relaxed)
                .ok();
            AI_KEY_EDIT.store(null_mut(), Ordering::Relaxed);
            AI_KEY_STATUS.store(null_mut(), Ordering::Relaxed);
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
    let title: Vec<u16> = "Rust Overlay PoT Settings\0".encode_utf16().collect();
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
        520,
        300,
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
    ShowWindow(window, SW_SHOW);
    UpdateWindow(window);
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
    let result = crate::secret_store::SecretStore::opencode().and_then(|store| store.save(&key));
    key.zeroize();
    buffer.zeroize();
    let empty: Vec<u16> = "\0".encode_utf16().collect();
    SetWindowTextW(edit, empty.as_ptr());
    match result {
        Ok(()) => set_ai_key_status("OpenCode key saved for this Windows user"),
        Err(_) => set_ai_key_status("Could not save OpenCode key"),
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
    let exit_text: Vec<u16> = "Exit PoT\0".encode_utf16().collect();
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
            _ => {}
        }
    }
    DestroyMenu(menu);
}

/// Registers the window class, creates a layered popup, presents one sample frame, and runs its message loop.
pub fn run(pixels: &[u8], width: i32, height: i32) -> Result<(), WinError> {
    unsafe {
        load_config();
        // Per-monitor notifications are required before creating the overlay HWND.
        SetProcessDpiAwarenessContext((-4isize) as *mut c_void);
        let instance = GetModuleHandleW(null());
        if instance.is_null() {
            return Err(last_error("GetModuleHandleW"));
        }
        let class_name: Vec<u16> = "Kil0bitRustOverlayPoT\0".encode_utf16().collect();
        let window_name: Vec<u16> = "Rust overlay PoT — right-click for menu\0"
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

        let hwnd = CreateWindowExW(
            WS_EX_LAYERED | WS_EX_TOOLWINDOW | WS_EX_TOPMOST,
            class_name.as_ptr(),
            window_name.as_ptr(),
            WS_POPUP,
            FREE_X.load(Ordering::Relaxed),
            FREE_Y.load(Ordering::Relaxed),
            width,
            height,
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
        ALTERNATE_ACCENT.store(false, Ordering::Relaxed);
        DPI_SCALE_PERCENT.store(100, Ordering::Relaxed);
        let surface_result = present_bitmap(hwnd, pixels, width, height);
        if surface_result.is_err() {
            DestroyWindow(hwnd);
            UnregisterClassW(class_name.as_ptr(), instance);
            return surface_result;
        }
        if SNAP_TO_TASKBAR.load(Ordering::Relaxed) {
            attach_to_taskbar(hwnd, height);
        }
        ShowWindow(hwnd, SW_SHOWNOACTIVATE);

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
    if let Ok(mut current) = config_lock().lock() {
        *current = config;
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
    let base = super::bitmap::build_demo_bitmap(ALTERNATE_ACCENT.load(Ordering::Relaxed));
    super::bitmap::scale_bitmap(&base, DPI_SCALE_PERCENT.load(Ordering::Relaxed))
}

unsafe fn refresh_overlay(hwnd: Hwnd) {
    let (pixels, width, height) = render_current_bitmap();
    let _ = present_bitmap(hwnd, &pixels, width, height);
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
