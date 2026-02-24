use chrono::Local;
use std::path::Path;

pub const CLOCK_ICON_ALIAS: &str = "clock.svg";
pub const CLOCK_ICON_PREFIX: &str = "clock://hh:mm";
pub const CLOCK_BACKGROUND_ICON: &str = "blank.png";
pub const CLOCK_FALLBACK_BACKGROUND_COLOR: &str = "#1f1f1f";
pub const CLOCK_VIEWBOX_SIZE: i32 = 72;
pub const CLOCK_DIGIT_WIDTH: i32 = 12;
pub const CLOCK_DIGIT_HEIGHT: i32 = 24;
pub const CLOCK_COLON_WIDTH: i32 = 4;
pub const CLOCK_CHAR_GAP: i32 = 2;

pub fn current_clock_text() -> String {
    Local::now().format("%H:%M").to_string()
}

pub fn is_clock_icon(icon: &str) -> bool {
    icon.eq_ignore_ascii_case(CLOCK_ICON_ALIAS) || icon.eq_ignore_ascii_case(CLOCK_ICON_PREFIX)
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

pub fn render_clock_segments_svg(
    image_dir: &Path,
    text: &str,
    background_name: Option<&str>,
) -> String {
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

#[cfg(test)]
mod tests {
    use super::*;
    use image::{Rgba, RgbaImage};
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static TEST_DIR_COUNTER: AtomicUsize = AtomicUsize::new(0);

    fn test_temp_dir(name: &str) -> PathBuf {
        let mut dir = std::env::temp_dir();
        let id = TEST_DIR_COUNTER.fetch_add(1, Ordering::Relaxed);
        dir.push(format!("streamrs-clock-tests-{name}-{id}"));
        fs::create_dir_all(&dir).expect("test temp dir should be creatable");
        dir
    }

    #[test]
    fn clock_icon_alias_and_prefix_are_case_insensitive() {
        assert!(is_clock_icon("clock.svg"));
        assert!(is_clock_icon("CLOCK.SVG"));
        assert!(is_clock_icon("clock://hh:mm"));
        assert!(is_clock_icon("CLOCK://HH:MM"));
        assert!(!is_clock_icon("clock.png"));
    }

    #[test]
    fn render_clock_svg_uses_image_background_when_present() {
        let dir = test_temp_dir("background");
        RgbaImage::from_pixel(8, 8, Rgba([0, 0, 0, 255]))
            .save(dir.join(CLOCK_BACKGROUND_ICON))
            .expect("background fixture should save");

        let svg = render_clock_segments_svg(&dir, "12:34", None);
        assert!(svg.contains(&format!(r#"href="{CLOCK_BACKGROUND_ICON}""#)));
        assert!(!svg.contains(CLOCK_FALLBACK_BACKGROUND_COLOR));
    }

    #[test]
    fn render_clock_svg_falls_back_for_missing_background() {
        let dir = test_temp_dir("fallback");
        let svg = render_clock_segments_svg(&dir, "12:34", Some("missing.png"));
        assert!(svg.contains(CLOCK_FALLBACK_BACKGROUND_COLOR));
        assert!(!svg.contains(r#"href="missing.png""#));
    }
}
