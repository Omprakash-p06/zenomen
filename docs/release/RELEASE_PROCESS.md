# ZenOmen Production Release Process

This document defines the standardized release engineering workflow, verification gates, and CI/CD build procedures for ZenOmen on Windows and Linux.

---

## 1. Release Architecture Overview

ZenOmen follows semantic versioning (`vMAJOR.MINOR.PATCH`). Production releases are built automatically via GitHub Actions upon pushing a version tag matching `v*`.

The pipeline enforces strict platform isolation:
- **Linux Release Job (`ubuntu-latest`):** Compiles Linux binaries, packages `zenomen_<version>_amd64.deb` and `ZenOmen-<version>-x86_64.AppImage`. It never contains Windows binaries or registry scripts.
- **Windows Release Job (`windows-latest`):** Compiles Windows binaries, packages `ZenOmen-Setup-x64.exe` (Inno Setup), `ZenOmen-x64.msi` (WiX Toolset), and `ZenOmen-windows-x64.zip`. It never contains Linux systemd units or D-Bus configurations.
- **Consolidation Job (`ubuntu-latest`):** Verifies and aggregates SHA-256 checksums from both platforms into a unified `SHA256SUMS` manifest and publishes the release.

---

## 2. Pre-Release Checklist

Before creating a release tag, maintainers must verify the following:

### 2.1 Version Synchronicity
Ensure the target version is identical in:
- `Cargo.toml` (`version = "x.y.z"`)
- `packaging/windows/inno/ZenOmen.iss` (`#define MyAppVersion "x.y.z"`)
- `packaging/windows/ZenOmen.wxs` (`Version="x.y.z"`)

### 2.2 Asset Pipeline Verification
Ensure branding assets are generated from the master 1024×1024 vector SVG without generational downsampling:
```bash
python scripts/branding/generate-branding.py
python scripts/validate-icons.py
```

### 2.3 Local Verification Gates
Both local and CI gates must pass with zero errors and zero warnings:

```bash
# Code formatting check
cargo fmt --all -- --check

# Strict lint check
cargo clippy --all-targets --all-features -- -D warnings

# Full headless test suite
cargo test --all
```

---

## 3. Creating and Publishing a Release

### Step 1: Commit and Tag
```bash
# Create signed or annotated release tag
git tag -a v1.0.0 -m "Release v1.0.0"
git push origin v1.0.0
```

### Step 2: CI Automation Execution
Pushing the tag triggers `.github/workflows/release.yml`:
1. `release-linux` runs unit/integration tests with `xvfb-run`, runs `cargo packager` for Debian, and runs `packaging/linux/build-appimage.sh` for AppImage.
2. `release-windows` runs Windows tests, installs Inno Setup, compiles `ZenOmen-Setup-x64.exe`, packages `ZenOmen-x64.msi`, and generates portable `.zip`.
3. `publish-release` combines checksums and publishes the GitHub release.

---

## 4. Local Packaging Reference

For testing packaging without running CI:

### Windows Local Build
```powershell
# 1. Compile release binaries
cargo build --release --locked --bins

# 2. Build Inno Setup installer
powershell -ExecutionPolicy Bypass -File packaging\windows\build-installer.ps1

# 3. Output located at target\installer\ZenOmen-Setup-x64.exe
```

### Linux Local Build
```bash
# 1. Compile release binaries
cargo build --release --locked --bins

# 2. Build Debian package (.deb)
cargo packager --release --formats deb

# 3. Build standalone AppImage (.AppImage)
bash packaging/linux/build-appimage.sh

# 4. Outputs located in dist/
```
