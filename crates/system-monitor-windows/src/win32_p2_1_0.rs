fn config_path() -> std::path::PathBuf {
    CONFIG_PATH
        .get_or_init(|| {
            let root = std::env::var_os("APPDATA")
                .map(std::path::PathBuf::from)
                .unwrap_or_else(std::env::temp_dir);
            root.join("Kil0bitSystemMonitorRust").join("config.json")
        })
        .clone()
}

fn config_lock() -> &'static Mutex<AppConfig> {
    APP_CONFIG.get_or_init(|| Mutex::new(AppConfig::default()))
}

fn load_config() {
    let config = config_store::load(&config_path());
    FREE_X.store(config.x as i32, Ordering::Relaxed);
    FREE_Y.store(config.y as i32, Ordering::Relaxed);
    POSITION_LOCKED.store(config.lock_position, Ordering::Relaxed);
    SNAP_TO_TASKBAR.store(config.stick_to_taskbar, Ordering::Relaxed);
    let _ = set_startup_registration(config.launch_on_startup);
    if let Ok(mut current) = config_lock().lock() {
        *current = config;
    }
}

fn set_startup_registration(enabled: bool) -> std::io::Result<()> {
    #[link(name = "advapi32")]
    extern "system" {
        fn RegCreateKeyExW(
            key: *mut c_void,
            subkey: *const u16,
            reserved: u32,
            class: *mut u16,
            options: u32,
            access: u32,
            security: *mut c_void,
            result: *mut *mut c_void,
            disposition: *mut u32,
        ) -> i32;
        fn RegSetValueExW(
            key: *mut c_void,
            name: *const u16,
            reserved: u32,
            value_type: u32,
            data: *const u8,
            data_size: u32,
        ) -> i32;
        fn RegDeleteValueW(key: *mut c_void, name: *const u16) -> i32;
        fn RegCloseKey(key: *mut c_void) -> i32;
    }
    const HKEY_CURRENT_USER: *mut c_void = (-2147483647_isize) as *mut c_void;
    const KEY_SET_VALUE: u32 = 0x0002;
    const REG_SZ: u32 = 1;
    const ERROR_FILE_NOT_FOUND: i32 = 2;
    let subkey: Vec<u16> = "Software\\Microsoft\\Windows\\CurrentVersion\\Run\0"
        .encode_utf16()
        .collect();
    let value_name: Vec<u16> = "Kil0bitSystemMonitorRust\0".encode_utf16().collect();
    let mut key = null_mut();
    let mut disposition = 0_u32;
    let open_status = unsafe {
        RegCreateKeyExW(
            HKEY_CURRENT_USER,
            subkey.as_ptr(),
            0,
            null_mut(),
            0,
            KEY_SET_VALUE,
            null_mut(),
            &mut key,
            &mut disposition,
        )
    };
    if open_status != 0 {
        return Err(std::io::Error::from_raw_os_error(open_status));
    }
    let status = if enabled {
        match std::env::current_exe() {
            Ok(path) => {
                let command = format!("\"{}\" --startup", path.display());
                let data: Vec<u16> = command.encode_utf16().chain(Some(0)).collect();
                unsafe {
                    RegSetValueExW(
                        key,
                        value_name.as_ptr(),
                        0,
                        REG_SZ,
                        data.as_ptr().cast(),
                        (data.len() * size_of::<u16>()) as u32,
                    )
                }
            }
            Err(error) => {
                unsafe {
                    RegCloseKey(key);
                }
                return Err(error);
            }
        }
    } else {
        let code = unsafe { RegDeleteValueW(key, value_name.as_ptr()) };
        if code == ERROR_FILE_NOT_FOUND {
            0
        } else {
            code
        }
    };
    unsafe {
        RegCloseKey(key);
    }
    if status == 0 {
        Ok(())
    } else {
        Err(std::io::Error::from_raw_os_error(status))
    }
}

fn save_config_flags() {
    if let Ok(mut config) = config_lock().lock() {
        config.lock_position = POSITION_LOCKED.load(Ordering::Relaxed);
        config.stick_to_taskbar = SNAP_TO_TASKBAR.load(Ordering::Relaxed);
        config.x = FREE_X.load(Ordering::Relaxed) as f64;
        config.y = FREE_Y.load(Ordering::Relaxed) as f64;
        let _ = config_store::save(&config_path(), &config);
    }
}
