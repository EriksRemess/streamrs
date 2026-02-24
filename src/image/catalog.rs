use std::fs;
use std::path::{Path, PathBuf};

pub fn is_supported_icon_extension(path: &Path) -> bool {
    matches!(
        path.extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or_default()
            .to_ascii_lowercase()
            .as_str(),
        "png" | "jpg" | "jpeg" | "gif" | "webp" | "svg"
    )
}

pub fn copy_supported_image_into_dir(source_path: &Path, target_dir: &Path) -> Result<String, String> {
    if !source_path.is_file() {
        return Err(format!(
            "Selected path '{}' is not a file",
            source_path.display()
        ));
    }
    if !is_supported_icon_extension(source_path) {
        return Err(format!(
            "Unsupported icon type for '{}'",
            source_path.display()
        ));
    }

    let file_name = source_path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| "Icon file name is not valid UTF-8".to_string())?
        .to_string();
    fs::create_dir_all(target_dir).map_err(|err| {
        format!(
            "Failed to create icon directory '{}': {err}",
            target_dir.display()
        )
    })?;

    let destination = target_dir.join(&file_name);
    if destination == source_path {
        return Ok(file_name);
    }

    fs::copy(source_path, &destination).map_err(|err| {
        format!(
            "Failed to copy icon '{}' to '{}': {err}",
            source_path.display(),
            destination.display()
        )
    })?;
    Ok(file_name)
}

pub fn discover_icons(
    image_dirs: &[PathBuf],
    excluded_names: &[&str],
    preferred_first: &str,
) -> Vec<String> {
    let mut icons = Vec::new();

    for image_dir in image_dirs {
        if let Ok(entries) = fs::read_dir(image_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if !path.is_file() || !is_supported_icon_extension(&path) {
                    continue;
                }

                if let Some(name) = path.file_name().and_then(|name| name.to_str()) {
                    if excluded_names.contains(&name) {
                        continue;
                    }
                    icons.push(name.to_string());
                }
            }
        }
    }

    icons.sort_by_key(|name| name.to_ascii_lowercase());
    icons.dedup();
    promote_or_insert(&mut icons, preferred_first);
    icons
}

pub fn discover_png_backgrounds_with_prefix(
    image_dirs: &[PathBuf],
    prefix: &str,
    preferred_first: &str,
) -> Vec<String> {
    let mut backgrounds = Vec::new();

    for image_dir in image_dirs {
        if let Ok(entries) = fs::read_dir(image_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if !path.is_file() {
                    continue;
                }
                let extension = path
                    .extension()
                    .and_then(|ext| ext.to_str())
                    .unwrap_or_default()
                    .to_ascii_lowercase();
                if extension != "png" {
                    continue;
                }
                let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
                    continue;
                };
                if !name.starts_with(prefix) {
                    continue;
                }
                backgrounds.push(name.to_string());
            }
        }
    }

    backgrounds.sort_by_key(|name| name.to_ascii_lowercase());
    backgrounds.dedup();
    promote_or_insert(&mut backgrounds, preferred_first);
    backgrounds
}

fn promote_or_insert(items: &mut Vec<String>, preferred_first: &str) {
    if let Some(index) = items.iter().position(|name| name == preferred_first) {
        if index != 0 {
            let item = items.remove(index);
            items.insert(0, item);
        }
    } else {
        items.insert(0, preferred_first.to_string());
    }
}
