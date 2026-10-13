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
    if let Ok(choices) = NETWORK_CHOICES.get_or_init(|| Mutex::new(Vec::new())).lock() {
        networks.extend(choices.iter().cloned());
    }
    for (index, name) in networks.iter().enumerate() {
        let wide: Vec<u16> = name.encode_utf16().chain(Some(0)).collect();
        SendMessageW(combo, 0x0143, 0, wide.as_ptr() as isize);
        if config.network_adapter.as_deref() == Some(name.as_str()) || (index == 0 && config.network_adapter.is_none()) {
            SendMessageW(combo, 0x014E, index, 0);
        }
    }
    devices_child(window, "STATIC", "Disks to monitor", 0, 20, 90, 470, 24, 0);
    let list = devices_child(window, "LISTBOX", "", 0, 20, 120, 465, 120,
        WS_VSCROLL | WS_TABSTOP | 0x0001 | 0x0080);
    DISK_LIST.store(list, Ordering::Relaxed);
    let mut disks = Vec::new();
    if let Ok(choices) = DISK_CHOICES.get_or_init(|| Mutex::new(Vec::new())).lock() {
        disks.extend(choices.iter().cloned());
    }
    for name in &disks {
        let wide: Vec<u16> = name.encode_utf16().chain(Some(0)).collect();
        let index = SendMessageW(list, 0x0180, 0, wide.as_ptr() as isize);
        if config.monitored_disks.iter().any(|d| d == name) {
            SendMessageW(list, 0x0185, 1, index);
        }
    }
    devices_child(window, "STATIC", "GPU index", 0, 20, 260, 200, 24, 0);
    let gpu_edit = devices_child(window, "EDIT", &config.gpu_index.to_string(), 0,
        20, 290, 100, 28, WS_BORDER | WS_TABSTOP);
    GPU_INDEX_EDIT.store(gpu_edit, Ordering::Relaxed);
    devices_child(window, "STATIC", "GPU vendor", 0, 150, 260, 200, 24, 0);
    let vendor_combo = devices_child(window, "COMBOBOX", "", 0, 150, 290, 200, 180,
        WS_VSCROLL | WS_TABSTOP | 0x0003);
    GPU_VENDOR_COMBO.store(vendor_combo, Ordering::Relaxed);
    let vendors = ["Auto", "NVIDIA", "AMD", "Intel"];
    for (i, v) in vendors.iter().enumerate() {
        let wide: Vec<u16> = v.encode_utf16().chain(Some(0)).collect();
        SendMessageW(vendor_combo, 0x0143, 0, wide.as_ptr() as isize);
        if config.gpu_vendor.as_deref() == Some(*v) || (i == 0 && config.gpu_vendor.is_none()) {
            SendMessageW(vendor_combo, 0x014E, i, 0);
        }
    }
    devices_child(window, "BUTTON", "Save devices", DEVICES_SAVE,
        20, 340, 150, 34, WS_TABSTOP);
    devices_child(window, "BUTTON", "Appearance...", DEVICES_APPEARANCE,
        190, 340, 150, 34, WS_TABSTOP);
    ShowWindow(window, SW_SHOW);
    UpdateWindow(window);
}
