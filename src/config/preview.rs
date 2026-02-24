use serde::Deserialize;
use std::path::Path;

#[derive(Debug, Deserialize)]
pub(super) struct Config {
    pub(super) keys: Vec<KeyBinding>,
}

#[derive(Debug, Deserialize)]
pub(super) struct KeyBinding {
    pub(super) icon: String,
    pub(super) icon_on: Option<String>,
    pub(super) icon_off: Option<String>,
    pub(super) status: Option<String>,
}

pub(super) fn load_config(path: &Path) -> Result<Config, String> {
    streamrs::config::toml::load_from_file(path)
}
