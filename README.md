# ZenOmen

A lightweight, native hardware control center for HP OMEN and Victus laptops on Linux.

[![Latest Release](https://img.shields.io/github/v/release/Omprakash-p06/zenomen?label=release)](https://github.com/Omprakash-p06/zenomen/releases/latest)
[![License: GPL v3](https://img.shields.io/badge/License-GPLv3-green.svg)](LICENSE)
[![CI](https://github.com/Omprakash-p06/zenomen/actions/workflows/release.yml/badge.svg)](https://github.com/Omprakash-p06/zenomen/actions)

---

## What It Does Today

ZenOmen is written from scratch in Rust and Slint to deliver a fast, responsive, and low-overhead control center. Features available today:

- **Hardware Telemetry**: Live CPU, GPU, and RAM utilization, package temperatures, and fan speeds (RPM), rendered via 60-second real-time telemetry sparklines and charts.
- **Fan Controls**: Switching between automatic fan management and max cooling mode via the privileged hardware daemon.
- **Power Profiles**: Dynamic discovery and switching across kernel power performance profiles (`quiet`, `balanced`, `performance`).
- **Keyboard RGB**: Lighting control with kernel patch sysfs, legacy sysfs, and 4-zone support, verified hardware write cycles, and built-in presets.
- **Diagnostics**: One-click system hardware capability auditing and JSON diagnostic reports for troubleshooting.
- **Desktop Integration**: Freedesktop system tray residency, close-to-tray lifecycle, XDG autostart, and single-instance D-Bus application service.
- **Privileged Hardware Daemon (`zenomen-hw`)**: Headless D-Bus system service with Polkit authorization (`com.zenomen.hardware.control`) to ensure safe hardware mutation without running GUI code as root.

---

## Installation

Pre-built packages are published on the [GitHub Releases page](https://github.com/Omprakash-p06/zenomen/releases/latest).

### Cryptographic Verification

Always verify downloaded release assets against `SHA256SUMS`:

```bash
sha256sum -c SHA256SUMS
```

### Debian / Ubuntu (`.deb`)

```bash
sudo dpkg -i zenomen_0.1.0_amd64.deb
sudo apt-get install -f
```

The package automatically installs the desktop entry, icons, D-Bus system bus configuration, Polkit actions, and the `zenomen-hw.service` systemd unit.

### AppImage

Make the AppImage executable and run it:

```bash
chmod +x ZenOmen-0.1.0-x86_64.AppImage
./ZenOmen-0.1.0-x86_64.AppImage
```

To enable hardware fan and power control under AppImage, register system integration files once as root:

```bash
sudo ./ZenOmen-0.1.0-x86_64.AppImage --install-system-integration
```

For full packaging details and instructions for building from source, see [docs/PACKAGING.md](docs/PACKAGING.md).

---

## Requirements

- **Linux Kernel**: 6.1+ recommended.
- **Kernel Drivers**: `hp-wmi` driver loaded (`modprobe hp-wmi`), standard `hwmon` sysfs interface, and platform driver support.
- **Privileged Access**: `zenomen-hw` daemon running on the system bus (`com.zenomen.Hardware1`) managed via Polkit.
- **Graphics**: OpenGL 2.1+ or Vulkan-compatible GPU driver. (A software fallback is provided by Mesa llvmpipe).
- *(Note: While the architecture separates hardware providers to facilitate Windows support, Windows is currently unadvertised and unsupported).*

---

## Architecture & Resource Budget

ZenOmen is split into two cleanly separated processes:
1. An unprivileged GUI desktop application (`zenomen`) built with Rust and the Slint UI toolkit. It performs zero direct hardware writes, runs zero shell processes, and makes zero network requests.
2. A privileged system daemon (`zenomen-hw`) exposing `com.zenomen.Hardware1` on D-Bus, validating all mutating requests through Polkit authorization before writing to hardware sysfs nodes.

Empirical measurements from [docs/RESOURCE-BUDGET.md](docs/RESOURCE-BUDGET.md):
- **Cold Start RSS**: 7.15 MB (target < 30 MB)
- **Warm Dashboard RSS**: 16.56 MB (target < 38 MB)
- **Tray Minimized RSS**: 16.59 MB (target < 25 MB)
- **Idle Dashboard CPU**: 0.0914% (target ≤ 0.10%)
- **Tray Minimized CPU**: 0.0000% (visibility-gated sampling halts polling; target ≤ 0.02%)
- **Hardware Service Daemon RSS**: 3.43 MB (target ≤ 15 MB, reported separately)

---

## Attribution

ZenOmen is a Rust + Slint descendant of the [OpenOMEN](https://github.com/openomen/openomen) project. It inherits the ACPI, fan control, and RGB interface discoveries originally established by OpenOMEN, while replacing the legacy Python runtime with an unprivileged, type-safe compiled architecture.

---

## License

ZenOmen is licensed under the [GNU General Public License v3.0 (GPL-3.0)](LICENSE). See [NOTICE](NOTICE) for copyright details and upstream attribution.

---

## Disclaimer

ZenOmen is an independent, open-source project and is **not affiliated with, endorsed by, or sponsored by HP Inc.** or the OMEN / Victus brands.

"OMEN" and "Victus" are trademarks of HP Inc. Any references to them in this project are strictly for the purpose of device compatibility and identification.

This software is provided **"as is"**, without warranty of any kind, express or implied. By using this software, you acknowledge that:

- You are responsible for any potential damage to your device.
- Modifying hardware or system behavior may void your warranty.
- The authors and contributors are not liable for any outcome resulting from the use of this software.

**Use this software at your own risk.**
