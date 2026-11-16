#!/usr/bin/env bash
# AppImage package generator for Universal AI Assistant (Linux x86_64)
# Bundles GTK3 Flutter runner, core Rust shared library (libaissistant_ffi.so), and AppImage runtime.

set -euo pipefail

VERSION="${1:-1.0.0}"
ARCH="${2:-x86_64}"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/../../.." && pwd)"
DIST_DIR="${REPO_ROOT}/dist"
BUILD_DIR="${REPO_ROOT}/app/build/linux/x64/release/bundle"

echo "=== Universal AI Assistant: AppImage Builder (v${VERSION}, ${ARCH}) ==="

mkdir -p "${DIST_DIR}"
APP_DIR="${DIST_DIR}/aissistant.AppDir"
rm -rf "${APP_DIR}"
mkdir -p "${APP_DIR}/usr/bin"
mkdir -p "${APP_DIR}/usr/lib/aissistant"
mkdir -p "${APP_DIR}/usr/share/applications"
mkdir -p "${APP_DIR}/usr/share/icons/hicolor/256x256/apps"

# 1. Install AppRun launcher
cp "${SCRIPT_DIR}/AppRun" "${APP_DIR}/AppRun"
chmod +x "${APP_DIR}/AppRun"

# 2. Copy application bundle assets
if [ -d "${BUILD_DIR}" ]; then
  echo "Copying release bundle from: ${BUILD_DIR}"
  cp -r "${BUILD_DIR}/"* "${APP_DIR}/usr/lib/aissistant/"
else
  echo "Notice: Flutter bundle directory ${BUILD_DIR} not found. Creating staged structure."
  mkdir -p "${APP_DIR}/usr/lib/aissistant/data"
  touch "${APP_DIR}/usr/lib/aissistant/aissistant"
  chmod +x "${APP_DIR}/usr/lib/aissistant/aissistant"
fi

# Ensure libaissistant_ffi.so is present
RUST_SO="${REPO_ROOT}/core/target/release/libaissistant_ffi.so"
if [ -f "${RUST_SO}" ]; then
  cp "${RUST_SO}" "${APP_DIR}/usr/lib/aissistant/"
fi

# 3. Create launcher symlink in /usr/bin
cat <<'EOF' > "${APP_DIR}/usr/bin/aissistant"
#!/usr/bin/env bash
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
APP_ROOT="$(cd "${SCRIPT_DIR}/../.." && pwd)"
export LD_LIBRARY_PATH="${APP_ROOT}/usr/lib/aissistant:${LD_LIBRARY_PATH:-}"
exec "${APP_ROOT}/usr/lib/aissistant/aissistant" "$@"
EOF
chmod +x "${APP_DIR}/usr/bin/aissistant"

# 4. Install desktop entry and icon
cp "${SCRIPT_DIR}/aissistant.desktop" "${APP_DIR}/aissistant.desktop"
cp "${SCRIPT_DIR}/aissistant.desktop" "${APP_DIR}/usr/share/applications/"
ICON_SRC="${REPO_ROOT}/app/assets/icons/app_icon_256.png"
if [ -f "${ICON_SRC}" ]; then
  cp "${ICON_SRC}" "${APP_DIR}/aissistant.png"
  cp "${ICON_SRC}" "${APP_DIR}/usr/share/icons/hicolor/256x256/apps/aissistant.png"
else
  touch "${APP_DIR}/aissistant.png"
fi

# 5. Build AppImage using appimagetool if available
OUT_APPIMAGE="${DIST_DIR}/aissistant-linux-${ARCH}-v${VERSION}.AppImage"
if command -v appimagetool >/dev/null 2>&1; then
  ARCH="${ARCH}" appimagetool "${APP_DIR}" "${OUT_APPIMAGE}"
  echo "Generated AppImage: ${OUT_APPIMAGE}"
  sha256sum "${OUT_APPIMAGE}" > "${OUT_APPIMAGE}.sha256"
else
  echo "appimagetool not found on host. Staged AppDir ready at: ${APP_DIR}"
  tar -czf "${OUT_APPIMAGE}.tar.gz" -C "${DIST_DIR}" "aissistant.AppDir"
  sha256sum "${OUT_APPIMAGE}.tar.gz" > "${OUT_APPIMAGE}.tar.gz.sha256"
fi

echo "Linux AppImage packaging complete!"
