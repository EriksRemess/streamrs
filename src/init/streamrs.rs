use std::env;
use std::ffi::{OsStr, OsString};
use std::fs;
use std::path::{Path, PathBuf};
use streamrs::config::current_profile::{
    BLANK_PROFILE, DEFAULT_PROFILE, discover_profiles, load_current_profile,
    save_current_profile_if_missing,
};
use streamrs::config::streamrs_schema::blank_profile_config;
use streamrs::i18n::{tr, trf};
use streamrs::streamdeck::{DisplayFill, info::SleepTimer};

#[derive(Debug)]
pub(crate) struct CliArgs {
    pub(crate) info: bool,
    pub(crate) fill: Option<DisplayFill>,
    pub(crate) compare_firmware: Option<PathBuf>,
    pub(crate) brightness: Option<u8>,
    pub(crate) standby_image: Option<PathBuf>,
    pub(crate) sleep_after: Option<SleepTimer>,
    pub(crate) debug: bool,
    pub(crate) profile: String,
    pub(crate) profile_explicit: bool,
    pub(crate) config_path: Option<PathBuf>,
    pub(crate) init: bool,
    pub(crate) force: bool,
    pub(crate) force_images: bool,
}

pub(crate) fn print_usage() {
    let program = env::args_os().next().unwrap_or_else(|| "streamrs".into());
    println!(
        "{}",
        trf(
            "Usage: {program} [--info | --fill-lcd <RRGGBB> | --fill-key <1-15> <RRGGBB> | --compare-firmware <file.bin> | --brightness <0-100> | --sleep-after <minutes|never> | --standby-image <path> | --init [--force] [--force-images]] [--debug] [--profile <name>] [--config <path>]",
            &[("program", program.to_string_lossy().into_owned())],
        )
    );
}

pub(crate) fn parse_args() -> Result<CliArgs, String> {
    parse_args_from(env::args_os().skip(1), resolve_default_profile)
}

fn parse_rgb(value: &OsStr) -> Result<[u8; 3], String> {
    let text = value.to_string_lossy();
    let hex = text.strip_prefix('#').unwrap_or(&text);
    if hex.len() != 6 || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(tr("Color must contain six hexadecimal digits (RRGGBB)"));
    }
    let [_, r, g, b] = u32::from_str_radix(hex, 16).unwrap().to_be_bytes();
    Ok([r, g, b])
}

fn parse_args_from(
    mut args: impl Iterator<Item = OsString>,
    default_profile: impl FnOnce() -> String,
) -> Result<CliArgs, String> {
    let mut info = false;
    let mut fill = None;
    let mut compare_firmware = None;
    let mut brightness = None;
    let mut sleep_after = None;
    let mut standby_image = None;
    let mut debug = false;
    let mut profile = None;
    let mut config_path = None;
    let mut init = false;
    let mut force = false;
    let mut force_images = false;

    while let Some(arg) = args.next() {
        let option = arg.to_string_lossy();
        let image_path = matches!(option.as_ref(), "--compare-firmware" | "--standby-image");
        let mut value = || {
            args.next()
                .filter(|v| {
                    !v.is_empty() && !(image_path && v.as_encoded_bytes().starts_with(b"--"))
                })
                .ok_or_else(|| {
                    trf(
                        "Missing value for {option}",
                        &[("option", option.to_string())],
                    )
                })
        };
        match option.as_ref() {
            "--debug" => debug = true,
            "--info" => info = true,
            "--fill-lcd" | "--fill-key" => {
                if fill.is_some() {
                    return Err(tr("Choose only one display fill per command"));
                }
                fill = Some(if option == "--fill-key" {
                    let key = value()?
                        .to_string_lossy()
                        .parse()
                        .map_err(|_| tr("Key number must be between 1 and 15."))?;
                    DisplayFill::key(key, parse_rgb(&value()?)?)?
                } else {
                    DisplayFill::lcd(parse_rgb(&value()?)?)
                });
            }
            "--compare-firmware" => {
                if compare_firmware.is_some() {
                    return Err(tr("Choose only one firmware image per command"));
                }
                compare_firmware = Some(PathBuf::from(value()?));
            }
            "--brightness" => {
                brightness = Some(
                    value()?
                        .to_string_lossy()
                        .parse::<u8>()
                        .ok()
                        .filter(|v| *v <= 100)
                        .ok_or_else(|| {
                            tr("--brightness requires a whole percentage from 0 to 100")
                        })?,
                );
            }
            "--standby-image" => standby_image = Some(PathBuf::from(value()?)),
            "--sleep-after" => {
                let duration = value()?;
                let minutes = if duration == "never" {
                    Some(0)
                } else {
                    duration.to_string_lossy().parse().ok()
                };
                sleep_after =
                    Some(minutes.and_then(SleepTimer::from_minutes).ok_or_else(|| {
                        tr("--sleep-after requires whole minutes from 0 to 35791394, or never")
                    })?);
            }
            "--profile" => {
                profile = Some(
                    streamrs::config::current_profile::normalize_profile_name(
                        &value()?.to_string_lossy(),
                    )
                    .ok_or_else(|| {
                        tr("Profile name must contain only letters, numbers, '-' or '_'")
                    })?,
                );
            }
            "--config" => config_path = Some(PathBuf::from(value()?)),
            "--init" => init = true,
            "--force" => force = true,
            "--force-images" => force_images = true,
            "--help" | "-h" => {
                print_usage();
                std::process::exit(0);
            }
            _ => {
                return Err(trf(
                    "Unknown argument: {arg}",
                    &[("arg", option.into_owned())],
                ));
            }
        }
    }

    let commands = [
        info,
        init,
        brightness.is_some(),
        sleep_after.is_some(),
        standby_image.is_some(),
        fill.is_some(),
        compare_firmware.is_some(),
    ];
    if commands.into_iter().filter(|enabled| *enabled).count() > 1 {
        return Err(tr(
            "Choose only one of --info, --init, --brightness, --sleep-after, --standby-image, --fill-lcd, --fill-key or --compare-firmware",
        ));
    }
    if force && !init {
        return Err(tr("--force requires --init"));
    }
    if force_images && !init {
        return Err(tr("--force-images requires --init"));
    }

    let profile_explicit = profile.is_some();
    let profile = profile.unwrap_or_else(|| {
        if config_path.is_some() {
            DEFAULT_PROFILE.to_owned()
        } else {
            default_profile()
        }
    });
    Ok(CliArgs {
        info,
        fill,
        compare_firmware,
        brightness,
        sleep_after,
        standby_image,
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
    resolve_default_profile_from(&profiles, load_current_profile())
}

fn save_current_profile_with_log(profile: &str, context: &str) {
    if let Err(err) = save_current_profile_if_missing(profile) {
        eprintln!("{err}");
        eprintln!("Failed to persist current profile while {context}");
    }
}

fn resolve_default_profile_from(
    profiles: &[String],
    current_profile: Result<Option<String>, String>,
) -> String {
    match current_profile {
        Ok(Some(profile)) => {
            if profile == BLANK_PROFILE {
                if profiles.is_empty() {
                    return profile;
                }
            } else {
                return profile;
            }
        }
        Ok(None) => {}
        Err(err) => {
            eprintln!("{err}");
            return BLANK_PROFILE.to_string();
        }
    }

    if let Some(profile) = profiles
        .iter()
        .find(|profile| profile.as_str() != DEFAULT_PROFILE)
    {
        return profile.clone();
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
    streamrs::paths::copy_file_atomically(src, dst, force).map_err(|err| {
        format!(
            "Failed to copy '{}' to '{}': {err}",
            src.display(),
            dst.display()
        )
    })
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

    save_current_profile_with_log(profile, "initializing profile");

    Ok(())
}

pub(crate) fn ensure_profile_initialized(
    profile: &str,
    config_path: &Path,
    image_dir: &Path,
) -> Result<(), String> {
    if config_path.exists() {
        ensure_profile_images_initialized(profile, image_dir)?;
        save_current_profile_with_log(profile, "confirming existing profile");
        return Ok(());
    }

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
        if streamrs::config::streamrs_profile::create_if_missing(
            config_path,
            &blank_profile_config(),
        )? {
            eprintln!(
                "Initialized blank profile config '{}'",
                config_path.display()
            );
        }
        save_current_profile_with_log(profile, "initializing blank profile");
        return Ok(());
    }

    eprintln!(
        "Config '{}' not found; initializing profile assets",
        config_path.display()
    );
    initialize_profile(profile, config_path, image_dir, false, false)?;
    save_current_profile_with_log(profile, "initializing missing profile config");
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
    use std::sync::atomic::{AtomicUsize, Ordering};

    static TEST_COUNTER: AtomicUsize = AtomicUsize::new(0);

    fn test_temp_dir(name: &str) -> PathBuf {
        let id = TEST_COUNTER.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("streamrs-init-tests-{name}-{id}"));
        fs::create_dir_all(&dir).expect("test directory should be creatable");
        dir
    }

    #[test]
    fn hardware_commands_validate_targets_colors_and_conflicts() {
        let parse = |args: &[&str]| parse_args_from(args.iter().map(OsString::from), String::new);
        assert_eq!(
            parse(&["--fill-lcd", "#fF0080"]).unwrap().fill,
            Some(DisplayFill::lcd([255, 0, 128]))
        );
        assert_eq!(
            parse(&["--fill-key", "15", "112233"]).unwrap().fill,
            Some(DisplayFill::key(15, [17, 34, 51]).unwrap())
        );
        assert!(
            parse(&["--compare-firmware", "fw.bin", "--config", "desk.toml"])
                .unwrap()
                .compare_firmware
                .is_some()
        );
        for args in [
            &["--fill-key", "0", "112233"][..],
            &["--fill-key", "16", "112233"],
            &["--fill-key", "2"],
            &["--fill-lcd", "+01234"],
            &["--fill-lcd", "aabb"],
            &["--fill-lcd", "112233", "--fill-key", "1", "aabbcc"],
            &["--compare-firmware", "--info"],
            &[
                "--compare-firmware",
                "one.bin",
                "--compare-firmware",
                "two.bin",
            ],
            &["--compare-firmware", "fw.bin", "--init"],
            &["--fill-lcd", "ff00ff", "--info"],
        ] {
            assert!(parse(args).is_err(), "{args:?}");
        }
    }

    #[cfg(unix)]
    #[test]
    fn device_commands_preserve_native_paths() {
        use std::os::unix::ffi::OsStringExt;
        let path = OsString::from_vec(b"image-\xff.bin".to_vec());
        let config = OsString::from_vec(b"profile-\xfe.toml".to_vec());
        let args = parse_args_from(
            [
                "--compare-firmware".into(),
                path.clone(),
                "--config".into(),
                config.clone(),
            ]
            .into_iter(),
            String::new,
        )
        .unwrap();
        assert_eq!(args.compare_firmware.as_deref(), Some(Path::new(&path)));
        assert_eq!(args.config_path.as_deref(), Some(Path::new(&config)));
    }

    #[test]
    fn brightness_requires_a_percentage_and_an_exclusive_device_command() {
        let parse = |args: &[&str]| parse_args_from(args.iter().map(OsString::from), String::new);
        for value in ["0", "35", "100"] {
            assert_eq!(
                parse(&["--profile", "desk", "--brightness", value])
                    .unwrap()
                    .brightness,
                value.parse().ok()
            );
        }
        for args in [
            &["--brightness"][..],
            &["--brightness", "-1"],
            &["--brightness", "101"],
            &["--brightness", "1.5"],
            &["--brightness", "0", "--init"],
            &["--info", "--brightness", "10"],
            &["--brightness", "10", "--sleep-after", "never"],
        ] {
            assert!(parse(args).is_err(), "{args:?}");
        }
    }

    #[test]
    fn sleep_after_accepts_whole_minutes_and_never_as_a_device_command() {
        let parse = |args: &[&str]| parse_args_from(args.iter().map(OsString::from), String::new);
        for (arg, minutes) in [("never", 0), ("0", 0), ("10", 10), ("35791394", 35791394)] {
            let args = parse(&["--sleep-after", arg, "--profile", "test"]).unwrap();
            assert_eq!(args.sleep_after.unwrap().whole_minutes(), Some(minutes));
            assert_eq!(args.profile, "test");
        }
        for args in [
            &["--sleep-after"][..],
            &["--sleep-after", "-1"],
            &["--sleep-after", "1.5"],
            &["--sleep-after", "35791395"],
            &["--info", "--sleep-after", "never"],
            &["--sleep-after", "1", "--init"],
        ] {
            assert!(parse(args).is_err(), "{args:?}");
        }
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
        let profiles = vec![DEFAULT_PROFILE.to_string()];
        let resolved = resolve_default_profile_from(&profiles, Ok(Some("test".to_string())));
        assert_eq!(
            resolved, "test",
            "saved non-blank current profile should not fall back to default"
        );
    }

    #[test]
    fn resolve_default_profile_uses_blank_when_current_profile_is_invalid() {
        let profiles = vec![DEFAULT_PROFILE.to_string()];
        let resolved = resolve_default_profile_from(
            &profiles,
            Err("current profile should fail to load".to_string()),
        );
        assert_eq!(
            resolved, BLANK_PROFILE,
            "invalid current profile should force blank fallback"
        );
    }

    #[test]
    fn resolve_default_profile_prefers_non_default_when_current_is_missing() {
        let profiles = vec!["default".to_string(), "main-profile".to_string()];
        let resolved = resolve_default_profile_from(&profiles, Ok(None));
        assert_eq!(
            resolved, "main-profile",
            "missing current profile should prefer an existing non-default profile"
        );
    }
    #[test]
    fn explicit_config_works_even_with_no_discovered_profiles() {
        let args = parse_args_from(
            ["--config", "custom.toml"].map(OsString::from).into_iter(),
            || BLANK_PROFILE.into(),
        )
        .unwrap();
        assert_ne!(args.profile, BLANK_PROFILE);
    }

    #[test]
    fn invalid_profile_name_is_rejected_before_initialization() {
        assert!(
            parse_args_from(
                ["--profile", "a/b", "--init"]
                    .map(OsString::from)
                    .into_iter(),
                || DEFAULT_PROFILE.into()
            )
            .is_err()
        );
    }

    #[test]
    fn forced_init_copy_can_share_its_source_inode() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("input");
        let dst = dir.path().join("output");
        fs::write(&src, "profile").unwrap();
        fs::hard_link(&src, &dst).unwrap();
        assert!(copy_file(&src, &dst, true).unwrap());
        assert_eq!(fs::read(&src).unwrap(), b"profile");
        assert_eq!(fs::read(&dst).unwrap(), b"profile");
    }
}
