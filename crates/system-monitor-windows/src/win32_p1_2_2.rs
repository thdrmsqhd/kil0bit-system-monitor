fn start_ai_worker(hwnd: Hwnd) {
    if hwnd.is_null() {
        return;
    }
    let config = config_lock().lock().map(|c| c.clone()).unwrap_or_default();
    if !config.opencode_enabled && !config.codex_enabled {
        return;
    }
    let store = if config.codex_enabled {
        None
    } else {
        let Ok(store) = crate::secret_store::SecretStore::opencode() else {
            return;
        };
        if !store.exists() {
            return;
        }
        Some(store)
    };
    if config.codex_enabled && super::ai_usage::codex_auth_path().is_err() {
        return;
    }
    let Ok(mut slot) = AI_WORKER.get_or_init(|| Mutex::new(None)).lock() else {
        return;
    };
    if let Some(worker) = slot.as_ref() {
        worker.wake();
        return;
    }
    let hwnd_value = hwnd as usize;
    let callback = move |event: super::ai_usage::ProviderEvent| {
        if let Some(snapshot) = AI_SNAPSHOT.get() {
            if let Ok(mut value) = snapshot.lock() {
                *value = event.snapshot;
            }
        }
        unsafe {
            PostMessageW(hwnd_value as Hwnd, WM_APP_REFRESH, 0, 0);
        }
    };
    let worker = if config.codex_enabled {
        super::ai_usage::AiUsageWorker::start_codex(config.ai_poll_interval_seconds, callback)
    } else {
        let Some(store) = store else {
            return;
        };
        super::ai_usage::AiUsageWorker::start(config.ai_poll_interval_seconds, store, callback)
    };
    *slot = Some(worker);
}

fn save_ai_options_from_settings() {
    let checked: Vec<bool> = AI_OPTION_CONTROLS
        .iter()
        .map(|control| {
            let handle = control.load(Ordering::Relaxed);
            !handle.is_null()
                && unsafe { SendMessageW(handle, BM_GETCHECK, 0, 0) } == BST_CHECKED as isize
        })
        .collect();
    let combo = AI_INTERVAL_COMBO.load(Ordering::Relaxed);
    let selected = if combo.is_null() {
        -1
    } else {
        unsafe { SendMessageW(combo, 0x0147, 0, 0) as i32 }
    };
    let interval = AI_INTERVALS
        .get(selected.max(0) as usize)
        .copied()
        .unwrap_or(300);
    let previous_interval = config_lock().lock()
        .map(|c| c.ai_poll_interval_seconds).unwrap_or(300);
    let mut provider_changed = false;
    if let Ok(mut config) = config_lock().lock() {
        if checked.len() == 5 {
            config.opencode_show_rolling = checked[0];
            config.opencode_show_weekly = checked[1];
            config.opencode_show_monthly = checked[2];
            config.ai_show_used_percent = checked[3];
            provider_changed = config.codex_enabled != checked[4];
            config.codex_enabled = checked[4];
            if config.codex_enabled {
                config.opencode_enabled = false;
            } else if provider_changed
                && crate::secret_store::SecretStore::opencode().is_ok_and(|store| store.exists())
            {
                config.opencode_enabled = true;
            }
        }
        config.ai_poll_interval_seconds = interval;
        let _ = config_store::save(&config_path(), &config);
    }
    if provider_changed {
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
        start_ai_worker(OVERLAY_HANDLE.load(Ordering::Relaxed));
    }
    if let Some(workers) = AI_WORKER.get() {
        if let Ok(slot) = workers.lock() {
            if let Some(worker) = slot.as_ref() {
                worker.set_interval(interval);
            }
        }
    }
    if previous_interval != interval {
        restart_deepseek_worker(OVERLAY_HANDLE.load(Ordering::Relaxed));
    }
    let overlay = OVERLAY_HANDLE.load(Ordering::Relaxed);
    if !overlay.is_null() {
        unsafe {
            PostMessageW(overlay, WM_APP_REFRESH, 0, 0);
        }
    }
}
