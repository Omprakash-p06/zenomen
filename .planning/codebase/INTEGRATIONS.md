---
last_mapped_commit: 4691619f4e741bd5944f2085fa455b135ff873af
last_mapped_at: 2026-11-08
---
# External Integrations

**Analysis Date:** 2026-11-08
**Branch:** `fix/zenomen-reliability`
**HEAD Commit:** `4691619f4e741bd5944f2085fa455b135ff873af`

## Overview

ZenOmen operates without remote cloud backends, databases, telemetry servers, or webhooks. All integration surfaces are strictly local to the host operating system:

- **Linux Platform:**
  - System D-Bus (`com.zenomen.Hardware1`) & Unix domain socket (`/run/zenomen-hw/hw.sock`) for privileged hardware control, built on `zbus` 4 / `zbus` 5 (resolved to `5.19.0`).
  - Session D-Bus for single-instance activation (`com.zenomen.App`) and system tray integration (`org.kde.StatusNotifierItem`, `com.canonical.dbusmenu`).
  - Polkit authorization through `org.freedesktop.PolicyKit1` (`com.zenomen.hardware.control`).
  - Kernel pseudo-filesystems: `procfs`, `sysfs`, DMI/SMBIOS, ACPI, DRM, and `hwmon`.
  - Process invocations: `nvidia-smi` (via long-lived `NvidiaSampler` thread `zenomen-nv-watch`), `systemctl` (installation only), and `xdg-open` (browser update links).
- **Windows Platform (Windows 10 22H2+ & Windows 11 24H2+):**
  - Win32 API Bindings: `windows-sys` 0.59 (`Win32_System_Performance` for PDH, `Win32_System_Services` for SCM, `Win32_System_Pipes`, `Win32_Storage_FileSystem`, `Win32_Security`, `Win32_UI_Shell`, `Win32_System_Wmi`, `Win32_System_Com`).
  - Named Pipe IPC (`\\.\pipe\ZenOmen.Hardware.v1`) connecting unprivileged GUI (`zenomen.exe`) to privileged background service (`zenomen-hw.exe`).
  - Wire protocol framing: 4-byte big-endian length prefix followed by UTF-8 JSON payload strictly capped at 64 KB (`MAX_PAYLOAD_SIZE = 65536`). Synchronous blocking I/O (no `FILE_FLAG_OVERLAPPED`).
  - Access Control & DACL: Explicit `PIPE_SDDL` security descriptor (`D:(A;;GA;;;SY)(A;;GA;;;BA)(A;;GRGW;;;IU)`) allowing `LocalSystem` (Full Control), `Builtin\Administrators` (Full Control), and `Interactive Users` (Read/Write).
  - Caller session verification against `WTSGetActiveConsoleSessionId()` via caller token impersonation.
  - Windows Service Control Manager (SCM) lifecycle: `ZenOmenHardwareService` (`src/service/windows/lifecycle.rs`) running under `NT AUTHORITY\LocalSystem` with delayed auto-start, dispatched via `StartServiceCtrlDispatcherW`, `RegisterServiceCtrlHandlerExW`, and `SetServiceStatus` (with fallback to standalone console mode).
  - Telemetry: Win32 `GetSystemTimes` (CPU), `GlobalMemoryStatusEx` (RAM), DirectX DXGI 1.3 `EnumAdapters1` (GPU adapter & dedicated VRAM), persistent PDH `GpuPerformanceCollector` querying `\GPU Engine(*)\Utilization Percentage`, and `GetSystemPowerStatus` (Battery).
  - Hardware Control: Direct ACPI BIOS execution via WMI Fast Command `hpqBIntM` (`ROOT\WMI`, method `hpqBMtdx`, GUID `{5FB7F034-2C63-45E9-BE91-3D44E2C707E4}`), gated by physical `BoardValidation` contracts.
  - Desktop integration: Win32 tray via `Shell_NotifyIconW` with `hwnd` binding (`nid.hWnd = hwnd` from `TrayHandle`) and thread-affinity message pump in dedicated thread (`zenomen-win32-tray-pump`), Windows Registry Run key autostart (`HKCU\Software\Microsoft\Windows\CurrentVersion\Run`), and single-instance named mutex (`Local\ZenOmen.SingleInstance`).

---

## Privileged Hardware Services & IPC

ZenOmen strictly isolates unprivileged user presentation from elevated firmware mutation using OS-native local IPC mechanisms.

### Linux: System D-Bus & Unix Domain Socket
- **D-Bus Bus Name:** `com.zenomen.Hardware1`
- **Object Path:** `/com/zenomen/Hardware1`
- **Interface:** `com.zenomen.Hardware1`
- **Unix Domain Socket:** `/run/zenomen-hw/hw.sock`
- **Server:** `zenomen-hw` (`src/bin/zenomen-hw.rs`, `src/service/contract.rs`, `src/service/backend.rs`)
- **Client:** `HardwareServiceClient` (`src/service/client.rs`) via `zbus` 4 / `zbus` 5 (`5.19.0` with `blocking` feature)
- **Service Unit:** `data/systemd/zenomen-hw.service` (`Type=dbus`, `BusName=com.zenomen.Hardware1`, `ExecStart=/usr/bin/zenomen-hw`)
- **Bus Activation:** `/usr/share/dbus-1/system-services/com.zenomen.Hardware1.service`
- **Bus Policy:** `/usr/share/dbus-1/system.d/com.zenomen.Hardware1.conf` (root owns; local users can send/receive)

**Methods:**
| Method | Mutating | Authorization | Return |
|---|---|---|---|
| `GetCapabilities` | No | None | JSON string of `HardwareCapabilities` |
| `GetFanStatus` | No | None | `(String mode, Vec<u32> rpms)` |
| `SetFanMode(mode)` | Yes | Polkit `com.zenomen.hardware.control` | Confirmed mode string |
| `GetPowerProfiles` | No | None | Advertised ACPI profile names |
| `GetPowerProfile` | No | None | Active ACPI profile string |
| `SetPowerProfile(profile)` | Yes | Polkit `com.zenomen.hardware.control` | Confirmed profile string |
| `ApplyPreset(preset)` | Yes | Polkit `com.zenomen.hardware.control` | Serialized `PresetResult` JSON |
| `ApplyRgbProfile(profile_json)` | Yes | Polkit `com.zenomen.hardware.control` | Confirmation string |
| `SetRgbEnabled(enabled)` | Yes | Polkit `com.zenomen.hardware.control` | Confirmation string |

### Windows: Local Named Pipe (`ZenOmen.Hardware.v1`) & SCM Service
- **Pipe Name:** `\\.\pipe\ZenOmen.Hardware.v1` (`src/service/windows/pipe.rs`)
- **Mode:** Byte stream duplex, synchronous blocking I/O (`PIPE_ACCESS_DUPLEX | FILE_FLAG_FIRST_PIPE_INSTANCE`, `PIPE_TYPE_MESSAGE | PIPE_READMODE_MESSAGE | PIPE_WAIT`). `FILE_FLAG_OVERLAPPED` is omitted to guarantee synchronous blocking I/O.
- **Buffer Size:** 64 KB (`PIPE_BUFFER_SIZE = 65536`)
- **Client Timeout:** 2000ms default query timeout; 30000ms connection timeout (`PIPE_TIMEOUT_MS = 30000`)
- **Wire Protocol Framing:**
  - Strict **4-byte big-endian length prefix** (`(json.len() as u32).to_be_bytes()`)
  - UTF-8 JSON payload strictly capped at 64 KB (`MAX_PAYLOAD_SIZE = 65536`)
  - Payloads exceeding 64 KB are rejected with `HardwareControlError::ProtocolError`
  - Bidirectional request/response mapping via `HardwareRequest` and `HardwareResponse` (`src/service/windows/protocol.rs`)
- **Security Descriptor & DACL (`PIPE_SDDL`):**
  - Explicit SDDL string: `D:(A;;GA;;;SY)(A;;GA;;;BA)(A;;GRGW;;;IU)`
  - Full Control (`GA`): `NT AUTHORITY\SYSTEM` (`SY`) and `BUILTIN\Administrators` (`BA`)
  - Read/Write (`GRGW`): `NT AUTHORITY\Interactive Users` (`IU`) and Authenticated Users
  - Strict Default Deny: Remote RPC callers, anonymous tokens, and network logins are denied
- **Caller Session Verification (`src/service/windows/security.rs`):**
  - Mutating operations query `WTSGetActiveConsoleSessionId()`
  - Caller token impersonation (`ImpersonateNamedPipeClient`) inspects `TokenSessionId` via `GetTokenInformation`
  - If caller session ID $\ne$ active physical console session, mutation is rejected with `AccessDenied`
  - Reverts impersonation via `RevertToSelf()` and closes handles cleanly
- **Windows Service Control Manager (SCM) Lifecycle (`src/service/windows/lifecycle.rs`):**
  - Service Name: `ZenOmenHardwareService`
  - Display Name: `ZenOmen Hardware Control Service`
  - Account: `NT AUTHORITY\LocalSystem` (`LocalSystem`)
  - Start Type: `SERVICE_AUTO_START` (with delayed auto-start support)
  - SCM registration and removal handled via native Win32 `CreateServiceW` / `DeleteService`
  - Service Dispatcher: `StartServiceCtrlDispatcherW` initializes `service_main`, registers `service_ctrl_handler` via `RegisterServiceCtrlHandlerExW`, and updates states via `SetServiceStatus` (`SERVICE_START_PENDING`, `SERVICE_RUNNING`, `SERVICE_STOP_PENDING`, `SERVICE_STOPPED`)
  - Console Mode Fallback: Automatically falls back to standalone console mode (`run_console_service()`) when invoked outside SCM (`ERROR_FAILED_SERVICE_CONTROLLER_CONNECT`) or via `zenomen-hw.exe --console` / `-c`

---

## Hardware Telemetry & Control Subsystems

### Core Telemetry Sensors (11-Sensor Architecture)
ZenOmen tracks exactly **11 core hardware telemetry sensors** across both operating systems (`TelemetrySnapshot::EXPECTED_SENSOR_COUNT = 11`):
1. CPU temperature (`cpu_temp_c`)
2. CPU utilization (`cpu_utilization`)
3. CPU frequency (`cpu_freq_mhz`)
4. GPU temperature (`gpu_temp_c`)
5. GPU utilization (`gpu_utilization`)
6. GPU dedicated VRAM used (`gpu_mem_used_mb`)
7. RAM used (`ram_used_mb`)
8. RAM total (`ram_total_mb`)
9. Fan tachometer RPMs (`fan_rpms`)
10. Fan mode (`fan_mode`)
11. Power profile / preset (`power_profile`)

*(Keyboard RGB profile state is presentation state managed in `AppState` and excluded from hardware sensor health count).*

### Linux Telemetry & Control (sysfs / procfs / hwmon)
- **CPU Metrics:** `/proc/stat` for utilization jiffies; `/sys/devices/system/cpu/cpu0/cpufreq/scaling_cur_freq` (or `/proc/cpuinfo`) for frequency; `/sys/class/hwmon/hwmon*/temp*_input` (`coretemp`/`k10temp`) for temperature.
- **Memory Metrics:** `/proc/meminfo` (`MemTotal`, `MemAvailable`).
- **GPU Metrics (AMD/Intel):** `/sys/class/drm/card*/device/gpu_busy_percent`, `mem_info_vram_used`, `mem_info_vram_total`, and drm hwmon temperatures.
- **GPU Metrics (NVIDIA):** Handled via `NvidiaSampler` (`src/hardware/linux/gpu.rs`) invoking `nvidia-smi` on a decoupled channel (`mpsc::sync_channel(1)`) with a 3-second cache window. Prevents GPU D3cold sleep delays from blocking the telemetry pipeline.
- **Cooling & Fan Control:** `/sys/devices/platform/hp-wmi/hwmon/hwmon*/pwm1_enable` (and `/sys/class/hwmon/hwmon*/pwm1_enable`).
- **ACPI Power Profiles:** `/sys/firmware/acpi/platform_profile` and `platform_profile_choices`.
- **Keyboard Lighting:** `/sys/devices/platform/hp-wmi/rgb_zones/zone0{0..3}`, `/sys/devices/platform/hp-wmi/keyboardleds`, or `/sys/class/leds/hp_omen::kbd_backlight/zone_colors`.

### Windows Telemetry & Control (Win32 / DXGI / WMI)
- **CPU Telemetry (`src/hardware/windows/cpu.rs`):**
  - Utilization: Monotonic delta calculation from `GetSystemTimes(&idle, &kernel, &user)`. Correctly subtracts `delta_idle` from `(delta_kernel + delta_user)`.
  - Frequency: `CallNtPowerInformation` (level 11 `ProcessorInformation`) from `powrprof.dll`, with PDH fallback.
  - Model Name: Registry `HKLM\HARDWARE\DESCRIPTION\System\CentralProcessor\0\ProcessorNameString` with CPUID brand string fallback.
- **Memory Telemetry (`src/hardware/windows/memory.rs`):**
  - Queried via `GlobalMemoryStatusEx(&mut MEMORYSTATUSEX)` using `ullTotalPhys` and `ullAvailPhys`.
- **GPU Telemetry (`src/hardware/windows/gpu.rs`):**
  - Adapter enumeration via DXGI 1.3 (`CreateDXGIFactory1`, `EnumAdapters1`).
  - Real-time video memory usage queried via `IDXGIAdapter3::QueryVideoMemoryInfo`.
  - PCI Vendor identification: NVIDIA (`0x10DE`), AMD (`0x1002`), Intel (`0x8086`).
  - GPU Utilization: Persistent `GpuPerformanceCollector` querying `\GPU Engine(*)\Utilization Percentage` via PDH (Performance Data Helper) with double-precision counter formatting.
  - Thermal / Sensor Policy: Returns honest `None` where unprivileged APIs lack thermal sensors.
- **Battery Telemetry (`src/hardware/windows/battery.rs`):**
  - Queried via `GetSystemPowerStatus(&mut SYSTEM_POWER_STATUS)`.
  - Parses charging status (`BatteryFlag & 8`), percentage (`BatteryLifePercent`), and remaining seconds (`BatteryLifeTime`). Reports `None` if `BatteryFlag == 128` (no battery present). Guard against false charging on flag `255`.
- **HP BIOS Hardware Control (`src/service/windows/hp_wmi.rs`, ADR-002):**
  - Direct ACPI BIOS invocation via WMI Fast Command class `hpqBIntM` under `ROOT\WMI`.
  - Method: `hpqBMtdx` on GUID `{5FB7F034-2C63-45E9-BE91-3D44E2C707E4}` with signature `0x55434553` (`"SECU"`).
  - Commands: `FAN_COUNT` (`0x10`), `FAN_SPEEDS` (`0x11`), `PERFORMANCE_MODE` (`0x1A`), `MAX_FAN_STATUS` (`0x26`), `MAX_FAN_SET` (`0x27`), `FOUR_ZONE_RGB` (`0x20009`).
  - Physical Hardware Validation: Gated by `BoardValidation` contracts (`src/service/windows/authorization.rs`, `src/service/windows/profiles.rs`). Baseline `new_unvalidated` disables mutations; mutations require `new_verified` motherboard profile match.

---

## Desktop Environment Integrations

### Linux Desktop Integration
- **System Tray:** Implements `org.kde.StatusNotifierItem` at `/StatusNotifierItem` and `com.canonical.dbusmenu` at `/MenuBar`. Registers with `org.kde.StatusNotifierWatcher` on session bus.
- **Single Instance:** Session D-Bus well-known name `com.zenomen.App` acquired with `DO_NOT_QUEUE`. Secondary launches call `PresentWindow` and exit immediately.
- **Autostart:** XDG standard desktop entry at `~/.config/autostart/zenomen.desktop` with `--minimized` argument.
- **Polkit Authorization:** Action `com.zenomen.hardware.control` defined in `/usr/share/polkit-1/actions/com.zenomen.hardware.policy`.

### Windows Desktop Integration
- **System Tray (`src/desktop/windows/tray.rs`):**
  - Win32 notification icon registered via `Shell_NotifyIconW`.
  - Dedicated background thread `zenomen-win32-tray-pump` creates a hidden window (`ZenOmenTrayMsgWindow`) and services the Win32 message pump, ensuring required thread-affinity for Windows messages and context menus (`TrackPopupMenu`).
  - Context menu provides Quick Actions: Open ZenOmen, Fan Auto, Fan Max, Quit.
- **Desktop Notifications (`src/desktop/windows/notifications.rs`):**
  - Implemented via `Shell_NotifyIconW(NIM_MODIFY)`.
  - Explicit **Window Handle Binding**: Sets `nid.hWnd = hwnd` from `TrayHandle` (`show_notification_with_hwnd`) ensuring notification balloon popups bind directly to the active system tray icon.
- **Single Instance (`src/desktop/windows/single_instance.rs`):**
  - Enforced via session-scoped Win32 Named Mutex: `Local\ZenOmen.SingleInstance`.
  - Secondary instances detect `ERROR_ALREADY_EXISTS`, locate the primary window using `FindWindowW(NULL, "ZenOmen")`, bring it to front with `ShowWindow(SW_RESTORE)` and `SetForegroundWindow`, and terminate immediately.
- **Autostart (`src/desktop/windows/autostart.rs`):**
  - Configured in Windows Registry under `HKEY_CURRENT_USER\Software\Microsoft\Windows\CurrentVersion\Run`.
  - Writes value `"ZenOmen"` pointing to `"<exe_path>" --minimized`. Runs unprivileged with zero UAC elevation required.

---

## Packaging, Distribution & CI/CD Matrix

### Packaging Formats
| Platform | Packaging Format | Tooling | Output Artifact | Notes |
|---|---|---|---|---|
| Linux | Debian Package (`.deb`) | `cargo-packager` | `dist/zenomen_<version>_amd64.deb` | Installs binaries, systemd unit, D-Bus policies, desktop entry |
| Linux | AppImage (`.AppImage`) | `cargo-packager` | `dist/ZenOmen_<version>_amd64.AppImage` | Portable binary with embedded self-install CLI (`--install-system-integration`) |
| Windows | Windows Installer (`.msi`) | WiX Toolset v4/v5 | `dist/ZenOmen-x64.msi` | Configures `ZenOmenHardwareService` with auto-start, shortcuts, and icon |
| Windows | Standalone ZIP (`.zip`) | `Compress-Archive` | `dist/zenomen-windows-x64.zip` | Standalone portable bundle with both binaries, license, and docs |

### GitHub Actions Dual-OS Matrix
- **Continuous Integration (`.github/workflows/ci.yml`):**
  - Trigger: Push and PR to `main`.
  - `linux-quality`: Runs on `ubuntu-latest`. Checks formatting (`cargo fmt`), strict clippy (`-D warnings`), headless tests under `xvfb-run --auto-servernum cargo test --all`, and vulnerability auditing via `cargo audit`.
  - `windows-quality`: Runs on `windows-latest`. Checks formatting, Windows clippy across all targets and features, and native Windows tests (`cargo test --all`).
- **Release Automation (`.github/workflows/release.yml`):**
  - Trigger: Semver tags matching `v*` and manual `workflow_dispatch`.
  - Validates tag against `Cargo.toml` version.
  - Builds `.deb` and `.AppImage` on Linux, and `.msi` and `.zip` on Windows.
  - Consolidates SHA-256 checksums into unified `SHA256SUMS` manifest and publishes unified GitHub Release.

---

## Failure & Degradation Matrix

| Component / Subsystem | Failure Condition | Graceful Degradation Behavior |
|---|---|---|
| `zenomen-hw` Daemon / Service | Offline or stopped | GUI remains interactive in read-only mode; status strip warns service is offline with actionable restart command (`systemctl start zenomen-hw` or `services.msc`). |
| Windows Named Pipe | Missing pipe or connection error | Client returns `HardwareControlError::ConnectionFailed`; GUI displays offline banner without crashing. |
| Polkit (Linux) | User cancels or authorization denied | Mutating call aborted; hardware remains unchanged; user notified in UI. |
| Console Session (Windows) | Client is RDP / non-active session | Named pipe service rejects mutation with `AccessDenied`; active physical console user protected. |
| HP WMI (`hpqBIntM`) | Unsupported board or missing WMI class | Capabilities report `has_fan_control: false` and `has_power_presets: false`; UI elements disabled. |
| NVIDIA GPU | Discrete GPU offline or in D3cold | `NvidiaSampler` thread returns `None` after 3s cache expiry; UI panels show `Unavailable`; sampling thread never blocks. |
| Battery | Desktop machine or disconnected battery | `BatteryFlag == 128`; `battery` telemetry returns `None`; UI safely hides or disables battery metrics. |
| System Tray | Watcher missing (Linux GNOME without extension) | Registration skipped with info log; application window continues operating normally. |
| Logging Subsystem | State directory unwritable | Non-blocking file appender fails cleanly; logging automatically falls back to stderr. |

---

*Integration specification updated: 2026-11-08 (HEAD `4691619f4e741bd5944f2085fa455b135ff873af`)*
