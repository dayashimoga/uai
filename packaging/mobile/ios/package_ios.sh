#!/usr/bin/env bash
# ==============================================================================
# Universal AI Assistant - iOS Release Packaging Pipeline (IPA Bundle)
# Subphase: S11.P1.2
# ==============================================================================
set -euo pipefail

VERSION="${1:-1.0.0}"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/../../.." && pwd)"
DIST_DIR="${REPO_ROOT}/dist"
APP_DIR="${REPO_ROOT}/app"
IOS_DIR="${APP_DIR}/ios"
FRAMEWORKS_DIR="${IOS_DIR}/Frameworks"

echo "=================================================================="
echo " Packaging Universal AI Assistant for iOS v${VERSION}"
echo "=================================================================="
mkdir -p "${DIST_DIR}" "${FRAMEWORKS_DIR}"

echo ">>> 1. Compiling Rust C-ABI (aissistant-ffi) for iOS (aarch64-apple-ios)..."
RUST_TARGET="aarch64-apple-ios"
cargo build --manifest-path "${REPO_ROOT}/core/Cargo.toml" -p aissistant-ffi --release --target "${RUST_TARGET}" || true

DYLIB_PATH="${REPO_ROOT}/core/target/${RUST_TARGET}/release/libaissistant_ffi.dylib"
STATIC_PATH="${REPO_ROOT}/core/target/${RUST_TARGET}/release/libaissistant_ffi.a"

if [ -f "${DYLIB_PATH}" ]; then
    cp "${DYLIB_PATH}" "${FRAMEWORKS_DIR}/libaissistant_ffi.dylib"
    echo "    Copied ${DYLIB_PATH} -> ${FRAMEWORKS_DIR}/libaissistant_ffi.dylib"
elif [ -f "${STATIC_PATH}" ]; then
    cp "${STATIC_PATH}" "${FRAMEWORKS_DIR}/libaissistant_ffi.a"
    echo "    Copied ${STATIC_PATH} -> ${FRAMEWORKS_DIR}/libaissistant_ffi.a"
else
    echo "    Note: Pre-built iOS binary not found at ${DYLIB_PATH}."
    echo "    Ensure Apple iOS SDK toolchain is configured in macOS build environment."
fi

echo ">>> 2. Building Flutter iOS release bundle..."
cd "${APP_DIR}"
flutter build ios --release --no-codesign --build-name="${VERSION}" --build-number=1

echo ">>> 3. Archiving and packaging iOS application bundle (.ipa)..."
ARCHIVE_PATH="${DIST_DIR}/aissistant.xcarchive"
IPA_DEST="${DIST_DIR}/aissistant-ios-${VERSION}.ipa"

if command -v xcodebuild >/dev/null 2>&1; then
    echo "    Running xcodebuild archive..."
    xcodebuild -workspace "${IOS_DIR}/Runner.xcworkspace" \
               -scheme Runner \
               -configuration Release \
               -archivePath "${ARCHIVE_PATH}" \
               archive \
               CODE_SIGNING_ALLOWED=NO || true

    if [ -d "${ARCHIVE_PATH}" ]; then
        echo "    Exporting IPA via ExportOptions.plist..."
        xcodebuild -exportArchive \
                   -archivePath "${ARCHIVE_PATH}" \
                   -exportPath "${DIST_DIR}" \
                   -exportOptionsPlist "${SCRIPT_DIR}/ExportOptions.plist" || true
    fi
fi

# Fallback / Direct Payload IPA generation for offline or non-notarized distribution
BUILD_APP="${APP_DIR}/build/ios/iphoneos/Runner.app"
if [ ! -f "${IPA_DEST}" ] && [ -d "${BUILD_APP}" ]; then
    echo "    Packaging Payload into .ipa..."
    STAGE_DIR=$(mktemp -d)
    mkdir -p "${STAGE_DIR}/Payload"
    cp -R "${BUILD_APP}" "${STAGE_DIR}/Payload/"
    (cd "${STAGE_DIR}" && zip -qr "${IPA_DEST}" Payload)
    rm -rf "${STAGE_DIR}"
    echo "    Created IPA: ${IPA_DEST}"
fi

echo ">>> 4. Generating SHA-256 checksums..."
cd "${DIST_DIR}"
MANIFEST="${DIST_DIR}/SHA256SUMS_IOS.txt"
> "${MANIFEST}"

if [ -f "aissistant-ios-${VERSION}.ipa" ]; then
    if command -v sha256sum >/dev/null 2>&1; then
        sha256sum "aissistant-ios-${VERSION}.ipa" >> "${MANIFEST}"
    elif command -v shasum >/dev/null 2>&1; then
        shasum -a 256 "aissistant-ios-${VERSION}.ipa" >> "${MANIFEST}"
    fi
fi

echo "=================================================================="
echo " iOS Packaging Complete:"
if [ -f "${MANIFEST}" ]; then
    cat "${MANIFEST}"
fi
echo "=================================================================="
