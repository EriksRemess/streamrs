use crate::config::current_profile::BLANK_PROFILE;
use crate::config::streamrs_schema::{StreamrsConfig, blank_profile_config};
use crate::paths::{config_load_candidates, profile_from_config_path};
use std::path::Path;

pub fn load_config_for_profile(path: &Path, profile: &str) -> Result<StreamrsConfig, String> {
    if profile == BLANK_PROFILE || profile_from_config_path(path) == BLANK_PROFILE {
        return Ok(blank_profile_config());
    }
    if !path.is_file() {
        return Ok(StreamrsConfig::default());
    }
    crate::config::toml::load_from_file(path)
}

pub fn load_with_fallbacks(path: &Path) -> Result<Option<StreamrsConfig>, String> {
    let profile = profile_from_config_path(path);
    for candidate in config_load_candidates(&profile, path) {
        if !candidate.is_file() {
            continue;
        }
        let config = crate::config::toml::load_from_file::<StreamrsConfig>(&candidate)?;
        return Ok(Some(config));
    }
    Ok(None)
}

pub fn save(path: &Path, config: &StreamrsConfig) -> Result<(), String> {
    crate::config::toml::save_to_file_pretty(path, config)
}
