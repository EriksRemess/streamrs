use super::*;

pub(crate) fn load_config(path: &Path) -> Result<Config, String> {
    let Some(mut config) = streamrs_profile::load_with_fallbacks(path)? else {
        return Ok(Config::default());
    };
    normalize_config(&mut config);
    Ok(config)
}

pub(crate) fn save_config(path: &Path, config: &Config) -> Result<(), String> {
    streamrs_profile::save(path, config)
}
