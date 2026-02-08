# Scripts Notes

## `build-deb.sh`
- Builds a Debian package for `streamrs`.
- Intended for release/packaging workflows.

## `mock_preview.py`
- Generates a mock Stream Deck preview image from config + icons.
- Default template: `scripts/blank.svg`.

Example:

```bash
scripts/mock_preview.py \
  --config ~/.config/streamrs/default.toml \
  --image-dir ~/.local/share/streamrs/default \
  --output dist/mock-current-config.png
```

## `blank.svg`
- Mock Stream Deck face/template used by `mock_preview.py`.
