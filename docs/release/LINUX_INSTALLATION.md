# ZenOmen Linux Installation & Packaging Guide

This guide describes installation, privilege separation, service setup, and desktop integration for ZenOmen on Linux distributions.

---

## 1. Supported Packages

| Format | File Pattern | Target Distributions | System Integration |
| :--- | :--- | :--- | :--- |
| **Debian Package** | `zenomen_<version>_amd64.deb` | Ubuntu 22.04+, Debian 12+, Linux Mint, Pop!_OS | Native systemd, D-Bus, and Polkit integration |
| **AppImage** | `ZenOmen-<version>-x86_64.AppImage` | Fedora, Arch Linux, openSUSE, RHEL, Void, NixOS | Portable standalone; explicit opt-in system integration |

---

## 2. Debian Package (`.deb`)

### Installation via APT

Using `apt` automatically resolves dependencies (`libxkbcommon0`, `libfontconfig1`, `libgl1`, `libdbus-1-3`):

```bash
sudo apt update
sudo apt install ./zenomen_1.0.0_amd64.deb
```

Or with `dpkg`:

```bash
sudo dpkg -i zenomen_1.0.0_amd64.deb
sudo apt-get install -f
```

### Installed File Hierarchy

```text
/usr/bin/zenomen                                              # GUI application
/usr/bin/zenomen-hw                                           # Privileged hardware daemon
/usr/share/applications/zenomen.desktop                       # FreeDesktop launcher
/usr/lib/systemd/system/zenomen-hw.service                    # Systemd service unit
/usr/share/dbus-1/system.d/com.zenomen.Hardware1.conf         # D-Bus security policy
/usr/share/dbus-1/system-services/com.zenomen.Hardware1.service # D-Bus activation unit
/usr/share/polkit-1/actions/com.zenomen.hardware.policy       # PolicyKit authorization rules
/usr/share/icons/hicolor/scalable/apps/zenomen.svg            # Scalable vector logo
/usr/share/icons/hicolor/<size>x<size>/apps/zenomen.png       # Standard bitmap icons (16px..512px)
```

### Starting the Hardware Daemon

The package installs the systemd unit. Enable and start the background daemon:

```bash
sudo systemctl daemon-reload
sudo systemctl enable --now zenomen-hw.service

# Verify daemon status
sudo systemctl status zenomen-hw.service
```

### Uninstallation

```bash
sudo apt remove zenomen
# Or purge configuration files:
sudo apt purge zenomen
```

---

## 3. AppImage (Standalone Portable Bundle)

The AppImage is completely standalone and bundles required libraries. It adheres to strict privilege separation: launching the AppImage as an unprivileged user **never** alters system directories or writes to `/usr/lib/systemd`.

### Running Portably

```bash
chmod +x ZenOmen-1.0.0-x86_64.AppImage
./ZenOmen-1.0.0-x86_64.AppImage
```

> **Note on FUSE 2 in modern distributions (Ubuntu 22.04+, 24.04+):**
> If your system does not have `libfuse2` installed, you can run the AppImage without FUSE using:
> ```bash
> ./ZenOmen-1.0.0-x86_64.AppImage --appimage-extract-and-run
> ```
> Or install the compatibility library: `sudo apt install libfuse2t64` (Ubuntu 24.04+) or `sudo apt install libfuse2` (Ubuntu 22.04 / Debian).

Options:
```bash
# Launch directly minimized to the system tray
./ZenOmen-1.0.0-x86_64.AppImage --minimized
```

### Optional Explicit System Integration

To allow the AppImage to manage system fan controls without manual terminal elevation each time, execute the explicit system integration command with `sudo`:

```bash
sudo ./ZenOmen-1.0.0-x86_64.AppImage --install-system-integration
```

This extracts the daemon binary to `/usr/bin/zenomen-hw`, writes the systemd unit and polkit policy, and enables the systemd service.

To reverse this and remove all system integrations:

```bash
sudo ./ZenOmen-1.0.0-x86_64.AppImage --uninstall-system-integration
```

---

## 4. Privilege Model & PolicyKit Security

ZenOmen uses a D-Bus system bus interface (`com.zenomen.Hardware1`) guarded by PolicyKit (`com.zenomen.hardware.policy`):

- **Telemetry Read Operations:** Reading fan RPM, CPU/GPU temperatures, battery state, and board profiles does **not** require elevation.
- **Hardware Mutation Operations:** Changing fan speed modes, setting manual fan curves, or modifying thermal profiles requires PolicyKit authorization (`auth_admin_keep`). The user's desktop environment displays a native authentication dialog.
