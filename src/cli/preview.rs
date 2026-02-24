use std::env;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub(super) struct CliArgs {
    pub(super) config: PathBuf,
    pub(super) image_dir: PathBuf,
    pub(super) output: PathBuf,
}

pub(super) fn print_usage(program: &str) {
    eprintln!("Usage: {program} [--output <path>]");
}

fn home_dir() -> Result<PathBuf, String> {
    env::var_os("HOME")
        .map(PathBuf::from)
        .ok_or_else(|| "HOME is not set".to_string())
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

fn default_config_path(home: &Path) -> PathBuf {
    let home_default = home.join(".config/streamrs/default.toml");
    let candidates = [
        home_default.clone(),
        PathBuf::from("/usr/share/streamrs/default/default.toml"),
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("config")
            .join("default.toml"),
    ];
    first_readable_file(&candidates).unwrap_or(home_default)
}

fn default_image_dir(home: &Path) -> PathBuf {
    let home_default = home.join(".local/share/streamrs/default");
    let candidates = [
        home_default.clone(),
        PathBuf::from("/usr/share/streamrs/default"),
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("all_images"),
    ];
    first_readable_dir(&candidates).unwrap_or(home_default)
}

pub(super) fn parse_args() -> Result<CliArgs, String> {
    let home = home_dir()?;
    let mut args = CliArgs {
        config: default_config_path(&home),
        image_dir: default_image_dir(&home),
        output: PathBuf::from("mock.png"),
    };

    let mut it = env::args().skip(1);
    while let Some(arg) = it.next() {
        match arg.as_str() {
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
    Ok(args)
}
