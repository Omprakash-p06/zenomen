# ZenOmen

A lightweight, native hardware control center for HP OMEN and Victus laptops on Linux.

[![Latest Release](https://img.shields.io/github/v/release/Omprakash-p06/zenomen?label=release)](https://github.com/Omprakash-p06/zenomen/releases/latest)
[![License: GPL v3](https://img.shields.io/badge/License-GPLv3-green.svg)](LICENSE)
[![CI](https://github.com/Omprakash-p06/zenomen/actions/workflows/release.yml/badge.svg)](https://github.com/Omprakash-p06/zenomen/actions)

---

## Overview

ZenOmen is written from scratch in Rust and Slint to deliver a fast, responsive, and low-overhead control center. It replaces legacy command-line scripts with a secure, unprivileged desktop GUI and a privileged system daemon:

- **Hardware Telemetry**: Live CPU, GPU, and RAM utilization, package temperatures, and fan speeds (RPM), rendered via real-time telemetry sparklines and a 120-second history chart.
- **Fan Controls**: Switching between automatic fan management and max cooling mode with sysfs read-back verification.
- **Power Profiles**: Dynamic discovery and switching across kernel power performance profiles (`quiet`, `balanced`, `performance`).
- **Coordinated Presets**: One-click combined performance presets (`Quiet`, `Balanced`, `Gaming`) coordinating ACPI profiles and cooling fans together.
- **Keyboard RGB**: Lighting control with kernel patch sysfs, legacy sysfs, and 4-zone support, verified hardware write cycles, and built-in presets.
- **Diagnostics**: One-click system hardware capability auditing and JSON diagnostic reports for troubleshooting.
- **Desktop Integration**: Freedesktop system tray residency, close-to-tray lifecycle, XDG autostart, and single-instance D-Bus application service.
- **Privileged Hardware Daemon (`zenomen-hw`)**: Headless D-Bus system service with Polkit authorization (`com.zenomen.hardware.control`) to ensure safe hardware mutation without running GUI code as root.

---

## Security Architecture & How Hardware Control Works

Unlike standalone CLI scripts that prompt for `sudo` directly in the terminal, ZenOmen enforces a strict security separation:

```
┌──────────────────────────────────────┐
│  zenomen (Unprivileged GUI Process)  │
│  - Runs as regular user              │
│  - Zero raw disk / kernel writes     │
│  - Zero network access               │
└──────────────────┬───────────────────┘
                   │ System D-Bus (com.zenomen.Hardware1)
                   │ Polkit Challenge (com.zenomen.hardware.control)
┌──────────────────▼───────────────────┐
│  zenomen-hw (Privileged Root Daemon) │
│  - Headless systemd service          │
│  - Verified read-back with retry     │
│  - Sandboxed sysfs write operations  │
└──────────────────────────────────────┘
```

When you launch `zenomen`, it communicates over the system D-Bus (`com.zenomen.Hardware1`) with `zenomen-hw`. If the daemon is not running, the GUI will operate normally in read-only telemetry mode and clearly explain that the hardware control daemon is offline.

---

## Installation

Pre-built packages are published on the [GitHub Releases page](https://github.com/Omprakash-p06/zenomen/releases/latest).

### Cryptographic Verification

Always verify downloaded release assets against `SHA256SUMS`:

```bash
sha256sum -c SHA256SUMS
```

### Debian / Ubuntu / Linux Mint (`.deb`)

```bash
sudo dpkg -i zenomen_0.1.0_amd64.deb
sudo apt-get install -f
```

The package automatically installs the desktop entry, icons, D-Bus system bus configuration (`/etc/dbus-1/system.d/com.zenomen.Hardware1.conf`), Polkit policy (`/usr/share/polkit-1/actions/com.zenomen.hardware.policy`), and enables the `zenomen-hw.service` systemd daemon.

### AppImage

Make the AppImage executable and run it:

```bash
chmod +x ZenOmen_0.1.0_amd64.AppImage
./ZenOmen_0.1.0_amd64.AppImage
```

To enable hardware fan and power control under AppImage, register system integration files once as root:

```bash
sudo ./ZenOmen_0.1.0_amd64.AppImage --install-system-integration
```

---

## Usage

### 1. Starting the Application

Launch ZenOmen from your desktop application launcher, or start it from terminal:

```bash
zenomen
```

To start minimized directly to the system tray:

```bash
zenomen --minimized
```

### 2. Ensuring the Hardware Daemon Is Active

For fan mode switching, power profiles, and RGB control, the privileged `zenomen-hw` daemon must be active.

- **Check systemd service status**:
  ```bash
  systemctl status zenomen-hw.service
  ```
- **Start the service if stopped**:
  ```bash
  sudo systemctl start zenomen-hw.service
  ```
- **Enable on boot**:
  ```bash
  sudo systemctl enable zenomen-hw.service
  ```

If running from a development build without package installation, you can run the daemon manually in a separate terminal:

```bash
sudo ./target/debug/zenomen-hw
```

### 3. Command Line Interface

```bash
zenomen [FLAGS]

FLAGS:
    --minimized                      Start application minimized directly to system tray
    --install-system-integration     Install systemd daemon unit, D-Bus configuration, and Polkit policy (requires root)
    --uninstall-system-integration   Remove system integration files from system (requires root)
    --export-diagnostics             Export sanitized JSON hardware diagnostics dump to terminal/file and exit
    -h, --help                       Prints help information
    -V, --version                    Prints version information
```

---

## Reproduction & Building from Source

### Prerequisites

ZenOmen requires Rust 2024 edition (1.85+) and standard Linux development libraries.

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
- `zenomen-hw` (privileged system daemon)

#### Release Build:

```bash
cargo build --release
```

Binaries will be placed in `target/release/`.

### Running Automated Test Suites

ZenOmen includes 123 automated unit and integration tests across 16 integration test suites. Because GUI tests render surfaces, run tests under `xvfb-run`:

```bash
xvfb-run -a cargo test --all
```

To run individual suites:

```bash
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

### Verifying Resource Budgets

To verify memory RSS and execution times against the project budget:

```bash
bash scripts/measure-budget.sh
```

---

## Requirements & Compatibility

### Supported Platforms
- **Linux**: Kernel 5.15+ with `hp-wmi` module loaded (`lsmod | grep hp_wmi` or `sudo modprobe hp-wmi`). Wayland or X11 desktop environments (GNOME, KDE Plasma, XFCE, Cinnamon). Freedesktop StatusNotifierItem supported for system tray.
- **Windows**: Windows 10/11 (x86_64). Desktop UI runs as a standard unprivileged user; optional privileged background service (`ZenOmenHardwareService` / `zenomen-hw.exe`) provides HP WMI ACPI integration over a secure local named pipe (`\\.\pipe\ZenOmen.Hardware.v1`).

### Hardware Compatibility & Testing Status
ZenOmen is designed for HP OMEN and Victus gaming laptops. Because HP uses differing embedded controller (EC) registers, WMI GUIDs, and BIOS interfaces across hardware generations and AMD/Intel motherboard revisions, hardware features are gated by strict motherboard profile matching:

- **Verified Testing**:
  - **HP Victus 15-fb0xxx (AMD)** — Motherboard Board ID `8A3D`, BIOS `F.22`:
    - CPU, GPU, RAM, and Battery telemetry verified operational.
    - Process memory monitoring validated with native Win32 `K32GetProcessMemoryInfo`.
    - Physical firmware mutations (fan speed boost, thermal power profiles) are safely gated by `BoardValidation` safety checks to protect unvalidated hardware until write-cycle ratification is completed.
- **Untested Variants & Community Call for Testers**:
  - Other OMEN (15, 16, 17, Transcend) and Victus (15, 16) variants (Intel/AMD) have varying BIOS and EC revisions.
  - **We need community contributors and testers!** If you own an OMEN or Victus laptop, run `zenomen` (or `zenomen --export-diagnostics`), open the **Diagnostics** tab, and share your diagnostic report or submit a pull request with your motherboard Board ID and verified read/write capabilities.

---

## Contributing

We welcome contributions! To maintain stability, safety, and visual polish, all pull requests must follow these standards:

### Development Workflow

1. Fork and create a topic branch:
   ```bash
   git checkout -b feature/my-feature
   ```
2. Make your changes adhering to existing design tokens (`ui/theme/colors.slint`, `metrics.slint`, `typography.slint`).
3. Ensure formatting passes:
   ```bash
   cargo fmt --check
   ```
4. Ensure zero Clippy warnings:
   ```bash
   cargo clippy --all-targets -- -D warnings
   ```
5. Ensure 100% test pass rate:
   ```bash
   xvfb-run -a cargo test --all
   ```
6. Commit using conventional commit format:
   ```bash
   git commit -m "feat(cooling): add custom fan speed readout label"
   ```

### Core Architecture Rules

- **Unprivileged GUI**: The `zenomen` binary must never require root and must never invoke `sudo` or execute raw sysfs writes directly.
- **Hardware Verification**: Every write operation in `zenomen-hw` must be verified by reading back the sysfs value with bounded retry and exponential backoff. Never assume a write succeeded without verification.
- **Honest Telemetry**: Missing sensors or interfaces must render `Unavailable` with clear diagnostic context rather than displaying fabricated numbers or zeros.
- **Strict Slint UI Design System**:
  - Dark mode palette: Obsidian `#0F1115` base, Charcoal `#17191E` cards, Slate `#232730` hover/selection, Gold `#E5A93C` accent.
  - Spacing: 4px grid increments (`ThemeMetrics`). No arbitrary pixel sizes.
  - Symmetrical layout: Control cards must stretch evenly and remain centered within the content area.

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
