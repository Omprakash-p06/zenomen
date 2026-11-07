#!/usr/bin/env bash
# scripts/generate-icons.sh
# Regenerate raster and platform icons from master SVG / high-resolution source.
# Does not redraw or modify master artwork in logo/.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"

SOURCE_SVG="${ROOT_DIR}/data/icons/zenomen.svg"
HICOLOR_DIR="${ROOT_DIR}/data/icons/hicolor"
SIZES=(16 24 32 48 64 128 256 512)

echo "==> Generating hicolor icon set in ${HICOLOR_DIR}..."

# Ensure scalable icon exists in hicolor
mkdir -p "${HICOLOR_DIR}/scalable/apps"
cp "${SOURCE_SVG}" "${HICOLOR_DIR}/scalable/apps/zenomen.svg"

# If inkscape is available, use it for direct vector rendering; otherwise use python3-PIL
if command -v inkscape >/dev/null 2>&1; then
    echo "Using inkscape for vector rasterization..."
    for size in "${SIZES[@]}"; do
        mkdir -p "${HICOLOR_DIR}/${size}x${size}/apps"
        inkscape "${SOURCE_SVG}" \
            --export-type=png \
            --export-filename="${HICOLOR_DIR}/${size}x${size}/apps/zenomen.png" \
            --export-width="${size}" \
            --export-height="${size}"
    done
elif command -v python3 >/dev/null 2>&1; then
    echo "Using python3 PIL with LANCZOS resampling from high-res asset..."
    python3 - <<EOF
from PIL import Image
import os

root = "${ROOT_DIR}"
sizes = [16, 24, 32, 48, 64, 128, 256, 512]
source_highres = os.path.join(root, "data/icons/hicolor/512x512/apps/zenomen.png")
if not os.path.exists(source_highres):
    source_highres = os.path.join(root, "data/icons/zenomen.png")

img = Image.open(source_highres)
for s in sizes:
    out_dir = os.path.join(root, f"data/icons/hicolor/{s}x{s}/apps")
    os.makedirs(out_dir, exist_ok=True)
    out_path = os.path.join(out_dir, "zenomen.png")
    if s == img.width and s == img.height:
        img.save(out_path)
    else:
        resized = img.resize((s, s), Image.Resampling.LANCZOS)
        resized.save(out_path)
print("PNG generation complete.")
EOF
else
    echo "Error: neither inkscape nor python3 is available." >&2
    exit 1
fi

# Ensure 256x256 app icon is in place
cp "${HICOLOR_DIR}/256x256/apps/zenomen.png" "${ROOT_DIR}/data/icons/zenomen.png"

# Generate multi-resolution Windows ICO
echo "==> Generating Windows multi-resolution ICO at data/icons/zenomen.ico..."
python3 - <<EOF
from PIL import Image
import os

root = "${ROOT_DIR}"
img256 = Image.open(os.path.join(root, "data/icons/hicolor/256x256/apps/zenomen.png"))
ico_sizes = [(16, 16), (24, 24), (32, 32), (48, 48), (64, 64), (128, 128), (256, 256)]
ico_path = os.path.join(root, "data/icons/zenomen.ico")
img256.save(ico_path, format="ICO", sizes=ico_sizes)
print("ICO generated successfully with sizes:", ico_sizes)
EOF

echo "==> Validating generated icon assets..."
python3 "${SCRIPT_DIR}/validate-icons.py"
