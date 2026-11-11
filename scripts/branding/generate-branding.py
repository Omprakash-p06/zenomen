#!/usr/bin/env python3
"""
scripts/branding/generate-branding.py

Authoritative asset generation pipeline for ZenOmen.
Renders all raster derivatives directly from the 1024x1024 master vector SVG
at exact target dimensions with zero lossy compression, preserving transparency
and avoiding repeated downsampling.
"""

import argparse
import io
import os
import shutil
import struct
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent.parent
DEFAULT_SOURCE_SVG = ROOT / "data" / "icons" / "zenomen.svg"
FALLBACK_SOURCE_SVG = ROOT / "logo" / "zenomen-logo-v3-indigo-gold.svg"

HICOLOR_SIZES = (16, 24, 32, 48, 64, 128, 256, 512)
ICO_SIZES = (16, 24, 32, 48, 64, 128, 256)


cargo_bin = Path.home() / ".cargo" / "bin"
if cargo_bin.exists() and str(cargo_bin) not in os.environ.get("PATH", ""):
    os.environ["PATH"] = f"{cargo_bin}:{os.environ.get('PATH', '')}"


def detect_vector_backend() -> str:
    """Detect available vector-to-raster backend (prioritizing engines that support gradients)."""
    if shutil.which("resvg"):
        return "resvg"
    if shutil.which("inkscape"):
        return "inkscape"
    try:
        import cairosvg  # noqa: F401
        return "cairosvg"
    except ImportError:
        pass
    try:
        import pymupdf  # noqa: F401
        return "pymupdf"
    except ImportError:
        pass
    try:
        import fitz  # noqa: F401
        return "pymupdf"
    except ImportError:
        pass
    return "none"


def render_svg_to_png(svg_path: Path, out_path: Path, size: int, backend: str) -> None:
    """
    Renders the vector SVG directly at the exact target dimensions (size x size).
    Preserves alpha channel and never downsamples from an existing bitmap.
    """
    out_path.parent.mkdir(parents=True, exist_ok=True)

    if backend == "inkscape":
        cmd = [
            "inkscape",
            str(svg_path),
            "--export-type=png",
            f"--export-filename={out_path}",
            f"--export-width={size}",
            f"--export-height={size}",
        ]
        res = subprocess.run(cmd, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
        if res.returncode != 0:
            raise RuntimeError(f"Inkscape failed rendering {size}x{size}: {res.stderr.decode()}")

    elif backend == "pymupdf":
        import pymupdf as fitz
        doc = fitz.open(str(svg_path))
        page = doc[0]
        # Calculate exact scale factor for width & height
        scale_x = float(size) / page.rect.width
        scale_y = float(size) / page.rect.height
        pix = page.get_pixmap(matrix=fitz.Matrix(scale_x, scale_y), alpha=True)
        if pix.width != size or pix.height != size:
            raise RuntimeError(f"PyMuPDF rendered unexpected dimension: {pix.width}x{pix.height}, expected {size}x{size}")
        pix.save(str(out_path))

    elif backend == "cairosvg":
        import cairosvg
        cairosvg.svg2png(
            url=str(svg_path),
            write_to=str(out_path),
            output_width=size,
            output_height=size,
        )

    elif backend == "resvg":
        cmd = [
            "resvg",
            "-w", str(size),
            "-h", str(size),
            str(svg_path),
            str(out_path),
        ]
        res = subprocess.run(cmd, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
        if res.returncode != 0:
            raise RuntimeError(f"resvg failed rendering {size}x{size}: {res.stderr.decode()}")

    else:
        raise RuntimeError(
            "No vector rendering engine found. Please install inkscape, pymupdf, cairosvg, or resvg."
        )


def build_multi_res_ico(ico_path: Path, png_entries: list[tuple[int, int, bytes]]) -> None:
    """
    Packages pre-rendered PNG bitmaps into a standard multi-resolution Windows ICO container.
    Each resolution was independently rendered from the vector SVG.
    """
    ico_path.parent.mkdir(parents=True, exist_ok=True)
    count = len(png_entries)
    # Header: Reserved (2 bytes = 0), Type (2 bytes = 1 for ICO), Count (2 bytes)
    header = struct.pack("<HHH", 0, 1, count)
    offset = 6 + count * 16

    dir_entries = []
    data_blobs = []

    for w, h, data in png_entries:
        size = len(data)
        dw = 0 if w == 256 else w
        dh = 0 if h == 256 else h
        # IconDirEntry: Width, Height, ColorCount (0), Reserved (0), Planes (1), Bpp (32), Size, Offset
        dir_entries.append(struct.pack("<BBBBHHII", dw, dh, 0, 0, 1, 32, size, offset))
        data_blobs.append(data)
        offset += size

    with open(ico_path, "wb") as f:
        f.write(header)
        for de in dir_entries:
            f.write(de)
        for blob in data_blobs:
            f.write(blob)


def verify_png_dimensions(path: Path, expected_size: int) -> None:
    """Verifies that the generated PNG matches expected dimensions."""
    with open(path, "rb") as f:
        header = f.read(24)
        if len(header) < 24 or header[:8] != b"\x89PNG\r\n\x1a\n":
            raise ValueError(f"{path} is not a valid PNG")
        w, h = struct.unpack(">II", header[16:24])
        if (w, h) != (expected_size, expected_size):
            raise ValueError(f"Expected {expected_size}x{expected_size}, got {w}x{h} in {path}")


def main() -> int:
    parser = argparse.ArgumentParser(description="Generate ZenOmen branding assets from vector SVG.")
    parser.add_argument("--source", type=Path, default=DEFAULT_SOURCE_SVG, help="Source master SVG")
    parser.add_argument("--force-backend", choices=["inkscape", "pymupdf", "cairosvg", "resvg"], help="Force backend")
    args = parser.parse_args()

    source_svg = args.source
    if not source_svg.exists():
        if FALLBACK_SOURCE_SVG.exists():
            source_svg = FALLBACK_SOURCE_SVG
        else:
            print(f"ERROR: Authoritative SVG not found at {source_svg}", file=sys.stderr)
            return 1

    content = source_svg.read_bytes()[:512]
    if b"<svg" not in content:
        print(f"ERROR: File at {source_svg} is not a valid SVG document", file=sys.stderr)
        return 1

    backend = args.force_backend or detect_vector_backend()
    if backend == "none":
        print("ERROR: No vector rendering engine available (install inkscape or pymupdf).", file=sys.stderr)
        return 1

    print(f"==> Source SVG: {source_svg.relative_to(ROOT)}")
    print(f"==> Vector Backend: {backend}")

    # 1. Update FreeDesktop scalable SVG
    scalable_target = ROOT / "data" / "icons" / "hicolor" / "scalable" / "apps" / "zenomen.svg"
    scalable_target.parent.mkdir(parents=True, exist_ok=True)
    if source_svg.resolve() != scalable_target.resolve():
        shutil.copy2(source_svg, scalable_target)
        print(f"  [OK] Updated scalable SVG: {scalable_target.relative_to(ROOT)}")

    # 2. Render each Linux hicolor raster icon directly from vector
    print("==> Rendering Linux hicolor PNG ladder from vector...")
    for s in HICOLOR_SIZES:
        out_png = ROOT / "data" / "icons" / "hicolor" / f"{s}x{s}" / "apps" / "zenomen.png"
        render_svg_to_png(source_svg, out_png, s, backend)
        verify_png_dimensions(out_png, s)
        print(f"  [OK] Rendered {s}x{s} PNG -> {out_png.relative_to(ROOT)}")

    # 3. Render root application 256x256 PNG
    root_png = ROOT / "data" / "icons" / "zenomen.png"
    render_svg_to_png(source_svg, root_png, 256, backend)
    verify_png_dimensions(root_png, 256)
    print(f"  [OK] Rendered root 256x256 PNG -> {root_png.relative_to(ROOT)}")

    # 3b. Render logo directory PNG
    logo_dir = ROOT / "logo"
    if logo_dir.exists():
        logo_png = logo_dir / "zenomen-logo-v3-indigo-gold.png"
        render_svg_to_png(source_svg, logo_png, 256, backend)
        verify_png_dimensions(logo_png, 256)
        print(f"  [OK] Rendered logo PNG -> {logo_png.relative_to(ROOT)}")

    # 4. Generate multi-resolution Windows ICO
    # Render each resolution independently from vector
    print("==> Generating multi-resolution Windows ICO from vector...")
    ico_png_entries = []
    for s in ICO_SIZES:
        temp_buf = io.BytesIO()
        temp_png = ROOT / "data" / "icons" / "hicolor" / f"{s}x{s}" / "apps" / "zenomen.png"
        if not temp_png.exists():
            render_svg_to_png(source_svg, temp_png, s, backend)
        ico_png_entries.append((s, s, temp_png.read_bytes()))

    ico_path = ROOT / "data" / "icons" / "zenomen.ico"
    build_multi_res_ico(ico_path, ico_png_entries)
    print(f"  [OK] Built multi-resolution ICO with {len(ico_png_entries)} layers -> {ico_path.relative_to(ROOT)}")

    print("\n==> Validating generated assets...")
    validate_script = ROOT / "scripts" / "validate-icons.py"
    if validate_script.exists():
        res = subprocess.run([sys.executable, str(validate_script)])
        if res.returncode != 0:
            print("ERROR: validate-icons.py reported failures", file=sys.stderr)
            return 1

    print("\nSUCCESS: All branding assets generated and verified directly from master vector SVG.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
