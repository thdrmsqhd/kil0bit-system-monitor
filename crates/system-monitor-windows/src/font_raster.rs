//! Rasterize Windows system fonts into the software BGRA overlay surface.
//! A fixed cell keeps metric widths stable as live values change.

#[cfg(windows)]
pub fn draw_text(
    pixels: &mut [u8], width: i32, height: i32,
    glyphs: &[(char, [u8; 3])], origin_x: i32, origin_y: i32,
    family: &str, bold: bool,
) -> bool {
    use std::ffi::c_void;
    use std::mem::size_of;
    use std::ptr::null_mut;
    type Handle = *mut c_void;
    #[repr(C)]
    struct BitmapHeader {
        size: u32, width: i32, height: i32, planes: u16, bits: u16,
        compression: u32, image_size: u32, x_pels: i32, y_pels: i32,
        used: u32, important: u32,
    }
    #[repr(C)]
    struct BitmapInfo { header: BitmapHeader, colors: [u32; 1] }
    #[link(name = "gdi32")]
    extern "system" {
        fn CreateCompatibleDC(dc: Handle) -> Handle;
        fn DeleteDC(dc: Handle) -> i32;
        fn CreateDIBSection(dc: Handle, info: *const BitmapInfo, usage: u32,
            bits: *mut Handle, section: Handle, offset: u32) -> Handle;
        fn CreateFontW(height: i32, width: i32, escapement: i32, orientation: i32,
            weight: i32, italic: u32, underline: u32, strike: u32,
            charset: u32, out_precision: u32, clip_precision: u32,
            quality: u32, pitch_family: u32, face: *const u16) -> Handle;
        fn SelectObject(dc: Handle, object: Handle) -> Handle;
        fn DeleteObject(object: Handle) -> i32;
        fn SetBkMode(dc: Handle, mode: i32) -> i32;
        fn SetTextColor(dc: Handle, color: u32) -> u32;
        fn TextOutW(dc: Handle, x: i32, y: i32, text: *const u16, length: i32) -> i32;
    }
    const CELL_W: i32 = 12;
    const CELL_H: i32 = 18;
    unsafe {
        let dc = CreateCompatibleDC(null_mut());
        if dc.is_null() { return false; }
        let info = BitmapInfo {
            header: BitmapHeader {
                size: size_of::<BitmapHeader>() as u32, width: CELL_W, height: -CELL_H,
                planes: 1, bits: 32, compression: 0, image_size: (CELL_W * CELL_H * 4) as u32,
                x_pels: 0, y_pels: 0, used: 0, important: 0,
            }, colors: [0],
        };
        let mut bits = null_mut();
        let bitmap = CreateDIBSection(dc, &info, 0, &mut bits, null_mut(), 0);
        if bitmap.is_null() || bits.is_null() { DeleteDC(dc); return false; }
        let face: Vec<u16> = family.encode_utf16().chain(Some(0)).collect();
        let font = CreateFontW(-15, 0, 0, 0, if bold { 700 } else { 400 },
            0, 0, 0, 1, 0, 0, 4, 0, face.as_ptr());
        if font.is_null() { DeleteObject(bitmap); DeleteDC(dc); return false; }
        let old_bitmap = SelectObject(dc, bitmap);
        let old_font = SelectObject(dc, font);
        SetBkMode(dc, 1);
        SetTextColor(dc, 0x00ff_ffff);
        let mask = bits.cast::<u8>();
        for (index, (ch, color)) in glyphs.iter().enumerate() {
            if *ch == ' ' { continue; }
            std::ptr::write_bytes(mask, 0, (CELL_W * CELL_H * 4) as usize);
            let mut character = [0_u16; 2];
            let encoded = ch.encode_utf16(&mut character);
            if TextOutW(dc, 0, 0, encoded.as_ptr(), encoded.len() as i32) == 0 { continue; }
            for y in 0..CELL_H {
                let dest_y = origin_y + y;
                if dest_y < 0 || dest_y >= height { continue; }
                for x in 0..CELL_W {
                    let dest_x = origin_x + index as i32 * CELL_W + x;
                    if dest_x < 0 || dest_x >= width { continue; }
                    let src = ((y * CELL_W + x) * 4) as usize;
                    let coverage = (*mask.add(src)).max(*mask.add(src + 1))
                        .max(*mask.add(src + 2));
                    if coverage == 0 { continue; }
                    let dest = ((dest_y * width + dest_x) * 4) as usize;
                    let remaining = 255_u16 - coverage as u16;
                    for channel in 0..3 {
                        pixels[dest + channel] = ((color[channel] as u16 * coverage as u16
                            + pixels[dest + channel] as u16 * remaining) / 255) as u8;
                    }
                    pixels[dest + 3] = (coverage as u16
                        + pixels[dest + 3] as u16 * remaining / 255).min(255) as u8;
                }
            }
        }
        SelectObject(dc, old_font);
        SelectObject(dc, old_bitmap);
        DeleteObject(font);
        DeleteObject(bitmap);
        DeleteDC(dc);
        true
    }
}

#[cfg(not(windows))]
pub fn draw_text(_: &mut [u8], _: i32, _: i32, _: &[(char, [u8; 3])],
    _: i32, _: i32, _: &str, _: bool) -> bool { false }
