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
    const WM_NCHITTEST: u32 = 0x0084;
    const WM_APP_DRAG_DIAGNOSTIC: u32 = 0x8003;
    const WM_CLOSE: u32 = 0x0010;
    const HTCLIENT: isize = 1;
    const HTCAPTION: isize = 2;
    const INPUT_MOUSE: u32 = 0;
    const MOUSEEVENTF_MOVE: u32 = 0x0001;
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
        fn SendMessageW(hwnd: Hwnd, message: u32, wparam: usize, lparam: isize) -> isize;
        fn SetCursorPos(x: i32, y: i32) -> i32;
        fn GetCursorPos(point: *mut Point) -> i32;
        fn SendInput(count: u32, inputs: *const Input, size: i32) -> u32;
        fn GetDC(hwnd: Hwnd) -> Hwnd;
        fn ReleaseDC(hwnd: Hwnd, dc: Hwnd) -> i32;
        fn GetSystemMetrics(index: i32) -> i32;
        fn WindowFromPoint(point: Point) -> Hwnd;
        fn GetForegroundWindow() -> Hwnd;
        fn SetForegroundWindow(hwnd: Hwnd) -> i32;
        fn GetGUIThreadInfo(thread_id: u32, info: *mut GuiThreadInfo) -> i32;
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
    struct Point {
        x: i32,
        y: i32,
    }

    #[repr(C)]
    struct GuiThreadInfo {
        size: u32,
        flags: u32,
        active: Hwnd,
        focus: Hwnd,
        capture: Hwnd,
        menu_owner: Hwnd,
        move_size: Hwnd,
        caret: Hwnd,
        caret_rect: Rect,
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
        send_mouse_event(flags, 0, 0)
    }

    fn send_mouse_event(flags: u32, dx: i32, dy: i32) -> Result<(), String> {
        let event = Input {
            kind: INPUT_MOUSE,
            data: InputData {
                mouse: MouseInput {
                    dx,
                    dy,
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
        click_left()?;
        // The input is queued to the overlay's UI thread; let TrackPopupMenu
        // return and apply the selected command before driving another action.
        thread::sleep(Duration::from_millis(150));
        Ok(())
    }

    fn wait_for_lock_state(hwnd: Hwnd, locked: bool) -> Result<(), String> {
        let deadline = Instant::now() + Duration::from_secs(3);
        loop {
            let mut rect = Rect {
                left: 0,
                top: 0,
                right: 0,
                bottom: 0,
            };
            if unsafe { GetWindowRect(hwnd, &mut rect) } == 0 {
                return Err("could not read overlay bounds for hit test".into());
            }
            let x = (rect.left + rect.right) / 2;
            let y = (rect.top + rect.bottom) / 2;
            let point = ((y as u16 as usize) << 16 | (x as u16 as usize)) as isize;
            let hit = unsafe { SendMessageW(hwnd, WM_NCHITTEST, 0, point) };
            if hit == (if locked { HTCLIENT } else { HTCAPTION }) {
                return Ok(());
            }
            if Instant::now() >= deadline {
                return Err(format!(
                    "lock menu did not set hit test state to {} (got {hit})",
                    if locked { "locked" } else { "unlocked" }
                ));
            }
            thread::sleep(Duration::from_millis(50));
        }
    }

    fn drag_window(hwnd: Hwnd, dx: i32, dy: i32) -> Result<(Rect, Rect), String> {
        if unsafe { IsWindowVisible(hwnd) } == 0 {
            return Err("overlay became hidden before drag".into());
        }
        let mut before = Rect {
            left: 0,
            top: 0,
            right: 0,
            bottom: 0,
        };
        if unsafe { GetWindowRect(hwnd, &mut before) } == 0 {
            return Err("could not read overlay bounds before drag".into());
        }
        let center_x = (before.left + before.right) / 2;
        let center_y = (before.top + before.bottom) / 2;
        let mut target = None;
        for radius in (0..(before.right - before.left).max(before.bottom - before.top)).step_by(3) {
            for y in (center_y - radius..=center_y + radius).step_by(3) {
                if y < before.top || y >= before.bottom {
                    continue;
                }
                for x in [center_x - radius, center_x + radius] {
                    if x >= before.left
                        && x < before.right
                        && unsafe { WindowFromPoint(Point { x, y }) } == hwnd
                    {
                        target = Some((x, y));
                        break;
                    }
                }
                if target.is_some() {
                    break;
                }
            }
            if target.is_some() {
                break;
            }
        }
        let (start_x, start_y) = target.ok_or("no opaque overlay pixel accepts mouse input")?;
        if unsafe { SetCursorPos(start_x, start_y) } == 0 {
            return Err("could not move pointer onto overlay".into());
        }
        // A popup menu or Settings may still own foreground activation after the
        // preceding menu clicks. Keep the actual SendInput drag as the acceptance check.
        let activated = unsafe { SetForegroundWindow(hwnd) } != 0;
        thread::sleep(Duration::from_millis(80));
        let foreground = unsafe { GetForegroundWindow() };
        let target_at_down = unsafe { WindowFromPoint(Point { x: start_x, y: start_y }) };
        send_mouse(MOUSEEVENTF_LEFTDOWN)?;
        thread::sleep(Duration::from_millis(100));
        for _ in 0..3 {
            send_mouse_event(MOUSEEVENTF_MOVE, dx / 3, dy / 3)?;
            thread::sleep(Duration::from_millis(70));
        }
        let mut cursor = Point { x: 0, y: 0 };
        if unsafe { GetCursorPos(&mut cursor) } == 0
            || (cursor.x - start_x).abs() < 10
            || (cursor.y - start_y).abs() < 8
        {
            return Err("SendInput did not move the cursor during overlay drag".into());
        }
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
        if before.left == after.left && before.top == after.top {
            let counts: Vec<_> = (0..4).map(|index|
                unsafe { SendMessageW(hwnd, WM_APP_DRAG_DIAGNOSTIC, index, 0) }).collect();
            let mut gui = GuiThreadInfo {
                size: std::mem::size_of::<GuiThreadInfo>() as u32,
                flags: 0, active: std::ptr::null_mut(), focus: std::ptr::null_mut(),
                capture: std::ptr::null_mut(), menu_owner: std::ptr::null_mut(),
                move_size: std::ptr::null_mut(), caret: std::ptr::null_mut(),
                caret_rect: Rect { left: 0, top: 0, right: 0, bottom: 0 },
            };
            let gui_available = unsafe { GetGUIThreadInfo(0, &mut gui) } != 0;
            return Err(format!(
                "unlocked overlay did not move in response to drag: ({}, {}) -> ({}, {}); nc_down={}, client_down={}, client_move={}, window_move={}; activated={activated}, foreground_overlay={}, down_target_overlay={}, gui_info={gui_available}, capture_present={}, menu_owner_present={}, gui_flags={}",
                before.left, before.top, after.left, after.top,
                counts[0], counts[1], counts[2], counts[3],
                foreground == hwnd, target_at_down == hwnd,
                !gui.capture.is_null(), !gui.menu_owner.is_null(), gui.flags,
            ));
        }
        Ok((before, after))
    }

    pub fn run() -> Result<(), String> {
        let executable = std::env::args_os()
            .nth(1)
            .ok_or("usage: overlay-smoke <path-to-system-monitor-windows.exe>")?;
        let profile =
            std::env::temp_dir().join(format!("kil0bit-rust-smoke-{}", std::process::id()));
        let settings_dir = profile.join("Kil0bitSystemMonitorRust");
        std::fs::create_dir_all(&settings_dir)
            .map_err(|error| format!("create isolated smoke profile: {error}"))?;
        std::fs::write(
            settings_dir.join("config.json"),
            br#"{"HideOnFullscreen":false,"StickToTaskbar":true,"LockPosition":false,"X":100,"Y":100}"#,
        )
        .map_err(|error| format!("write isolated smoke config: {error}"))?;
        let mut child = Command::new(executable.clone())
            .env("APPDATA", &profile)
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
            .env("APPDATA", &profile)
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
                return Err(format!(
                    "Snap to Taskbar did not restore centered taskbar position: actual y={}, expected y={}, height={}, taskbar=[{},{}]",
                    snapped_rect.top, snap_y, snapped_rect.bottom - snapped_rect.top,
                    taskbar_rect.top, taskbar_rect.bottom
                ));
            }
            if let Err(error) = click_context_item(hwnd, 2) {
                let _ = child.kill();
                let _ = child.wait();
                return Err(error);
            }
        }

        wait_for_lock_state(hwnd, false)?;
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
            return Err(format!(
                "unlocked overlay did not move in response to drag: ({}, {}) -> ({}, {})",
                unlocked_before.left, unlocked_before.top, unlocked_after.left, unlocked_after.top
            ));
        }
        if let Err(error) = click_context_item(hwnd, 0) {
            let _ = child.kill();
            let _ = child.wait();
            return Err(error);
        }
        wait_for_lock_state(hwnd, true)?;
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
        wait_for_lock_state(hwnd, false)?;
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
        let mut restarted = Command::new(executable)
            .arg("--startup")
            .env("APPDATA", &profile)
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
        let _ = std::fs::remove_dir_all(profile);
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
