#!/usr/bin/env bash
# ==============================================================================
# Universal AI Assistant - Android Release Packaging Pipeline (APK & AAB)
# Subphase: S11.P1.2
# ==============================================================================
set -euo pipefail

VERSION="${1:-1.0.0}"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/../../.." && pwd)"
DIST_DIR="${REPO_ROOT}/dist"
APP_DIR="${REPO_ROOT}/app"
JNILIBS_DIR="${APP_DIR}/android/app/src/main/jniLibs"

echo "=================================================================="
echo " Packaging Universal AI Assistant for Android v${VERSION}"
echo "=================================================================="
mkdir -p "${DIST_DIR}"

# ABI mappings for Rust and Android NDK
declare -A TARGET_MAP=(
    ["arm64-v8a"]="aarch64-linux-android"
    ["armeabi-v7a"]="armv7-linux-androideabi"
    ["x86_64"]="x86_64-linux-android"
)

echo ">>> 1. Compiling Rust C-ABI (aissistant-ffi) for Android targets..."
for ABI in "${!TARGET_MAP[@]}"; do
    RUST_TARGET="${TARGET_MAP[$ABI]}"
    ABI_OUT_DIR="${JNILIBS_DIR}/${ABI}"
    mkdir -p "${ABI_OUT_DIR}"

    echo "    Building target: ${RUST_TARGET} (${ABI})..."
    if command -v cargo-ndk >/dev/null 2>&1; then
        cargo ndk -t "${ABI}" --manifest-path "${REPO_ROOT}/core/Cargo.toml" build -p aissistant-ffi --release
        SO_PATH="${REPO_ROOT}/core/target/${RUST_TARGET}/release/libaissistant_ffi.so"
    else
        cargo build --manifest-path "${REPO_ROOT}/core/Cargo.toml" -p aissistant-ffi --release --target "${RUST_TARGET}" || true
        SO_PATH="${REPO_ROOT}/core/target/${RUST_TARGET}/release/libaissistant_ffi.so"
    fi

    if [ -f "${SO_PATH}" ]; then
        cp "${SO_PATH}" "${ABI_OUT_DIR}/libaissistant_ffi.so"
        echo "    Copied ${SO_PATH} -> ${ABI_OUT_DIR}/libaissistant_ffi.so"
    else
        echo "    Note: Pre-built or host cross-compilation libaissistant_ffi.so not found for ${RUST_TARGET}."
        echo "    Ensure NDK toolchain is configured in CI or development environment."
    fi
done

echo ">>> 2. Building Flutter Android APK (Universal Release)..."
cd "${APP_DIR}"
flutter build apk --release --build-name="${VERSION}" --build-number=1

APK_SRC="${APP_DIR}/build/app/outputs/flutter-apk/app-release.apk"
APK_DEST="${DIST_DIR}/aissistant-android-${VERSION}.apk"
if [ -f "${APK_SRC}" ]; then
    cp "${APK_SRC}" "${APK_DEST}"
    echo "    Created APK: ${APK_DEST}"
fi

echo ">>> 3. Building Flutter Android App Bundle (AAB)..."
flutter build appbundle --release --build-name="${VERSION}" --build-number=1

AAB_SRC="${APP_DIR}/build/app/outputs/bundle/release/app-release.aab"
AAB_DEST="${DIST_DIR}/aissistant-android-${VERSION}.aab"
if [ -f "${AAB_SRC}" ]; then
    cp "${AAB_SRC}" "${AAB_DEST}"
    echo "    Created AAB: ${AAB_DEST}"
fi

echo ">>> 4. Generating SHA-256 checksums..."
cd "${DIST_DIR}"
MANIFEST="${DIST_DIR}/SHA256SUMS_ANDROID.txt"
> "${MANIFEST}"

for FILE in "aissistant-android-${VERSION}.apk" "aissistant-android-${VERSION}.aab"; do
    if [ -f "${FILE}" ]; then
        if command -v sha256sum >/dev/null 2>&1; then
            sha256sum "${FILE}" >> "${MANIFEST}"
        elif command -v shasum >/dev/null 2>&1; then
            shasum -a 256 "${FILE}" >> "${MANIFEST}"
        fi
    fi
done

echo "=================================================================="
echo " Android Packaging Complete:"
if [ -f "${MANIFEST}" ]; then
    cat "${MANIFEST}"
fi
echo "=================================================================="
