# Scripts Notes

## `build-deb.sh`
- Builds a Debian package for `streamrs`.
- Intended for release/packaging workflows.

## `streamrs-preview` (Rust binary)
- Generates a mock Stream Deck preview image from config + icons.
- Source: `src/bin/streamrs-preview.rs`
- Template is embedded in the binary from `scripts/streamdeck.svg` at build time.

Example:

```bash
cargo run --bin streamrs-preview -- --output mock.png
```

Notes:
- The preview uses built-in defaults for config/image directories and rendering parameters.

## `streamrs-icon-compose` (Rust binary)
- Builds a PNG icon from a provided logo (`.svg` or `.png`).
- Source: `src/bin/streamrs-icon-compose.rs`
- Uses embedded `blank*.png` backgrounds and picks the closest accent color match.
- Writes by default into `~/.local/share/streamrs/default/` with auto-suffixed filenames when collisions exist.

## `streamdeck.svg`
- Mock Stream Deck face/template used by `streamrs-preview`.
