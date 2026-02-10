use adw::prelude::*;
use adw::{Application, ApplicationWindow, HeaderBar};
use chrono::Local;
use gtk::{
    Align, Box as GtkBox, Button, CssProvider, DropDown, Entry, Fixed, Image, Label, Orientation,
    Overlay, Paned, Picture, ScrolledWindow, SpinButton, STYLE_PROVIDER_PRIORITY_APPLICATION,
};
use image::imageops::{FilterType::Lanczos3, resize};
use image::RgbaImage;
use resvg::tiny_skia;
use resvg::usvg;
use serde::{Deserialize, Serialize};
use std::collections::hash_map::DefaultHasher;
use std::cell::{Cell, RefCell};
use std::env;
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::rc::Rc;

const KEY_COUNT: usize = 15;
const EDGE_PAGE_ACTION_KEY_COUNT: usize = 14;
const PAGED_ACTION_KEY_COUNT: usize = 13;
const DEFAULT_STATUS_INTERVAL_MS: u64 = 1000;
const MIN_STATUS_INTERVAL_MS: u64 = 100;
const MAX_STATUS_INTERVAL_MS: u64 = 60_000;

const CLOCK_ICON_ALIAS: &str = "clock.svg";
const CLOCK_ICON_PREFIX: &str = "clock://hh:mm";
const CLOCK_BACKGROUND_ICON: &str = "blank.png";
const NAV_PREVIOUS_ICON: &str = "stream-deck-previous-page.png";
const NAV_NEXT_ICON: &str = "stream-deck-next-page.png";
const CLOCK_FALLBACK_BACKGROUND_COLOR: &str = "#1f1f1f";
const CLOCK_VIEWBOX_SIZE: i32 = 72;
const CLOCK_DIGIT_WIDTH: i32 = 12;
const CLOCK_DIGIT_HEIGHT: i32 = 24;
const CLOCK_COLON_WIDTH: i32 = 4;
const CLOCK_CHAR_GAP: i32 = 2;

const TEMPLATE_RENDER_WIDTH: u32 = 1560;
const TEMPLATE_RENDER_HEIGHT: u32 = 1108;
const PREVIEW_WIDTH: u32 = 936;
const PREVIEW_HEIGHT: u32 = 665;
const DECK_MIN_SCALE: f32 = 0.5;
const DECK_MIN_WIDTH: i32 = (PREVIEW_WIDTH as f32 * DECK_MIN_SCALE) as i32;
const DECK_MIN_HEIGHT: i32 = (PREVIEW_HEIGHT as f32 * DECK_MIN_SCALE) as i32;
const INSPECTOR_MIN_WIDTH: i32 = 390;
const WINDOW_MIN_WIDTH: i32 = DECK_MIN_WIDTH + INSPECTOR_MIN_WIDTH + 120;
const WINDOW_MIN_HEIGHT: i32 = DECK_MIN_HEIGHT + 190;

#[derive(Debug, Clone, Serialize, Deserialize)]
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
    #[serde(default)]
    keys: Vec<KeyBinding>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct KeyBinding {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    action: Option<String>,
    #[serde(default = "default_icon_name")]
    icon: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    clock_background: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    icon_on: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    icon_off: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    status: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    status_interval_ms: Option<u64>,
}

#[derive(Debug, Clone)]
struct AppState {
    config: Config,
    config_path: PathBuf,
    profile: String,
    image_dirs: Vec<PathBuf>,
    writable_image_dir: PathBuf,
}

#[derive(Clone)]
struct EditorWidgets {
    config_path_entry: Entry,
    selected_label: Label,
    action_entry: Entry,
    icon_kind_dropdown: DropDown,
    icon_label: Label,
    icon_row: GtkBox,
    icon_dropdown: DropDown,
    clock_background_label: Label,
    clock_background_dropdown: DropDown,
    status_command_label: Label,
    status_entry: Entry,
    icon_on_label: Label,
    icon_on_dropdown: DropDown,
    icon_off_label: Label,
    icon_off_dropdown: DropDown,
    interval_label: Label,
    interval_spin: SpinButton,
    icon_preview: Picture,
    apply_button: Button,
    clear_button: Button,
    status_label: Label,
}

#[derive(Clone, Copy, Debug)]
struct KeySlot {
    x0: u32,
    y0: u32,
    x1: u32,
    y1: u32,
    cx: f32,
    cy: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ReservedNavigationSlot {
    PreviousPage,
    NextPage,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum EditorMode {
    Regular,
    Status,
    Clock,
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

fn default_icon_name() -> String {
    "blank.png".to_string()
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

fn profile_from_config_path(path: &Path) -> String {
    path.file_stem()
        .and_then(|stem| stem.to_str())
        .map(|value| value.to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "default".to_string())
}

fn default_config_path_for_profile(profile: &str) -> PathBuf {
    xdg_config_home()
        .unwrap_or_else(|_| PathBuf::from("/tmp"))
        .join("streamrs")
        .join(format!("{profile}.toml"))
}

fn writable_image_dir_for_profile(profile: &str) -> PathBuf {
    xdg_data_home()
        .unwrap_or_else(|_| PathBuf::from("/tmp"))
        .join("streamrs")
        .join(profile)
}

fn config_load_candidates(profile: &str, preferred_path: &Path) -> Vec<PathBuf> {
    let mut candidates = vec![preferred_path.to_path_buf()];
    candidates.push(PathBuf::from(format!(
        "/usr/share/streamrs/{profile}/default.toml"
    )));
    candidates.push(PathBuf::from("/usr/share/streamrs/default/default.toml"));
    candidates.push(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("config")
            .join("default.toml"),
    );
    candidates
}

fn image_dir_candidates(profile: &str, writable_dir: &Path) -> Vec<PathBuf> {
    vec![
        writable_dir.to_path_buf(),
        PathBuf::from(format!("/usr/share/streamrs/{profile}")),
        PathBuf::from("/usr/share/streamrs/default"),
    ]
}

fn resolve_image_dirs(profile: &str, writable_dir: &Path) -> Vec<PathBuf> {
    image_dir_candidates(profile, writable_dir)
        .into_iter()
        .filter(|path| path.is_dir())
        .collect()
}

fn image_paths_for_profile(profile: &str) -> (PathBuf, Vec<PathBuf>) {
    let writable_image_dir = writable_image_dir_for_profile(profile);
    let mut image_dirs = resolve_image_dirs(profile, &writable_image_dir);
    if !image_dirs.iter().any(|path| path == &writable_image_dir) {
        image_dirs.insert(0, writable_image_dir.clone());
    }
    (writable_image_dir, image_dirs)
}

fn update_state_profile_paths(state: &mut AppState, config_path: &Path) {
    let profile = profile_from_config_path(config_path);
    let (writable_image_dir, image_dirs) = image_paths_for_profile(&profile);
    state.profile = profile;
    state.config_path = config_path.to_path_buf();
    state.writable_image_dir = writable_image_dir;
    state.image_dirs = image_dirs;
}
