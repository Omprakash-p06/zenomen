#!/usr/bin/env python3
"""
scripts/validate-icons.py
Validates the completeness, format, dimensions, and multi-resolution ICO entries
for ZenOmen platform and desktop icon assets.
"""

import os
import struct
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
HICOLOR = ROOT / "data" / "icons" / "hicolor"
SIZES = (16, 24, 32, 48, 64, 128, 256, 512)
ICO_MIN_ENTRIES = 6  # Expecting at least 16, 24, 32, 48, 64, 128, 256 (7 total)


def png_size(path: Path) -> tuple[int, int]:
    """Read the width and height directly from PNG IHDR chunk header."""
    with open(path, "rb") as f:
        header = f.read(24)
        if len(header) < 24 or header[:8] != b"\x89PNG\r\n\x1a\n":
            raise ValueError(f"{path} is not a valid PNG file")
        # IHDR chunk starts at byte 12; width at 16, height at 20
        width, height = struct.unpack(">II", header[16:24])
        return width, height


def inspect_ico_entries(path: Path) -> list[tuple[int, int, int]]:
    """Inspect ICO header and directory entries (width, height, bpp)."""
    entries = []
    with open(path, "rb") as f:
        header = f.read(6)
        if len(header) < 6:
            return entries
        reserved, ico_type, count = struct.unpack("<HHH", header)
        if reserved != 0 or ico_type != 1:
            raise ValueError(f"{path} is not a valid Windows ICO file")

        for _ in range(count):
            entry_bytes = f.read(16)
            if len(entry_bytes) < 16:
                break
            w, h, colors, res, planes, bpp, size, offset = struct.unpack(
                "<BBBBHHII", entry_bytes
            )
            # 0 width/height in ICO directory represents 256px
            actual_w = 256 if w == 0 else w
            actual_h = 256 if h == 0 else h
            entries.append((actual_w, actual_h, bpp))
    return entries


def main() -> int:
    failures: list[str] = []

    # 1. Validate Linux hicolor PNG assets
    for size in SIZES:
        png_path = HICOLOR / f"{size}x{size}" / "apps" / "zenomen.png"
        if not png_path.exists():
            failures.append(f"Missing hicolor icon: {png_path.relative_to(ROOT)}")
            continue

        try:
            w, h = png_size(png_path)
            if (w, h) != (size, size):
                failures.append(
                    f"Dimension mismatch in {png_path.relative_to(ROOT)}: found {w}x{h}, expected {size}x{size}"
                )
        except Exception as e:
            failures.append(f"Corrupt PNG {png_path.relative_to(ROOT)}: {e}")

    # 2. Validate Linux scalable SVG icon
    svg_path = HICOLOR / "scalable" / "apps" / "zenomen.svg"
    if not svg_path.exists():
        failures.append(f"Missing scalable SVG: {svg_path.relative_to(ROOT)}")
    else:
        try:
            content = svg_path.read_bytes()[:512]
            if b"<svg" not in content:
                failures.append(f"Invalid SVG document: {svg_path.relative_to(ROOT)}")
        except Exception as e:
            failures.append(f"Failed reading SVG {svg_path.relative_to(ROOT)}: {e}")

    # 3. Validate root 256x256 zenomen.png
    root_png = ROOT / "data" / "icons" / "zenomen.png"
    if not root_png.exists():
        failures.append("Missing data/icons/zenomen.png")
    else:
        try:
            w, h = png_size(root_png)
            if (w, h) != (256, 256):
                failures.append(f"data/icons/zenomen.png is {w}x{h}, expected 256x256")
        except Exception as e:
            failures.append(f"Corrupt data/icons/zenomen.png: {e}")

    # 4. Validate Windows ICO asset
    ico_path = ROOT / "data" / "icons" / "zenomen.ico"
    if not ico_path.exists():
        failures.append("Missing data/icons/zenomen.ico")
    else:
        try:
            entries = inspect_ico_entries(ico_path)
            if len(entries) < ICO_MIN_ENTRIES:
                failures.append(
                    f"data/icons/zenomen.ico has {len(entries)} entries, expected >= {ICO_MIN_ENTRIES}"
                )
            else:
                entry_dims = {(w, h) for (w, h, _) in entries}
                required_ico_dims = {(16, 16), (24, 24), (32, 32), (48, 48), (64, 64), (128, 128), (256, 256)}
                missing = required_ico_dims - entry_dims
                if missing:
                    failures.append(f"data/icons/zenomen.ico is missing required sizes: {sorted(missing)}")
        except Exception as e:
            failures.append(f"Failed inspecting data/icons/zenomen.ico: {e}")

    if failures:
        print(f"FAILED: {len(failures)} icon validation errors found:", file=sys.stderr)
        for msg in failures:
            print(f"  - {msg}", file=sys.stderr)
        return 1

    print(
        f"SUCCESS: Verified all {len(SIZES)} hicolor PNGs, scalable SVG, and multi-res ICO ({len(entries)} entries)."
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
