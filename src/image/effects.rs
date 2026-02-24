use image::RgbaImage;

pub fn apply_rounded_corners(image: &mut RgbaImage, radius_fraction: f32) {
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
