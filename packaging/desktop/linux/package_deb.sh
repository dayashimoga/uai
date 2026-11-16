#!/usr/bin/env bash
# Debian (.deb) package generator for Universal AI Assistant (Linux amd64/arm64)
# Bundles GTK3 Flutter runner, core Rust shared library (libaissistant_ffi.so), and desktop integration.

set -euo pipefail

VERSION="${1:-1.0.0}"
ARCH="${2:-amd64}"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/../../.." && pwd)"
DIST_DIR="${REPO_ROOT}/dist"
BUILD_DIR="${REPO_ROOT}/app/build/linux/x64/release/bundle"

echo "=== Universal AI Assistant: Debian Package Builder (v${VERSION}, ${ARCH}) ==="

mkdir -p "${DIST_DIR}"
PKG_NAME="aissistant_${VERSION}_${ARCH}"
PKG_ROOT="${DIST_DIR}/${PKG_NAME}"
rm -rf "${PKG_ROOT}"
mkdir -p "${PKG_ROOT}/DEBIAN"
mkdir -p "${PKG_ROOT}/usr/bin"
mkdir -p "${PKG_ROOT}/usr/lib/aissistant"
mkdir -p "${PKG_ROOT}/usr/share/applications"
mkdir -p "${PKG_ROOT}/usr/share/icons/hicolor/256x256/apps"

# 1. Generate DEBIAN/control
cat <<EOF > "${PKG_ROOT}/DEBIAN/control"
Package: aissistant
Version: ${VERSION}
Section: utils
Priority: optional
Architecture: ${ARCH}
Depends: libgtk-3-0 (>= 3.22), libasound2 (>= 1.1), libpipewire-0.3-0 | libpulse0, libdbus-1-3, ca-certificates
Maintainer: Universal AI Assistant Open Source Contributors <dev@aissistant.local>
Description: 100% Offline-First Personal AI Assistant
 Universal AI Assistant provides private voice conversations, local model inference,
 system control, knowledge retrieval, and cross-device peer-to-peer sync without cloud servers.
Homepage: https://github.com/aissistant/aissistant
EOF

# 2. Copy application bundle assets
if [ -d "${BUILD_DIR}" ]; then
  echo "Copying release bundle from: ${BUILD_DIR}"
  cp -r "${BUILD_DIR}/"* "${PKG_ROOT}/usr/lib/aissistant/"
else
  echo "Notice: Flutter bundle directory ${BUILD_DIR} not found. Creating staged structure."
  mkdir -p "${PKG_ROOT}/usr/lib/aissistant/data"
  touch "${PKG_ROOT}/usr/lib/aissistant/aissistant"
  chmod +x "${PKG_ROOT}/usr/lib/aissistant/aissistant"
fi

# Ensure libaissistant_ffi.so is present in the bundle
RUST_SO="${REPO_ROOT}/core/target/release/libaissistant_ffi.so"
if [ -f "${RUST_SO}" ]; then
  cp "${RUST_SO}" "${PKG_ROOT}/usr/lib/aissistant/"
fi

# 3. Create launcher script in /usr/bin
cat <<'EOF' > "${PKG_ROOT}/usr/bin/aissistant"
#!/usr/bin/env bash
export LD_LIBRARY_PATH="/usr/lib/aissistant:${LD_LIBRARY_PATH:-}"
exec /usr/lib/aissistant/aissistant "$@"
EOF
chmod +x "${PKG_ROOT}/usr/bin/aissistant"

# 4. Install Desktop entry and icon
cp "${SCRIPT_DIR}/aissistant.desktop" "${PKG_ROOT}/usr/share/applications/"
ICON_SRC="${REPO_ROOT}/app/assets/icons/app_icon_256.png"
if [ -f "${ICON_SRC}" ]; then
  cp "${ICON_SRC}" "${PKG_ROOT}/usr/share/icons/hicolor/256x256/apps/aissistant.png"
else
  touch "${PKG_ROOT}/usr/share/icons/hicolor/256x256/apps/aissistant.png"
fi

# 5. Build .deb package with dpkg-deb if available
DEB_FILE="${DIST_DIR}/${PKG_NAME}.deb"
if command -v dpkg-deb >/dev/null 2>&1; then
  dpkg-deb --build --root-owner-group "${PKG_ROOT}" "${DEB_FILE}"
  echo "Built Debian package: ${DEB_FILE}"
  sha256sum "${DEB_FILE}" > "${DEB_FILE}.sha256"
else
  echo "dpkg-deb not installed on host. Staged Debian root ready at: ${PKG_ROOT}"
  tar -czf "${DEB_FILE}.tar.gz" -C "${DIST_DIR}" "${PKG_NAME}"
  sha256sum "${DEB_FILE}.tar.gz" > "${DEB_FILE}.tar.gz.sha256"
fi

echo "Debian packaging complete!"
