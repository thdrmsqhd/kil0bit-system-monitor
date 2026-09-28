//! Small deterministic PoT surface; intentionally independent of Win32.

pub const WIDTH: i32 = 192;
pub const HEIGHT: i32 = 52;
pub const BYTES_PER_PIXEL: usize = 4;

/// Builds a premultiplied-alpha BGRA sample capsule containing `RUST`.
pub fn build_demo_bitmap(alternate_accent: bool) -> Vec<u8> {
    let mut pixels = vec![0; WIDTH as usize * HEIGHT as usize * BYTES_PER_PIXEL];
    let alpha = 230u8;
    let capsule = if alternate_accent {
        [28u8, 78, 30] // BGRA source color, dark green.
    } else {
        [38u8, 31, 24] // BGRA source color, dark blue-gray.
    };
    let white = [242u8, 242, 242];
    let radius = 13i32;

    for y in 0..HEIGHT {
        for x in 0..WIDTH {
            let cx = if x < radius {
                radius
            } else if x >= WIDTH - radius {
                WIDTH - radius - 1
            } else {
                x
            };
            let cy = if y < radius {
                radius
            } else if y >= HEIGHT - radius {
                HEIGHT - radius - 1
            } else {
                y
            };
            let dx = x - cx;
            let dy = y - cy;
            if dx * dx + dy * dy <= radius * radius {
                put_pixel(&mut pixels, x, y, capsule, alpha);
            }
        }
    }

    // 5x7 bitmap glyphs, scaled 3x, with a three-pixel scaled gap.
    const RUST: [[u8; 7]; 4] = [
        [
            0b11110, 0b10001, 0b10001, 0b11110, 0b10100, 0b10010, 0b10001,
        ], // R
        [
            0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110,
        ], // U
        [
            0b01111, 0b10000, 0b10000, 0b01110, 0b00001, 0b00001, 0b11110,
        ], // S
        [
            0b11111, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100,
        ], // T
    ];
    let scale = 3i32;
    let origin_x = 24i32;
    let origin_y = 15i32;
    for (letter, glyph) in RUST.iter().enumerate() {
        for (row, bits) in glyph.iter().enumerate() {
            for col in 0..5 {
                if bits & (1 << (4 - col)) != 0 {
                    for sy in 0..scale {
                        for sx in 0..scale {
                            put_pixel(
                                &mut pixels,
                                origin_x + letter as i32 * 18 + col * scale + sx,
                                origin_y + row as i32 * scale + sy,
                                white,
                                255,
                            );
                        }
                    }
                }
            }
        }
    }
    pixels
}

/// Scales a premultiplied BGRA surface with nearest-neighbor sampling.
pub fn scale_bitmap(pixels: &[u8], scale_percent: u32) -> (Vec<u8>, i32, i32) {
    let scale_percent = scale_percent.clamp(50, 300);
    let width = ((WIDTH as u32 * scale_percent + 50) / 100) as i32;
    let height = ((HEIGHT as u32 * scale_percent + 50) / 100) as i32;
    let mut scaled = vec![0; width as usize * height as usize * BYTES_PER_PIXEL];
    for y in 0..height {
        let source_y = y as usize * HEIGHT as usize / height as usize;
        for x in 0..width {
            let source_x = x as usize * WIDTH as usize / width as usize;
            let source = (source_y * WIDTH as usize + source_x) * BYTES_PER_PIXEL;
            let target = (y as usize * width as usize + x as usize) * BYTES_PER_PIXEL;
            scaled[target..target + BYTES_PER_PIXEL]
                .copy_from_slice(&pixels[source..source + BYTES_PER_PIXEL]);
        }
    }
    (scaled, width, height)
}

fn put_pixel(pixels: &mut [u8], x: i32, y: i32, bgr: [u8; 3], alpha: u8) {
    let offset = (y as usize * WIDTH as usize + x as usize) * BYTES_PER_PIXEL;
    let a = alpha as u16;
    pixels[offset] = (bgr[0] as u16 * a / 255) as u8;
    pixels[offset + 1] = (bgr[1] as u16 * a / 255) as u8;
    pixels[offset + 2] = (bgr[2] as u16 * a / 255) as u8;
    pixels[offset + 3] = alpha;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bitmap_has_transparent_corners_and_translucent_capsule() {
        let pixels = build_demo_bitmap(false);
        let corner = 0;
        let center = (HEIGHT as usize / 2 * WIDTH as usize + WIDTH as usize / 2) * BYTES_PER_PIXEL;
        assert_eq!(pixels[corner + 3], 0);
        assert_eq!(pixels[center + 3], 230);
        assert!(pixels[center] <= pixels[center + 3]);
        assert!(pixels[center + 1] <= pixels[center + 3]);
        assert!(pixels[center + 2] <= pixels[center + 3]);
    }

    #[test]
    fn dpi_scaling_preserves_pixel_alpha_and_scales_bounds() {
        let original = build_demo_bitmap(false);
        let (scaled, width, height) = scale_bitmap(&original, 150);
        assert_eq!(width, 288);
        assert_eq!(height, 78);
        assert_eq!(
            scaled.len(),
            width as usize * height as usize * BYTES_PER_PIXEL
        );
        assert_eq!(scaled[3], 0);
        let center = (height as usize / 2 * width as usize + width as usize / 2) * BYTES_PER_PIXEL;
        assert_eq!(scaled[center + 3], 230);
    }

    #[test]
    fn bitmap_contains_opaque_glyph_pixels() {
        let pixels = build_demo_bitmap(false);
        let glyph = (15 * WIDTH as usize + 24) * BYTES_PER_PIXEL;
        assert_eq!(pixels[glyph + 3], 255);
        assert_eq!(&pixels[glyph..glyph + 3], &[242, 242, 242]);
    }
}
