# ZenOmen Windows Installation & Deployment Guide

This guide covers distribution formats, installation steps, silent enterprise deployment, service lifecycle, and uninstallation for ZenOmen on Windows 10 and 11 (64-bit).

---

## 1. Distribution Packages

| Package | Format | Primary Use Case | Elevation Required |
| :--- | :--- | :--- | :--- |
| **`ZenOmen-Setup-x64.exe`** | Inno Setup Executable | Standard user installation wizard | Yes (UAC Prompt) |
| **`ZenOmen-x64.msi`** | Windows Installer Database | Enterprise / Active Directory GPO deployment | Yes (Per-Machine) |
| **`ZenOmen-windows-x64.zip`** | Portable ZIP Archive | Manual extraction and portable usage | Optional (Service requires Admin) |

All official release artifacts are signed and published with cryptographic SHA-256 checksums in `SHA256SUMS`.

---

## 2. Interactive Installation Wizard (`ZenOmen-Setup-x64.exe`)

The setup wizard guides you through standard installation options:

1. **License Agreement:** Review the GNU General Public License v3.0.
2. **Destination Location:** Defaults to `{autopf}\ZenOmen` (`C:\Program Files\ZenOmen`).
3. **Component Selection:**
   - **ZenOmen Graphical Interface (Required):** Installs `zenomen.exe` desktop GUI, system tray monitor, and application assets.
   - **ZenOmen Hardware Service (Recommended):** Installs `zenomen-hw.exe` and registers it as a background Windows service (`ZenOmenHardwareService`) for elevated ACPI/WMI hardware telemetry and fan control.
4. **Additional Tasks:**
   - *Create a desktop shortcut:* Places a shortcut on the desktop.
   - *Start ZenOmen automatically on Windows logon:* Adds an entry to `HKLM\Software\Microsoft\Windows\CurrentVersion\Run` to launch ZenOmen minimized to the system tray on logon.
   - *Add ZenOmen to system PATH:* Appends `{app}` to the system `Path` environment variable, enabling direct CLI execution of `zenomen` and `zenomen-hw` from any shell.
5. **Completion:** Setup starts the hardware service and optionally launches the ZenOmen GUI.

---

## 3. Unattended & Silent Deployment

System administrators can automate installation via standard Inno Setup command-line parameters:

```powershell
# Completely silent installation with all components and PATH integration
.\ZenOmen-Setup-x64.exe /VERYSILENT /SUPPRESSMSGBOXES /NORESTART /COMPONENTS="gui,service" /TASKS="addtopath,autostart"

# Silent install without automatic logon startup
.\ZenOmen-Setup-x64.exe /VERYSILENT /SUPPRESSMSGBOXES /NORESTART /COMPONENTS="gui,service" /TASKS="addtopath"
```

For MSI deployment using `msiexec`:

```powershell
msiexec /i ZenOmen-x64.msi /qn /norestart
```

---

## 4. Hardware Service Architecture & Lifecycle

ZenOmen operates using a privilege-separated model:
- **GUI Process (`zenomen.exe`):** Runs within standard user credentials and presents the Slint UI and tray icon.
- **Hardware Daemon (`zenomen-hw.exe`):** Executes under `NT AUTHORITY\LocalSystem` to interact with HP BIOS WMI ACPI methods (`HP_WmiGuid` / `0x48505143`).
- **IPC Transport:** A secure local Named Pipe at `\\.\pipe\zenomen-hw` governed by a strict Discretionary Access Control List (DACL):
  - Read access is granted to `NT AUTHORITY\Authenticated Users`.
  - Mutation commands require `Builtin\Administrators` or `LocalSystem`.

### Service Management via Command Line

You can manually manage the service lifecycle using `zenomen-hw.exe` or standard Windows Service Control Manager (`sc.exe`):

```cmd
:: Install and register service
"C:\Program Files\ZenOmen\zenomen-hw.exe" --install-service

:: Check service status
sc query ZenOmenHardwareService

:: Start or stop service
sc start ZenOmenHardwareService
sc stop ZenOmenHardwareService

:: Uninstall and unregister service
"C:\Program Files\ZenOmen\zenomen-hw.exe" --uninstall-service
```

---

## 5. Command-Line Interface (CLI)

When PATH integration is enabled, ZenOmen commands can be executed directly from Command Prompt, PowerShell, or Windows Terminal:

```cmd
:: Launch GUI minimized to system tray
zenomen --minimized

:: Open GUI directly into Fan Control settings
zenomen --tab fan

:: Query hardware daemon service status
zenomen-hw --status

:: View hardware daemon logs or test telemetry
zenomen-hw --test-telemetry
```

---

## 6. Uninstallation

To remove ZenOmen from Windows:
1. Open **Settings** > **Apps** > **Installed apps** (or **Control Panel** > **Programs and Features**).
2. Select **ZenOmen** and click **Uninstall**.
3. Alternatively, run `"C:\Program Files\ZenOmen\unins000.exe"`.

**What the uninstaller cleans:**
- Stops and unregisters `ZenOmenHardwareService`.
- Removes program executables, assets, and icons from `C:\Program Files\ZenOmen`.
- Removes desktop and Start Menu shortcuts.
- Cleans the logon autostart entry from the Windows Registry.
- Removes `C:\Program Files\ZenOmen` from the system `Path` variable and broadcasts `WM_SETTINGCHANGE`.

*Note:* User-specific application logs and configuration under `%LOCALAPPDATA%\ZenOmen` (`C:\Users\<User>\AppData\Local\ZenOmen`) are preserved to prevent accidental loss of custom fan curves. They may be removed manually if a purge is desired.
