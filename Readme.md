![Mock Stream Deck Preview](mock.png)

# streamrs

A lightweight Rust Stream Deck toolkit with:
- `streamrs`: daemon that maps keys to icons and shell commands
- `streamrs-preview`: profile preview renderer
- `streamrs-gui`: GTK/libadwaita GUI configurator

## Hardware

`streamrs` is built for and tested on:
- Elgato Stream Deck MK.2
- USB ID: `0fd9:0080`
- `vendor_id = 4057` (`0x0fd9`)
- `product_id = 128` (`0x0080`)

## Features

- Supports static icons: PNG, JPEG/JPG, SVG
- Supports animated icons: GIF, APNG, animated WebP (rendered by streamrs frame-by-frame)
- Built-in digital clock icon via `icon = "clock.svg"` (or `icon = "clock://hh:mm"`)
- Optional per-key clock background via `clock_background = "blank-*.png"`
- Optional key actions: missing or blank `action` means no-op on press
- Status-driven toggle icons via polling commands
- Automatic pagination when config has more than 15 keys
- Auto-initializes profile assets when config is missing

Clock details:
- Renders `HH:MM` once per second
- Uses `blank.png` as background when present
- Falls back to an internal dark background when `blank.png` is missing

Status icon fields:
- `status = "<command>"`: executed with `sh -c`; exit code `0` = ON, non-zero = OFF
- `icon_on` / `icon_off`: optional ON/OFF icons (fallback to `icon` when omitted)
- `status_interval_ms`: optional poll interval in milliseconds

## Debian Package Install (Recommended)

Install a release package:

```bash
sudo apt install ./streamrs_<version>_<arch>.deb
```

Example architecture values: `amd64`, `arm64`.

Package contents:
- `/usr/bin/streamrs`
- `/usr/bin/streamrs-preview`
- `/usr/bin/streamrs-gui`
- `/usr/lib/systemd/user/streamrs.service`
- `/usr/share/applications/lv.apps.streamrs.desktop`
- `/usr/share/icons/hicolor/512x512/apps/lv.apps.streamrs.png`
- `/usr/share/streamrs/default/default.toml`
- `/usr/share/streamrs/default/` (bundled icons)

First-time setup after install:

```bash
streamrs --init
systemctl --user daemon-reload
systemctl --user enable --now streamrs.service
```

Notes:
- `streamrs --init` copies default config/icons into your user profile paths.
- If you skip `--init`, `streamrs` auto-initializes on first run when config is missing.
- To update profile files from packaged defaults later, run `streamrs --init --force`.

## Usage

After installing the `.deb`, run manually:

```bash
streamrs
```

Open the GUI configurator:

```bash
streamrs-gui
```

CLI flags:
- `--debug`: inherit child process stdout/stderr
- `--profile <name>`: use `~/.config/streamrs/<name>.toml` and `~/.local/share/streamrs/<name>/`
- `--config <path>`: use a custom config path
- `--init`: initialize profile config + images, print service commands, then exit
- `--force`: with `--init`, overwrite existing config/images from source assets

Notes:
- If config is missing, streamrs auto-runs profile initialization before startup.
- `--force` overwrites known source files but does not remove extra files already in the profile image directory.

Pagination:
- `stream-deck-next-page.png` appears on bottom-right when a next page exists.
- `stream-deck-previous-page.png` appears in the bottom-right area when a previous page exists.

## Preview

Use `streamrs-preview` to generate an image from your current `default` profile config + icons:

```bash
streamrs-preview --output mock.png
```

Notes:
- `streamrs-preview` only supports `--output`.
- If `--output` is omitted, it writes `mock.png` in the current directory.
- It reads your current profile data from:
  - `~/.config/streamrs/default.toml`
  - `~/.local/share/streamrs/default/`
- If those are missing, it falls back to packaged defaults under `/usr/share/streamrs/default/`.

## Source Install (Optional)

If you are working from a checkout instead of a `.deb` install:

```bash
make install
```

This installs:
- Binaries:
  - `~/.local/bin/streamrs`
  - `~/.local/bin/streamrs-preview`
  - `~/.local/bin/streamrs-gui`
- Config: `~/.config/streamrs/default.toml`
- Images: `~/.local/share/streamrs/default/`
- Desktop entry: `~/.local/share/applications/lv.apps.streamrs.desktop`
- App icon: `~/.local/share/icons/hicolor/512x512/apps/lv.apps.streamrs.png`
- User service: `~/.config/systemd/user/streamrs.service`

Generate the README mock from source:

```bash
make mock
```

Build a `.deb` locally from source:

```bash
cargo build --release --locked --bin streamrs --bin streamrs-preview --bin streamrs-gui
bash scripts/build-deb.sh <version> dist
```

Build dependencies for GUI-enabled source builds include GTK4 and libadwaita dev packages (for example on Debian/Ubuntu: `libgtk-4-dev` and `libadwaita-1-dev`).

## Credits

- Icon pack source: https://marketplace.elgato.com/product/hexaza-3d4ed1dc-bf33-4f30-9ecd-201769f10c0d
