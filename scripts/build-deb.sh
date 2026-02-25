#!/usr/bin/env bash
set -euo pipefail

if [[ $# -lt 1 || $# -gt 2 ]]; then
    echo "Usage: $0 <version> [output-dir]" >&2
    exit 1
fi

VERSION="$1"
OUTPUT_DIR="${2:-dist}"
ARCH="${DEB_ARCH:-$(dpkg --print-architecture)}"

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BUILD_ROOT="${REPO_ROOT}/target/deb-build/streamrs_${VERSION}_${ARCH}"
PKG_DIR="${BUILD_ROOT}/pkg"
DEBIAN_DIR="${PKG_DIR}/DEBIAN"
APPLICATION_ID="lv.apps.streamrs"
STREAMRS_BIN="${REPO_ROOT}/target/release/streamrs"
PREVIEW_BIN="${REPO_ROOT}/target/release/streamrs-preview"
GUI_BIN="${REPO_ROOT}/target/release/streamrs-gui"
ICON_COMPOSE_BIN="${REPO_ROOT}/target/release/streamrs-icon-compose"
DESKTOP_FILE="${REPO_ROOT}/config/${APPLICATION_ID}.desktop"
APP_ICON_SOURCE="${REPO_ROOT}/config/${APPLICATION_ID}.png"
APP_ICON_NAME="${APPLICATION_ID}.png"

if [[ ! -x "${STREAMRS_BIN}" || ! -x "${PREVIEW_BIN}" || ! -x "${GUI_BIN}" || ! -x "${ICON_COMPOSE_BIN}" ]]; then
    echo "Missing release binaries." >&2
    echo "Expected:" >&2
    echo "  ${STREAMRS_BIN}" >&2
    echo "  ${PREVIEW_BIN}" >&2
    echo "  ${GUI_BIN}" >&2
    echo "  ${ICON_COMPOSE_BIN}" >&2
    echo "Build them first with:" >&2
    echo "  cargo build --release --bin streamrs --bin streamrs-preview --bin streamrs-gui --bin streamrs-icon-compose" >&2
    exit 1
fi

if [[ ! -f "${DESKTOP_FILE}" ]]; then
    echo "Missing desktop file: ${DESKTOP_FILE}" >&2
    exit 1
fi

if [[ ! -f "${APP_ICON_SOURCE}" ]]; then
    echo "Missing app icon source: ${APP_ICON_SOURCE}" >&2
    exit 1
fi

rm -rf "${BUILD_ROOT}"
mkdir -p \
    "${DEBIAN_DIR}" \
    "${PKG_DIR}/usr/bin" \
    "${PKG_DIR}/usr/lib/systemd/user" \
    "${PKG_DIR}/usr/share/applications" \
    "${PKG_DIR}/usr/share/icons/hicolor/512x512/apps" \
    "${PKG_DIR}/usr/share/streamrs/default" \
    "${PKG_DIR}/usr/share/doc/streamrs"

install -m 0755 "${STREAMRS_BIN}" "${PKG_DIR}/usr/bin/streamrs"
install -m 0755 "${PREVIEW_BIN}" "${PKG_DIR}/usr/bin/streamrs-preview"
install -m 0755 "${GUI_BIN}" "${PKG_DIR}/usr/bin/streamrs-gui"
install -m 0755 "${ICON_COMPOSE_BIN}" "${PKG_DIR}/usr/bin/streamrs-icon-compose"
install -m 0644 "${REPO_ROOT}/config/default.toml" "${PKG_DIR}/usr/share/streamrs/default/default.toml"
install -m 0644 "${REPO_ROOT}/systemd/streamrs.service" "${PKG_DIR}/usr/lib/systemd/user/streamrs.service"
install -m 0644 "${DESKTOP_FILE}" "${PKG_DIR}/usr/share/applications/${APPLICATION_ID}.desktop"
install -m 0644 "${APP_ICON_SOURCE}" "${PKG_DIR}/usr/share/icons/hicolor/512x512/apps/${APP_ICON_NAME}"
install -m 0644 "${REPO_ROOT}/Readme.md" "${PKG_DIR}/usr/share/doc/streamrs/README.md"

if [[ -d "${REPO_ROOT}/all_images" ]]; then
    cp -a "${REPO_ROOT}/all_images/." "${PKG_DIR}/usr/share/streamrs/default/"
fi

cat > "${DEBIAN_DIR}/control" <<EOF
Package: streamrs
Version: ${VERSION}
Section: utils
Priority: optional
Architecture: ${ARCH}
Maintainer: Ēriks Remess <eriks@remess.lv>
Depends: libc6 (>= 2.31), libhidapi-hidraw0 | libhidapi-libusb0, libgtk-4-1, libadwaita-1-0
Description: Stream Deck daemon and GUI configurator in Rust
 streamrs sets predefined icons and actions on Stream Deck hardware.
 This package includes streamrs, streamrs-preview, streamrs-gui, and streamrs-icon-compose binaries,
 a systemd user service unit, desktop entry, app icon, sample default profile config,
 and bundled icons.
EOF

cat > "${DEBIAN_DIR}/postinst" <<'EOF'
#!/bin/sh
set -e

SERVICE_NAME="streamrs.service"

disable_global_service_if_present() {
    if command -v systemctl >/dev/null 2>&1; then
        # Migration for older package versions that globally enabled the user unit.
        systemctl --global disable "${SERVICE_NAME}" >/dev/null 2>&1 || true
    fi
}

reload_and_manage_for_active_users() {
    action="$1"

    if ! command -v loginctl >/dev/null 2>&1; then
        return 0
    fi

    loginctl list-users --no-legend 2>/dev/null | while read -r uid user _rest; do
        [ -n "${uid}" ] || continue
        [ -n "${user}" ] || continue
        # Only target regular interactive users; skip greeter/system accounts (e.g. gdm).
        [ "${uid}" -ge 1000 ] 2>/dev/null || continue
        [ -S "/run/user/${uid}/bus" ] || continue

        systemctl --machine="${user}@.host" --user daemon-reload >/dev/null 2>&1 || continue
        systemctl --machine="${user}@.host" --user enable "${SERVICE_NAME}" >/dev/null 2>&1 || true
        if [ "${action}" = "restart" ]; then
            if ! systemctl --machine="${user}@.host" --user restart "${SERVICE_NAME}" >/dev/null 2>&1; then
                systemctl --machine="${user}@.host" --user start "${SERVICE_NAME}" >/dev/null 2>&1 || true
            fi
        else
            systemctl --machine="${user}@.host" --user start "${SERVICE_NAME}" >/dev/null 2>&1 || true
        fi
    done
}

case "${1:-}" in
    configure)
        disable_global_service_if_present
        if [ -n "${2:-}" ]; then
            reload_and_manage_for_active_users restart
        else
            reload_and_manage_for_active_users start
        fi
        ;;
esac

exit 0
EOF
chmod 0755 "${DEBIAN_DIR}/postinst"

cat > "${DEBIAN_DIR}/prerm" <<'EOF'
#!/bin/sh
set -e

SERVICE_NAME="streamrs.service"

disable_service_globally() {
    if command -v systemctl >/dev/null 2>&1; then
        systemctl --global disable "${SERVICE_NAME}" >/dev/null 2>&1 || true
    fi
}

stop_for_active_users() {
    if ! command -v loginctl >/dev/null 2>&1; then
        return 0
    fi

    loginctl list-users --no-legend 2>/dev/null | while read -r uid user _rest; do
        [ -n "${uid}" ] || continue
        [ -n "${user}" ] || continue
        # Only target regular interactive users; skip greeter/system accounts (e.g. gdm).
        [ "${uid}" -ge 1000 ] 2>/dev/null || continue
        [ -S "/run/user/${uid}/bus" ] || continue

        if ! systemctl --machine="${user}@.host" --user disable --now "${SERVICE_NAME}" >/dev/null 2>&1; then
            systemctl --machine="${user}@.host" --user stop "${SERVICE_NAME}" >/dev/null 2>&1 || true
        fi
    done
}

case "${1:-}" in
    remove|deconfigure|purge)
        disable_service_globally
        stop_for_active_users
        ;;
esac

exit 0
EOF
chmod 0755 "${DEBIAN_DIR}/prerm"

cat > "${DEBIAN_DIR}/postrm" <<'EOF'
#!/bin/sh
set -e

reload_active_user_daemons() {
    if ! command -v loginctl >/dev/null 2>&1; then
        return 0
    fi

    loginctl list-users --no-legend 2>/dev/null | while read -r uid user _rest; do
        [ -n "${uid}" ] || continue
        [ -n "${user}" ] || continue
        [ -S "/run/user/${uid}/bus" ] || continue

        systemctl --machine="${user}@.host" --user daemon-reload >/dev/null 2>&1 || true
    done
}

case "${1:-}" in
    remove|purge)
        reload_active_user_daemons
        ;;
esac

exit 0
EOF
chmod 0755 "${DEBIAN_DIR}/postrm"

mkdir -p "${REPO_ROOT}/${OUTPUT_DIR}"
DEB_PATH="${REPO_ROOT}/${OUTPUT_DIR}/streamrs_${VERSION}_${ARCH}.deb"
dpkg-deb --build --root-owner-group "${PKG_DIR}" "${DEB_PATH}" >/dev/null

echo "Built ${DEB_PATH}"
