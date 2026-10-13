unsafe extern "system" fn window_proc(
    hwnd: Hwnd,
    message: u32,
    wparam: usize,
    lparam: isize,
) -> Lresult {
    match message {
        WM_NCLBUTTONDOWN => { DRAG_NC_DOWN.fetch_add(1, Ordering::Relaxed); }
        WM_LBUTTONDOWN => { DRAG_CLIENT_DOWN.fetch_add(1, Ordering::Relaxed); }
        WM_MOUSEMOVE => { DRAG_CLIENT_MOVE.fetch_add(1, Ordering::Relaxed); }
        WM_MOVE => { DRAG_WINDOW_MOVE.fetch_add(1, Ordering::Relaxed); }
        _ => {}
    }
    match message {
        WM_APP_DRAG_DIAGNOSTIC => match wparam {
            0 => DRAG_NC_DOWN.load(Ordering::Relaxed) as isize,
            1 => DRAG_CLIENT_DOWN.load(Ordering::Relaxed) as isize,
            2 => DRAG_CLIENT_MOVE.load(Ordering::Relaxed) as isize,
            3 => DRAG_WINDOW_MOVE.load(Ordering::Relaxed) as isize,
            _ => 0,
        },
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
        WM_MOUSEMOVE | WM_NCMOUSEMOVE => {
            if let Ok(drag) = DRAG_ORIGIN.lock() {
                if let Some((cursor_start, window_start)) = *drag {
                    let mut cursor = Point { x: 0, y: 0 };
                    if GetCursorPos(&mut cursor) != 0 {
                        SetWindowPos(
                            hwnd,
                            null_mut(),
                            window_start.x + cursor.x - cursor_start.x,
                            window_start.y + cursor.y - cursor_start.y,
                            0,
                            0,
                            SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE,
                        );
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
        WM_APP_REFRESH => {
            let (scaled, width, height) = render_current_bitmap();
            if SNAP_TO_TASKBAR.load(Ordering::Relaxed) {
                align_to_taskbar(hwnd, height);
            }
            let _ = present_bitmap(hwnd, &scaled, width, height);
            0
        }
        WM_APP_SHOW_SETTINGS => {
            open_settings_window(hwnd);
            0
        }
        WM_TIMER if wparam == FADE_TIMER_ID => {
            handle_fade_timer(hwnd);
            0
        }
        WM_TIMER => {
            update_fullscreen_hide(hwnd);
            0
        }
        WM_COMMAND => {
            match wparam & 0xffff {
                MENU_TOGGLE_LOCK => {
                    let locked = !POSITION_LOCKED.load(Ordering::Relaxed);
                    POSITION_LOCKED.store(locked, Ordering::Relaxed);
                    if let Ok(mut config) = config_lock().lock() {
                        config.position_locked = locked;
                        let _ = config_store::save(&config_path(), &config);
                    }
                }
                MENU_TOGGLE_SNAP => {
                    let snap = !SNAP_TO_TASKBAR.load(Ordering::Relaxed);
                    SNAP_TO_TASKBAR.store(snap, Ordering::Relaxed);
                    if snap {
                        let (_, _, height) = render_current_bitmap();
                        align_to_taskbar(hwnd, height);
                        attach_to_taskbar(hwnd, height);
                    } else {
                        detach_from_taskbar(hwnd);
                    }
                    if let Ok(mut config) = config_lock().lock() {
                        config.snap_to_taskbar = snap;
                        let _ = config_store::save(&config_path(), &config);
                    }
                }
                MENU_TOGGLE_TOPMOST => {
                    if let Ok(mut config) = config_lock().lock() {
                        config.always_on_top = !config.always_on_top;
                        let top = config.always_on_top;
                        let _ = config_store::save(&config_path(), &config);
                        SetWindowPos(
                            hwnd,
                            if top { HWND_TOPMOST } else { HWND_NOTOPMOST },
                            0, 0, 0, 0,
                            SWP_NOSIZE | SWP_NOACTIVATE | 0x0002,
                        );
                    }
                }
                MENU_TOGGLE_FULLSCREEN => {
                    if let Ok(mut config) = config_lock().lock() {
                        config.hide_on_fullscreen = !config.hide_on_fullscreen;
                        let _ = config_store::save(&config_path(), &config);
                    }
                }
                MENU_SETTINGS => open_settings_window(hwnd),
                MENU_TASK_MANAGER => {
                    let file: Vec<u16> = "taskmgr.exe\0".encode_utf16().collect();
                    ShellExecuteW(null_mut(), null(), file.as_ptr(), null(), null(), SW_SHOW);
                }
                MENU_ABOUT => {
                    let text: Vec<u16> = "kil0bit System Monitor (Rust port)\0".encode_utf16().collect();
                    let caption: Vec<u16> = "About\0".encode_utf16().collect();
                    MessageBoxW(hwnd, text.as_ptr(), caption.as_ptr(), 0);
                }
                MENU_EXIT => {
                    DestroyWindow(hwnd);
                }
                _ => {}
            }
            0
        }
        WM_CLOSE => {
            DestroyWindow(hwnd);
            0
        }
        WM_DESTROY => {
            remove_appbar(hwnd);
            if let Some(worker) = TELEMETRY_WORKER.get().and_then(|v| v.lock().ok()).and_then(|mut v| v.take()) {
                drop(worker);
            }
            if let Some(worker) = AI_WORKER.get().and_then(|v| v.lock().ok()).and_then(|mut v| v.take()) {
                drop(worker);
            }
            if let Some(worker) = DEEPSEEK_WORKER.get().and_then(|v| v.lock().ok()).and_then(|mut v| v.take()) {
                drop(worker);
            }
            OVERLAY_HANDLE.store(null_mut(), Ordering::Relaxed);
            PostQuitMessage(0);
            0
        }
        WM_MOVE if !SNAP_TO_TASKBAR.load(Ordering::Relaxed) => {
            let mut rect = Rect { left: 0, top: 0, right: 0, bottom: 0 };
            if !POSITION_LOCKED.load(Ordering::Relaxed) && GetWindowRect(hwnd, &mut rect) != 0 {
                FREE_X.store(rect.left, Ordering::Relaxed);
                FREE_Y.store(rect.top, Ordering::Relaxed);
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
        WM_RBUTTONUP | WM_NCRBUTTONUP => {
            show_context_menu(hwnd);
            0
        }
        _ => DefWindowProcW(hwnd, message, wparam, lparam),
    }
}
