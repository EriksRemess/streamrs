use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use streamrs::config::current_profile::{
    DEFAULT_PROFILE, load_current_profile, normalize_profile_name,
};
use streamrs::paths::{config_load_candidates, default_config_path_for_profile, writable_icon_dir};

#[derive(Debug)]
pub(super) struct CliArgs {
    pub(super) profile: String,
    pub(super) config: PathBuf,
    pub(super) page: usize,
    pub(super) image_dirs: [PathBuf; 3],
    pub(super) output: PathBuf,
}

pub(super) fn print_usage(program: &str) {
    eprintln!("Usage: {program} [--profile <name>] [--page <number>] [--output <path>]");
}

fn is_readable_file(path: &Path) -> bool {
    path.is_file() && fs::File::open(path).is_ok()
}

impl CliArgs {
    pub(super) fn image_dir_for(&self, name: &Path) -> &Path {
        self.image_dirs
            .iter()
            .find(|dir| is_readable_file(&dir.join(name)))
            .unwrap_or(&self.image_dirs[0])
    }
}

fn resolve_default_profile_name() -> String {
    match load_current_profile() {
        Ok(Some(profile)) => profile,
        Ok(None) => DEFAULT_PROFILE.to_string(),
        Err(err) => {
            eprintln!("{err}");
            DEFAULT_PROFILE.to_string()
        }
    }
}

fn default_config_path(profile: &str) -> PathBuf {
    let home_default = default_config_path_for_profile(profile);
    config_load_candidates(profile, &home_default)
        .into_iter()
        .find(|path| is_readable_file(path))
        .unwrap_or(home_default)
}

fn default_image_dirs() -> [PathBuf; 3] {
    [
        writable_icon_dir(),
        PathBuf::from("/usr/share/streamrs/icons"),
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("icons"),
    ]
}

fn apply_cli_overrides(
    args: &mut CliArgs,
    mut it: impl Iterator<Item = String>,
) -> Result<(), String> {
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--profile" => {
                let raw = it
                    .next()
                    .ok_or_else(|| "Missing value for --profile".to_string())?;
                let profile = normalize_profile_name(&raw).ok_or_else(|| {
                    "Profile name must contain only letters, numbers, '-' or '_'".to_string()
                })?;
                args.profile = profile.clone();
                args.config = default_config_path(&profile);
            }
            "--page" => {
                let raw = it
                    .next()
                    .ok_or_else(|| "Missing value for --page".to_string())?;
                args.page = raw.parse().map_err(|_| {
                    format!("Invalid page '{raw}': expected a page number starting at 1")
                })?;
            }
            "--output" => {
                args.output = PathBuf::from(
                    it.next()
                        .ok_or_else(|| "Missing value for --output".to_string())?,
                )
            }
            "--help" | "-h" => {
                print_usage(
                    &env::args()
                        .next()
                        .unwrap_or_else(|| "streamrs-preview".to_string()),
                );
                std::process::exit(0);
            }
            _ => return Err(format!("Unknown argument: {arg}")),
        }
    }
    Ok(())
}

pub(super) fn parse_args() -> Result<CliArgs, String> {
    let profile = resolve_default_profile_name();
    let mut args = CliArgs {
        profile: profile.clone(),
        config: default_config_path(&profile),
        page: 1,
        image_dirs: default_image_dirs(),
        output: PathBuf::from("mock.png"),
    };

    apply_cli_overrides(&mut args, env::args().skip(1))?;
    Ok(args)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn apply_cli_overrides_updates_profile_and_config_path() {
        let mut args = CliArgs {
            profile: DEFAULT_PROFILE.to_string(),
            config: default_config_path(DEFAULT_PROFILE),
            page: 1,
            image_dirs: default_image_dirs(),
            output: PathBuf::from("mock.png"),
        };

        apply_cli_overrides(
            &mut args,
            vec![
                "--profile".to_string(),
                "test_profile".to_string(),
                "--page".to_string(),
                "2".to_string(),
                "--output".to_string(),
                "preview.png".to_string(),
            ]
            .into_iter(),
        )
        .expect("CLI overrides should parse");

        assert_eq!(args.profile, "test_profile");
        assert_eq!(args.config, default_config_path("test_profile"));
        assert_eq!(args.page, 2);
        assert_eq!(args.output, PathBuf::from("preview.png"));
    }

    #[test]
    fn apply_cli_overrides_rejects_invalid_profile_name() {
        let mut args = CliArgs {
            profile: DEFAULT_PROFILE.to_string(),
            config: default_config_path(DEFAULT_PROFILE),
            page: 1,
            image_dirs: default_image_dirs(),
            output: PathBuf::from("mock.png"),
        };

        let err = apply_cli_overrides(
            &mut args,
            vec!["--profile".to_string(), "bad profile".to_string()].into_iter(),
        )
        .expect_err("invalid profile should be rejected");
        assert!(err.contains("Profile name"));
    }

    #[test]
    fn images_fall_back_individually_and_user_overrides_take_precedence() {
        let root = tempfile::tempdir().unwrap();
        let image_dirs = ["user", "installed", "source"].map(|name| root.path().join(name));
        let name = Path::new("arrow.png");
        let [user, installed, source] = &image_dirs;
        for dir in &image_dirs {
            fs::create_dir(dir).unwrap();
        }
        let args = CliArgs {
            profile: DEFAULT_PROFILE.into(),
            config: root.path().join("profile.toml"),
            page: 1,
            image_dirs: image_dirs.clone(),
            output: root.path().join("preview.png"),
        };
        for dir in [source, installed, user] {
            fs::write(dir.join(name), []).unwrap();
            assert_eq!(args.image_dir_for(name), dir);
        }
    }
}
