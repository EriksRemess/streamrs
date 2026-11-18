use crate::i18n::trf;
use serde::{Serialize, de::DeserializeOwned};
use std::fs;
use std::path::Path;

pub fn read_to_string(path: &Path) -> Result<String, String> {
    fs::read_to_string(path).map_err(|err| read_error(path, err))
}

pub(super) fn read_error(path: &Path, err: std::io::Error) -> String {
    trf(
        "Failed to read config '{path}': {err}",
        &[
            ("path", path.display().to_string()),
            ("err", err.to_string()),
        ],
    )
}

pub fn parse_from_str<T: DeserializeOwned>(path: &Path, raw: &str) -> Result<T, String> {
    toml::from_str(raw).map_err(|err| {
        trf(
            "Failed to parse config '{path}': {err}",
            &[
                ("path", path.display().to_string()),
                ("err", err.to_string()),
            ],
        )
    })
}

pub fn load_from_file<T: DeserializeOwned>(path: &Path) -> Result<T, String> {
    let raw = read_to_string(path)?;
    parse_from_str(path, &raw)
}

pub fn to_string_pretty<T: Serialize>(path: &Path, value: &T) -> Result<String, String> {
    toml::to_string_pretty(value)
        .map_err(|err| format!("Failed to serialize config '{}': {err}", path.display()))
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum WriteMode {
    Replace,
    Private,
    Create,
}

pub fn save_to_file_pretty<T: Serialize>(path: &Path, value: &T) -> Result<(), String> {
    save_pretty(path, value, WriteMode::Replace).map(drop)
}

pub(crate) fn save_private_pretty<T: Serialize>(path: &Path, value: &T) -> Result<(), String> {
    save_pretty(path, value, WriteMode::Private).map(drop)
}

pub(super) fn create_pretty_if_missing<T: Serialize>(
    path: &Path,
    value: &T,
) -> Result<bool, String> {
    save_pretty(path, value, WriteMode::Create)
}

fn save_pretty<T: Serialize>(path: &Path, value: &T, mode: WriteMode) -> Result<bool, String> {
    let output = to_string_pretty(path, value)?;
    write_config(path, output.as_bytes(), mode).map_err(|err| write_error(path, err))
}

pub(super) fn save_text(path: &Path, text: &str) -> Result<(), String> {
    write_config(path, text.as_bytes(), WriteMode::Replace)
        .map(drop)
        .map_err(|err| write_error(path, err))
}

fn write_error(path: &Path, err: std::io::Error) -> String {
    trf(
        "Failed to write config '{path}': {err}",
        &[
            ("path", path.display().to_string()),
            ("err", err.to_string()),
        ],
    )
}

// Atomic replacement changes the file inode. Lock its directory across read-modify-write operations.
pub(crate) fn lock_for_update(
    path: &Path,
    create_parent: bool,
    acquire: impl FnOnce(&fs::File) -> std::io::Result<()>,
) -> Result<fs::File, String> {
    let lock = || -> std::io::Result<_> {
        let target = resolve_target(path)?;
        let parent = target
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        if create_parent {
            fs::create_dir_all(parent)?;
        }
        let dir = fs::File::open(parent)?;
        acquire(&dir)?;
        Ok(dir)
    };
    lock().map_err(|err| {
        if err.kind() == std::io::ErrorKind::WouldBlock {
            trf(
                "Config '{path}' is busy. Try saving again.",
                &[("path", path.display().to_string())],
            )
        } else {
            write_error(path, err)
        }
    })
}

fn resolve_target(path: &Path) -> std::io::Result<std::path::PathBuf> {
    let mut target = path.to_path_buf();
    // Resolve the final link too, including a link whose target is being created.
    let mut links = 0;
    while target.is_symlink() {
        if links == 40 {
            return Err(std::io::Error::other("too many symbolic links"));
        }
        let link = fs::read_link(&target)?;
        target.pop();
        target.push(link);
        links += 1;
    }
    Ok(target)
}

fn write_config(path: &Path, data: &[u8], mode: WriteMode) -> std::io::Result<bool> {
    use std::io::{Error, ErrorKind, Write};
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let target = resolve_target(path)?;
    let parent = target
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let permissions = match fs::metadata(&target) {
        Ok(meta) => {
            if !meta.is_file() {
                return Err(Error::other("not a regular file"));
            }
            if mode == WriteMode::Create {
                return Ok(false);
            }
            fs::OpenOptions::new().write(true).open(&target)?;
            Some(meta.permissions())
        }
        Err(err) if err.kind() == ErrorKind::NotFound => None,
        Err(err) => return Err(err),
    };
    let mut temp = tempfile::NamedTempFile::new_in(parent)?;
    temp.write_all(data)?;
    if mode == WriteMode::Replace
        && let Some(permissions) = permissions
    {
        temp.as_file().set_permissions(permissions)?;
    }
    temp.as_file().sync_all()?;
    if mode == WriteMode::Create {
        if let Err(err) = temp.persist_noclobber(&target) {
            if err.error.kind() == ErrorKind::AlreadyExists && target.is_file() {
                return Ok(false);
            }
            return Err(err.error);
        }
    } else {
        temp.persist(&target)?;
    }
    Ok(true)
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::io::Read;
    use std::os::unix::fs::{PermissionsExt, symlink};
    #[test]
    fn saving_replaces_complete_config_and_preserves_symlink_and_mode() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("target.toml");
        let link = dir.path().join("linked.toml");
        fs::write(&target, "version = 1\n").unwrap();
        fs::set_permissions(&target, fs::Permissions::from_mode(0o640)).unwrap();
        symlink("target.toml", &link).unwrap();
        let mut reader = fs::File::open(&link).unwrap();
        let new = std::collections::BTreeMap::from([("version", 2)]);
        save_to_file_pretty(&link, &new).unwrap();
        let mut old = String::new();
        reader.read_to_string(&mut old).unwrap();
        assert_eq!(old, "version = 1\n");
        assert_eq!(
            load_from_file::<std::collections::BTreeMap<String, i32>>(&link).unwrap()["version"],
            2
        );
        assert!(link.is_symlink());
        assert_eq!(
            fs::metadata(target).unwrap().permissions().mode() & 0o777,
            0o640
        );
    }
}
