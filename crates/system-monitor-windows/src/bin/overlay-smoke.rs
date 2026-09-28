#![cfg_attr(windows, windows_subsystem = "console")]

#[cfg(windows)]
mod windows_smoke {
    use std::ffi::c_void;
    use std::os::windows::process::CommandExt;
    use std::process::{Command, Stdio};
    use std::thread;
    use std::time::{Duration, Instant};

    type Hwnd = *mut c_void;
    const GWL_EXSTYLE: i32 = -20;
    const WS_EX_LAYERED: isize = 0x0008_0000;
    const WM_RBUTTONUP: u32 = 0x0205;
    const WM_CLOSE: u32 = 0x0010;
    const INPUT_MOUSE: u32 = 0;
    const MOUSEEVENTF_LEFTDOWN: u32 = 0x0002;
    const MOUSEEVENTF_LEFTUP: u32 = 0x0004;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;

    #[link(name = "user32")]
    extern "system" {
        fn FindWindowW(class_name: *const u16, window_name: *const u16) -> Hwnd;
        fn GetWindowLongPtrW(hwnd: Hwnd, index: i32) -> isize;
        fn IsWindowVisible(hwnd: Hwnd) -> i32;
        fn GetWindowRect(hwnd: Hwnd, rect: *mut Rect) -> i32;
        fn PostMessageW(hwnd: Hwnd, message: u32, wparam: usize, lparam: isize) -> i32;
        fn SetCursorPos(x: i32, y: i32) -> i32;
        fn SendInput(count: u32, inputs: *const Input, size: i32) -> u32;
    }

    #[repr(C)]
    struct Rect {
        left: i32,
        top: i32,
        right: i32,
        bottom: i32,
    }

    #[repr(C)]
    #[derive(Clone, Copy)]
    struct MouseInput {
        dx: i32,
        dy: i32,
        mouse_data: u32,
        flags: u32,
        time: u32,
        extra_info: usize,
    }

    #[repr(C)]
    union InputData {
        mouse: MouseInput,
        padding: [u64; 4],
    }

    #[repr(C)]
    struct Input {
        kind: u32,
        data: InputData,
    }

    fn click_left() -> Result<(), String> {
        let down = Input {
            kind: INPUT_MOUSE,
            data: InputData {
                mouse: MouseInput {
                    dx: 0,
                    dy: 0,
                    mouse_data: 0,
                    flags: MOUSEEVENTF_LEFTDOWN,
                    time: 0,
                    extra_info: 0,
                },
            },
        };
        let up = Input {
            kind: INPUT_MOUSE,
            data: InputData {
                mouse: MouseInput {
                    dx: 0,
                    dy: 0,
                    mouse_data: 0,
                    flags: MOUSEEVENTF_LEFTUP,
                    time: 0,
                    extra_info: 0,
                },
            },
        };
        let events = [down, up];
        let sent = unsafe {
            SendInput(
                events.len() as u32,
                events.as_ptr(),
                std::mem::size_of::<Input>() as i32,
            )
        };
        if sent == events.len() as u32 {
            Ok(())
        } else {
            Err(format!(
                "SendInput inserted {sent}/{} key events",
                events.len()
            ))
        }
    }

    pub fn run() -> Result<(), String> {
        let executable = std::env::args_os()
            .nth(1)
            .ok_or("usage: overlay-smoke <path-to-system-monitor-windows.exe>")?;
        let mut child = Command::new(executable)
            .creation_flags(CREATE_NO_WINDOW)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| format!("launch overlay PoT: {e}"))?;

        let class: Vec<u16> = "Kil0bitRustOverlayPoT\0".encode_utf16().collect();
        let deadline = Instant::now() + Duration::from_secs(10);
        let hwnd = loop {
            if let Some(status) = child.try_wait().map_err(|e| e.to_string())? {
                return Err(format!(
                    "overlay exited before creating its window: {status}"
                ));
            }
            let found = unsafe { FindWindowW(class.as_ptr(), std::ptr::null()) };
            if !found.is_null() {
                break found;
            }
            if Instant::now() >= deadline {
                let _ = child.kill();
                let _ = child.wait();
                return Err("overlay HWND did not appear within 10 seconds".into());
            }
            thread::sleep(Duration::from_millis(100));
        };

        let style = unsafe { GetWindowLongPtrW(hwnd, GWL_EXSTYLE) };
        if style & WS_EX_LAYERED == 0 {
            let _ = unsafe { PostMessageW(hwnd, WM_CLOSE, 0, 0) };
            let _ = child.kill();
            let _ = child.wait();
            return Err("overlay HWND is missing WS_EX_LAYERED".into());
        }
        if unsafe { IsWindowVisible(hwnd) } == 0 {
            let _ = unsafe { PostMessageW(hwnd, WM_CLOSE, 0, 0) };
            let _ = child.kill();
            let _ = child.wait();
            return Err("overlay HWND was created but is not visible".into());
        }

        let mut rect = Rect {
            left: 0,
            top: 0,
            right: 0,
            bottom: 0,
        };
        if unsafe { GetWindowRect(hwnd, &mut rect) } == 0
            || rect.right <= rect.left
            || rect.bottom <= rect.top
        {
            let _ = unsafe { PostMessageW(hwnd, WM_CLOSE, 0, 0) };
            let _ = child.kill();
            let _ = child.wait();
            return Err("overlay HWND has invalid screen bounds".into());
        }

        let menu_class: Vec<u16> = "#32768\0".encode_utf16().collect();
        let settings_class: Vec<u16> = "Kil0bitRustSettingsPoT\0".encode_utf16().collect();
        let menu_deadline = Instant::now() + Duration::from_secs(3);
        if unsafe { SetCursorPos((rect.left + rect.right) / 2, (rect.top + rect.bottom) / 2) } == 0
            || unsafe { PostMessageW(hwnd, WM_RBUTTONUP, 0, 0) } == 0
        {
            let _ = unsafe { PostMessageW(hwnd, WM_CLOSE, 0, 0) };
            let _ = child.kill();
            let _ = child.wait();
            return Err("could not open overlay context menu".into());
        }
        let menu = loop {
            let found = unsafe { FindWindowW(menu_class.as_ptr(), std::ptr::null()) };
            if !found.is_null() {
                break found;
            }
            if Instant::now() >= menu_deadline {
                let _ = unsafe { PostMessageW(hwnd, WM_CLOSE, 0, 0) };
                let _ = child.kill();
                let _ = child.wait();
                return Err("right-click did not open a native popup menu".into());
            }
            thread::sleep(Duration::from_millis(50));
        };
        let mut menu_rect = Rect {
            left: 0,
            top: 0,
            right: 0,
            bottom: 0,
        };
        if unsafe { GetWindowRect(menu, &mut menu_rect) } == 0
            || unsafe { SetCursorPos(menu_rect.right - 10, menu_rect.bottom - 10) } == 0
        {
            let _ = child.kill();
            let _ = child.wait();
            return Err("could not locate the Settings menu row".into());
        }
        if let Err(error) = click_left() {
            let _ = child.kill();
            let _ = child.wait();
            return Err(error);
        }

        let settings_deadline = Instant::now() + Duration::from_secs(3);
        let settings = loop {
            let found = unsafe { FindWindowW(settings_class.as_ptr(), std::ptr::null()) };
            if !found.is_null() {
                break found;
            }
            if Instant::now() >= settings_deadline {
                let _ = unsafe { PostMessageW(hwnd, WM_CLOSE, 0, 0) };
                let _ = child.kill();
                let _ = child.wait();
                return Err("Settings window did not open from the context menu".into());
            }
            thread::sleep(Duration::from_millis(50));
        };
        if unsafe { IsWindowVisible(settings) } == 0 {
            let _ = unsafe { PostMessageW(hwnd, WM_CLOSE, 0, 0) };
            let _ = child.kill();
            let _ = child.wait();
            return Err("Settings HWND was created but is not visible".into());
        }
        unsafe { PostMessageW(settings, WM_CLOSE, 0, 0) };

        if unsafe { PostMessageW(hwnd, WM_CLOSE, 0, 0) } == 0 {
            let _ = child.kill();
            let _ = child.wait();
            return Err("could not request graceful overlay shutdown".into());
        }
        let exit_deadline = Instant::now() + Duration::from_secs(5);
        loop {
            if let Some(status) = child.try_wait().map_err(|e| e.to_string())? {
                if !status.success() {
                    return Err(format!("overlay exited with status {status}"));
                }
                println!(
                    "PASS: visible layered HWND ({}x{}), native context menu, Settings HWND, graceful shutdown",
                    rect.right - rect.left,
                    rect.bottom - rect.top
                );
                return Ok(());
            }
            if Instant::now() >= exit_deadline {
                let _ = child.kill();
                let _ = child.wait();
                return Err("overlay did not exit within 5 seconds after WM_CLOSE".into());
            }
            thread::sleep(Duration::from_millis(100));
        }
    }
}

#[cfg(windows)]
fn main() {
    if let Err(error) = windows_smoke::run() {
        eprintln!("FAIL: {error}");
        std::process::exit(1);
    }
}

#[cfg(not(windows))]
fn main() {
    eprintln!("overlay-smoke must run on Windows");
    std::process::exit(2);
}
