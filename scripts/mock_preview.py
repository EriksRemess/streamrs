#!/usr/bin/env python3
from __future__ import annotations

import argparse
import base64
import subprocess
import tempfile
import tomllib
from pathlib import Path

import numpy as np
from PIL import Image, ImageChops
from scipy import ndimage as ndi

CLOCK_ICON_ALIAS = "clock.svg"
CLOCK_ICON_PREFIX = "clock://hh:mm"
CLOCK_BACKGROUND_ICON = "blank.png"
CLOCK_FALLBACK_BACKGROUND_COLOR = "#1f1f1f"
CLOCK_VIEWBOX_SIZE = 72
CLOCK_DIGIT_WIDTH = 12
CLOCK_DIGIT_HEIGHT = 24
CLOCK_COLON_WIDTH = 4
CLOCK_CHAR_GAP = 2


def run_cmd(command: list[str]) -> None:
    subprocess.run(command, check=True)


def run_status_command(command: str) -> bool:
    proc = subprocess.run(
        ["sh", "-c", command],
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
    )
    return proc.returncode == 0


def current_clock_text() -> str:
    return subprocess.check_output(["date", "+%H:%M"], text=True).strip()


def seven_segment_pattern(ch: str) -> list[bool]:
    return {
        "0": [True, True, True, True, True, True, False],
        "1": [False, True, True, False, False, False, False],
        "2": [True, True, False, True, True, False, True],
        "3": [True, True, True, True, False, False, True],
        "4": [False, True, True, False, False, True, True],
        "5": [True, False, True, True, False, True, True],
        "6": [True, False, True, True, True, True, True],
        "7": [True, True, True, False, False, False, False],
        "8": [True, True, True, True, True, True, True],
        "9": [True, True, True, True, False, True, True],
    }.get(ch, [False] * 7)


def push_clock_digit_rects(svg_parts: list[str], x: int, y: int, ch: str) -> None:
    segments = seven_segment_pattern(ch)
    segment_rects = [
        (x + 2, y, 8, 2),
        (x + 10, y + 2, 2, 8),
        (x + 10, y + 14, 2, 8),
        (x + 2, y + 22, 8, 2),
        (x, y + 14, 2, 8),
        (x, y + 2, 2, 8),
        (x + 2, y + 11, 8, 2),
    ]
    for enabled, (rx, ry, rw, rh) in zip(segments, segment_rects):
        fill = "#ffffff" if enabled else "#2f2f2f"
        svg_parts.append(
            f'<rect x="{rx}" y="{ry}" width="{rw}" height="{rh}" fill="{fill}"/>'
        )


def clock_char_width(ch: str) -> int:
    return CLOCK_COLON_WIDTH if ch == ":" else CLOCK_DIGIT_WIDTH


def clock_background_svg(image_dir: Path) -> str:
    bg_path = image_dir / CLOCK_BACKGROUND_ICON
    if bg_path.is_file():
        encoded = base64.b64encode(bg_path.read_bytes()).decode("ascii")
        return (
            '<image href="data:image/png;base64,'
            f'{encoded}" x="0" y="0" width="72" height="72"/>'
        )
    return (
        '<rect x="0" y="0" width="72" height="72" '
        f'fill="{CLOCK_FALLBACK_BACKGROUND_COLOR}"/>'
    )


def render_clock_segments_svg(image_dir: Path, text: str) -> str:
    chars = list(text)
    total_width = sum(clock_char_width(ch) for ch in chars)
    total_width += max(0, len(chars) - 1) * CLOCK_CHAR_GAP
    x = (CLOCK_VIEWBOX_SIZE - total_width) // 2
    y = (CLOCK_VIEWBOX_SIZE - CLOCK_DIGIT_HEIGHT) // 2
    glyph_parts: list[str] = []
    for ch in chars:
        if ch == ":":
            glyph_parts.append(
                f'<rect x="{x + 1}" y="{y + 8}" width="2" height="2" fill="#ffffff"/>'
            )
            glyph_parts.append(
                f'<rect x="{x + 1}" y="{y + 16}" width="2" height="2" fill="#ffffff"/>'
            )
            x += clock_char_width(ch) + CLOCK_CHAR_GAP
            continue
        push_clock_digit_rects(glyph_parts, x, y, ch)
        x += clock_char_width(ch) + CLOCK_CHAR_GAP

    background = clock_background_svg(image_dir)
    glyphs = "".join(glyph_parts)
    return (
        '<svg xmlns="http://www.w3.org/2000/svg" width="72" height="72" '
        'viewBox="0 0 72 72">'
        f"{background}{glyphs}</svg>"
    )


def render_svg_to_image(svg_text: str, width: int, height: int) -> Image.Image:
    with tempfile.NamedTemporaryFile(suffix=".svg") as src, tempfile.NamedTemporaryFile(
        suffix=".png"
    ) as out:
        src.write(svg_text.encode("utf-8"))
        src.flush()
        run_cmd(
            [
                "rsvg-convert",
                "-w",
                str(width),
                "-h",
                str(height),
                "-o",
                out.name,
                src.name,
            ]
        )
        return Image.open(out.name).convert("RGBA")


def load_icon_image(icon_name: str, image_dir: Path) -> Image.Image:
    if icon_name.lower() in (CLOCK_ICON_ALIAS, CLOCK_ICON_PREFIX):
        clock_svg = render_clock_segments_svg(image_dir, current_clock_text())
        return render_svg_to_image(clock_svg, 256, 256)

    icon_path = image_dir / icon_name
    ext = icon_path.suffix.lower()
    if ext == ".svg":
        with tempfile.NamedTemporaryFile(suffix=".png") as out:
            run_cmd(
                [
                    "rsvg-convert",
                    "-w",
                    "256",
                    "-h",
                    "256",
                    "-o",
                    out.name,
                    str(icon_path),
                ]
            )
            return Image.open(out.name).convert("RGBA")

    img = Image.open(icon_path)
    try:
        img.seek(0)
    except EOFError:
        pass
    return img.convert("RGBA")


def choose_icon_name(key: dict, image_dir: Path) -> str:
    icon = key.get("icon", "")
    status_command = key.get("status")
    if not status_command:
        return icon
    icon_on = key.get("icon_on")
    icon_off = key.get("icon_off")
    is_on = run_status_command(status_command)
    if is_on and icon_on:
        return icon_on
    if (not is_on) and icon_off:
        return icon_off
    return icon


def render_blank_base(blank_svg: Path, width: int, height: int) -> Image.Image:
    with tempfile.NamedTemporaryFile(suffix=".png") as out:
        run_cmd(
            [
                "rsvg-convert",
                "-w",
                str(width),
                "-h",
                str(height),
                "-o",
                out.name,
                str(blank_svg),
            ]
        )
        return Image.open(out.name).convert("RGBA")


def detect_key_slots(base_image: Image.Image) -> list[dict]:
    gray = np.array(base_image.convert("L"))
    mask = gray < 20
    labels, _ = ndi.label(mask)
    objects = ndi.find_objects(labels)
    slots: list[dict] = []
    for index, obj in enumerate(objects, start=1):
        if obj is None:
            continue
        ys, xs = obj
        height = ys.stop - ys.start
        width = xs.stop - xs.start
        region_mask = labels[obj] == index
        area = int(region_mask.sum())
        if not (20000 <= area <= 70000):
            continue
        if not (150 <= width <= 260 and 150 <= height <= 260):
            continue
        fill = area / float(width * height)
        if not (0.6 <= fill <= 1.1):
            continue
        cx = (xs.start + xs.stop - 1) / 2
        cy = (ys.start + ys.stop - 1) / 2
        slots.append(
            {
                "x0": xs.start,
                "y0": ys.start,
                "x1": xs.stop,
                "y1": ys.stop,
                "width": width,
                "height": height,
                "cx": cx,
                "cy": cy,
                "mask": region_mask.astype(np.uint8) * 255,
            }
        )
    if len(slots) != 15:
        raise RuntimeError(f"Expected 15 key slots, found {len(slots)}")
    slots.sort(key=lambda s: (round(s["cy"] / 40), s["cx"]))
    return slots


def compose_preview(
    blank_svg: Path,
    config_path: Path,
    image_dir: Path,
    out_path: Path,
    width: int,
    height: int,
    icon_inset: int,
    bottom_row_y_offset: int,
    bottom_row_extra_inset: int,
    icon_content_shrink_x: int,
    icon_content_shrink_y: int,
    icon_mask_expand: int,
) -> None:
    config = tomllib.loads(config_path.read_text())
    keys = config.get("keys", [])
    base = render_blank_base(blank_svg, width, height)
    slots = detect_key_slots(base)

    for idx, slot in enumerate(slots):
        if idx >= len(keys):
            continue
        icon_name = choose_icon_name(keys[idx], image_dir)
        try:
            icon = load_icon_image(icon_name, image_dir)
        except Exception:
            icon = Image.new("RGBA", (256, 256), "#202020")

        row_index = idx // 5
        row_icon_inset = icon_inset + (bottom_row_extra_inset if row_index == 2 else 0)

        # Convert inset from Stream Deck key pixels (72x72) to mock render pixels.
        slot_min = min(slot["width"], slot["height"])
        inset_px = int(round((row_icon_inset / 72.0) * slot_min))
        inset_px = max(0, min(inset_px, slot_min // 2 - 1))

        inner_width = max(1, slot["width"] - (inset_px * 2))
        inner_height = max(1, slot["height"] - (inset_px * 2))

        expand_px = max(0, icon_mask_expand)
        box_width = min(slot["width"], inner_width + (expand_px * 2))
        box_height = min(slot["height"], inner_height + (expand_px * 2))

        content_width = max(1, box_width - max(0, icon_content_shrink_x))
        content_height = max(1, box_height - max(0, icon_content_shrink_y))
        fitted_inner = icon.resize((content_width, content_height), Image.Resampling.LANCZOS)
        fitted = Image.new("RGBA", (slot["width"], slot["height"]), (0, 0, 0, 0))
        offset_x = (slot["width"] - box_width) // 2
        offset_y = (slot["height"] - box_height) // 2
        # Keep top-left anchored; shrinking only reduces right/bottom extent.
        fitted.alpha_composite(fitted_inner, (offset_x, offset_y))

        slot_mask_array = slot["mask"] > 0
        slot_mask_img = Image.fromarray((slot_mask_array.astype(np.uint8) * 255)).convert("L")
        if inset_px > 0 or expand_px > 0:
            inner_mask = slot_mask_img.resize(
                (box_width, box_height), Image.Resampling.LANCZOS
            )
            slot_mask = Image.new("L", (slot["width"], slot["height"]), 0)
            slot_mask.paste(inner_mask, (offset_x, offset_y))
        else:
            slot_mask = slot_mask_img
        alpha = fitted.split()[3]
        fitted.putalpha(ImageChops.multiply(alpha, slot_mask))
        y_target = slot["y0"] + (bottom_row_y_offset if row_index == 2 else 0)
        y_target = max(0, min(y_target, height - slot["height"]))
        base.alpha_composite(fitted, (slot["x0"], y_target))

    out_path.parent.mkdir(parents=True, exist_ok=True)
    base.save(out_path)


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Generate a Stream Deck mock preview from config + icons."
    )
    parser.add_argument(
        "--blank-svg",
        default="scripts/blank.svg",
        help="Path to mock deck SVG template (default: scripts/blank.svg).",
    )
    parser.add_argument(
        "--config",
        default=str(Path.home() / ".config/streamrs/default.toml"),
        help="Path to streamrs TOML config.",
    )
    parser.add_argument(
        "--image-dir",
        default=str(Path.home() / ".local/share/streamrs/default"),
        help="Path to icon directory.",
    )
    parser.add_argument(
        "--output",
        default="dist/mock-current-config.png",
        help="Output preview PNG path.",
    )
    parser.add_argument(
        "--width",
        type=int,
        default=1560,
        help="Output width in pixels (default: 1560).",
    )
    parser.add_argument(
        "--height",
        type=int,
        default=1108,
        help="Output height in pixels (default: 1108).",
    )
    parser.add_argument(
        "--icon-inset",
        type=int,
        default=2,
        help="Inset for key icons in Stream Deck key pixels (72x72 scale, default: 2).",
    )
    parser.add_argument(
        "--bottom-row-y-offset",
        type=int,
        default=-2,
        help="Vertical offset for bottom-row icons in output pixels (default: -2).",
    )
    parser.add_argument(
        "--bottom-row-extra-inset",
        type=int,
        default=0,
        help="Additional inset applied only to bottom-row icons (72x72 key scale).",
    )
    parser.add_argument(
        "--icon-content-shrink-x",
        type=int,
        default=2,
        help="Reduce rendered icon content width by this many output pixels; top-left stays fixed.",
    )
    parser.add_argument(
        "--icon-content-shrink-y",
        type=int,
        default=2,
        help="Reduce rendered icon content height by this many output pixels; top-left stays fixed.",
    )
    parser.add_argument(
        "--icon-mask-expand",
        type=int,
        default=0,
        help="Expand the rendered icon+mask box in all directions by this many output pixels.",
    )
    return parser.parse_args()


def main() -> None:
    args = parse_args()
    compose_preview(
        blank_svg=Path(args.blank_svg),
        config_path=Path(args.config),
        image_dir=Path(args.image_dir),
        out_path=Path(args.output),
        width=args.width,
        height=args.height,
        icon_inset=args.icon_inset,
        bottom_row_y_offset=args.bottom_row_y_offset,
        bottom_row_extra_inset=args.bottom_row_extra_inset,
        icon_content_shrink_x=args.icon_content_shrink_x,
        icon_content_shrink_y=args.icon_content_shrink_y,
        icon_mask_expand=args.icon_mask_expand,
    )


if __name__ == "__main__":
    main()
