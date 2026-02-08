use chrono::Local;
use hidapi::{HidApi, HidDevice};
use image::codecs::gif::GifDecoder;
use image::codecs::jpeg::JpegEncoder;
use image::codecs::png::PngDecoder;
use image::codecs::webp::WebPDecoder;
use image::imageops::FilterType::Lanczos3;
use image::imageops::{crop_imm, resize, rotate180};
use image::{
    AnimationDecoder, DynamicImage, Frame as ImageFrame, GenericImageView, RgbImage, RgbaImage,
    load_from_memory,
};
use resvg::tiny_skia;
use resvg::usvg;
use serde::Deserialize;
use std::cmp::min;
use std::env;
use std::fs;
use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread::sleep;
use std::time::{Duration, Instant};

const KEY_COUNT: usize = 15;
const EDGE_PAGE_ACTION_KEY_COUNT: usize = 14;
const PAGED_ACTION_KEY_COUNT: usize = 13;
const PREVIOUS_PAGE_KEY: usize = 13;
const NEXT_PAGE_KEY: usize = 14;
const NEXT_PAGE_ICON: &str = "stream-deck-next-page.png";
const PREVIOUS_PAGE_ICON: &str = "stream-deck-previous-page.png";
const CLOCK_ICON_ALIAS: &str = "clock.svg";
const CLOCK_ICON_PREFIX: &str = "clock://hh:mm";
const CLOCK_BACKGROUND_ICON: &str = "blank.png";
const CLOCK_FALLBACK_BACKGROUND_COLOR: &str = "#1f1f1f";
const SVG_RENDER_SIZE: u32 = 256;
const MIN_GIF_FRAME_DELAY_MS: u64 = 66;
const DEFAULT_STATUS_CHECK_INTERVAL_MS: u64 = 1000;
const MIN_STATUS_CHECK_INTERVAL_MS: u64 = 100;
const CLOCK_VIEWBOX_SIZE: i32 = 72;
const CLOCK_DIGIT_WIDTH: i32 = 12;
const CLOCK_DIGIT_HEIGHT: i32 = 24;
const CLOCK_COLON_WIDTH: i32 = 4;
const CLOCK_CHAR_GAP: i32 = 2;

#[derive(Clone)]
enum ButtonAction {
    Launch(String),
    PreviousPage,
    NextPage,
}

struct AnimatedKeyState {
    frames: Vec<Vec<u8>>,
    delays: Vec<Duration>,
    current_frame: usize,
    next_frame_at: Instant,
}

struct ClockKeyState {
    current_text: String,
    next_update_at: Instant,
}

struct StatusKeyState {
    command: String,
    icon_on: String,
    icon_off: String,
    check_interval: Duration,
    next_check_at: Instant,
    current_on: Option<bool>,
}

enum DynamicKeyState {
    Animated(AnimatedKeyState),
    Clock(ClockKeyState),
}

struct PageState {
    button_actions: [Option<ButtonAction>; KEY_COUNT],
    dynamic_states: [Option<DynamicKeyState>; KEY_COUNT],
    status_states: [Option<StatusKeyState>; KEY_COUNT],
}

enum LoadedKeyImage {
    Static(Vec<u8>),
    Animated {
        frames: Vec<Vec<u8>>,
        delays: Vec<Duration>,
    },
    Clock {
        image: Vec<u8>,
        current_text: String,
    },
}

#[derive(Debug, Deserialize)]
struct Config {
    #[serde(default = "default_vendor_id")]
    vendor_id: u16,
    #[serde(default = "default_product_id")]
    product_id: u16,
    #[serde(default = "default_usage")]
    usage: u16,
    #[serde(default = "default_usage_page")]
    usage_page: u16,
    #[serde(default = "default_brightness")]
    brightness: usize,
    keys: Vec<KeyBinding>,
}

#[derive(Debug, Deserialize)]
struct KeyBinding {
    action: Option<String>,
    icon: String,
    icon_on: Option<String>,
    icon_off: Option<String>,
    status: Option<String>,
    status_interval_ms: Option<u64>,
}

#[derive(Debug)]
struct CliArgs {
    debug: bool,
    profile: String,
    config_path: Option<PathBuf>,
    init: bool,
    force: bool,
}

fn default_vendor_id() -> u16 {
    0x0fd9
}

fn default_product_id() -> u16 {
    0x0080
}

fn default_usage() -> u16 {
    0x0001
}

fn default_usage_page() -> u16 {
    0x000c
}

fn default_brightness() -> usize {
    60
}

fn print_usage(program: &str) {
    println!("Usage: {program} [--debug] [--profile <name>] [--config <path>] [--init] [--force]");
}

fn parse_args() -> Result<CliArgs, String> {
    let mut debug = false;
    let mut profile = "default".to_string();
    let mut config_path = None;
    let mut init = false;
    let mut force = false;

    let mut args = env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--debug" => debug = true,
            "--profile" => {
                profile = args
                    .next()
                    .ok_or_else(|| "Missing value for --profile".to_string())?;
            }
            "--config" => {
                let value = args
                    .next()
                    .ok_or_else(|| "Missing value for --config".to_string())?;
                config_path = Some(PathBuf::from(value));
            }
            "--init" => init = true,
            "--force" => force = true,
            "--help" | "-h" => {
                let program = env::args().next().unwrap_or_else(|| "streamrs".to_string());
                print_usage(&program);
                std::process::exit(0);
            }
            _ => return Err(format!("Unknown argument: {arg}")),
        }
    }

    if force && !init {
        return Err("--force requires --init".to_string());
    }

    Ok(CliArgs {
        debug,
        profile,
        config_path,
        init,
        force,
    })
}

fn home_dir() -> Result<PathBuf, String> {
    env::var_os("HOME")
        .map(PathBuf::from)
        .ok_or_else(|| "HOME is not set".to_string())
}

fn xdg_config_home() -> Result<PathBuf, String> {
    if let Some(path) = env::var_os("XDG_CONFIG_HOME") {
        return Ok(PathBuf::from(path));
    }
    Ok(home_dir()?.join(".config"))
}

fn xdg_data_home() -> Result<PathBuf, String> {
    if let Some(path) = env::var_os("XDG_DATA_HOME") {
        return Ok(PathBuf::from(path));
    }
    Ok(home_dir()?.join(".local/share"))
}

fn default_config_path(profile: &str) -> Result<PathBuf, String> {
    Ok(xdg_config_home()?
        .join("streamrs")
        .join(format!("{profile}.toml")))
}

fn default_image_dir(profile: &str) -> Result<PathBuf, String> {
    Ok(xdg_data_home()?.join("streamrs").join(profile))
}

fn find_default_config_source(profile: &str) -> Option<PathBuf> {
    let candidates = [
        PathBuf::from(format!("/usr/share/streamrs/{profile}/default.toml")),
        PathBuf::from("/usr/share/streamrs/default/default.toml"),
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("config")
            .join("default.toml"),
    ];
    candidates.into_iter().find(|path| path.is_file())
}

fn find_image_source_dir(profile: &str) -> Option<PathBuf> {
    let candidates = [
        PathBuf::from(format!("/usr/share/streamrs/{profile}")),
        PathBuf::from("/usr/share/streamrs/default"),
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("all_images"),
    ];
    candidates.into_iter().find(|path| path.is_dir())
}

fn copy_file(src: &Path, dst: &Path, force: bool) -> Result<bool, String> {
    if dst.exists() && !force {
        return Ok(false);
    }
    if let Some(parent) = dst.parent() {
        fs::create_dir_all(parent)
            .map_err(|err| format!("Failed to create directory '{}': {err}", parent.display()))?;
    }
    fs::copy(src, dst).map_err(|err| {
        format!(
            "Failed to copy '{}' to '{}': {err}",
            src.display(),
            dst.display()
        )
    })?;
    Ok(true)
}

fn copy_dir_contents(src: &Path, dst: &Path, force: bool) -> Result<(usize, usize), String> {
    fs::create_dir_all(dst)
        .map_err(|err| format!("Failed to create directory '{}': {err}", dst.display()))?;

    let mut copied = 0usize;
    let mut skipped = 0usize;
    for entry in fs::read_dir(src)
        .map_err(|err| format!("Failed to read directory '{}': {err}", src.display()))?
    {
        let entry =
            entry.map_err(|err| format!("Failed to read entry in '{}': {err}", src.display()))?;
        let src_path = entry.path();
        let name = entry.file_name();
        if name == "default.toml" {
            continue;
        }
        let dst_path = dst.join(&name);
        let file_type = entry.file_type().map_err(|err| {
            format!(
                "Failed to read file type for '{}': {err}",
                src_path.display()
            )
        })?;

        if file_type.is_dir() {
            let (sub_copied, sub_skipped) = copy_dir_contents(&src_path, &dst_path, force)?;
            copied += sub_copied;
            skipped += sub_skipped;
        } else if file_type.is_file() {
            if copy_file(&src_path, &dst_path, force)? {
                copied += 1;
            } else {
                skipped += 1;
            }
        }
    }

    Ok((copied, skipped))
}

fn initialize_profile(
    profile: &str,
    config_path: &Path,
    image_dir: &Path,
    force: bool,
) -> Result<(), String> {
    let config_src = find_default_config_source(profile).ok_or_else(|| {
        "Could not find a default config source. Expected /usr/share/streamrs/default/default.toml or repository config.".to_string()
    })?;
    let images_src = find_image_source_dir(profile).ok_or_else(|| {
        "Could not find an image source directory. Expected /usr/share/streamrs/default or repository all_images.".to_string()
    })?;

    let config_copied = copy_file(&config_src, config_path, force)?;
    let (images_copied, images_skipped) = copy_dir_contents(&images_src, image_dir, force)?;

    if config_copied {
        eprintln!(
            "Initialized config '{}' from '{}'",
            config_path.display(),
            config_src.display()
        );
    } else {
        eprintln!(
            "Config '{}' already exists; keeping existing file (use --force to overwrite)",
            config_path.display()
        );
    }

    eprintln!(
        "Initialized images in '{}': {} copied, {} skipped",
        image_dir.display(),
        images_copied,
        images_skipped
    );

    Ok(())
}

fn ensure_profile_initialized(
    profile: &str,
    config_path: &Path,
    image_dir: &Path,
) -> Result<(), String> {
    if config_path.exists() {
        return Ok(());
    }

    eprintln!(
        "Config '{}' not found; initializing profile assets",
        config_path.display()
    );
    initialize_profile(profile, config_path, image_dir, false)
}

fn read_config_file(path: &Path) -> Result<String, String> {
    fs::read_to_string(path)
        .map_err(|err| format!("Failed to read config '{}': {err}", path.display()))
}

fn parse_config(path: &Path, raw: &str) -> Result<Config, String> {
    let config: Config = toml::from_str(raw)
        .map_err(|err| format!("Failed to parse config '{}': {err}", path.display()))?;

    if config.keys.is_empty() {
        return Err(format!("Config '{}' has no keys", path.display()));
    }

    Ok(config)
}

fn key_launch_action(key: &KeyBinding) -> Option<String> {
    key.action.as_ref().and_then(|action| {
        let trimmed = action.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    })
}

fn trimmed_non_empty(value: Option<&str>) -> Option<String> {
    value.and_then(|value| {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    })
}

fn key_status_command(key: &KeyBinding) -> Option<String> {
    trimmed_non_empty(key.status.as_deref())
}

fn key_status_icon_on(key: &KeyBinding) -> String {
    trimmed_non_empty(key.icon_on.as_deref()).unwrap_or_else(|| key.icon.clone())
}

fn key_status_icon_off(key: &KeyBinding) -> String {
    trimmed_non_empty(key.icon_off.as_deref()).unwrap_or_else(|| key.icon.clone())
}

fn key_status_interval(key: &KeyBinding) -> Duration {
    let interval_ms = key
        .status_interval_ms
        .unwrap_or(DEFAULT_STATUS_CHECK_INTERVAL_MS)
        .max(MIN_STATUS_CHECK_INTERVAL_MS);
    Duration::from_millis(interval_ms)
}

fn get_device(vendor_id: u16, product_id: u16, usage: u16, usage_page: u16) -> Option<HidDevice> {
    let api = HidApi::new().expect("Failed to create HID API");
    for dev in api.device_list() {
        if (
            dev.vendor_id(),
            dev.product_id(),
            dev.usage(),
            dev.usage_page(),
        ) == (vendor_id, product_id, usage, usage_page)
        {
            match dev.open_device(&api) {
                Ok(device) => {
                    return Some(device);
                }
                Err(e) => eprintln!("Error: {:?}", e),
            }
        }
    }
    eprintln!("Device not found");
    None
}

fn set_brightness(device: &HidDevice, percentage: usize) -> Result<(), String> {
    let mut buf = [0u8; 32];
    buf[0..3].copy_from_slice(&[0x03, 0x08, percentage as u8]);
    device
        .send_feature_report(&buf)
        .map_err(|err| format!("Failed to set brightness: {err}"))?;
    Ok(())
}

fn launch_app(action: &str, debug: bool) {
    let path: Vec<&str> = action.split_whitespace().collect();
    if path.is_empty() {
        return;
    }

    let mut cmd = Command::new(path[0]);
    cmd.args(&path[1..]).stdin(Stdio::null());

    if debug {
        cmd.stdout(Stdio::inherit()).stderr(Stdio::inherit());
    } else {
        cmd.stdout(Stdio::null()).stderr(Stdio::null());
    }

    if let Err(e) = cmd.spawn() {
        eprintln!("Error launching '{action}': {e}");
    }
}

fn run_status_check(command: &str) -> Result<bool, String> {
    if command.trim().is_empty() {
        return Err("Status check command is empty".to_string());
    }

    let status = Command::new("sh")
        .arg("-c")
        .arg(command)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map_err(|err| format!("Failed to run status check '{command}': {err}"))?;

    Ok(status.success())
}

fn get_pressed_button(buf: &[u8]) -> Option<usize> {
    buf.iter().position(|&x| x == 1)
}

fn read_states(device: &HidDevice, timeout_ms: i32) -> Option<usize> {
    let mut buf = [0u8; 32];
    buf[0] = 19;
    match device.read_timeout(&mut buf, timeout_ms) {
        Ok(size) if size > 0 => get_pressed_button(&buf[4..19]),
        Ok(_) => None,
        Err(err) => {
            eprintln!("Failed to read key state: {err}");
            None
        }
    }
}

fn set_key_image_data(device: &HidDevice, key: u8, data: &[u8]) -> Result<(), String> {
    let mut page_number = 0;
    let mut bytes_remaining = data.len();
    while bytes_remaining > 0 {
        let this_length = min(bytes_remaining, 1024 - 8);
        let bytes_sent = page_number * (1024 - 8);
        let header = [
            0x02,
            0x07,
            key,
            if this_length == bytes_remaining { 1 } else { 0 },
            (this_length & 0xFF) as u8,
            (this_length >> 8) as u8,
            (page_number & 0xFF) as u8,
            (page_number >> 8) as u8,
        ];

        let mut payload = Vec::with_capacity(1024);
        payload.extend_from_slice(&header);
        payload.extend_from_slice(&data[bytes_sent..bytes_sent + this_length]);
        payload.resize(1024, 0);
        device
            .write(&payload)
            .map_err(|err| format!("Failed to write image to key {key}: {err}"))?;

        bytes_remaining -= this_length;
        page_number += 1;
    }

    Ok(())
}

fn set_key_image(device: &HidDevice, key: u8, icon_path: &Path) -> Result<(), String> {
    let img_data = fs::read(icon_path)
        .map_err(|err| format!("Failed to read icon '{}': {err}", icon_path.display()))?;
    let img = get_image_data(icon_path, &img_data)?;
    set_key_image_data(device, key, &img)
}

fn encode_streamdeck_image(img: DynamicImage) -> Result<Vec<u8>, String> {
    let (width, height) = img.dimensions();
    let crop_size = min(width, height);
    let x_offset = (width - crop_size) / 2;
    let y_offset = (height - crop_size) / 2;
    let mut img = crop_imm(&img, x_offset, y_offset, crop_size, crop_size).to_image();
    img = resize(&rotate180(&img), 72, 72, Lanczos3);

    let mut data = Vec::new();
    JpegEncoder::new_with_quality(&mut data, 100)
        .encode_image(&img)
        .map_err(|err| format!("Failed to encode key image: {err}"))?;
    Ok(data)
}

fn get_image_data(icon_path: &Path, img_data: &[u8]) -> Result<Vec<u8>, String> {
    let ext = icon_path
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();

    match ext.as_str() {
        "svg" => {
            let img = load_svg_image(icon_path, img_data)?;
            encode_streamdeck_image(img)
        }
        "gif" => {
            let img = load_gif_first_frame(icon_path, img_data)?;
            encode_streamdeck_image(img)
        }
        _ => {
            let img = load_from_memory(img_data).map_err(|err| {
                format!("Invalid image data for '{}': {err}", icon_path.display())
            })?;
            encode_streamdeck_image(img)
        }
    }
}

fn load_svg_data(
    label: &str,
    svg_data: &[u8],
    resources_dir: Option<&Path>,
) -> Result<DynamicImage, String> {
    let mut options = usvg::Options::default();
    options.resources_dir = resources_dir.map(|path| path.to_path_buf());
    let tree = usvg::Tree::from_data(svg_data, &options)
        .map_err(|err| format!("Failed to parse SVG icon '{}': {err}", label))?;

    let svg_size = tree.size();
    let mut pixmap = tiny_skia::Pixmap::new(SVG_RENDER_SIZE, SVG_RENDER_SIZE)
        .ok_or_else(|| format!("Failed to allocate SVG render target for '{}'", label))?;

    let scale =
        (SVG_RENDER_SIZE as f32 / svg_size.width()).min(SVG_RENDER_SIZE as f32 / svg_size.height());
    let x_offset = (SVG_RENDER_SIZE as f32 - svg_size.width() * scale) / 2.0;
    let y_offset = (SVG_RENDER_SIZE as f32 - svg_size.height() * scale) / 2.0;
    let transform =
        tiny_skia::Transform::from_scale(scale, scale).post_translate(x_offset, y_offset);

    resvg::render(&tree, transform, &mut pixmap.as_mut());

    let rgba = RgbaImage::from_raw(SVG_RENDER_SIZE, SVG_RENDER_SIZE, pixmap.take())
        .ok_or_else(|| format!("Failed to build rasterized SVG image for '{}'", label))?;

    Ok(DynamicImage::ImageRgba8(rgba))
}

fn load_svg_image(icon_path: &Path, img_data: &[u8]) -> Result<DynamicImage, String> {
    load_svg_data(
        &icon_path.display().to_string(),
        img_data,
        icon_path.parent(),
    )
}

fn load_gif_first_frame(icon_path: &Path, img_data: &[u8]) -> Result<DynamicImage, String> {
    let decoder = GifDecoder::new(Cursor::new(img_data))
        .map_err(|err| format!("Failed to decode GIF icon '{}': {err}", icon_path.display()))?;
    let frame = decoder
        .into_frames()
        .next()
        .transpose()
        .map_err(|err| {
            format!(
                "Failed to decode GIF frame for '{}': {err}",
                icon_path.display()
            )
        })?
        .ok_or_else(|| format!("GIF icon '{}' has no frames", icon_path.display()))?;

    Ok(DynamicImage::ImageRgba8(frame.into_buffer()))
}

fn delay_to_duration_ms(delay: image::Delay) -> Duration {
    let (numerator, denominator) = delay.numer_denom_ms();
    let delay_ms = if denominator == 0 {
        MIN_GIF_FRAME_DELAY_MS
    } else {
        ((numerator as u64) / (denominator as u64)).max(MIN_GIF_FRAME_DELAY_MS)
    };
    Duration::from_millis(delay_ms)
}

fn encode_animated_frames(
    frames: Vec<ImageFrame>,
    icon_path: &Path,
) -> Result<LoadedKeyImage, String> {
    if frames.is_empty() {
        return Err(format!(
            "Animated icon '{}' has no frames",
            icon_path.display()
        ));
    }

    let mut encoded_frames = Vec::with_capacity(frames.len());
    let mut delays = Vec::with_capacity(frames.len());

    for frame in frames {
        let delay = delay_to_duration_ms(frame.delay());
        let image = DynamicImage::ImageRgba8(frame.into_buffer());
        encoded_frames.push(encode_streamdeck_image(image)?);
        delays.push(delay);
    }

    if encoded_frames.len() == 1 {
        Ok(LoadedKeyImage::Static(encoded_frames.remove(0)))
    } else {
        Ok(LoadedKeyImage::Animated {
            frames: encoded_frames,
            delays,
        })
    }
}

fn load_animated_gif(icon_path: &Path, img_data: &[u8]) -> Result<LoadedKeyImage, String> {
    let decoder = GifDecoder::new(Cursor::new(img_data))
        .map_err(|err| format!("Failed to decode GIF icon '{}': {err}", icon_path.display()))?;
    let frames: Vec<ImageFrame> = decoder.into_frames().collect_frames().map_err(|err| {
        format!(
            "Failed to decode GIF frames '{}': {err}",
            icon_path.display()
        )
    })?;
    encode_animated_frames(frames, icon_path)
}

fn load_apng_or_static_png(icon_path: &Path, img_data: &[u8]) -> Result<LoadedKeyImage, String> {
    let decoder = PngDecoder::new(Cursor::new(img_data))
        .map_err(|err| format!("Failed to decode PNG icon '{}': {err}", icon_path.display()))?;
    let is_apng = decoder.is_apng().map_err(|err| {
        format!(
            "Failed to inspect PNG icon '{}': {err}",
            icon_path.display()
        )
    })?;
    if !is_apng {
        return Ok(LoadedKeyImage::Static(get_image_data(icon_path, img_data)?));
    }

    let apng_decoder = decoder.apng().map_err(|err| {
        format!(
            "Failed to decode APNG icon '{}': {err}",
            icon_path.display()
        )
    })?;
    let frames: Vec<ImageFrame> = apng_decoder.into_frames().collect_frames().map_err(|err| {
        format!(
            "Failed to decode APNG frames '{}': {err}",
            icon_path.display()
        )
    })?;
    encode_animated_frames(frames, icon_path)
}

fn load_animated_webp_or_static(
    icon_path: &Path,
    img_data: &[u8],
) -> Result<LoadedKeyImage, String> {
    let decoder = WebPDecoder::new(Cursor::new(img_data)).map_err(|err| {
        format!(
            "Failed to decode WebP icon '{}': {err}",
            icon_path.display()
        )
    })?;
    if !decoder.has_animation() {
        return Ok(LoadedKeyImage::Static(get_image_data(icon_path, img_data)?));
    }

    let frames: Vec<ImageFrame> = decoder.into_frames().collect_frames().map_err(|err| {
        format!(
            "Failed to decode animated WebP frames '{}': {err}",
            icon_path.display()
        )
    })?;
    encode_animated_frames(frames, icon_path)
}

fn current_clock_text() -> String {
    Local::now().format("%H:%M").to_string()
}

fn is_clock_icon(icon: &str) -> bool {
    icon.eq_ignore_ascii_case(CLOCK_ICON_ALIAS) || icon.eq_ignore_ascii_case(CLOCK_ICON_PREFIX)
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
        (x + 2, y, 8, 2),       // a
        (x + 10, y + 2, 2, 8),  // b
        (x + 10, y + 14, 2, 8), // c
        (x + 2, y + 22, 8, 2),  // d
        (x, y + 14, 2, 8),      // e
        (x, y + 2, 2, 8),       // f
        (x + 2, y + 11, 8, 2),  // g
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

fn render_clock_svg(image_dir: &Path, text: &str) -> Result<Vec<u8>, String> {
    let svg = render_clock_segments_svg(image_dir, text);
    let img = load_svg_data(CLOCK_ICON_ALIAS, svg.as_bytes(), Some(image_dir))?;
    encode_streamdeck_image(img)
}

fn load_clock_icon(image_dir: &Path) -> Result<LoadedKeyImage, String> {
    let text = current_clock_text();
    let image = render_clock_svg(image_dir, &text)?;
    Ok(LoadedKeyImage::Clock {
        image,
        current_text: text,
    })
}

fn load_key_image(image_dir: &Path, icon: &str) -> Result<LoadedKeyImage, String> {
    if is_clock_icon(icon) {
        return load_clock_icon(image_dir);
    }

    let icon_path = image_dir.join(icon);
    let img_data = fs::read(&icon_path)
        .map_err(|err| format!("Failed to read icon '{}': {err}", icon_path.display()))?;
    let ext = icon_path
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();

    match ext.as_str() {
        "gif" => load_animated_gif(&icon_path, &img_data),
        "png" => load_apng_or_static_png(&icon_path, &img_data),
        "webp" => load_animated_webp_or_static(&icon_path, &img_data),
        _ => Ok(LoadedKeyImage::Static(get_image_data(
            &icon_path, &img_data,
        )?)),
    }
}

fn apply_loaded_key_image(
    device: &HidDevice,
    state: &mut PageState,
    key_index: usize,
    loaded: LoadedKeyImage,
) -> Result<(), String> {
    match loaded {
        LoadedKeyImage::Static(data) => {
            set_key_image_data(device, key_index as u8, &data)?;
            state.dynamic_states[key_index] = None;
        }
        LoadedKeyImage::Animated { frames, delays } => {
            set_key_image_data(device, key_index as u8, &frames[0])?;
            let initial_delay = delays[0];
            state.dynamic_states[key_index] = Some(DynamicKeyState::Animated(AnimatedKeyState {
                frames,
                delays,
                current_frame: 0,
                next_frame_at: Instant::now() + initial_delay,
            }));
        }
        LoadedKeyImage::Clock {
            image,
            current_text,
        } => {
            set_key_image_data(device, key_index as u8, &image)?;
            state.dynamic_states[key_index] = Some(DynamicKeyState::Clock(ClockKeyState {
                current_text,
                next_update_at: Instant::now() + Duration::from_secs(1),
            }));
        }
    }
    Ok(())
}

fn apply_icon_to_key(
    device: &HidDevice,
    image_dir: &Path,
    state: &mut PageState,
    key_index: usize,
    icon: &str,
) -> Result<(), String> {
    let loaded = load_key_image(image_dir, icon)?;
    apply_loaded_key_image(device, state, key_index, loaded)
}

fn blank_image_data() -> Result<Vec<u8>, String> {
    let img = RgbImage::new(72, 72);
    let mut data = Vec::new();
    JpegEncoder::new_with_quality(&mut data, 100)
        .encode_image(&img)
        .map_err(|err| format!("Failed to encode blank key image: {err}"))?;
    Ok(data)
}

fn page_count(key_count: usize) -> usize {
    if key_count <= KEY_COUNT {
        1
    } else if key_count <= EDGE_PAGE_ACTION_KEY_COUNT * 2 {
        2
    } else {
        2 + (key_count - (EDGE_PAGE_ACTION_KEY_COUNT * 2) + PAGED_ACTION_KEY_COUNT - 1)
            / PAGED_ACTION_KEY_COUNT
    }
}

fn page_capacity(page: usize, total_pages: usize) -> usize {
    if total_pages == 1 {
        KEY_COUNT
    } else if page == 0 || page + 1 == total_pages {
        EDGE_PAGE_ACTION_KEY_COUNT
    } else {
        PAGED_ACTION_KEY_COUNT
    }
}

fn set_page(
    device: &HidDevice,
    config: &Config,
    image_dir: &Path,
    page: usize,
    blank_image: &[u8],
) -> PageState {
    let mut state = PageState {
        button_actions: std::array::from_fn(|_| None),
        dynamic_states: std::array::from_fn(|_| None),
        status_states: std::array::from_fn(|_| None),
    };

    for key in 0..KEY_COUNT {
        if let Err(err) = set_key_image_data(device, key as u8, blank_image) {
            eprintln!("{err}");
        }
    }

    let total_pages = page_count(config.keys.len());
    let page = min(page, total_pages.saturating_sub(1));
    let keys_per_page = page_capacity(page, total_pages);
    let offset = (0..page)
        .map(|page_index| page_capacity(page_index, total_pages))
        .sum::<usize>();
    for (index, key) in config
        .keys
        .iter()
        .skip(offset)
        .take(keys_per_page)
        .enumerate()
    {
        if let Some(command) = key_status_command(key) {
            let icon_on = key_status_icon_on(key);
            let icon_off = key_status_icon_off(key);
            let check_interval = key_status_interval(key);
            let initial_state = match run_status_check(&command) {
                Ok(is_on) => is_on,
                Err(err) => {
                    eprintln!("{err}");
                    false
                }
            };
            let icon = if initial_state { &icon_on } else { &icon_off };
            if let Err(err) = apply_icon_to_key(device, image_dir, &mut state, index, icon) {
                eprintln!("{err}");
            }
            state.status_states[index] = Some(StatusKeyState {
                command,
                icon_on,
                icon_off,
                check_interval,
                next_check_at: Instant::now() + check_interval,
                current_on: Some(initial_state),
            });
        } else if let Err(err) = apply_icon_to_key(device, image_dir, &mut state, index, &key.icon)
        {
            eprintln!("{err}");
        }
        if let Some(action) = key_launch_action(key) {
            state.button_actions[index] = Some(ButtonAction::Launch(action));
        }
    }

    if total_pages > 1 {
        let has_prev = page > 0;
        let has_next = page + 1 < total_pages;

        if has_prev {
            let key = if has_next {
                PREVIOUS_PAGE_KEY
            } else {
                NEXT_PAGE_KEY
            };
            let icon_path = image_dir.join(PREVIOUS_PAGE_ICON);
            if let Err(err) = set_key_image(device, key as u8, &icon_path) {
                eprintln!("{err}");
            }
            state.button_actions[key] = Some(ButtonAction::PreviousPage);
        }

        if has_next {
            let icon_path = image_dir.join(NEXT_PAGE_ICON);
            if let Err(err) = set_key_image(device, NEXT_PAGE_KEY as u8, &icon_path) {
                eprintln!("{err}");
            }
            state.button_actions[NEXT_PAGE_KEY] = Some(ButtonAction::NextPage);
        }
    }

    state
}

fn advance_dynamic_keys(device: &HidDevice, image_dir: &Path, state: &mut PageState) {
    let now = Instant::now();
    for key in 0..KEY_COUNT {
        let check = match state.status_states[key].as_ref() {
            Some(status) if now >= status.next_check_at => Some((
                status.command.clone(),
                status.icon_on.clone(),
                status.icon_off.clone(),
                status.current_on,
                status.check_interval,
            )),
            _ => None,
        };

        if let Some((command, icon_on, icon_off, current_on, check_interval)) = check {
            let new_state = match run_status_check(&command) {
                Ok(is_on) => Some(is_on),
                Err(err) => {
                    eprintln!("{err}");
                    None
                }
            };

            if let Some(is_on) = new_state
                && current_on != Some(is_on)
            {
                let icon = if is_on { &icon_on } else { &icon_off };
                if let Err(err) = apply_icon_to_key(device, image_dir, state, key, icon) {
                    eprintln!("{err}");
                }
            }

            if let Some(status) = state.status_states[key].as_mut() {
                if let Some(is_on) = new_state {
                    status.current_on = Some(is_on);
                }
                status.next_check_at = now + check_interval;
            }
        }
    }

    for (key, dynamic_state) in state.dynamic_states.iter_mut().enumerate() {
        if let Some(dynamic_state) = dynamic_state {
            match dynamic_state {
                DynamicKeyState::Animated(animation) => {
                    if now < animation.next_frame_at {
                        continue;
                    }

                    animation.current_frame =
                        (animation.current_frame + 1) % animation.frames.len();
                    if let Err(err) = set_key_image_data(
                        device,
                        key as u8,
                        &animation.frames[animation.current_frame],
                    ) {
                        eprintln!("{err}");
                        continue;
                    }
                    animation.next_frame_at = now + animation.delays[animation.current_frame];
                }
                DynamicKeyState::Clock(clock) => {
                    if now < clock.next_update_at {
                        continue;
                    }

                    let next_text = current_clock_text();
                    if next_text != clock.current_text {
                        match render_clock_svg(image_dir, &next_text) {
                            Ok(image) => {
                                if let Err(err) = set_key_image_data(device, key as u8, &image) {
                                    eprintln!("{err}");
                                } else {
                                    clock.current_text = next_text;
                                }
                            }
                            Err(err) => eprintln!("{err}"),
                        }
                    }
                    clock.next_update_at = now + Duration::from_secs(1);
                }
            }
        }
    }
}

fn main() {
    let program = env::args().next().unwrap_or_else(|| "streamrs".to_string());
    let args = match parse_args() {
        Ok(args) => args,
        Err(err) => {
            eprintln!("{err}");
            print_usage(&program);
            return;
        }
    };

    let config_path = match args.config_path {
        Some(path) => path,
        None => match default_config_path(&args.profile) {
            Ok(path) => path,
            Err(err) => {
                eprintln!("{err}");
                return;
            }
        },
    };

    let image_dir = match default_image_dir(&args.profile) {
        Ok(path) => path,
        Err(err) => {
            eprintln!("{err}");
            return;
        }
    };

    if args.init {
        if let Err(err) = initialize_profile(&args.profile, &config_path, &image_dir, args.force) {
            eprintln!("{err}");
        }
        return;
    }

    if let Err(err) = ensure_profile_initialized(&args.profile, &config_path, &image_dir) {
        eprintln!("{err}");
        return;
    }

    let mut config_raw = match read_config_file(&config_path) {
        Ok(raw) => raw,
        Err(err) => {
            eprintln!("{err}");
            return;
        }
    };

    let mut config = match parse_config(&config_path, &config_raw) {
        Ok(config) => config,
        Err(err) => {
            eprintln!("{err}");
            return;
        }
    };

    let blank_image = match blank_image_data() {
        Ok(data) => data,
        Err(err) => {
            eprintln!("{err}");
            return;
        }
    };

    if let Some(device) = get_device(
        config.vendor_id,
        config.product_id,
        config.usage,
        config.usage_page,
    ) {
        if let Err(err) = set_brightness(&device, config.brightness.clamp(0, 100)) {
            eprintln!("{err}");
        }

        let mut current_page = 0usize;
        let mut total_pages = page_count(config.keys.len());
        let mut page_state = set_page(&device, &config, &image_dir, current_page, &blank_image);
        let mut last_reload_check = Instant::now();
        let mut last_pressed_button = None;

        loop {
            advance_dynamic_keys(&device, &image_dir, &mut page_state);
            let pressed_button = read_states(&device, 10);
            if pressed_button != last_pressed_button {
                if let Some(index) = pressed_button
                    && let Some(action) = page_state.button_actions[index].clone()
                {
                    match action {
                        ButtonAction::Launch(action) => launch_app(&action, args.debug),
                        ButtonAction::PreviousPage => {
                            if current_page > 0 {
                                current_page -= 1;
                                page_state = set_page(
                                    &device,
                                    &config,
                                    &image_dir,
                                    current_page,
                                    &blank_image,
                                );
                            }
                        }
                        ButtonAction::NextPage => {
                            if current_page + 1 < total_pages {
                                current_page += 1;
                                page_state = set_page(
                                    &device,
                                    &config,
                                    &image_dir,
                                    current_page,
                                    &blank_image,
                                );
                            }
                        }
                    }
                }

                last_pressed_button = pressed_button;
            }

            if last_reload_check.elapsed() >= Duration::from_secs(10) {
                last_reload_check = Instant::now();

                match read_config_file(&config_path) {
                    Ok(raw) => {
                        if raw != config_raw {
                            match parse_config(&config_path, &raw) {
                                Ok(new_config) => {
                                    if (
                                        new_config.vendor_id,
                                        new_config.product_id,
                                        new_config.usage,
                                        new_config.usage_page,
                                    ) != (
                                        config.vendor_id,
                                        config.product_id,
                                        config.usage,
                                        config.usage_page,
                                    ) {
                                        eprintln!(
                                            "Warning: HID identifiers changed in config; restart streamrs to apply device selection changes"
                                        );
                                    }

                                    if let Err(err) =
                                        set_brightness(&device, new_config.brightness.clamp(0, 100))
                                    {
                                        eprintln!("{err}");
                                    }

                                    total_pages = page_count(new_config.keys.len());
                                    current_page = min(current_page, total_pages.saturating_sub(1));
                                    page_state = set_page(
                                        &device,
                                        &new_config,
                                        &image_dir,
                                        current_page,
                                        &blank_image,
                                    );
                                    last_pressed_button = None;
                                    config = new_config;
                                    config_raw = raw;
                                    eprintln!("Config reloaded from '{}'", config_path.display());
                                }
                                Err(err) => eprintln!("{err}"),
                            }
                        }
                    }
                    Err(err) => eprintln!("{err}"),
                }
            }

            sleep(Duration::from_millis(10));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::Rgba;
    use image::codecs::gif::GifEncoder;
    use std::io::Cursor;

    #[test]
    fn svg_icon_is_supported() {
        let svg = r##"<svg xmlns="http://www.w3.org/2000/svg" width="128" height="64"><rect width="128" height="64" fill="#00ff00"/></svg>"##;
        let data = get_image_data(Path::new("icon.svg"), svg.as_bytes())
            .expect("SVG should decode and encode for Stream Deck");
        assert!(data.len() > 2);
        assert_eq!(data[0], 0xFF);
        assert_eq!(data[1], 0xD8);
    }

    #[test]
    fn gif_icon_is_supported() {
        let gif: &[u8] = &[
            0x47, 0x49, 0x46, 0x38, 0x39, 0x61, 0x01, 0x00, 0x01, 0x00, 0x80, 0x00, 0x00, 0x00,
            0x00, 0x00, 0xFF, 0xFF, 0xFF, 0x21, 0xF9, 0x04, 0x01, 0x00, 0x00, 0x00, 0x00, 0x2C,
            0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x01, 0x00, 0x00, 0x02, 0x02, 0x44, 0x01, 0x00,
            0x3B,
        ];
        let data = get_image_data(Path::new("icon.gif"), gif.as_ref())
            .expect("GIF should decode and encode for Stream Deck");
        assert!(data.len() > 2);
        assert_eq!(data[0], 0xFF);
        assert_eq!(data[1], 0xD8);
    }

    #[test]
    fn animated_gif_fixture_is_supported() {
        let path = Path::new("animated.gif");
        let mut gif_data = Vec::new();
        {
            let mut encoder = GifEncoder::new(&mut gif_data);
            let frame1 = ImageFrame::from_parts(
                RgbaImage::from_pixel(2, 2, Rgba([0, 0, 0, 255])),
                0,
                0,
                image::Delay::from_numer_denom_ms(100, 1),
            );
            let frame2 = ImageFrame::from_parts(
                RgbaImage::from_pixel(2, 2, Rgba([255, 255, 255, 255])),
                0,
                0,
                image::Delay::from_numer_denom_ms(100, 1),
            );
            encoder
                .encode_frames(vec![frame1, frame2].into_iter())
                .expect("fixture GIF should encode");
        }

        let decoder = GifDecoder::new(Cursor::new(gif_data.as_slice()))
            .expect("fixture should decode as GIF");
        let frames = decoder
            .into_frames()
            .collect_frames()
            .expect("fixture frames should decode");
        assert!(frames.len() > 1, "fixture should be an animated GIF");

        let loaded =
            load_animated_gif(path, &gif_data).expect("animated GIF should load for animation");
        match loaded {
            LoadedKeyImage::Animated { frames, delays } => {
                assert!(frames.len() > 1);
                assert_eq!(frames.len(), delays.len());
            }
            LoadedKeyImage::Static(_) => panic!("animated GIF should not load as static"),
            LoadedKeyImage::Clock { .. } => panic!("animated GIF should not load as clock"),
        }
    }

    #[test]
    fn delay_conversion_uses_millisecond_ratio() {
        let exact = delay_to_duration_ms(image::Delay::from_numer_denom_ms(150, 1));
        assert_eq!(exact, Duration::from_millis(150));

        let tiny = delay_to_duration_ms(image::Delay::from_numer_denom_ms(1, 100));
        assert_eq!(tiny, Duration::from_millis(MIN_GIF_FRAME_DELAY_MS));
    }

    #[test]
    fn encode_animated_frames_builds_animation_state() {
        let frame1 = ImageFrame::from_parts(
            RgbaImage::new(8, 8),
            0,
            0,
            image::Delay::from_numer_denom_ms(20, 1),
        );
        let frame2 = ImageFrame::from_parts(
            RgbaImage::new(8, 8),
            0,
            0,
            image::Delay::from_numer_denom_ms(200, 1),
        );

        let loaded = encode_animated_frames(vec![frame1, frame2], Path::new("anim.gif"))
            .expect("multi-frame animation should load");
        match loaded {
            LoadedKeyImage::Animated { frames, delays } => {
                assert_eq!(frames.len(), 2);
                assert_eq!(delays.len(), 2);
                assert_eq!(delays[0], Duration::from_millis(MIN_GIF_FRAME_DELAY_MS));
                assert_eq!(delays[1], Duration::from_millis(200));
            }
            LoadedKeyImage::Static(_) => panic!("expected animated state"),
            LoadedKeyImage::Clock { .. } => panic!("expected animated state"),
        }
    }

    #[test]
    fn clock_icon_renders_svg_without_background_file() {
        let missing_dir = Path::new("/tmp/streamrs-missing-clock-assets");
        let loaded =
            load_key_image(missing_dir, CLOCK_ICON_ALIAS).expect("clock icon should render");
        match loaded {
            LoadedKeyImage::Clock {
                image,
                current_text,
            } => {
                assert_eq!(current_text.len(), 5);
                assert_eq!(&current_text[2..3], ":");
                assert!(image.len() > 2);
                assert_eq!(image[0], 0xFF);
                assert_eq!(image[1], 0xD8);
            }
            _ => panic!("expected clock image variant"),
        }
    }

    #[test]
    fn clock_svg_uses_fallback_background_when_blank_png_is_missing() {
        let missing_dir = Path::new("/tmp/streamrs-missing-clock-assets");
        let svg = render_clock_segments_svg(missing_dir, "12:34");
        assert!(svg.contains(CLOCK_FALLBACK_BACKGROUND_COLOR));
        assert!(!svg.contains(CLOCK_BACKGROUND_ICON));
    }

    #[test]
    fn parse_config_allows_missing_action() {
        let raw = r#"
            [[keys]]
            icon = "blank.png"
        "#;
        let config = parse_config(Path::new("test.toml"), raw)
            .expect("config with missing action should parse");
        assert_eq!(config.keys.len(), 1);
        assert!(key_launch_action(&config.keys[0]).is_none());
    }

    #[test]
    fn blank_action_is_treated_as_noop() {
        let raw = r#"
            [[keys]]
            action = "   "
            icon = "blank.png"
        "#;
        let config = parse_config(Path::new("test.toml"), raw)
            .expect("config with blank action should parse");
        assert_eq!(config.keys.len(), 1);
        assert!(key_launch_action(&config.keys[0]).is_none());
    }

    #[test]
    fn status_config_parses_and_falls_back_icons() {
        let raw = r#"
            [[keys]]
            icon = "default.png"
            status = "test-command"
            icon_on = "on.png"
        "#;
        let config = parse_config(Path::new("test.toml"), raw).expect("status config should parse");
        let key = &config.keys[0];
        assert_eq!(key_status_command(key).as_deref(), Some("test-command"));
        assert_eq!(key_status_icon_on(key), "on.png");
        assert_eq!(key_status_icon_off(key), "default.png");
    }

    #[test]
    fn status_interval_is_clamped() {
        let raw = r#"
            [[keys]]
            icon = "default.png"
            status = "test-command"
            status_interval_ms = 1
        "#;
        let config =
            parse_config(Path::new("test.toml"), raw).expect("status interval config should parse");
        let key = &config.keys[0];
        assert_eq!(
            key_status_interval(key),
            Duration::from_millis(MIN_STATUS_CHECK_INTERVAL_MS)
        );
    }
}
