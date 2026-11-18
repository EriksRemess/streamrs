use super::*;
use chrono::{FixedOffset, TimeZone};
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
        shortcut: None,
        icon: icon.to_string(),
        clock_background: None,
        icon_on: None,
        icon_off: None,
        status: None,
        status_interval_seconds: None,
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
        page_wallpapers: Default::default(),
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
        LoadedKeyImage::Calendar { .. } => panic!("animated GIF should not load as calendar"),
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
        LoadedKeyImage::Calendar { .. } => panic!("expected animated state"),
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
fn calendar_icon_renders_svg_without_background_file() {
    let missing_dir = Path::new("/tmp/streamrs-missing-calendar-assets");
    let loaded = load_key_image(missing_dir, CALENDAR_ICON_ALIAS, None)
        .expect("calendar icon should render");
    match loaded {
        LoadedKeyImage::Calendar { image, current_key } => {
            assert_eq!(current_key.len(), 10);
            assert_eq!(&current_key[4..5], "-");
            assert_eq!(&current_key[7..8], "-");
            assert!(image.len() > 2);
            assert_eq!(image[0], 0xFF);
            assert_eq!(image[1], 0xD8);
        }
        _ => panic!("expected calendar image variant"),
    }
}

#[test]
fn calendar_refresh_delay_targets_next_midnight() {
    let offset = FixedOffset::east_opt(2 * 3600).expect("fixed offset should be valid");
    let now = offset
        .with_ymd_and_hms(2026, 3, 11, 23, 59, 30)
        .single()
        .expect("fixed-offset datetime should be valid");
    assert_eq!(
        duration_until_next_midnight_fixed(now),
        Duration::from_secs(30)
    );

    let midday = offset
        .with_ymd_and_hms(2026, 3, 11, 12, 0, 0)
        .single()
        .expect("fixed-offset datetime should be valid");
    assert_eq!(
        duration_until_next_midnight_fixed(midday),
        Duration::from_secs(12 * 3600)
    );
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
fn shortcut_config_is_parsed_as_keyboard_action() {
    let raw = r#"
            [[keys]]
            shortcut = "Ctrl+Shift+T"
            icon = "blank.png"
        "#;
    let config =
        parse_config(Path::new("test.toml"), raw).expect("config with shortcut should parse");
    assert_eq!(
        key_configured_action(&config.keys[0]),
        Some(ConfiguredAction::KeyboardShortcut(
            "Ctrl+Shift+T".to_string()
        ))
    );
}

#[test]
fn blank_icon_family_renders_black_key_image() {
    let loaded = load_key_image(Path::new("/tmp/streamrs-unused"), "blank_3.png", None)
        .expect("blank icon alias should render as black image");
    match loaded {
        LoadedKeyImage::Static(image) => {
            assert!(image.len() > 2);
            assert_eq!(image[0], 0xFF);
            assert_eq!(image[1], 0xD8);
        }
        LoadedKeyImage::Animated { .. } => panic!("blank icon should be static"),
        LoadedKeyImage::Clock { .. } => panic!("blank icon should not load as clock"),
        LoadedKeyImage::Calendar { .. } => panic!("blank icon should not load as calendar"),
    }
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
            status_interval_seconds = 0
        "#;
    let config =
        parse_config(Path::new("test.toml"), raw).expect("status interval config should parse");
    let key = &config.keys[0];
    assert_eq!(
        key_status_interval(key),
        Duration::from_secs(MIN_STATUS_CHECK_INTERVAL_SECONDS)
    );
}

#[test]
fn legacy_status_interval_ms_is_supported() {
    let raw = r#"
            [[keys]]
            icon = "default.png"
            status = "test-command"
            status_interval_ms = 2500
        "#;
    let config = parse_config(Path::new("test.toml"), raw)
        .expect("legacy millisecond status interval config should parse");
    let key = &config.keys[0];
    assert_eq!(config::key_status_interval_seconds(key), 3);
    assert_eq!(key_status_interval(key), Duration::from_secs(3));
}

#[test]
fn parse_config_rejects_empty_key_list() {
    let raw = r#"
            keys = []
        "#;
    let err = parse_config(Path::new("test.toml"), raw).expect_err("empty button list should fail");
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
            shortcut: None,
            icon: "base.png".to_string(),
            clock_background: None,
            icon_on: None,
            icon_off: None,
            status: None,
            status_interval_seconds: None,
            status_interval_ms: None,
        });
    }
    keys.push(KeyBinding {
        action: None,
        shortcut: None,
        icon: "status-default.png".to_string(),
        clock_background: None,
        icon_on: Some("status-on.png".to_string()),
        icon_off: Some("status-off.png".to_string()),
        status: Some("test-status".to_string()),
        status_interval_seconds: None,
        status_interval_ms: None,
    });

    let config = Config {
        vendor_id: default_vendor_id(),
        product_id: default_product_id(),
        usage: default_usage(),
        usage_page: default_usage_page(),
        brightness: default_brightness(),
        keys_per_page: default_keys_per_page(),
        page_wallpapers: Default::default(),
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
    key.status_interval_seconds = Some(3);
    let config = test_config_with_keys(vec![key]);

    let no_cache_plan = plan_page_layout(&config, &StatusCache::new(), 0);
    let no_cache_status = no_cache_plan.status_slots[0]
        .as_ref()
        .expect("status slot should be planned");
    assert_eq!(no_cache_status.current_on, None);
    assert!(no_cache_status.poll_now);
    assert_eq!(no_cache_status.check_interval, Duration::from_secs(3));
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
    assert_eq!(cached_status.check_interval, Duration::from_secs(3));
    assert_eq!(
        cached_plan.icons[0].as_ref().map(|(icon, _)| icon.as_str()),
        Some("on.png")
    );
}

fn page_with_status_key() -> PageState {
    let mut state = PageState::new();
    state.status_states[0] = Some(StatusKeyState {
        command: "echo ok".to_string(),
        icon_on: "on.png".to_string(),
        icon_off: "off.png".to_string(),
        clock_background: None,
        check_interval: Duration::from_secs(5),
        next_check_at: Instant::now() + Duration::from_secs(5),
        current_on: Some(false),
    });

    state
}

#[test]
fn first_page_draw_retries_only_failed_keys_and_activates_their_bindings_on_success() {
    let mut key = test_key("animation.gif");
    key.action = Some("launch-animation".into());
    let mut ready_key = test_key("blank.png");
    ready_key.action = Some("launch-ready".into());
    let config = test_config_with_keys(vec![key, ready_key]);
    let mut page = prepare_page(&config, &StatusCache::new(), 0);
    let mut cache = ImageCache::new();
    cache.insert(
        ImageCacheKey {
            icon: "animation.gif".into(),
            clock_background: None,
        },
        LoadedKeyImage::Animated {
            frames: vec![vec![10], vec![20]],
            delays: vec![Duration::from_secs(1); 2],
        },
    );
    let now = Instant::now();
    let blank = [0xba];
    page.draw_pending_images(now, Path::new("."), &mut cache, &blank, |index, _| {
        if index == 0 || index == 2 {
            Err("short write".into())
        } else {
            Ok(())
        }
    });
    assert_eq!(page.button_actions[0], None);
    assert!(page.dynamic_states[0].is_none());
    assert_eq!(
        page.button_actions[1],
        Some(ButtonAction::Launch("launch-ready".into()))
    );
    page.draw_pending_images(now, Path::new("."), &mut cache, &blank, |_, _| {
        panic!("early retry")
    });

    let retry = now + Duration::from_secs(1);
    let mut uploads = vec![];
    page.draw_pending_images(retry, Path::new("."), &mut cache, &blank, |index, data| {
        uploads.push((index, data.to_vec()));
        Ok(())
    });
    assert_eq!(uploads, [(0, vec![10]), (2, blank.to_vec())]);
    assert_eq!(
        page.button_actions[0],
        Some(ButtonAction::Launch("launch-animation".into()))
    );
    assert!(
        matches!(&page.dynamic_states[0], Some(DynamicKeyState::Animated(animation)) if animation.current_frame == 0)
    );
    page.draw_pending_images(
        retry + Duration::from_secs(1),
        Path::new("."),
        &mut cache,
        &blank,
        |_, _| panic!("already uploaded"),
    );
}

#[test]
fn queued_press_before_retry_requires_release_and_a_fresh_press() {
    let mut key = test_key("blank.png");
    key.action = Some("retried".into());
    let mut other = key.clone();
    other.action = Some("ready".into());
    let mut page = prepare_page(
        &test_config_with_keys(vec![key, other]),
        &StatusCache::new(),
        0,
    );
    let mut keys = KeyState::default();
    let mut cache = ImageCache::new();
    let now = Instant::now();
    page.draw_pending_images(now, Path::new("."), &mut cache, &[0], |index, _| {
        if index == 0 {
            Err("short write".into())
        } else {
            Ok(())
        }
    });
    assert!(
        page.process_input(
            &mut keys,
            |_| Ok(InputReport::Empty),
            |_, _| panic!("unexpected press")
        )
        .is_ok()
    );

    let report = |first, second| {
        let mut bytes = [0; 4 + KEY_COUNT];
        bytes[..4].copy_from_slice(&[1, 0, KEY_COUNT as u8, 0]);
        bytes[4] = first;
        bytes[5] = second;
        InputReport::from_bytes(&bytes)
    };
    let mut backlog = [
        InputReport::from_bytes(&[0xff]),
        report(1, 1),
        report(0, 0),
        report(1, 0),
        InputReport::Empty,
    ]
    .into_iter();
    page.draw_pending_images(
        now + Duration::from_secs(1),
        Path::new("."),
        &mut cache,
        &[0],
        |_, _| Ok(()),
    );
    let mut actions = Vec::new();
    page.process_input(
        &mut keys,
        |timeout| {
            assert_eq!(timeout, 0);
            Ok(backlog.next().unwrap())
        },
        |_, pressed| actions.extend(pressed),
    )
    .unwrap();
    assert_eq!(actions, [(1, ButtonAction::Launch("ready".into()))]);
    assert!(backlog.next().is_none());

    let mut press = |first| {
        let mut actions = Vec::new();
        page.process_input(
            &mut keys,
            |timeout| {
                assert_eq!(timeout, 10);
                Ok(report(first, 0))
            },
            |_, pressed| actions.extend(pressed),
        )
        .unwrap();
        actions
    };
    assert!(press(1).is_empty());
    assert!(press(0).is_empty());
    assert_eq!(press(1), [(0, ButtonAction::Launch("retried".into()))]);
    assert!(press(1).is_empty());
}

#[test]
fn unreadable_icon_requires_a_successful_clear_before_enabling_its_action() {
    let mut key = test_key("missing.png");
    key.action = Some("launch".into());
    let mut page = prepare_page(&test_config_with_keys(vec![key]), &StatusCache::new(), 0);
    let mut cache = ImageCache::new();
    let dir = test_temp_dir("missing-page-icon");
    let now = Instant::now();
    let blank = [0xbb];
    page.draw_pending_images(now, &dir, &mut cache, &blank, |index, data| {
        assert_eq!(data, &blank);
        if index == 0 {
            Err("short write".into())
        } else {
            Ok(())
        }
    });
    assert!(page.button_actions[0].is_none());
    page.draw_pending_images(
        now + Duration::from_secs(1),
        &dir,
        &mut cache,
        &blank,
        |index, data| {
            assert_eq!((index, data), (0, blank.as_slice()));
            Ok(())
        },
    );
    assert_eq!(
        page.button_actions[0],
        Some(ButtonAction::Launch("launch".into()))
    );
}

#[test]
fn navigating_away_discards_a_pending_initial_upload() {
    let mut keys = vec![test_key("blank.png"); 16];
    keys[0].action = Some("old-page".into());
    keys[14].action = Some("new-page".into());
    let config = test_config_with_keys(keys);
    let status = StatusCache::new();
    let mut cache = ImageCache::new();
    let mut page = prepare_page(&config, &status, 0);
    page.draw_pending_images(Instant::now(), Path::new("."), &mut cache, &[], |_, _| {
        Err("short write".into())
    });
    assert_eq!(page.button_actions[0], None);

    page = prepare_page(&config, &status, 1);
    let now = Instant::now();
    page.draw_pending_images(now, Path::new("."), &mut cache, &[], |_, _| Ok(()));
    assert_eq!(
        page.button_actions[0],
        Some(ButtonAction::Launch("new-page".into()))
    );
    page.draw_pending_images(
        now + Duration::from_secs(2),
        Path::new("."),
        &mut cache,
        &[],
        |_, _| panic!("stale retry"),
    );
}

#[test]
fn failed_status_icon_is_uploaded_again_on_the_next_poll() {
    let mut page = page_with_status_key();
    let status = page.status_states[0].as_mut().unwrap();
    assert_eq!(status.current_on, Some(false));
    assert!(
        status
            .display(true, |_, _| Err("short write".into()))
            .is_err()
    );
    assert_eq!(status.current_on, None);
    let mut uploaded = None;
    status
        .display(true, |icon, _| {
            uploaded = Some(icon.to_owned());
            Ok(())
        })
        .unwrap();
    assert_eq!(uploaded.as_deref(), Some("on.png"));
    assert_eq!(status.current_on, Some(true));
}

#[test]
fn failed_animation_upload_retries_the_same_frame_after_its_delay() {
    let now = Instant::now();
    let delay = Duration::from_millis(100);
    let mut animation = AnimatedKeyState {
        frames: vec![vec![0], vec![1], vec![2]],
        delays: vec![delay; 3],
        current_frame: 0,
        next_frame_at: now,
    };
    assert!(
        animation
            .advance(now, |_| Err("short write".into()))
            .is_err()
    );
    assert_eq!(animation.current_frame, 0);
    assert_eq!(animation.next_frame_at, now + delay);
    animation
        .advance(now + delay, |frame| {
            assert_eq!(frame, [1]);
            Ok(())
        })
        .unwrap();
    assert_eq!(animation.current_frame, 1);
}

#[test]
fn failed_calendar_upload_retries_the_current_date() {
    let now = Instant::now();
    let mut calendar = CalendarKeyState {
        current_key: "yesterday".into(),
        next_update_at: now,
    };
    assert!(
        calendar
            .refresh(now, "today".into(), || Err("short write".into()))
            .is_err()
    );
    assert_eq!(calendar.current_key, "yesterday");
    assert_eq!(calendar.next_update_at, now + Duration::from_secs(1));
    calendar
        .refresh(calendar.next_update_at, "today".into(), || Ok(()))
        .unwrap();
    assert_eq!(calendar.current_key, "today");
}

#[test]
fn immediate_status_refresh_schedules_status_key_one_second_later() {
    let mut state = page_with_status_key();
    let before = Instant::now();
    state.refresh_tx.send(0).unwrap();
    state.apply_pending_refreshes();
    let after = Instant::now();

    let scheduled = state.status_states[0]
        .as_ref()
        .expect("status slot should remain present")
        .next_check_at;
    assert!(scheduled >= before + Duration::from_secs(1));
    assert!(scheduled <= after + Duration::from_secs(1));
}

#[test]
fn action_completion_after_navigation_does_not_refresh_the_reused_slot() {
    let mut page = page_with_status_key();
    let action_completion = page.refresh_tx.clone();
    page = page_with_status_key();
    let original_deadline = page.status_states[0].as_ref().unwrap().next_check_at;

    assert!(action_completion.send(0).is_err());
    page.apply_pending_refreshes();

    assert_eq!(
        page.status_states[0].as_ref().unwrap().next_check_at,
        original_deadline
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

#[test]
fn page_layout_plan_uses_keyboard_shortcut_action() {
    let mut key = test_key("default.png");
    key.shortcut = Some("Ctrl+Shift+T".to_string());
    let config = test_config_with_keys(vec![key]);

    let plan = plan_page_layout(&config, &StatusCache::new(), 0);
    assert_eq!(
        plan.button_actions[0],
        Some(ButtonAction::KeyboardShortcut("Ctrl+Shift+T".to_string()))
    );
}

#[test]
fn configured_shortcut_takes_precedence_over_launch_command() {
    let mut key = test_key("default.png");
    key.action = Some("open https://example.com".to_string());
    key.shortcut = Some("Ctrl+Shift+T".to_string());

    assert_eq!(
        key_configured_action(&key),
        Some(ConfiguredAction::KeyboardShortcut(
            "Ctrl+Shift+T".to_string()
        ))
    );
}

#[test]
fn transparent_raster_is_composited_on_black_for_the_deck() {
    let dir = test_temp_dir("transparent");
    let path = dir.join("transparent.png");
    write_test_png(&path, [240, 0, 0, 0]);
    let jpeg = get_image_data(&path, &fs::read(&path).unwrap()).unwrap();
    assert_eq!(
        image::load_from_memory(&jpeg)
            .unwrap()
            .to_rgb8()
            .get_pixel(36, 36)
            .0,
        [0; 3]
    );
}

#[test]
fn reserved_blank_profile_loads_with_no_actions() {
    let path = Path::new("blank.toml");
    assert!(
        load_profile_config(BLANK_PROFILE, path, "keys = []")
            .unwrap()
            .keys
            .is_empty()
    );
}

#[test]
fn wallpaper_draws_empty_faces_and_preserves_configured_keys_on_both_upload_paths() {
    let dir = tempfile::tempdir().unwrap();
    let source = image::RgbImage::from_pixel(480, 272, image::Rgb([200, 0, 0]));
    source.save(dir.path().join("wall.png")).unwrap();
    let mut key = test_key("blank.png");
    key.action = Some("test".into());
    let mut config = test_config_with_keys(vec![key]);
    config.page_wallpapers.insert("1".into(), "wall.png".into());

    for full_upload_succeeds in [true, false] {
        let mut page = prepare_page(&config, &StatusCache::new(), 0);
        let mut margins = PanelMargins::Black;
        page.draw_background(
            dir.path(),
            &mut margins,
            |data| {
                let panel = image::load_from_memory(data).unwrap().to_rgb8();
                assert_eq!(panel.dimensions(), (480, 272));
                // Panel JPEGs are rotated 180 degrees. Sample slot 0, the gap, then slot 1.
                assert!(panel.get_pixel(432, 230)[0] < 5);
                assert!(panel.get_pixel(384, 230)[0] < 5);
                assert!(panel.get_pixel(335, 230)[0] > 190);
                if full_upload_succeeds {
                    Ok(())
                } else {
                    Err("disconnected".into())
                }
            },
            || {
                assert!(!full_upload_succeeds);
                Ok(())
            },
        );
        assert_eq!(
            margins,
            if full_upload_succeeds {
                PanelMargins::Unknown
            } else {
                PanelMargins::Black
            }
        );
        let mut writes = 0;
        page.draw_pending_images(
            Instant::now(),
            dir.path(),
            &mut ImageCache::new(),
            &[],
            |slot, data| {
                let tile = image::load_from_memory(data).unwrap().to_rgb8();
                assert_eq!(tile.get_pixel(36, 36)[0] > 190, slot != 0);
                writes += 1;
                Ok(())
            },
        );
        assert_eq!(writes, if full_upload_succeeds { 1 } else { 15 });
        assert!(matches!(
            page.button_actions[0],
            Some(ButtonAction::Launch(_))
        ));
        assert!(page.awaiting_input_drain[0]);
    }
}

#[test]
fn transparent_icons_upload_identical_bytes_with_and_without_wallpaper() {
    let dir = tempfile::tempdir().unwrap();
    image::RgbImage::from_pixel(480, 272, image::Rgb([0, 0, 240]))
        .save(dir.path().join("wall.png"))
        .unwrap();
    let mut icon = image::RgbaImage::from_pixel(72, 72, image::Rgba([255, 0, 0, 0]));
    icon.put_pixel(35, 10, image::Rgba([240, 0, 0, 255]));
    icon.save(dir.path().join("icon.png")).unwrap();
    let mut config = test_config_with_keys(vec![test_key("icon.png")]);
    let upload = |config: &Config| {
        let mut page = prepare_page(config, &StatusCache::new(), 0);
        page.draw_background(dir.path(), &mut PanelMargins::Black, |_| Ok(()), || Ok(()));
        let mut first = Vec::new();
        page.draw_pending_images(
            Instant::now(),
            dir.path(),
            &mut ImageCache::new(),
            &[],
            |slot, data| {
                if slot == 0 {
                    first.extend_from_slice(data);
                }
                Ok(())
            },
        );
        first
    };
    let expected = upload(&config);
    config.page_wallpapers.insert("1".into(), "wall.png".into());
    assert!(!expected.is_empty());
    assert_eq!(upload(&config), expected);
}

#[test]
fn page_navigation_selects_each_wallpaper_and_clears_unconfigured_pages() {
    let mut config = test_config_with_keys(vec![test_key("blank.png"); 30]);
    config
        .page_wallpapers
        .insert("1".into(), "first.png".into());
    config
        .page_wallpapers
        .insert("2".into(), "second.png".into());
    for (index, expected) in [
        (0, Some("first.png")),
        (1, Some("second.png")),
        (2, None),
        (0, Some("first.png")),
    ] {
        let page = prepare_page(&config, &StatusCache::new(), index);
        assert_eq!(
            page.pending_background,
            Some(
                expected.map_or(PageBackground::Plain, |name| PageBackground::Wallpaper(
                    name.into(),
                    config.wallpaper_slots(index)
                ))
            )
        );
        assert_eq!(page.pending_images.len(), KEY_COUNT);
    }
}

#[test]
fn plain_pages_clear_unknown_margins_once_before_drawing_keys() {
    let mut margins = PanelMargins::on_connect(0x0fd9, 0x0080);
    let mut clears = 0;
    let mut draw = |page: &mut PageState, margins: &mut PanelMargins| {
        page.draw_background(
            Path::new("."),
            margins,
            |_| panic!("unexpected upload"),
            || {
                clears += 1;
                Ok(())
            },
        );
    };
    let mut page = PageState::new();
    draw(&mut page, &mut margins);
    draw(&mut page, &mut margins);
    draw(&mut PageState::new(), &mut margins);
    assert_eq!(margins, PanelMargins::Black);
    draw(
        &mut PageState::new(),
        &mut PanelMargins::on_connect(0x0fd9, 0x0060),
    );
    assert_eq!(clears, 1);
}

#[test]
fn missing_wallpaper_clears_a_previous_wallpapers_margins() {
    let dir = tempfile::tempdir().unwrap();
    let mut margins = PanelMargins::Unknown;
    let mut page = PageState::new();
    page.pending_background = Some(PageBackground::Wallpaper(
        "missing.png".into(),
        [true; KEY_COUNT],
    ));
    page.draw_background(
        dir.path(),
        &mut margins,
        |_| panic!("unexpected upload"),
        || Ok(()),
    );
    assert_eq!(margins, PanelMargins::Black);
    assert!(page.tiles.iter().all(Option::is_none));
}

#[test]
fn failed_lcd_fill_tries_a_black_panel_before_drawing_individual_keys() {
    let dir = tempfile::tempdir().unwrap();
    let config = test_config_with_keys(vec![test_key("blank.png")]);
    for upload_succeeds in [true, false] {
        let mut state = prepare_page(&config, &StatusCache::new(), 0);
        let mut margins = PanelMargins::Unknown;
        let mut uploads = 0;
        state.draw_background(
            dir.path(),
            &mut margins,
            |data| {
                uploads += 1;
                let panel = image::load_from_memory(data).unwrap().to_rgb8();
                assert_eq!(panel.dimensions(), (480, 272));
                assert!(panel.pixels().all(|p| p.0 == [0, 0, 0]));
                if upload_succeeds {
                    Ok(())
                } else {
                    Err("write failed".into())
                }
            },
            || Err("fill failed".into()),
        );
        assert_eq!(uploads, 1);
        assert_eq!(margins == PanelMargins::Black, upload_succeeds);
        let mut writes = 0;
        state.draw_pending_images(
            Instant::now(),
            dir.path(),
            &mut ImageCache::new(),
            &[],
            |_, _| {
                writes += 1;
                Ok(())
            },
        );
        assert_eq!(writes, KEY_COUNT);
    }
}
