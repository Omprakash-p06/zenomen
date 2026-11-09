# External Integrations

**Analysis Date:** 2026-11-09

## APIs & External Services

### Linux Kernel Interfaces (sysfs / procfs)

Direct filesystem reads and writes via standard library (`std::fs`); no dependence on external sensor daemons (e.g. `lm-sensors`) or userland intermediaries. Path discovery and validation live in `src/hardware/linux/provider.rs`, with specialized parsers in `src/hardware/linux/`.

| Subsystem | Target Filesystem Path | Purpose | Implementation Module |
|---|---|---|---|
| **CPU Utilization** | `/proc/stat` | Aggregate and per-core CPU jiffies calculation | `src/hardware/linux/cpu.rs` |
| **CPU Identification** | `/proc/cpuinfo` | Processor model name string, fallback clock speeds | `src/hardware/linux/cpu.rs` |
| **CPU Frequency** | `/sys/devices/system/cpu/cpu*/cpufreq/scaling_cur_freq` | Dynamic core clock frequency (kHz to MHz conversion) | `src/hardware/linux/cpu.rs` |
| **Memory Metrics** | `/proc/meminfo` | `MemTotal`, `MemAvailable`, `MemFree`, buffers, and cache | `src/hardware/linux/memory.rs` |
| **DRM GPU Enumeration** | `/sys/class/drm/card*` | Dynamic discrete and integrated GPU discovery | `src/hardware/linux/gpu.rs` |
| **AMD GPU Telemetry** | `<drm_card>/device/gpu_busy_percent`<br>`<drm_card>/device/mem_info_vram_used`<br>`<drm_card>/device/mem_info_vram_total` | AMD GPU core load percentage and dedicated VRAM usage | `src/hardware/linux/gpu.rs` |
| **Thermal & Tachometer Sensors** | `/sys/class/hwmon/hwmon*` (`name`, `temp*_input`, `temp*_label`, `fan*_input`, `fan*_label`) | Temperature probe readings (m°C to °C) and fan speeds (RPM) | `src/hardware/linux/hwmon.rs` |
| **ACPI Platform Profile** | `/sys/firmware/acpi/platform_profile`<br>`/sys/firmware/acpi/platform_profile_choices` | Reading and writing ACPI power profiles (`quiet`, `balanced`, `performance`) | `src/service/backend.rs` |
| **HP WMI Fan Mode** | `/sys/devices/platform/hp-wmi/hwmon/hwmon*/pwm1_enable`<br>fallback: `/sys/class/hwmon/hwmon*/pwm1_enable` | Hardware cooling mode write/read: `0` = Max Fan, `1` = Manual, `2` = Automatic | `src/service/backend.rs` |
| **Battery Subsystem** | `/sys/class/power_supply/BAT*` (`capacity`, `status`) | Battery charge percentage and AC/discharging status | `src/hardware/linux/provider.rs` |
| **4-Zone RGB Lighting** | `/sys/devices/platform/hp-wmi/rgb_zones/zone00` … `zone03` | Hex/RGB color control for 4 independent keyboard zones | `src/hardware/rgb.rs` |
| **Monolithic RGB (Legacy)** | `/sys/devices/platform/hp-wmi/keyboardleds` | Legacy single-zone RGB backlight hex write | `src/hardware/rgb.rs` |
| **Upstream LED Subsystem** | `/sys/class/leds/hp_omen::kbd_backlight/zone_colors` | Kernel-patch standard LED class RGB interface | `src/hardware/rgb.rs` |

**Verification & Write Semantics:**
Mutations to `pwm1_enable` and `platform_profile` are exclusively performed by the privileged `zenomen-hw` daemon. After writing to sysfs, values are read back immediately with exponential backoff retries (`src/service/backend.rs`). Mutating operations via D-Bus are enforced by Polkit authorization checks (`src/service/contract.rs`).

---

### External Executables (Linux)

- **`nvidia-smi` (NVIDIA GPU Metrics):**
  - **Path Probe:** `/usr/bin/nvidia-smi`, `/bin/nvidia-smi`, or resolved through `$PATH`.
  - **Invocation:** `nvidia-smi --query-gpu=name,utilization.gpu,temperature.gpu,memory.used,memory.total --format=csv,noheader,nounits`.
  - **Architecture:** Managed via a single persistent background watcher thread (`src/hardware/linux/gpu.rs`). Polling occurs asynchronously; cached samples older than 3 seconds are treated as unavailable to prevent presenting stale telemetry.
- **`xdg-open` (Desktop Web Browser Invocation):**
  - Used in `src/desktop/releases.rs` to launch the user's default browser at the GitHub Releases page when update checks are clicked.

---

### D-Bus — Linux System Bus (Privilege Separation Boundary)

- **Service Bus Name:** `com.zenomen.Hardware1`
- **Object Path:** `/com/zenomen/Hardware1`
- **Interface Name:** `com.zenomen.Hardware1`
- **Daemon Implementation:** `src/service/contract.rs` (`#[interface(name = "com.zenomen.Hardware1")]`), started by `src/bin/zenomen-hw.rs`.
- **Client Implementation:** `src/service/client.rs` (`HardwareServiceClient`).
- **Transport Library:** `zbus` 5.19 (using synchronous/blocking APIs; no async runtime required).
- **Bus Configuration File:** `data/dbus/com.zenomen.Hardware1.conf` (allows unprivileged users to call methods; root owns service name).
- **Activation Service File:** `data/dbus/com.zenomen.Hardware1.service` (declares `Exec=/usr/bin/zenomen-hw` and `SystemdService=zenomen-hw.service`).
- **Exposed D-Bus Methods:**
  - `GetCapabilities() -> String` (JSON containing supported features and versions)
  - `GetFanStatus() -> (String, Vec<u32>)` (Active mode: `"auto"`, `"max"`, etc., and fan RPMs)
  - `SetFanMode(String) -> String` (*Mutating*, Polkit-gated)
  - `GetPowerProfiles() -> Vec<String>` (List of supported ACPI profile strings)
  - `GetPowerProfile() -> String` (Current active profile)
  - `SetPowerProfile(String) -> String` (*Mutating*, Polkit-gated)
  - `ApplyPreset(String) -> String` (*Mutating*, Polkit-gated, returns JSON `PresetResult`)
  - `ApplyRgbProfile(String) -> String` (*Mutating*, Polkit-gated, accepts serialized `RgbProfile`)
  - `SetRgbEnabled(bool) -> String` (*Mutating*, Polkit-gated)
- **Development Flag:** Setting `ZENOMEN_HW_SESSION=1` forces `zenomen-hw` to connect to the session bus instead of the system bus (`src/bin/zenomen-hw.rs`).

---

### D-Bus — Linux Session Bus (Desktop Integration)

- **Single Instance Enforcement:**
  - Registered under bus name `com.zenomen.App` with object path `/com/zenomen/App` (`src/desktop/single_instance.rs`).
  - Second instance attempts to acquire name; if already owned, calls `com.zenomen.App.Activate()` to raise existing window and immediately exits.
- **System Tray & Application Menu:**
  - Implements `org.kde.StatusNotifierItem` at `/StatusNotifierItem` (`src/desktop/tray.rs`).
  - Implements `com.canonical.dbusmenu` at the same object path for tray context menus.
  - Registers with desktop StatusNotifierWatcher via `org.kde.StatusNotifierWatcher.RegisterStatusNotifierItem`.

---

### Polkit Authorization (Linux Privilege Gate)

- **Action Identifier:** `com.zenomen.hardware.control`
- **Policy Definition:** `data/polkit/com.zenomen.hardware.policy`
  - `allow_any`: `auth_admin`
  - `allow_inactive`: `auth_admin`
  - `allow_active`: `auth_admin_keep` (caches authentication for session window)
- **Enforcement:**
  - `PolkitVerifier::check_authorization(&conn, sender, POLKIT_ACTION_CONTROL, true)` is executed inside `src/service/contract.rs` on every mutating D-Bus invocation prior to modifying kernel sysfs.

---

### systemd Service Lifecycle (Linux)

- **Unit File:** `data/systemd/zenomen-hw.service` installed to `/usr/lib/systemd/system/zenomen-hw.service`.
- **Command:** `ExecStart=/usr/bin/zenomen-hw`.
- **Security Sandboxing:**
  - `ProtectSystem=strict`
  - `ProtectHome=true`
  - `CapabilityBoundingSet=` (drops all POSIX capabilities)
  - `SystemCallFilter=@system-service`
  - `ReadWritePaths=-/sys/devices/platform/hp-wmi -/sys/firmware/acpi -/sys/class/hwmon -/sys/class/leds` (restricts write access exclusively to HP hardware control paths).

---

### Windows HP WMI Firmware Interface

Direct hardware control on HP OMEN and Victus laptops under Windows interacts with the HP BIOS ACPI WMI provider (`src/service/windows/hp_wmi.rs`).

- **Namespace:** `ROOT\WMI`
- **Class:** `hpqBIntM`
- **Method:** `hpqBMtdx`
- **BIOS GUID:** `5FB7F034-2C63-45E9-BE91-3D44E2C707E4`
- **COM FFI Implementation:** Direct custom COM FFI (`src/service/windows/wmi_ffi.rs`) using `IWbemLocator`, `IWbemServices`, `IWbemClassObject`, and `IEnumWbemClassObject`. Avoids heavy third-party WMI crates.
- **Protocol & Buffer Layout (HP BIOS Mailbox):**
  - `Sign` parameter: `0x55434553` (ASCII `"SECU"` little-endian).
  - `ReqType` parameter: `0x20008` (Read) / `0x20009` (Write).
  - `InBuf` (140 bytes total):
    `[cmd: u32 LE (0..4), subcmd: u32 LE (4..8), data_len: u32 LE (8..12), payload: [u8; 128] (12..140)]`
  - `OutBuf` (≥136 bytes):
    `[status: u32 LE (0..4), out_len: u32 LE (4..8), payload: [u8; 128] (8..136)]`
  - **Firmware Status Codes (`FirmwareStatus`):**
    - `0x00`: Success
    - `0x01`: Unknown Command (unsupported on this motherboard)
    - `0x02`: Invalid Input
    - `0x04`: Access Denied (BIOS password lock)
    - `0x05`: Busy (Embedded Controller transient busy state; triggered retry with 200 ms sleep)
- **Supported Command IDs (`src/service/windows/hp_wmi.rs:58-66`):**
  - `0x10`: Fan Count probe
  - `0x11`: Fan Speeds query
  - `0x1A`: ACPI Performance Profile get/set (`0x00` = Balanced, `0x01` = Performance, `0x02` = Cool)
  - `0x26`: Max Fan Status query
  - `0x27`: Max Fan Set (`0x00` = Auto, `0x01` = Max, `0x02` = Quiet)
  - `0x28`: System Design Data
  - `0x00020009`: Four-Zone Keyboard RGB color write (16-byte payload representing 4 x RGBA/RGB0 tuples)
- **Read-Back Verification:**
  - Mutations to fan mode (`set_fan_mode`) and performance mode (`set_performance_mode`) verify state by issuing a read query 150 ms after writing. If firmware returns discordant values, `HardwareControlError::VerificationFailed` is raised. No process-local statics are used.

---

### Windows Motherboard Safety & Physical Validation (WIN-07)

Firmware mutation safety is enforced in `src/service/windows/profiles.rs` and `src/service/windows/authorization.rs`.

- **Board Catalog (`resolve_board_profile`):**
  - `88F8`: HP OMEN 15/16 (AMD Zen 3) — 2 fans, Max Fan supported, 4 RGB zones.
  - `8A24`: HP OMEN 16/17 (Intel 12th/13th Gen) — 2 fans, Max Fan supported, 4 RGB zones.
  - `8A3D`: HP Victus 15-fb0xxx (AMD) — 2 fans, Max Fan supported, 0 RGB zones.
  - `8A3E`: HP Victus 15/16 (Intel) — 2 fans, Max Fan supported, 0 RGB zones.
  - `88D0`: HP OMEN 15 (2020 Intel) — 2 fans, Max Fan supported, 4 RGB zones.
- **Physical Validation Gating (`BoardValidation`):**
  - Only boards with verified physical test records (`BoardValidation::new_verified`) have write flags set (`fan_write_verified`, `power_write_verified`, `rgb_write_verified`).
  - `88F8` is validated on BIOS `F.20` and `F.21`.
  - `8A24` is validated on BIOS `F.10`.
  - **Board `8A3D` (Victus 15-fb0xxx, BIOS F.22) is currently unvalidated:** Fan and power write operations are strictly blocked by authorization until physical validation is completed per `docs/testing/HP_BOARD_VALIDATION.md`. Safe read-only WMI probing is permitted.

---

### Windows Named-Pipe IPC (Privilege Separation Boundary)

- **Pipe Endpoint:** `\\.\pipe\ZenOmen.Hardware.v1` (`src/service/windows/pipe.rs:7`).
- **Pipe Mode:** `PIPE_TYPE_MESSAGE | PIPE_READMODE_MESSAGE | PIPE_WAIT`, 4 max instances, 64 KiB buffer (`PIPE_BUFFER_SIZE = 65536`), 30-second default timeout (`src/service/windows/pipe.rs`).
- **Framing Protocol:**
  - Length Prefix: 4-byte **big-endian** integer defining payload byte length (`src/service/windows/protocol.rs:41-90`).
  - Payload: Serialized JSON representation of `HardwareRequest` or `HardwareResponse`.
  - Max Frame Size: 65,536 bytes (`MAX_PAYLOAD_SIZE`).
- **Security & DACL Restrictions (`src/service/windows/security.rs`):**
  - Pipe Security Descriptor created via SDDL string:
    `D:(A;;GA;;;SY)(A;;GA;;;BA)(A;;GRGW;;;IU)`
    - `(A;;GA;;;SY)`: Generic All for `NT AUTHORITY\SYSTEM`
    - `(A;;GA;;;BA)`: Generic All for `BUILTIN\Administrators`
    - `(A;;GRGW;;;IU)`: Generic Read + Generic Write for `NT AUTHORITY\Interactive Users`
- **Session Hijack Prevention:**
  - Server invokes `GetNamedPipeClientSessionId(pipe, &mut pipe_session_id)` and compares it against `WTSGetActiveConsoleSessionId()`.
  - Cross-session connections (remote desktop, SSH terminals, background services) are rejected with `HardwareControlError::AccessDenied`.
  - In elevated service mode, client token is impersonated via `ImpersonateNamedPipeClient` to perform defense-in-depth token inspection before reverting to self (`RevertToSelf`).

---

### Windows Service Control Manager (SCM)

- **Service Name:** `ZenOmenHardwareService`
- **Display Name:** `ZenOmen Hardware Control Service`
- **Service Account:** `LocalSystem`
- **Binary:** `zenomen-hw.exe`
- **Lifecycle Implementation:** `src/service/windows/lifecycle.rs`
- **CLI Commands on `zenomen-hw.exe`:**
  - `--install-service`: Registers service in SCM with `SERVICE_WIN32_OWN_PROCESS` and `SERVICE_AUTO_START`.
  - `--uninstall-service`: Marks service for deletion in SCM.
  - `--console` / `-c`: Runs the named-pipe server in standalone console mode for testing.
  - (No flags): Invokes `StartServiceCtrlDispatcherW`; automatically falls back to console mode if `ERROR_FAILED_SERVICE_CONTROLLER_CONNECT` is detected.
- **SCM Control Handler:** Handles `SERVICE_CONTROL_STOP` and `SERVICE_CONTROL_SHUTDOWN`. On shutdown, the service restores fan mode to Auto (`0x00`) so the EC resumes autonomous cooling safely.
- **GUI Elevation Helper:** `src/desktop/windows/installer.rs` provides `install_service_elevated`, executing `ShellExecuteExW` with the `"runas"` verb to prompt for UAC elevation during service setup.

---

### Windows Telemetry & Desktop APIs

| Data / Subsystem | Windows API / Source | File Reference |
|---|---|---|
| **CPU Utilization** | `GetSystemTimes` (kernel, user, idle delta calculation) | `src/hardware/windows/cpu.rs` |
| **CPU Identification** | Registry `HKLM\HARDWARE\DESCRIPTION\System\CentralProcessor\0\ProcessorNameString` | `src/hardware/windows/cpu.rs` |
| **Physical Memory** | `GlobalMemoryStatusEx` (`ullTotalPhys`, `ullAvailPhys`) | `src/hardware/windows/memory.rs` |
| **Battery Status** | `GetSystemPowerStatus` (`BatteryFlag`, `BatteryLifePercent`, `ACLineStatus`) | `src/hardware/windows/battery.rs` |
| **GPU Enumeration** | DXGI 1.3 (`dxgi.dll` -> `CreateDXGIFactory1`, `EnumAdapters1`, `GetDesc1`) for name, vendor ID, dedicated/shared VRAM | `src/hardware/windows/gpu.rs` |
| **GPU Utilization** | PDH (`pdh.dll` -> `\GPU Engine(*)\Utilization Percentage`) query across all GPU engines | `src/hardware/windows/gpu.rs` |
| **Temperature / Fan Sensors** | Guaranteed Absence: returns `(None, None)` and empty fan vectors because no ring-0 driver is bundled | `src/hardware/windows/sensors.rs` |
| **Motherboard Identity** | Registry `HKLM\HARDWARE\DESCRIPTION\System\BIOS` (`BaseBoardProduct`, `BIOSVersion`, `SystemProductName`) | `src/hardware/windows/device.rs` |
| **System Tray** | Win32 `Shell_NotifyIconW` with custom window message `WM_USER + 1` | `src/desktop/windows/tray.rs` |
| **Balloon Notifications** | `Shell_NotifyIconW` with `NIF_INFO` flags | `src/desktop/windows/notifications.rs` |
| **Single Instance** | Win32 Named Mutex `Local\ZenOmen.SingleInstance` via `CreateMutexW` | `src/desktop/windows/single_instance.rs` |
| **Registry Autostart** | Registry `HKCU\Software\Microsoft\Windows\CurrentVersion\Run`, value `ZenOmen` | `src/desktop/windows/autostart.rs` |
| **Release Browser Launch** | Win32 `ShellExecuteW` with `"open"` verb | `src/desktop/windows/releases.rs` |
| **Known Folder Paths** | `SHGetKnownFolderPath` (`FOLDERID_LocalAppData`, `FOLDERID_ProgramData`) | `src/desktop/windows/paths.rs` |

---

## Data Storage

**Databases:**
- None. No SQL/NoSQL embedded database engine is linked or required.

**Configuration Files:**
- **Linux:** `~/.config/zenomen/config.toml` (standard XDG path).
- **Windows GUI:** `%LOCALAPPDATA%\ZenOmen\config.toml`.
- **Windows Service State:** `%PROGRAMDATA%\ZenOmen\`.
- **Storage Strategy:** Parsed via `toml` 0.8 into `AppConfig`. Writes use atomic temporary file creation followed by rename. Corrupted configurations are automatically backed up as `config.toml.bak.<timestamp>` before resetting to verified defaults (`src/config/mod.rs`).

**Logging Storage:**
- **Linux:** `~/.local/state/zenomen/logs/` (daily rolling `zenomen*.log`, capped at 7 historical files).
- **Windows GUI:** `%LOCALAPPDATA%\ZenOmen\logs\`.
- **Windows Service:** `%PROGRAMDATA%\ZenOmen\logs\`.
- **Implementation:** `src/logging.rs` via `tracing-appender::rolling::daily`.

**In-Memory Caching:**
- Bounded ring buffer (`src/core/telemetry.rs`) holding the most recent 120 `TelemetrySample` entries for real-time charting.
- Single-slot cache for asynchronous NVIDIA metrics with a 3-second expiration threshold.

---

## Authentication & Authorization

- **Linux:**
  - Polkit authorization policy `com.zenomen.hardware.control` enforced by the privileged D-Bus daemon (`src/service/polkit.rs`).
- **Windows:**
  - Local-only Named Pipe security:
    - DACL explicitly restricts pipe access to `LocalSystem`, `Administrators`, and `Interactive Users`.
    - Session ID check strictly matches `WTSGetActiveConsoleSessionId` to block non-console callers.
- **Cloud / Network Identity:**
  - Zero cloud, web, or remote network authentication. No API keys, passwords, or tokens exist in the codebase.

---

## Webhooks, Callbacks & Network Communications

- **Inbound:** No HTTP servers, webhook receivers, or TCP/UDP listeners. The only entry points are local IPC endpoints:
  - Linux D-Bus System Bus: `com.zenomen.Hardware1`.
  - Windows Named Pipe: `\\.\pipe\ZenOmen.Hardware.v1`.
- **Outbound:** Application code performs zero outbound network requests. Update checking delegates entirely to the host OS default browser opening `https://github.com/Omprakash-p06/zenomen/releases/latest` via `xdg-open` or `ShellExecuteW`.

---

## Platform Capability Matrix

| Feature / Subsystem | Linux Implementation | Windows Implementation |
|---|---|---|
| **CPU Telemetry** | sysfs (`cpufreq`) + procfs (`/proc/stat`, `/proc/cpuinfo`) | Win32 `GetSystemTimes` + Registry `CentralProcessor` |
| **RAM Telemetry** | procfs (`/proc/meminfo`) | Win32 `GlobalMemoryStatusEx` |
| **Battery Telemetry** | sysfs (`/sys/class/power_supply/BAT*`) | Win32 `GetSystemPowerStatus` |
| **GPU Enumeration** | sysfs DRM (`/sys/class/drm/card*`) | DXGI 1.3 (`CreateDXGIFactory1`, `EnumAdapters1`) |
| **GPU Utilization** | AMD sysfs (`gpu_busy_percent`) / `nvidia-smi` | PDH (`\GPU Engine(*)\Utilization Percentage`) |
| **GPU Dedicated VRAM** | AMD sysfs (`mem_info_vram_*`) / `nvidia-smi` | DXGI adapter description (`dedicated_video_memory`) |
| **Thermal Telemetry** | hwmon sysfs (`/sys/class/hwmon/hwmon*`) | Unavailable (`(None, None)` returned; no ring-0 driver) |
| **Fan Tachometer (RPM)** | hwmon sysfs (`fan*_input`) | Unavailable (empty vector returned; no ring-0 driver) |
| **Fan Mode Control** | sysfs `pwm1_enable` via D-Bus daemon | HP WMI (`hpqBMtdx`, cmd `0x27`) via LocalSystem service |
| **Power Profiles** | ACPI `platform_profile` via D-Bus daemon | HP WMI (`hpqBMtdx`, cmd `0x1A`) via LocalSystem service |
| **Keyboard RGB Lighting** | hp-wmi sysfs driver variants | HP WMI (`hpqBMtdx`, cmd `0x00020009`) |
| **IPC Transport** | D-Bus system bus (`com.zenomen.Hardware1`) | Message-mode Named Pipe (`\\.\pipe\ZenOmen.Hardware.v1`) |
| **Access Authorization** | Polkit (`com.zenomen.hardware.control`) | Named Pipe DACL + Active Console Session ID check |
| **Desktop System Tray** | `org.kde.StatusNotifierItem` + `com.canonical.dbusmenu` | Win32 `Shell_NotifyIconW` |
| **Single Instance Lock** | Session D-Bus name `com.zenomen.App` | Win32 Named Mutex `Local\ZenOmen.SingleInstance` |
| **Autostart Mechanism** | XDG desktop file in `~/.config/autostart/` | Registry key `HKCU\...\CurrentVersion\Run` |

---

*Integration audit verified: 2026-11-09*
