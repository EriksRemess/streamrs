# Stream Deck daemon in Rust

Simple utility daemon to set predefined icons[^1] and actions on Stream Deck.

## Installation

```bash
cargo install --path .
make install-assets
```

This installs:

- `~/.config/streamrs/default.toml`
- `~/.local/share/streamrs/default/` (copied from `all_images/`)

## Usage

```bash
streamrs
```

Optional flags:

- `--debug`: inherit child process stdout/stderr
- `--profile <name>`: load `~/.config/streamrs/<name>.toml` and images from `~/.local/share/streamrs/<name>/`
- `--config <path>`: load a config file from a custom path

If your config defines more than 15 keys, streamrs paginates automatically:
- `stream-deck-next-page.png` appears on the bottom-right key when a next page exists.
- `stream-deck-previous-page.png` appears on the bottom-right area when a previous page exists.

[^1]: Icons from https://marketplace.elgato.com/product/hexaza-3d4ed1dc-bf33-4f30-9ecd-201769f10c0d
