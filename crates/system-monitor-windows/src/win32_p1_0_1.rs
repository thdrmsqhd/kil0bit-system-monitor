unsafe fn devices_child(parent: Hwnd, class: &str, caption: &str, id: usize,
    x: i32, y: i32, width: i32, height: i32, style: u32) -> Hwnd {
    let class: Vec<u16> = class.encode_utf16().chain(Some(0)).collect();
    let caption: Vec<u16> = caption.encode_utf16().chain(Some(0)).collect();
    CreateWindowExW(0, class.as_ptr(), caption.as_ptr(), WS_CHILD | WS_VISIBLE | style,
        x, y, width, height, parent, id as *mut c_void, GetModuleHandleW(null()), null_mut())
}

unsafe fn open_devices_window(owner: Hwnd) {
    let existing = DEVICES_HANDLE.load(Ordering::Relaxed);
    if !existing.is_null() { ShowWindow(existing, SW_SHOW); return; }
    let class_name: Vec<u16> = "Kil0bitRustDevices\0".encode_utf16().collect();
    let title: Vec<u16> = "Monitoring devices\0".encode_utf16().collect();
    let instance = GetModuleHandleW(null());
    let class = WindowClass {
        style: 0, window_proc: Some(devices_window_proc), class_extra: 0,
        window_extra: 0, instance, icon: null_mut(), cursor: null_mut(),
        background: null_mut(), menu_name: null(), class_name: class_name.as_ptr(),
    };
    if RegisterClassW(&class) == 0 && GetLastError() != ERROR_CLASS_ALREADY_EXISTS { return; }
    let window = CreateWindowExW(WS_EX_TOOLWINDOW, class_name.as_ptr(), title.as_ptr(),
        WS_OVERLAPPEDWINDOW, 360, 120, 520, 560, owner, null_mut(), instance, null_mut());
    if window.is_null() { return; }
    DEVICES_HANDLE.store(window, Ordering::Relaxed);
    let config = config_lock().lock().map(|c| c.clone()).unwrap_or_default();
    devices_child(window, "STATIC", "Network adapter", 0, 20, 20, 470, 24, 0);
    let combo = devices_child(window, "COMBOBOX", "", 0, 20, 50, 465, 180,
        WS_VSCROLL | WS_TABSTOP | 0x0003);
    NETWORK_COMBO.store(combo, Ordering::Relaxed);
    let mut networks = vec!["Default".to_owned()];
    networks.extend(sysinfo::Networks::new_with_refreshed_list().keys()
        .filter(|name| super::telemetry::is_eligible_adapter(name)).cloned());
    networks.sort();
    networks.dedup();
    let selected_network = networks.iter().position(|name| *name == config.network_adapter)
        .unwrap_or_else(|| networks.iter().position(|name| name == "Default").unwrap_or(0));
    for name in &networks {
        let wide: Vec<u16> = name.encode_utf16().chain(Some(0)).collect();
        SendMessageW(combo, 0x0143, 0, wide.as_ptr() as isize);
    }
    SendMessageW(combo, 0x014E, selected_network, 0);
    *NETWORK_CHOICES.get_or_init(|| Mutex::new(Vec::new())).lock().unwrap() = networks;
    devices_child(window, "STATIC", "Disk volumes", 0, 20, 95, 470, 24, 0);
    let list = devices_child(window, "LISTBOX", "", 0, 20, 125, 465, 140,
        WS_VSCROLL | WS_TABSTOP | 0x0001 | 0x0080);
    DISK_LIST.store(list, Ordering::Relaxed);
    let mut disks = Vec::new();
    disks.extend(sysinfo::Disks::new_with_refreshed_list().list().iter()
        .filter_map(|disk| disk.mount_point().to_str().map(str::to_owned)));
    disks.sort();
    disks.dedup();
    for name in &disks {
        let wide: Vec<u16> = name.encode_utf16().chain(Some(0)).collect();
        let index = SendMessageW(list, 0x0180, 0, wide.as_ptr() as isize);
        if config.disk_volumes.iter().any(|volume| volume == name) {
            SendMessageW(list, 0x0185, 1, index);
        }
    }
    *DISK_CHOICES.get_or_init(|| Mutex::new(Vec::new())).lock().unwrap() = disks;
    devices_child(window, "STATIC", "GPU adapter", 0, 20, 285, 300, 24, 0);
    let gpu_vendor_combo = devices_child(window, "COMBOBOX", "", 0,
        20, 315, 290, 200, WS_VSCROLL | WS_TABSTOP | 0x0003);
    GPU_VENDOR_COMBO.store(gpu_vendor_combo, Ordering::Relaxed);
    let mut vendors = vec!["Auto".to_owned()];
    if let Ok(engines) = super::pdh::list_gpu_engine_instances() {
        let engine_names: Vec<String> = engines.into_iter().map(|(_, name)| name).collect();
        for device in super::gpu::enumerate_gpus(&engine_names) {
            if !vendors.iter().any(|name| name.eq_ignore_ascii_case(&device.name)) {
                vendors.push(device.name);
            }
        }
    }
    if !vendors.iter().any(|name| name == &config.gpu_adapter) {
        vendors.push(config.gpu_adapter.clone());
    }
    let vendor_index = vendors.iter().position(|name| name == &config.gpu_adapter).unwrap_or(0);
    for vendor in &vendors {
        let wide: Vec<u16> = vendor.encode_utf16().chain(Some(0)).collect();
        SendMessageW(gpu_vendor_combo, 0x0143, 0, wide.as_ptr() as isize);
    }
    SendMessageW(gpu_vendor_combo, 0x014E, vendor_index, 0);
    *GPU_VENDOR_CHOICES.get_or_init(|| Mutex::new(Vec::new())).lock().unwrap() = vendors;
    devices_child(window, "STATIC", "Index", 0, 325, 285, 50, 24, 0);
    let edit = devices_child(window, "EDIT", &config.gpu_index.to_string(), 0,
        390, 283, 95, 28, WS_BORDER | WS_TABSTOP);
    GPU_INDEX_EDIT.store(edit, Ordering::Relaxed);
    devices_child(window, "STATIC", "Telemetry refresh", 0,
        20, 330, 180, 24, 0);
    let interval = devices_child(window, "COMBOBOX", "", 0,
        205, 328, 175, 120, WS_VSCROLL | WS_TABSTOP | 0x0003);
    for caption in ["0.5 seconds", "1 second", "2 seconds", "5 seconds"] {
        let wide: Vec<u16> = caption.encode_utf16().chain(Some(0)).collect();
        SendMessageW(interval, 0x0143, 0, wide.as_ptr() as isize);
    }
    let interval_index = [500, 1000, 2000, 5000]
        .iter().position(|value| *value == config.update_interval_ms).unwrap_or(1);
    SendMessageW(interval, 0x014E, interval_index, 0);
    UPDATE_COMBO.store(interval, Ordering::Relaxed);
    for (index, (caption, checked)) in [
        ("Hide in fullscreen", config.hide_on_fullscreen),
        ("Show disk activity", config.show_disk_speed),
    ].iter().enumerate() {
        let control = devices_child(window, "BUTTON", caption, 0,
            20 + index as i32 * 225, 385, 220, 25, WS_TABSTOP | 0x0003);
        DEVICE_CHECKS[index].store(control, Ordering::Relaxed);
        if *checked { SendMessageW(control, BM_SETCHECK, BST_CHECKED, 0); }
    }
    devices_child(window, "BUTTON", "Save selection", DEVICES_SAVE,
        20, 440, 175, 36, WS_TABSTOP);
    devices_child(window, "BUTTON", "Appearance...", DEVICES_APPEARANCE,
        215, 440, 175, 36, WS_TABSTOP);
    ShowWindow(window, SW_SHOW);
    UpdateWindow(window);
}
