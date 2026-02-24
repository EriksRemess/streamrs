use serde::{Serialize, de::DeserializeOwned};
use std::fs;
use std::path::Path;

pub fn read_to_string(path: &Path) -> Result<String, String> {
    fs::read_to_string(path)
        .map_err(|err| format!("Failed to read config '{}': {err}", path.display()))
}

pub fn parse_from_str<T: DeserializeOwned>(path: &Path, raw: &str) -> Result<T, String> {
    toml::from_str(raw).map_err(|err| format!("Failed to parse config '{}': {err}", path.display()))
}

pub fn load_from_file<T: DeserializeOwned>(path: &Path) -> Result<T, String> {
    let raw = read_to_string(path)?;
    parse_from_str(path, &raw)
}

pub fn to_string_pretty<T: Serialize>(path: &Path, value: &T) -> Result<String, String> {
    toml::to_string_pretty(value)
        .map_err(|err| format!("Failed to serialize config '{}': {err}", path.display()))
}

pub fn save_to_file_pretty<T: Serialize>(path: &Path, value: &T) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|err| {
            format!(
                "Failed to create config directory '{}': {err}",
                parent.display()
            )
        })?;
    }

    let output = to_string_pretty(path, value)?;
    fs::write(path, output)
        .map_err(|err| format!("Failed to write config '{}': {err}", path.display()))
}
