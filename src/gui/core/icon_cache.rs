use super::*;

pub(crate) fn rounded_icons_dir() -> PathBuf {
    env::temp_dir().join("streamrs-gui-rounded-icons")
}

const CLOCK_MISSING_BACKGROUND_SENTINEL: &str = "__streamrs-missing-clock-bg__.png";

pub(crate) fn cached_path_if_valid(cache_key: &str) -> Option<PathBuf> {
    cached_png_path_if_valid(&rounded_icons_dir(), cache_key)
}

pub(crate) fn write_rounded_png(cache_key: &str, mut image: RgbaImage) -> Option<PathBuf> {
    apply_rounded_corners(&mut image, 0.17);
    write_cached_png(&rounded_icons_dir(), cache_key, &image)
}

pub(crate) fn render_clock_icon_png(
    image_dirs: &[PathBuf],
    background_name: Option<&str>,
) -> Option<PathBuf> {
    let clock_text = current_clock_text();
    let background = background_name.unwrap_or(CLOCK_BACKGROUND_ICON);
    let cache_key = format!("clock-{}-{}", clock_text, background);
    if let Some(path) = cached_path_if_valid(&cache_key) {
        return Some(path);
    }

    let background_dir = find_readable_background_dir(image_dirs, background);
    let resources_dir = background_dir.as_deref();
    let svg = render_clock_segments_svg(
        resources_dir.unwrap_or_else(|| Path::new(".")),
        &clock_text,
        Some(background),
    );
    let image = load_svg_image_data(CLOCK_ICON_ALIAS, svg.as_bytes(), resources_dir, 256, 256)
        .or_else(|_| {
            let fallback_svg = render_clock_segments_svg(
                Path::new("."),
                &clock_text,
                Some(CLOCK_MISSING_BACKGROUND_SENTINEL),
            );
            load_svg_image_data(CLOCK_ICON_ALIAS, fallback_svg.as_bytes(), None, 256, 256)
        })
        .ok()?;
    write_rounded_png(&cache_key, image)
}

pub(crate) fn render_calendar_icon_png() -> Option<PathBuf> {
    let svg = render_calendar_svg();
    let image = load_svg_image_data(CALENDAR_ICON_ALIAS, svg.as_bytes(), None, 256, 256).ok()?;
    write_calendar_live_png(image)
}

fn write_calendar_live_png(mut image: RgbaImage) -> Option<PathBuf> {
    apply_rounded_corners(&mut image, 0.17);
    let path = rounded_icons_dir().join("calendar-live.png");
    let parent = path.parent()?;
    fs::create_dir_all(parent).ok()?;
    image.save(&path).ok()?;
    Some(path)
}

fn find_readable_background_dir(image_dirs: &[PathBuf], background_name: &str) -> Option<PathBuf> {
    for dir in image_dirs {
        let candidate = dir.join(background_name);
        if !candidate.is_file() {
            continue;
        }
        if image::open(&candidate).is_ok() {
            return Some(dir.clone());
        }
    }
    None
}

pub(crate) fn render_regular_icon_png(image_dirs: &[PathBuf], icon_name: &str) -> Option<PathBuf> {
    if is_blank_background_icon_name(icon_name) {
        return None;
    }

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

#[cfg(test)]
mod tests {
    use super::*;
    use image::{Rgba, RgbaImage};
    use std::fs;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static TEST_COUNTER: AtomicUsize = AtomicUsize::new(0);

    fn test_temp_dir(name: &str) -> PathBuf {
        let id = TEST_COUNTER.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("streamrs-gui-icon-cache-tests-{name}-{id}"));
        fs::create_dir_all(&dir).expect("test directory should be creatable");
        dir
    }

    #[test]
    fn find_readable_background_dir_skips_corrupt_candidates() {
        let root = test_temp_dir("readable-bg");
        let writable = root.join("writable");
        let system = root.join("system");
        fs::create_dir_all(&writable).expect("writable dir should be creatable");
        fs::create_dir_all(&system).expect("system dir should be creatable");

        let background = "blank-test.png";
        fs::write(writable.join(background), b"not-a-valid-png")
            .expect("corrupt writable background should be written");
        RgbaImage::from_pixel(8, 8, Rgba([0, 0, 0, 255]))
            .save(system.join(background))
            .expect("valid system background should be saved");

        let selected = find_readable_background_dir(&[writable, system.clone()], background)
            .expect("readable background directory should be detected");
        assert_eq!(selected, system);
    }

    #[test]
    fn render_clock_icon_png_survives_corrupt_writable_background() {
        let _ = fs::remove_dir_all(rounded_icons_dir());
        let root = test_temp_dir("clock-corrupt-bg");
        let writable = root.join("writable");
        let system = root.join("system");
        fs::create_dir_all(&writable).expect("writable dir should be creatable");
        fs::create_dir_all(&system).expect("system dir should be creatable");

        let background = "blank-clock-corrupt.png";
        fs::write(writable.join(background), b"not-a-valid-png")
            .expect("corrupt writable background should be written");
        RgbaImage::from_pixel(8, 8, Rgba([30, 30, 30, 255]))
            .save(system.join(background))
            .expect("valid system background should be saved");

        let rendered = render_clock_icon_png(&[writable, system], Some(background));
        let path = rendered.expect("clock icon should still render");
        assert!(
            path.is_file(),
            "clock render output should be cached to disk"
        );
        assert!(
            image::open(&path).is_ok(),
            "clock render cache should be a valid image"
        );
    }

    #[test]
    fn render_calendar_icon_png_writes_live_image() {
        let rendered = render_calendar_icon_png();
        let path = rendered.expect("calendar icon should render");
        assert!(path.is_file(), "calendar render output should be written");
        assert!(
            path.ends_with("calendar-live.png"),
            "calendar output path should be the live non-cached target"
        );
        assert!(
            image::open(&path).is_ok(),
            "calendar output should be valid"
        );
    }

    #[test]
    fn write_calendar_live_png_overwrites_target_file() {
        let _ = fs::remove_dir_all(rounded_icons_dir());
        let first = RgbaImage::from_pixel(8, 8, Rgba([10, 20, 30, 255]));
        let second = RgbaImage::from_pixel(8, 8, Rgba([200, 180, 160, 255]));

        let first_path = write_calendar_live_png(first).expect("first calendar write should work");
        let second_path =
            write_calendar_live_png(second).expect("second calendar write should work");

        assert_eq!(
            first_path, second_path,
            "calendar live path should stay stable between renders"
        );
        assert!(
            image::open(&second_path).is_ok(),
            "calendar file should remain readable"
        );
    }
}
