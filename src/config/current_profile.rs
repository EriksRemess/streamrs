use crate::paths::{current_profile_path, default_config_path_for_profile};
use std::fs;

pub const DEFAULT_PROFILE: &str = "default";
pub const BLANK_PROFILE: &str = "blank";

fn is_discoverable_profile_name(profile: &str) -> bool {
    profile != BLANK_PROFILE && normalize_profile_name(profile).is_some()
}

pub fn normalize_profile_name(raw: &str) -> Option<String> {
    let profile = raw.trim();
    if profile.is_empty() {
        return None;
    }
    if !profile
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_')
    {
        return None;
    }
    Some(profile.to_string())
}

pub fn profile_slug_from_input(raw: &str) -> Option<String> {
    if let Some(profile) = normalize_profile_name(raw) {
        return Some(profile);
    }

    let mut slug = String::new();
    let mut previous_was_separator = false;
    for ch in raw.trim().chars() {
        if ch.is_ascii_alphanumeric() {
            slug.push(ch.to_ascii_lowercase());
            previous_was_separator = false;
            continue;
        }
        if ch.is_whitespace() || ch == '-' || ch == '_' || ch.is_ascii_punctuation() {
            if !slug.is_empty() && !previous_was_separator {
                slug.push('-');
                previous_was_separator = true;
            }
            continue;
        }
    }

    while slug.ends_with('-') {
        slug.pop();
    }

    if slug.is_empty() { None } else { Some(slug) }
}

pub fn profile_display_name(profile: &str) -> String {
    let words: Vec<String> = profile
        .split(['-', '_'])
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut chars = part.chars();
            let first = chars
                .next()
                .map(|ch| ch.to_ascii_uppercase())
                .unwrap_or_default();
            let rest = chars.as_str().to_ascii_lowercase();
            format!("{first}{rest}")
        })
        .collect();

    if words.is_empty() {
        profile.to_string()
    } else {
        words.join(" ")
    }
}

pub fn discover_profiles() -> Vec<String> {
    let mut profiles = Vec::new();
    let default_path = default_config_path_for_profile(DEFAULT_PROFILE);
    if let Some(config_dir) = default_path.parent()
        && let Ok(entries) = fs::read_dir(config_dir)
    {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|ext| ext.to_str()) != Some("toml") {
                continue;
            }
            let Some(stem) = path.file_stem().and_then(|value| value.to_str()) else {
                continue;
            };
            if is_discoverable_profile_name(stem) {
                profiles.push(stem.to_string());
            }
        }
    }
    profiles.sort_unstable();
    profiles.dedup();
    profiles
}

pub fn load_current_profile() -> Result<Option<String>, String> {
    let path = current_profile_path();
    if !path.is_file() {
        return Ok(None);
    }
    let raw = fs::read_to_string(&path)
        .map_err(|err| format!("Failed to read current profile '{}': {err}", path.display()))?;
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    normalize_profile_name(trimmed).map(Some).ok_or_else(|| {
        format!(
            "Current profile file '{}' contains invalid profile '{}'",
            path.display(),
            trimmed
        )
    })
}

pub fn save_current_profile(profile: &str) -> Result<(), String> {
    let profile = normalize_profile_name(profile)
        .ok_or_else(|| format!("Invalid profile name '{profile}'"))?;
    let path = current_profile_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|err| {
            format!(
                "Failed to create current profile directory '{}': {err}",
                parent.display()
            )
        })?;
    }
    fs::write(&path, format!("{profile}\n")).map_err(|err| {
        format!(
            "Failed to write current profile '{}': {err}",
            path.display()
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Mutex, OnceLock};

    static TEST_COUNTER: AtomicUsize = AtomicUsize::new(0);

    fn env_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    fn with_temp_xdg_config_home(name: &str, run: impl FnOnce()) {
        let _guard = env_lock().lock().expect("env lock should be available");
        let id = TEST_COUNTER.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("streamrs-current-profile-tests-{name}-{id}"));
        fs::create_dir_all(&dir).expect("test temp config dir should be creatable");

        let previous = std::env::var_os("XDG_CONFIG_HOME");
        // SAFETY: Tests hold a process-wide mutex so env mutation is serialized.
        unsafe {
            std::env::set_var("XDG_CONFIG_HOME", &dir);
        }

        run();

        if let Some(value) = previous {
            // SAFETY: Tests hold a process-wide mutex so env mutation is serialized.
            unsafe {
                std::env::set_var("XDG_CONFIG_HOME", value);
            }
        } else {
            // SAFETY: Tests hold a process-wide mutex so env mutation is serialized.
            unsafe {
                std::env::remove_var("XDG_CONFIG_HOME");
            }
        }

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn blank_profile_is_not_discoverable() {
        assert!(!is_discoverable_profile_name(BLANK_PROFILE));
    }

    #[test]
    fn valid_profiles_remain_discoverable() {
        assert!(is_discoverable_profile_name(DEFAULT_PROFILE));
        assert!(is_discoverable_profile_name("test_profile-1"));
    }

    #[test]
    fn save_and_load_current_profile_round_trip() {
        with_temp_xdg_config_home("roundtrip", || {
            save_current_profile("test_profile").expect("current profile should save");
            let loaded = load_current_profile().expect("current profile should load");
            assert_eq!(loaded.as_deref(), Some("test_profile"));
        });
    }

    #[test]
    fn discover_profiles_excludes_blank_profile_file() {
        with_temp_xdg_config_home("discover", || {
            let root = std::env::var_os("XDG_CONFIG_HOME")
                .map(PathBuf::from)
                .expect("XDG_CONFIG_HOME should be set for this test");
            let config_dir = root.join("streamrs");
            fs::create_dir_all(&config_dir).expect("config dir should be created");
            fs::write(config_dir.join("default.toml"), "brightness = 60\n")
                .expect("default profile fixture should be written");
            fs::write(config_dir.join("blank.toml"), "brightness = 60\n")
                .expect("blank profile fixture should be written");

            let profiles = discover_profiles();
            assert!(profiles.iter().any(|profile| profile == DEFAULT_PROFILE));
            assert!(!profiles.iter().any(|profile| profile == BLANK_PROFILE));
        });
    }

    #[test]
    fn load_current_profile_rejects_invalid_value() {
        with_temp_xdg_config_home("invalid-current", || {
            let path = current_profile_path();
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).expect("current profile dir should be created");
            }
            fs::write(&path, "bad profile!\n").expect("invalid fixture should be written");

            let err = load_current_profile().expect_err("invalid profile should fail to load");
            assert!(err.contains("invalid profile"));
        });
    }

    #[test]
    fn profile_slug_from_input_accepts_spaces() {
        assert_eq!(
            profile_slug_from_input("My Profile").as_deref(),
            Some("my-profile")
        );
        assert_eq!(
            profile_slug_from_input("  Work Setup  ").as_deref(),
            Some("work-setup")
        );
    }

    #[test]
    fn profile_display_name_formats_slug_for_ui() {
        assert_eq!(profile_display_name("my-profile"), "My Profile");
        assert_eq!(profile_display_name("work_setup"), "Work Setup");
        assert_eq!(profile_display_name("work"), "Work");
    }
}
