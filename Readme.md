# Stream Deck daemon in Rust

Simple utility daemon to set predefined icons[^1] and actions on Stream Deck.

Supported icon formats include PNG/JPEG, animated GIF/APNG/animated WebP (rendered frame-by-frame by streamrs), and SVG.

Built-in dynamic clock icon:
- use `icon = "clock.svg"` (or `icon = "clock://hh:mm"`) in your key config.
- streamrs renders `HH:MM` over `blank.png` and refreshes automatically.

Key actions are optional: if `action` is missing or blank, pressing that key does nothing.

Status-driven on/off icons:
- `status = "<command>"`: command is run periodically via `sh -c`; exit code `0` means ON, non-zero means OFF.
- `icon_on` / `icon_off`: optional icons for ON/OFF state (fallback to `icon` when omitted).
- `status_interval_ms`: optional poll interval in milliseconds.

## Installation

```bash
cargo install --path .
make install-assets
```

This installs:

- `~/.config/streamrs/default.toml`
- `~/.local/share/streamrs/default/` (copied from `all_images/`)

## Debian Releases

- GitHub Actions builds a `.deb` package on PRs and pushes.
- Pushing a tag like `v0.2.1` publishes the `.deb` to GitHub Releases.

## Usage

```bash
streamrs
```

Optional flags:

- `--debug`: inherit child process stdout/stderr
- `--profile <name>`: load `~/.config/streamrs/<name>.toml` and images from `~/.local/share/streamrs/<name>/`
- `--config <path>`: load a config file from a custom path
- `--init`: initialize per-user config and images into XDG paths
- `--force`: with `--init`, overwrite existing files

If the selected config file does not exist, streamrs auto-initializes profile assets before startup.

If your config defines more than 15 keys, streamrs paginates automatically:
- `stream-deck-next-page.png` appears on the bottom-right key when a next page exists.
- `stream-deck-previous-page.png` appears on the bottom-right area when a previous page exists.

Quick SVG/GIF check:

```bash
make install-assets
streamrs --profile default
```

Per-user bootstrap after system install:

```bash
streamrs --init
```

The default config now maps:
- key 1 icon to `streamrs-test-svg.svg`
- key 2 icon to `twitch-stream-btn_twitch_toggle_slowchat_inactive.gif` (animated GIF input)

[^1]: Icons from https://marketplace.elgato.com/product/hexaza-3d4ed1dc-bf33-4f30-9ecd-201769f10c0d
