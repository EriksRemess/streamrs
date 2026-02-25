use super::*;

pub(crate) const EMBEDDED_DECK_LABEL: &str = "embedded:scripts/streamdeck.svg";
pub(crate) const EMBEDDED_DECK_SVG: &[u8] = include_bytes!("../../../scripts/streamdeck.svg");

pub(crate) fn render_blank_base(
    label: &str,
    svg_data: &[u8],
    resources_dir: Option<&Path>,
    width: u32,
    height: u32,
) -> Result<RgbaImage, String> {
    let render_w = width.max(TEMPLATE_RENDER_WIDTH);
    let render_h = height.max(TEMPLATE_RENDER_HEIGHT);
    let rendered = load_svg_image_data(label, svg_data, resources_dir, render_w, render_h)?;

    if render_w == width && render_h == height {
        return Ok(rendered);
    }

    Ok(resize(&rendered, width, height, Lanczos3))
}

pub(crate) fn detect_key_slots(base: &RgbaImage) -> Result<Vec<KeySlot>, String> {
    let width = base.width() as usize;
    let height = base.height() as usize;

    let mut dark = vec![false; width * height];
    for y in 0..height {
        for x in 0..width {
            let p = base.get_pixel(x as u32, y as u32);
            let g = ((p[0] as u16 + p[1] as u16 + p[2] as u16) / 3) as u8;
            dark[y * width + x] = g < 20;
        }
    }

    let min_area = 20_000usize;
    let max_area = 70_000usize;
    let min_w = 150usize;
    let max_w = 260usize;
    let min_h = 150usize;
    let max_h = 260usize;

    let mut visited = vec![false; width * height];
    let mut slots = Vec::new();

    for y in 0..height {
        for x in 0..width {
            let index = y * width + x;
            if visited[index] || !dark[index] {
                continue;
            }

            let mut stack = vec![(x, y)];
            visited[index] = true;

            let mut count = 0usize;
            let mut min_x = x;
            let mut max_x = x;
            let mut min_y = y;
            let mut max_y = y;

            while let Some((cx, cy)) = stack.pop() {
                count += 1;
                min_x = min_x.min(cx);
                max_x = max_x.max(cx);
                min_y = min_y.min(cy);
                max_y = max_y.max(cy);

                let neighbors = [
                    (cx.wrapping_sub(1), cy),
                    (cx + 1, cy),
                    (cx, cy.wrapping_sub(1)),
                    (cx, cy + 1),
                ];

                for (nx, ny) in neighbors {
                    if nx >= width || ny >= height {
                        continue;
                    }
                    let nidx = ny * width + nx;
                    if dark[nidx] && !visited[nidx] {
                        visited[nidx] = true;
                        stack.push((nx, ny));
                    }
                }
            }

            let box_w = max_x - min_x + 1;
            let box_h = max_y - min_y + 1;
            if count < min_area || count > max_area {
                continue;
            }
            if box_w < min_w || box_w > max_w || box_h < min_h || box_h > max_h {
                continue;
            }

            let fill = count as f32 / (box_w * box_h) as f32;
            if !(0.6..=1.1).contains(&fill) {
                continue;
            }

            let x0 = min_x as u32;
            let y0 = min_y as u32;
            let x1 = (max_x + 1) as u32;
            let y1 = (max_y + 1) as u32;

            slots.push(KeySlot {
                x0,
                y0,
                x1,
                y1,
                cx: (x0 + x1 - 1) as f32 / 2.0,
                cy: (y0 + y1 - 1) as f32 / 2.0,
            });
        }
    }

    if slots.len() != KEY_COUNT {
        return Err(format!("Expected 15 key slots, found {}", slots.len()));
    }

    slots.sort_by(|left, right| {
        let left_row = (left.cy / 40.0).round();
        let right_row = (right.cy / 40.0).round();
        left_row
            .partial_cmp(&right_row)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| {
                left.cx
                    .partial_cmp(&right.cx)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
    });

    Ok(slots)
}

pub(crate) fn scale_slots(template_slots: &[KeySlot], width: u32, height: u32) -> Vec<KeySlot> {
    let sx = width as f32 / TEMPLATE_RENDER_WIDTH as f32;
    let sy = height as f32 / TEMPLATE_RENDER_HEIGHT as f32;

    template_slots
        .iter()
        .map(|slot| {
            let mut x0 = (slot.x0 as f32 * sx).round() as i32;
            let mut y0 = (slot.y0 as f32 * sy).round() as i32;
            let mut x1 = (slot.x1 as f32 * sx).round() as i32;
            let mut y1 = (slot.y1 as f32 * sy).round() as i32;

            x0 = x0.clamp(0, width as i32 - 1);
            y0 = y0.clamp(0, height as i32 - 1);
            x1 = x1.clamp(x0 + 1, width as i32);
            y1 = y1.clamp(y0 + 1, height as i32);

            KeySlot {
                x0: x0 as u32,
                y0: y0 as u32,
                x1: x1 as u32,
                y1: y1 as u32,
                cx: (x0 + x1 - 1) as f32 / 2.0,
                cy: (y0 + y1 - 1) as f32 / 2.0,
            }
        })
        .collect()
}

pub(crate) fn fallback_slots(width: u32, height: u32) -> Vec<KeySlot> {
    let margin_x = (width as f32 * 0.12) as i32;
    let margin_top = (height as f32 * 0.18) as i32;
    let gap_x = (width as f32 * 0.035) as i32;
    let gap_y = (height as f32 * 0.065) as i32;

    let key_w = ((width as i32 - (margin_x * 2) - (gap_x * 4)) / 5).max(1);
    let key_h = ((height as i32 - (margin_top * 2) - (gap_y * 2)) / 3).max(1);

    let mut slots = Vec::with_capacity(KEY_COUNT);
    for row in 0..3 {
        for col in 0..5 {
            let x0 = margin_x + col * (key_w + gap_x);
            let y0 = margin_top + row * (key_h + gap_y);
            let x1 = x0 + key_w;
            let y1 = y0 + key_h;
            slots.push(KeySlot {
                x0: x0.max(0) as u32,
                y0: y0.max(0) as u32,
                x1: x1.max(x0 + 1) as u32,
                y1: y1.max(y0 + 1) as u32,
                cx: (x0 + x1 - 1) as f32 / 2.0,
                cy: (y0 + y1 - 1) as f32 / 2.0,
            });
        }
    }

    slots
}

pub(crate) fn key_slots_for_deck(deck_svg_path: &Path) -> Vec<KeySlot> {
    let rendered = match fs::read(deck_svg_path) {
        Ok(svg) => render_blank_base(
            &deck_svg_path.display().to_string(),
            &svg,
            deck_svg_path.parent(),
            TEMPLATE_RENDER_WIDTH,
            TEMPLATE_RENDER_HEIGHT,
        ),
        Err(err) => {
            eprintln!(
                "Failed to read deck SVG '{}': {err}; falling back to embedded template",
                deck_svg_path.display()
            );
            render_blank_base(
                EMBEDDED_DECK_LABEL,
                EMBEDDED_DECK_SVG,
                None,
                TEMPLATE_RENDER_WIDTH,
                TEMPLATE_RENDER_HEIGHT,
            )
        }
    };

    let rendered = match rendered {
        Ok(image) => image,
        Err(err) => {
            eprintln!("{err}; using fallback key layout");
            return fallback_slots(PREVIEW_WIDTH, PREVIEW_HEIGHT);
        }
    };

    let template_slots = match detect_key_slots(&rendered) {
        Ok(slots) => slots,
        Err(err) => {
            eprintln!("{err}; using fallback key layout");
            return fallback_slots(PREVIEW_WIDTH, PREVIEW_HEIGHT);
        }
    };

    scale_slots(&template_slots, PREVIEW_WIDTH, PREVIEW_HEIGHT)
}

pub(crate) fn relayout_deck(
    overlay: &Overlay,
    deck_picture: &Picture,
    key_layer: &Fixed,
    slots: &[KeySlot],
    key_buttons: &[Button],
    key_pictures: &[Picture],
) {
    let available_width = overlay.allocated_width().max(1);
    let available_height = overlay.allocated_height().max(1);

    let scale_x = available_width as f64 / PREVIEW_WIDTH as f64;
    let scale_y = available_height as f64 / PREVIEW_HEIGHT as f64;
    let scale = scale_x.min(scale_y).max(0.01);

    let deck_width = ((PREVIEW_WIDTH as f64) * scale).round() as i32;
    let deck_height = ((PREVIEW_HEIGHT as f64) * scale).round() as i32;
    let offset_x = ((available_width - deck_width) / 2).max(0);
    let offset_y = ((available_height - deck_height) / 2).max(0);

    deck_picture.set_halign(Align::Fill);
    deck_picture.set_valign(Align::Fill);
    deck_picture.set_margin_start(0);
    deck_picture.set_margin_top(0);

    key_layer.set_halign(Align::Fill);
    key_layer.set_valign(Align::Fill);
    key_layer.set_margin_start(0);
    key_layer.set_margin_top(0);

    for (index, button) in key_buttons.iter().enumerate() {
        let slot = slots[index];
        let slot_width = (((slot.x1 - slot.x0) as f64) * scale).round() as i32;
        let slot_height = (((slot.y1 - slot.y0) as f64) * scale).round() as i32;

        let x = ((slot.x0 as f64) * scale).round() + offset_x as f64;
        let y = ((slot.y0 as f64) * scale).round() + offset_y as f64;

        button.set_size_request(slot_width.max(1), slot_height.max(1));
        key_layer.move_(button, x, y);

        let icon_width = ((slot_width as f64) * 0.88).round() as i32;
        let icon_height = ((slot_height as f64) * 0.88).round() as i32;
        key_pictures[index].set_size_request(icon_width.max(1), icon_height.max(1));
    }
}

pub(crate) fn deck_background_temp_path() -> PathBuf {
    env::temp_dir().join("streamrs-gui-deck-background.png")
}

pub(crate) fn write_deck_background_png(
    deck_svg_path: &Path,
    width: u32,
    height: u32,
) -> Option<PathBuf> {
    let rendered = match fs::read(deck_svg_path) {
        Ok(svg) => render_blank_base(
            &deck_svg_path.display().to_string(),
            &svg,
            deck_svg_path.parent(),
            width,
            height,
        ),
        Err(err) => {
            eprintln!(
                "Failed to read deck SVG '{}': {err}; falling back to embedded template",
                deck_svg_path.display()
            );
            render_blank_base(EMBEDDED_DECK_LABEL, EMBEDDED_DECK_SVG, None, width, height)
        }
    }
    .ok()?;

    let output_path = deck_background_temp_path();
    if rendered.save(&output_path).is_ok() {
        Some(output_path)
    } else {
        None
    }
}
