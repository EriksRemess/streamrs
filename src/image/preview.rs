use crate::image::calendar::{CALENDAR_ICON_ALIAS, is_calendar_icon, render_calendar_svg};
use crate::image::clock::{
    CLOCK_ICON_ALIAS, current_clock_text, is_clock_icon, render_clock_segments_svg,
};
use crate::image::svg::load_svg_data;
use image::{Rgba, RgbaImage, load_from_memory};
use std::{fs, path::Path};

const CLOCK_RENDER_SIZE: u32 = 256;

pub fn load_icon_image(
    icon_name: &str,
    image_dir: &Path,
    clock_background: Option<&str>,
) -> Result<RgbaImage, String> {
    if crate::image::catalog::is_blank_background_icon_name(icon_name) {
        return Ok(RgbaImage::from_pixel(72, 72, Rgba([0, 0, 0, 255])));
    }
    if is_clock_icon(icon_name) {
        let clock_svg =
            render_clock_segments_svg(image_dir, &current_clock_text(), clock_background);
        let img = load_svg_data(
            CLOCK_ICON_ALIAS,
            clock_svg.as_bytes(),
            Some(image_dir),
            CLOCK_RENDER_SIZE,
            CLOCK_RENDER_SIZE,
        )?;
        return Ok(img);
    }
    if is_calendar_icon(icon_name) {
        let calendar_svg = render_calendar_svg();
        let img = load_svg_data(
            CALENDAR_ICON_ALIAS,
            calendar_svg.as_bytes(),
            None,
            CLOCK_RENDER_SIZE,
            CLOCK_RENDER_SIZE,
        )?;
        return Ok(img);
    }

    let path = image_dir.join(icon_name);
    let data =
        fs::read(&path).map_err(|e| format!("Failed to read icon '{}': {e}", path.display()))?;
    if path
        .extension()
        .and_then(|x| x.to_str())
        .unwrap_or_default()
        .eq_ignore_ascii_case("svg")
    {
        let img = load_svg_data(
            &path.display().to_string(),
            &data,
            path.parent(),
            CLOCK_RENDER_SIZE,
            CLOCK_RENDER_SIZE,
        )?;
        return Ok(img);
    }
    load_from_memory(&data)
        .map_err(|e| format!("Failed to decode icon '{}': {e}", path.display()))
        .map(|d| d.to_rgba8())
}
