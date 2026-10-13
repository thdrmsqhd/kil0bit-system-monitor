unsafe fn open_settings_window(_owner: Hwnd) {
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
        null_mut(),
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
