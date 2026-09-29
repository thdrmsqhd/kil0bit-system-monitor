//! Small deterministic PoT surface; intentionally independent of Win32.

pub const WIDTH: i32 = 192;
pub const HEIGHT: i32 = 52;
pub const BYTES_PER_PIXEL: usize = 4;

/// Builds the live text row from a snapshot using the port's selected metric flags.
pub fn build_metrics_bitmap(
    metrics: &system_monitor_core::SystemMetrics,
    config: &system_monitor_core::AppConfig,
    ai: Option<&system_monitor_core::AiUsageSnapshot>,
    deepseek: Option<&super::ai_usage::DeepSeekBalanceSnapshot>,
    alternate_accent: bool,
) -> (Vec<u8>, i32, i32) {
    let compact = config.display_style == "Compact";
    let mut fields = Vec::new();
    let global_color = parse_hex_bgr(&config.accent_color_hex).unwrap_or([242, 242, 242]);
    let mut add = |enabled: bool, full: &str, short: &str, value: String| {
        if enabled {
            fields.push((
                format!("{} {value}", if compact { short } else { full }),
                global_color,
            ));
        }
    };
    add(
        config.show_cpu,
        "CPU",
        "C",
        format!("{}%", metrics.cpu_usage_percent.clamp(0.0, 100.0) as u32),
    );
    add(
        config.show_ram,
        "RAM",
        "R",
        format!("{}%", metrics.ram_percent.clamp(0.0, 100.0) as u32),
    );
    add(
        config.show_gpu,
        "GPU",
        "G",
        if metrics.gpu_usage_available {
            format!("{}%", metrics.gpu_usage_percent.clamp(0.0, 100.0) as u32)
        } else {
            "NA".into()
        },
    );
    add(
        config.show_temp,
        "TEMP",
        "T",
        metrics
            .gpu_temperature_c
            .map(|v| format!("{}C", v as i32))
            .unwrap_or_else(|| "NA".into()),
    );
    add(config.show_net_up, "UP", "U", metrics.net_up_text.clone());
    add(
        config.show_net_down,
        "DOWN",
        "D",
        metrics.net_down_text.clone(),
    );
    add(
        config.show_disk,
        "DISK",
        "K",
        format!("{}%", metrics.disk_used_percent.clamp(0.0, 100.0) as u32),
    );
    add(
        config.show_disk_speed,
        "ACT",
        "A",
        if metrics.disk_activity_available {
            format!("{}%", metrics.disk_usage_percent.clamp(0.0, 100.0) as u32)
        } else {
            "NA".into()
        },
    );
    if config.show_disk {
        for disk in &metrics.disks {
            let name = disk.name.trim_end_matches(['\\', '/']);
            fields.push((
                format!("{name} {}%", disk.space_percent as u32),
                global_color,
            ));
        }
    }
    if config.opencode_enabled || config.codex_enabled {
        if let Some(snapshot) = ai {
            let mut add_window =
                |enabled: bool,
                 label: &str,
                 window: Option<&system_monitor_core::AiUsageWindow>| {
                    if !enabled {
                        return;
                    }
                    if let Some(window) = window {
                        let shown = if config.ai_show_used_percent {
                            window.used_percent
                        } else {
                            window.remaining_percent()
                        };
                        let color = if window.used_percent >= 95 {
                            [40, 40, 240]
                        } else if window.used_percent >= config.ai_warning_threshold_percent {
                            [0, 165, 255]
                        } else {
                            [30, 200, 80]
                        };
                        fields.push((format!("{label} {shown}%"), color));
                    }
                };
            add_window(
                config.opencode_show_rolling,
                "5H",
                snapshot.rolling.as_ref(),
            );
            add_window(config.opencode_show_weekly, "W", snapshot.weekly.as_ref());
            add_window(config.opencode_show_monthly, "M", snapshot.monthly.as_ref());
        }
    }
    if config.deepseek_enabled {
        if let Some(balance) = deepseek {
            for entry in &balance.balances {
                fields.push((
                    format!("DS {} {:.2}", entry.currency, entry.total),
                    [30, 200, 80],
                ));
            }
        }
    }
    let mut glyphs = Vec::new();
    let default_label = parse_hex_bgr(&config.label_color_hex).unwrap_or(global_color);
    let no_override: Option<String> = None;
    for (index, (field, color)) in fields.iter().enumerate() {
        if index > 0 {
            for _ in 0..(2 + config.column_spacing / 3) {
                glyphs.push((' ', global_color));
            }
        }
        let (label, value) = field.split_once(' ').unwrap_or((field, ""));
        let (label_override, value_override) = match label {
            "UP" | "DOWN" | "U" | "D" =>
                (&config.net_label_color_hex, &config.net_accent_color_hex),
            "CPU" | "RAM" | "C" | "R" =>
                (&config.cpu_ram_label_color_hex, &config.cpu_ram_accent_color_hex),
            "GPU" | "TEMP" | "G" | "T" =>
                (&config.gpu_label_color_hex, &config.gpu_accent_color_hex),
            "DISK" | "ACT" | "K" | "A" =>
                (&config.disk_label_color_hex, &config.disk_accent_color_hex),
            "5H" | "W" | "M" | "DS" => (&no_override, &no_override),
            _ => (&config.disk_label_color_hex, &config.disk_accent_color_hex),
        };
        let label_color = label_override.as_deref().and_then(parse_hex_bgr).unwrap_or(default_label);
        let value_color = value_override.as_deref().and_then(parse_hex_bgr).unwrap_or(*color);
        glyphs.extend(label.chars().map(|ch| (ch, label_color)));
        glyphs.push((' ', label_color));
        glyphs.extend(value.chars().map(|ch| (ch, value_color)));
    }
    let char_count = glyphs.len() as i32;
    let width = (32 + char_count * 12).max(52);
    let height = HEIGHT;
    let mut pixels = vec![0; width as usize * height as usize * BYTES_PER_PIXEL];
    let alpha = if config.show_background || config.show_pods {
        230
    } else {
        0
    };
    let capsule = if alternate_accent {
        [28u8, 78, 30]
    } else {
        parse_hex_bgr(if config.show_background {
            &config.background_color_hex
        } else {
            &config.pod_color_hex
        })
        .unwrap_or([38u8, 31, 24])
    };
    let radius = if config.show_pods { 13 } else { 0 };
    for y in 0..height {
        for x in 0..width {
            let cx = x.clamp(radius, width - radius - 1);
            let cy = y.clamp(radius, height - radius - 1);
            let dx = x - cx;
            let dy = y - cy;
            if dx * dx + dy * dy <= radius * radius {
                put_pixel_sized(&mut pixels, width, x, y, capsule, alpha);
            }
        }
    }
    let scale = 2;
    let text_width = char_count * 12 - if char_count > 0 { 2 } else { 0 };
    let origin_x = ((width - text_width) / 2).max(16);
    let origin_y = (height - 7 * scale) / 2;
    for (index, (ch, color)) in glyphs.iter().enumerate() {
        let rows = glyph(*ch);
        for (row, bits) in rows.iter().enumerate() {
            for col in 0..5 {
                if bits & (1 << (4 - col)) != 0 {
                    for sy in 0..scale {
                        for sx in 0..scale {
                            put_pixel_sized(
                                &mut pixels,
                                width,
                                origin_x + index as i32 * 12 + col * scale + sx,
                                origin_y + row as i32 * scale + sy,
                                *color,
                                255,
                            );
                            if config.is_text_bold && sx == scale - 1 {
                                put_pixel_sized(&mut pixels, width,
                                    origin_x + index as i32 * 12 + col * scale + sx + 1,
                                    origin_y + row as i32 * scale + sy, *color, 255);
                            }
                        }
                    }
                }
            }
        }
    }
    (pixels, width, height)
}

fn parse_hex_bgr(input: &str) -> Option<[u8; 3]> {
    let hex = input.trim_start_matches('#');
    let rgb = if hex.len() == 8 { &hex[2..] } else { hex };
    if rgb.len() != 6 {
        return None;
    }
    let red = u8::from_str_radix(&rgb[0..2], 16).ok()?;
    let green = u8::from_str_radix(&rgb[2..4], 16).ok()?;
    let blue = u8::from_str_radix(&rgb[4..6], 16).ok()?;
    Some([blue, green, red])
}

fn put_pixel_sized(pixels: &mut [u8], width: i32, x: i32, y: i32, bgr: [u8; 3], alpha: u8) {
    let offset = (y as usize * width as usize + x as usize) * BYTES_PER_PIXEL;
    let a = alpha as u16;
    pixels[offset] = (bgr[0] as u16 * a / 255) as u8;
    pixels[offset + 1] = (bgr[1] as u16 * a / 255) as u8;
    pixels[offset + 2] = (bgr[2] as u16 * a / 255) as u8;
    pixels[offset + 3] = alpha;
}

fn glyph(ch: char) -> [u8; 7] {
    match ch.to_ascii_uppercase() {
        'A' => [14, 17, 17, 31, 17, 17, 17],
        'B' => [30, 17, 17, 30, 17, 17, 30],
        'C' => [14, 17, 16, 16, 16, 17, 14],
        'D' => [30, 17, 17, 17, 17, 17, 30],
        'E' => [31, 16, 16, 30, 16, 16, 31],
        'F' => [31, 16, 16, 30, 16, 16, 16],
        'G' => [14, 17, 16, 23, 17, 17, 15],
        'H' => [17, 17, 17, 31, 17, 17, 17],
        'I' => [14, 4, 4, 4, 4, 4, 14],
        'K' => [17, 18, 20, 24, 20, 18, 17],
        'M' => [17, 27, 21, 21, 17, 17, 17],
        'N' => [17, 25, 25, 21, 19, 19, 17],
        'O' => [14, 17, 17, 17, 17, 17, 14],
        'P' => [30, 17, 17, 30, 16, 16, 16],
        'R' => [30, 17, 17, 30, 20, 18, 17],
        'T' => [31, 4, 4, 4, 4, 4, 4],
        'U' => [17, 17, 17, 17, 17, 17, 14],
        'S' => [15, 16, 16, 14, 1, 1, 30],
        'W' => [17, 17, 17, 21, 21, 21, 10],
        '0' => [14, 17, 19, 21, 25, 17, 14],
        '1' => [4, 12, 4, 4, 4, 4, 14],
        '2' => [14, 17, 1, 2, 4, 8, 31],
        '3' => [30, 1, 1, 14, 1, 1, 30],
        '4' => [2, 6, 10, 18, 31, 2, 2],
        '5' => [31, 16, 16, 30, 1, 1, 30],
        '6' => [14, 16, 16, 30, 17, 17, 14],
        '7' => [31, 1, 2, 4, 8, 8, 8],
        '8' => [14, 17, 17, 14, 17, 17, 14],
        '9' => [14, 17, 17, 15, 1, 1, 14],
        '%' => [17, 2, 4, 8, 17, 0, 0],
        '.' => [0, 0, 0, 0, 0, 12, 12],
        ':' => [0, 12, 12, 0, 12, 12, 0],
        '/' => [1, 2, 2, 4, 8, 8, 16],
        ' ' => [0; 7],
        _ => [0; 7],
    }
}

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
#[cfg(any(not(windows), test))]
pub fn scale_bitmap(pixels: &[u8], scale_percent: u32) -> (Vec<u8>, i32, i32) {
    scale_surface(pixels, WIDTH, HEIGHT, scale_percent)
}

pub fn scale_surface(
    pixels: &[u8],
    source_width: i32,
    source_height: i32,
    scale_percent: u32,
) -> (Vec<u8>, i32, i32) {
    let scale_percent = scale_percent.clamp(50, 300);
    let width = ((source_width as u32 * scale_percent + 50) / 100) as i32;
    let height = ((source_height as u32 * scale_percent + 50) / 100) as i32;
    let mut scaled = vec![0; width as usize * height as usize * BYTES_PER_PIXEL];
    for y in 0..height {
        let source_y = y as usize * source_height as usize / height as usize;
        for x in 0..width {
            let source_x = x as usize * source_width as usize / width as usize;
            let source = (source_y * source_width as usize + source_x) * BYTES_PER_PIXEL;
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

    #[test]
    fn metric_row_respects_selection_and_contains_live_text_pixels() {
        let metrics = system_monitor_core::SystemMetrics {
            cpu_usage_percent: 37.0,
            ..system_monitor_core::SystemMetrics::default()
        };
        let config = system_monitor_core::AppConfig {
            show_ram: false,
            show_gpu: false,
            show_temp: false,
            show_disk: false,
            show_disk_speed: false,
            show_net_up: false,
            show_net_down: false,
            ..system_monitor_core::AppConfig::default()
        };
        let (pixels, width, height) = build_metrics_bitmap(&metrics, &config, None, None, false);
        assert_eq!(height, HEIGHT);
        assert!(width > 52);
        assert!(pixels.as_chunks::<4>().0.contains(&[255, 255, 255, 255]));

        let empty = system_monitor_core::AppConfig {
            show_cpu: false,
            show_ram: false,
            show_gpu: false,
            show_temp: false,
            show_disk: false,
            show_disk_speed: false,
            show_net_up: false,
            show_net_down: false,
            ..system_monitor_core::AppConfig::default()
        };
        let (_, empty_width, _) = build_metrics_bitmap(&metrics, &empty, None, None, false);
        assert_eq!(empty_width, 52);
    }
}
