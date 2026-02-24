use image::{DynamicImage, RgbaImage};
use resvg::tiny_skia;
use resvg::usvg;
use std::path::Path;

pub fn load_svg_data(
    label: &str,
    svg_data: &[u8],
    resources_dir: Option<&Path>,
    target_w: u32,
    target_h: u32,
) -> Result<RgbaImage, String> {
    let options = usvg::Options {
        resources_dir: resources_dir.map(|p| p.to_path_buf()),
        ..usvg::Options::default()
    };

    let tree = usvg::Tree::from_data(svg_data, &options)
        .map_err(|e| format!("Failed to parse SVG '{label}': {e}"))?;
    let size = tree.size();
    if size.width() <= 0.0 || size.height() <= 0.0 {
        return Err(format!("SVG '{label}' has invalid dimensions"));
    }

    let mut pixmap = tiny_skia::Pixmap::new(target_w, target_h)
        .ok_or_else(|| format!("Failed to allocate raster target for '{label}'"))?;

    let scale = (target_w as f32 / size.width()).min(target_h as f32 / size.height());
    let x_offset = (target_w as f32 - size.width() * scale) / 2.0;
    let y_offset = (target_h as f32 - size.height() * scale) / 2.0;
    let transform =
        tiny_skia::Transform::from_scale(scale, scale).post_translate(x_offset, y_offset);

    resvg::render(&tree, transform, &mut pixmap.as_mut());

    RgbaImage::from_raw(target_w, target_h, pixmap.take())
        .ok_or_else(|| format!("Failed to create RGBA image for '{label}'"))
}

pub fn load_svg_dynamic(
    label: &str,
    svg_data: &[u8],
    resources_dir: Option<&Path>,
    target_w: u32,
    target_h: u32,
) -> Result<DynamicImage, String> {
    load_svg_data(label, svg_data, resources_dir, target_w, target_h).map(DynamicImage::ImageRgba8)
}
