use image::imageops::{FilterType::Lanczos3, overlay, resize};
use image::{ImageFormat, RgbaImage};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use streamrs::image::svg::load_svg_data;

const DEFAULT_PADDING_RATIO: f32 = 0.15;
const LOGO_ANALYSIS_SIZE: u32 = 512;
const ACCENT_EDGE_RATIO: f32 = 0.12;
const ACCENT_SPAN_RATIO: f32 = 0.58;

struct EmbeddedBlank {
    name: &'static str,
    png_bytes: &'static [u8],
}

const EMBEDDED_BLANKS: &[EmbeddedBlank] = &[
    EmbeddedBlank {
        name: "blank.png",
        png_bytes: include_bytes!("../../all_images/blank.png"),
    },
    EmbeddedBlank {
        name: "blank_2.png",
        png_bytes: include_bytes!("../../all_images/blank_2.png"),
    },
    EmbeddedBlank {
        name: "blank_3.png",
        png_bytes: include_bytes!("../../all_images/blank_3.png"),
    },
    EmbeddedBlank {
        name: "blank_4.png",
        png_bytes: include_bytes!("../../all_images/blank_4.png"),
    },
    EmbeddedBlank {
        name: "blank_5.png",
        png_bytes: include_bytes!("../../all_images/blank_5.png"),
    },
    EmbeddedBlank {
        name: "blank_6.png",
        png_bytes: include_bytes!("../../all_images/blank_6.png"),
    },
    EmbeddedBlank {
        name: "blank_7.png",
        png_bytes: include_bytes!("../../all_images/blank_7.png"),
    },
    EmbeddedBlank {
        name: "blank_8.png",
        png_bytes: include_bytes!("../../all_images/blank_8.png"),
    },
    EmbeddedBlank {
        name: "blank_9.png",
        png_bytes: include_bytes!("../../all_images/blank_9.png"),
    },
];

#[derive(Debug)]
struct CliArgs {
    logo: PathBuf,
    output: PathBuf,
    padding_ratio: f32,
}

#[derive(Clone)]
struct LoadedBlank {
    name: &'static str,
    image: RgbaImage,
    accent_color: [f32; 3],
}

#[derive(Clone, Copy, Default)]
struct ColorBin {
    weight: f32,
    sum_r: f32,
    sum_g: f32,
    sum_b: f32,
}

fn print_usage(program: &str) {
    eprintln!("Usage: {program} <logo.svg|logo.png> [--output <path>] [--padding <ratio>]");
    eprintln!("  --padding defaults to {:.2} (15%)", DEFAULT_PADDING_RATIO);
    eprintln!(
        "  default output: ~/.local/share/streamrs/default/<logo>-icon.png (auto-suffixed with -2, -3, ... if needed)"
    );
}

fn default_stem(path: &Path, fallback: &str) -> String {
    path.file_stem()
        .and_then(|s| s.to_str())
        .filter(|s| !s.is_empty())
        .unwrap_or(fallback)
        .to_string()
}

fn default_output_dir() -> PathBuf {
    let home = env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    home.join(".local/share/streamrs/default")
}

fn unique_output_path(dir: &Path, stem: &str) -> PathBuf {
    let direct = dir.join(format!("{stem}.png"));
    if !direct.exists() {
        return direct;
    }

    for index in 2.. {
        let candidate = dir.join(format!("{stem}-{index}.png"));
        if !candidate.exists() {
            return candidate;
        }
    }

    unreachable!("infinite output filename namespace exhausted")
}

fn default_output_path(logo: &Path) -> PathBuf {
    let logo_stem = default_stem(logo, "logo");
    let base_stem = format!("{logo_stem}-icon");
    unique_output_path(&default_output_dir(), &base_stem)
}

fn parse_args() -> Result<CliArgs, String> {
    let program = env::args()
        .next()
        .unwrap_or_else(|| "streamrs-icon-compose".to_string());
    let mut logo: Option<PathBuf> = None;
    let mut output: Option<PathBuf> = None;
    let mut padding_ratio = DEFAULT_PADDING_RATIO;

    let mut it = env::args().skip(1);
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--help" | "-h" => {
                print_usage(&program);
                std::process::exit(0);
            }
            "--output" | "-o" => {
                let value = it
                    .next()
                    .ok_or_else(|| "Missing value for --output".to_string())?;
                output = Some(PathBuf::from(value));
            }
            "--padding" => {
                let value = it
                    .next()
                    .ok_or_else(|| "Missing value for --padding".to_string())?;
                let parsed = value
                    .parse::<f32>()
                    .map_err(|_| format!("Invalid --padding value: {value}"))?;
                if !(0.0..0.5).contains(&parsed) {
                    return Err("--padding must be between 0.0 and 0.5 (exclusive)".to_string());
                }
                padding_ratio = parsed;
            }
            _ => {
                if logo.is_none() {
                    logo = Some(PathBuf::from(arg));
                } else {
                    return Err(format!("Unexpected argument: {arg}"));
                }
            }
        }
    }

    let logo = logo.ok_or_else(|| "Missing required <logo.svg|logo.png> path".to_string())?;
    let output = output.unwrap_or_else(|| default_output_path(&logo));

    Ok(CliArgs {
        logo,
        output,
        padding_ratio,
    })
}

fn has_extension(path: &Path, ext: &str) -> bool {
    path.extension()
        .and_then(|value| value.to_str())
        .is_some_and(|value| value.eq_ignore_ascii_case(ext))
}

fn fit_within(src_w: u32, src_h: u32, max_w: u32, max_h: u32) -> (u32, u32) {
    let src_w = src_w.max(1);
    let src_h = src_h.max(1);
    let max_w = max_w.max(1);
    let max_h = max_h.max(1);

    let scale_w = max_w as f32 / src_w as f32;
    let scale_h = max_h as f32 / src_h as f32;
    let scale = scale_w.min(scale_h);

    let width = ((src_w as f32) * scale).round().max(1.0) as u32;
    let height = ((src_h as f32) * scale).round().max(1.0) as u32;

    (width.min(max_w), height.min(max_h))
}

fn rgb_to_hsv(r: f32, g: f32, b: f32) -> (f32, f32, f32) {
    let max = r.max(g.max(b));
    let min = r.min(g.min(b));
    let delta = max - min;

    let hue = if delta == 0.0 {
        0.0
    } else if max == r {
        let h = ((g - b) / delta) % 6.0;
        60.0 * if h < 0.0 { h + 6.0 } else { h }
    } else if max == g {
        60.0 * (((b - r) / delta) + 2.0)
    } else {
        60.0 * (((r - g) / delta) + 4.0)
    };

    let saturation = if max == 0.0 { 0.0 } else { delta / max };
    let value = max;
    (hue, saturation, value)
}

fn dominant_color_from_histogram<F>(image: &RgbaImage, weight_fn: F) -> Option<[f32; 3]>
where
    F: Fn(f32, f32, f32, f32, f32, f32, f32) -> f32,
{
    const BINS_PER_CHANNEL: usize = 16;
    const BIN_COUNT: usize = BINS_PER_CHANNEL * BINS_PER_CHANNEL * BINS_PER_CHANNEL;

    let mut bins = [ColorBin::default(); BIN_COUNT];

    for pixel in image.pixels() {
        let alpha = pixel[3] as f32 / 255.0;
        if alpha <= 0.0 {
            continue;
        }

        let r = pixel[0] as f32 / 255.0;
        let g = pixel[1] as f32 / 255.0;
        let b = pixel[2] as f32 / 255.0;
        let (h, s, v) = rgb_to_hsv(r, g, b);

        let weight = weight_fn(r, g, b, alpha, h, s, v);
        if weight <= 0.0 {
            continue;
        }

        let ri = ((pixel[0] as usize) * BINS_PER_CHANNEL / 256).min(BINS_PER_CHANNEL - 1);
        let gi = ((pixel[1] as usize) * BINS_PER_CHANNEL / 256).min(BINS_PER_CHANNEL - 1);
        let bi = ((pixel[2] as usize) * BINS_PER_CHANNEL / 256).min(BINS_PER_CHANNEL - 1);
        let idx = (ri * BINS_PER_CHANNEL * BINS_PER_CHANNEL) + (gi * BINS_PER_CHANNEL) + bi;

        bins[idx].weight += weight;
        bins[idx].sum_r += r * weight;
        bins[idx].sum_g += g * weight;
        bins[idx].sum_b += b * weight;
    }

    let best = bins
        .iter()
        .filter(|bin| bin.weight > 0.0)
        .max_by(|a, b| a.weight.total_cmp(&b.weight))?;

    Some([
        best.sum_r / best.weight,
        best.sum_g / best.weight,
        best.sum_b / best.weight,
    ])
}

fn is_in_corner_accent_band(x: u32, y: u32, w: u32, h: u32) -> bool {
    let edge_w = ((w as f32) * ACCENT_EDGE_RATIO).round().max(1.0) as u32;
    let edge_h = ((h as f32) * ACCENT_EDGE_RATIO).round().max(1.0) as u32;
    let span_w = ((w as f32) * ACCENT_SPAN_RATIO).round().max(1.0) as u32;
    let span_h = ((h as f32) * ACCENT_SPAN_RATIO).round().max(1.0) as u32;

    let top_left = (x < edge_w && y < span_h) || (y < edge_h && x < span_w);
    let bottom_right = (x >= w.saturating_sub(edge_w) && y >= h.saturating_sub(span_h))
        || (y >= h.saturating_sub(edge_h) && x >= w.saturating_sub(span_w));

    top_left || bottom_right
}

fn fallback_average_color(image: &RgbaImage) -> Option<[f32; 3]> {
    let mut weight_sum = 0.0f32;
    let mut sum_r = 0.0f32;
    let mut sum_g = 0.0f32;
    let mut sum_b = 0.0f32;

    for pixel in image.pixels() {
        let alpha = pixel[3] as f32 / 255.0;
        if alpha <= 0.0 {
            continue;
        }
        let r = pixel[0] as f32 / 255.0;
        let g = pixel[1] as f32 / 255.0;
        let b = pixel[2] as f32 / 255.0;

        weight_sum += alpha;
        sum_r += r * alpha;
        sum_g += g * alpha;
        sum_b += b * alpha;
    }

    if weight_sum <= 0.0 {
        None
    } else {
        Some([sum_r / weight_sum, sum_g / weight_sum, sum_b / weight_sum])
    }
}

fn estimate_background_value(image: &RgbaImage) -> f32 {
    let w = image.width().max(1);
    let h = image.height().max(1);
    let x0 = ((w as f32) * 0.30).round() as u32;
    let x1 = ((w as f32) * 0.70).round().max((x0 + 1) as f32) as u32;
    let y0 = ((h as f32) * 0.30).round() as u32;
    let y1 = ((h as f32) * 0.70).round().max((y0 + 1) as f32) as u32;

    let mut sum_v = 0.0f32;
    let mut weight_sum = 0.0f32;

    for y in y0.min(h - 1)..y1.min(h) {
        for x in x0.min(w - 1)..x1.min(w) {
            let pixel = image.get_pixel(x, y);
            let alpha = pixel[3] as f32 / 255.0;
            if alpha <= 0.0 {
                continue;
            }
            let r = pixel[0] as f32 / 255.0;
            let g = pixel[1] as f32 / 255.0;
            let b = pixel[2] as f32 / 255.0;
            let (_, _, v) = rgb_to_hsv(r, g, b);
            sum_v += v * alpha;
            weight_sum += alpha;
        }
    }

    if weight_sum > 0.0 {
        sum_v / weight_sum
    } else {
        0.0
    }
}

fn blank_accent_color(image: &RgbaImage) -> Option<[f32; 3]> {
    let w = image.width().max(1);
    let h = image.height().max(1);
    let background_v = estimate_background_value(image);
    let accent_min_v = (background_v + 0.06).min(1.0);

    let mut weight_sum = 0.0f32;
    let mut sum_r = 0.0f32;
    let mut sum_g = 0.0f32;
    let mut sum_b = 0.0f32;

    for (x, y, pixel) in image.enumerate_pixels() {
        if !is_in_corner_accent_band(x, y, w, h) {
            continue;
        }

        let alpha = pixel[3] as f32 / 255.0;
        if alpha <= 0.0 {
            continue;
        }

        let r = pixel[0] as f32 / 255.0;
        let g = pixel[1] as f32 / 255.0;
        let b = pixel[2] as f32 / 255.0;
        let (_, s, v) = rgb_to_hsv(r, g, b);
        if v < accent_min_v {
            continue;
        }

        let brightness_boost = (v - background_v).max(0.0);
        let weight = alpha * (0.3 + (brightness_boost * 1.8) + (s * 0.7));
        if weight <= 0.0 {
            continue;
        }
        weight_sum += weight;
        sum_r += r * weight;
        sum_g += g * weight;
        sum_b += b * weight;
    }

    if weight_sum > 0.0 {
        return Some([sum_r / weight_sum, sum_g / weight_sum, sum_b / weight_sum]);
    }

    dominant_color_from_histogram(image, |_, _, _, alpha, _, s, v| {
        if s >= 0.22 && v >= 0.16 {
            alpha * (0.7 + (s * 0.9) + (v * 0.4))
        } else {
            0.0
        }
    })
    .or_else(|| {
        dominant_color_from_histogram(image, |_, _, _, alpha, _, s, v| {
            if s >= 0.12 && v >= 0.08 {
                alpha * (0.3 + s + (v * 0.2))
            } else {
                0.0
            }
        })
    })
    .or_else(|| fallback_average_color(image))
}

fn logo_main_color(image: &RgbaImage) -> Option<[f32; 3]> {
    dominant_color_from_histogram(image, |_, _, _, alpha, _, _, _| alpha)
        .or_else(|| {
            dominant_color_from_histogram(image, |_, _, _, alpha, _, _, v| alpha * (0.2 + v * 0.8))
        })
        .or_else(|| fallback_average_color(image))
}

fn srgb_to_linear(v: f32) -> f32 {
    if v <= 0.04045 {
        v / 12.92
    } else {
        ((v + 0.055) / 1.055).powf(2.4)
    }
}

fn color_distance(a: [f32; 3], b: [f32; 3]) -> f32 {
    let ar = srgb_to_linear(a[0]);
    let ag = srgb_to_linear(a[1]);
    let ab = srgb_to_linear(a[2]);
    let br = srgb_to_linear(b[0]);
    let bg = srgb_to_linear(b[1]);
    let bb = srgb_to_linear(b[2]);

    let dr = ar - br;
    let dg = ag - bg;
    let db = ab - bb;
    dr * dr + dg * dg + db * db
}

fn load_logo_source(logo_path: &Path) -> Result<RgbaImage, String> {
    if has_extension(logo_path, "svg") {
        let data = fs::read(logo_path)
            .map_err(|e| format!("Failed to read logo '{}': {e}", logo_path.display()))?;
        return load_svg_data(
            &logo_path.display().to_string(),
            &data,
            logo_path.parent(),
            LOGO_ANALYSIS_SIZE,
            LOGO_ANALYSIS_SIZE,
        );
    }

    if !has_extension(logo_path, "png") {
        return Err(format!(
            "Unsupported logo format for '{}': expected .svg or .png",
            logo_path.display()
        ));
    }

    image::open(logo_path)
        .map_err(|e| format!("Failed to decode logo '{}': {e}", logo_path.display()))
        .map(|img| img.to_rgba8())
}

fn load_embedded_blanks() -> Result<Vec<LoadedBlank>, String> {
    let mut loaded = Vec::with_capacity(EMBEDDED_BLANKS.len());

    for blank in EMBEDDED_BLANKS {
        let image = image::load_from_memory_with_format(blank.png_bytes, ImageFormat::Png)
            .map_err(|e| format!("Failed to decode embedded blank '{}': {e}", blank.name))?
            .to_rgba8();

        let accent_color = blank_accent_color(&image)
            .ok_or_else(|| format!("Failed to detect accent color for '{}'", blank.name))?;

        loaded.push(LoadedBlank {
            name: blank.name,
            image,
            accent_color,
        });
    }

    Ok(loaded)
}

fn choose_blank_for_logo(logo_color: [f32; 3], blanks: &[LoadedBlank]) -> &LoadedBlank {
    blanks
        .iter()
        .min_by(|a, b| {
            let da = color_distance(logo_color, a.accent_color);
            let db = color_distance(logo_color, b.accent_color);
            da.total_cmp(&db)
        })
        .expect("blanks must not be empty")
}

fn resize_logo_to_fit(logo: &RgbaImage, max_w: u32, max_h: u32) -> RgbaImage {
    let (target_w, target_h) = fit_within(logo.width(), logo.height(), max_w, max_h);
    resize(logo, target_w, target_h, Lanczos3)
}

fn compose(logo_path: &Path, output: &Path, padding_ratio: f32) -> Result<&'static str, String> {
    let blanks = load_embedded_blanks()?;
    if blanks.is_empty() {
        return Err("No embedded blanks available".to_string());
    }

    let logo_source = load_logo_source(logo_path)?;
    let logo_color = logo_main_color(&logo_source)
        .ok_or_else(|| format!("Failed to detect main color for '{}'", logo_path.display()))?;

    let selected_blank = choose_blank_for_logo(logo_color, &blanks);
    let mut background = selected_blank.image.clone();

    let bg_w = background.width().max(1);
    let bg_h = background.height().max(1);

    let side_padding = ((bg_w as f32) * padding_ratio).round() as u32;
    let vertical_padding = ((bg_h as f32) * padding_ratio).round() as u32;
    let max_logo_w = bg_w.saturating_sub(side_padding.saturating_mul(2)).max(1);
    let max_logo_h = bg_h
        .saturating_sub(vertical_padding.saturating_mul(2))
        .max(1);

    let logo_image = resize_logo_to_fit(&logo_source, max_logo_w, max_logo_h);
    let x = ((bg_w - logo_image.width()) / 2) as i64;
    let y = ((bg_h - logo_image.height()) / 2) as i64;

    overlay(&mut background, &logo_image, x, y);

    if let Some(parent) = output.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent).map_err(|e| {
            format!(
                "Failed to create output directory '{}': {e}",
                parent.display()
            )
        })?;
    }

    background
        .save_with_format(output, ImageFormat::Png)
        .map_err(|e| format!("Failed to save output '{}': {e}", output.display()))?;

    Ok(selected_blank.name)
}

pub(crate) fn run() {
    let args = match parse_args() {
        Ok(value) => value,
        Err(err) => {
            eprintln!("Error: {err}");
            print_usage(
                &env::args()
                    .next()
                    .unwrap_or_else(|| "streamrs-icon-compose".to_string()),
            );
            std::process::exit(1);
        }
    };

    match compose(&args.logo, &args.output, args.padding_ratio) {
        Ok(blank_name) => {
            println!("Selected {blank_name}");
            println!("Wrote {}", args.output.display());
        }
        Err(err) => {
            eprintln!("Error: {err}");
            std::process::exit(1);
        }
    }
}
