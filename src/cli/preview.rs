use std::env;
use std::fs;
use std::path::PathBuf;
use streamrs::config::current_profile::{
    DEFAULT_PROFILE, load_current_profile, normalize_profile_name,
};
use streamrs::paths::{default_config_path_for_profile, writable_icon_dir};

#[derive(Debug)]
pub(super) struct CliArgs {
    pub(super) profile: String,
    pub(super) config: PathBuf,
    pub(super) image_dir: PathBuf,
    pub(super) output: PathBuf,
}

pub(super) fn print_usage(program: &str) {
    eprintln!("Usage: {program} [--profile <name>] [--output <path>]");
}

fn first_readable_file(candidates: &[PathBuf]) -> Option<PathBuf> {
    candidates
        .iter()
        .find(|path| path.is_file() && fs::File::open(path).is_ok())
        .cloned()
}

fn first_readable_dir(candidates: &[PathBuf]) -> Option<PathBuf> {
    candidates
        .iter()
        .find(|path| path.is_dir() && fs::read_dir(path).is_ok())
        .cloned()
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
    let candidates = [
        home_default.clone(),
        PathBuf::from(format!("/usr/share/streamrs/{profile}/default.toml")),
        PathBuf::from("/usr/share/streamrs/default/default.toml"),
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("config")
            .join("default.toml"),
    ];
    first_readable_file(&candidates).unwrap_or(home_default)
}

fn default_image_dir() -> PathBuf {
    let home_default = writable_icon_dir();
    let candidates = [
        home_default.clone(),
        PathBuf::from("/usr/share/streamrs/icons"),
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("icons"),
    ];
    first_readable_dir(&candidates).unwrap_or(home_default)
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
        image_dir: default_image_dir(),
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
            image_dir: default_image_dir(),
            output: PathBuf::from("mock.png"),
        };

        apply_cli_overrides(
            &mut args,
            vec![
                "--profile".to_string(),
                "test_profile".to_string(),
                "--output".to_string(),
                "preview.png".to_string(),
            ]
            .into_iter(),
        )
        .expect("CLI overrides should parse");

        assert_eq!(args.profile, "test_profile");
        assert_eq!(args.config, default_config_path("test_profile"));
        assert_eq!(args.output, PathBuf::from("preview.png"));
    }

    #[test]
    fn apply_cli_overrides_rejects_invalid_profile_name() {
        let mut args = CliArgs {
            profile: DEFAULT_PROFILE.to_string(),
            config: default_config_path(DEFAULT_PROFILE),
            image_dir: default_image_dir(),
            output: PathBuf::from("mock.png"),
        };

        let err = apply_cli_overrides(
            &mut args,
            vec!["--profile".to_string(), "bad profile".to_string()].into_iter(),
        )
        .expect_err("invalid profile should be rejected");
        assert!(err.contains("Profile name"));
    }
}
