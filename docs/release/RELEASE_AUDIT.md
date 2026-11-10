# ZenOmen Release & Distribution Engineering Audit

- **Date:** 2026-11-10
- **Status:** Phase 0 Completed
- **Scope:** Branding asset pipeline, platform code separation, Windows/Linux packaging, installer lifecycle, and CI/CD distribution.

---

## 1. Executive Summary

ZenOmen is a native Rust + Slint application providing hardware telemetry and control for HP OMEN and Victus laptops on both Windows and Linux.
Following the successful validation and ratification of Windows ACPI fan control and initial thermal profile interfaces on HP Victus motherboard `8A3D` (BIOS `F.22`), distribution engineering is required to prepare clean, production-grade releases.

This audit evaluates the existing repository structure, identifies defect sources in the logo pipeline, isolates cross-platform dependencies, details Windows and Linux packaging mechanisms, and defines the roadmap for production release automation.

---

## 2. Comprehensive Audit Findings

### 2.1 Authoritative Logo Source & Dimensions
- **Authoritative Vector Source:** `logo/zenomen-logo-v3-indigo-gold.svg` (mirrored at `data/icons/zenomen.svg`).
  - **Dimensions:** 1024×1024 viewBox (`viewBox="0 0 1024 1024" width="1024" height="1024"`).
  - **Visual Elements:** Indigo-gold palette, rounded rectangle border (`#frame`), dark indigo radial gradient background (`#bg`), serene open enso stroke (`#enso`), eyelid curves (`#lid`), iris (`#iris`), flame pupil (`#flameP`), and subtle blur filter (`#blur`).
  - **Integrity:** SHA-256 `116968784E01A1EC2257E546FA35D081A02CCDA17EE8183A0896F10ED89BB1B7` is verified. The original SVG is completely lossless and must never be altered or overwritten.

### 2.2 SVG References Across Codebase
1. `data/icons/zenomen.svg`: Primary icon source for packaging.
2. `data/icons/hicolor/scalable/apps/zenomen.svg`: Target FreeDesktop scalable icon for Linux desktop environments.
3. `Cargo.toml` (`[package.metadata.packager]`): References `data/icons/hicolor/scalable/apps/zenomen.svg`.
4. `tests/desktop_integration_tests.rs`: Tests presence and content of `data/icons/zenomen.svg`.
5. **Defect Found in UI:**
   - `ui/app.slint` (line 31): Declares `icon: @image-url("../data/icons/zenomen.png");` (referencing a downscaled 256×256 raster PNG instead of vector).
   - `ui/components/sidebar.slint` (line 103): Declares `source: @image-url("../../data/icons/zenomen.png");` scaled down to `36px × 36px` inside a `VerticalLayout`.

### 2.3 Asset Generation Scripts & Pipeline Defects
- **Existing Scripts:**
  - `scripts/generate-icons.sh`: Shell script responsible for generating hicolor PNG ladder (16, 24, 32, 48, 64, 128, 256, 512px) and Windows ICO.
  - `scripts/validate-icons.py`: Validates presence and binary dimensions of PNGs and ICO directory entries.
- **Root Cause of Logo Degradation:**
  1. **Chained Raster Downsampling:** In `scripts/generate-icons.sh` (lines 33–54), when Inkscape is not present in the environment, the script opens `data/icons/hicolor/512x512/apps/zenomen.png` (or 256×256) and resizes it with PIL Lanczos down to 16, 24, 32, 48, 64, 128, 256. This is generational loss—downsampling a bitmap repeatedly rather than rasterizing the vector SVG directly at the target pixel grid.
  2. **Multi-Res ICO Sub-sample Degradation:** Lines 65–75 open `hicolor/256x256/apps/zenomen.png` and invoke PIL's `img256.save(ico_path, format="ICO", sizes=ico_sizes)`. PIL generates the smaller mip levels (16, 24, 32, 48, 64, 128) by downsampling the 256px raster, producing blurry, anti-aliased mud on fine elements like the enso tip and flame pupil.
  3. **UI Bitmaps Instead of Vector:** In `ui/components/sidebar.slint`, Slint was instructed to render `zenomen.png` at 36px, forcing runtime bilinear/bicubic scaling of a 256px bitmap rather than rendering the crisp vector SVG directly through Slint's vector rendering engine.

### 2.4 SVG vs. Raster Component Capabilities
- **Components Supporting Native SVG:**
  - **Slint UI:** Slint with `renderer-femtovg` integrates vector rendering (`resvg`). Both window icon declarations and in-app graphics (`sidebar.slint`) support `.svg` directly, preserving crisp lines at any DPI scaling.
  - **Linux Desktop Environments:** GNOME, KDE Plasma, XFCE, and Wayland compositors support SVG icons via `/usr/share/icons/hicolor/scalable/apps/zenomen.svg`.
- **Components Requiring Raster Assets:**
  - **Windows PE Executable (`winres` in `build.rs`):** Requires Windows `.ico` format embedding multi-resolution layers (16, 24, 32, 48, 64, 128, 256).
  - **Windows Installer Wizards:** Inno Setup / WiX requires `.ico` for shortcut creation and `ARPPRODUCTICON`.
  - **Linux Fixed Icon Themes:** `/usr/share/icons/hicolor/<size>x<size>/apps/zenomen.png` requires exact dimensions (16, 24, 32, 48, 64, 128, 256, 512).
  - **Windows System Tray (`Shell_NotifyIconW`):** Requires Win32 `HICON` (16×16 at 100% scaling, 24×24 / 32×32 at high DPI).

### 2.5 Platform-Specific Code & Dependency Separation
- **Defects Discovered in Cargo Dependencies:**
  - `Cargo.toml`: `zbus = { version = "5", features = ["blocking"] }` was placed in top-level `[dependencies]`. As a result, Windows compiles Linux D-Bus types and dependencies unnecessarily.
- **Defects Discovered in Module Declarations:**
  - `src/desktop/mod.rs` unconditionally exports `single_instance`, `tray`, and `system_integration`.
  - `src/desktop/system_integration.rs` compiles Linux-only paths (`/usr/share/applications`, `/usr/lib/systemd/system`, `/usr/share/dbus-1`) into Windows binaries.
  - `src/service/types.rs`: `From<HardwareControlError> for zbus::fdo::Error` was compiled unconditionally without `#[cfg(target_os = "linux")]` or `#[cfg(unix)]`.

### 2.6 Windows Installer Assessment
- **Existing State:** WiX v4 file `packaging/windows/ZenOmen.wxs` compiles to `.msi`.
- **User Requirement:** Production Windows Setup Wizard producing `ZenOmen-Setup-x64.exe` with Inno Setup.
- **Missing Lifecycle Operations:**
  1. Welcome, GPL-3.0 License, Destination Folder pages.
  2. Component Selection (GUI vs Privileged Hardware Service).
  3. Optional PATH integration checkbox with clean add/remove logic in registry (`HKLM\SYSTEM\CurrentControlSet\Control\Session Manager\Environment`).
  4. SCM Service Lifecycle: Execute `zenomen-hw.exe --install-service` (or `sc.exe`) on install/upgrade; stop and execute `zenomen-hw.exe --uninstall-service` on uninstallation.
  5. Clean uninstallation removing executables, shortcuts, PATH entry, and registry keys, while preserving user configuration under `%APPDATA%\ZenOmen`.

### 2.7 Linux Packaging Assessment
- **Debian Package (`zenomen_<version>_amd64.deb`):**
  - Requires GUI in `/usr/bin/zenomen`, daemon in `/usr/bin/zenomen-hw`, desktop file in `/usr/share/applications`, systemd unit in `/usr/lib/systemd/system/zenomen-hw.service`, D-Bus policy in `/usr/share/dbus-1/system.d/`, polkit policy in `/usr/share/polkit-1/actions/`, and complete hicolor icon ladder.
  - Package scripts (`postinst`, `prerm`, `postrm`) must reload systemd (`systemctl daemon-reload`) without starting unsupported mutations automatically.
- **AppImage (`ZenOmen-<version>-x86_64.AppImage`):**
  - AppImage is a portable unprivileged user-space bundle.
  - It must **not** attempt to write to `/usr/lib/systemd` during ordinary user launch.
  - System integration of the privileged daemon must be separate and explicit (`--install-system-integration` / root helper).

### 2.8 CI/CD Workflow Separation
- **Existing Workflows:**
  - `.github/workflows/ci.yml`: Has `linux-quality` and `windows-quality` jobs.
  - `.github/workflows/release.yml`: Mixes `cargo packager` and WiX, generating `.msi` and `.zip`.
- **Target Separation Required:**
  - Strict job isolation: Linux runner builds ONLY `.deb` and `.AppImage`. Windows runner builds ONLY `ZenOmen-Setup-x64.exe`.
  - Zero cross-contamination: No Windows binaries in Linux packages; no Linux systemd/D-Bus files in Windows installer.
  - Deterministic artifact naming: `ZenOmen-Setup-x64.exe`, `zenomen_<version>_amd64.deb`, `ZenOmen-<version>-x86_64.AppImage`.

---

## 3. Action Plan by Phase

1. **Phase 1: Branding Asset Pipeline:**
   - Switch Slint UI (`sidebar.slint`, `app.slint`) to vector SVG (`data/icons/zenomen.svg`).
   - Create `scripts/branding/generate-branding.py` to independently rasterize the 1024×1024 vector source directly at every target resolution (16, 24, 32, 48, 64, 128, 256, 512, ICO).
   - Document pipeline in `docs/release/BRANDING_ASSETS.md`.
2. **Phase 2: Platform Code Separation:**
   - Move `zbus` in `Cargo.toml` to `[target.'cfg(target_os = "linux")'.dependencies]`.
   - Gate Linux-only modules (`system_integration`, Linux `single_instance`, Linux `tray`, `contract`, `polkit`, `backend`) behind `#[cfg(target_os = "linux")]`.
3. **Phase 3: Windows Inno Setup Installer:**
   - Create `packaging/windows/inno/ZenOmen.iss` configured for `ZenOmen-Setup-x64.exe`.
   - Wire service install/uninstall and PATH manipulation tasks.
4. **Phase 4: Linux Packaging:**
   - Validate Debian and AppImage definitions ensuring strict privilege separation.
5. **Phase 5: Executable & Environment Discovery:**
   - Document PATH handling and CLI flags for both OS targets.
6. **Phase 6: CI/CD Restructure:**
   - Refactor `.github/workflows/release.yml` for isolated Linux & Windows runners.
7. **Phase 7 & 8: Verification & Release Documentation:**
   - Execute verification tests and author comprehensive release guides.
