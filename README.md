<p align="center">
  <img src="logo/zenomen-logo-v3-indigo-gold.svg" alt="ZenOmen Logo" width="128" height="128">
</p>

<h1 align="center">ZenOmen</h1>

<p align="center">
  <strong>A lightweight, native hardware control center for HP OMEN and Victus laptops on Linux and Windows.</strong>
</p>

<p align="center">
  <a href="https://github.com/Omprakash-p06/zenomen/releases"><img src="https://img.shields.io/github/v/tag/Omprakash-p06/zenomen?sort=semver&label=version" alt="Latest Version"></a>
  <a href="https://github.com/Omprakash-p06/zenomen/releases/latest"><img src="https://img.shields.io/github/v/release/Omprakash-p06/zenomen?label=release" alt="Latest Release"></a>
  <a href="https://github.com/Omprakash-p06/zenomen/actions/workflows/release.yml"><img src="https://github.com/Omprakash-p06/zenomen/actions/workflows/release.yml/badge.svg" alt="Release Build Status"></a>
  <a href="LICENSE"><img src="https://img.shields.io/badge/License-GPLv3-green.svg" alt="License: GPL v3"></a>
</p>

---

## Overview

ZenOmen is written from scratch in Rust and Slint to deliver a fast, responsive, and low-overhead control center. It replaces legacy command-line scripts with a secure, unprivileged desktop GUI and a privileged hardware service:

- **Hardware Telemetry**: Live CPU, GPU, and RAM utilization, package temperatures, fan speeds (RPM), and a configurable telemetry history window (30 s, 60 s, 120 s, or 300 s; default 120 s).
- **Fan Controls**: Switching between automatic fan management and max cooling mode, with read-back verification through sysfs (`pwm1_enable`) on Linux and HP WMI on Windows.
- **Power Profiles**: Kernel power performance profiles (`quiet`, `balanced`, `performance`) on Linux, and HP thermal/power profiles on supported Windows boards.
- **Coordinated Presets**: One-click combined system profiles (`Quiet`, `Balanced`, `Gaming`) that coordinate display refresh rate (Windows), power profile, and cooling fans, reporting partial success honestly.
- **Keyboard RGB**: Lighting control with kernel-patch sysfs, legacy sysfs, and 4-zone support, verified hardware write cycles, and built-in presets on Linux. Windows RGB has no verified firmware contract yet and is refused rather than guessed.
- **Diagnostics**: One-click hardware capability auditing and JSON diagnostic reports exported from the **Diagnostics** tab.
- **Desktop Integration**: Freedesktop StatusNotifierItem tray and XDG autostart on Linux; Win32 tray, HKCU autostart, and single-instance mutex on Windows. Close-to-tray lifecycle on both.
- **Privileged Hardware Service (`zenomen-hw`)**: Headless systemd service with Polkit authorization (`com.zenomen.hardware.control`) on Linux, and an SCM service (`ZenOmenHardwareService`) reached over a secured named pipe on Windows — no GUI code ever runs as root/System.
- **Honest Telemetry**: A sensor that cannot be read renders as `Unavailable`; ZenOmen never displays a fabricated zero or a guessed profile.

---

## Documentation

The authoritative engineering reference is the codebase map in `.planning/codebase/`:

| Document | Covers |
|---|---|
| [`.planning/codebase/STACK.md`](.planning/codebase/STACK.md) | Languages, dependencies, build profiles, configuration, packaging metadata, CI gates |
| [`.planning/codebase/INTEGRATIONS.md`](.planning/codebase/INTEGRATIONS.md) | Kernel interfaces, D-Bus, Polkit, HP WMI opcodes, named pipe, SCM, platform capability matrix |
| [`.planning/codebase/ARCHITECTURE.md`](.planning/codebase/ARCHITECTURE.md) | Process model, threading, data flows, key abstractions, architectural invariants |
| [`.planning/codebase/STRUCTURE.md`](.planning/codebase/STRUCTURE.md) | Directory layout, module boundaries, key file locations, where to add new code |
| [`.planning/codebase/CONVENTIONS.md`](.planning/codebase/CONVENTIONS.md) | Naming, error handling, logging, module layering, mock hardware conventions |
| [`.planning/codebase/TESTING.md`](.planning/codebase/TESTING.md) | Test framework, suite inventory, hardware doubles, coverage gaps |
| [`.planning/codebase/CONCERNS.md`](.planning/codebase/CONCERNS.md) | Technical debt, known defects, security notes, fragile areas, missing features |

Long-form material lives in `docs/architecture/` (e.g. `HP_WMI_PROTOCOL.md`, `IPC_SECURITY.md`), `docs/decisions/` (ADR-001…ADR-004), `docs/release/` (installation, troubleshooting, release process), and `docs/testing/` (`HP_BOARD_VALIDATION.md`, `WINDOWS_TEST_MATRIX.md`).

### Project Structure

```
src/main.rs             Unprivileged GUI entry point
src/bin/zenomen-hw.rs   Privileged hardware service entry point
src/core/               Typed AppCommand/AppEvent contract, AppState, worker, telemetry, diagnostics
src/hardware/           Read-only telemetry: provider trait, sensor registry, linux/, windows/
src/service/            Privileged control: Linux D-Bus/Polkit/sysfs, windows/ pipe + HP WMI driver
src/desktop/            Tray, single instance, autostart, system integration installers
src/config/  src/rgb/   Configuration persistence and RGB profiles/presets
src/app.rs  ui/         Slint bridge and the live declarative UI
tests/                  19 integration suites
data/  packaging/       Installed system assets and DEB/AppImage/EXE/MSI packaging
```

`src/ui/` contains a dormant GPUI-CE view-model scaffold (ADR-002); it is compiled but not used by either binary.

---

## Security Architecture & How Hardware Control Works

Unlike standalone CLI scripts that prompt for `sudo` directly in the terminal, ZenOmen enforces a strict security separation:

```
┌──────────────────────────────────────┐
│  zenomen (Unprivileged GUI Process)  │
│  - Runs as regular user              │
│  - Zero raw kernel / firmware writes │
│  - Zero network access               │
└──────────────────┬───────────────────┘
                   │ Linux:  system D-Bus (com.zenomen.Hardware1)
                   │         + Polkit (com.zenomen.hardware.control)
                   │ Windows: named pipe \\.\pipe\ZenOmen.Hardware.v1
                   │         + interactive-session check
┌──────────────────▼───────────────────┐
│  zenomen-hw (Privileged Service)     │
│  - systemd (root) / SCM (LocalSystem)│
│  - Verified read-back after writes   │
│  - Board validation before firmware  │
└──────────────────────────────────────┘
```

- **Linux**: every mutating D-Bus method verifies the caller subject against the Polkit action `com.zenomen.hardware.control`. Writes go through `write_and_verify` (bounded retries with exponential backoff) and the confirmed value is read back before success is reported.
- **Windows**: the GUI never opens a firmware handle. `zenomen-hw` serves the pipe with a DACL restricted to SYSTEM, Administrators, and Interactive Users, rejects callers outside the active console session, and checks the detected motherboard plus BIOS against a physical-validation record before any fan, power, preset, or RGB write.
- **Cross-platform**: an unrecognized board or an unvalidated BIOS exposes read-only telemetry and refuses mutations. Missing interfaces are reported as `Unavailable`, never simulated.

When you launch `zenomen`, it communicates with `zenomen-hw`. If the service is not running, the GUI operates in read-only telemetry mode and explains how to start the service.

---

## Installation

Pre-built packages are published on the [GitHub Releases page](https://github.com/Omprakash-p06/zenomen/releases/latest).

### Cryptographic Verification

Always verify downloaded release assets against `SHA256SUMS`:

```bash
sha256sum -c SHA256SUMS
```

```powershell
Get-FileHash -Algorithm SHA256 .\ZenOmen-Setup-x64.exe
```

### Debian / Ubuntu / Linux Mint (`.deb`)

```bash
sudo dpkg -i zenomen_1.0.0_amd64.deb
sudo apt-get install -f
```

The package automatically installs the desktop entry, icons, D-Bus system bus configuration (`/usr/share/dbus-1/system.d/com.zenomen.Hardware1.conf`), Polkit policy (`/usr/share/polkit-1/actions/com.zenomen.hardware.policy`), and enables the `zenomen-hw.service` systemd daemon.

### AppImage

Make the AppImage executable and run it:

```bash
chmod +x ZenOmen-1.0.0-x86_64.AppImage
./ZenOmen-1.0.0-x86_64.AppImage
```

> [!NOTE]
> If your Linux distribution does not have FUSE 2 installed by default (e.g. Ubuntu 22.04+ / 24.04+), either run directly with:
> ```bash
> ./ZenOmen-1.0.0-x86_64.AppImage --appimage-extract-and-run
> ```
> or install the compatibility library: `sudo apt install libfuse2t64` (Ubuntu 24.04+) or `sudo apt install libfuse2` (Ubuntu 22.04 / Debian).

To enable hardware fan and power control under AppImage, register the system integration files once as root:

```bash
sudo ./ZenOmen-1.0.0-x86_64.AppImage --install-system-integration
```

### Windows 10/11 x64

Use one of the release artifacts:

- **`ZenOmen-Setup-x64.exe`** (Inno Setup) — installs the GUI and optionally the hardware service; the service is registered with `zenomen-hw.exe --install-service` and removed on uninstall.
- **`ZenOmen-x64.msi`** (WiX) — installs both binaries and registers `ZenOmenHardwareService` as a delayed-auto-start service.
- **`ZenOmen-windows-x64.zip`** — portable `zenomen.exe` + `zenomen-hw.exe`; register the service manually from an elevated prompt with `zenomen.exe --install-service`.

The GUI runs as a standard user (`asInvoker`). Only the service performs firmware writes.

---

## Usage

### 1. Starting the Application

Launch ZenOmen from your desktop application launcher, or start it from a terminal:

```bash
zenomen
```

To start minimized directly to the system tray:

```bash
zenomen --minimized
```

### 2. Ensuring the Hardware Service Is Active

For fan mode switching, power profiles, presets, and RGB control, the privileged `zenomen-hw` service must be running.

- **Linux — check the systemd service**:
  ```bash
  systemctl status zenomen-hw.service
  ```
- **Linux — start or enable it**:
  ```bash
  sudo systemctl start zenomen-hw.service
  sudo systemctl enable zenomen-hw.service
  ```
- **Linux — run a development build manually**:
  ```bash
  sudo ./target/debug/zenomen-hw
  ```
- **Windows — check the service** in `services.msc` (`ZenOmen Hardware Service`) or:
  ```powershell
  Get-Service ZenOmenHardwareService
  ```
- **Windows — run in console mode for development** (elevated prompt):
  ```powershell
  .\zenomen-hw.exe --console
  ```

### 3. Command Line Interface

The GUI implements these flags:

```text
zenomen [FLAGS]

FLAGS:
    --minimized, -m                  Start minimized directly to the system tray
    --install-system-integration     Install systemd unit, D-Bus configuration and Polkit policy (Linux, requires root)
    --uninstall-system-integration   Remove those files (Linux, requires root)
    --install-service                Register the Windows hardware service (Windows, requires administrator)
    --uninstall-service              Unregister the Windows hardware service (Windows, requires administrator)
```

Diagnostics are exported from the **Diagnostics** tab, which writes a JSON report under the platform data directory.

On Windows, the service binary exposes maintenance commands:

```text
zenomen-hw [FLAG]

FLAGS:
    --console, -c            Run the Windows hardware service in console mode
    --install-service        Register the Windows service in the SCM
    --uninstall-service      Remove the Windows service from the SCM
    --wmi-diag, --diag       Run read-only HP BIOS WMI diagnostics
    --validate-8a3d-fan      Run the HP Victus 8A3D physical fan-control validation runbook
    --validate-8a3d-power    Run the HP Victus 8A3D physical power-profile validation runbook
```

---

## Reproduction & Building from Source

### Prerequisites

ZenOmen requires Rust 2024 edition (1.85+) and, on Linux, the standard development libraries below.

#### On Ubuntu / Debian / Linux Mint:

```bash
sudo apt-get update
sudo apt-get install -y \
    build-essential \
    pkg-config \
    libfontconfig1-dev \
    libx11-dev \
    libxkbcommon-dev \
    libdbus-1-dev \
    xvfb
```

#### On Fedora:

```bash
sudo dnf install -y \
    gcc \
    pkg-config \
    fontconfig-devel \
    libX11-devel \
    libxkbcommon-devel \
    dbus-devel \
    xorg-x11-server-Xvfb
```

#### On Arch Linux:

```bash
sudo pacman -S --needed \
    base-devel \
    pkgconf \
    fontconfig \
    libx11 \
    libxkbcommon \
    dbus \
    xorg-server-xvfb
```

#### Install Rust Toolchain:

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source "$HOME/.cargo/env"
```

### Building ZenOmen

Clone the repository:

```bash
git clone https://github.com/Omprakash-p06/zenomen.git
cd zenomen
```

#### Debug Build:

```bash
cargo build
```

This produces two binaries in `target/debug/`:

- `zenomen` (unprivileged GUI application)
- `zenomen-hw` (privileged hardware service)

#### Release Build:

```bash
cargo build --release
```

Binaries are placed in `target/release/`. The release profile is size- and memory-oriented (`opt-level = "s"`, LTO, `panic = "abort"`, stripped) to protect the RSS budget.

#### Building the Service Without Slint

The GUI is a default Cargo feature. To build only the privileged service without linking the Slint toolkit:

```bash
cargo build --release --no-default-features --bin zenomen-hw
```

### Running Automated Test Suites

ZenOmen contains **322 tests**: 84 co-located unit tests in `src/` and 238 integration tests across 19 suites in `tests/`. Because GUI tests render surfaces, run the full suite under `xvfb-run` on Linux:

```bash
xvfb-run -a cargo test --all
```

To run individual layers or suites:

```bash
# Unit tests only
cargo test --lib

# Windows named-pipe protocol, board profiles, and WMI authorization
cargo test --test windows_service_tests

# Benchmark and worker latency tests
cargo test --test benchmark_tests

# Dynamic hardware degradation tests
cargo test --test degradation_tests

# Fan control and hardware client tests
cargo test --test fan_control_tests

# ACPI platform profile and power preset tests
cargo test --test power_preset_tests

# Desktop integration, tray, and autostart tests
cargo test --test desktop_integration_tests
```

Windows service tests use an injected `FakeWmiAdapter`, so they run deterministically on any host without touching real firmware. Linux-only suites (`linux_procfs_tests`, `linux_gpu_tests`) are gated with `#![cfg(target_os = "linux")]`.

### Verifying Resource Budgets

ZenOmen targets **≤40 MB peak RSS** for the GUI and **≤15 MB RSS** for the service. Measure them on Linux with:

```bash
bash scripts/measure-budget.sh dashboard-open 60 ./target/release/zenomen
bash scripts/measure-budget.sh --soak 7200 tray-idle ./target/release/zenomen --minimized
```

The script samples `/proc/<pid>/status` (`VmRSS`) and process CPU time, and writes results to `dist/budget-<stage>.csv`. It is a manual harness; CI runs format, Clippy, tests, and `cargo audit`, but does not gate on RSS.

---

## Requirements & Compatibility

### Supported Platforms

- **Linux**: Kernel 5.15+ with the `hp-wmi` module loaded (`lsmod | grep hp_wmi` or `sudo modprobe hp-wmi`). Wayland or X11 desktop environments (GNOME, KDE Plasma, XFCE, Cinnamon). A Freedesktop StatusNotifierItem host for the system tray.
- **Windows**: Windows 10/11 (x86_64). The desktop UI runs as a standard unprivileged user; the privileged background service (`ZenOmenHardwareService` / `zenomen-hw.exe`) provides HP WMI integration over a secured local named pipe (`\\.\pipe\ZenOmen.Hardware.v1`).
- **Other platforms**: no backend is implemented. The app reports zero capabilities and does not probe or guess hardware.

### Hardware Compatibility & Testing Status

ZenOmen is designed for HP OMEN and Victus gaming laptops. Because HP uses differing embedded controller (EC) registers, WMI GUIDs, and BIOS interfaces across hardware generations and AMD/Intel revisions, hardware features are gated by strict motherboard **and BIOS** validation records:

- **Verified testing**:
  - **HP Victus 15-fb0xxx (AMD)** — board `8A3D`, BIOS `F.22`:
    - CPU, GPU, RAM, and battery telemetry verified, including process memory via `K32GetProcessMemoryInfo`.
    - **Max Fan** (WMI `0x27=1`) physically validated: CPU 4000→5400 RPM, GPU 3600→5200 RPM, read-back confirmed.
    - **Automatic cooling restore** validated (`0x27=0` → `0x10` → `0x1A {0xFF, 0x01}`); the unsafe Victus S `0x2E {0,0}` reset is refused on this board because it stalls the fans.
    - **Power profiles**: writes are authorized via WMI `0x1A`, but the active profile lives in EC register `0x95` and has no userland read path, so results are reported as *accepted, unverified* rather than verified.
    - **Manual per-fan PWM / custom curves** and **RGB** remain blocked (not physically validated; no RGB zones on this board).
  - **HP OMEN 15/16 (AMD, `88F8`) on BIOS `F.20`/`F.21`** and **HP OMEN 16/17 (Intel, `8A24`) on BIOS `F.10`**: fan, Max Fan, automatic restore, and power-profile writes validated. RGB writes are authorized for these boards, but the driver still refuses RGB payloads until a verified contract exists (see Known Limitations).
- **Untested variants & community call for testers**:
  - Other OMEN (15, 16, 17, Transcend) and Victus (15, 16) variants (Intel/AMD) have differing BIOS and EC revisions.
  - **We need community contributors and testers!** If you own an OMEN or Victus laptop, run `zenomen`, open the **Diagnostics** tab, export the report, and share it (or submit a pull request) with your motherboard Board ID, BIOS version, and verified read/write capabilities.

### Known Limitations

- **Windows RGB** has no verified firmware payload contract. `zenomen-hw` refuses RGB writes instead of sending an unvalidated command.
- **Manual fan curves / per-fan PWM** are parsed and validated but disabled on every board (`manual_fan_write_verified = false`).
- **8A3D power-profile read-back** is unavailable (EC-managed); switching a profile always reports the unverified status.
- **Linux display refresh-rate control** is not implemented, so combined presets change display mode only on Windows.
- **Fan, power, preset, and RGB control require the privileged service**; without it ZenOmen is read-only telemetry and says so.
- **Resource budgets are verified manually**; there is no CI gate on RSS or a GUI visual-regression suite.
- **`src/ui/` is a dormant GPUI scaffold** compiled into the library but not used by either binary.

---

## Contributing

We welcome contributions! To maintain stability, safety, and visual polish, all pull requests must follow these standards.

### Development Workflow

1. Fork and create a topic branch:
   ```bash
   git checkout -b feature/my-feature
   ```
2. Make your changes adhering to existing design tokens (`ui/theme/colors.slint`, `metrics.slint`, `typography.slint`); spacing follows the 4 px grid.
3. Ensure formatting passes:
   ```bash
   cargo fmt --all -- --check
   ```
4. Ensure zero Clippy warnings:
   ```bash
   cargo clippy --all-targets --all-features -- -D warnings
   ```
5. Ensure a 100% test pass rate:
   ```bash
   xvfb-run -a cargo test --all
   ```
6. Commit using the conventional commit format:
   ```bash
   git commit -m "feat(cooling): add custom fan speed readout label"
   ```

CI runs the same gates on Ubuntu and Windows, plus `cargo audit` on Linux.

### Core Architecture Rules

- **Unprivileged GUI**: the `zenomen` binary must never require root/administrator and must never invoke `sudo` or perform raw sysfs/firmware writes.
- **Hardware Verification**: every write must be verified by reading the value back. On Linux use `write_and_verify`; on Windows use pre-write read, write, 150 ms settle, post-write read. Never assume a write succeeded.
- **Board Safety**: never grant a Windows write capability from a board ID alone. Update `BoardValidation` only after completing the procedure in `docs/testing/HP_BOARD_VALIDATION.md`.
- **Honest Telemetry**: missing sensors or interfaces must render `Unavailable` with diagnostic context rather than fabricated numbers or zeros.
- **Single-writer UI state**: visible state changes flow through `AppEvent` → `AppState::apply`; the UI never mutates domain state directly.
- **Strict Slint UI design system**:
  - Dark mode palette: Obsidian `#0F1115` base, Charcoal `#17191E` cards, Slate `#232730` hover/selection, Gold `#E5A93C` accent.
  - Spacing: 4 px grid increments (`ThemeMetrics`). No arbitrary pixel sizes.
  - Symmetrical layout: control cards must stretch evenly and remain centered within the content area.
- **Documentation stays in sync**: interface, test-count, or build-requirement changes must update `README.md` and the relevant `.planning/codebase/` document.

---

## License

ZenOmen is licensed under the [GNU General Public License v3.0 (GPL-3.0)](LICENSE). See [NOTICE](NOTICE) for copyright details and upstream attribution.

---

## Attribution

ZenOmen is a Rust + Slint descendant of the [OpenOMEN](https://github.com/openomen/openomen) project. It inherits the ACPI, fan control, and RGB interface discoveries originally established by OpenOMEN, while replacing the legacy Python runtime with an unprivileged, type-safe compiled architecture.

---

## Disclaimer

ZenOmen is an independent, open-source project and is **not affiliated with, endorsed by, or sponsored by HP Inc.** or the OMEN / Victus brands.

"OMEN" and "Victus" are trademarks of HP Inc. Any references to them in this project are strictly for the purpose of device compatibility and identification.

This software is provided **"as is"**, without warranty of any kind, express or implied. By using this software, you acknowledge that:

- You are responsible for any potential damage to your device.
- Modifying hardware or system behavior may void your warranty.
- The authors and contributors are not liable for any outcome resulting from the use of this software.

**Use this software at your own risk.**
