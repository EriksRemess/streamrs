use std::path::Path;
pub(super) use streamrs::config::streamrs_schema::StreamrsConfig as Config;

pub(super) fn load_config(path: &Path) -> Result<Config, String> {
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
}
