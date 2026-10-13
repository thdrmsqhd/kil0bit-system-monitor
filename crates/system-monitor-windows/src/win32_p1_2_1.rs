unsafe fn open_deepseek_window(owner: Hwnd) {
    let existing = DEEPSEEK_HANDLE.load(Ordering::Relaxed);
    if !existing.is_null() { ShowWindow(existing, SW_SHOW); return; }
    let class_name: Vec<u16> = "Kil0bitRustDeepSeek\0".encode_utf16().collect();
    let title: Vec<u16> = "DeepSeek balance\0".encode_utf16().collect();
    let instance = GetModuleHandleW(null());
    let class = WindowClass {
        style: 0, window_proc: Some(deepseek_window_proc), class_extra: 0,
        window_extra: 0, instance, icon: null_mut(), cursor: null_mut(),
        background: null_mut(), menu_name: null(), class_name: class_name.as_ptr(),
    };
    if RegisterClassW(&class) == 0 && GetLastError() != ERROR_CLASS_ALREADY_EXISTS { return; }
    let window = CreateWindowExW(WS_EX_TOOLWINDOW, class_name.as_ptr(), title.as_ptr(),
        WS_OVERLAPPEDWINDOW, 420, 230, 460, 235, owner, null_mut(), instance, null_mut());
    if window.is_null() { return; }
    DEEPSEEK_HANDLE.store(window, Ordering::Relaxed);
    let status = if crate::secret_store::SecretStore::deepseek().is_ok_and(|s| s.exists()) {
        "API key saved for this Windows user" } else { "Enter a DeepSeek API key" };
    devices_child(window, "STATIC", status, 0, 20, 20, 410, 25, 0);
    let edit = devices_child(window, "EDIT", "", 0, 20, 55, 410, 28,
        WS_BORDER | WS_TABSTOP | ES_PASSWORD);
    DEEPSEEK_KEY_EDIT.store(edit, Ordering::Relaxed);
    devices_child(window, "BUTTON", "Save key", DEEPSEEK_SAVE,
        20, 105, 150, 34, WS_TABSTOP);
    devices_child(window, "BUTTON", "Remove key", DEEPSEEK_REMOVE,
        190, 105, 150, 34, WS_TABSTOP);
    ShowWindow(window, SW_SHOW);
    UpdateWindow(window);
}

fn start_deepseek_worker(hwnd: Hwnd) {
    let config = config_lock().lock().map(|c| c.clone()).unwrap_or_default();
    if hwnd.is_null() || !config.deepseek_enabled { return; }
    let Ok(store) = crate::secret_store::SecretStore::deepseek() else { return; };
    if !store.exists() { return; }
    let handle = hwnd as usize;
    let worker = super::ai_usage::DeepSeekWorker::start(
        config.ai_poll_interval_seconds, store, move |event| {
            if let Some(state) = DEEPSEEK_SNAPSHOT.get() {
                if let Ok(mut current) = state.lock() {
                    match event {
                        Ok(snapshot) => *current = Some(snapshot),
                        Err(_) => {
                            if let Some(snapshot) = current.as_mut() { snapshot.stale = true; }
                        }
                    }
                }
            }
            unsafe { PostMessageW(handle as Hwnd, WM_APP_REFRESH, 0, 0); }
        });
    if let Some(slot) = DEEPSEEK_WORKER.get() {
        if let Ok(mut slot) = slot.lock() { *slot = Some(worker); }
    }
}

fn restart_deepseek_worker(hwnd: Hwnd) {
    if let Some(worker) = DEEPSEEK_WORKER.get()
        .and_then(|v| v.lock().ok()).and_then(|mut v| v.take()) { drop(worker); }
    start_deepseek_worker(hwnd);
}

unsafe fn save_opencode_key_from_settings() {
    let edit = AI_KEY_EDIT.load(Ordering::Relaxed);
    if edit.is_null() {
        return;
    }
    let length = GetWindowTextLengthW(edit).clamp(0, 4096) as usize;
    let mut buffer = vec![0_u16; length + 1];
    let actual = GetWindowTextW(edit, buffer.as_mut_ptr(), buffer.len() as i32).max(0) as usize;
    let mut key = String::from_utf16_lossy(&buffer[..actual]);
    if key.trim().is_empty() {
        key.zeroize();
        buffer.zeroize();
        set_ai_key_status("Enter an OpenCode Go API key first");
        return;
    }
    let result = crate::secret_store::SecretStore::opencode().and_then(|store| store.save(&key));
    key.zeroize();
    buffer.zeroize();
    let empty: Vec<u16> = "\0".encode_utf16().collect();
    SetWindowTextW(edit, empty.as_ptr());
    match result {
        Ok(()) => {
            if let Ok(mut config) = config_lock().lock() {
                config.opencode_enabled = true;
                config.codex_enabled = false;
                let _ = config_store::save(&config_path(), &config);
            }
            set_ai_key_status("OpenCode key saved for this Windows user");
            if let Some(worker) = AI_WORKER.get()
                .and_then(|v| v.lock().ok()).and_then(|mut v| v.take()) { drop(worker); }
            start_ai_worker(OVERLAY_HANDLE.load(Ordering::Relaxed));
        }
        Err(_) => set_ai_key_status("Could not save OpenCode key"),
    }
}
