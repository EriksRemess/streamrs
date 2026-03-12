use crate::paths::{current_profile_path, default_config_path_for_profile};
use std::fs;
use std::io::Write;
use std::time::{SystemTime, UNIX_EPOCH};

pub const DEFAULT_PROFILE: &str = "default";
pub const BLANK_PROFILE: &str = "blank";

fn is_discoverable_profile_name(profile: &str) -> bool {
    profile != BLANK_PROFILE && normalize_profile_name(profile).is_some()
}

fn normalize_current_profile_candidate(raw: &str) -> Option<String> {
    let mut candidate = raw.trim();
    if let Some((key, value)) = candidate.split_once('=')
        && key.trim().eq_ignore_ascii_case("profile")
    {
        candidate = value.trim();
    }
    candidate = candidate.trim_matches(|ch| ch == '"' || ch == '\'');
    normalize_profile_name(candidate)
}

fn parse_current_profile_contents(raw: &str) -> Result<Option<String>, String> {
    let mut first_invalid = None::<String>;
    for line in raw.lines() {
        let mut candidate = line.trim();
        if candidate.is_empty() || candidate.starts_with('#') {
            continue;
        }
        candidate = candidate.trim_start_matches('\u{feff}').trim_start();
        if candidate.is_empty() || candidate.starts_with('#') {
            continue;
        }
        if let Some(profile) = normalize_current_profile_candidate(candidate) {
            return Ok(Some(profile));
        }
        if first_invalid.is_none() {
            first_invalid = Some(candidate.to_string());
        }
    }
    if let Some(invalid) = first_invalid {
        return Err(invalid);
    }
    Ok(None)
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
    let bytes = fs::read(&path)
        .map_err(|err| format!("Failed to read current profile '{}': {err}", path.display()))?;
    let raw = String::from_utf8_lossy(&bytes);
    parse_current_profile_contents(&raw).map_err(|invalid| {
        format!(
            "Current profile file '{}' contains invalid profile '{}'",
            path.display(),
            invalid
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
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("current_profile");
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let tmp_path = path.with_file_name(format!(".{file_name}.tmp-{}-{unique}", std::process::id()));
    let data = format!("{profile}\n");

    let mut file = fs::File::create(&tmp_path).map_err(|err| {
        format!(
            "Failed to create temporary current profile '{}': {err}",
            tmp_path.display()
        )
    })?;
    file.write_all(data.as_bytes()).map_err(|err| {
        format!(
            "Failed to write temporary current profile '{}': {err}",
            tmp_path.display()
        )
    })?;
    file.sync_all().map_err(|err| {
        format!(
            "Failed to sync temporary current profile '{}': {err}",
            tmp_path.display()
        )
    })?;

    fs::rename(&tmp_path, &path).map_err(|err| {
        let _ = fs::remove_file(&tmp_path);
        format!(
            "Failed to write current profile '{}': {err}",
            path.display()
        )
    })
}

pub fn save_current_profile_if_missing(profile: &str) -> Result<bool, String> {
    match load_current_profile() {
        Ok(Some(_)) => Ok(false),
        Ok(None) => {
            save_current_profile(profile)?;
            Ok(true)
        }
        Err(err) => Err(err),
    }
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
    fn load_current_profile_accepts_profile_assignment_line() {
        with_temp_xdg_config_home("profile-assignment", || {
            let path = current_profile_path();
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).expect("current profile dir should be created");
            }
            fs::write(&path, "profile = \"test_profile\"\n")
                .expect("assignment fixture should be written");

            let loaded = load_current_profile().expect("profile assignment should load");
            assert_eq!(loaded.as_deref(), Some("test_profile"));
        });
    }

    #[test]
    fn load_current_profile_ignores_comments_and_blank_lines() {
        with_temp_xdg_config_home("comments-and-blanks", || {
            let path = current_profile_path();
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).expect("current profile dir should be created");
            }
            fs::write(&path, "\n# selected profile\n\nwork_setup\n")
                .expect("comment fixture should be written");

            let loaded = load_current_profile().expect("commented profile should load");
            assert_eq!(loaded.as_deref(), Some("work_setup"));
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

    #[test]
    fn save_current_profile_if_missing_writes_once() {
        with_temp_xdg_config_home("save-if-missing", || {
            let wrote = save_current_profile_if_missing("test_profile")
                .expect("missing current profile should be created");
            assert!(wrote, "first write should persist profile");

            let wrote_again = save_current_profile_if_missing("default")
                .expect("existing current profile should not be overwritten");
            assert!(
                !wrote_again,
                "existing current profile should remain unchanged"
            );

            let loaded = load_current_profile().expect("current profile should load");
            assert_eq!(loaded.as_deref(), Some("test_profile"));
        });
    }
}
