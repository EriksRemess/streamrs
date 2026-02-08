use hidapi::{HidApi, HidDevice};
use image::codecs::jpeg::JpegEncoder;
use image::imageops::FilterType::Lanczos3;
use image::imageops::{crop_imm, resize, rotate180};
use image::{GenericImageView, load_from_memory};
use serde::Deserialize;
use std::cmp::min;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread::sleep;
use std::time::{Duration, Instant};

const KEY_COUNT: usize = 15;

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
    action: String,
    icon: String,
}

#[derive(Debug)]
struct CliArgs {
    debug: bool,
    profile: String,
    config_path: Option<PathBuf>,
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
    println!("Usage: {program} [--debug] [--profile <name>] [--config <path>]");
}

fn parse_args() -> Result<CliArgs, String> {
    let mut debug = false;
    let mut profile = "default".to_string();
    let mut config_path = None;

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
            "--help" | "-h" => {
                let program = env::args().next().unwrap_or_else(|| "streamrs".to_string());
                print_usage(&program);
                std::process::exit(0);
            }
            _ => return Err(format!("Unknown argument: {arg}")),
        }
    }

    Ok(CliArgs {
        debug,
        profile,
        config_path,
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
            if let Ok(device) = dev.open_device(&api) {
                return Some(device);
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

fn get_pressed_button(buf: &[u8], actions: &[String], debug: bool) {
    if let Some(index) = buf.iter().position(|&x| x == 1)
        && let Some(action) = actions.get(index)
    {
        launch_app(action, debug);
    }
}

fn read_states(device: &HidDevice, actions: &[String], debug: bool) {
    let mut buf = [0u8; 32];
    buf[0] = 19;
    match device.read_timeout(&mut buf, 100) {
        Ok(size) if size > 0 => get_pressed_button(&buf[4..19], actions, debug),
        Ok(_) => {}
        Err(err) => eprintln!("Failed to read key state: {err}"),
    }
}

fn set_key_image(device: &HidDevice, key: u8, icon_path: &Path) -> Result<(), String> {
    let img_data = fs::read(icon_path)
        .map_err(|err| format!("Failed to read icon '{}': {err}", icon_path.display()))?;
    let img = get_image_data(&img_data)?;

    let mut page_number = 0;
    let mut bytes_remaining = img.len();
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
        payload.extend_from_slice(&img[bytes_sent..bytes_sent + this_length]);
        payload.resize(1024, 0);
        device
            .write(&payload)
            .map_err(|err| format!("Failed to write image to key {key}: {err}"))?;

        bytes_remaining -= this_length;
        page_number += 1;
    }

    Ok(())
}

fn get_image_data(img_data: &[u8]) -> Result<Vec<u8>, String> {
    let img = load_from_memory(img_data).map_err(|err| format!("Invalid image data: {err}"))?;
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

fn warn_key_count(config: &Config) {
    if config.keys.len() < KEY_COUNT {
        eprintln!(
            "Warning: config has {} keys, expected {}",
            config.keys.len(),
            KEY_COUNT
        );
    } else if config.keys.len() > KEY_COUNT {
        eprintln!(
            "Warning: config has {} keys, only first {} will be used",
            config.keys.len(),
            KEY_COUNT
        );
    }
}

fn apply_config(device: &HidDevice, config: &Config, image_dir: &Path) -> Vec<String> {
    if let Err(err) = set_brightness(device, config.brightness.clamp(0, 100)) {
        eprintln!("{err}");
    }

    for (index, key) in config.keys.iter().take(KEY_COUNT).enumerate() {
        let icon_path = image_dir.join(&key.icon);
        if let Err(err) = set_key_image(device, index as u8, &icon_path) {
            eprintln!("{err}");
        }
    }

    config
        .keys
        .iter()
        .take(KEY_COUNT)
        .map(|key| key.action.clone())
        .collect()
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

    warn_key_count(&config);

    if let Some(device) = get_device(
        config.vendor_id,
        config.product_id,
        config.usage,
        config.usage_page,
    ) {
        let mut actions = apply_config(&device, &config, &image_dir);
        let mut last_reload_check = Instant::now();

        loop {
            read_states(&device, &actions, args.debug);

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

                                    warn_key_count(&new_config);
                                    actions = apply_config(&device, &new_config, &image_dir);
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

            sleep(Duration::from_millis(100));
        }
    }
}
