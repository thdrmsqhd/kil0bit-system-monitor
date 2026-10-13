unsafe extern "system" fn settings_window_proc(
    hwnd: Hwnd,
    message: u32,
    wparam: usize,
    lparam: isize,
) -> Lresult {
    match message {
        WM_COMMAND if (wparam & 0xffff) == SETTINGS_TOGGLE_ACCENT => {
            let enabled = !ALTERNATE_ACCENT.load(Ordering::Relaxed);
            ALTERNATE_ACCENT.store(enabled, Ordering::Relaxed);
            let overlay = OVERLAY_HANDLE.load(Ordering::Relaxed);
            if !overlay.is_null() {
                PostMessageW(overlay, WM_APP_REFRESH, 0, 0);
            }
            0
        }
        WM_COMMAND if (wparam & 0xffff) == SETTINGS_SAVE_OPENCODE_KEY => {
            save_opencode_key_from_settings();
            0
        }
        WM_COMMAND if (wparam & 0xffff) == SETTINGS_REMOVE_OPENCODE_KEY => {
            let result =
                crate::secret_store::SecretStore::opencode().and_then(|store| store.remove());
            if result.is_ok() {
                let codex_active = config_lock().lock()
                    .map(|c| c.codex_enabled).unwrap_or(false);
                if let Ok(mut config) = config_lock().lock() {
                    config.opencode_enabled = false;
                    let _ = config_store::save(&config_path(), &config);
                }
                if !codex_active {
                if let Some(worker) = AI_WORKER
                    .get()
                    .and_then(|v| v.lock().ok())
                    .and_then(|mut v| v.take())
                {
                    drop(worker);
                }
                if let Some(snapshot) = AI_SNAPSHOT.get() {
                    if let Ok(mut value) = snapshot.lock() {
                        *value = None;
                    }
                }
                }
                set_ai_key_status("OpenCode key removed");
            } else {
                set_ai_key_status("Could not remove OpenCode key");
            }
            let overlay = OVERLAY_HANDLE.load(Ordering::Relaxed);
            if !overlay.is_null() {
                PostMessageW(overlay, WM_APP_REFRESH, 0, 0);
            }
            0
        }
        WM_COMMAND if (wparam & 0xffff) == SETTINGS_TOGGLE_STARTUP => {
            let checkbox = STARTUP_CHECKBOX.load(Ordering::Relaxed);
            if !checkbox.is_null() {
                let enabled = SendMessageW(checkbox, BM_GETCHECK, 0, 0) == BST_CHECKED as isize;
                if let Ok(mut config) = config_lock().lock() {
                    config.launch_on_startup = enabled;
                    let saved = config_store::save(&config_path(), &config).is_ok();
                    let registered = set_startup_registration(enabled).is_ok();
                    set_startup_status(if saved && registered {
                        "Startup setting saved"
                    } else {
                        "Could not update startup setting"
                    });
                }
            }
            0
        }
        WM_COMMAND
            if ((SETTINGS_AI_ROLLING..=SETTINGS_AI_INTERVAL).contains(&(wparam & 0xffff))
                || (wparam & 0xffff) == 1010) =>
        {
            save_ai_options_from_settings();
            0
        }
        WM_COMMAND if (SETTINGS_PORT_FIRST..=SETTINGS_PORT_LAST).contains(&(wparam & 0xffff)) => {
            save_port_options_from_settings(hwnd);
            0
        }
        WM_COMMAND if (wparam & 0xffff) == SETTINGS_DEVICES => {
            open_devices_window(hwnd);
            0
        }
        WM_COMMAND if (wparam & 0xffff) == SETTINGS_DEEPSEEK => {
            open_deepseek_window(hwnd);
            0
        }
        WM_COMMAND if (wparam & 0xffff) == SETTINGS_MAINTENANCE => {
            open_maintenance_window(hwnd);
            0
        }
        WM_CLOSE => {
            DestroyWindow(hwnd);
            0
        }
        WM_DESTROY => {
            let devices = DEVICES_HANDLE.swap(null_mut(), Ordering::Relaxed);
            if !devices.is_null() { DestroyWindow(devices); }
            let deepseek = DEEPSEEK_HANDLE.swap(null_mut(), Ordering::Relaxed);
            if !deepseek.is_null() { DestroyWindow(deepseek); }
            let maintenance = MAINTENANCE_HANDLE.swap(null_mut(), Ordering::Relaxed);
            if !maintenance.is_null() { DestroyWindow(maintenance); }
            SETTINGS_HANDLE
                .compare_exchange(hwnd, null_mut(), Ordering::Relaxed, Ordering::Relaxed)
                .ok();
            AI_KEY_EDIT.store(null_mut(), Ordering::Relaxed);
            AI_KEY_STATUS.store(null_mut(), Ordering::Relaxed);
            STARTUP_CHECKBOX.store(null_mut(), Ordering::Relaxed);
            STARTUP_STATUS.store(null_mut(), Ordering::Relaxed);
            for control in &AI_OPTION_CONTROLS {
                control.store(null_mut(), Ordering::Relaxed);
            }
            AI_INTERVAL_COMBO.store(null_mut(), Ordering::Relaxed);
            for control in &PORT_OPTION_CONTROLS {
                control.store(null_mut(), Ordering::Relaxed);
            }
            0
        }
        _ => DefWindowProcW(hwnd, message, wparam, lparam),
    }
}
