fn current_clock_text() -> String {
    Local::now().format("%H:%M").to_string()
}

fn icon_is_clock(icon_name: &str) -> bool {
    icon_name.eq_ignore_ascii_case(CLOCK_ICON_ALIAS)
        || icon_name.eq_ignore_ascii_case(CLOCK_ICON_PREFIX)
}

fn seven_segment_pattern(ch: char) -> [bool; 7] {
    match ch {
        '0' => [true, true, true, true, true, true, false],
        '1' => [false, true, true, false, false, false, false],
        '2' => [true, true, false, true, true, false, true],
        '3' => [true, true, true, true, false, false, true],
        '4' => [false, true, true, false, false, true, true],
        '5' => [true, false, true, true, false, true, true],
        '6' => [true, false, true, true, true, true, true],
        '7' => [true, true, true, false, false, false, false],
        '8' => [true, true, true, true, true, true, true],
        '9' => [true, true, true, true, false, true, true],
        _ => [false; 7],
    }
}

fn push_clock_digit_rects(svg: &mut String, x: i32, y: i32, ch: char) {
    let segments = seven_segment_pattern(ch);
    let segment_rects = [
        (x + 2, y, 8, 2),
        (x + 10, y + 2, 2, 8),
        (x + 10, y + 14, 2, 8),
        (x + 2, y + 22, 8, 2),
        (x, y + 14, 2, 8),
        (x, y + 2, 2, 8),
        (x + 2, y + 11, 8, 2),
    ];

    for (enabled, (rx, ry, rw, rh)) in segments.iter().zip(segment_rects) {
        let fill = if *enabled { "#ffffff" } else { "#2f2f2f" };
        svg.push_str(&format!(
            r##"<rect x="{rx}" y="{ry}" width="{rw}" height="{rh}" fill="{fill}"/>"##
        ));
    }
}

fn clock_char_width(ch: char) -> i32 {
    if ch == ':' {
        CLOCK_COLON_WIDTH
    } else {
        CLOCK_DIGIT_WIDTH
    }
}

fn clock_background_svg(image_dir: &Path, background_name: Option<&str>) -> String {
    let selected = background_name.unwrap_or(CLOCK_BACKGROUND_ICON);
    if image_dir.join(selected).is_file() {
        format!(r##"<image href="{selected}" x="0" y="0" width="72" height="72"/>"##)
    } else {
        format!(
            r##"<rect x="0" y="0" width="72" height="72" fill="{CLOCK_FALLBACK_BACKGROUND_COLOR}"/>"##
        )
    }
}

fn render_clock_segments_svg(image_dir: &Path, text: &str, background_name: Option<&str>) -> String {
    let chars: Vec<char> = text.chars().collect();
    let gaps = chars.len().saturating_sub(1) as i32;
    let total_width =
        chars.iter().map(|ch| clock_char_width(*ch)).sum::<i32>() + (gaps * CLOCK_CHAR_GAP);

    let mut x = (CLOCK_VIEWBOX_SIZE - total_width) / 2;
    let y = (CLOCK_VIEWBOX_SIZE - CLOCK_DIGIT_HEIGHT) / 2;
    let mut glyphs = String::new();

    for ch in chars {
        if ch == ':' {
            glyphs.push_str(&format!(
                r##"<rect x="{}" y="{}" width="2" height="2" fill="#ffffff"/><rect x="{}" y="{}" width="2" height="2" fill="#ffffff"/>"##,
                x + 1,
                y + 8,
                x + 1,
                y + 16
            ));
            x += clock_char_width(ch) + CLOCK_CHAR_GAP;
            continue;
        }

        push_clock_digit_rects(&mut glyphs, x, y, ch);
        x += clock_char_width(ch) + CLOCK_CHAR_GAP;
    }

    format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="72" height="72" viewBox="0 0 72 72">
{background}
{glyphs}
</svg>"##,
        background = clock_background_svg(image_dir, background_name),
        glyphs = glyphs
    )
}

fn rounded_icons_dir() -> PathBuf {
    env::temp_dir().join("streamrs-gui-rounded-icons")
}

fn cache_hash_key(key: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    key.hash(&mut hasher);
    hasher.finish()
}

fn rounded_icon_path(cache_key: &str) -> PathBuf {
    rounded_icons_dir().join(format!("{:016x}.png", cache_hash_key(cache_key)))
}

fn cached_path_if_valid(cache_key: &str) -> Option<PathBuf> {
    let path = rounded_icon_path(cache_key);
    if path.is_file() && image::open(&path).is_ok() {
        Some(path)
    } else {
        None
    }
}

fn apply_rounded_corners(image: &mut RgbaImage, radius_fraction: f32) {
    let width = image.width() as i32;
    let height = image.height() as i32;
    if width <= 1 || height <= 1 {
        return;
    }

    let radius = (((width.min(height) as f32) * radius_fraction).round() as i32)
        .clamp(2, width.min(height) / 2);
    let edge = radius - 1;
    let right_start = width - radius;
    let bottom_start = height - radius;
    let radius_sq = radius * radius;

    for y in 0..height {
        for x in 0..width {
            let dx = if x < radius {
                edge - x
            } else if x >= right_start {
                x - right_start
            } else {
                0
            };

            let dy = if y < radius {
                edge - y
            } else if y >= bottom_start {
                y - bottom_start
            } else {
                0
            };

            if dx > 0 && dy > 0 && (dx * dx + dy * dy > radius_sq) {
                let pixel = image.get_pixel_mut(x as u32, y as u32);
                pixel[3] = 0;
            }
        }
    }
}

fn write_rounded_png(cache_key: &str, mut image: RgbaImage) -> Option<PathBuf> {
    let path = rounded_icon_path(cache_key);
    if path.is_file() {
        if image::open(&path).is_ok() {
            return Some(path);
        }
    }
    let parent = path.parent()?;
    fs::create_dir_all(parent).ok()?;
    apply_rounded_corners(&mut image, 0.17);
    image.save(&path).ok()?;
    Some(path)
}

fn render_clock_icon_png(image_dirs: &[PathBuf], background_name: Option<&str>) -> Option<PathBuf> {
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
    let image = load_svg_data(CLOCK_ICON_ALIAS, svg.as_bytes(), resources_dir, 256, 256).ok()?;
    write_rounded_png(&cache_key, image)
}

fn render_regular_icon_png(image_dirs: &[PathBuf], icon_name: &str) -> Option<PathBuf> {
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
        load_svg_data(
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

