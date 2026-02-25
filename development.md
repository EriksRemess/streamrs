# Development

Developer-focused notes for building, running, and packaging `streamrs` from source.
For end-user install and usage docs, see [Readme.md](Readme.md).

## Contributing

Issues and pull requests are welcome.

## Prerequisites

- Rust toolchain (`cargo`, `rustc`)
- GUI build dependencies (GTK4 + libadwaita dev packages)
  - Debian/Ubuntu example: `libgtk-4-dev`, `libadwaita-1-dev`

## Build From Source

Debug build:

```bash
cargo build
```

Release build (all binaries):

```bash
cargo build --release --locked --bin streamrs --bin streamrs-preview --bin streamrs-gui --bin streamrs-icon-compose
```

## Run From Source

Run the daemon:

```bash
cargo run --bin streamrs
```

Run the GUI:

```bash
cargo run --bin streamrs-gui
```

Render a preview image:

```bash
cargo run --bin streamrs-preview -- --output mock.png
```

Compose an icon:

```bash
cargo run --bin streamrs-icon-compose -- path/to/logo.svg
```

## Local Install (User)

Install from a checkout into your user environment:

```bash
make install
```

This installs:
- Binaries into `~/.local/bin/`
- Default profile config into `~/.config/streamrs/default.toml`
- Default images into `~/.local/share/streamrs/default/`
- Desktop entry + icon
- User systemd service into `~/.config/systemd/user/streamrs.service`

Generate the README mock image:

```bash
make mock
```

## Debian Package Build

After building release binaries (see above), create a `.deb`:

```bash
bash scripts/build-deb.sh <version> dist
```

Output path:
- `dist/streamrs_<version>_<arch>.deb`

## Packaging Behavior Notes

- The package installs a user service unit: `streamrs.service`
- Packaging does not globally enable the user unit for all users
- Post-install/upgrade only enables/starts/restarts for active logged-in regular users
- The service unit runs `streamrs --init --force-images` via `ExecStartPre`
- End-user service behavior and manual enable commands are documented in [Readme.md](Readme.md#download)

## Repo Notes

- Default packaged config template: `config/default.toml`
- Packaged systemd user unit: `systemd/streamrs.service`
- Debian packaging script: `scripts/build-deb.sh`
