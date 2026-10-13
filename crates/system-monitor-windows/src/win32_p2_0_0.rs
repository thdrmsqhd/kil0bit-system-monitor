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
    let fullscreen_text: Vec<u16> = "Hide in Fullscreen\0".encode_utf16().collect();
    AppendMenuW(menu,
        MF_STRING | if config.hide_on_fullscreen { MF_CHECKED } else { 0 },
        MENU_TOGGLE_FULLSCREEN as usize, fullscreen_text.as_ptr());
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
                    let (_, _, height) = render_current_bitmap();
                    attach_to_taskbar(hwnd, height);
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
            MENU_TOGGLE_FULLSCREEN => {
                if let Ok(mut config) = config_lock().lock() {
                    config.hide_on_fullscreen = !config.hide_on_fullscreen;
                    let _ = config_store::save(&config_path(), &config);
                }
                update_fullscreen_visibility(hwnd);
                refresh_overlay(hwnd);
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
