pub(crate) use adw::prelude::*;
pub(crate) use adw::{Application, ApplicationWindow, HeaderBar};
pub(crate) use gtk::{
    Align, Box as GtkBox, Button, CssProvider, DropDown, Entry, Fixed, Image, Label, Orientation,
    Overlay, Paned, Picture, STYLE_PROVIDER_PRIORITY_APPLICATION, ScrolledWindow, SpinButton,
};
pub(crate) use image::RgbaImage;
pub(crate) use image::imageops::{FilterType::Lanczos3, resize};
pub(crate) use std::cell::{Cell, RefCell};
pub(crate) use std::env;
pub(crate) use std::fs;
pub(crate) use std::path::{Path, PathBuf};
pub(crate) use std::rc::Rc;
pub(crate) use streamrs::config::streamrs_profile;
pub(crate) use streamrs::config::streamrs_schema::{
    StreamrsConfig as Config, StreamrsKeyBinding as KeyBinding, default_icon_name,
};
pub(crate) use streamrs::image::cache_fs::{cached_png_path_if_valid, write_cached_png};
pub(crate) use streamrs::image::catalog::{
    copy_supported_image_into_dir, discover_icons as discover_icons_generic,
    discover_png_backgrounds_with_prefix,
};
pub(crate) use streamrs::image::clock::{
    CLOCK_BACKGROUND_ICON, CLOCK_ICON_ALIAS, current_clock_text, is_clock_icon as icon_is_clock,
    render_clock_segments_svg,
};
pub(crate) use streamrs::image::effects::apply_rounded_corners;
pub(crate) use streamrs::image::svg::load_svg_data as load_svg_image_data;
pub(crate) use streamrs::paging::{
    NavigationSlot as ReservedNavigationSlot, PagingLayout, STREAMDECK_KEY_COUNT,
};
pub(crate) use streamrs::paths::{
    default_config_path_for_profile, profile_from_config_path, writable_image_dir_for_profile,
};

pub(crate) const KEY_COUNT: usize = STREAMDECK_KEY_COUNT;
pub(crate) const DEFAULT_STATUS_INTERVAL_MS: u64 = 1000;
pub(crate) const MIN_STATUS_INTERVAL_MS: u64 = 100;
pub(crate) const MAX_STATUS_INTERVAL_MS: u64 = 60_000;

pub(crate) const NAV_PREVIOUS_ICON: &str = "stream-deck-previous-page.png";
pub(crate) const NAV_NEXT_ICON: &str = "stream-deck-next-page.png";

pub(crate) const TEMPLATE_RENDER_WIDTH: u32 = 1560;
pub(crate) const TEMPLATE_RENDER_HEIGHT: u32 = 1108;
pub(crate) const PREVIEW_WIDTH: u32 = 936;
pub(crate) const PREVIEW_HEIGHT: u32 = 665;
pub(crate) const DECK_MIN_SCALE: f32 = 0.5;
pub(crate) const DECK_MIN_WIDTH: i32 = (PREVIEW_WIDTH as f32 * DECK_MIN_SCALE) as i32;
pub(crate) const DECK_MIN_HEIGHT: i32 = (PREVIEW_HEIGHT as f32 * DECK_MIN_SCALE) as i32;
pub(crate) const INSPECTOR_MIN_WIDTH: i32 = 390;
pub(crate) const WINDOW_MIN_WIDTH: i32 = DECK_MIN_WIDTH + INSPECTOR_MIN_WIDTH + 120;
pub(crate) const WINDOW_MIN_HEIGHT: i32 = DECK_MIN_HEIGHT + 190;

#[derive(Debug, Clone)]
pub(crate) struct AppState {
    pub(crate) config: Config,
    pub(crate) config_path: PathBuf,
    pub(crate) profile: String,
    pub(crate) image_dirs: Vec<PathBuf>,
    pub(crate) writable_image_dir: PathBuf,
}

#[derive(Clone)]
pub(crate) struct EditorWidgets {
    pub(crate) config_path_entry: Entry,
    pub(crate) selected_label: Label,
    pub(crate) action_entry: Entry,
    pub(crate) icon_kind_dropdown: DropDown,
    pub(crate) icon_label: Label,
    pub(crate) icon_row: GtkBox,
    pub(crate) icon_dropdown: DropDown,
    pub(crate) clock_background_label: Label,
    pub(crate) clock_background_dropdown: DropDown,
    pub(crate) status_command_label: Label,
    pub(crate) status_entry: Entry,
    pub(crate) icon_on_label: Label,
    pub(crate) icon_on_dropdown: DropDown,
    pub(crate) icon_off_label: Label,
    pub(crate) icon_off_dropdown: DropDown,
    pub(crate) interval_label: Label,
    pub(crate) interval_spin: SpinButton,
    pub(crate) icon_preview: Picture,
    pub(crate) apply_button: Button,
    pub(crate) clear_button: Button,
    pub(crate) status_label: Label,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct KeySlot {
    pub(crate) x0: u32,
    pub(crate) y0: u32,
    pub(crate) x1: u32,
    pub(crate) y1: u32,
    pub(crate) cx: f32,
    pub(crate) cy: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum EditorMode {
    Regular,
    Status,
    Clock,
}

pub(crate) fn resolve_image_dirs(profile: &str, writable_dir: &Path) -> Vec<PathBuf> {
    streamrs::paths::image_dir_candidates(profile, writable_dir)
        .into_iter()
        .filter(|path| path.is_dir())
        .collect()
}

pub(crate) fn image_paths_for_profile(profile: &str) -> (PathBuf, Vec<PathBuf>) {
    let writable_image_dir = writable_image_dir_for_profile(profile);
    let mut image_dirs = resolve_image_dirs(profile, &writable_image_dir);
    if !image_dirs.iter().any(|path| path == &writable_image_dir) {
        image_dirs.insert(0, writable_image_dir.clone());
    }
    (writable_image_dir, image_dirs)
}

pub(crate) fn update_state_profile_paths(state: &mut AppState, config_path: &Path) {
    let profile = profile_from_config_path(config_path);
    let (writable_image_dir, image_dirs) = image_paths_for_profile(&profile);
    state.profile = profile;
    state.config_path = config_path.to_path_buf();
    state.writable_image_dir = writable_image_dir;
    state.image_dirs = image_dirs;
}
