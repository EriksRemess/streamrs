use crate::config::current_profile::BLANK_PROFILE;
use crate::config::streamrs_schema::{StreamrsConfig, blank_profile_config};
use crate::paths::{config_load_candidates, profile_from_config_path};
use std::path::Path;

#[derive(serde::Deserialize)]
struct BrightnessField {
    brightness: Option<toml::Spanned<usize>>,
}

pub fn blank_config_from_str(raw: &str) -> StreamrsConfig {
    let mut config = blank_profile_config();
    if let Ok(BrightnessField {
        brightness: Some(setting),
    }) = toml::from_str(raw)
    {
        config.brightness = setting.into_inner();
    }
    config
}

fn read_profile_text(path: &Path) -> std::io::Result<String> {
    use std::io::Read;
    use std::os::unix::fs::OpenOptionsExt;
    let mut file = std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NONBLOCK)
        .open(path)?;
    if !file.metadata()?.is_file() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "not a regular file",
        ));
    }
    let mut raw = String::new();
    file.read_to_string(&mut raw)?;
    Ok(raw)
}

pub fn load_config_for_profile(path: &Path, profile: &str) -> Result<StreamrsConfig, String> {
    let profile = if profile_from_config_path(path) == BLANK_PROFILE {
        BLANK_PROFILE
    } else {
        profile
    };
    read_profile_config(path, profile, true)
}

pub fn read_profile_config(
    path: &Path,
    profile: &str,
    allow_missing: bool,
) -> Result<StreamrsConfig, String> {
    let blank = profile == BLANK_PROFILE;
    match read_profile_text(path) {
        Ok(raw) if blank => Ok(blank_config_from_str(&raw)),
        Ok(raw) => super::toml::parse_from_str(path, &raw),
        Err(err) if allow_missing && err.kind() == std::io::ErrorKind::NotFound => Ok(if blank {
            blank_profile_config()
        } else {
            StreamrsConfig::default()
        }),
        Err(err) => Err(super::toml::read_error(path, err)),
    }
}

pub fn try_update_config(
    path: &Path,
    edit: impl FnOnce(&mut StreamrsConfig),
) -> Result<StreamrsConfig, String> {
    let _lock = super::toml::lock_for_update(path, true, |dir| dir.try_lock().map_err(Into::into))?;
    let mut config = load_config_for_profile(path, &profile_from_config_path(path))?;
    edit(&mut config);
    save(path, &config)?;
    Ok(config)
}

// Keep comments, unknown options and the original formatting when updating one scalar.
pub(crate) fn save_brightness(
    path: &Path,
    profile: &str,
    value: u8,
) -> Result<StreamrsConfig, String> {
    let blank = profile == BLANK_PROFILE;
    let mut raw = match read_profile_text(path) {
        Ok(raw) => raw,
        Err(err) if blank && err.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(err) => return Err(super::toml::read_error(path, err)),
    };
    let mut config = if blank {
        blank_config_from_str(&raw)
    } else {
        super::toml::parse_from_str(path, &raw)?
    };
    let field: BrightnessField = super::toml::parse_from_str(path, &raw)?;
    match field.brightness {
        Some(old) if *old.get_ref() == usize::from(value) => return Ok(config),
        Some(old) => raw.replace_range(old.span(), &value.to_string()),
        None => {
            let start = raw
                .strip_prefix('\u{feff}')
                .map_or(0, |_| '\u{feff}'.len_utf8());
            let newline = if raw
                .split_once('\n')
                .is_some_and(|(line, _)| line.ends_with('\r'))
            {
                "\r\n"
            } else {
                "\n"
            };
            raw.insert_str(start, &format!("brightness = {value}{newline}"));
        }
    }
    super::toml::save_text(path, &raw)?;
    config.brightness = value.into();
    Ok(config)
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

pub fn create_if_missing(path: &Path, config: &StreamrsConfig) -> Result<bool, String> {
    super::toml::create_pretty_if_missing(path, config)
}

pub fn save(path: &Path, config: &StreamrsConfig) -> Result<(), String> {
    crate::config::toml::save_to_file_pretty(path, config)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn device_config_uses_blank_defaults_and_requires_explicit_paths() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("desk.toml");
        assert!(read_profile_config(&path, "desk", false).is_err());
        let defaults = read_profile_config(&path, "desk", true).unwrap();
        std::fs::write(&path, "vendor_id = 1234\nbrightness = 34").unwrap();
        let blank = read_profile_config(&path, BLANK_PROFILE, false).unwrap();
        assert_eq!(
            (blank.vendor_id, blank.brightness),
            (defaults.vendor_id, 34)
        );
        assert_eq!(
            read_profile_config(&path, "desk", false).unwrap().vendor_id,
            1234
        );
    }

    #[test]
    fn special_files_are_rejected_before_parsing_a_profile() {
        let path = Path::new("/dev/null");
        assert_eq!(
            read_profile_text(path).unwrap_err().kind(),
            std::io::ErrorKind::InvalidInput
        );
        assert!(load_config_for_profile(path, BLANK_PROFILE).is_err());
    }

    #[test]
    fn busy_profile_does_not_block_an_interactive_edit() {
        use std::sync::mpsc;
        use std::time::Duration;

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("desk.toml");
        save(&path, &StreamrsConfig::default()).unwrap();
        let lock = crate::config::toml::lock_for_update(&path, false, std::fs::File::lock).unwrap();

        std::thread::scope(|scope| {
            let (sent, received) = mpsc::channel();
            let path = &path;
            scope.spawn(move || {
                let result = try_update_config(path, |config| config.keys.clear());
                let _ = sent.send(result);
            });
            let result = received.recv_timeout(Duration::from_secs(2));
            drop(lock);
            assert!(result.expect("profile edit blocked").is_err());
        });
        assert!(
            !load_config_for_profile(&path, "desk")
                .unwrap()
                .keys
                .is_empty()
        );
    }

    #[test]
    fn brightness_preserves_everything_outside_the_root_value() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("desk.toml");
        let raw = "\u{feff}# café\r\n'brightness' = 0x32 # level\r\n[[keys]]\r\nicon = 'brightness'\r\nbrightness = 7\r\n";
        std::fs::write(&path, raw).unwrap();
        let config = save_brightness(&path, "desk", 0).unwrap();
        assert_eq!(config.brightness, 0);
        assert_eq!(config.keys[0].icon, "brightness");
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            raw.replace("0x32", "0")
        );
    }

    #[test]
    fn brightness_is_inserted_at_root_and_leaves_invalid_profiles_intact() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("desk.toml");
        let raw = "\u{feff}[[keys]]\r\nicon = 'test.png'\r\n";
        std::fs::write(&path, raw).unwrap();
        save_brightness(&path, "desk", 100).unwrap();
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            raw.replace('\u{feff}', "\u{feff}brightness = 100\r\n")
        );
        std::fs::write(&path, "keys = 123").unwrap();
        assert!(save_brightness(&path, "desk", 0).is_err());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "keys = 123");
    }

    #[test]
    fn initializing_blank_keeps_brightness_saved_by_another_writer() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("blank.toml");
        assert!(create_if_missing(&path, &blank_profile_config()).unwrap());
        save_brightness(&path, BLANK_PROFILE, 43).unwrap();
        let saved = std::fs::read(&path).unwrap();
        assert!(!create_if_missing(&path, &blank_profile_config()).unwrap());
        assert_eq!(std::fs::read(path).unwrap(), saved);
    }

    #[test]
    fn blank_profile_remembers_brightness_and_ignores_stale_keys() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("blank.toml");
        save_brightness(&path, BLANK_PROFILE, 20).unwrap();
        assert_eq!(
            load_config_for_profile(&path, BLANK_PROFILE)
                .unwrap()
                .brightness,
            20
        );
        let raw = "brightness = 20\n[[keys]]\naction = 'stale'\n";
        let config = blank_config_from_str(raw);
        assert_eq!(config.brightness, 20);
        assert!(config.keys.is_empty());
    }

    #[test]
    fn applying_pending_keys_keeps_the_latest_saved_brightness() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("desk.toml");
        save(&path, &StreamrsConfig::default()).unwrap();
        let mut pending = load_config_for_profile(&path, "desk").unwrap();
        pending.keys[0].icon = "new.png".into();
        let config = save_brightness(&path, "desk", 35).unwrap();
        assert_eq!(config.keys[0].icon, "blank.png");
        try_update_config(&path, |saved| saved.keys = pending.keys).unwrap();
        let config = load_config_for_profile(&path, "desk").unwrap();
        assert_eq!(config.brightness, 35);
        assert_eq!(config.keys[0].icon, "new.png");
    }
}
