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
    const WM_CLOSE: u32 = 0x0010;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;

    #[link(name = "user32")]
    extern "system" {
        fn FindWindowW(class_name: *const u16, window_name: *const u16) -> Hwnd;
        fn GetWindowLongPtrW(hwnd: Hwnd, index: i32) -> isize;
        fn IsWindowVisible(hwnd: Hwnd) -> i32;
        fn GetWindowRect(hwnd: Hwnd, rect: *mut Rect) -> i32;
        fn PostMessageW(hwnd: Hwnd, message: u32, wparam: usize, lparam: isize) -> i32;
    }

    #[repr(C)]
    struct Rect {
        left: i32,
        top: i32,
        right: i32,
        bottom: i32,
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
                    "PASS: visible layered HWND, {}x{} pixels, graceful WM_CLOSE shutdown",
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
