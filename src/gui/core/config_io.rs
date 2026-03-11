use super::*;

pub(crate) fn load_config(path: &Path) -> Result<Config, String> {
    if profile_from_config_path(path) == BLANK_PROFILE {
        return Ok(streamrs::config::streamrs_schema::blank_profile_config());
    }

    if !path.is_file() {
        return Ok(Config::default());
    }

    let mut config = streamrs::config::toml::load_from_file::<Config>(path)?;
    normalize_config(&mut config);
    Ok(config)
}

pub(crate) fn save_config(path: &Path, config: &Config) -> Result<(), String> {
    streamrs_profile::save(path, config)
}

pub(crate) fn signal_daemon_reload() -> Result<(), String> {
    let systemctl = std::process::Command::new("systemctl")
        .args(["--user", "kill", "-s", "HUP", "streamrs.service"])
        .status();
    if let Ok(status) = &systemctl
        && status.success()
    {
        return Ok(());
    }

    let pkill = std::process::Command::new("pkill")
        .args(["-HUP", "-x", "streamrs"])
        .status();
    if let Ok(status) = &pkill
        && status.success()
    {
        return Ok(());
    }

    let systemctl_err = match systemctl {
        Ok(status) => format!("systemctl exit status {status}"),
        Err(err) => format!("systemctl failed: {err}"),
    };
    let pkill_err = match pkill {
        Ok(status) => format!("pkill exit status {status}"),
        Err(err) => format!("pkill failed: {err}"),
    };
    Err(format!(
        "Failed to signal streamrs daemon ({systemctl_err}; {pkill_err})"
    ))
}

pub(crate) fn profile_slug_from_input(raw: &str) -> Option<String> {
    profile_slug_from_input_generic(raw)
}

pub(crate) fn profile_display_name(profile: &str) -> String {
    profile_display_name_generic(profile)
}

pub(crate) fn discover_profiles() -> Vec<String> {
    discover_profiles_generic()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static TEST_COUNTER: AtomicUsize = AtomicUsize::new(0);

    fn test_temp_dir(name: &str) -> PathBuf {
        let id = TEST_COUNTER.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("streamrs-gui-config-io-tests-{name}-{id}"));
        fs::create_dir_all(&dir).expect("test directory should be creatable");
        dir
    }

    #[test]
    fn load_config_missing_profile_uses_blank_template_not_fallback_default() {
        let dir = test_temp_dir("missing-profile");
        let path = dir.join("new_profile.toml");
        let config = load_config(&path).expect("missing profile config should load as template");
        assert_eq!(config.keys.len(), KEY_COUNT);
        assert_eq!(config.keys[0].icon, "blank.png");
        assert!(
            config.keys[0].action.is_none(),
            "new profile template should not inherit launcher actions"
        );
    }

    #[test]
    fn load_config_reads_exact_profile_file_when_present() {
        let dir = test_temp_dir("existing-profile");
        let path = dir.join("test_profile.toml");
        fs::write(
            &path,
            r#"
            [[keys]]
            icon = "custom.png"
            "#,
        )
        .expect("fixture config should be written");

        let config = load_config(&path).expect("existing profile config should load");
        assert_eq!(config.keys[0].icon, "custom.png");
    }

    #[test]
    fn load_config_blank_profile_remains_empty() {
        let dir = test_temp_dir("blank-profile");
        let path = dir.join("blank.toml");
        let config = load_config(&path).expect("blank profile config should load");
        assert!(
            config.keys.is_empty(),
            "blank profile must remain empty and show black deck"
        );
    }
}
