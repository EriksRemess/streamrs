#!/usr/bin/env bash
set -euo pipefail

DOMAIN="streamrs"

mapfile -t SOURCES < <(rg --files src/gui)

./scripts/extract-gettext.sh "po/${DOMAIN}.pot" "${SOURCES[@]}" src/streamdeck.rs src/streamdeck/info.rs src/config/toml.rs

for po in po/*.po; do
    [ -f "${po}" ] || continue
    msgmerge --update --quiet "${po}" "po/${DOMAIN}.pot"
done
