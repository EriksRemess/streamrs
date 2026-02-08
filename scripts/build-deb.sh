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

rm -rf "${BUILD_ROOT}"
mkdir -p \
    "${DEBIAN_DIR}" \
    "${PKG_DIR}/usr/bin" \
    "${PKG_DIR}/usr/lib/systemd/user" \
    "${PKG_DIR}/usr/share/streamrs/default" \
    "${PKG_DIR}/usr/share/doc/streamrs"

install -m 0755 "${REPO_ROOT}/target/release/streamrs" "${PKG_DIR}/usr/bin/streamrs"
install -m 0644 "${REPO_ROOT}/config/default.toml" "${PKG_DIR}/usr/share/streamrs/default/default.toml"
install -m 0644 "${REPO_ROOT}/systemd/streamrs.service" "${PKG_DIR}/usr/lib/systemd/user/streamrs.service"
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
Maintainer: streamrs maintainers <noreply@users.noreply.github.com>
Depends: libc6 (>= 2.31), libhidapi-hidraw0 | libhidapi-libusb0
Description: Stream Deck daemon in Rust
 streamrs sets predefined icons and actions on Stream Deck hardware.
 This package includes the streamrs binary, systemd user service unit,
 sample default profile config, and bundled icons.
EOF

mkdir -p "${REPO_ROOT}/${OUTPUT_DIR}"
DEB_PATH="${REPO_ROOT}/${OUTPUT_DIR}/streamrs_${VERSION}_${ARCH}.deb"
dpkg-deb --build --root-owner-group "${PKG_DIR}" "${DEB_PATH}" >/dev/null

echo "Built ${DEB_PATH}"
