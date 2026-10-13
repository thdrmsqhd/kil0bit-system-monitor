unsafe fn save_port_options_from_settings(_settings_hwnd: Hwnd) {
    let checked: Vec<bool> = PORT_OPTION_CONTROLS
        .iter()
        .map(|control| {
            let handle = control.load(Ordering::Relaxed);
            !handle.is_null() && SendMessageW(handle, BM_GETCHECK, 0, 0) == BST_CHECKED as isize
        })
        .collect();
    if checked.len() != 12 {
        return;
    }
    let (old_snap, old_topmost) = config_lock()
        .lock()
        .map(|c| (c.stick_to_taskbar, c.always_on_top))
        .unwrap_or((true, true));
    if let Ok(mut config) = config_lock().lock() {
        config.show_cpu = checked[0];
        config.show_ram = checked[1];
        config.show_gpu = checked[2];
        config.show_temp = checked[3];
        config.show_disk = checked[4];
        config.show_net_up = checked[5];
        config.show_net_down = checked[6];
        config.show_overlay = checked[7];
        config.lock_position = checked[8];
        config.stick_to_taskbar = checked[9];
        config.always_on_top = checked[10];
        config.display_style = if checked[11] { "Compact" } else { "Text" }.into();
        let _ = config_store::save(&config_path(), &config);
    }
    POSITION_LOCKED.store(checked[8], Ordering::Relaxed);
    let overlay = OVERLAY_HANDLE.load(Ordering::Relaxed);
    if overlay.is_null() {
        return;
    }
    if old_snap != checked[9] {
        if checked[9] {
            attach_to_taskbar(overlay, 52);
        } else {
            detach_from_taskbar(overlay);
        }
    }
    if old_topmost != checked[10] {
        SetWindowPos(
            overlay,
            if checked[10] {
                HWND_TOPMOST
            } else {
                HWND_NOTOPMOST
            },
            0,
            0,
            0,
            0,
            SWP_NOSIZE | SWP_NOACTIVATE | 0x0002,
        );
    }
    if checked[7] {
        ShowWindow(overlay, SW_SHOWNOACTIVATE);
        refresh_overlay(overlay);
    } else {
        ShowWindow(overlay, 0);
    }
}
