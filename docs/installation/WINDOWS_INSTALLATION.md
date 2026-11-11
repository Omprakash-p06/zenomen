# ZenOmen Windows Installation & Deployment Guide

- **Target Release:** ZenOmen 1.0.0
- **Supported OS:** Windows 10 (22H2 19045+), Windows 11 (22H2, 23H2, 24H2)
- **Architecture:** `x86_64` (`x64`)

---

## 1. Distribution Overview

ZenOmen provides two installer formats for Windows:
1. **Interactive Setup Wizard (`ZenOmen-Setup-x64.exe` — Inno Setup):**
   Recommended for end users. Features a modern installation wizard, component selection (GUI vs Service), optional PATH integration, desktop shortcut creation, and automatic SCM service registration.
2. **Standard Windows Installer (`ZenOmen-x64.msi` — WiX Toolset v4):**
   Recommended for system administrators and enterprise deployment. Native MSI package with standard MSI properties and group policy rollout support.
3. **Portable Archive (`ZenOmen-windows-x64.zip`):**
   Standalone binaries for portable or testing environments without automatic SCM service registration.

---

## 2. Interactive Installation (`ZenOmen-Setup-x64.exe`)

### 2.1 Standard Installation Steps
1. Download `ZenOmen-Setup-x64.exe` from GitHub Releases.
2. Double-click to launch the installer.
3. Accept the UAC (User Account Control) prompt. Administrative elevation is required exclusively to register and start the `ZenOmenHardwareService` in Windows Service Control Manager (SCM).
4. Review the GPL-3.0 License Agreement.
5. Select Components:
   - **ZenOmen Graphical Interface (Required):** The Slint-based desktop monitoring and control interface.
   - **ZenOmen Hardware Control Service (Recommended):** The background Windows Service enabling low-level ACPI/WMI fan and power preset management.
6. Select Additional Tasks:
   - **Create Desktop Shortcut** (Optional)
   - **Start ZenOmen automatically on Windows logon** (Optional; launches minimized to system tray)
   - **Add ZenOmen to system PATH** (Optional; enables `zenomen` and `zenomen-hw` from Command Prompt/PowerShell)
7. Click **Install**.
8. The installer writes files to `C:\Program Files\ZenOmen\`, registers the service, sets startup to automatic (delayed), and optionally launches ZenOmen.

### 2.2 Silent / Automated Installation (CLI)
For scripted or headless installations, run in an elevated PowerShell/CMD prompt:

```powershell
ZenOmen-Setup-x64.exe /VERYSILENT /NORESTART /TASKS="desktopicon,autostart"
```

Common Inno Setup flags:
- `/VERYSILENT`: Suppresses all wizard windows and status dialogs.
- `/NORESTART`: Prevents unexpected system reboots.
- `/DIR="C:\Custom\ZenOmen"`: Specifies a custom destination directory.

---

## 3. Enterprise MSI Installation (`ZenOmen-x64.msi`)

The WiX MSI installer is compiled using **WiX Toolset v4.0.6**.

### 3.1 Standard GUI Install
Double-click `ZenOmen-x64.msi` and follow on-screen prompts.

### 3.2 Silent MSI Deployment
Run via `msiexec` in an elevated shell:

```cmd
msiexec /i ZenOmen-x64.msi /qn /l*v "C:\install-zenomen.log"
```

### 3.3 WiX Toolset Version & Licensing Note
Previous CI builds encountered the following error with WiX v5:
```text
WIX7015: You must accept the Open Source Maintenance Fee (OSMF) EULA
```
- **Policy Decision:** ZenOmen pins WiX Toolset to **v4.0.6**.
- **Rationale:** WiX v4 is licensed under the Microsoft Reciprocal License (MS-RL) and does not require commercial or maintenance agreement EULA acceptance in automated CI/CD pipelines. This ensures clean, fully compliant open-source distribution without non-consensual license acceptance flags.

---

## 4. Hardware Service Architecture & Security

ZenOmen enforces strict privilege separation on Windows:

```text
+--------------------------------------------------------------+
|                    ZenOmen GUI (zenomen.exe)                |
|           Runs as unprivileged standard user token           |
|                Native Win32 Tray + Slint UI                 |
+--------------------------------------------------------------+
                               |
                               | Secure Named Pipe IPC
                               | \\.\pipe\ZenOmen.Hardware.v1
                               v
+--------------------------------------------------------------+
|            ZenOmenHardwareService (zenomen-hw.exe)           |
|            Runs as NT AUTHORITY\LocalSystem                  |
|                 SCM Managed, Delayed AutoStart               |
|                                                              |
|   -> HP BIOS WMI Fast Queries & Opcodes (0x27, 0x1A, 0x10)   |
|   -> Win32 System Performance & Memory Telemetry             |
|   -> Display Refresh Switching (ChangeDisplaySettingsExW)    |
+--------------------------------------------------------------+
```

### Security Measures:
- **Named Pipe DACL:** Configured via an explicit Security Descriptor Definition Language (SDDL) allowing read/write access to `NT AUTHORITY\LocalSystem`, `BUILTIN\Administrators`, and `NT AUTHORITY\Authenticated Users`.
- **Wire Protocol Validation:** Uses a binary header (`MAGIC = 0x5A4F4D4E`, `VERSION = 1`) with strict message length caps (maximum 64 KB) and mandatory 2000 ms read timeouts to prevent denial-of-service or memory exhaustion.
- **Fail-Closed Authorization:** Write commands (fan speeds, thermal profiles) are validated against motherboard capability tables before issuance.

---

## 5. Filesystem Locations & Configuration

ZenOmen strictly follows Windows filesystem conventions:

| Component | Filesystem Path | Description |
| :--- | :--- | :--- |
| **Installation Directory** | `C:\Program Files\ZenOmen\` | Executables (`zenomen.exe`, `zenomen-hw.exe`), icons, and docs |
| **User Configuration** | `%APPDATA%\ZenOmen\config.toml` | Per-user GUI configuration (preserved during upgrades/uninstallation) |
| **GUI Session Logs** | `%LOCALAPPDATA%\ZenOmen\logs\zenomen.log` | Daily rotating non-blocking UI logs |
| **Service System Logs** | `%PROGRAMDATA%\ZenOmen\logs\zenomen-hw.log` | Background hardware service operational logs |

---

## 6. Upgrades & Uninstallation

### 6.1 Upgrading ZenOmen
Installing a newer version over an existing installation automatically:
1. Stops the running hardware service.
2. Replaces binaries in `C:\Program Files\ZenOmen\`.
3. Restarts the hardware service.
4. **Preserves** all user settings in `%APPDATA%\ZenOmen\`.

### 6.2 Clean Uninstallation
To remove ZenOmen:
1. Open **Settings > Apps > Installed apps** (or **Control Panel > Programs and Features**).
2. Select **ZenOmen** and click **Uninstall**.
3. The uninstaller executes the following lifecycle steps:
   - Gracefully stops `ZenOmenHardwareService`.
   - Unregisters the service from Windows SCM.
   - Removes application files from `C:\Program Files\ZenOmen\`.
   - Removes Start Menu and Desktop shortcuts.
   - Cleans the autostart Registry Run key and PATH entry (if configured).
   - **Leaves** user config in `%APPDATA%\ZenOmen` intact to prevent accidental data loss.

To completely wipe all traces including user config, delete `%APPDATA%\ZenOmen` after uninstallation.
