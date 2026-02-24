use std::env;
use std::path::{Path, PathBuf};

pub fn home_dir() -> Result<PathBuf, String> {
    env::var_os("HOME")
        .map(PathBuf::from)
        .ok_or_else(|| "HOME is not set".to_string())
}

pub fn xdg_config_home() -> Result<PathBuf, String> {
    if let Some(path) = env::var_os("XDG_CONFIG_HOME") {
        return Ok(PathBuf::from(path));
    }
    Ok(home_dir()?.join(".config"))
}

pub fn xdg_data_home() -> Result<PathBuf, String> {
    if let Some(path) = env::var_os("XDG_DATA_HOME") {
        return Ok(PathBuf::from(path));
    }
    Ok(home_dir()?.join(".local/share"))
}

pub fn profile_from_config_path(path: &Path) -> String {
    path.file_stem()
        .and_then(|stem| stem.to_str())
        .map(|value| value.to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "default".to_string())
}

pub fn default_config_path_for_profile(profile: &str) -> PathBuf {
    xdg_config_home()
        .unwrap_or_else(|_| PathBuf::from("/tmp"))
        .join("streamrs")
        .join(format!("{profile}.toml"))
}

pub fn writable_image_dir_for_profile(profile: &str) -> PathBuf {
    xdg_data_home()
        .unwrap_or_else(|_| PathBuf::from("/tmp"))
        .join("streamrs")
        .join(profile)
}

pub fn config_load_candidates(profile: &str, preferred_path: &Path) -> Vec<PathBuf> {
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

pub fn image_dir_candidates(profile: &str, writable_dir: &Path) -> Vec<PathBuf> {
    vec![
        writable_dir.to_path_buf(),
        PathBuf::from(format!("/usr/share/streamrs/{profile}")),
        PathBuf::from("/usr/share/streamrs/default"),
    ]
}
