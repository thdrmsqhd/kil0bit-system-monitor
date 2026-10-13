pub fn run(_pixels: &[u8], _width: i32, _height: i32) -> Result<(), WinError> {
    let Some(_instance_lock) = SingleInstanceLock::acquire()? else {
        return Ok(());
    };
    unsafe {
        load_config();
        let mut telemetry = super::telemetry::TelemetryCollector::new();
        let initial_config = config_lock().lock().map(|c| c.clone()).unwrap_or_default();
        let first_sample = telemetry.sample(&initial_config);
        let _ = CURRENT_METRICS.set(Mutex::new(first_sample));
        let (initial_pixels, initial_width, initial_height) = render_current_bitmap();
        // Per-monitor notifications are required before creating the overlay HWND.
        SetProcessDpiAwarenessContext((-4isize) as *mut c_void);
        let instance = GetModuleHandleW(null());
        if instance.is_null() {
            return Err(last_error("GetModuleHandleW"));
        }
        let class_name: Vec<u16> = "Kil0bitRustOverlayPoT\0".encode_utf16().collect();
        let window_name: Vec<u16> = "Kil0bit System Monitor Rust — right-click for menu\0"
            .encode_utf16()
            .collect();
        let class = WindowClass {
            style: 0,
            window_proc: Some(window_proc),
            class_extra: 0,
            window_extra: 0,
            instance,
            icon: null_mut(),
            cursor: null_mut(),
            background: null_mut(),
            menu_name: null(),
            class_name: class_name.as_ptr(),
        };
        if RegisterClassW(&class) == 0 {
            return Err(last_error("RegisterClassW"));
        }

        let always_on_top = config_lock()
            .lock()
            .map(|c| c.always_on_top)
            .unwrap_or(true);
        let hwnd = CreateWindowExW(
            WS_EX_LAYERED | WS_EX_TOOLWINDOW | if always_on_top { WS_EX_TOPMOST } else { 0 },
            class_name.as_ptr(),
            window_name.as_ptr(),
            WS_POPUP | WS_CAPTION as u32,
            FREE_X.load(Ordering::Relaxed),
            FREE_Y.load(Ordering::Relaxed),
            initial_width,
            initial_height,
            null_mut(),
            null_mut(),
            instance,
            null_mut(),
        );
        if hwnd.is_null() {
            UnregisterClassW(class_name.as_ptr(), instance);
            return Err(last_error("CreateWindowExW"));
        }

        OVERLAY_HANDLE.store(hwnd, Ordering::Relaxed);
        let _ = AI_SNAPSHOT.set(Mutex::new(None));
        let _ = AI_WORKER.set(Mutex::new(None));
        let _ = DEEPSEEK_SNAPSHOT.set(Mutex::new(None));
        let _ = DEEPSEEK_WORKER.set(Mutex::new(None));
        let surface_result = present_bitmap(hwnd, &initial_pixels, initial_width, initial_height);
        if surface_result.is_err() {
            DestroyWindow(hwnd);
            UnregisterClassW(class_name.as_ptr(), instance);
            return surface_result;
        }
        if SNAP_TO_TASKBAR.load(Ordering::Relaxed) {
            attach_to_taskbar(hwnd, initial_height);
        }
        if config_lock().lock().map(|c| c.show_overlay).unwrap_or(true) {
            ShowWindow(hwnd, SW_SHOWNOACTIVATE);
        } else {
            ShowWindow(hwnd, 0);
        }
        start_ai_worker(hwnd);
        start_deepseek_worker(hwnd);
        start_telemetry_worker(hwnd, telemetry);
        if !std::env::args().any(|arg| arg == "--startup") {
            open_settings_window(hwnd);
        }

        let mut message: Message = zeroed();
        loop {
            let result = GetMessageW(&mut message, null_mut(), 0, 0);
            if result == 0 {
                break;
            }
            if result == -1 {
                DestroyWindow(hwnd);
                UnregisterClassW(class_name.as_ptr(), instance);
                return Err(last_error("GetMessageW"));
            }
            TranslateMessage(&message);
            DispatchMessageW(&message);
        }
        UnregisterClassW(class_name.as_ptr(), instance);
        Ok(())
    }
}

struct SingleInstanceLock(*mut c_void);

impl SingleInstanceLock {
    fn acquire() -> Result<Option<Self>, WinError> {
        let name: Vec<u16> = "Local\\Kil0bitSystemMonitorRust-SingleInstance\0"
            .encode_utf16()
            .collect();
        let handle = unsafe { CreateMutexW(null_mut(), 0, name.as_ptr()) };
        if handle.is_null() {
            return Err(unsafe { last_error("CreateMutexW") });
        }
        if unsafe { GetLastError() } == ERROR_ALREADY_EXISTS {
            let class: Vec<u16> = "Kil0bitRustOverlayPoT\0".encode_utf16().collect();
            let existing = unsafe { FindWindowW(class.as_ptr(), null()) };
            if !existing.is_null() {
                unsafe {
                    PostMessageW(existing, WM_APP_SHOW_SETTINGS, 0, 0);
                }
            }
            unsafe {
                CloseHandle(handle);
            }
            Ok(None)
        } else {
            Ok(Some(Self(handle)))
        }
    }
}

impl Drop for SingleInstanceLock {
    fn drop(&mut self) {
        unsafe {
            CloseHandle(self.0);
        }
    }
}
