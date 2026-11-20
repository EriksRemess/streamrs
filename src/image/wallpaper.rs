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

    pub fn standby_jpeg(&self) -> Result<Vec<u8>, String> {
        let rgb = DynamicImage::ImageRgba8(rotate180(&self.0)).into_rgb8();
        for quality in [95, 85, 70, 50] {
            let mut data = Vec::new();
            JpegEncoder::new_with_quality(&mut data, quality)
                .encode_image(&rgb)
                .map_err(|err| err.to_string())?;
            if let Some(data) = fit_standby_jpeg(data) {
                return Ok(data);
            }
        }
        Err("Standby image exceeds the MK.2 logo storage capacity".into())
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

fn fit_standby_jpeg(mut data: Vec<u8>) -> Option<Vec<u8>> {
    const SECTOR: usize = 4096;
    if data.len() >= 47 * SECTOR {
        return None;
    }
    // The MK.2 counts sectors before prepending its one-byte storage header.
    if data.len().is_multiple_of(SECTOR) {
        data.push(0);
    }
    Some(data)
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
    fn standby_transfer_preserves_the_end_marker_at_flash_sector_boundaries() {
        for len in [4095, 4096, 4097, 8192, 47 * 4096 - 1, 47 * 4096] {
            let mut jpeg = vec![0xab; len];
            jpeg[len - 2..].copy_from_slice(&[0xff, 0xd9]);
            let transfer = fit_standby_jpeg(jpeg.clone());
            if len == 47 * 4096 {
                assert!(transfer.is_none());
                continue;
            }
            let transfer = transfer.unwrap();
            // The firmware writes this many sectors, starting with a one-byte header.
            let stored_payload = transfer.len().div_ceil(4096) * 4096 - 1;
            assert!(
                stored_payload >= jpeg.len(),
                "lost end marker for {len} bytes"
            );
            assert_eq!(&transfer[..len], jpeg);
            assert_eq!(transfer.len(), len + usize::from(len % 4096 == 0));
        }
    }

    #[test]
    fn standby_jpeg_covers_the_panel_and_uses_device_orientation() {
        let image = RgbaImage::from_fn(480, 272, |x, _| {
            if x < 240 {
                Rgba([255, 0, 0, 255])
            } else {
                Rgba([0, 0, 255, 255])
            }
        });
        let jpeg = Wallpaper(image).standby_jpeg().unwrap();
        let decoded = image::load_from_memory(&jpeg).unwrap().into_rgb8();
        assert_eq!(decoded.dimensions(), (480, 272));
        assert!(decoded[(0, 0)][2] > 240);
        assert!(decoded[(479, 271)][0] > 240);
    }

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
