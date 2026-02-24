use serde::{Deserialize, Serialize};

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
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StreamrsKeyBinding {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub action: Option<String>,
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
    pub status_interval_ms: Option<u64>,
}

impl Default for StreamrsKeyBinding {
    fn default() -> Self {
        Self {
            action: None,
            icon: default_icon_name(),
            clock_background: None,
            icon_on: None,
            icon_off: None,
            status: None,
            status_interval_ms: None,
        }
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
            keys: vec![StreamrsKeyBinding::default(); crate::paging::STREAMDECK_KEY_COUNT],
        }
    }
}
