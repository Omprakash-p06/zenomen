# ZenOmen Linux Installation & Deployment Guide

- **Target Release:** ZenOmen 1.0.0
- **Supported Distributions:** Linux Mint 21 / 22, Ubuntu 22.04 LTS / 24.04 LTS, Debian 12 (Bookworm)+
- **Architecture:** `x86_64` (`amd64`)

---

## 1. Distribution Overview

ZenOmen provides two installation channels for Linux:
1. **Debian Package (`.deb`) — Strongly Recommended for Linux Mint & Ubuntu:**
   Includes automatic desktop menu integration, high-resolution vector icons, systemd hardware service daemon registration, D-Bus system bus activation, and Polkit permission policies.
2. **AppImage (`.AppImage`) — Portable Standalone:**
   Self-contained executable for portable use without root installation. Requires user execution permissions and host FUSE compatibility.

---

## 2. Recommended Route: Debian Package (`.deb`)

The `.deb` package installs the complete ZenOmen ecosystem:
- GUI Client: `/usr/bin/zenomen`
- Hardware Service Daemon: `/usr/bin/zenomen-hw`
- Systemd Service: `/usr/lib/systemd/system/zenomen-hw.service`
- D-Bus Configuration: `/usr/share/dbus-1/system.d/com.zenomen.Hardware1.conf`
- D-Bus Activation: `/usr/share/dbus-1/system-services/com.zenomen.Hardware1.service`
- Polkit Policy: `/usr/share/polkit-1/actions/com.zenomen.hardware.policy`
- Desktop Menu Entry: `/usr/share/applications/zenomen.desktop`
- Icons: `/usr/share/icons/hicolor/` (complete vector SVG and multi-resolution PNG ladder)

### 2.1 Installation Steps
1. Download the latest `zenomen_1.0.0_amd64.deb` from the GitHub Releases page.
2. Install via `apt` (recommended so dependencies are resolved automatically):
   ```bash
   sudo apt update
   sudo apt install ./zenomen_1.0.0_amd64.deb
   ```
   *(Alternatively, use `sudo dpkg -i zenomen_1.0.0_amd64.deb && sudo apt install -f`)*

3. Verify service status:
   ```bash
   systemctl status zenomen-hw.service
   ```
   The maintainer scripts automatically reload systemd, reload D-Bus, and start the daemon.

4. Launch ZenOmen from your application menu (under **System** or **Utilities**), or run:
   ```bash
   zenomen
   ```

### 2.2 Uninstallation & Upgrades
- **Upgrade to a new version:**
  ```bash
  sudo apt install ./zenomen_<new-version>_amd64.deb
  ```
  Your personal settings under `~/.config/zenomen/` are strictly preserved.
- **Uninstall ZenOmen:**
  ```bash
  sudo apt remove zenomen
  ```
  This cleanly stops and disables the systemd service, unregisters D-Bus policies, and removes application binaries.
- **Purge all system configuration:**
  ```bash
  sudo apt purge zenomen
  ```

---

## 3. Portable Route: AppImage Packaging

The `ZenOmen-1.0.0-x86_64.AppImage` allows running ZenOmen directly without root privileges.

### 3.1 Step 1: Make the File Executable
Browsers do not mark downloaded files as executable by default. Before opening, grant execute permissions:
```bash
chmod +x ZenOmen-1.0.0-x86_64.AppImage
```

### 3.2 Step 2: FUSE Compatibility Requirements
Standard Type-2 AppImages use FUSE (Filesystem in Userspace) to mount the application bundle.

Modern Linux distributions (such as Linux Mint 21/22 and Ubuntu 22.04/24.04) ship with FUSE 3 (`libfuse3`) by default, but standard AppImage runtimes require FUSE 2 compatibility libraries.

If double-clicking the AppImage in your file manager does not open it, install the appropriate FUSE 2 package for your system:

#### Linux Mint 21 / Ubuntu 22.04:
```bash
sudo apt install libfuse2
```

#### Linux Mint 22 / Ubuntu 24.04:
```bash
sudo apt install libfuse2t64
```

Once installed, double-clicking `ZenOmen-1.0.0-x86_64.AppImage` in Nemo, Nautilus, or Dolphin will launch the application normally.

### 3.3 Step 3: FUSE-Free Fallback (`--appimage-extract-and-run`)
If your system cannot install FUSE 2 libraries or runs in a restricted container/sandbox environment, you can run ZenOmen using the extract-and-run fallback:

```bash
./ZenOmen-1.0.0-x86_64.AppImage --appimage-extract-and-run
```
This extracts the bundle into a temporary directory in memory/temp and executes ZenOmen immediately without mounting through FUSE.

---

## 4. Hardware Service & D-Bus Permissions

ZenOmen operates on a two-tier architecture:
1. **Unprivileged GUI (`zenomen`):** Runs within the user's desktop session.
2. **Elevated Daemon (`zenomen-hw`):** Manages sysfs, hwmon, and WMI cooling interfaces requiring elevated permissions.

### Communication Channels:
- **Primary:** D-Bus System Bus (`com.zenomen.Hardware1`)
- **Fallback:** Unix Domain Socket (`/run/zenomen-hw/hw.sock`)

If the daemon is not running (e.g., when using the standalone AppImage without the system service), ZenOmen displays hardware telemetry via sysfs read-only queries, but privileged operations (such as forcing Max Fan mode) will request Polkit authentication or require starting `zenomen-hw`.

---

## 5. Standard Filesystem Locations (XDG Compliance)

ZenOmen adheres to the XDG Base Directory specification and does not use hardcoded paths:

| Component | Standard Path | Purpose |
| :--- | :--- | :--- |
| **User Configuration** | `~/.config/zenomen/config.toml` | UI preferences, launch behavior, polling rates |
| **Application Logs** | `~/.local/state/zenomen/logs/zenomen.log` | Non-blocking daily rotating user session logs |
| **Daemon Logs** | `/var/log/zenomen/zenomen-hw.log` or systemd journal | Hardware service operational logs |
| **Systemd Service** | `/usr/lib/systemd/system/zenomen-hw.service` | System service definition (installed by `.deb`) |
| **D-Bus Policy** | `/usr/share/dbus-1/system.d/com.zenomen.Hardware1.conf` | D-Bus system security policy |
| **Polkit Policy** | `/usr/share/polkit-1/actions/com.zenomen.hardware.policy` | Polkit privilege escalation definitions |

---

## 6. Troubleshooting Common Linux Issues

### Issue 1: AppImage double-click does nothing
- **Fix:** Open terminal in your Downloads folder and run `./ZenOmen-1.0.0-x86_64.AppImage`.
- If error states `cannot mount AppImage, please check your FUSE setup`:
  - Run `sudo apt install libfuse2` (Mint 21) or `sudo apt install libfuse2t64` (Mint 22).
- If error states `Permission denied`:
  - Run `chmod +x ZenOmen-1.0.0-x86_64.AppImage`.

### Issue 2: Fans report 0 RPM at idle
- **Explanation:** On HP Victus laptops (including motherboard 8A3D), 0 RPM at low temperatures (~35–37°C) is normal passive cooling. Fans automatically spin up once temperature rises under load.
- See `docs/hardware/AUTO_FAN_VALIDATION.md` for complete technical details.

### Issue 3: Hardware service shows "Disconnected" in GUI
- **Fix:** Check if the background service is running:
  ```bash
  sudo systemctl restart zenomen-hw.service
  systemctl status zenomen-hw.service
  ```
