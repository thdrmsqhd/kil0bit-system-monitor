unsafe extern "system" fn maintenance_window_proc(
    hwnd: Hwnd, message: u32, wparam: usize, lparam: isize,
) -> Lresult {
    match message {
        WM_COMMAND if (wparam & 0xffff) == MAINTENANCE_RESET_APPEARANCE => {
            if confirm_maintenance(hwnd, "Reset appearance settings?") {
                let defaults = AppConfig::default();
                if let Ok(mut config) = config_lock().lock() {
                    config.display_style = defaults.display_style;
                    config.font_family = defaults.font_family;
                    config.accent_color_hex = defaults.accent_color_hex;
                    config.label_color_hex = defaults.label_color_hex;
                    config.background_color_hex = defaults.background_color_hex;
                    config.pod_color_hex = defaults.pod_color_hex;
                    config.scale_factor = defaults.scale_factor;
                    config.column_spacing = defaults.column_spacing;
                    config.is_text_bold = defaults.is_text_bold;
                    config.show_pods = defaults.show_pods;
                    config.show_background = defaults.show_background;
                    config.theme = defaults.theme;
                    config.net_label_color_hex = None;
                    config.cpu_ram_label_color_hex = None;
                    config.gpu_label_color_hex = None;
                    config.disk_label_color_hex = None;
                    config.net_accent_color_hex = None;
                    config.cpu_ram_accent_color_hex = None;
                    config.gpu_accent_color_hex = None;
                    config.disk_accent_color_hex = None;
                    let _ = config_store::save(&config_path(), &config);
                }
                maintenance_refresh();
            }
            0
        }
        WM_COMMAND if (wparam & 0xffff) == MAINTENANCE_RESET_ALL => {
            if confirm_maintenance(hwnd, "Reset all Rust monitor settings?") {
                if let Ok(mut config) = config_lock().lock() {
                    *config = AppConfig::default();
                    let _ = config_store::save(&config_path(), &config);
                }
                load_config();
                apply_loaded_config();
                maintenance_refresh();
            }
            0
        }
        WM_COMMAND if (wparam & 0xffff) == MAINTENANCE_IMPORT_LEGACY => {
            let legacy = std::env::var_os("APPDATA")
                .map(std::path::PathBuf::from)
                .map(|root| root.join("kil0bit-system-monitor").join("config.json"));
            let imported = legacy.as_deref()
                .and_then(|path| config_store::import_legacy_with_report(path).ok());
            let result = if let Some((config, unsupported)) = imported {
                if let Ok(mut current) = config_lock().lock() {
                    *current = config;
                    let _ = config_store::save(&config_path(), &current);
                }
                load_config();
                apply_loaded_config();
                maintenance_refresh();
                format!("Legacy settings imported without changing the original file. Unsupported keys: {}",
                    if unsupported.is_empty() { "none".into() } else { unsupported.join(", ") })
            } else { "Could not read a valid legacy settings file.".into() };
            let wide: Vec<u16> = result.encode_utf16().chain(Some(0)).collect();
            let title: Vec<u16> = "Import legacy config\0".encode_utf16().collect();
            MessageBoxW(hwnd, wide.as_ptr(), title.as_ptr(), 0);
            0
        }
        WM_COMMAND if (wparam & 0xffff) == MAINTENANCE_RESTORE_BACKUP => {
            if confirm_maintenance(hwnd, "Restore the previous Rust settings backup?") {
                let result = config_store::restore_backup(&config_path());
                if result.is_ok() {
                    load_config();
                    apply_loaded_config();
                    maintenance_refresh();
                } else {
                    let wide: Vec<u16> = "No valid settings backup was found.\0".encode_utf16().collect();
                    let title: Vec<u16> = "Restore backup\0".encode_utf16().collect();
                    MessageBoxW(hwnd, wide.as_ptr(), title.as_ptr(), 0);
                }
            }
            0
        }
        WM_CLOSE => { DestroyWindow(hwnd); 0 }
        WM_DESTROY => { MAINTENANCE_HANDLE.store(null_mut(), Ordering::Relaxed); 0 }
        _ => DefWindowProcW(hwnd, message, wparam, lparam),
    }
}
