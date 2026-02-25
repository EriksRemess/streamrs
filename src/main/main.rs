use hidapi::HidDevice;
#[cfg(test)]
use image::codecs::gif::GifDecoder;
#[cfg(test)]
use image::{AnimationDecoder, Frame as ImageFrame, RgbaImage};
use std::cmp::min;
use std::collections::HashMap;
use std::env;
use std::path::Path;
use std::thread::sleep;
use std::time::{Duration, Instant};
#[cfg(test)]
use streamrs::image::clock::{
    CLOCK_BACKGROUND_ICON, CLOCK_FALLBACK_BACKGROUND_COLOR, CLOCK_ICON_ALIAS,
};

#[path = "../config/streamrs.rs"]
mod config;
#[path = "../init/streamrs.rs"]
mod init;
#[cfg(test)]
#[path = "tests.rs"]
mod main_tests;
#[path = "../image/streamrs.rs"]
mod stream_image;

use config::{
    is_launcher_like_command, key_clock_background, key_launch_action, key_status_command,
    key_status_icon_off, key_status_icon_on, key_status_interval, parse_config, read_config_file,
};
use init::{
    default_config_path, default_image_dir, ensure_profile_initialized, initialize_profile,
    parse_args, print_post_init_service_hint, print_usage,
};
use stream_image::{
    blank_image_data, build_image_cache, current_clock_text, load_key_image_cached,
    render_clock_svg,
};
#[cfg(test)]
use stream_image::{
    delay_to_duration_ms, encode_animated_frames, get_image_data, load_animated_gif,
    load_key_image, render_clock_segments_svg,
};
use streamrs::config::streamrs_schema::{
    StreamrsConfig as Config, StreamrsKeyBinding as KeyBinding,
};
#[cfg(test)]
use streamrs::config::streamrs_schema::{
    default_brightness as schema_default_brightness,
    default_keys_per_page as schema_default_keys_per_page,
    default_product_id as schema_default_product_id, default_usage as schema_default_usage,
    default_usage_page as schema_default_usage_page, default_vendor_id as schema_default_vendor_id,
};
use streamrs::paging::PagingLayout;
use streamrs::process::{launch_split_command, run_shell_status};
use streamrs::streamdeck::{get_device, read_states, set_brightness, set_key_image_data};

const KEY_COUNT: usize = streamrs::paging::STREAMDECK_KEY_COUNT;
const MIN_KEYS_PER_PAGE: usize = streamrs::paging::MIN_KEYS_PER_PAGE;
const NEXT_PAGE_ICON: &str = "stream-deck-next-page.png";
const PREVIOUS_PAGE_ICON: &str = "stream-deck-previous-page.png";
const SVG_RENDER_SIZE: u32 = 256;
const MIN_GIF_FRAME_DELAY_MS: u64 = 66;
const DEFAULT_STATUS_CHECK_INTERVAL_MS: u64 = 1000;
const MIN_STATUS_CHECK_INTERVAL_MS: u64 = 100;

#[derive(Clone, Debug, PartialEq, Eq)]
enum ButtonAction {
    Launch(String),
    PreviousPage,
    NextPage,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct PlannedStatusKey {
    command: String,
    icon_on: String,
    icon_off: String,
    clock_background: Option<String>,
    check_interval: Duration,
    current_on: Option<bool>,
    poll_now: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum PagePlanWarning {
    LauncherLikeStatusWithoutAction { key_number: usize, command: String },
    LauncherLikeStatusIgnored { key_number: usize },
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct PageLayoutPlan {
    page: usize,
    total_pages: usize,
    icons: [Option<(String, Option<String>)>; KEY_COUNT],
    button_actions: [Option<ButtonAction>; KEY_COUNT],
    status_slots: [Option<PlannedStatusKey>; KEY_COUNT],
    warnings: Vec<PagePlanWarning>,
}

struct AnimatedKeyState {
    frames: Vec<Vec<u8>>,
    delays: Vec<Duration>,
    current_frame: usize,
    next_frame_at: Instant,
}

struct ClockKeyState {
    current_text: String,
    background_name: Option<String>,
    next_update_at: Instant,
}

struct StatusKeyState {
    command: String,
    icon_on: String,
    icon_off: String,
    clock_background: Option<String>,
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

#[derive(Clone)]
enum LoadedKeyImage {
    Static(Vec<u8>),
    Animated {
        frames: Vec<Vec<u8>>,
        delays: Vec<Duration>,
    },
    Clock {
        image: Vec<u8>,
        current_text: String,
        background_name: Option<String>,
    },
}

#[derive(Clone, Eq, Hash, PartialEq)]
struct ImageCacheKey {
    icon: String,
    clock_background: Option<String>,
}

type ImageCache = HashMap<ImageCacheKey, LoadedKeyImage>;
type StatusCache = HashMap<String, bool>;

#[cfg(test)]
fn default_vendor_id() -> u16 {
    schema_default_vendor_id()
}

#[cfg(test)]
fn default_product_id() -> u16 {
    schema_default_product_id()
}

#[cfg(test)]
fn default_usage() -> u16 {
    schema_default_usage()
}

#[cfg(test)]
fn default_usage_page() -> u16 {
    schema_default_usage_page()
}

#[cfg(test)]
fn default_brightness() -> usize {
    schema_default_brightness()
}

#[cfg(test)]
fn default_keys_per_page() -> usize {
    schema_default_keys_per_page()
}

fn paging_layout(config: &Config) -> PagingLayout {
    PagingLayout::new(KEY_COUNT, config.keys_per_page)
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
            background_name,
        } => {
            set_key_image_data(device, key_index as u8, &image)?;
            state.dynamic_states[key_index] = Some(DynamicKeyState::Clock(ClockKeyState {
                current_text,
                background_name,
                next_update_at: Instant::now() + Duration::from_secs(1),
            }));
        }
    }
    Ok(())
}

fn launch_app(action: &str, debug: bool) {
    if let Err(err) = launch_split_command(action, debug) {
        eprintln!("{err}");
    }
}

fn run_status_check(command: &str) -> Result<bool, String> {
    run_shell_status(command)
}

fn apply_icon_to_key(
    device: &HidDevice,
    image_dir: &Path,
    image_cache: &mut ImageCache,
    state: &mut PageState,
    key_index: usize,
    icon: &str,
    clock_background: Option<&str>,
) -> Result<(), String> {
    let loaded = load_key_image_cached(image_dir, image_cache, icon, clock_background)?;
    apply_loaded_key_image(device, state, key_index, loaded)
}

fn page_count(config: &Config) -> usize {
    paging_layout(config).page_count(config.keys.len())
}

fn plan_page_layout(config: &Config, status_cache: &StatusCache, page: usize) -> PageLayoutPlan {
    let mut icons = std::array::from_fn(|_| None);
    let mut button_actions = std::array::from_fn(|_| None);
    let mut status_slots = std::array::from_fn(|_| None);
    let mut warnings = Vec::new();

    let layout = paging_layout(config);
    let total_pages = layout.page_count(config.keys.len());
    let page = min(page, total_pages.saturating_sub(1));
    let keys_per_page = layout.page_capacity(page, total_pages);
    let offset = (0..page)
        .map(|page_index| layout.page_capacity(page_index, total_pages))
        .sum::<usize>();

    for (index, key) in config
        .keys
        .iter()
        .skip(offset)
        .take(keys_per_page)
        .enumerate()
    {
        let clock_background = key_clock_background(key);
        let launch_action = key_launch_action(key);
        let status_command = key_status_command(key);
        let status_is_launcher = status_command
            .as_deref()
            .is_some_and(is_launcher_like_command);

        if let Some(command) = status_command.clone()
            && !status_is_launcher
        {
            let icon_on = key_status_icon_on(key);
            let icon_off = key_status_icon_off(key);
            let check_interval = key_status_interval(key);
            let cached_state = status_cache.get(&command).copied();
            let initial_state = cached_state.unwrap_or(false);
            let initial_icon = if initial_state {
                icon_on.clone()
            } else {
                icon_off.clone()
            };
            icons[index] = Some((initial_icon, clock_background.clone()));
            status_slots[index] = Some(PlannedStatusKey {
                command,
                icon_on,
                icon_off,
                clock_background,
                check_interval,
                current_on: cached_state,
                poll_now: cached_state.is_none(),
            });
        } else {
            icons[index] = Some((key.icon.clone(), clock_background.clone()));
        }

        if status_is_launcher {
            if launch_action.is_none() {
                if let Some(command) = status_command {
                    warnings.push(PagePlanWarning::LauncherLikeStatusWithoutAction {
                        key_number: offset + index + 1,
                        command: command.clone(),
                    });
                    button_actions[index] = Some(ButtonAction::Launch(command));
                }
            } else {
                warnings.push(PagePlanWarning::LauncherLikeStatusIgnored {
                    key_number: offset + index + 1,
                });
            }
        }

        if let Some(action) = launch_action {
            button_actions[index] = Some(ButtonAction::Launch(action));
        }
    }

    if total_pages > 1 {
        let has_prev = page > 0;
        let has_next = page + 1 < total_pages;

        if has_prev {
            let key = if has_next {
                layout.previous_page_key()
            } else {
                layout.next_page_key()
            };
            icons[key] = Some((PREVIOUS_PAGE_ICON.to_string(), None));
            button_actions[key] = Some(ButtonAction::PreviousPage);
        }

        if has_next {
            let next_key = layout.next_page_key();
            icons[next_key] = Some((NEXT_PAGE_ICON.to_string(), None));
            button_actions[next_key] = Some(ButtonAction::NextPage);
        }
    }

    PageLayoutPlan {
        page,
        total_pages,
        icons,
        button_actions,
        status_slots,
        warnings,
    }
}

fn set_page(
    device: &HidDevice,
    config: &Config,
    image_dir: &Path,
    image_cache: &mut ImageCache,
    status_cache: &StatusCache,
    page: usize,
    blank_image: &[u8],
) -> PageState {
    let mut state = PageState {
        button_actions: std::array::from_fn(|_| None),
        dynamic_states: std::array::from_fn(|_| None),
        status_states: std::array::from_fn(|_| None),
    };
    let plan = plan_page_layout(config, status_cache, page);

    for warning in &plan.warnings {
        match warning {
            PagePlanWarning::LauncherLikeStatusWithoutAction {
                key_number,
                command,
            } => eprintln!(
                "Key {} has launcher-like status command '{}' with no action; treating it as action",
                key_number, command
            ),
            PagePlanWarning::LauncherLikeStatusIgnored { key_number } => eprintln!(
                "Key {} has launcher-like status command; ignoring status polling for this key",
                key_number
            ),
        }
    }

    state.button_actions = plan.button_actions.clone();

    for key in 0..KEY_COUNT {
        if let Some((ref icon, ref clock_background)) = plan.icons[key]
            && let Err(err) = apply_icon_to_key(
                device,
                image_dir,
                image_cache,
                &mut state,
                key,
                icon,
                clock_background.as_deref(),
            )
        {
            eprintln!("{err}");
        }

        if let Some(status) = &plan.status_slots[key] {
            state.status_states[key] = Some(StatusKeyState {
                command: status.command.clone(),
                icon_on: status.icon_on.clone(),
                icon_off: status.icon_off.clone(),
                clock_background: status.clock_background.clone(),
                check_interval: status.check_interval,
                next_check_at: if status.poll_now {
                    Instant::now()
                } else {
                    Instant::now() + status.check_interval
                },
                current_on: status.current_on,
            });
        }
    }

    for (key, icon) in plan.icons.iter().enumerate() {
        if icon.is_some() {
            continue;
        }
        if let Err(err) = set_key_image_data(device, key as u8, blank_image) {
            eprintln!("{err}");
        }
    }

    state
}

fn advance_dynamic_keys(
    device: &HidDevice,
    image_dir: &Path,
    image_cache: &mut ImageCache,
    status_cache: &mut StatusCache,
    state: &mut PageState,
) {
    let now = Instant::now();
    for key in 0..KEY_COUNT {
        let check = match state.status_states[key].as_ref() {
            Some(status) if now >= status.next_check_at => Some((
                status.command.clone(),
                status.icon_on.clone(),
                status.icon_off.clone(),
                status.clock_background.clone(),
                status.current_on,
                status.check_interval,
            )),
            _ => None,
        };

        if let Some((command, icon_on, icon_off, clock_background, current_on, check_interval)) =
            check
        {
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
                if let Err(err) = apply_icon_to_key(
                    device,
                    image_dir,
                    image_cache,
                    state,
                    key,
                    icon,
                    clock_background.as_deref(),
                ) {
                    eprintln!("{err}");
                }
            }

            if let Some(status) = state.status_states[key].as_mut() {
                if let Some(is_on) = new_state {
                    status_cache.insert(status.command.clone(), is_on);
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
                        match render_clock_svg(
                            image_dir,
                            &next_text,
                            clock.background_name.as_deref(),
                        ) {
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

pub(crate) fn run() {
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
        match initialize_profile(
            &args.profile,
            &config_path,
            &image_dir,
            args.force,
            args.force || args.force_images,
        ) {
            Ok(()) => {
                print_post_init_service_hint();
                return;
            }
            Err(err) => {
                eprintln!("{err}");
                std::process::exit(1);
            }
        }
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
    let mut image_cache = build_image_cache(&config, &image_dir);
    let mut status_cache = StatusCache::new();

    let blank_image = match blank_image_data() {
        Ok(data) => data,
        Err(err) => {
            eprintln!("{err}");
            return;
        }
    };

    let mut device: Option<HidDevice> = None;
    let mut current_page = 0usize;
    let mut total_pages = page_count(&config);
    let mut page_state: Option<PageState> = None;
    let mut last_reload_check = Instant::now();
    let mut last_pressed_button = None;
    let mut last_device_probe = Instant::now() - Duration::from_secs(1);
    let mut waiting_for_device_logged = false;

    loop {
        if device.is_none() && last_device_probe.elapsed() >= Duration::from_millis(500) {
            last_device_probe = Instant::now();
            if !waiting_for_device_logged {
                eprintln!("Waiting for Stream Deck connection...");
                waiting_for_device_logged = true;
            }

            if let Some(connected_device) = get_device(
                config.vendor_id,
                config.product_id,
                config.usage,
                config.usage_page,
            ) {
                eprintln!("Stream Deck connected");
                if let Err(err) = set_brightness(&connected_device, config.brightness.clamp(0, 100))
                {
                    eprintln!("{err}");
                }
                total_pages = page_count(&config);
                current_page = min(current_page, total_pages.saturating_sub(1));
                page_state = Some(set_page(
                    &connected_device,
                    &config,
                    &image_dir,
                    &mut image_cache,
                    &status_cache,
                    current_page,
                    &blank_image,
                ));
                last_pressed_button = None;
                waiting_for_device_logged = false;
                device = Some(connected_device);
            }
        }

        let mut disconnected = false;
        if let (Some(device_ref), Some(page_state_ref)) = (device.as_ref(), page_state.as_mut()) {
            advance_dynamic_keys(
                device_ref,
                &image_dir,
                &mut image_cache,
                &mut status_cache,
                page_state_ref,
            );

            match read_states(device_ref, 10) {
                Ok(pressed_button) => {
                    if pressed_button != last_pressed_button {
                        if let Some(index) = pressed_button
                            && let Some(action) = page_state_ref.button_actions[index].clone()
                        {
                            match action {
                                ButtonAction::Launch(action) => launch_app(&action, args.debug),
                                ButtonAction::PreviousPage => {
                                    if current_page > 0 {
                                        current_page -= 1;
                                        *page_state_ref = set_page(
                                            device_ref,
                                            &config,
                                            &image_dir,
                                            &mut image_cache,
                                            &status_cache,
                                            current_page,
                                            &blank_image,
                                        );
                                    }
                                }
                                ButtonAction::NextPage => {
                                    if current_page + 1 < total_pages {
                                        current_page += 1;
                                        *page_state_ref = set_page(
                                            device_ref,
                                            &config,
                                            &image_dir,
                                            &mut image_cache,
                                            &status_cache,
                                            current_page,
                                            &blank_image,
                                        );
                                    }
                                }
                            }
                        }

                        last_pressed_button = pressed_button;
                    }
                }
                Err(err) => {
                    if err.to_ascii_lowercase().contains("device disconnected") {
                        eprintln!("Lost Stream Deck connection while reading key state");
                    } else {
                        eprintln!("{err}");
                    }
                    disconnected = true;
                }
            }
        }

        if disconnected {
            eprintln!("Stream Deck disconnected");
            device = None;
            page_state = None;
            last_pressed_button = None;
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
                                        "Warning: HID identifiers changed in config; existing connection will keep using the current device until it reconnects"
                                    );
                                }

                                total_pages = page_count(&new_config);
                                current_page = min(current_page, total_pages.saturating_sub(1));
                                image_cache = build_image_cache(&new_config, &image_dir);

                                if let Some(device_ref) = device.as_ref() {
                                    if let Err(err) = set_brightness(
                                        device_ref,
                                        new_config.brightness.clamp(0, 100),
                                    ) {
                                        eprintln!("{err}");
                                    }

                                    page_state = Some(set_page(
                                        device_ref,
                                        &new_config,
                                        &image_dir,
                                        &mut image_cache,
                                        &status_cache,
                                        current_page,
                                        &blank_image,
                                    ));
                                    last_pressed_button = None;
                                }

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
