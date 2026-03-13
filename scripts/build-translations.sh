#!/usr/bin/env bash
set -euo pipefail

DOMAIN="streamrs"
LOCALE_ROOT="po/locale"

if ! command -v msgfmt >/dev/null 2>&1; then
    echo "msgfmt is required to build translations. Install gettext and retry." >&2
    exit 1
fi

mkdir -p "${LOCALE_ROOT}"

for po in po/*.po; do
    [ -f "${po}" ] || continue
    lang="$(basename "${po}" .po)"
    target_dir="${LOCALE_ROOT}/${lang}/LC_MESSAGES"
    mkdir -p "${target_dir}"
    msgfmt --check --output-file="${target_dir}/${DOMAIN}.mo" "${po}"
done
