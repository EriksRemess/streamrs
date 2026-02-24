use super::*;

pub(crate) fn rounded_icons_dir() -> PathBuf {
    env::temp_dir().join("streamrs-gui-rounded-icons")
}

pub(crate) fn cached_path_if_valid(cache_key: &str) -> Option<PathBuf> {
    cached_png_path_if_valid(&rounded_icons_dir(), cache_key)
}

pub(crate) fn write_rounded_png(cache_key: &str, mut image: RgbaImage) -> Option<PathBuf> {
    apply_rounded_corners(&mut image, 0.17);
    write_cached_png(&rounded_icons_dir(), cache_key, &image)
}

pub(crate) fn render_clock_icon_png(image_dirs: &[PathBuf], background_name: Option<&str>) -> Option<PathBuf> {
    let clock_text = current_clock_text();
    let background = background_name.unwrap_or(CLOCK_BACKGROUND_ICON);
    let cache_key = format!("clock-{}-{}", clock_text, background);
    if let Some(path) = cached_path_if_valid(&cache_key) {
        return Some(path);
    }

    let background_dir = find_icon_file(image_dirs, background)
        .and_then(|path| path.parent().map(PathBuf::from));
    let resources_dir = background_dir.as_deref();
    let svg = render_clock_segments_svg(
        resources_dir.unwrap_or_else(|| Path::new(".")),
        &clock_text,
        Some(background),
    );
    let image = load_svg_image_data(CLOCK_ICON_ALIAS, svg.as_bytes(), resources_dir, 256, 256).ok()?;
    write_rounded_png(&cache_key, image)
}

pub(crate) fn render_regular_icon_png(image_dirs: &[PathBuf], icon_name: &str) -> Option<PathBuf> {
    let cache_key = format!("icon-{}", icon_name);
    if let Some(path) = cached_path_if_valid(&cache_key) {
        return Some(path);
    }

    let requested = find_icon_file(image_dirs, icon_name)?;
    let extension = requested
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();

    let image = if extension == "svg" {
        let data = fs::read(&requested).ok()?;
        load_svg_image_data(
            &requested.display().to_string(),
            &data,
            requested.parent(),
            256,
            256,
        )
        .ok()?
    } else {
        image::open(&requested).ok()?.to_rgba8()
    };

    write_rounded_png(&cache_key, image)
}
