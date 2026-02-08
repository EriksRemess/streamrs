use chrono::Local;
use image::imageops::{FilterType::Lanczos3, overlay, resize};
use image::{DynamicImage, GrayImage, Luma, Rgba, RgbaImage, load_from_memory};
use resvg::tiny_skia;
use resvg::usvg;
use serde::Deserialize;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

const CLOCK_RENDER_SIZE: u32 = 256;
const CLOCK_ICON_ALIAS: &str = "clock.svg";
const CLOCK_ICON_PREFIX: &str = "clock://hh:mm";
const CLOCK_BACKGROUND_ICON: &str = "blank.png";
const CLOCK_FALLBACK_BACKGROUND_COLOR: &str = "#1f1f1f";
const CLOCK_VIEWBOX_SIZE: i32 = 72;
const CLOCK_DIGIT_WIDTH: i32 = 12;
const CLOCK_DIGIT_HEIGHT: i32 = 24;
const CLOCK_COLON_WIDTH: i32 = 4;
const CLOCK_CHAR_GAP: i32 = 2;
const TEMPLATE_RENDER_WIDTH: u32 = 1560;
const TEMPLATE_RENDER_HEIGHT: u32 = 1108;

#[derive(Debug)]
struct CliArgs {
    blank_svg: PathBuf,
    config: PathBuf,
    image_dir: PathBuf,
    output: PathBuf,
    width: u32,
    height: u32,
    icon_inset: i32,
    bottom_row_y_offset: i32,
    bottom_row_extra_inset: i32,
    icon_content_shrink_x: i32,
    icon_content_shrink_y: i32,
    icon_mask_expand: i32,
    icon_payload_offset_x: i32,
    icon_payload_offset_y: i32,
    evaluate_status: bool,
}

#[derive(Debug, Deserialize)]
struct Config {
    keys: Vec<KeyBinding>,
}

#[derive(Debug, Deserialize)]
struct KeyBinding {
    icon: String,
    icon_on: Option<String>,
    icon_off: Option<String>,
    status: Option<String>,
}

#[derive(Clone)]
struct Slot {
    x0: u32,
    y0: u32,
    x1: u32,
    y1: u32,
    width: u32,
    height: u32,
    cx: f32,
    cy: f32,
    mask: GrayImage,
}

fn print_usage(program: &str) {
    eprintln!(
        "Usage: {program} [--blank-svg <path>] [--config <path>] [--image-dir <path>] [--output <path>]
                 [--width <px>] [--height <px>] [--icon-inset <n>] [--bottom-row-y-offset <n>]
                 [--bottom-row-extra-inset <n>] [--icon-content-shrink-x <n>] [--icon-content-shrink-y <n>]
                 [--icon-mask-expand <n>] [--icon-payload-offset-x <n>] [--icon-payload-offset-y <n>]
                 [--evaluate-status]"
    );
}

fn home_dir() -> Result<PathBuf, String> {
    env::var_os("HOME")
        .map(PathBuf::from)
        .ok_or_else(|| "HOME is not set".to_string())
}

fn parse_i32(value: &str, name: &str) -> Result<i32, String> {
    value
        .parse::<i32>()
        .map_err(|e| format!("Invalid value for {name}: {e}"))
}

fn parse_u32(value: &str, name: &str) -> Result<u32, String> {
    value
        .parse::<u32>()
        .map_err(|e| format!("Invalid value for {name}: {e}"))
}

fn parse_args() -> Result<CliArgs, String> {
    let home = home_dir()?;
    let mut args = CliArgs {
        blank_svg: PathBuf::from("scripts/streamdeck.svg"),
        config: home.join(".config/streamrs/default.toml"),
        image_dir: home.join(".local/share/streamrs/default"),
        output: PathBuf::from("dist/mock-current-config.png"),
        width: 512,
        height: 364,
        icon_inset: 8,
        bottom_row_y_offset: 0,
        bottom_row_extra_inset: 1,
        icon_content_shrink_x: 13,
        icon_content_shrink_y: 13,
        icon_mask_expand: 10,
        icon_payload_offset_x: 0,
        icon_payload_offset_y: 0,
        evaluate_status: false,
    };

    let mut it = env::args().skip(1);
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--blank-svg" => {
                args.blank_svg = PathBuf::from(
                    it.next()
                        .ok_or_else(|| "Missing value for --blank-svg".to_string())?,
                )
            }
            "--config" => {
                args.config = PathBuf::from(
                    it.next()
                        .ok_or_else(|| "Missing value for --config".to_string())?,
                )
            }
            "--image-dir" => {
                args.image_dir = PathBuf::from(
                    it.next()
                        .ok_or_else(|| "Missing value for --image-dir".to_string())?,
                )
            }
            "--output" => {
                args.output = PathBuf::from(
                    it.next()
                        .ok_or_else(|| "Missing value for --output".to_string())?,
                )
            }
            "--width" => {
                args.width = parse_u32(
                    &it.next()
                        .ok_or_else(|| "Missing value for --width".to_string())?,
                    "--width",
                )?
            }
            "--height" => {
                args.height = parse_u32(
                    &it.next()
                        .ok_or_else(|| "Missing value for --height".to_string())?,
                    "--height",
                )?
            }
            "--icon-inset" => {
                args.icon_inset = parse_i32(
                    &it.next()
                        .ok_or_else(|| "Missing value for --icon-inset".to_string())?,
                    "--icon-inset",
                )?
            }
            "--bottom-row-y-offset" => {
                args.bottom_row_y_offset = parse_i32(
                    &it.next()
                        .ok_or_else(|| "Missing value for --bottom-row-y-offset".to_string())?,
                    "--bottom-row-y-offset",
                )?
            }
            "--bottom-row-extra-inset" => {
                args.bottom_row_extra_inset = parse_i32(
                    &it.next()
                        .ok_or_else(|| "Missing value for --bottom-row-extra-inset".to_string())?,
                    "--bottom-row-extra-inset",
                )?
            }
            "--icon-content-shrink-x" => {
                args.icon_content_shrink_x = parse_i32(
                    &it.next()
                        .ok_or_else(|| "Missing value for --icon-content-shrink-x".to_string())?,
                    "--icon-content-shrink-x",
                )?
            }
            "--icon-content-shrink-y" => {
                args.icon_content_shrink_y = parse_i32(
                    &it.next()
                        .ok_or_else(|| "Missing value for --icon-content-shrink-y".to_string())?,
                    "--icon-content-shrink-y",
                )?
            }
            "--icon-mask-expand" => {
                args.icon_mask_expand = parse_i32(
                    &it.next()
                        .ok_or_else(|| "Missing value for --icon-mask-expand".to_string())?,
                    "--icon-mask-expand",
                )?
            }
            "--icon-payload-offset-x" => {
                args.icon_payload_offset_x = parse_i32(
                    &it.next()
                        .ok_or_else(|| "Missing value for --icon-payload-offset-x".to_string())?,
                    "--icon-payload-offset-x",
                )?
            }
            "--icon-payload-offset-y" => {
                args.icon_payload_offset_y = parse_i32(
                    &it.next()
                        .ok_or_else(|| "Missing value for --icon-payload-offset-y".to_string())?,
                    "--icon-payload-offset-y",
                )?
            }
            "--evaluate-status" => args.evaluate_status = true,
            "--help" | "-h" => {
                print_usage(&env::args().next().unwrap_or_else(|| "streamrs-preview".to_string()));
                std::process::exit(0);
            }
            _ => return Err(format!("Unknown argument: {arg}")),
        }
    }
    Ok(args)
}

fn run_status_command(command: &str) -> bool {
    Command::new("sh")
        .arg("-c")
        .arg(command)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn current_clock_text() -> String {
    Local::now().format("%H:%M").to_string()
}

fn seven_segment_pattern(ch: char) -> [bool; 7] {
    match ch {
        '0' => [true, true, true, true, true, true, false],
        '1' => [false, true, true, false, false, false, false],
        '2' => [true, true, false, true, true, false, true],
        '3' => [true, true, true, true, false, false, true],
        '4' => [false, true, true, false, false, true, true],
        '5' => [true, false, true, true, false, true, true],
        '6' => [true, false, true, true, true, true, true],
        '7' => [true, true, true, false, false, false, false],
        '8' => [true, true, true, true, true, true, true],
        '9' => [true, true, true, true, false, true, true],
        _ => [false; 7],
    }
}

fn push_clock_digit_rects(svg: &mut String, x: i32, y: i32, ch: char) {
    let segments = seven_segment_pattern(ch);
    let segment_rects = [
        (x + 2, y, 8, 2),
        (x + 10, y + 2, 2, 8),
        (x + 10, y + 14, 2, 8),
        (x + 2, y + 22, 8, 2),
        (x, y + 14, 2, 8),
        (x, y + 2, 2, 8),
        (x + 2, y + 11, 8, 2),
    ];
    for (enabled, (rx, ry, rw, rh)) in segments.iter().zip(segment_rects) {
        let fill = if *enabled { "#ffffff" } else { "#2f2f2f" };
        svg.push_str(&format!(
            r##"<rect x="{rx}" y="{ry}" width="{rw}" height="{rh}" fill="{fill}"/>"##
        ));
    }
}

fn clock_char_width(ch: char) -> i32 {
    if ch == ':' {
        CLOCK_COLON_WIDTH
    } else {
        CLOCK_DIGIT_WIDTH
    }
}

fn clock_background_svg(image_dir: &Path) -> String {
    if image_dir.join(CLOCK_BACKGROUND_ICON).is_file() {
        format!(r##"<image href="{CLOCK_BACKGROUND_ICON}" x="0" y="0" width="72" height="72"/>"##)
    } else {
        format!(
            r##"<rect x="0" y="0" width="72" height="72" fill="{CLOCK_FALLBACK_BACKGROUND_COLOR}"/>"##
        )
    }
}

fn render_clock_segments_svg(image_dir: &Path, text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let gaps = chars.len().saturating_sub(1) as i32;
    let total_width =
        chars.iter().map(|ch| clock_char_width(*ch)).sum::<i32>() + (gaps * CLOCK_CHAR_GAP);
    let mut x = (CLOCK_VIEWBOX_SIZE - total_width) / 2;
    let y = (CLOCK_VIEWBOX_SIZE - CLOCK_DIGIT_HEIGHT) / 2;
    let mut glyphs = String::new();

    for ch in chars {
        if ch == ':' {
            glyphs.push_str(&format!(
                r##"<rect x="{}" y="{}" width="2" height="2" fill="#ffffff"/><rect x="{}" y="{}" width="2" height="2" fill="#ffffff"/>"##,
                x + 1,
                y + 8,
                x + 1,
                y + 16
            ));
            x += clock_char_width(ch) + CLOCK_CHAR_GAP;
            continue;
        }
        push_clock_digit_rects(&mut glyphs, x, y, ch);
        x += clock_char_width(ch) + CLOCK_CHAR_GAP;
    }

    format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="72" height="72" viewBox="0 0 72 72">
{background}
{glyphs}
</svg>"##,
        background = clock_background_svg(image_dir),
        glyphs = glyphs
    )
}

fn load_svg_data(
    label: &str,
    svg_data: &[u8],
    resources_dir: Option<&Path>,
    target_w: u32,
    target_h: u32,
) -> Result<DynamicImage, String> {
    let mut options = usvg::Options::default();
    options.resources_dir = resources_dir.map(|p| p.to_path_buf());
    let tree = usvg::Tree::from_data(svg_data, &options)
        .map_err(|e| format!("Failed to parse SVG '{label}': {e}"))?;

    let mut pixmap = tiny_skia::Pixmap::new(target_w, target_h)
        .ok_or_else(|| format!("Failed to allocate target for '{label}'"))?;

    let size = tree.size();
    let scale = (target_w as f32 / size.width()).min(target_h as f32 / size.height());
    let x_offset = (target_w as f32 - size.width() * scale) / 2.0;
    let y_offset = (target_h as f32 - size.height() * scale) / 2.0;
    let transform =
        tiny_skia::Transform::from_scale(scale, scale).post_translate(x_offset, y_offset);
    resvg::render(&tree, transform, &mut pixmap.as_mut());

    let rgba = image::RgbaImage::from_raw(target_w, target_h, pixmap.take())
        .ok_or_else(|| format!("Failed to build rasterized image for '{label}'"))?;
    Ok(DynamicImage::ImageRgba8(rgba))
}

fn render_blank_base(path: &Path, width: u32, height: u32) -> Result<RgbaImage, String> {
    let data =
        fs::read(path).map_err(|e| format!("Failed to read blank SVG '{}': {e}", path.display()))?;
    // Render at least at template resolution, then downscale with high-quality filtering.
    // This avoids visibly blocky sampling from embedded raster textures in the SVG.
    let render_w = width.max(TEMPLATE_RENDER_WIDTH);
    let render_h = height.max(TEMPLATE_RENDER_HEIGHT);
    let img = load_svg_data(
        &path.display().to_string(),
        &data,
        path.parent(),
        render_w,
        render_h,
    )?;
    let rendered = img.to_rgba8();
    if render_w == width && render_h == height {
        return Ok(rendered);
    }
    Ok(resize(&rendered, width, height, Lanczos3))
}

fn load_icon_image(icon_name: &str, image_dir: &Path) -> Result<RgbaImage, String> {
    if icon_name.eq_ignore_ascii_case(CLOCK_ICON_ALIAS)
        || icon_name.eq_ignore_ascii_case(CLOCK_ICON_PREFIX)
    {
        let clock_svg = render_clock_segments_svg(image_dir, &current_clock_text());
        let img = load_svg_data(
            CLOCK_ICON_ALIAS,
            clock_svg.as_bytes(),
            Some(image_dir),
            CLOCK_RENDER_SIZE,
            CLOCK_RENDER_SIZE,
        )?;
        return Ok(img.to_rgba8());
    }

    let path = image_dir.join(icon_name);
    let data = fs::read(&path).map_err(|e| format!("Failed to read icon '{}': {e}", path.display()))?;
    if path
        .extension()
        .and_then(|x| x.to_str())
        .unwrap_or_default()
        .eq_ignore_ascii_case("svg")
    {
        let img = load_svg_data(
            &path.display().to_string(),
            &data,
            path.parent(),
            CLOCK_RENDER_SIZE,
            CLOCK_RENDER_SIZE,
        )?;
        return Ok(img.to_rgba8());
    }
    load_from_memory(&data)
        .map_err(|e| format!("Failed to decode icon '{}': {e}", path.display()))
        .map(|d| d.to_rgba8())
}

fn choose_icon_name(key: &KeyBinding, evaluate_status: bool) -> String {
    let icon = key.icon.clone();
    let status = key
        .status
        .as_ref()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    if !evaluate_status || status.is_none() {
        return icon;
    }
    let is_on = run_status_command(status.unwrap().as_str());
    let on = key
        .icon_on
        .as_ref()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    let off = key
        .icon_off
        .as_ref()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    if is_on {
        on.unwrap_or(icon)
    } else {
        off.unwrap_or(icon)
    }
}

fn detect_key_slots(base: &RgbaImage, use_scaled_thresholds: bool) -> Result<Vec<Slot>, String> {
    let w = base.width() as usize;
    let h = base.height() as usize;
    let mut dark = vec![false; w * h];
    for y in 0..h {
        for x in 0..w {
            let p = base.get_pixel(x as u32, y as u32);
            let g = ((p[0] as u16 + p[1] as u16 + p[2] as u16) / 3) as u8;
            dark[y * w + x] = g < 20;
        }
    }

    let (min_area, max_area, min_w, max_w, min_h, max_h) = if use_scaled_thresholds {
        let sx = base.width() as f32 / TEMPLATE_RENDER_WIDTH as f32;
        let sy = base.height() as f32 / TEMPLATE_RENDER_HEIGHT as f32;
        let s = sx.min(sy);
        (
            (20000.0 * s * s * 0.6) as usize,
            (70000.0 * s * s * 1.6) as usize,
            (150.0 * s * 0.7) as usize,
            (260.0 * s * 1.4) as usize,
            (150.0 * s * 0.7) as usize,
            (260.0 * s * 1.4) as usize,
        )
    } else {
        (20000, 70000, 150, 260, 150, 260)
    };

    let mut visited = vec![false; w * h];
    let mut slots = Vec::<Slot>::new();
    for y in 0..h {
        for x in 0..w {
            let idx = y * w + x;
            if !dark[idx] || visited[idx] {
                continue;
            }
            let mut stack = vec![(x, y)];
            visited[idx] = true;
            let mut points = Vec::new();
            let mut minx = x;
            let mut maxx = x;
            let mut miny = y;
            let mut maxy = y;
            while let Some((cx, cy)) = stack.pop() {
                points.push((cx, cy));
                minx = minx.min(cx);
                maxx = maxx.max(cx);
                miny = miny.min(cy);
                maxy = maxy.max(cy);

                let neighbors = [
                    (cx.wrapping_sub(1), cy),
                    (cx + 1, cy),
                    (cx, cy.wrapping_sub(1)),
                    (cx, cy + 1),
                ];
                for (nx, ny) in neighbors {
                    if nx >= w || ny >= h {
                        continue;
                    }
                    let nidx = ny * w + nx;
                    if dark[nidx] && !visited[nidx] {
                        visited[nidx] = true;
                        stack.push((nx, ny));
                    }
                }
            }

            let area = points.len();
            let bw = maxx - minx + 1;
            let bh = maxy - miny + 1;
            if area < min_area || area > max_area {
                continue;
            }
            if bw < min_w || bw > max_w || bh < min_h || bh > max_h {
                continue;
            }
            let fill = area as f32 / (bw * bh) as f32;
            if !(0.6..=1.1).contains(&fill) {
                continue;
            }

            let mut mask = GrayImage::new(bw as u32, bh as u32);
            for (px, py) in points {
                mask.put_pixel((px - minx) as u32, (py - miny) as u32, Luma([255]));
            }
            let x0 = minx as u32;
            let y0 = miny as u32;
            let x1 = (maxx + 1) as u32;
            let y1 = (maxy + 1) as u32;
            slots.push(Slot {
                x0,
                y0,
                x1,
                y1,
                width: bw as u32,
                height: bh as u32,
                cx: (x0 + x1 - 1) as f32 / 2.0,
                cy: (y0 + y1 - 1) as f32 / 2.0,
                mask,
            });
        }
    }

    if slots.len() != 15 {
        return Err(format!("Expected 15 key slots, found {}", slots.len()));
    }
    slots.sort_by(|a, b| {
        let ar = (a.cy / 40.0).round();
        let br = (b.cy / 40.0).round();
        ar.partial_cmp(&br)
            .unwrap()
            .then_with(|| a.cx.partial_cmp(&b.cx).unwrap())
    });
    Ok(slots)
}

fn scale_slots(template_slots: &[Slot], width: u32, height: u32) -> Vec<Slot> {
    let sx = width as f32 / TEMPLATE_RENDER_WIDTH as f32;
    let sy = height as f32 / TEMPLATE_RENDER_HEIGHT as f32;
    template_slots
        .iter()
        .map(|s| {
            let mut x0 = (s.x0 as f32 * sx).round() as i32;
            let mut y0 = (s.y0 as f32 * sy).round() as i32;
            let mut x1 = (s.x1 as f32 * sx).round() as i32;
            let mut y1 = (s.y1 as f32 * sy).round() as i32;
            x0 = x0.clamp(0, width as i32 - 1);
            y0 = y0.clamp(0, height as i32 - 1);
            x1 = x1.clamp(x0 + 1, width as i32);
            y1 = y1.clamp(y0 + 1, height as i32);
            let w = (x1 - x0) as u32;
            let h = (y1 - y0) as u32;
            let mask = resize(&s.mask, w, h, Lanczos3);
            Slot {
                x0: x0 as u32,
                y0: y0 as u32,
                x1: x1 as u32,
                y1: y1 as u32,
                width: w,
                height: h,
                cx: (x0 + x1 - 1) as f32 / 2.0,
                cy: (y0 + y1 - 1) as f32 / 2.0,
                mask,
            }
        })
        .collect()
}

fn rounded_rect_mask(width: u32, height: u32, radius: u32) -> GrayImage {
    let mut out = GrayImage::new(width, height);
    let r = radius.min(width.min(height) / 2) as i32;
    if r <= 0 {
        for p in out.pixels_mut() {
            *p = Luma([255]);
        }
        return out;
    }
    let w = width as i32;
    let h = height as i32;
    for y in 0..h {
        for x in 0..w {
            let in_h = x >= r && x < (w - r);
            let in_v = y >= r && y < (h - r);
            let inside = if in_h || in_v {
                true
            } else {
                let cx = if x < r { r - 1 } else { w - r };
                let cy = if y < r { r - 1 } else { h - r };
                let dx = x - cx;
                let dy = y - cy;
                dx * dx + dy * dy <= r * r
            };
            if inside {
                out.put_pixel(x as u32, y as u32, Luma([255]));
            }
        }
    }
    out
}

fn paste_gray(dst: &mut GrayImage, src: &GrayImage, x: i32, y: i32) {
    for sy in 0..src.height() as i32 {
        for sx in 0..src.width() as i32 {
            let dx = x + sx;
            let dy = y + sy;
            if dx < 0 || dy < 0 || dx >= dst.width() as i32 || dy >= dst.height() as i32 {
                continue;
            }
            let p = src.get_pixel(sx as u32, sy as u32);
            dst.put_pixel(dx as u32, dy as u32, *p);
        }
    }
}

fn multiply_gray(a: &GrayImage, b: &GrayImage) -> GrayImage {
    let mut out = GrayImage::new(a.width(), a.height());
    for y in 0..a.height() {
        for x in 0..a.width() {
            let av = a.get_pixel(x, y)[0] as u16;
            let bv = b.get_pixel(x, y)[0] as u16;
            out.put_pixel(x, y, Luma([((av * bv) / 255) as u8]));
        }
    }
    out
}

fn apply_mask_to_alpha(img: &mut RgbaImage, mask: &GrayImage) {
    for y in 0..img.height() {
        for x in 0..img.width() {
            let m = mask.get_pixel(x, y)[0] as u16;
            let p = img.get_pixel_mut(x, y);
            p[3] = ((p[3] as u16 * m) / 255) as u8;
        }
    }
}

fn compose_preview(args: &CliArgs) -> Result<(), String> {
    let raw = fs::read_to_string(&args.config)
        .map_err(|e| format!("Failed to read config '{}': {e}", args.config.display()))?;
    let config: Config = toml::from_str(&raw)
        .map_err(|e| format!("Failed to parse config '{}': {e}", args.config.display()))?;
    let mut base = render_blank_base(&args.blank_svg, args.width, args.height)?;
    let template_base = render_blank_base(&args.blank_svg, TEMPLATE_RENDER_WIDTH, TEMPLATE_RENDER_HEIGHT)?;
    let template_slots = detect_key_slots(&template_base, false)?;
    let slots = scale_slots(&template_slots, args.width, args.height);

    for (idx, slot) in slots.iter().enumerate() {
        if idx >= config.keys.len() {
            continue;
        }
        let icon_name = choose_icon_name(&config.keys[idx], args.evaluate_status);
        let icon = load_icon_image(&icon_name, &args.image_dir).unwrap_or_else(|_| {
            let mut img = RgbaImage::new(CLOCK_RENDER_SIZE, CLOCK_RENDER_SIZE);
            for p in img.pixels_mut() {
                *p = Rgba([0x20, 0x20, 0x20, 0xFF]);
            }
            img
        });

        let row_index = (idx / 5) as i32;
        let row_icon_inset =
            args.icon_inset + if row_index == 2 { args.bottom_row_extra_inset } else { 0 };
        let slot_min = slot.width.min(slot.height) as i32;
        let mut inset_px = ((row_icon_inset as f32 / 72.0) * slot_min as f32).round() as i32;
        inset_px = inset_px.clamp(0, slot_min / 2 - 1);

        let inner_w = (slot.width as i32 - inset_px * 2).max(1);
        let inner_h = (slot.height as i32 - inset_px * 2).max(1);
        let expand_px = args.icon_mask_expand.max(0);
        let box_w = (inner_w + expand_px * 2).min(slot.width as i32).max(1);
        let box_h = (inner_h + expand_px * 2).min(slot.height as i32).max(1);

        let content_w = (box_w - args.icon_content_shrink_x.max(0)).max(1);
        let content_h = (box_h - args.icon_content_shrink_y.max(0)).max(1);

        let mut fitted_inner = resize(&icon, content_w as u32, content_h as u32, Lanczos3);
        let content_radius = ((content_w.min(content_h) as f32) * 0.16).round().max(2.0) as u32;
        let content_round_mask = rounded_rect_mask(content_w as u32, content_h as u32, content_radius);
        apply_mask_to_alpha(&mut fitted_inner, &content_round_mask);

        let mut fitted = RgbaImage::new(slot.width, slot.height);
        let offset_x = ((slot.width as i32 - box_w) / 2).max(0);
        let offset_y = ((slot.height as i32 - box_h) / 2).max(0);
        let content_offset_x = offset_x + ((box_w - content_w) / 2).max(0);
        let content_offset_y = offset_y + ((box_h - content_h) / 2).max(0);
        overlay(
            &mut fitted,
            &fitted_inner,
            content_offset_x as i64,
            content_offset_y as i64,
        );

        let slot_mask = if inset_px > 0 || expand_px > 0 {
            let inner_mask = resize(&slot.mask, box_w as u32, box_h as u32, Lanczos3);
            let mut out = GrayImage::new(slot.width, slot.height);
            paste_gray(&mut out, &inner_mask, offset_x, offset_y);
            out
        } else {
            slot.mask.clone()
        };
        let radius = ((box_w.min(box_h) as f32) * 0.12).round().max(2.0) as u32;
        let rounded = rounded_rect_mask(box_w as u32, box_h as u32, radius);
        let mut rounded_slot = GrayImage::new(slot.width, slot.height);
        paste_gray(&mut rounded_slot, &rounded, offset_x, offset_y);
        let final_mask = multiply_gray(&slot_mask, &rounded_slot);
        apply_mask_to_alpha(&mut fitted, &final_mask);

        let mut x_target = slot.x0 as i32 + args.icon_payload_offset_x;
        let mut y_target = slot.y0 as i32
            + if row_index == 2 {
                args.bottom_row_y_offset
            } else {
                0
            }
            + args.icon_payload_offset_y;
        x_target = x_target.clamp(0, args.width as i32 - slot.width as i32);
        y_target = y_target.clamp(0, args.height as i32 - slot.height as i32);
        overlay(&mut base, &fitted, x_target as i64, y_target as i64);
    }

    if let Some(parent) = args.output.parent() {
        fs::create_dir_all(parent).map_err(|e| {
            format!(
                "Failed to create output directory '{}': {e}",
                parent.display()
            )
        })?;
    }
    base.save(&args.output)
        .map_err(|e| format!("Failed to save output '{}': {e}", args.output.display()))?;
    Ok(())
}

fn main() {
    let args = match parse_args() {
        Ok(v) => v,
        Err(e) => {
            eprintln!("Error: {e}");
            print_usage(&env::args().next().unwrap_or_else(|| "streamrs-preview".to_string()));
            std::process::exit(1);
        }
    };
    if let Err(e) = compose_preview(&args) {
        eprintln!("Error: {e}");
        std::process::exit(1);
    }
}
