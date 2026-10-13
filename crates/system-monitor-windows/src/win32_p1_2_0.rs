unsafe fn open_palette_window(owner: Hwnd) {
    let existing = PALETTE_HANDLE.load(Ordering::Relaxed);
    if !existing.is_null() { ShowWindow(existing, SW_SHOW); return; }
    let class_name: Vec<u16> = "Kil0bitRustPalette\0".encode_utf16().collect();
    let title: Vec<u16> = "Section colors (blank inherits global color)\0".encode_utf16().collect();
    let instance = GetModuleHandleW(null());
    let class = WindowClass {
        style: 0, window_proc: Some(palette_window_proc), class_extra: 0,
        window_extra: 0, instance, icon: null_mut(), cursor: null_mut(),
        background: null_mut(), menu_name: null(), class_name: class_name.as_ptr(),
    };
    if RegisterClassW(&class) == 0 && GetLastError() != ERROR_CLASS_ALREADY_EXISTS { return; }
    let window = CreateWindowExW(WS_EX_TOOLWINDOW, class_name.as_ptr(), title.as_ptr(),
        WS_OVERLAPPEDWINDOW, 520, 140, 480, 570, owner, null_mut(), instance, null_mut());
    if window.is_null() { return; }
    PALETTE_HANDLE.store(window, Ordering::Relaxed);
    let config = config_lock().lock().map(|c| c.clone()).unwrap_or_default();
    let values = [
        ("Network label", config.net_label_color_hex),
        ("Network value", config.net_accent_color_hex),
        ("CPU/RAM label", config.cpu_ram_label_color_hex),
        ("CPU/RAM value", config.cpu_ram_accent_color_hex),
        ("GPU label", config.gpu_label_color_hex),
        ("GPU value", config.gpu_accent_color_hex),
        ("Disk label", config.disk_label_color_hex),
        ("Disk value", config.disk_accent_color_hex),
    ];
    for (index, (label, value)) in values.iter().enumerate() {
        let y = 20 + index as i32 * 54;
        devices_child(window, "STATIC", label, 0, 20, y, 410, 20, 0);
        let edit = devices_child(window, "EDIT", value.as_deref().unwrap_or(""), 0,
            20, y + 20, 410, 27, WS_BORDER | WS_TABSTOP);
        PALETTE_EDITS[index].store(edit, Ordering::Relaxed);
    }
    devices_child(window, "BUTTON", "Save section colors", PALETTE_SAVE,
        20, 468, 210, 34, WS_TABSTOP);
    ShowWindow(window, SW_SHOW);
    UpdateWindow(window);
}

unsafe extern "system" fn deepseek_window_proc(
    hwnd: Hwnd, message: u32, wparam: usize, lparam: isize,
) -> Lresult {
    match message {
        WM_COMMAND if (wparam & 0xffff) == DEEPSEEK_SAVE => {
            let edit = DEEPSEEK_KEY_EDIT.load(Ordering::Relaxed);
            let length = GetWindowTextLengthW(edit).clamp(0, 4096) as usize;
            let mut buffer = vec![0_u16; length + 1];
            let actual = GetWindowTextW(edit, buffer.as_mut_ptr(), buffer.len() as i32).max(0) as usize;
            let mut key = String::from_utf16_lossy(&buffer[..actual]);
            if !key.trim().is_empty() {
                let result = crate::secret_store::SecretStore::deepseek()
                    .and_then(|store| store.save(&key));
                if result.is_ok() {
                    if let Ok(mut config) = config_lock().lock() {
                        config.deepseek_enabled = true;
                        let _ = config_store::save(&config_path(), &config);
                    }
                    restart_deepseek_worker(OVERLAY_HANDLE.load(Ordering::Relaxed));
                }
            }
            key.zeroize();
            buffer.zeroize();
            let blank = [0_u16];
            SetWindowTextW(edit, blank.as_ptr());
            DestroyWindow(hwnd);
            0
        }
        WM_COMMAND if (wparam & 0xffff) == DEEPSEEK_REMOVE => {
            let _ = crate::secret_store::SecretStore::deepseek().and_then(|store| store.remove());
            if let Ok(mut config) = config_lock().lock() {
                config.deepseek_enabled = false;
                let _ = config_store::save(&config_path(), &config);
            }
            if let Some(worker) = DEEPSEEK_WORKER.get()
                .and_then(|v| v.lock().ok()).and_then(|mut v| v.take()) { drop(worker); }
            if let Some(snapshot) = DEEPSEEK_SNAPSHOT.get() {
                if let Ok(mut value) = snapshot.lock() { *value = None; }
            }
            let overlay = OVERLAY_HANDLE.load(Ordering::Relaxed);
            if !overlay.is_null() { PostMessageW(overlay, WM_APP_REFRESH, 0, 0); }
            DestroyWindow(hwnd);
            0
        }
        WM_CLOSE => { DestroyWindow(hwnd); 0 }
        WM_DESTROY => {
            DEEPSEEK_HANDLE.store(null_mut(), Ordering::Relaxed);
            DEEPSEEK_KEY_EDIT.store(null_mut(), Ordering::Relaxed);
            0
        }
        _ => DefWindowProcW(hwnd, message, wparam, lparam),
    }
}
