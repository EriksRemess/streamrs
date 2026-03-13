use super::{
    Config, DEFAULT_STATUS_CHECK_INTERVAL_MS, KEY_COUNT, KeyBinding, MIN_KEYS_PER_PAGE,
    MIN_STATUS_CHECK_INTERVAL_MS,
};
use std::path::Path;
use std::time::Duration;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ConfiguredAction {
    Launch(String),
    KeyboardShortcut(String),
}

pub(crate) fn read_config_file(path: &Path) -> Result<String, String> {
    streamrs::config::toml::read_to_string(path)
}

fn validate_config(path: &Path, config: &Config) -> Result<(), String> {
    if config.keys.is_empty() {
        return Err(format!("Config '{}' has no keys", path.display()));
    }
    if !(MIN_KEYS_PER_PAGE..=KEY_COUNT).contains(&config.keys_per_page) {
        return Err(format!(
            "Config '{}' has invalid keys_per_page {}; expected {}..={}",
            path.display(),
            config.keys_per_page,
            MIN_KEYS_PER_PAGE,
            KEY_COUNT
        ));
    }
    Ok(())
}

pub(crate) fn load_config(path: &Path, profile: &str) -> Result<Config, String> {
    let config = streamrs::config::streamrs_profile::load_config_for_profile(path, profile)?;
    validate_config(path, &config)?;
    Ok(config)
}

#[cfg(test)]
pub(crate) fn parse_config(path: &Path, raw: &str) -> Result<Config, String> {
    let config: Config = streamrs::config::toml::parse_from_str(path, raw)?;
    validate_config(path, &config)?;
    Ok(config)
}

pub(crate) fn key_launch_action(key: &KeyBinding) -> Option<String> {
    key.action.as_ref().and_then(|action| {
        let trimmed = action.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    })
}

pub(crate) fn key_keyboard_shortcut(key: &KeyBinding) -> Option<String> {
    trimmed_non_empty(key.shortcut.as_deref())
}

pub(crate) fn key_configured_action(key: &KeyBinding) -> Option<ConfiguredAction> {
    if let Some(shortcut) = key_keyboard_shortcut(key) {
        return Some(ConfiguredAction::KeyboardShortcut(shortcut));
    }

    key_launch_action(key).map(ConfiguredAction::Launch)
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

pub(crate) fn key_status_command(key: &KeyBinding) -> Option<String> {
    trimmed_non_empty(key.status.as_deref())
}

pub(crate) fn is_launcher_like_command(command: &str) -> bool {
    let mut parts = command.split_whitespace();
    matches!(
        (parts.next(), parts.next()),
        (Some("open"), _) | (Some("xdg-open"), _) | (Some("gio"), Some("open"))
    )
}

pub(crate) fn key_status_icon_on(key: &KeyBinding) -> String {
    trimmed_non_empty(key.icon_on.as_deref()).unwrap_or_else(|| key.icon.clone())
}

pub(crate) fn key_status_icon_off(key: &KeyBinding) -> String {
    trimmed_non_empty(key.icon_off.as_deref()).unwrap_or_else(|| key.icon.clone())
}

pub(crate) fn key_status_interval(key: &KeyBinding) -> Duration {
    let interval_ms = key
        .status_interval_ms
        .unwrap_or(DEFAULT_STATUS_CHECK_INTERVAL_MS)
        .max(MIN_STATUS_CHECK_INTERVAL_MS);
    Duration::from_millis(interval_ms)
}

pub(crate) fn key_clock_background(key: &KeyBinding) -> Option<String> {
    trimmed_non_empty(key.clock_background.as_deref())
}
