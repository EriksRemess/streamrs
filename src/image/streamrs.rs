use super::config::{
    key_clock_background, key_status_command, key_status_icon_off, key_status_icon_on,
};
use super::{
    Config, ImageCache, ImageCacheKey, LoadedKeyImage, MIN_GIF_FRAME_DELAY_MS, NEXT_PAGE_ICON,
    PREVIOUS_PAGE_ICON, SVG_RENDER_SIZE, page_count,
};
use image::codecs::gif::GifDecoder;
use image::codecs::jpeg::JpegEncoder;
use image::codecs::png::PngDecoder;
use image::codecs::webp::WebPDecoder;
use image::imageops::FilterType::Lanczos3;
use image::imageops::{crop_imm, resize, rotate180};
use image::{
    AnimationDecoder, DynamicImage, Frame as ImageFrame, GenericImageView, RgbImage, load_from_memory,
};
use std::cmp::min;
use std::fs;
use std::io::Cursor;
use std::path::Path;
use std::time::Duration;
use streamrs::image::clock::{
    CLOCK_ICON_ALIAS, current_clock_text as generic_current_clock_text, is_clock_icon,
    render_clock_segments_svg as generic_render_clock_segments_svg,
};
use streamrs::image::svg::{load_svg_data as load_svg_data_generic, load_svg_dynamic};

fn encode_streamdeck_image(img: DynamicImage) -> Result<Vec<u8>, String> {
    let (width, height) = img.dimensions();
    let crop_size = min(width, height);
    let x_offset = (width - crop_size) / 2;
    let y_offset = (height - crop_size) / 2;
    let mut img = crop_imm(&img, x_offset, y_offset, crop_size, crop_size).to_image();
    img = resize(&rotate180(&img), 72, 72, Lanczos3);

    let mut data = Vec::new();
    JpegEncoder::new_with_quality(&mut data, 100)
        .encode_image(&img)
        .map_err(|err| format!("Failed to encode key image: {err}"))?;
    Ok(data)
}

pub(super) fn get_image_data(icon_path: &Path, img_data: &[u8]) -> Result<Vec<u8>, String> {
    let ext = icon_path
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();

    match ext.as_str() {
        "svg" => {
            let img = load_svg_image(icon_path, img_data)?;
            encode_streamdeck_image(img)
        }
        "gif" => {
            let img = load_gif_first_frame(icon_path, img_data)?;
            encode_streamdeck_image(img)
        }
        _ => {
            let img = load_from_memory(img_data).map_err(|err| {
                format!("Invalid image data for '{}': {err}", icon_path.display())
            })?;
            encode_streamdeck_image(img)
        }
    }
}

fn load_svg_image(icon_path: &Path, img_data: &[u8]) -> Result<DynamicImage, String> {
    load_svg_dynamic(
        &icon_path.display().to_string(),
        img_data,
        icon_path.parent(),
        SVG_RENDER_SIZE,
        SVG_RENDER_SIZE,
    )
}

fn load_gif_first_frame(icon_path: &Path, img_data: &[u8]) -> Result<DynamicImage, String> {
    let decoder = GifDecoder::new(Cursor::new(img_data))
        .map_err(|err| format!("Failed to decode GIF icon '{}': {err}", icon_path.display()))?;
    let frame = decoder
        .into_frames()
        .next()
        .transpose()
        .map_err(|err| {
            format!(
                "Failed to decode GIF frame for '{}': {err}",
                icon_path.display()
            )
        })?
        .ok_or_else(|| format!("GIF icon '{}' has no frames", icon_path.display()))?;

    Ok(DynamicImage::ImageRgba8(frame.into_buffer()))
}

pub(super) fn delay_to_duration_ms(delay: image::Delay) -> Duration {
    let (numerator, denominator) = delay.numer_denom_ms();
    let delay_ms = if denominator == 0 {
        MIN_GIF_FRAME_DELAY_MS
    } else {
        ((numerator as u64) / (denominator as u64)).max(MIN_GIF_FRAME_DELAY_MS)
    };
    Duration::from_millis(delay_ms)
}

pub(super) fn encode_animated_frames(
    frames: Vec<ImageFrame>,
    icon_path: &Path,
) -> Result<LoadedKeyImage, String> {
    if frames.is_empty() {
        return Err(format!(
            "Animated icon '{}' has no frames",
            icon_path.display()
        ));
    }

    let mut encoded_frames = Vec::with_capacity(frames.len());
    let mut delays = Vec::with_capacity(frames.len());

    for frame in frames {
        let delay = delay_to_duration_ms(frame.delay());
        let image = DynamicImage::ImageRgba8(frame.into_buffer());
        encoded_frames.push(encode_streamdeck_image(image)?);
        delays.push(delay);
    }

    if encoded_frames.len() == 1 {
        Ok(LoadedKeyImage::Static(encoded_frames.remove(0)))
    } else {
        Ok(LoadedKeyImage::Animated {
            frames: encoded_frames,
            delays,
        })
    }
}

pub(super) fn load_animated_gif(icon_path: &Path, img_data: &[u8]) -> Result<LoadedKeyImage, String> {
    let decoder = GifDecoder::new(Cursor::new(img_data))
        .map_err(|err| format!("Failed to decode GIF icon '{}': {err}", icon_path.display()))?;
    let frames: Vec<ImageFrame> = decoder.into_frames().collect_frames().map_err(|err| {
        format!(
            "Failed to decode GIF frames '{}': {err}",
            icon_path.display()
        )
    })?;
    encode_animated_frames(frames, icon_path)
}

fn load_apng_or_static_png(icon_path: &Path, img_data: &[u8]) -> Result<LoadedKeyImage, String> {
    let decoder = PngDecoder::new(Cursor::new(img_data))
        .map_err(|err| format!("Failed to decode PNG icon '{}': {err}", icon_path.display()))?;
    let is_apng = decoder.is_apng().map_err(|err| {
        format!(
            "Failed to inspect PNG icon '{}': {err}",
            icon_path.display()
        )
    })?;
    if !is_apng {
        return Ok(LoadedKeyImage::Static(get_image_data(icon_path, img_data)?));
    }

    let apng_decoder = decoder.apng().map_err(|err| {
        format!(
            "Failed to decode APNG icon '{}': {err}",
            icon_path.display()
        )
    })?;
    let frames: Vec<ImageFrame> = apng_decoder.into_frames().collect_frames().map_err(|err| {
        format!(
            "Failed to decode APNG frames '{}': {err}",
            icon_path.display()
        )
    })?;
    encode_animated_frames(frames, icon_path)
}

fn load_animated_webp_or_static(
    icon_path: &Path,
    img_data: &[u8],
) -> Result<LoadedKeyImage, String> {
    let decoder = WebPDecoder::new(Cursor::new(img_data)).map_err(|err| {
        format!(
            "Failed to decode WebP icon '{}': {err}",
            icon_path.display()
        )
    })?;
    if !decoder.has_animation() {
        return Ok(LoadedKeyImage::Static(get_image_data(icon_path, img_data)?));
    }

    let frames: Vec<ImageFrame> = decoder.into_frames().collect_frames().map_err(|err| {
        format!(
            "Failed to decode animated WebP frames '{}': {err}",
            icon_path.display()
        )
    })?;
    encode_animated_frames(frames, icon_path)
}

pub(super) fn render_clock_segments_svg(
    image_dir: &Path,
    text: &str,
    background_name: Option<&str>,
) -> String {
    generic_render_clock_segments_svg(image_dir, text, background_name)
}

pub(super) fn current_clock_text() -> String {
    generic_current_clock_text()
}

pub(super) fn render_clock_svg(
    image_dir: &Path,
    text: &str,
    background_name: Option<&str>,
) -> Result<Vec<u8>, String> {
    let svg = render_clock_segments_svg(image_dir, text, background_name);
    let img = load_svg_data_generic(
        CLOCK_ICON_ALIAS,
        svg.as_bytes(),
        Some(image_dir),
        SVG_RENDER_SIZE,
        SVG_RENDER_SIZE,
    )?;
    encode_streamdeck_image(DynamicImage::ImageRgba8(img))
}

fn load_clock_icon(
    image_dir: &Path,
    background_name: Option<&str>,
) -> Result<LoadedKeyImage, String> {
    let text = current_clock_text();
    let image = render_clock_svg(image_dir, &text, background_name)?;
    Ok(LoadedKeyImage::Clock {
        image,
        current_text: text,
        background_name: background_name.map(|name| name.to_string()),
    })
}

pub(super) fn load_key_image(
    image_dir: &Path,
    icon: &str,
    clock_background: Option<&str>,
) -> Result<LoadedKeyImage, String> {
    if is_clock_icon(icon) {
        return load_clock_icon(image_dir, clock_background);
    }

    let icon_path = image_dir.join(icon);
    let img_data = fs::read(&icon_path)
        .map_err(|err| format!("Failed to read icon '{}': {err}", icon_path.display()))?;
    let ext = icon_path
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();

    match ext.as_str() {
        "gif" => load_animated_gif(&icon_path, &img_data),
        "png" => load_apng_or_static_png(&icon_path, &img_data),
        "webp" => load_animated_webp_or_static(&icon_path, &img_data),
        _ => Ok(LoadedKeyImage::Static(get_image_data(
            &icon_path, &img_data,
        )?)),
    }
}

fn image_cache_key(icon: &str, clock_background: Option<&str>) -> ImageCacheKey {
    ImageCacheKey {
        icon: icon.to_string(),
        clock_background: clock_background.map(|name| name.to_string()),
    }
}

fn refresh_cached_clock_image(
    image_dir: &Path,
    cached: &mut LoadedKeyImage,
) -> Result<(), String> {
    let LoadedKeyImage::Clock {
        image,
        current_text,
        background_name,
    } = cached
    else {
        return Ok(());
    };

    let next_text = current_clock_text();
    if *current_text == next_text {
        return Ok(());
    }

    *image = render_clock_svg(image_dir, &next_text, background_name.as_deref())?;
    *current_text = next_text;
    Ok(())
}

pub(super) fn load_key_image_cached(
    image_dir: &Path,
    image_cache: &mut ImageCache,
    icon: &str,
    clock_background: Option<&str>,
) -> Result<LoadedKeyImage, String> {
    let cache_key = image_cache_key(icon, clock_background);
    if let Some(cached) = image_cache.get_mut(&cache_key) {
        refresh_cached_clock_image(image_dir, cached)?;
        return Ok(cached.clone());
    }

    let loaded = load_key_image(image_dir, icon, clock_background)?;
    image_cache.insert(cache_key, loaded.clone());
    Ok(loaded)
}

fn warm_cached_icon(
    image_dir: &Path,
    image_cache: &mut ImageCache,
    icon: &str,
    clock_background: Option<&str>,
) {
    if let Err(err) = load_key_image_cached(image_dir, image_cache, icon, clock_background) {
        eprintln!("{err}");
    }
}

pub(super) fn build_image_cache(config: &Config, image_dir: &Path) -> ImageCache {
    let mut image_cache = ImageCache::new();

    for key in &config.keys {
        let clock_background = key_clock_background(key);
        warm_cached_icon(
            image_dir,
            &mut image_cache,
            &key.icon,
            clock_background.as_deref(),
        );

        if key_status_command(key).is_some() {
            let icon_on = key_status_icon_on(key);
            let icon_off = key_status_icon_off(key);
            warm_cached_icon(
                image_dir,
                &mut image_cache,
                &icon_on,
                clock_background.as_deref(),
            );
            warm_cached_icon(
                image_dir,
                &mut image_cache,
                &icon_off,
                clock_background.as_deref(),
            );
        }
    }

    if page_count(config) > 1 {
        warm_cached_icon(image_dir, &mut image_cache, PREVIOUS_PAGE_ICON, None);
        warm_cached_icon(image_dir, &mut image_cache, NEXT_PAGE_ICON, None);
    }

    image_cache
}

pub(super) fn blank_image_data() -> Result<Vec<u8>, String> {
    let img = RgbImage::new(72, 72);
    let mut data = Vec::new();
    JpegEncoder::new_with_quality(&mut data, 100)
        .encode_image(&img)
        .map_err(|err| format!("Failed to encode blank key image: {err}"))?;
    Ok(data)
}
