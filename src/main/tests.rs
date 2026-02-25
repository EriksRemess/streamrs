use super::*;
use image::Rgba;
use image::codecs::gif::GifEncoder;
use std::fs;
use std::io::Cursor;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

static TEST_DIR_COUNTER: AtomicUsize = AtomicUsize::new(0);

fn test_temp_dir(name: &str) -> PathBuf {
    let mut dir = std::env::temp_dir();
    let id = TEST_DIR_COUNTER.fetch_add(1, Ordering::Relaxed);
    dir.push(format!("streamrs-tests-{name}-{id}"));
    fs::create_dir_all(&dir).expect("test temp dir should be creatable");
    dir
}

fn write_test_png(path: &Path, rgba: [u8; 4]) {
    RgbaImage::from_pixel(8, 8, Rgba(rgba))
        .save(path)
        .expect("test PNG should be written");
}

fn test_key(icon: &str) -> KeyBinding {
    KeyBinding {
        action: None,
        icon: icon.to_string(),
        clock_background: None,
        icon_on: None,
        icon_off: None,
        status: None,
        status_interval_ms: None,
    }
}

fn test_config_with_keys(keys: Vec<KeyBinding>) -> Config {
    Config {
        vendor_id: default_vendor_id(),
        product_id: default_product_id(),
        usage: default_usage(),
        usage_page: default_usage_page(),
        brightness: default_brightness(),
        keys_per_page: default_keys_per_page(),
        keys,
    }
}

#[test]
fn svg_icon_is_supported() {
    let svg = r##"<svg xmlns="http://www.w3.org/2000/svg" width="128" height="64"><rect width="128" height="64" fill="#00ff00"/></svg>"##;
    let data = get_image_data(Path::new("icon.svg"), svg.as_bytes())
        .expect("SVG should decode and encode for Stream Deck");
    assert!(data.len() > 2);
    assert_eq!(data[0], 0xFF);
    assert_eq!(data[1], 0xD8);
}

#[test]
fn gif_icon_is_supported() {
    let gif: &[u8] = &[
        0x47, 0x49, 0x46, 0x38, 0x39, 0x61, 0x01, 0x00, 0x01, 0x00, 0x80, 0x00, 0x00, 0x00, 0x00,
        0x00, 0xFF, 0xFF, 0xFF, 0x21, 0xF9, 0x04, 0x01, 0x00, 0x00, 0x00, 0x00, 0x2C, 0x00, 0x00,
        0x00, 0x00, 0x01, 0x00, 0x01, 0x00, 0x00, 0x02, 0x02, 0x44, 0x01, 0x00, 0x3B,
    ];
    let data = get_image_data(Path::new("icon.gif"), gif)
        .expect("GIF should decode and encode for Stream Deck");
    assert!(data.len() > 2);
    assert_eq!(data[0], 0xFF);
    assert_eq!(data[1], 0xD8);
}

#[test]
fn animated_gif_fixture_is_supported() {
    let path = Path::new("animated.gif");
    let mut gif_data = Vec::new();
    {
        let mut encoder = GifEncoder::new(&mut gif_data);
        let frame1 = ImageFrame::from_parts(
            RgbaImage::from_pixel(2, 2, Rgba([0, 0, 0, 255])),
            0,
            0,
            image::Delay::from_numer_denom_ms(100, 1),
        );
        let frame2 = ImageFrame::from_parts(
            RgbaImage::from_pixel(2, 2, Rgba([255, 255, 255, 255])),
            0,
            0,
            image::Delay::from_numer_denom_ms(100, 1),
        );
        encoder
            .encode_frames(vec![frame1, frame2])
            .expect("fixture GIF should encode");
    }

    let decoder =
        GifDecoder::new(Cursor::new(gif_data.as_slice())).expect("fixture should decode as GIF");
    let frames = decoder
        .into_frames()
        .collect_frames()
        .expect("fixture frames should decode");
    assert!(frames.len() > 1, "fixture should be an animated GIF");

    let loaded =
        load_animated_gif(path, &gif_data).expect("animated GIF should load for animation");
    match loaded {
        LoadedKeyImage::Animated { frames, delays } => {
            assert!(frames.len() > 1);
            assert_eq!(frames.len(), delays.len());
        }
        LoadedKeyImage::Static(_) => panic!("animated GIF should not load as static"),
        LoadedKeyImage::Clock { .. } => panic!("animated GIF should not load as clock"),
    }
}

#[test]
fn delay_conversion_uses_millisecond_ratio() {
    let exact = delay_to_duration_ms(image::Delay::from_numer_denom_ms(150, 1));
    assert_eq!(exact, Duration::from_millis(150));

    let tiny = delay_to_duration_ms(image::Delay::from_numer_denom_ms(1, 100));
    assert_eq!(tiny, Duration::from_millis(MIN_GIF_FRAME_DELAY_MS));
}

#[test]
fn encode_animated_frames_builds_animation_state() {
    let frame1 = ImageFrame::from_parts(
        RgbaImage::new(8, 8),
        0,
        0,
        image::Delay::from_numer_denom_ms(20, 1),
    );
    let frame2 = ImageFrame::from_parts(
        RgbaImage::new(8, 8),
        0,
        0,
        image::Delay::from_numer_denom_ms(200, 1),
    );

    let loaded = encode_animated_frames(vec![frame1, frame2], Path::new("anim.gif"))
        .expect("multi-frame animation should load");
    match loaded {
        LoadedKeyImage::Animated { frames, delays } => {
            assert_eq!(frames.len(), 2);
            assert_eq!(delays.len(), 2);
            assert_eq!(delays[0], Duration::from_millis(MIN_GIF_FRAME_DELAY_MS));
            assert_eq!(delays[1], Duration::from_millis(200));
        }
        LoadedKeyImage::Static(_) => panic!("expected animated state"),
        LoadedKeyImage::Clock { .. } => panic!("expected animated state"),
    }
}

#[test]
fn clock_icon_renders_svg_without_background_file() {
    let missing_dir = Path::new("/tmp/streamrs-missing-clock-assets");
    let loaded =
        load_key_image(missing_dir, CLOCK_ICON_ALIAS, None).expect("clock icon should render");
    match loaded {
        LoadedKeyImage::Clock {
            image,
            current_text,
            background_name,
        } => {
            assert_eq!(current_text.len(), 5);
            assert_eq!(&current_text[2..3], ":");
            assert!(background_name.is_none());
            assert!(image.len() > 2);
            assert_eq!(image[0], 0xFF);
            assert_eq!(image[1], 0xD8);
        }
        _ => panic!("expected clock image variant"),
    }
}

#[test]
fn clock_svg_uses_fallback_background_when_blank_png_is_missing() {
    let missing_dir = Path::new("/tmp/streamrs-missing-clock-assets");
    let svg = render_clock_segments_svg(missing_dir, "12:34", None);
    assert!(svg.contains(CLOCK_FALLBACK_BACKGROUND_COLOR));
    assert!(!svg.contains(CLOCK_BACKGROUND_ICON));
}

#[test]
fn parse_config_allows_missing_action() {
    let raw = r#"
            [[keys]]
            icon = "blank.png"
        "#;
    let config =
        parse_config(Path::new("test.toml"), raw).expect("config with missing action should parse");
    assert_eq!(config.keys.len(), 1);
    assert!(key_launch_action(&config.keys[0]).is_none());
}

#[test]
fn blank_action_is_treated_as_noop() {
    let raw = r#"
            [[keys]]
            action = "   "
            icon = "blank.png"
        "#;
    let config =
        parse_config(Path::new("test.toml"), raw).expect("config with blank action should parse");
    assert_eq!(config.keys.len(), 1);
    assert!(key_launch_action(&config.keys[0]).is_none());
}

#[test]
fn status_config_parses_and_falls_back_icons() {
    let raw = r#"
            [[keys]]
            icon = "default.png"
            status = "test-command"
            icon_on = "on.png"
        "#;
    let config = parse_config(Path::new("test.toml"), raw).expect("status config should parse");
    let key = &config.keys[0];
    assert_eq!(key_status_command(key).as_deref(), Some("test-command"));
    assert_eq!(key_status_icon_on(key), "on.png");
    assert_eq!(key_status_icon_off(key), "default.png");
}

#[test]
fn status_interval_is_clamped() {
    let raw = r#"
            [[keys]]
            icon = "default.png"
            status = "test-command"
            status_interval_ms = 1
        "#;
    let config =
        parse_config(Path::new("test.toml"), raw).expect("status interval config should parse");
    let key = &config.keys[0];
    assert_eq!(
        key_status_interval(key),
        Duration::from_millis(MIN_STATUS_CHECK_INTERVAL_MS)
    );
}

#[test]
fn parse_config_rejects_empty_key_list() {
    let raw = r#"
            keys = []
        "#;
    let err = parse_config(Path::new("test.toml"), raw).expect_err("empty key list should fail");
    assert!(err.contains("has no keys"));
}

#[test]
fn parse_config_rejects_invalid_keys_per_page() {
    let raw = r#"
            keys_per_page = 2
            [[keys]]
            icon = "blank.png"
        "#;
    let err =
        parse_config(Path::new("test.toml"), raw).expect_err("invalid keys_per_page should fail");
    assert!(err.contains("keys_per_page"));
}

#[test]
fn launcher_like_status_commands_are_detected() {
    assert!(is_launcher_like_command("open https://example.com"));
    assert!(is_launcher_like_command("xdg-open https://example.com"));
    assert!(is_launcher_like_command("gio open https://example.com"));

    assert!(!is_launcher_like_command("gio info file.txt"));
    assert!(!is_launcher_like_command("echo open"));
    assert!(!is_launcher_like_command("systemctl is-active sshd"));
}

#[test]
fn image_cache_warming_includes_status_and_navigation_icons() {
    let dir = test_temp_dir("image-cache-warm");
    for (name, color) in [
        ("base.png", [10, 10, 10, 255]),
        ("status-default.png", [20, 20, 20, 255]),
        ("status-on.png", [0, 255, 0, 255]),
        ("status-off.png", [255, 0, 0, 255]),
        (NEXT_PAGE_ICON, [0, 0, 255, 255]),
        (PREVIOUS_PAGE_ICON, [255, 255, 0, 255]),
    ] {
        write_test_png(&dir.join(name), color);
    }

    let mut keys = Vec::new();
    for _ in 0..15 {
        keys.push(KeyBinding {
            action: None,
            icon: "base.png".to_string(),
            clock_background: None,
            icon_on: None,
            icon_off: None,
            status: None,
            status_interval_ms: None,
        });
    }
    keys.push(KeyBinding {
        action: None,
        icon: "status-default.png".to_string(),
        clock_background: None,
        icon_on: Some("status-on.png".to_string()),
        icon_off: Some("status-off.png".to_string()),
        status: Some("test-status".to_string()),
        status_interval_ms: None,
    });

    let config = Config {
        vendor_id: default_vendor_id(),
        product_id: default_product_id(),
        usage: default_usage(),
        usage_page: default_usage_page(),
        brightness: default_brightness(),
        keys_per_page: default_keys_per_page(),
        keys,
    };

    let cache = build_image_cache(&config, &dir);
    assert_eq!(cache.len(), 6, "duplicate icons should be cached once");
    assert!(cache.contains_key(&ImageCacheKey {
        icon: "base.png".to_string(),
        clock_background: None,
    }));
    assert!(cache.contains_key(&ImageCacheKey {
        icon: "status-on.png".to_string(),
        clock_background: None,
    }));
    assert!(cache.contains_key(&ImageCacheKey {
        icon: "status-off.png".to_string(),
        clock_background: None,
    }));
    assert!(cache.contains_key(&ImageCacheKey {
        icon: NEXT_PAGE_ICON.to_string(),
        clock_background: None,
    }));
    assert!(cache.contains_key(&ImageCacheKey {
        icon: PREVIOUS_PAGE_ICON.to_string(),
        clock_background: None,
    }));
}

#[test]
fn clock_cache_key_includes_background_name() {
    let dir = test_temp_dir("clock-cache-key");
    write_test_png(&dir.join("bg-a.png"), [10, 10, 10, 255]);
    write_test_png(&dir.join("bg-b.png"), [30, 30, 30, 255]);

    let mut cache = ImageCache::new();
    let first = load_key_image_cached(&dir, &mut cache, CLOCK_ICON_ALIAS, Some("bg-a.png"))
        .expect("first clock variant should render");
    let second = load_key_image_cached(&dir, &mut cache, CLOCK_ICON_ALIAS, Some("bg-b.png"))
        .expect("second clock variant should render");
    let repeat = load_key_image_cached(&dir, &mut cache, CLOCK_ICON_ALIAS, Some("bg-a.png"))
        .expect("repeat clock variant should come from cache");

    assert!(matches!(first, LoadedKeyImage::Clock { .. }));
    assert!(matches!(second, LoadedKeyImage::Clock { .. }));
    assert!(matches!(repeat, LoadedKeyImage::Clock { .. }));
    assert_eq!(cache.len(), 2, "clock background variants must not collide");
}

#[test]
fn page_layout_plan_places_navigation_keys_across_pages() {
    let keys = (0..30)
        .map(|i| test_key(&format!("icon-{i}.png")))
        .collect::<Vec<_>>();
    let config = test_config_with_keys(keys);
    let status_cache = StatusCache::new();
    let layout = paging_layout(&config);
    let prev_key = layout.previous_page_key();
    let next_key = layout.next_page_key();

    let first = plan_page_layout(&config, &status_cache, 0);
    assert_eq!(first.total_pages, 3);
    assert_eq!(first.page, 0);
    assert_eq!(first.button_actions[next_key], Some(ButtonAction::NextPage));
    assert_eq!(first.button_actions[prev_key], None);
    assert_eq!(
        first.icons[0].as_ref().map(|(icon, _)| icon.as_str()),
        Some("icon-0.png")
    );
    assert_eq!(
        first.icons[13].as_ref().map(|(icon, _)| icon.as_str()),
        Some("icon-13.png")
    );
    assert_eq!(
        first.icons[next_key]
            .as_ref()
            .map(|(icon, _)| icon.as_str()),
        Some(NEXT_PAGE_ICON)
    );

    let middle = plan_page_layout(&config, &status_cache, 1);
    assert_eq!(middle.page, 1);
    assert_eq!(
        middle.button_actions[prev_key],
        Some(ButtonAction::PreviousPage)
    );
    assert_eq!(
        middle.button_actions[next_key],
        Some(ButtonAction::NextPage)
    );
    assert_eq!(
        middle.icons[0].as_ref().map(|(icon, _)| icon.as_str()),
        Some("icon-14.png")
    );
    assert_eq!(
        middle.icons[12].as_ref().map(|(icon, _)| icon.as_str()),
        Some("icon-26.png")
    );
    assert_eq!(
        middle.icons[prev_key]
            .as_ref()
            .map(|(icon, _)| icon.as_str()),
        Some(PREVIOUS_PAGE_ICON)
    );
    assert_eq!(
        middle.icons[next_key]
            .as_ref()
            .map(|(icon, _)| icon.as_str()),
        Some(NEXT_PAGE_ICON)
    );

    let last = plan_page_layout(&config, &status_cache, 2);
    assert_eq!(last.page, 2);
    assert_eq!(last.button_actions[prev_key], None);
    assert_eq!(
        last.button_actions[next_key],
        Some(ButtonAction::PreviousPage)
    );
    assert_eq!(
        last.icons[0].as_ref().map(|(icon, _)| icon.as_str()),
        Some("icon-27.png")
    );
    assert_eq!(
        last.icons[2].as_ref().map(|(icon, _)| icon.as_str()),
        Some("icon-29.png")
    );
    assert_eq!(
        last.icons[next_key].as_ref().map(|(icon, _)| icon.as_str()),
        Some(PREVIOUS_PAGE_ICON)
    );
}

#[test]
fn page_layout_plan_uses_cached_status_for_initial_icon_and_poll_timing() {
    let mut key = test_key("default.png");
    key.status = Some("test-status".to_string());
    key.icon_on = Some("on.png".to_string());
    key.icon_off = Some("off.png".to_string());
    key.status_interval_ms = Some(2500);
    let config = test_config_with_keys(vec![key]);

    let no_cache_plan = plan_page_layout(&config, &StatusCache::new(), 0);
    let no_cache_status = no_cache_plan.status_slots[0]
        .as_ref()
        .expect("status slot should be planned");
    assert_eq!(no_cache_status.current_on, None);
    assert!(no_cache_status.poll_now);
    assert_eq!(
        no_cache_plan.icons[0]
            .as_ref()
            .map(|(icon, _)| icon.as_str()),
        Some("off.png")
    );

    let mut status_cache = StatusCache::new();
    status_cache.insert("test-status".to_string(), true);
    let cached_plan = plan_page_layout(&config, &status_cache, 0);
    let cached_status = cached_plan.status_slots[0]
        .as_ref()
        .expect("status slot should be planned");
    assert_eq!(cached_status.current_on, Some(true));
    assert!(!cached_status.poll_now);
    assert_eq!(
        cached_plan.icons[0].as_ref().map(|(icon, _)| icon.as_str()),
        Some("on.png")
    );
}

#[test]
fn page_layout_plan_treats_launcher_like_status_as_action_when_missing_action() {
    let mut key = test_key("default.png");
    key.status = Some("xdg-open https://example.com".to_string());
    let config = test_config_with_keys(vec![key]);

    let plan = plan_page_layout(&config, &StatusCache::new(), 0);
    assert!(
        plan.status_slots[0].is_none(),
        "launcher-like status should not poll"
    );
    assert_eq!(
        plan.button_actions[0],
        Some(ButtonAction::Launch(
            "xdg-open https://example.com".to_string()
        ))
    );
    assert_eq!(
        plan.icons[0].as_ref().map(|(icon, _)| icon.as_str()),
        Some("default.png")
    );
    assert!(matches!(
        plan.warnings.as_slice(),
        [PagePlanWarning::LauncherLikeStatusWithoutAction { .. }]
    ));
}
