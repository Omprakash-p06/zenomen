# ZenOmen Branding & Asset Distribution Specification

- **Status:** Ratified (Phase 1)
- **Scope:** Authoritative vector assets, direct SVG UI rendering, exact-dimension rasterization, and multi-resolution platform packaging.

---

## 1. Authoritative Vector Asset

ZenOmen's visual identity centers on the serene enso eyeball motif in an indigo-gold palette:
- **Master Vector File:** `logo/zenomen-logo-v3-indigo-gold.svg` (mirrored at `data/icons/zenomen.svg`).
- **Canvas / Dimensions:** 1024×1024 vector viewBox (`viewBox="0 0 1024 1024"`).
- **Integrity Rule:** The master SVG is the single immutable source of truth. Generation scripts must **never** modify, overwrite, or simplify the master SVG.

---

## 2. Rendering Strategy: Vector vs. Raster

| Location | Rendering Engine | Asset Used | Resolution / Behavior |
| :--- | :--- | :--- | :--- |
| **Slint UI Sidebar** | Slint `renderer-femtovg` (`resvg`) | `data/icons/zenomen.svg` | Vector rendered dynamically at UI display size (36×36px) |
| **Slint Window Icon** | Slint runtime | `data/icons/zenomen.svg` | Vector rendered at window titlebar scale |
| **Linux Desktop Scalable** | FreeDesktop Icon Theme | `data/icons/hicolor/scalable/apps/zenomen.svg` | Scalable vector for Wayland/X11 app switchers |
| **Linux Hicolor Ladder** | Desktop Shell / Dock | `data/icons/hicolor/<size>x<size>/apps/zenomen.png` | Pre-rendered lossless PNGs: 16, 24, 32, 48, 64, 128, 256, 512 |
| **Windows Executable (PE)** | `winres` / Win32 Shell | `data/icons/zenomen.ico` | Multi-resolution container embedding 16, 24, 32, 48, 64, 128, 256 |
| **Windows Installer** | Inno Setup / WiX | `data/icons/zenomen.ico` | Embedded icon for Setup wizard, Control Panel, and Shortcuts |

---

## 3. Prevention of Logo Degradation

Previous asset pipelines exhibited blurring due to **iterative bitmap resampling**:
1. An initial bitmap (e.g., 512px or 256px) was generated.
2. Smaller sizes (128, 64, 48, 32, 24, 16) were downscaled from the previous raster image.
3. Multi-resolution ICOs downscaled from 256px using low-quality default filters.
4. The Slint UI scaled down a 256px raster instead of rendering vector.

### Enforced Pipeline Rules:
- **Zero Generational Downsampling:** Every raster asset is rendered directly from the 1024×1024 master vector SVG.
- **Independent ICO Resolution Layers:** Each resolution inside `zenomen.ico` (16, 24, 32, 48, 64, 128, 256) is rendered directly from vector before packaging into the ICO container.
- **Full Transparency & Lossless PNG:** Alpha channel (RGBA) is strictly preserved; lossy quantization or compression is prohibited.
- **Native Vector in UI:** Slint components reference `.svg` directly via `@image-url(...)`.

---

## 4. Reproducible Generation Procedure

The authoritative pipeline is implemented in [`scripts/branding/generate-branding.py`](file:///C:/Users/OM%20Prakash/Documents/zenomen/scripts/branding/generate-branding.py).

### Prerequisites:
Any of the following vector rendering backends:
- **Inkscape:** `inkscape` in system PATH (standard in Linux build runners).
- **PyMuPDF:** `python -m pip install pymupdf` (standard in Windows/Python environments).
- **CairoSVG:** `python -m pip install cairosvg`.
- **resvg:** `resvg` CLI binary.

### Execution:
To regenerate all raster assets and validate them:
```bash
# On Linux / macOS:
./scripts/generate-icons.sh

# Or directly via Python (cross-platform):
python scripts/branding/generate-branding.py
```

### Validation:
The pipeline automatically invokes [`scripts/validate-icons.py`](file:///C:/Users/OM%20Prakash/Documents/zenomen/scripts/validate-icons.py) to assert:
- Exact pixel dimensions of all 8 hicolor PNGs.
- Valid SVG structure for the scalable icon.
- Correct directory entries (16, 24, 32, 48, 64, 128, 256px) in `zenomen.ico`.
