use std::path::Path;
pub(super) use streamrs::config::streamrs_schema::StreamrsConfig as Config;

pub(super) fn load_config(path: &Path, profile: &str) -> Result<Config, String> {
    if profile == streamrs::config::current_profile::BLANK_PROFILE {
        return Ok(streamrs::config::streamrs_schema::blank_profile_config());
    }
    streamrs::config::toml::load_from_file(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn preview_accepts_profile_defaults() {
        let parse = |raw| {
            streamrs::config::toml::parse_from_str::<Config>(Path::new("profile.toml"), raw)
                .unwrap()
        };
        assert!(parse("brightness = 60").keys.is_empty());
        assert_eq!(parse("[[keys]]\naction = 'test'").keys[0].icon, "blank.png");
    }

    #[test]
    fn reserved_blank_profile_always_has_one_empty_page() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("blank.toml");
        let check = || {
            let config = load_config(&path, "blank").unwrap();
            assert!(config.keys.is_empty());
            assert!(config.page_wallpapers.is_empty());
        };
        check();
        std::fs::write(
            &path,
            "[page_wallpapers]\n'2'='wall.png'\n[[keys]]\nicon='old.png'",
        )
        .unwrap();
        check();
    }
}
