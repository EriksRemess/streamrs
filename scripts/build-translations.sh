#!/usr/bin/env bash
set -euo pipefail

DOMAIN="streamrs"
LOCALE_ROOT="po/locale"

mkdir -p "${LOCALE_ROOT}"

for po in po/*.po; do
    [ -f "${po}" ] || continue
    lang="$(basename "${po}" .po)"
    target_dir="${LOCALE_ROOT}/${lang}/LC_MESSAGES"
    mkdir -p "${target_dir}"
    msgfmt --check --output-file="${target_dir}/${DOMAIN}.mo" "${po}"
done
