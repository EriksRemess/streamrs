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

pub fn streamrs_config_dir() -> Result<PathBuf, String> {
    Ok(xdg_config_home()?.join("streamrs"))
}

pub fn xdg_data_home() -> Result<PathBuf, String> {
    if let Some(path) = env::var_os("XDG_DATA_HOME") {
        return Ok(PathBuf::from(path));
    }
    Ok(home_dir()?.join(".local/share"))
}

pub fn xdg_state_home() -> Result<PathBuf, String> {
    if let Some(path) = env::var_os("XDG_STATE_HOME") {
        return Ok(PathBuf::from(path));
    }
    Ok(home_dir()?.join(".local/state"))
}

pub fn current_profile_path() -> PathBuf {
    streamrs_config_dir()
        .unwrap_or_else(|_| PathBuf::from("/tmp").join("streamrs"))
        .join("current_profile")
}

pub fn streamrs_state_dir() -> Result<PathBuf, String> {
    Ok(xdg_state_home()?.join("streamrs"))
}

pub fn streamrs_state_path() -> PathBuf {
    xdg_state_home()
        .unwrap_or_else(|_| PathBuf::from("/tmp").join("streamrs"))
        .join("streamrs")
        .join("state.toml")
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

pub fn writable_icon_dir() -> PathBuf {
    xdg_data_home()
        .unwrap_or_else(|_| PathBuf::from("/tmp"))
        .join("streamrs")
        .join("icons")
}

pub fn writable_image_dir_for_profile(_profile: &str) -> PathBuf {
    writable_icon_dir()
}

pub fn config_load_candidates(profile: &str, preferred_path: &Path) -> Vec<PathBuf> {
    if profile == "blank" {
        return vec![preferred_path.to_path_buf()];
    }
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
    let mut dirs = vec![
        writable_dir.to_path_buf(),
        PathBuf::from("/usr/share/streamrs/icons"),
    ];
    dirs.dedup();
    let _ = profile;
    dirs
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writable_image_dir_is_shared_across_profiles() {
        let default_dir = writable_image_dir_for_profile("default");
        let test_dir = writable_image_dir_for_profile("test");
        assert_eq!(default_dir, test_dir);
        assert!(default_dir.ends_with("streamrs/icons"));
    }

    #[test]
    fn image_candidates_include_shared_dir_first() {
        let shared = writable_icon_dir();
        let candidates = image_dir_candidates("test", &shared);
        assert_eq!(candidates.first(), Some(&shared));
        assert_eq!(candidates.len(), 2);
        assert!(
            candidates
                .iter()
                .any(|path| path == Path::new("/usr/share/streamrs/icons")),
            "packaged shared icon dir should be part of candidates"
        );
    }

    #[test]
    fn state_path_uses_streamrs_state_dir() {
        let path = streamrs_state_path();
        assert!(path.ends_with("state/streamrs/state.toml"));
    }
}
