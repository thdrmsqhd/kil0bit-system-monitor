unsafe extern "system" fn window_proc(
    hwnd: Hwnd,
    message: u32,
    wparam: usize,
    lparam: isize,
) -> Lresult {
    // PART0_PLACEHOLDER - full content uploaded via next batch if truncated
    DefWindowProcW(hwnd, message, wparam, lparam)
}
