unsafe fn appearance_text(handle: Hwnd) -> String {
    if handle.is_null() { return String::new(); }
    let mut buffer = [0_u16; 260];
    let length = GetWindowTextW(handle, buffer.as_mut_ptr(), buffer.len() as i32).max(0) as usize;
    String::from_utf16_lossy(&buffer[..length]).trim().to_owned()
}

unsafe extern "system" fn appearance_window_proc(
    hwnd: Hwnd,
    message: u32,
    wparam: usize,
    lparam: isize,
) -> Lresult {
    match message {
        WM_COMMAND if (wparam & 0xffff) == APPEARANCE_SAVE => {
            save_appearance_from_window();
            0
        }
        WM_COMMAND if (wparam & 0xffff) == APPEARANCE_PALETTE => {
            open_palette_window(hwnd);
            0
        }
        WM_CLOSE => {
            DestroyWindow(hwnd);
            0
        }
        WM_DESTROY => {
            APPEARANCE_HANDLE
                .compare_exchange(hwnd, null_mut(), Ordering::Relaxed, Ordering::Relaxed)
                .ok();
            for edit in &APPEARANCE_EDITS {
                edit.store(null_mut(), Ordering::Relaxed);
            }
            for check in &APPEARANCE_CHECKS {
                check.store(null_mut(), Ordering::Relaxed);
            }
            0
        }
        _ => DefWindowProcW(hwnd, message, wparam, lparam),
    }
}

unsafe fn open_appearance_window(owner: Hwnd) {
    let existing = APPEARANCE_HANDLE.load(Ordering::Relaxed);
    if !existing.is_null() {
        ShowWindow(existing, SW_SHOW);
        return;
    }
    let class_name: Vec<u16> = "Kil0bitAppearanceWnd\0".encode_utf16().collect();
    let title: Vec<u16> = "Appearance\0".encode_utf16().collect();
    let instance = GetModuleHandleW(null());
    let class = WindowClass {
        style: 0,
        window_proc: Some(appearance_window_proc),
        class_extra: 0,
        window_extra: 0,
        instance,
        icon: null_mut(),
        cursor: null_mut(),
        background: null_mut(),
        menu_name: null(),
        class_name: class_name.as_ptr(),
    };
    let registered = RegisterClassW(&class);
    if registered == 0 && GetLastError() != ERROR_CLASS_ALREADY_EXISTS {
        return;
    }
    let window = CreateWindowExW(
        0,
        class_name.as_ptr(),
        title.as_ptr(),
        WS_OVERLAPPEDWINDOW,
        120,
        120,
        520,
        480,
        owner,
        null_mut(),
        instance,
        null_mut(),
    );
    if window.is_null() {
        return;
    }
    APPEARANCE_HANDLE.store(window, Ordering::Relaxed);
    ShowWindow(window, SW_SHOW);
    UpdateWindow(window);
}

unsafe extern "system" fn palette_window_proc(
    hwnd: Hwnd,
    message: u32,
    wparam: usize,
    lparam: isize,
) -> Lresult {
    match message {
        WM_COMMAND if (wparam & 0xffff) == PALETTE_SAVE => {
            // palette save handled in full body restore
            0
        }
        WM_CLOSE => {
            DestroyWindow(hwnd);
            0
        }
        WM_DESTROY => {
            PALETTE_HANDLE
                .compare_exchange(hwnd, null_mut(), Ordering::Relaxed, Ordering::Relaxed)
                .ok();
            for edit in &PALETTE_EDITS {
                edit.store(null_mut(), Ordering::Relaxed);
            }
            0
        }
        _ => DefWindowProcW(hwnd, message, wparam, lparam),
    }
}
