# ZenOmen Release & Runtime Troubleshooting Guide

This guide details resolutions for common setup, packaging, driver, and permission issues on Windows and Linux.

---

## 1. Windows Troubleshooting

### 1.1 "Hardware Daemon Offline" in GUI
**Symptoms:** The ZenOmen interface displays "Hardware Daemon: Disconnected" and fan controls are disabled.

**Causes & Solutions:**
1. **Service Not Running:**
   Open an elevated PowerShell or Command Prompt:
   ```cmd
   sc query ZenOmenHardwareService
   ```
   If stopped, start it:
   ```cmd
   sc start ZenOmenHardwareService
   ```
2. **Service Not Installed:**
   If `ZenOmenHardwareService` does not exist:
   ```cmd
   "C:\Program Files\ZenOmen\zenomen-hw.exe" --install-service
   ```
3. **Inspect Daemon Log Files:**
   Check the hardware service log located at:
   `C:\ProgramData\ZenOmen\logs\hardware-service.log`
   or user GUI logs at:
   `%LOCALAPPDATA%\ZenOmen\logs\zenomen.log`

### 1.2 Named Pipe IPC Errors (`0x5` / Access Denied)
**Cause:** The named pipe `\\.\pipe\zenomen-hw` utilizes Windows Security Descriptors (DACL).
- Ensure the current Windows user is recognized as an `Authenticated User`.
- If running under a restricted sandbox or guest profile, standard named pipe access may be restricted by local group policy.

### 1.3 Motherboard 8A3D Capabilities & Safeguards
- **Fan RPM Telemetry:** Successfully read on Victus 15-fb0xxx (motherboard `8A3D`, BIOS `F.22`).
- **Max Fan Activation & Restore:** Fully ratified and active on BIOS `F.22`.
- **Thermal Profile Writes:** BIOS `F.22` rejects thermal query `0x4C` (status `0x05`). ZenOmen handles this gracefully and bypasses unsupported thermal register mutations.
- **Power Budget Writes:** Kept intentionally locked (`power_write_verified: false`) under strict fail-closed safety invariants until physical verification.

### 1.4 Binary In-Use / Locked Files During Update
**Cause:** Windows prevents overwriting running executables (`os error 5`).
**Solution:**
1. Right-click the ZenOmen system tray icon and select **Quit**.
2. Stop the background service:
   ```cmd
   net stop ZenOmenHardwareService
   ```
3. Run the new installer or replace the executable.

---

## 2. Linux Troubleshooting

### 2.1 "Failed to connect to D-Bus interface"
**Symptoms:** The GUI cannot communicate with `zenomen-hw` via D-Bus.

**Causes & Solutions:**
1. **Verify Daemon Service:**
   ```bash
   systemctl status zenomen-hw.service
   ```
   If inactive, enable and start:
   ```bash
   sudo systemctl enable --now zenomen-hw.service
   ```
2. **Review Journal Logs:**
   ```bash
   journalctl -u zenomen-hw.service -e --no-pager
   ```
3. **Verify D-Bus Policy:**
   Confirm `/usr/share/dbus-1/system.d/com.zenomen.Hardware1.conf` is present, then reload D-Bus:
   ```bash
   sudo systemctl reload dbus
   ```

### 2.2 PolicyKit Authentication Dialog Does Not Appear
**Cause:** A desktop PolicyKit authentication agent (e.g., `polkit-gnome`, `lxpolkit`, or `polkit-kde-agent`) is not running in your window manager session.
**Solution:**
- Ensure a Polkit agent is active:
  ```bash
  ps aux | grep polkit
  ```
- In lightweight window managers (i3, bspwm, Sway), ensure `lxpolkit &` or `/usr/libexec/polkit-gnome-authentication-agent-1 &` is started in your session configuration.

### 2.3 AppImage FUSE Error on Ubuntu 24.04+
**Symptoms:** `dlopen(): error loading libfuse.so.2: cannot open shared object file`
**Solution:**
Install the compatibility library or use the extraction fallback:
```bash
sudo apt install libfuse2
# Or run without FUSE:
./ZenOmen-0.1.0-x86_64.AppImage --appimage-extract-and-run
```

---

## 3. Desktop Assets & Tray Icon Visibility

### 3.1 Icon Missing in Desktop Application Launcher
If the ZenOmen logo does not appear in your desktop app menu after manual installation:
- **Linux:**
  ```bash
  gtk-update-icon-cache -f /usr/share/icons/hicolor
  update-desktop-database /usr/share/applications
  ```
- **Windows:**
  Windows Explorer caches shell icons. Rebuilding the icon cache:
  ```cmd
  ie4uinit.exe -show
  ```
