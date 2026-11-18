use crate::paging::STREAMDECK_KEY_COUNT;
use image::codecs::jpeg::JpegEncoder;
use image::imageops::{FilterType, crop_imm, rotate180};
use image::{DynamicImage, ImageDecoder, ImageReader, Limits, Pixel, Rgba, RgbaImage};
use std::path::Path;

#[derive(Debug)]
pub struct Wallpaper(RgbaImage);

impl Wallpaper {
    pub fn load(path: &Path) -> Result<Self, String> {
        let image =
            read_oriented_image(path).map_err(|err| format!("{}: {err}", path.display()))?;
        if image.width() == 0 || image.height() == 0 {
            return Err(format!("{}: empty image", path.display()));
        }
        Ok(Self(cover(image, 480, 272)))
    }

    fn origin(slot: usize) -> (u32, u32) {
        (11 + 97 * (slot % 5) as u32, 5 + 97 * (slot / 5) as u32)
    }

    pub fn key(&self, slot: usize) -> RgbaImage {
        let (x, y) = Self::origin(slot);
        crop_imm(&self.0, x, y, 72, 72).to_image()
    }

    pub fn panel_jpeg(
        tiles: &[Option<RgbaImage>; STREAMDECK_KEY_COUNT],
    ) -> Result<Vec<u8>, String> {
        let mut panel = RgbaImage::from_pixel(480, 272, Rgba([0, 0, 0, 255]));
        for (slot, tile) in tiles.iter().enumerate() {
            if let Some(tile) = tile {
                let (x, y) = Self::origin(slot);
                image::imageops::replace(&mut panel, tile, x.into(), y.into());
            }
        }
        device_jpeg(&panel)
    }
}

pub fn device_jpeg(image: &RgbaImage) -> Result<Vec<u8>, String> {
    let rgb = DynamicImage::ImageRgba8(rotate180(image)).into_rgb8();
    let mut data = Vec::new();
    JpegEncoder::new_with_quality(&mut data, 100)
        .encode_image(&rgb)
        .map_err(|err| err.to_string())?;
    Ok(data)
}

fn read_oriented_image(path: &Path) -> image::ImageResult<DynamicImage> {
    let mut decoder = ImageReader::open(path)?
        .with_guessed_format()?
        .into_decoder()?;
    // Account for the output buffer just as ImageReader::decode does.
    let mut limits = Limits::default();
    limits.reserve(decoder.total_bytes())?;
    decoder.set_limits(limits)?;
    let orientation = decoder.orientation()?;
    let mut image = DynamicImage::from_decoder(decoder)?;
    image.apply_orientation(orientation);
    Ok(image)
}

fn cover(image: DynamicImage, width: u32, height: u32) -> RgbaImage {
    let (w, h) = (image.width(), image.height());
    let crop_w = u64::from(h) * u64::from(width) / u64::from(height);
    let crop_h = u64::from(w) * u64::from(height) / u64::from(width);
    let crop_w = crop_w.clamp(1, u64::from(w)) as u32;
    let crop_h = crop_h.clamp(1, u64::from(h)) as u32;
    let mut cropped = if (w, h) == (crop_w, crop_h) {
        image
    } else {
        image.crop_imm((w - crop_w) / 2, (h - crop_h) / 2, crop_w, crop_h)
    };
    if cropped.color().has_alpha() {
        let mut rgba = cropped.into_rgba8();
        for pixel in rgba.pixels_mut().filter(|p| p[3] != 255) {
            let mut black = Rgba([0, 0, 0, 255]);
            black.blend(pixel);
            *pixel = black;
        }
        cropped = DynamicImage::ImageRgba8(rgba);
    }
    cropped
        .resize_exact(width, height, FilterType::Lanczos3)
        .into_rgba8()
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{ImageEncoder, ImageFormat, Rgb, RgbImage};

    #[test]
    fn wallpaper_format_is_detected_from_its_contents() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("wallpaper");
        let color = Rgba([90, 120, 150, 255]);
        RgbaImage::from_pixel(8, 8, color)
            .save_with_format(&path, ImageFormat::Png)
            .unwrap();
        assert_eq!(Wallpaper::load(&path).unwrap().key(0)[(36, 36)], color);
    }

    #[test]
    fn photo_orientation_is_applied_before_the_panel_crop() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("portrait.jpg");
        let image = RgbImage::from_fn(27, 48, |_, y| {
            if y < 24 {
                Rgb([240, 0, 0])
            } else {
                Rgb([0, 0, 240])
            }
        });
        let mut encoder = JpegEncoder::new(std::fs::File::create(&path).unwrap());
        // Little-endian TIFF with one orientation tag, 6 (rotate clockwise).
        encoder
            .set_exif_metadata(vec![
                0x49, 0x49, 42, 0, 8, 0, 0, 0, 1, 0, 0x12, 1, 3, 0, 1, 0, 0, 0, 6, 0, 0, 0, 0, 0,
                0, 0,
            ])
            .unwrap();
        encoder.encode_image(&image).unwrap();

        let wallpaper = Wallpaper::load(&path).unwrap();
        for left in [0, 5, 10] {
            assert!(wallpaper.key(left)[(36, 36)][2] > 220);
            assert!(wallpaper.key(left + 4)[(36, 36)][0] > 220);
        }
    }

    #[test]
    fn hidden_rgb_does_not_tint_neighboring_wallpaper_pixels() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("transparent.png");
        let image = RgbaImage::from_fn(960, 544, |x, _| {
            if x % 2 == 0 {
                Rgba([250, 0, 0, 255])
            } else {
                Rgba([0, 0, 255, 0])
            }
        });
        image.save(&path).unwrap();
        let tile = Wallpaper::load(&path).unwrap().key(0);
        assert_eq!(tile[(36, 36)], Rgba([125, 0, 0, 255]));
    }
}
