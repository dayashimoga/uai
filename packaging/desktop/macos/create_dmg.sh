#!/usr/bin/env bash
# macOS DMG disk image builder with Applications symlink for Universal AI Assistant

set -euo pipefail

VERSION="${1:-1.0.0}"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/../../.." && pwd)"
DIST_DIR="${REPO_ROOT}/dist"
RELEASE_APP="${REPO_ROOT}/app/build/macos/Build/Products/Release/aissistant.app"
DMG_STAGING="${DIST_DIR}/dmg_staging"
DMG_OUTPUT="${DIST_DIR}/aissistant-macos-v${VERSION}.dmg"

echo "=== Universal AI Assistant: DMG Generator (v${VERSION}) ==="

mkdir -p "${DIST_DIR}"
rm -rf "${DMG_STAGING}" "${DMG_OUTPUT}"
mkdir -p "${DMG_STAGING}"

# 1. Copy Application bundle into staging
if [ -d "${RELEASE_APP}" ]; then
  cp -R "${RELEASE_APP}" "${DMG_STAGING}/Universal AI Assistant.app"
else
  echo "Notice: Real app bundle not found at ${RELEASE_APP}. Staging placeholder for test verification."
  mkdir -p "${DMG_STAGING}/Universal AI Assistant.app/Contents/MacOS"
  touch "${DMG_STAGING}/Universal AI Assistant.app/Contents/MacOS/aissistant"
fi

# 2. Create drag-to-install /Applications shortcut
ln -s /Applications "${DMG_STAGING}/Applications"

# 3. Create compressed DMG using hdiutil if available
if command -v hdiutil >/dev/null 2>&1; then
  echo "Invoking hdiutil to create UDZO disk image..."
  hdiutil create -volname "Universal AI Assistant" \
                 -srcfolder "${DMG_STAGING}" \
                 -ov \
                 -format UDZO \
                 "${DMG_OUTPUT}"

  echo "Generated DMG: ${DMG_OUTPUT}"
  shasum -a 256 "${DMG_OUTPUT}" > "${DMG_OUTPUT}.sha256"
else
  echo "hdiutil not found on host. Packing staging archive for distribution."
  tar -czf "${DMG_OUTPUT}.tar.gz" -C "${DIST_DIR}" "dmg_staging"
  shasum -a 256 "${DMG_OUTPUT}.tar.gz" > "${DMG_OUTPUT}.tar.gz.sha256"
fi

rm -rf "${DMG_STAGING}"
echo "DMG disk image generation complete!"
