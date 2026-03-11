use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use streamrs::config::current_profile::{
    BLANK_PROFILE, DEFAULT_PROFILE, discover_profiles, load_current_profile, save_current_profile,
};
use streamrs::config::streamrs_schema::blank_profile_config;

#[derive(Debug)]
pub(crate) struct CliArgs {
    pub(crate) debug: bool,
    pub(crate) profile: String,
    pub(crate) profile_explicit: bool,
    pub(crate) config_path: Option<PathBuf>,
    pub(crate) init: bool,
    pub(crate) force: bool,
    pub(crate) force_images: bool,
}

pub(crate) fn print_usage(program: &str) {
    println!(
        "Usage: {program} [--debug] [--profile <name>] [--config <path>] [--init] [--force] [--force-images]"
    );
}

pub(crate) fn parse_args() -> Result<CliArgs, String> {
    let mut debug = false;
    let mut profile = resolve_default_profile();
    let mut profile_explicit = false;
    let mut config_path = None;
    let mut init = false;
    let mut force = false;
    let mut force_images = false;

    let mut args = env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--debug" => debug = true,
            "--profile" => {
                profile = args
                    .next()
                    .ok_or_else(|| "Missing value for --profile".to_string())?;
                profile_explicit = true;
            }
            "--config" => {
                let value = args
                    .next()
                    .ok_or_else(|| "Missing value for --config".to_string())?;
                config_path = Some(PathBuf::from(value));
            }
            "--init" => init = true,
            "--force" => force = true,
            "--force-images" => force_images = true,
            "--help" | "-h" => {
                let program = env::args().next().unwrap_or_else(|| "streamrs".to_string());
                print_usage(&program);
                std::process::exit(0);
            }
            _ => return Err(format!("Unknown argument: {arg}")),
        }
    }

    if force && !init {
        return Err("--force requires --init".to_string());
    }
    if force_images && !init {
        return Err("--force-images requires --init".to_string());
    }

    Ok(CliArgs {
        debug,
        profile,
        profile_explicit,
        config_path,
        init,
        force,
        force_images,
    })
}

fn resolve_default_profile() -> String {
    let profiles = discover_profiles();

    if let Ok(Some(profile)) = load_current_profile() {
        if profile == BLANK_PROFILE {
            if profiles.is_empty() {
                return profile;
            }
        } else {
            return profile;
        }
    }

    if let Some(profile) = profiles
        .iter()
        .find(|profile| profile.as_str() == DEFAULT_PROFILE)
    {
        return profile.clone();
    }
    if let Some(profile) = profiles.first() {
        return profile.clone();
    }
    BLANK_PROFILE.to_string()
}

pub(crate) fn default_config_path(profile: &str) -> Result<PathBuf, String> {
    Ok(streamrs::paths::default_config_path_for_profile(profile))
}

pub(crate) fn default_image_dir(profile: &str) -> Result<PathBuf, String> {
    Ok(streamrs::paths::writable_image_dir_for_profile(profile))
}

fn find_default_config_source(profile: &str) -> Option<PathBuf> {
    let candidates = [
        PathBuf::from(format!("/usr/share/streamrs/{profile}/default.toml")),
        PathBuf::from("/usr/share/streamrs/default/default.toml"),
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("config")
            .join("default.toml"),
    ];
    candidates.into_iter().find(|path| path.is_file())
}

fn find_image_source_dir(profile: &str) -> Option<PathBuf> {
    let candidates = [
        PathBuf::from("/usr/share/streamrs/icons"),
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("icons"),
    ];
    let _ = profile;
    candidates.into_iter().find(|path| path.is_dir())
}

fn copy_file(src: &Path, dst: &Path, force: bool) -> Result<bool, String> {
    if dst.exists() && !force {
        return Ok(false);
    }
    if let Some(parent) = dst.parent() {
        fs::create_dir_all(parent)
            .map_err(|err| format!("Failed to create directory '{}': {err}", parent.display()))?;
    }
    fs::copy(src, dst).map_err(|err| {
        format!(
            "Failed to copy '{}' to '{}': {err}",
            src.display(),
            dst.display()
        )
    })?;
    Ok(true)
}

fn copy_dir_contents(src: &Path, dst: &Path, force: bool) -> Result<(usize, usize), String> {
    fs::create_dir_all(dst)
        .map_err(|err| format!("Failed to create directory '{}': {err}", dst.display()))?;

    let mut copied = 0usize;
    let mut skipped = 0usize;
    for entry in fs::read_dir(src)
        .map_err(|err| format!("Failed to read directory '{}': {err}", src.display()))?
    {
        let entry =
            entry.map_err(|err| format!("Failed to read entry in '{}': {err}", src.display()))?;
        let src_path = entry.path();
        let name = entry.file_name();
        if name == "default.toml" {
            continue;
        }
        let dst_path = dst.join(&name);
        let file_type = entry.file_type().map_err(|err| {
            format!(
                "Failed to read file type for '{}': {err}",
                src_path.display()
            )
        })?;

        if file_type.is_dir() {
            let (sub_copied, sub_skipped) = copy_dir_contents(&src_path, &dst_path, force)?;
            copied += sub_copied;
            skipped += sub_skipped;
        } else if file_type.is_file() {
            if copy_file(&src_path, &dst_path, force)? {
                copied += 1;
            } else {
                skipped += 1;
            }
        }
    }

    Ok((copied, skipped))
}

fn ensure_profile_images_initialized(profile: &str, image_dir: &Path) -> Result<(), String> {
    if profile == BLANK_PROFILE {
        fs::create_dir_all(image_dir).map_err(|err| {
            format!(
                "Failed to create blank profile image directory '{}': {err}",
                image_dir.display()
            )
        })?;
        return Ok(());
    }

    let images_src = find_image_source_dir(profile).ok_or_else(|| {
        "Could not find an image source directory. Expected /usr/share/streamrs/default or repository icons.".to_string()
    })?;
    let _ = copy_dir_contents(&images_src, image_dir, false)?;
    Ok(())
}

pub(crate) fn initialize_profile(
    profile: &str,
    config_path: &Path,
    image_dir: &Path,
    force_config: bool,
    force_images: bool,
) -> Result<(), String> {
    let config_src = find_default_config_source(profile).ok_or_else(|| {
        "Could not find a default config source. Expected /usr/share/streamrs/default/default.toml or repository config.".to_string()
    })?;
    let images_src = find_image_source_dir(profile).ok_or_else(|| {
        "Could not find an image source directory. Expected /usr/share/streamrs/default or repository icons.".to_string()
    })?;

    let config_copied = copy_file(&config_src, config_path, force_config)?;
    let (images_copied, images_skipped) = copy_dir_contents(&images_src, image_dir, force_images)?;

    if config_copied {
        eprintln!(
            "Initialized config '{}' from '{}'",
            config_path.display(),
            config_src.display()
        );
    } else {
        eprintln!(
            "Config '{}' already exists; keeping existing file (use --force to overwrite)",
            config_path.display()
        );
    }

    eprintln!(
        "Initialized images in '{}': {} copied, {} skipped",
        image_dir.display(),
        images_copied,
        images_skipped
    );

    let _ = save_current_profile(profile);

    Ok(())
}

pub(crate) fn ensure_profile_initialized(
    profile: &str,
    config_path: &Path,
    image_dir: &Path,
) -> Result<(), String> {
    if profile == BLANK_PROFILE {
        if let Some(parent) = config_path.parent() {
            fs::create_dir_all(parent).map_err(|err| {
                format!(
                    "Failed to create blank profile config directory '{}': {err}",
                    parent.display()
                )
            })?;
        }
        ensure_profile_images_initialized(profile, image_dir)?;
        streamrs::config::streamrs_profile::save(config_path, &blank_profile_config())?;
        let _ = save_current_profile(profile);
        eprintln!(
            "Initialized blank profile config '{}'",
            config_path.display()
        );
        return Ok(());
    }

    if config_path.exists() {
        ensure_profile_images_initialized(profile, image_dir)?;
        let _ = save_current_profile(profile);
        return Ok(());
    }

    eprintln!(
        "Config '{}' not found; initializing profile assets",
        config_path.display()
    );
    initialize_profile(profile, config_path, image_dir, false, false)?;
    let _ = save_current_profile(profile);
    Ok(())
}

pub(crate) fn print_post_init_service_hint() {
    eprintln!("Initialization complete.");
    eprintln!("To enable and start streamrs as a user service:");
    eprintln!("  systemctl --user daemon-reload");
    eprintln!("  systemctl --user enable --now streamrs.service");
    eprintln!("If streamrs is already running, restart it:");
    eprintln!("  systemctl --user restart streamrs.service");
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Mutex, OnceLock};
    use std::sync::atomic::{AtomicUsize, Ordering};

    static TEST_COUNTER: AtomicUsize = AtomicUsize::new(0);

    fn env_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    fn with_temp_xdg_config_home(name: &str, run: impl FnOnce(&Path)) {
        let _guard = env_lock().lock().expect("env lock should be available");
        let id = TEST_COUNTER.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("streamrs-init-xdg-tests-{name}-{id}"));
        fs::create_dir_all(&dir).expect("test XDG config dir should be creatable");

        let previous = std::env::var_os("XDG_CONFIG_HOME");
        // SAFETY: Test helper serializes env mutation via a process-wide mutex.
        unsafe {
            std::env::set_var("XDG_CONFIG_HOME", &dir);
        }

        run(&dir);

        if let Some(value) = previous {
            // SAFETY: Test helper serializes env mutation via a process-wide mutex.
            unsafe {
                std::env::set_var("XDG_CONFIG_HOME", value);
            }
        } else {
            // SAFETY: Test helper serializes env mutation via a process-wide mutex.
            unsafe {
                std::env::remove_var("XDG_CONFIG_HOME");
            }
        }
    }

    fn test_temp_dir(name: &str) -> PathBuf {
        let id = TEST_COUNTER.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("streamrs-init-tests-{name}-{id}"));
        fs::create_dir_all(&dir).expect("test directory should be creatable");
        dir
    }

    #[test]
    fn ensure_profile_initialized_keeps_existing_config_and_populates_images() {
        let dir = test_temp_dir("existing-config-images");
        let config_path = dir.join("default.toml");
        fs::write(
            &config_path,
            "brightness = 42\nkeys_per_page = 15\n[[keys]]\nicon = \"blank.png\"\n",
        )
        .expect("test config should be written");
        let image_dir = dir.join("images");

        ensure_profile_initialized("default", &config_path, &image_dir)
            .expect("existing config should still ensure images");

        let raw = fs::read_to_string(&config_path).expect("config should remain readable");
        assert!(
            raw.contains("brightness = 42"),
            "existing config should not be overwritten"
        );
        assert!(
            image_dir.join("blank.png").is_file(),
            "default image assets should be present for icon loading"
        );
    }

    #[test]
    fn ensure_blank_profile_initializes_blank_config_and_image_dir() {
        let dir = test_temp_dir("blank-profile");
        let config_path = dir.join("blank.toml");
        let image_dir = dir.join("blank-images");

        ensure_profile_initialized(BLANK_PROFILE, &config_path, &image_dir)
            .expect("blank profile should initialize");

        assert!(
            config_path.is_file(),
            "blank profile config should be created"
        );
        assert!(
            image_dir.is_dir(),
            "blank profile image directory should be created"
        );

        let loaded = streamrs::config::streamrs_profile::load_with_fallbacks(&config_path)
            .expect("blank config should be loadable")
            .expect("blank config file should exist");
        assert!(
            loaded.keys.is_empty(),
            "blank profile must remain empty (no configured buttons)"
        );
    }

    #[test]
    fn default_image_dir_is_shared_for_all_profiles() {
        let default_dir = default_image_dir("default").expect("default image dir should resolve");
        let test_dir = default_image_dir("test").expect("test image dir should resolve");
        assert_eq!(default_dir, test_dir);
    }

    #[test]
    fn resolve_default_profile_keeps_saved_non_blank_profile_even_if_not_discovered() {
        with_temp_xdg_config_home("resolve-current", |xdg_config_home| {
            let streamrs_dir = xdg_config_home.join("streamrs");
            fs::create_dir_all(&streamrs_dir).expect("streamrs config dir should be creatable");
            fs::write(streamrs_dir.join("current_profile"), "test\n")
                .expect("current profile fixture should be written");
            fs::write(streamrs_dir.join("default.toml"), "brightness = 60\n")
                .expect("default config fixture should be written");

            let resolved = resolve_default_profile();
            assert_eq!(
                resolved, "test",
                "saved non-blank current profile should not fall back to default"
            );
        });
    }
}
