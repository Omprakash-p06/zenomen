#!/usr/bin/env bash
# Build script for ZenOmen AppImage
# Generates a portable, unprivileged ZenOmen-<version>-x86_64.AppImage

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "${SCRIPT_DIR}/../.." && pwd)"

if [ -f "$HOME/.cargo/env" ]; then
    source "$HOME/.cargo/env"
fi

VERSION="$(sed -n 's/^version = "\(.*\)"/\1/p' "${PROJECT_ROOT}/Cargo.toml" | head -n1)"
if [ -z "${VERSION}" ]; then
    VERSION="1.0.0"
fi
TARGET_DIR="${PROJECT_ROOT}/target/release"
BUILD_DIR="${PROJECT_ROOT}/target/appimage"
APP_DIR="${BUILD_DIR}/AppDir"
OUTPUT_DIR="${PROJECT_ROOT}/dist"

echo "==> Building ZenOmen AppImage v${VERSION}..."

# 1. Verify release binaries exist
if [ ! -f "${TARGET_DIR}/zenomen" ] || [ ! -f "${TARGET_DIR}/zenomen-hw" ]; then
    echo "==> Compiling release binaries..."
    cargo build --release --bins --manifest-path "${PROJECT_ROOT}/Cargo.toml"
fi

# 2. Prepare AppDir structure
rm -rf "${APP_DIR}"
mkdir -p "${APP_DIR}/usr/bin"
mkdir -p "${APP_DIR}/usr/share/applications"
mkdir -p "${APP_DIR}/usr/share/icons/hicolor/scalable/apps"
mkdir -p "${APP_DIR}/usr/share/icons/hicolor/256x256/apps"
mkdir -p "${OUTPUT_DIR}"

# 3. Copy binaries
cp -a "${TARGET_DIR}/zenomen" "${APP_DIR}/usr/bin/zenomen"
cp -a "${TARGET_DIR}/zenomen-hw" "${APP_DIR}/usr/bin/zenomen-hw"
chmod 755 "${APP_DIR}/usr/bin/zenomen" "${APP_DIR}/usr/bin/zenomen-hw"

# 4. Copy desktop integration & branding assets
cp -a "${PROJECT_ROOT}/data/applications/zenomen.desktop" "${APP_DIR}/zenomen.desktop"
cp -a "${PROJECT_ROOT}/data/applications/zenomen.desktop" "${APP_DIR}/usr/share/applications/zenomen.desktop"

# Vector icon for scalable theme
cp -a "${PROJECT_ROOT}/data/icons/zenomen.svg" "${APP_DIR}/zenomen.svg"
cp -a "${PROJECT_ROOT}/data/icons/zenomen.svg" "${APP_DIR}/usr/share/icons/hicolor/scalable/apps/zenomen.svg"

# Raster icons for legacy theme support
if [ -f "${PROJECT_ROOT}/data/icons/hicolor/256x256/apps/zenomen.png" ]; then
    cp -a "${PROJECT_ROOT}/data/icons/hicolor/256x256/apps/zenomen.png" "${APP_DIR}/zenomen.png"
    cp -a "${PROJECT_ROOT}/data/icons/hicolor/256x256/apps/zenomen.png" "${APP_DIR}/usr/share/icons/hicolor/256x256/apps/zenomen.png"
fi

# Copy all available hicolor sizes if present
for size_dir in "${PROJECT_ROOT}/data/icons/hicolor/"*x*; do
    if [ -d "${size_dir}" ]; then
        size_name="$(basename "${size_dir}")"
        mkdir -p "${APP_DIR}/usr/share/icons/hicolor/${size_name}/apps"
        if [ -f "${size_dir}/apps/zenomen.png" ]; then
            cp -a "${size_dir}/apps/zenomen.png" "${APP_DIR}/usr/share/icons/hicolor/${size_name}/apps/zenomen.png"
        fi
    fi
done

# 5. Copy launcher AppRun
cp -a "${SCRIPT_DIR}/AppRun" "${APP_DIR}/AppRun"
chmod 755 "${APP_DIR}/AppRun"

# 6. Locate or acquire appimagetool
APPIMAGETOOL=""
if command -v appimagetool >/dev/null 2>&1; then
    APPIMAGETOOL="appimagetool"
elif [ -f "${BUILD_DIR}/appimagetool-x86_64.AppImage" ]; then
    APPIMAGETOOL="${BUILD_DIR}/appimagetool-x86_64.AppImage"
else
    echo "==> Downloading appimagetool..."
    mkdir -p "${BUILD_DIR}"
    APPIMAGETOOL_URL="https://github.com/AppImage/appimagetool/releases/download/continuous/appimagetool-x86_64.AppImage"
    curl -sSL -o "${BUILD_DIR}/appimagetool-x86_64.AppImage" "${APPIMAGETOOL_URL}"
    chmod +x "${BUILD_DIR}/appimagetool-x86_64.AppImage"
    APPIMAGETOOL="${BUILD_DIR}/appimagetool-x86_64.AppImage"
fi

# 7. Package AppImage
OUTPUT_FILE="${OUTPUT_DIR}/ZenOmen-${VERSION}-x86_64.AppImage"
echo "==> Generating ${OUTPUT_FILE}..."
APPIMAGETOOL_FLAGS=()
if [[ "${APPIMAGETOOL}" == *.AppImage ]]; then
    APPIMAGETOOL_FLAGS+=("--appimage-extract-and-run")
fi
ARCH=x86_64 "${APPIMAGETOOL}" "${APPIMAGETOOL_FLAGS[@]}" "${APP_DIR}" "${OUTPUT_FILE}"

echo "==> AppImage created successfully: ${OUTPUT_FILE}"
