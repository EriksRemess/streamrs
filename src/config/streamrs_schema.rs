use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

pub const DEFAULT_VENDOR_ID: u16 = 0x0fd9;
pub const DEFAULT_PRODUCT_ID: u16 = 0x0080;
pub const DEFAULT_USAGE: u16 = 0x0001;
pub const DEFAULT_USAGE_PAGE: u16 = 0x000c;
pub const DEFAULT_BRIGHTNESS: usize = 60;
pub const DEFAULT_KEYS_PER_PAGE: usize = crate::paging::STREAMDECK_KEY_COUNT;

pub fn default_vendor_id() -> u16 {
    DEFAULT_VENDOR_ID
}

pub fn default_product_id() -> u16 {
    DEFAULT_PRODUCT_ID
}

pub fn default_usage() -> u16 {
    DEFAULT_USAGE
}

pub fn default_usage_page() -> u16 {
    DEFAULT_USAGE_PAGE
}

pub fn default_brightness() -> usize {
    DEFAULT_BRIGHTNESS
}

pub fn default_keys_per_page() -> usize {
    DEFAULT_KEYS_PER_PAGE
}

pub fn default_icon_name() -> String {
    "blank.png".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamrsConfig {
    #[serde(default = "default_vendor_id")]
    pub vendor_id: u16,
    #[serde(default = "default_product_id")]
    pub product_id: u16,
    #[serde(default = "default_usage")]
    pub usage: u16,
    #[serde(default = "default_usage_page")]
    pub usage_page: u16,
    #[serde(default = "default_brightness")]
    pub brightness: usize,
    #[serde(default = "default_keys_per_page")]
    pub keys_per_page: usize,
    #[serde(default)]
    pub keys: Vec<StreamrsKeyBinding>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub page_wallpapers: BTreeMap<String, PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StreamrsKeyBinding {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub action: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shortcut: Option<String>,
    #[serde(default = "default_icon_name")]
    pub icon: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub clock_background: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon_on: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon_off: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status_interval_seconds: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status_interval_ms: Option<u64>,
}

impl Default for StreamrsKeyBinding {
    fn default() -> Self {
        Self {
            action: None,
            shortcut: None,
            icon: default_icon_name(),
            clock_background: None,
            icon_on: None,
            icon_off: None,
            status: None,
            status_interval_seconds: None,
            status_interval_ms: None,
        }
    }
}

impl StreamrsConfig {
    pub fn wallpaper_slots(&self, page: usize) -> [bool; crate::paging::STREAMDECK_KEY_COUNT] {
        use crate::paging::{MIN_KEYS_PER_PAGE, PagingLayout, STREAMDECK_KEY_COUNT};
        let layout = PagingLayout::new(
            STREAMDECK_KEY_COUNT,
            self.keys_per_page
                .clamp(MIN_KEYS_PER_PAGE, STREAMDECK_KEY_COUNT),
        );
        let count = self.keys.len();
        let pages = layout.page_count(count);
        std::array::from_fn(|slot| {
            if layout.navigation_slot_for_slot(page, pages, slot).is_some() {
                return false;
            }
            let Some(index) = layout.key_index_for_slot(count, page, slot) else {
                return true;
            };
            let key = &self.keys[index];
            crate::image::catalog::is_blank_background_icon_name(&key.icon)
                && [
                    &key.action,
                    &key.shortcut,
                    &key.status,
                    &key.icon_on,
                    &key.icon_off,
                ]
                .iter()
                .all(|field| field.as_deref().is_none_or(|s| s.trim().is_empty()))
        })
    }

    pub fn page_wallpaper(&self, page: usize) -> Option<&Path> {
        self.page_wallpapers
            .get(&(page + 1).to_string())
            .map(PathBuf::as_path)
    }
}

impl Default for StreamrsConfig {
    fn default() -> Self {
        Self {
            vendor_id: default_vendor_id(),
            product_id: default_product_id(),
            usage: default_usage(),
            usage_page: default_usage_page(),
            brightness: default_brightness(),
            keys_per_page: default_keys_per_page(),
            page_wallpapers: BTreeMap::new(),
            keys: vec![StreamrsKeyBinding::default(); crate::paging::STREAMDECK_KEY_COUNT],
        }
    }
}

pub fn blank_profile_config() -> StreamrsConfig {
    StreamrsConfig {
        keys: Vec::new(),
        ..StreamrsConfig::default()
    }
}
