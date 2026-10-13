unsafe fn confirm_maintenance(hwnd: Hwnd, question: &str) -> bool {
    let wide: Vec<u16> = question.encode_utf16().chain(Some(0)).collect();
    let title: Vec<u16> = "Confirm settings change\0".encode_utf16().collect();
    MessageBoxW(hwnd, wide.as_ptr(), title.as_ptr(), 4 | 0x20) == 6
}

unsafe fn maintenance_refresh() {
    let overlay = OVERLAY_HANDLE.load(Ordering::Relaxed);
    if !overlay.is_null() { PostMessageW(overlay, WM_APP_REFRESH, 0, 0); }
    let settings = SETTINGS_HANDLE.load(Ordering::Relaxed);
    if !settings.is_null() {
        DestroyWindow(settings);
        if !overlay.is_null() { open_settings_window(overlay); }
    }
}

unsafe fn apply_loaded_config() {
    let overlay = OVERLAY_HANDLE.load(Ordering::Relaxed);
    if overlay.is_null() { return; }
    let config = config_lock().lock().map(|c| c.clone()).unwrap_or_default();
    if config.stick_to_taskbar {
        let mut rect = Rect { left: 0, top: 0, right: 0, bottom: 0 };
        let height = if GetWindowRect(overlay, &mut rect) != 0 {
            (rect.bottom - rect.top).max(1)
        } else { 52 };
        attach_to_taskbar(overlay, height);
    } else {
        detach_from_taskbar(overlay);
    }
    SetWindowPos(overlay, if config.always_on_top { HWND_TOPMOST } else { HWND_NOTOPMOST },
        0, 0, 0, 0, SWP_NOSIZE | SWP_NOACTIVATE | 0x0002);
    ShowWindow(overlay, if config.show_overlay { SW_SHOWNOACTIVATE } else { 0 });
    if let Some(worker) = AI_WORKER.get()
        .and_then(|v| v.lock().ok()).and_then(|mut v| v.take()) { drop(worker); }
    if let Some(worker) = DEEPSEEK_WORKER.get()
        .and_then(|v| v.lock().ok()).and_then(|mut v| v.take()) { drop(worker); }
    if let Some(snapshot) = AI_SNAPSHOT.get() {
        if let Ok(mut value) = snapshot.lock() { *value = None; }
    }
    if let Some(snapshot) = DEEPSEEK_SNAPSHOT.get() {
        if let Ok(mut value) = snapshot.lock() { *value = None; }
    }
    start_ai_worker(overlay);
    start_deepseek_worker(overlay);
}

unsafe fn open_maintenance_window(owner: Hwnd) {
    let existing = MAINTENANCE_HANDLE.load(Ordering::Relaxed);
    if !existing.is_null() { ShowWindow(existing, SW_SHOW); return; }
    let class_name: Vec<u16> = "Kil0bitRustMaintenance\0".encode_utf16().collect();
    let title: Vec<u16> = "Reset and import\0".encode_utf16().collect();
    let instance = GetModuleHandleW(null());
    let class = WindowClass {
        style: 0, window_proc: Some(maintenance_window_proc), class_extra: 0,
        window_extra: 0, instance, icon: null_mut(), cursor: null_mut(),
        background: null_mut(), menu_name: null(), class_name: class_name.as_ptr(),
    };
    if RegisterClassW(&class) == 0 && GetLastError() != ERROR_CLASS_ALREADY_EXISTS { return; }
    let window = CreateWindowExW(WS_EX_TOOLWINDOW, class_name.as_ptr(), title.as_ptr(),
        WS_OVERLAPPEDWINDOW, 430, 230, 460, 350, owner, null_mut(), instance, null_mut());
    if window.is_null() { return; }
    MAINTENANCE_HANDLE.store(window, Ordering::Relaxed);
    devices_child(window, "BUTTON", "Reset appearance", MAINTENANCE_RESET_APPEARANCE,
        25, 25, 385, 40, WS_TABSTOP);
    devices_child(window, "BUTTON", "Reset all Rust settings", MAINTENANCE_RESET_ALL,
        25, 80, 385, 40, WS_TABSTOP);
    devices_child(window, "BUTTON", "Import legacy settings (read-only)", MAINTENANCE_IMPORT_LEGACY,
        25, 135, 385, 40, WS_TABSTOP);
    devices_child(window, "BUTTON", "Restore previous Rust settings", MAINTENANCE_RESTORE_BACKUP,
        25, 190, 385, 40, WS_TABSTOP);
    ShowWindow(window, SW_SHOW);
    UpdateWindow(window);
}

unsafe extern "system" fn devices_window_proc(
    hwnd: Hwnd, message: u32, wparam: usize, lparam: isize,
) -> Lresult {
    match message {
        WM_COMMAND if (wparam & 0xffff) == DEVICES_SAVE => {
            let combo = NETWORK_COMBO.load(Ordering::Relaxed);
            let list = DISK_LIST.load(Ordering::Relaxed);
            let edit = GPU_INDEX_EDIT.load(Ordering::Relaxed);
            let gpu_vendor_index = SendMessageW(GPU_VENDOR_COMBO.load(Ordering::Relaxed),
                0x0147, 0, 0).max(0) as usize;
            let gpu_vendor = GPU_VENDOR_CHOICES.get()
                .and_then(|choices| choices.lock().ok())
                .and_then(|choices| choices.get(gpu_vendor_index).cloned())
                .unwrap_or_else(|| "Default".into());
            let network_index = SendMessageW(combo, 0x0147, 0, 0).max(0) as usize;
            let network = NETWORK_CHOICES.get()
                .and_then(|choices| choices.lock().ok())
                .and_then(|choices| choices.get(network_index).cloned())
                .unwrap_or_else(|| "Default".into());
            let mut disks = Vec::new();
            if let Some(choices) = DISK_CHOICES.get().and_then(|v| v.lock().ok()) {
                for (index, name) in choices.iter().enumerate() {
                    if SendMessageW(list, 0x0187, index, 0) > 0 {
                        disks.push(name.clone());
                    }
                }
            }
            let mut text = [0_u16; 12];
            let length = GetWindowTextW(edit, text.as_mut_ptr(), text.len() as i32).max(0) as usize;
            let gpu_index = String::from_utf16_lossy(&text[..length]).trim().parse::<u32>()
                .unwrap_or(0).min(15);
            let update_index = SendMessageW(UPDATE_COMBO.load(Ordering::Relaxed),
                0x0147, 0, 0).max(0) as usize;
            let update_interval = [500, 1000, 2000, 5000]
                .get(update_index).copied().unwrap_or(1000);
            let checked = |index: usize| SendMessageW(
                DEVICE_CHECKS[index].load(Ordering::Relaxed), BM_GETCHECK, 0, 0)
                == BST_CHECKED as isize;
            if let Ok(mut config) = config_lock().lock() {
                config.network_adapter = network;
                config.selected_disks = if disks.is_empty() { "None".into() }
                    else { disks.join(";") };
                config.gpu_index = gpu_index;
                config.gpu_adapter = gpu_vendor;
                config.update_interval_ms = update_interval;
                config.hide_on_fullscreen = checked(0);
                config.show_disk_speed = checked(1);
                let _ = config_store::save(&config_path(), &config);
            }
            let overlay = OVERLAY_HANDLE.load(Ordering::Relaxed);
            if !overlay.is_null() { PostMessageW(overlay, WM_APP_REFRESH, 0, 0); }
            DestroyWindow(hwnd);
            0
        }
        WM_COMMAND if (wparam & 0xffff) == DEVICES_APPEARANCE => {
            open_appearance_window(hwnd);
            0
        }
        WM_CLOSE => { DestroyWindow(hwnd); 0 }
        WM_DESTROY => {
            let appearance = APPEARANCE_HANDLE.swap(null_mut(), Ordering::Relaxed);
            if !appearance.is_null() { DestroyWindow(appearance); }
            DEVICES_HANDLE.store(null_mut(), Ordering::Relaxed);
            NETWORK_COMBO.store(null_mut(), Ordering::Relaxed);
            DISK_LIST.store(null_mut(), Ordering::Relaxed);
            GPU_INDEX_EDIT.store(null_mut(), Ordering::Relaxed);
            GPU_VENDOR_COMBO.store(null_mut(), Ordering::Relaxed);
            UPDATE_COMBO.store(null_mut(), Ordering::Relaxed);
            for control in &DEVICE_CHECKS { control.store(null_mut(), Ordering::Relaxed); }
            0
        }
        _ => DefWindowProcW(hwnd, message, wparam, lparam),
    }
}

#[allow(clippy::too_many_arguments)]
