# Scripts Notes

## `build-deb.sh`
- Builds a Debian package for `streamrs`.
- Intended for release/packaging workflows.

## `streamrs-preview` (Rust binary)
- Generates a mock Stream Deck preview image from config + icons.
- Source: `src/bin/streamrs-preview.rs`
- Default template: `scripts/streamdeck.svg`.

Example:

```bash
cargo run --bin streamrs-preview -- \
  --config ~/.config/streamrs/default.toml \
  --image-dir ~/.local/share/streamrs/default \
  --output dist/mock-current-config.png
```

Notes:
- Status commands are disabled by default for safety. Add `--evaluate-status` to enable.

## `streamdeck.svg`
- Mock Stream Deck face/template used by `streamrs-preview`.
