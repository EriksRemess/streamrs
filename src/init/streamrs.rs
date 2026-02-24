use std::env;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub(crate) struct CliArgs {
    pub(crate) debug: bool,
    pub(crate) profile: String,
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
    let mut profile = "default".to_string();
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
        config_path,
        init,
        force,
        force_images,
    })
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
        PathBuf::from(format!("/usr/share/streamrs/{profile}")),
        PathBuf::from("/usr/share/streamrs/default"),
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("all_images"),
    ];
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
        "Could not find an image source directory. Expected /usr/share/streamrs/default or repository all_images.".to_string()
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

    Ok(())
}

pub(crate) fn ensure_profile_initialized(
    profile: &str,
    config_path: &Path,
    image_dir: &Path,
) -> Result<(), String> {
    if config_path.exists() {
        return Ok(());
    }

    eprintln!(
        "Config '{}' not found; initializing profile assets",
        config_path.display()
    );
    initialize_profile(profile, config_path, image_dir, false, false)
}

pub(crate) fn print_post_init_service_hint() {
    eprintln!("Initialization complete.");
    eprintln!("To enable and start streamrs as a user service:");
    eprintln!("  systemctl --user daemon-reload");
    eprintln!("  systemctl --user enable --now streamrs.service");
    eprintln!("If streamrs is already running, restart it:");
    eprintln!("  systemctl --user restart streamrs.service");
}
