use image::RgbaImage;
use std::collections::hash_map::DefaultHasher;
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};

pub fn cache_hash_key(key: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    key.hash(&mut hasher);
    hasher.finish()
}

pub fn cache_png_path(cache_dir: &Path, cache_key: &str) -> PathBuf {
    cache_dir.join(format!("{:016x}.png", cache_hash_key(cache_key)))
}

pub fn cached_png_path_if_valid(cache_dir: &Path, cache_key: &str) -> Option<PathBuf> {
    let path = cache_png_path(cache_dir, cache_key);
    if path.is_file() && image::open(&path).is_ok() {
        Some(path)
    } else {
        None
    }
}

pub fn write_cached_png(cache_dir: &Path, cache_key: &str, image: &RgbaImage) -> Option<PathBuf> {
    let path = cache_png_path(cache_dir, cache_key);
    if path.is_file() && image::open(&path).is_ok() {
        return Some(path);
    }

    let parent = path.parent()?;
    fs::create_dir_all(parent).ok()?;
    image.save(&path).ok()?;
    Some(path)
}
