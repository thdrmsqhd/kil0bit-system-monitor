unsafe fn appearance_text(handle: Hwnd) -> String {
    if handle.is_null() { return String::new(); }
    let mut buffer = [0_u16; 260];
    let length = GetWindowTextW(handle, buffer.as_mut_ptr(), buffer.len() as i32).max(0) as usize;
    String::from_utf16_lossy(&buffer[..length]).trim().to_owned()
}

unsafe extern "system" fn appearance_window_proc(
    hwnd: Hwnd, message: u32, wparam: usize, lparam: isize,
) -> Lresult {
    match message {
        WM_COMMAND if (wparam & 0xffff) == APPEARANCE_PALETTE => {
            open_palette_window(hwnd);
            0
        }
        WM_COMMAND if (wparam & 0xffff) == APPEARANCE_SAVE => {
            let values: Vec<String> = APPEARANCE_EDITS.iter()
                .map(|control| appearance_text(control.load(Ordering::Relaxed))).collect();
            let checks: Vec<bool> = APPEARANCE_CHECKS.iter().map(|control| {
                SendMessageW(control.load(Ordering::Relaxed), BM_GETCHECK, 0, 0)
                    == BST_CHECKED as isize
            }).collect();
            if let Ok(mut config) = config_lock().lock() {
                let theme_changed = config.theme != values[7];
                config.accent_color_hex = values[0].clone();
                config.label_color_hex = values[1].clone();
                config.background_color_hex = values[2].clone();
                config.pod_color_hex = values[3].clone();
                config.scale_factor = values[4].parse().unwrap_or(1.0);
                config.column_spacing = values[5].parse().unwrap_or(6);
                config.font_family = values[6].clone();
                config.theme = values[7].clone();
                config.show_background = checks[0];
                config.show_pods = checks[1];
                config.is_text_bold = checks[2];
                if theme_changed {
                    let theme = config.theme.clone();
                    config.apply_theme(&theme);
                }
                config.normalize();
                let _ = config_store::save(&config_path(), &config);
            }
            let overlay = OVERLAY_HANDLE.load(Ordering::Relaxed);
            if !overlay.is_null() { PostMessageW(overlay, WM_APP_REFRESH, 0, 0); }
            DestroyWindow(hwnd);
            0
        }
        WM_CLOSE => { DestroyWindow(hwnd); 0 }
        WM_DESTROY => {
            APPEARANCE_HANDLE.store(null_mut(), Ordering::Relaxed);
            for edit in &APPEARANCE_EDITS { edit.store(null_mut(), Ordering::Relaxed); }
            for check in &APPEARANCE_CHECKS { check.store(null_mut(), Ordering::Relaxed); }
            0
        }
        _ => DefWindowProcW(hwnd, message, wparam, lparam),
    }
}

unsafe fn open_appearance_window(owner: Hwnd) {
    let existing = APPEARANCE_HANDLE.load(Ordering::Relaxed);
    if !existing.is_null() { ShowWindow(existing, SW_SHOW); return; }
    let class_name: Vec<u16> = "Kil0bitRustAppearance\0".encode_utf16().collect();
    let title: Vec<u16> = "Overlay appearance\0".encode_utf16().collect();
    let instance = GetModuleHandleW(null());
    let class = WindowClass {
        style: 0, window_proc: Some(appearance_window_proc), class_extra: 0,
        window_extra: 0, instance, icon: null_mut(), cursor: null_mut(),
        background: null_mut(), menu_name: null(), class_name: class_name.as_ptr(),
    };
    if RegisterClassW(&class) == 0 && GetLastError() != ERROR_CLASS_ALREADY_EXISTS { return; }
    let window = CreateWindowExW(WS_EX_TOOLWINDOW, class_name.as_ptr(), title.as_ptr(),
        WS_OVERLAPPEDWINDOW, 480, 120, 480, 610, owner, null_mut(), instance, null_mut());
    if window.is_null() { return; }
    APPEARANCE_HANDLE.store(window, Ordering::Relaxed);
    let config = config_lock().lock().map(|c| c.clone()).unwrap_or_default();
    let values = [
        ("Value color", config.accent_color_hex),
        ("Label color", config.label_color_hex),
        ("Background color", config.background_color_hex),
        ("Capsule color", config.pod_color_hex),
        ("Scale (0.5-2.0)", config.scale_factor.to_string()),
        ("Spacing (0-20)", config.column_spacing.to_string()),
        ("Font family", config.font_family),
        ("Theme preset", config.theme),
    ];
    for (index, (label, value)) in values.iter().enumerate() {
        let y = 20 + index as i32 * 49;
        devices_child(window, "STATIC", label, 0, 20, y, 410, 20, 0);
        let edit = if index == 7 {
            let combo = devices_child(window, "COMBOBOX", "", 0, 20, y + 19, 410, 265,
                WS_TABSTOP | 0x0003 | 0x0040);
            let presets = ["Default", "Cyberpunk", "Matrix", "Stealth", "Synthwave",
                "Midnight Gold", "Frost", "Inferno", "Toxic", "Nordic"];
            for name in presets {
                let encoded: Vec<u16> = name.encode_utf16().chain(std::iter::once(0)).collect();
                SendMessageW(combo, 0x0143, 0, encoded.as_ptr() as isize);
            }
            let selected = presets.iter().position(|name| *name == value).unwrap_or(0);
            SendMessageW(combo, 0x014e, selected, 0);
            combo
        } else {
            devices_child(window, "EDIT", value, 0, 20, y + 19, 410, 27,
                WS_BORDER | WS_TABSTOP)
        };
        APPEARANCE_EDITS[index].store(edit, Ordering::Relaxed);
    }
    let config = config_lock().lock().map(|c| c.clone()).unwrap_or_default();
    for (index, (caption, checked)) in [
        ("Background", config.show_background),
        ("Capsules", config.show_pods),
        ("Bold text", config.is_text_bold),
    ].iter().enumerate() {
        let control = devices_child(window, "BUTTON", caption, 0,
            20 + index as i32 * 140, 425, 135, 26, WS_TABSTOP | 0x0003);
        APPEARANCE_CHECKS[index].store(control, Ordering::Relaxed);
        if *checked { SendMessageW(control, BM_SETCHECK, BST_CHECKED, 0); }
    }
    devices_child(window, "BUTTON", "Save appearance", APPEARANCE_SAVE,
        20, 480, 200, 36, WS_TABSTOP);
    devices_child(window, "BUTTON", "Section colors...", APPEARANCE_PALETTE,
        235, 480, 195, 36, WS_TABSTOP);
    ShowWindow(window, SW_SHOW);
    UpdateWindow(window);
}

unsafe extern "system" fn palette_window_proc(
    hwnd: Hwnd, message: u32, wparam: usize, lparam: isize,
) -> Lresult {
    match message {
        WM_COMMAND if (wparam & 0xffff) == PALETTE_SAVE => {
            let values: Vec<Option<String>> = PALETTE_EDITS.iter().map(|edit| {
                let text = appearance_text(edit.load(Ordering::Relaxed));
                if text.is_empty() { None } else { Some(text) }
            }).collect();
            if let Ok(mut config) = config_lock().lock() {
                config.net_label_color_hex = values[0].clone();
                config.net_accent_color_hex = values[1].clone();
                config.cpu_ram_label_color_hex = values[2].clone();
                config.cpu_ram_accent_color_hex = values[3].clone();
                config.gpu_label_color_hex = values[4].clone();
                config.gpu_accent_color_hex = values[5].clone();
                config.disk_label_color_hex = values[6].clone();
                config.disk_accent_color_hex = values[7].clone();
                let _ = config_store::save(&config_path(), &config);
            }
            let overlay = OVERLAY_HANDLE.load(Ordering::Relaxed);
            if !overlay.is_null() { PostMessageW(overlay, WM_APP_REFRESH, 0, 0); }
            DestroyWindow(hwnd);
            0
        }
        WM_CLOSE => { DestroyWindow(hwnd); 0 }
        WM_DESTROY => {
            PALETTE_HANDLE.store(null_mut(), Ordering::Relaxed);
            for edit in &PALETTE_EDITS { edit.store(null_mut(), Ordering::Relaxed); }
            0
        }
        _ => DefWindowProcW(hwnd, message, wparam, lparam),
    }
}
