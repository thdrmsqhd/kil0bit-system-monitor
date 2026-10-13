unsafe fn present_bitmap(
    hwnd: Hwnd,
    pixels: &[u8],
    width: i32,
    height: i32,
) -> Result<(), WinError> {
    if width <= 0 || height <= 0 || pixels.len() != width as usize * height as usize * 4 {
        return Err(WinError("Invalid premultiplied BGRA surface", 0));
    }
    let screen_dc = GetDC(null_mut());
    if screen_dc.is_null() {
        return Err(last_error("GetDC"));
    }
    let memory_dc = CreateCompatibleDC(screen_dc);
    if memory_dc.is_null() {
        ReleaseDC(null_mut(), screen_dc);
        return Err(last_error("CreateCompatibleDC"));
    }
    let info = BitmapInfo {
        header: BitmapInfoHeader {
            size: size_of::<BitmapInfoHeader>() as u32,
            width,
            height: -height,
            planes: 1,
            bit_count: 32,
            compression: BI_RGB,
            size_image: (width * height * 4) as u32,
            x_pixels_per_meter: 0,
            y_pixels_per_meter: 0,
            colors_used: 0,
            colors_important: 0,
        },
        colors: [RgbQuad {
            blue: 0,
            green: 0,
            red: 0,
            reserved: 0,
        }],
    };
    let mut bits = null_mut();
    let bitmap = CreateDIBSection(screen_dc, (&info as *const BitmapInfo).cast(),
        DIB_RGB_COLORS, &mut bits, null_mut(), 0);
    if bitmap.is_null() || bits.is_null() {
        DeleteDC(memory_dc);
        ReleaseDC(null_mut(), screen_dc);
        return Err(last_error("CreateDIBSection"));
    }
    std::ptr::copy_nonoverlapping(pixels.as_ptr(), bits.cast::<u8>(), pixels.len());
    // A zero-alpha pixel in a layered window is hit-test transparent. Keep
    // the transparent appearance while making the surface receive pointer
    // input so WM_NCHITTEST can return HTCAPTION for dragging.
    let surface = std::slice::from_raw_parts_mut(bits.cast::<u8>(), pixels.len());
    preserve_layered_input_surface(surface);
    let old_bitmap = SelectObject(memory_dc, bitmap);
    let mut window_rect = Rect {
        left: 100,
        top: 100,
        right: 100 + width,
        bottom: 100 + height,
    };
    GetWindowRect(hwnd, &mut window_rect);
    let destination = Point {
        x: window_rect.left,
        y: window_rect.top,
    };
    let source = Point { x: 0, y: 0 };
    let size = Size {
        cx: width,
        cy: height,
    };
    let blend = BlendFunction {
        operation: AC_SRC_OVER,
        flags: 0,
        source_constant_alpha: OVERLAY_ALPHA.load(Ordering::Relaxed) as u8,
        alpha_format: AC_SRC_ALPHA,
    };
    let success = UpdateLayeredWindow(
        hwnd,
        screen_dc,
        &destination,
        &size,
        memory_dc,
        &source,
        0,
        &blend,
        ULW_ALPHA,
    );
    SelectObject(memory_dc, old_bitmap);
    DeleteObject(bitmap);
    DeleteDC(memory_dc);
    ReleaseDC(null_mut(), screen_dc);
    if success == 0 {
        return Err(last_error("UpdateLayeredWindow"));
    }
    Ok(())
}

fn preserve_layered_input_surface(surface: &mut [u8]) {
    for pixel in surface.as_chunks_mut::<4>().0 {
        if pixel[3] == 0 {
            pixel[..3].fill(0);
            pixel[3] = 1;
        }
    }
}

unsafe fn last_error(operation: &'static str) -> WinError {
    WinError(operation, GetLastError())
}

#[cfg(test)]
mod tests {
    use super::preserve_layered_input_surface;

    #[test]
    fn transparent_pixels_remain_visually_black_but_receive_input() {
        let mut surface = [0, 0, 0, 0, 12, 24, 36, 128, 90, 80, 70, 255];
        preserve_layered_input_surface(&mut surface);
        assert_eq!(&surface[0..4], &[0, 0, 0, 1]);
        assert_eq!(&surface[4..8], &[12, 24, 36, 128]);
        assert_eq!(&surface[8..12], &[90, 80, 70, 255]);
    }
}
