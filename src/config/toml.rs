use serde::{Serialize, de::DeserializeOwned};
use std::fs;
use std::path::Path;

pub fn read_to_string(path: &Path) -> Result<String, String> {
    fs::read_to_string(path)
        .map_err(|err| format!("Failed to read config '{}': {err}", path.display()))
}

pub fn parse_from_str<T: DeserializeOwned>(path: &Path, raw: &str) -> Result<T, String> {
    toml::from_str(raw).map_err(|err| format!("Failed to parse config '{}': {err}", path.display()))
}

pub fn load_from_file<T: DeserializeOwned>(path: &Path) -> Result<T, String> {
    let raw = read_to_string(path)?;
    parse_from_str(path, &raw)
}

pub fn to_string_pretty<T: Serialize>(path: &Path, value: &T) -> Result<String, String> {
    toml::to_string_pretty(value)
        .map_err(|err| format!("Failed to serialize config '{}': {err}", path.display()))
}

pub fn save_to_file_pretty<T: Serialize>(path: &Path, value: &T) -> Result<(), String> {
    save_pretty(path, value, false)
}

pub(crate) fn save_private_pretty<T: Serialize>(path: &Path, value: &T) -> Result<(), String> {
    save_pretty(path, value, true)
}

fn save_pretty<T: Serialize>(path: &Path, value: &T, private: bool) -> Result<(), String> {
    let output = to_string_pretty(path, value)?;
    write_config(path, output.as_bytes(), private)
        .map_err(|err| format!("Failed to write config '{}': {err}", path.display()))
}

fn write_config(path: &Path, data: &[u8], private: bool) -> std::io::Result<()> {
    use std::io::{Error, ErrorKind, Write};
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut target = path.to_path_buf();
    // Resolve the final link too, including a link whose target is being created.
    let mut links = 0;
    while target.is_symlink() {
        if links == 40 {
            return Err(Error::other("too many symbolic links"));
        }
        let link = fs::read_link(&target)?;
        target.pop();
        target.push(link);
        links += 1;
    }
    let parent = target
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let mut temp = tempfile::NamedTempFile::new_in(parent)?;
    let permissions = match fs::metadata(&target) {
        Ok(meta) => {
            if !meta.is_file() {
                return Err(Error::other("not a regular file"));
            }
            fs::OpenOptions::new().write(true).open(&target)?;
            Some(meta.permissions())
        }
        Err(err) if err.kind() == ErrorKind::NotFound => None,
        Err(err) => return Err(err),
    };
    temp.write_all(data)?;
    if !private && let Some(permissions) = permissions {
        temp.as_file().set_permissions(permissions)?;
    }
    temp.as_file().sync_all()?;
    temp.persist(&target)?;
    Ok(())
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
