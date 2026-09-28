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
    const SM_CYMENU: i32 = 15;
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
        fn GetDC(hwnd: Hwnd) -> Hwnd;
        fn ReleaseDC(hwnd: Hwnd, dc: Hwnd) -> i32;
        fn GetSystemMetrics(index: i32) -> i32;
    }

    #[link(name = "gdi32")]
    extern "system" {
        fn GetPixel(dc: Hwnd, x: i32, y: i32) -> u32;
    }

    #[repr(C)]
    #[derive(Clone, Copy)]
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

    fn send_mouse(flags: u32) -> Result<(), String> {
        let event = Input {
            kind: INPUT_MOUSE,
            data: InputData {
                mouse: MouseInput {
                    dx: 0,
                    dy: 0,
                    mouse_data: 0,
                    flags,
                    time: 0,
                    extra_info: 0,
                },
            },
        };
        let sent = unsafe { SendInput(1, &event, std::mem::size_of::<Input>() as i32) };
        if sent == 1 {
            Ok(())
        } else {
            Err(format!("SendInput inserted {sent}/1 mouse events"))
        }
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

    fn click_context_item(hwnd: Hwnd, item_index: i32) -> Result<(), String> {
        let mut overlay = Rect {
            left: 0,
            top: 0,
            right: 0,
            bottom: 0,
        };
        if unsafe { GetWindowRect(hwnd, &mut overlay) } == 0
            || unsafe {
                SetCursorPos(
                    (overlay.left + overlay.right) / 2,
                    (overlay.top + overlay.bottom) / 2,
                )
            } == 0
            || unsafe { PostMessageW(hwnd, WM_RBUTTONUP, 0, 0) } == 0
        {
            return Err("could not open overlay context menu".into());
        }
        let menu_class: Vec<u16> = "#32768\0".encode_utf16().collect();
        let deadline = Instant::now() + Duration::from_secs(3);
        let menu = loop {
            let found = unsafe { FindWindowW(menu_class.as_ptr(), std::ptr::null()) };
            if !found.is_null() {
                break found;
            }
            if Instant::now() >= deadline {
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
        let item_height = unsafe { GetSystemMetrics(SM_CYMENU) }.max(1);
        if unsafe { GetWindowRect(menu, &mut menu_rect) } == 0 {
            return Err("could not read native context menu bounds".into());
        }
        let target_y = menu_rect.top + 2 + item_index * item_height + item_height / 2;
        if unsafe { SetCursorPos(menu_rect.right - 10, target_y) } == 0 {
            return Err("could not move pointer to the selected context menu item".into());
        }
        click_left()
    }

    fn drag_window(hwnd: Hwnd, dx: i32, dy: i32) -> Result<(Rect, Rect), String> {
        let mut before = Rect {
            left: 0,
            top: 0,
            right: 0,
            bottom: 0,
        };
        if unsafe { GetWindowRect(hwnd, &mut before) } == 0 {
            return Err("could not read overlay bounds before drag".into());
        }
        let start_x = (before.left + before.right) / 2;
        let start_y = (before.top + before.bottom) / 2;
        if unsafe { SetCursorPos(start_x, start_y) } == 0 {
            return Err("could not move pointer onto overlay".into());
        }
        send_mouse(MOUSEEVENTF_LEFTDOWN)?;
        thread::sleep(Duration::from_millis(100));
        if unsafe { SetCursorPos(start_x + dx, start_y + dy) } == 0 {
            return Err("could not move pointer during overlay drag".into());
        }
        thread::sleep(Duration::from_millis(100));
        send_mouse(MOUSEEVENTF_LEFTUP)?;
        thread::sleep(Duration::from_millis(100));
        let mut after = Rect {
            left: 0,
            top: 0,
            right: 0,
            bottom: 0,
        };
        if unsafe { GetWindowRect(hwnd, &mut after) } == 0 {
            return Err("could not read overlay bounds after drag".into());
        }
        Ok((before, after))
    }

    pub fn run() -> Result<(), String> {
        let executable = std::env::args_os()
            .nth(1)
            .ok_or("usage: overlay-smoke <path-to-system-monitor-windows.exe>")?;
        let position_file = std::env::temp_dir().join("kil0bit-rust-overlay-pot-position.txt");
        let _ = std::fs::remove_file(&position_file);
        let mut child = Command::new(executable.clone())
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
        thread::sleep(Duration::from_millis(100));
        let screen_dc = unsafe { GetDC(std::ptr::null_mut()) };
        if screen_dc.is_null() {
            return Err("could not capture desktop pixels".into());
        }
        let mut glyph_pixel = u32::MAX;
        'scan: for y in rect.top + 8..rect.bottom - 8 {
            for x in rect.left + 8..rect.right - 8 {
                let sample = unsafe { GetPixel(screen_dc, x, y) };
                if sample != u32::MAX
                    && sample & 0xff > 210
                    && (sample >> 8) & 0xff > 210
                    && (sample >> 16) & 0xff > 210
                {
                    glyph_pixel = sample;
                    break 'scan;
                }
            }
        }
        unsafe { ReleaseDC(std::ptr::null_mut(), screen_dc) };
        if glyph_pixel == u32::MAX
            || glyph_pixel & 0xff < 210
            || (glyph_pixel >> 8) & 0xff < 210
            || (glyph_pixel >> 16) & 0xff < 210
        {
            let _ = unsafe { PostMessageW(hwnd, WM_CLOSE, 0, 0) };
            let _ = child.kill();
            let _ = child.wait();
            return Err(format!(
                "screen capture did not contain the white overlay glyph (COLORREF=0x{glyph_pixel:06x})"
            ));
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
        let mut second_instance = Command::new(executable.clone())
            .arg("--startup")
            .creation_flags(CREATE_NO_WINDOW)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| format!("launch second instance: {e}"))?;
        let second_deadline = Instant::now() + Duration::from_secs(5);
        loop {
            if let Some(status) = second_instance.try_wait().map_err(|e| e.to_string())? {
                if !status.success() || child.try_wait().map_err(|e| e.to_string())?.is_some() {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err("second launch did not forward to the first process cleanly".into());
                }
                break;
            }
            if Instant::now() >= second_deadline {
                let _ = second_instance.kill();
                let _ = second_instance.wait();
                let _ = child.kill();
                let _ = child.wait();
                return Err("second instance did not exit after activation forwarding".into());
            }
            thread::sleep(Duration::from_millis(50));
        }
        unsafe { PostMessageW(settings, WM_CLOSE, 0, 0) };

        let taskbar_class: Vec<u16> = "Shell_TrayWnd\0".encode_utf16().collect();
        let taskbar = unsafe { FindWindowW(taskbar_class.as_ptr(), std::ptr::null()) };
        if !taskbar.is_null() {
            let mut taskbar_rect = Rect {
                left: 0,
                top: 0,
                right: 0,
                bottom: 0,
            };
            if unsafe { GetWindowRect(taskbar, &mut taskbar_rect) } == 0 {
                let _ = child.kill();
                let _ = child.wait();
                return Err("could not read primary taskbar bounds".into());
            }
            let snap_y = taskbar_rect.top
                + ((taskbar_rect.bottom - taskbar_rect.top) - (rect.bottom - rect.top)) / 2;
            if (rect.top - snap_y).abs() > 2 {
                let _ = child.kill();
                let _ = child.wait();
                return Err(format!(
                    "overlay was not vertically centered on taskbar: y={}, expected={snap_y}",
                    rect.top
                ));
            }
            if let Err(error) = click_context_item(hwnd, 2) {
                let _ = child.kill();
                let _ = child.wait();
                return Err(error);
            }
            thread::sleep(Duration::from_millis(100));
            let mut free_rect = Rect {
                left: 0,
                top: 0,
                right: 0,
                bottom: 0,
            };
            if unsafe { GetWindowRect(hwnd, &mut free_rect) } == 0 || free_rect.top != 100 {
                let _ = child.kill();
                let _ = child.wait();
                return Err("Free Position did not restore the saved Y coordinate".into());
            }
            if let Err(error) = click_context_item(hwnd, 2) {
                let _ = child.kill();
                let _ = child.wait();
                return Err(error);
            }
            thread::sleep(Duration::from_millis(100));
            let mut snapped_rect = Rect {
                left: 0,
                top: 0,
                right: 0,
                bottom: 0,
            };
            if unsafe { GetWindowRect(hwnd, &mut snapped_rect) } == 0
                || (snapped_rect.top - snap_y).abs() > 2
            {
                let _ = child.kill();
                let _ = child.wait();
                return Err("Snap to Taskbar did not restore centered taskbar position".into());
            }
            if let Err(error) = click_context_item(hwnd, 2) {
                let _ = child.kill();
                let _ = child.wait();
                return Err(error);
            }
        }

        let (unlocked_before, unlocked_after) = match drag_window(hwnd, 36, 28) {
            Ok(bounds) => bounds,
            Err(error) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(error);
            }
        };
        if unlocked_before.left == unlocked_after.left && unlocked_before.top == unlocked_after.top
        {
            let _ = child.kill();
            let _ = child.wait();
            return Err("unlocked overlay did not move in response to drag".into());
        }
        if let Err(error) = click_context_item(hwnd, 0) {
            let _ = child.kill();
            let _ = child.wait();
            return Err(error);
        }
        let (locked_before, locked_after) = match drag_window(hwnd, 36, 28) {
            Ok(bounds) => bounds,
            Err(error) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(error);
            }
        };
        if locked_before.left != locked_after.left || locked_before.top != locked_after.top {
            let _ = child.kill();
            let _ = child.wait();
            return Err("locked overlay moved in response to drag".into());
        }
        if let Err(error) = click_context_item(hwnd, 0) {
            let _ = child.kill();
            let _ = child.wait();
            return Err(error);
        }
        let (unlocked_again_before, unlocked_again_after) = match drag_window(hwnd, 36, 28) {
            Ok(bounds) => bounds,
            Err(error) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(error);
            }
        };
        if unlocked_again_before.left == unlocked_again_after.left
            && unlocked_again_before.top == unlocked_again_after.top
        {
            let _ = child.kill();
            let _ = child.wait();
            return Err("unlocked overlay did not resume dragging after unlock".into());
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
                    "PASS: visible layered HWND ({}x{}), desktop glyph #{:02x}{:02x}{:02x}, Settings, single instance, snap/free, drag, lock/unlock, graceful shutdown",
                    rect.right - rect.left,
                    rect.bottom - rect.top,
                    glyph_pixel & 0xff,
                    (glyph_pixel >> 8) & 0xff,
                    (glyph_pixel >> 16) & 0xff
                );
                break;
            }
            if Instant::now() >= exit_deadline {
                let _ = child.kill();
                let _ = child.wait();
                return Err("overlay did not exit within 5 seconds after WM_CLOSE".into());
            }
            thread::sleep(Duration::from_millis(100));
        }
        let _ = std::fs::remove_file(&position_file);
        let mut restarted = Command::new(executable)
            .arg("--startup")
            .creation_flags(CREATE_NO_WINDOW)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| format!("restart overlay for position check: {e}"))?;
        let restart_deadline = Instant::now() + Duration::from_secs(10);
        let restarted_hwnd = loop {
            if let Some(status) = restarted.try_wait().map_err(|e| e.to_string())? {
                return Err(format!("overlay exited during persistence check: {status}"));
            }
            let found = unsafe { FindWindowW(class.as_ptr(), std::ptr::null()) };
            if !found.is_null() {
                break found;
            }
            if Instant::now() >= restart_deadline {
                let _ = restarted.kill();
                let _ = restarted.wait();
                return Err("overlay HWND did not reappear after restart".into());
            }
            thread::sleep(Duration::from_millis(100));
        };
        let mut restored = Rect {
            left: 0,
            top: 0,
            right: 0,
            bottom: 0,
        };
        if unsafe { GetWindowRect(restarted_hwnd, &mut restored) } == 0
            || restored.left != unlocked_again_after.left
            || restored.top != unlocked_again_after.top
        {
            let _ = child.kill();
            let _ = child.wait();
            return Err(format!(
                "free position was not restored after restart: got ({}, {}), expected ({}, {})",
                restored.left, restored.top, unlocked_again_after.left, unlocked_again_after.top
            ));
        }
        let _ = unsafe { PostMessageW(restarted_hwnd, WM_CLOSE, 0, 0) };
        let restart_exit_deadline = Instant::now() + Duration::from_secs(5);
        loop {
            if let Some(status) = restarted.try_wait().map_err(|e| e.to_string())? {
                if !status.success() {
                    return Err(format!("restarted overlay exited with status {status}"));
                }
                break;
            }
            if Instant::now() >= restart_exit_deadline {
                let _ = restarted.kill();
                let _ = restarted.wait();
                return Err("restarted overlay did not exit gracefully".into());
            }
            thread::sleep(Duration::from_millis(100));
        }
        let _ = std::fs::remove_file(&position_file);
        Ok(())
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
