#!/usr/bin/env bash
# scripts/generate-icons.sh
# Regenerate raster and platform icons from master vector SVG without lossy degradation.
# Authoritative pipeline: scripts/branding/generate-branding.py
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"

if command -v python3 >/dev/null 2>&1; then
    exec python3 "${ROOT_DIR}/scripts/branding/generate-branding.py"
elif command -v python >/dev/null 2>&1; then
    exec python "${ROOT_DIR}/scripts/branding/generate-branding.py"
elif command -v inkscape >/dev/null 2>&1; then
    SOURCE_SVG="${ROOT_DIR}/data/icons/zenomen.svg"
    HICOLOR_DIR="${ROOT_DIR}/data/icons/hicolor"
    SIZES=(16 24 32 48 64 128 256 512)

    echo "==> Generating hicolor icon set with inkscape vector rendering..."
    mkdir -p "${HICOLOR_DIR}/scalable/apps"
    cp "${SOURCE_SVG}" "${HICOLOR_DIR}/scalable/apps/zenomen.svg"

    for size in "${SIZES[@]}"; do
        mkdir -p "${HICOLOR_DIR}/${size}x${size}/apps"
        inkscape "${SOURCE_SVG}" \
            --export-type=png \
            --export-filename="${HICOLOR_DIR}/${size}x${size}/apps/zenomen.png" \
            --export-width="${size}" \
            --export-height="${size}"
    done
    cp "${HICOLOR_DIR}/256x256/apps/zenomen.png" "${ROOT_DIR}/data/icons/zenomen.png"
    echo "==> Validating generated icon assets..."
    python3 "${SCRIPT_DIR}/validate-icons.py" || true
else
    echo "Error: Python or Inkscape is required to generate vector branding assets." >&2
    exit 1
fi
