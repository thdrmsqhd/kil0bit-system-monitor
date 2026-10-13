fn save_position() {
    if let Ok(mut config) = config_lock().lock() {
        config.x = FREE_X.load(Ordering::Relaxed) as f64;
        if !SNAP_TO_TASKBAR.load(Ordering::Relaxed) {
            config.y = FREE_Y.load(Ordering::Relaxed) as f64;
        }
        config.stick_to_taskbar = SNAP_TO_TASKBAR.load(Ordering::Relaxed);
        let _ = config_store::save(&config_path(), &config);
    }
}

fn render_current_bitmap() -> (Vec<u8>, i32, i32) {
    let metrics = CURRENT_METRICS
        .get()
        .and_then(|state| state.lock().ok().map(|value| value.clone()))
        .unwrap_or_default();
    let config = config_lock()
        .lock()
        .map(|value| value.clone())
        .unwrap_or_default();
    let (base, width, height) = super::bitmap::build_metrics_bitmap(
        &metrics,
        &config,
        AI_SNAPSHOT
            .get()
            .and_then(|v| v.lock().ok())
            .and_then(|v| v.clone())
            .as_ref(),
        DEEPSEEK_SNAPSHOT.get().and_then(|v| v.lock().ok())
            .and_then(|v| v.clone()).as_ref(),
        ALTERNATE_ACCENT.load(Ordering::Relaxed),
    );
    super::bitmap::scale_surface(
        &base,
        width,
        height,
        (DPI_SCALE_PERCENT.load(Ordering::Relaxed) as f64 * config.scale_factor)
            .round().clamp(25.0, 400.0) as u32,
    )
}

fn start_telemetry_worker(hwnd: Hwnd, mut collector: super::telemetry::TelemetryCollector) {
    let (stop, stopped) = mpsc::channel();
    let handle = hwnd as usize;
    let thread = std::thread::spawn(move || loop {
        let config = config_lock().lock().map(|c| c.clone()).unwrap_or_default();
        if stopped.recv_timeout(Duration::from_millis(config.update_interval_ms as u64)).is_ok() {
            break;
        }
        let sample = collector.sample(&config);
        if let Some(state) = CURRENT_METRICS.get() {
            if let Ok(mut current) = state.lock() {
                *current = sample;
            }
        }
        unsafe { PostMessageW(handle as Hwnd, WM_APP_REFRESH, 0, 0); }
    });
    let _ = TELEMETRY_WORKER.set(Mutex::new(Some(TelemetryWorker { stop, thread: Some(thread) })));
}

unsafe fn refresh_overlay(hwnd: Hwnd) {
    let visible = config_lock()
        .lock()
        .map(|config| config.show_overlay)
        .unwrap_or(true);
    if !visible || (FULLSCREEN_HIDDEN.load(Ordering::Relaxed)
        && OVERLAY_ALPHA.load(Ordering::Relaxed) == 0) {
        ShowWindow(hwnd, 0);
        return;
    }
    let (pixels, width, height) = render_current_bitmap();
    let _ = present_bitmap(hwnd, &pixels, width, height);
}

unsafe fn update_fullscreen_visibility(hwnd: Hwnd) {
    let (enabled, app_visible) = config_lock()
        .lock()
        .map(|c| (c.hide_on_fullscreen, c.show_overlay))
        .unwrap_or((false, true));
    let foreground = GetForegroundWindow();
    let mut hidden = false;
    if enabled
        && app_visible
        && !foreground.is_null()
        && foreground != hwnd
        && foreground != SETTINGS_HANDLE.load(Ordering::Relaxed)
    {
        let mut class_name = [0_u16; 128];
        let length = GetClassNameW(foreground, class_name.as_mut_ptr(), class_name.len() as i32)
            .max(0) as usize;
        let class = String::from_utf16_lossy(&class_name[..length]);
        let shell_exempt = [
            "Shell_TrayWnd",
            "Progman",
            "WorkerW",
            "MultitaskingViewFrame",
            "XamlExplorerHostIslandWindow",
        ]
        .iter()
        .any(|candidate| candidate.eq_ignore_ascii_case(&class));
        if !shell_exempt {
            let mut rect = Rect {
                left: 0,
                top: 0,
                right: 0,
                bottom: 0,
            };
            let monitor = MonitorFromWindow(foreground, 2);
            let mut info = MonitorInfo {
                size: size_of::<MonitorInfo>() as u32,
                monitor: Rect { left: 0, top: 0, right: 0, bottom: 0 },
                work: Rect { left: 0, top: 0, right: 0, bottom: 0 },
                flags: 0,
            };
            hidden = !monitor.is_null()
                && monitor == MonitorFromWindow(hwnd, 2)
                && GetMonitorInfoW(monitor, &mut info) != 0
                && GetWindowRect(foreground, &mut rect) != 0
                && GetWindowLongPtrW(foreground, -16) & WS_CAPTION == 0
                && rect.left <= info.monitor.left && rect.top <= info.monitor.top
                && rect.right >= info.monitor.right && rect.bottom >= info.monitor.bottom;
        }
    }
    if let Ok(mut candidate) = FULLSCREEN_CANDIDATE.lock() {
        if candidate.0 != hidden {
            *candidate = (hidden, 1);
            return;
        }
        candidate.1 = candidate.1.saturating_add(1);
        if candidate.1 < 2 { return; }
    }
    let previous = FULLSCREEN_HIDDEN.swap(hidden, Ordering::Relaxed);
    if previous != hidden {
        if !hidden && app_visible {
            ShowWindow(hwnd, SW_SHOWNOACTIVATE);
        }
        if app_visible { SetTimer(hwnd, FADE_TIMER_ID, 16, null_mut()); }
    }
}
