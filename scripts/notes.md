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

## `streamdeck.svg`
- Mock Stream Deck face/template used by `streamrs-preview`.
