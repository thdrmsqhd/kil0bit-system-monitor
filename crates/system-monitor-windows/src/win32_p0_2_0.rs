unsafe fn open_settings_window(_owner: Hwnd) {
    let current = SETTINGS_HANDLE.load(Ordering::Relaxed);
    if !current.is_null() {
        unsafe {
            let _ = ShowWindow(current, SW_SHOW);
            let _ = SetWindowPos(
                current,
                HWND_TOPMOST,
                0,
                0,
                0,
                0,
                SWP_NOSIZE | SWP_NOACTIVATE,
            );
        }
        return;
    }
    // remainder truncated for this test - will fix next
}