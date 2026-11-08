---
last_mapped_commit: 4691619f4e741bd5944f2085fa455b135ff873af
last_mapped_at: 2026-11-08
---
<!-- refreshed: 2026-11-08 -->

# Architecture

**Analysis Date:** 2026-11-08

## System Overview

ZenOmen is a cross-platform, two-process Rust (edition 2024) application designed for HP OMEN and Victus gaming laptops on both **Linux** and **Windows 10/11** (`x86_64`). Across both operating systems, the architecture strictly decouples unprivileged user presentation and telemetry observation from privileged hardware mutation:

1. **Unprivileged Desktop GUI (`zenomen`):** Runs within the interactive desktop session with standard user permissions (`asInvoker` on Windows; standard UID > 1000 on Linux). Owns rendering, telemetry polling, configuration persistence, and diagnostics export. Never touches raw `/dev`, sysfs write nodes, or privileged firmware interfaces directly.
2. **Privileged Hardware Daemon (`zenomen-hw`):** Runs as a persistent system service (root `systemd` daemon on Linux; `NT AUTHORITY\LocalSystem` Windows Service on Windows). Owns every hardware mutation (fan modes, power profiles, RGB keyboard lighting) and exposes a domain-specific, typed, authorization-checked IPC boundary.
3. **Cross-Platform Seam Isolation:** Core business logic, single-writer state management, and telemetry buffers have zero operating system dependencies. Platform-specific implementations are strictly isolated behind compiler cfg gates (`#[cfg(target_os = "...")]`) and unified traits (`HardwareProvider`).

```text
┌─────────────────────────────────────────────────────────────────────────────────────────────┐
│ Process A — `zenomen` Desktop GUI (Standard User / asInvoker; built with `--features ui`)   │
│                                                                                             │
│  [Active GUI: Slint 1.9 / 1.17]           [Target Scaffold: GPUI-CE 0.3.0]                  │
│  ui/app.slint + ui/screens/ + ui/comp/    src/ui/app.rs + src/ui/screens/ + src/ui/comp/    │
│            ▲                                         ▲                                      │
│            │ (Slint Properties)                      │ (Typed Element Tree)                 │
│  src/app.rs (Slint ↔ core bridge)         src/ui/app.rs (GPUI-CE event bridge scaffold)     │
│            ▲                                         ▲                                      │
│            └────────────────────────┬────────────────┘                                      │
│                                     │ AppState::apply / AppCommand                          │
│                                     ▼                                                       │
│  src/core/ (events, single-writer state, 1 Hz worker, bounded telemetry history, diag)      │
│  src/config/ (TOML persistence), src/logging.rs (early non-blocking rolling logs)           │
│                                     │                                                       │
│                                     ▼ sample_telemetry()                                    │
│  src/hardware/provider.rs (`HardwareProvider` trait: Send + Sync)                           │
│  ├── Linux: LinuxHardwareProvider (procfs, sysfs, hwmon, DRM, NVIDIA watcher)               │
│  ├── Windows: WindowsHardwareProvider (Win32 GetSystemTimes, GlobalMemoryStatusEx,          │
│  │                                     DXGI 1.3 EnumAdapters1, PDH GpuPerformanceCollector) │
│  ├── Mock: MockHardware (deterministic scenarios for headless CI and tests)                 │
│  └── Unsupported: UnavailableHardwareProvider (honest zero-capability fallback stub)        │
│                                                                                             │
│  Desktop Integration:                                                                       │
│  ├── Linux (src/desktop/*): session D-Bus single instance, SNI tray, XDG autostart          │
│  └── Windows (src/desktop/windows/*): Win32 Mutex, Win32 tray pump, HKCU Run, ShellExecuteW │
└──────────────────────────┬────────────────────────────────────┬─────────────────────────────┘
                           │ Linux IPC                          │ Windows IPC
                           │ (System D-Bus)                     │ (Named Pipe)
                           ▼                                    ▼
             com.zenomen.Hardware1                \\.\pipe\ZenOmen.Hardware.v1
             /com/zenomen/Hardware1               Security: DACL SDDL (SYSTEM, BA, IU)
             Polkit: com.zenomen.hardware.control Session: WTSGetActiveConsoleSessionId check
                           │                                    │
                           ▼                                    ▼
┌──────────────────────────────────────────────────┐   ┌──────────────────────────────────────┐
│ Process B (Linux) — `zenomen-hw` (root)          │   │ Process B (Windows) — `zenomen-hw`   │
│                                                  │   │ ("ZenOmenHardwareService", SYSTEM)   │
│  src/service/contract.rs (zbus #[interface])     │   │  src/service/windows/lifecycle.rs    │
│  src/service/polkit.rs (Polkit authority check)  │   │    (SCM dispatcher & console mode)   │
│  src/service/backend.rs (verified sysfs writes,  │   │  src/service/windows/server.rs (pipe)│
│                         backoff retries)         │   │  src/service/windows/authorization.rs│
│  systemd sandbox: ProtectSystem=strict           │   │    (BoardValidation safety gating)   │
│                                                  │   │  src/service/windows/hp_wmi.rs       │
│                                                  │   │    (HP WMI BIOS hpqBIntM / hpqBMtdx) │
└──────────────────────────────────────────────────┘   └──────────────────────────────────────┘
```

---

## Dual-OS Architecture & Abstraction

### 1. Unified Hardware Abstraction (`HardwareProvider`)
The core monitoring worker communicates with hardware backends through a single, read-only seam: the `HardwareProvider` trait defined in [`src/hardware/provider.rs`](file:///home/omprakash/Documents/zenomen/src/hardware/provider.rs).

```rust
pub trait HardwareProvider: Send + Sync {
    fn capabilities(&self) -> HardwareCapabilities;
    fn sample_telemetry(&self) -> Result<TelemetrySnapshot, String>;
}
```

The application provides four implementations of this trait:
- **`LinuxHardwareProvider`** ([`src/hardware/linux/provider.rs`](file:///home/omprakash/Documents/zenomen/src/hardware/linux/provider.rs)): Used on Linux (`#[cfg(target_os = "linux")]`). Samples CPU usage from `/proc/stat`, CPU frequency from `/sys/devices/system/cpu/cpu*/cpufreq/scaling_cur_freq` (falling back to `/proc/cpuinfo`), CPU temperature via canonical `SensorRegistry` bindings (`k10temp` → `coretemp` → `acpitz`), memory from `/proc/meminfo`, fan tachometers from `/sys/class/hwmon`, and GPU telemetry via DRM sysfs or background `NvidiaSampler`.
- **`WindowsHardwareProvider`** ([`src/hardware/windows/provider.rs`](file:///home/omprakash/Documents/zenomen/src/hardware/windows/provider.rs)): Used on Windows (`#[cfg(target_os = "windows")]`). Implements native Win32/DXGI telemetry without external kernel drivers or unverified DLLs:
  - `cpu.rs`: Reads processor branding string from `HKLM\HARDWARE\DESCRIPTION\System\CentralProcessor\0\ProcessorNameString`. Calculates accurate multi-core CPU utilization via monotonic 64-bit `FILETIME` arithmetic from `GetSystemTimes(&idle, &kernel, &user)`. On Windows, `KernelTime` includes `IdleTime`, so true busy time is `(delta_kernel + delta_user) - delta_idle`. Zero elapsed deltas and backwards timestamps safely return `None`.
  - `memory.rs`: Queries total physical and available RAM via `GlobalMemoryStatusEx(&mut MEMORYSTATUSEX)` and computes used/total MB.
  - `gpu.rs`: Enumerates display adapters using DXGI 1.3 (`CreateDXGIFactory1`, `EnumAdapters1`), filtering software rasterizers (`DXGI_ADAPTER_FLAG_SOFTWARE`) and extracting dedicated video memory from `DXGI_ADAPTER_DESC1`. Aggregates live GPU load via `GpuPerformanceCollector`, which opens a persistent Performance Data Helper (PDH) query against `\GPU Engine(*)\Utilization Percentage` with double-precision counter formatting.
  - `battery.rs`: Samples battery charge percentage, AC line status, and charging flags via `GetSystemPowerStatus(&mut SYSTEM_POWER_STATUS)`.
  - `sensors.rs`: Defines structured `TemperatureReading`, `TemperatureSource` (`AcpiThermalZone`, `CpuPackage`, `GpuCore`, `Unavailable`), and `FanReading` models. Strictly enforces **honest absence** (MON-01, ARCH-03): unprivileged Windows userland lacks thermal telemetry without ring-0 kernel drivers, so `read_temperature_sensors()` returns `(None, None)` and `read_fan_rpms()` returns an empty vector rather than fabricating fake `0.0 °C` or dummy RPM numbers.
  - `device.rs`: Discovers system manufacturer, product model, and motherboard board ID via registry key `HARDWARE\DESCRIPTION\System\BIOS\BaseBoardProduct` for profile matching.
- **`MockHardware`** ([`src/hardware/mock.rs`](file:///home/omprakash/Documents/zenomen/src/hardware/mock.rs)): Deterministic telemetry simulator activated via the `ZENOMEN_MOCK_SCENARIO` environment variable (`nominal`, `zero-capability`, `overheating`, `sensor-gap`). Allows comprehensive headless testing in CI and on machines without HP hardware.
- **`UnavailableHardwareProvider`** ([`src/hardware/unavailable.rs`](file:///home/omprakash/Documents/zenomen/src/hardware/unavailable.rs)): Honest zero-capability fallback stub selected when compiling for unsupported platforms (`#[cfg(not(any(target_os = "linux", target_os = "windows")))]`). Refuses sensor sampling with an informative error rather than guessing or fabricating zero values.

---

## Service & IPC Subsystem

### 1. Dual-OS Daemon Entry Point (`src/bin/zenomen-hw.rs`)
The `zenomen-hw` binary serves as the privileged hardware controller across both platforms:
- **On Linux:** Connects to the D-Bus system bus via `zbus`, registers the well-known name `com.zenomen.Hardware1`, and exports `HardwareService` at `/com/zenomen/Hardware1`. Every mutating call invokes `PolkitVerifier::check_authorization` against `com.zenomen.hardware.control`.
- **On Windows:** Integrates with the Windows Service Control Manager (SCM). Supports CLI flags:
  - `--install-service`: Registers `zenomen-hw.exe` with SCM as `SERVICE_WIN32_OWN_PROCESS` with `SERVICE_AUTO_START` under `LocalSystem`.
  - `--uninstall-service`: Marks service for deletion in SCM.
  - `--console` / `-c`: Starts service directly in standalone interactive console mode without SCM dispatching.
  - Default invocation: Calls `run_service()` which invokes `StartServiceCtrlDispatcherW`. If invoked directly outside SCM (returning `ERROR_FAILED_SERVICE_CONTROLLER_CONNECT`), automatically falls back to standalone console mode.

### 2. SCM Service Lifecycle (`src/service/windows/lifecycle.rs`)
The Windows daemon implements full Win32 Service Control Manager lifecycle management:
- **Service Name & Display:** Service name is `ZenOmenHardwareService`; display name is `ZenOmen Hardware Control Service`.
- **Dispatcher:** `StartServiceCtrlDispatcherW` initializes the service entry table binding `service_main`.
- **Control Handler:** `service_main` registers `service_ctrl_handler` via `RegisterServiceCtrlHandlerExW`.
- **Status Reporting:** `report_service_status` tracks state transitions (`SERVICE_START_PENDING`, `SERVICE_RUNNING`, `SERVICE_STOP_PENDING`, `SERVICE_STOPPED`) with checkpoint increments and wait hints via `SetServiceStatus`.
- **Stop Signal:** SCM stop and shutdown controls (`SERVICE_CONTROL_STOP`, `SERVICE_CONTROL_SHUTDOWN`) store true into `SERVICE_STOP_SIGNAL: Arc<AtomicBool>`, allowing the named pipe server loop to exit gracefully.

### 3. Windows Named Pipe Server & IPC Subsystem (`src/service/windows/`)
- **Endpoint:** Local named pipe at [`\\.\pipe\ZenOmen.Hardware.v1`](file:///home/omprakash/Documents/zenomen/src/service/windows/pipe.rs) (`PIPE_NAME`), configured in duplex message mode (`PIPE_ACCESS_DUPLEX | PIPE_TYPE_MESSAGE | PIPE_READMODE_MESSAGE | PIPE_WAIT`) with a maximum of 4 concurrent instances. `FILE_FLAG_OVERLAPPED` is omitted to guarantee synchronous, blocking I/O compatibility.
- **Framing & Limits:** Strict 64 KB message framing ([`src/service/windows/protocol.rs`](file:///home/omprakash/Documents/zenomen/src/service/windows/protocol.rs), `MAX_PAYLOAD_SIZE = 65536`). Every message is preceded by a 4-byte big-endian payload length. Payloads exceeding 64 KB are dropped and rejected with `HardwareControlError::ProtocolError`.
- **DACL SDDL Security:** The pipe security descriptor ([`src/service/windows/security.rs`](file:///home/omprakash/Documents/zenomen/src/service/windows/security.rs)) is initialized with explicit SDDL:
  ```text
  D:(A;;GA;;;SY)(A;;GA;;;BA)(A;;GRGW;;;IU)
  ```
  - `(A;;GA;;;SY)`: Generic All access for `NT AUTHORITY\SYSTEM` (the service itself).
  - `(A;;GA;;;BA)`: Generic All access for `BUILTIN\Administrators`.
  - `(A;;GRGW;;;IU)`: Generic Read and Write access for `NT AUTHORITY\Interactive Users`.
  - Network and remote callers (`NU`, `AN`) are denied access by default.
- **Session Authentication:** Upon client connection, `verify_client_session` impersonates the named-pipe client via `ImpersonateNamedPipeClient`, queries the client token's `TokenSessionId` via `OpenThreadToken` + `GetTokenInformation`, and immediately verifies it against `WTSGetActiveConsoleSessionId()`. Reverts impersonation via `RevertToSelf()` and closes token handles. Callers originating from non-interactive sessions, remote desktop sessions (RDP), SSH terminals, or background services are rejected with `HardwareControlError::AccessDenied`.
- **Server Execution Loop (`run_pipe_server`):** [`src/service/windows/server.rs`](file:///home/omprakash/Documents/zenomen/src/service/windows/server.rs) manages synchronous connection acceptance (`ConnectNamedPipe`), session validation, framed payload reading (`ReadFile`), request dispatching (`handle_request`), response encoding, and transmission (`WriteFile` + `FlushFileBuffers`). Graceful shutdown is triggered by `stop_signal`.

### 4. Hardware Safety & Physical Validation Policy (`src/service/windows/authorization.rs`, `profiles.rs`)
Firmware mutations are strictly constrained to prevent motherboard damage, EC lockup, or firmware corruption:
- **Board Profile Catalogue (`BoardProfile`):** Motherboard product IDs read from `HARDWARE\DESCRIPTION\System\BIOS\BaseBoardProduct` resolve to validated profiles (`88F8` for OMEN AMD, `8A24` for OMEN Intel, `8A3E` for Victus AMD/Intel, `88D0` for OMEN 15 2020 Intel). Unknown motherboards return `None`, causing all mutation commands to be rejected and restricting the daemon to safe read-only telemetry.
- **Physical Validation Evidence (`BoardValidation`):** Defined in [`src/service/windows/profiles.rs`](file:///home/omprakash/Documents/zenomen/src/service/windows/profiles.rs), tracking:
  - `board_id` and `bios_version`
  - `fan_read_verified` / `fan_write_verified`
  - `power_read_verified` / `power_write_verified`
  - `rgb_read_verified` / `rgb_write_verified`
  - `validation_record_version`
  - Baseline `BoardValidation::new_unvalidated` leaves all write flags `false`.
  - Bench-validated benches use `BoardValidation::new_verified`.
- **Mutation Authorization Gate (`authorize_mutation`):**
  - Read-only queries (`GetCapabilities`, `GetFanStatus`, `GetPowerProfiles`, `GetPowerProfile`) always succeed regardless of validation record status.
  - Mutations (`SetFanMode`, `SetPowerProfile`, `ApplyPreset`, `ApplyRgbProfile`, `SetRgbEnabled`) require matching `board_id` and explicit verification flags (`fan_write_verified`, `power_write_verified`, `rgb_write_verified`).
  - Fan modes are restricted to `auto` and `max` (and `max` is permitted only if `board.has_max_fan` is true and `validation.fan_write_verified` is true; `manual` curves are rejected).
  - Power profiles must match advertised entries in `board.supported_power_profiles`.
  - Presets (`quiet`, `balanced`, `gaming`) require matching power and fan write verifications.
  - Keyboard RGB commands verify zone capacity and `rgb_write_verified`.
  - Any request targeting an unverified board or lacking validation evidence returns `HardwareControlError::UnsupportedMode`.

### 5. HP WMI Fast Command Protocol (`src/service/windows/hp_wmi.rs`)
On Windows, HP OMEN and Victus laptops expose platform management through ACPI BIOS WMI methods:
- **Namespace:** `ROOT\WMI`
- **Class:** `hpqBIntM` (GUID `{5FB7F034-2C63-45E9-BE91-3D44E2C707E4}`)
- **Method:** `hpqBMtdx` evaluating commands:
  - `0x10`: Fan Count
  - `0x11`: Fan Speeds (RPM readings)
  - `0x1A`: Performance Mode / Thermal Profile selection
  - `0x26`: Max Fan Status
  - `0x27`: Max Fan Set (0 = auto, 1 = max)
  - `0x28`: System Design Data (EC capabilities query)
  - `0x20009`: Four-Zone RGB keyboard color control
- Specification details are documented in [`docs/architecture/HP_WMI_PROTOCOL.md`](file:///home/omprakash/Documents/zenomen/docs/architecture/HP_WMI_PROTOCOL.md) and [`docs/architecture/WINDOWS_FUNCTION_CONTRACTS.md`](file:///home/omprakash/Documents/zenomen/docs/architecture/WINDOWS_FUNCTION_CONTRACTS.md).

---

## Desktop Subsystem

### 1. Linux Desktop Integration ([`src/desktop/`](file:///home/omprakash/Documents/zenomen/src/desktop/))
- **Single Instance:** Requests ownership of `com.zenomen.App` on the session bus with `DO_NOT_QUEUE`. Secondary instances invoke `PresentWindow` and immediately exit.
- **System Tray:** Registers a StatusNotifierItem (`/StatusNotifierItem`, `org.kde.StatusNotifierItem`) and DBusMenu (`/MenuBar`, `com.canonical.dbusmenu`) on the session bus. Missing tray watcher is handled gracefully.
- **Autostart:** Writes or removes XDG desktop file at `~/.config/autostart/zenomen.desktop`.
- **Releases:** Invokes `xdg-open` pointing to project GitHub releases.

### 2. Windows Desktop Integration ([`src/desktop/windows/`](file:///home/omprakash/Documents/zenomen/src/desktop/windows/))
- **Single Instance Mutex:** Acquires a session-scoped Win32 Mutex named `Local\ZenOmen.SingleInstance` ([`src/desktop/windows/single_instance.rs`](file:///home/omprakash/Documents/zenomen/src/desktop/windows/single_instance.rs)). If `ERROR_ALREADY_EXISTS` is encountered, finds the existing window via `FindWindowW` and restores it with `ShowWindow(SW_RESTORE)` before exiting.
- **Dedicated Tray Pump Thread:** Win32 system tray notifications require window message queues strictly affine to the thread that registered them. [`src/desktop/windows/tray.rs`](file:///home/omprakash/Documents/zenomen/src/desktop/windows/tray.rs) spawns a dedicated background thread named `zenomen-win32-tray-pump` that registers a hidden window class (`ZenOmenTrayMsgWindow`), registers the notification icon via `Shell_NotifyIconW`, and executes a standard `GetMessageW` / `TranslateMessage` / `DispatchMessageW` loop. Context menu actions (`Open Window`, `Fan Auto`, `Fan Max`, `Quit`) route to core via thread-safe `mpsc` channels.
- **Window Handle Bound Notifications:** [`src/desktop/windows/notifications.rs`](file:///home/omprakash/Documents/zenomen/src/desktop/windows/notifications.rs) provides `show_notification_with_hwnd(title, message, hwnd)`. `TrayHandle::show_notification` passes its own `hwnd` so that `nid.hWnd = hwnd` is explicitly set in `Shell_NotifyIconW(NIM_MODIFY)`, binding balloon/toast notifications to the active tray icon instance.
- **Registry Autostart:** Manages user autostart by writing `zenomen.exe --minimized` into the Windows Registry under `HKCU\Software\Microsoft\Windows\CurrentVersion\Run` ([`src/desktop/windows/autostart.rs`](file:///home/omprakash/Documents/zenomen/src/desktop/windows/autostart.rs)).
- **Known Folder Paths:** Resolves user roaming app data (`%APPDATA%\ZenOmen`) and local app data (`%LOCALAPPDATA%\ZenOmen\logs`) via `SHGetKnownFolderPath` with `FOLDERID_RoamingAppData` and `FOLDERID_LocalAppData` ([`src/desktop/windows/paths.rs`](file:///home/omprakash/Documents/zenomen/src/desktop/windows/paths.rs)).
- **ShellExecute Releases:** Opens release URLs via Win32 `ShellExecuteW(..., "open", ...)`.

---

## Presentation Layer & GPUI-CE Scaffold

ZenOmen maintains an active, production-ready Slint GUI while advancing toward a unified cross-platform GPUI-CE rendering architecture:

### 1. Active Production GUI (Slint 1.9 / 1.17)
- Located in `ui/` (`ui/app.slint`, `ui/screens/*.slint`, `ui/components/*.slint`, `ui/theme/*.slint`).
- Driven from `src/main.rs` and bridged by `src/app.rs`.
- Purely declarative UI with fluent styling, bundled offline Inter and JetBrains Mono fonts, and property bindings updated inside `slint::invoke_from_event_loop`.
- Slint is feature-gated behind `ui` in `Cargo.toml`. `zenomen-hw` builds with `--no-default-features` and excludes Slint completely, keeping the daemon binary under 3 MB and zero GUI dependencies.

### 2. Target Cross-Platform Architecture Scaffold (`src/ui/`)
In accordance with **ADR-002** ([`docs/decisions/ADR-002-gpui-ce-migration.md`](file:///home/omprakash/Documents/zenomen/docs/decisions/ADR-002-gpui-ce-migration.md)), the codebase maintains a complete, production-grade GPUI-CE scaffold under [`src/ui/`](file:///home/omprakash/Documents/zenomen/src/ui/):
- **Application Shell & View-Model Bridge:** `src/ui/app.rs` defines `GpuiApp`, managing typed element trees and mapping `AppCommand` and `AppEvent` messages without direct hardware dependencies.
- **Design Tokens & Theme:**
  - `src/ui/theme/colors.rs`: Canonical dark-palette RGB/RGBA tokens matching Slint styling.
  - `src/ui/theme/spacing.rs`: Standardized padding, margin, radius, and elevation constants.
  - `src/ui/theme/typography.rs`: Font scale, line height, and weight tokens.
- **Component Contracts:**
  - `src/ui/components/sidebar.rs`: 6-screen navigation sidebar with app versioning.
  - `src/ui/components/status_strip.rs`: System health, daemon connection, and degradation alerts.
  - `src/ui/components/telemetry_chart.rs`: Triple-metric historical graph with SVG geometry rendering and gap handling.
  - `src/ui/components/metric_card.rs`, `action_button.rs`, `value_readout.rs`: Reusable interactive widgets.
- **Screen View Models:**
  - `src/ui/screens/overview.rs`: High-level dashboard showing CPU/GPU loads and temperatures.
  - `src/ui/screens/cooling.rs`: Fan speed tachometers, fan curve display, auto/max modes.
  - `src/ui/screens/performance.rs`: Power profiles (quiet, balanced, performance) and presets.
  - `src/ui/screens/lighting.rs`: 4-zone RGB keyboard controls and color presets.
  - `src/ui/screens/settings.rs`: Polling intervals, autostart, minimize-to-tray toggles.
  - `src/ui/screens/diagnostics.rs`: System DMI data, ACPI tables, and atomic JSON export.
- **Migration & Parity Gates:** Slint remains active until all 9 checks of the ADR-002 release gate pass on both Linux (Wayland/X11) and Windows (Win32/DirectX).

---

## Component Responsibilities

| Component | Responsibility | File |
|-----------|----------------|------|
| GUI entry point | Logging, config load, single instance, provider selection, thread orchestration, ordered shutdown | `src/main.rs` |
| Slint bridge | Queue UI callbacks as `AppCommand`; fold `AppEvent` into `AppState`; publish properties on UI thread | `src/app.rs` |
| Slint UI shell | Declarative window layout, navigation sidebar, 6-screen router; zero hardware calls | `ui/app.slint` |
| GPUI-CE scaffold | Target cross-platform element tree, view models, and component contracts | `src/ui/*` |
| Event contract | `AppCommand` (UI→core), `AppEvent` (core→UI), compile-time `ScreenId` | `src/core/events.rs` |
| Single-writer state | `AppState` + `apply(&AppEvent)` + degradation/status derivation | `src/core/state.rs` |
| Monitoring worker | 1 Hz deadline loop, on-demand refresh, config persistence, atomic `InFlightGuard` mutation slot | `src/core/worker.rs` |
| Telemetry history | Bounded `VecDeque` (120 points) and SVG path geometry generation | `src/core/telemetry.rs` |
| Diagnostics | System DMI/procfs/WMI inspection and atomic JSON report export | `src/core/diagnostics.rs` |
| Hardware contract | `HardwareProvider` trait, `HardwareCapabilities`, `TelemetrySnapshot` | `src/hardware/provider.rs` |
| Sensor registry | Canonical CPU-temp/fan/ACPI/battery discovery on Linux | `src/hardware/registry.rs` |
| Linux hardware backend | Linux procfs/sysfs/DRM/NVIDIA sensor sampling | `src/hardware/linux/*` |
| Windows hardware backend | Windows native Win32/DXGI/PDH sensor sampling (`WindowsHardwareProvider`) | `src/hardware/windows/*` |
| Windows GPU collector | DXGI 1.3 adapter enumeration + persistent PDH `\GPU Engine(*)\Utilization Percentage` query | `src/hardware/windows/gpu.rs` |
| Windows sensor models | Structured `TemperatureReading`, `TemperatureSource`, `FanReading`, honest absence | `src/hardware/windows/sensors.rs` |
| Mock / unavailable backends | Deterministic simulation scenarios and honest zero-capability fallback stub | `src/hardware/mock.rs`, `src/hardware/unavailable.rs` |
| Linux D-Bus server | `com.zenomen.Hardware1` interface and Polkit authority check | `src/service/contract.rs`, `src/service/polkit.rs` |
| Linux sysfs writes | Verified sysfs writes, exponential-backoff retries, preset execution | `src/service/backend.rs` |
| Linux D-Bus client | Blocking `zbus` client used by the Linux GUI worker | `src/service/client.rs` |
| Windows service lifecycle | Windows SCM service registration, `StartServiceCtrlDispatcherW`, status reporting, `--console` mode | `src/service/windows/lifecycle.rs` |
| Windows named pipe server | Named pipe endpoint `\\.\pipe\ZenOmen.Hardware.v1`, synchronous 64 KB framing, loop | `src/service/windows/server.rs`, `pipe.rs` |
| Windows security & auth | DACL SDDL, interactive console session verification, board safety profile check | `src/service/windows/security.rs`, `authorization.rs` |
| Motherboard validation | `BoardValidation` physical verification evidence & `BoardProfile` catalogue | `src/service/windows/profiles.rs` |
| HP WMI BIOS driver | ACPI BIOS WMI `hpqBIntM` method execution for fan/power/RGB controls | `src/service/windows/hp_wmi.rs` |
| Windows named pipe client | Named pipe client connecting GUI to Windows service | `src/service/windows/client.rs` |
| Linux desktop integration | Session D-Bus single instance, SNI tray, XDG autostart | `src/desktop/*` |
| Windows desktop integration | Win32 Mutex, dedicated tray pump thread, HKCU Run autostart, ShellExecute | `src/desktop/windows/*` |
| Windows tray pump | Thread-affine Win32 message pump thread (`zenomen-win32-tray-pump`) | `src/desktop/windows/tray.rs` |
| Windows notifications | Balloon/toast notifications via `Shell_NotifyIconW` bound to tray `hwnd` | `src/desktop/windows/notifications.rs` |
| Config persistence | TOML configuration with atomic save and corrupt-file backup | `src/config/*` |
| RGB domain | 4-zone color model, strict hex validation, OpenOMEN profile migration | `src/rgb/*` |
| Logging | Early non-blocking rolling file appender in platform state directory + stderr | `src/logging.rs` |
| Windows WiX packaging | WiX v4/v5 installer defining GUI and SCM service components | `packaging/windows/ZenOmen.wxs` |
| Architecture contracts | Win32 function contracts (`WINDOWS_FUNCTION_CONTRACTS.md`), cross-platform design | `docs/architecture/*` |
| Architecture decisions | Formal ADRs: UI framework (`ADR-001`, `ADR-002`), Windows hardware & security | `docs/decisions/*` |
| Verification protocols | Cross-platform test matrices and motherboard validation guidelines | `docs/testing/*` |

---

## Pattern Overview

- **Layered Hexagonal Architecture:** Application core is strictly separated from presentation and hardware drivers by typed command/event boundaries.
- **Single-Writer Discipline:** All UI state changes must flow through `AppState::apply(&AppEvent)` ([`src/core/state.rs`](file:///home/omprakash/Documents/zenomen/src/core/state.rs)); the presentation layer receives only formatted values.
- **Read/Write Privilege Boundary:** Sensor sampling is strictly unprivileged and in-process via `HardwareProvider`. Hardware mutation is strictly privileged and crosses an IPC channel (D-Bus or Named Pipe).
- **Atomic In-Flight Concurrency Control (`InFlightGuard`):** Concurrency conflicts across user timeouts are prevented via an RAII guard held in the RPC thread until the physical hardware write completes ([`src/core/worker.rs`](file:///home/omprakash/Documents/zenomen/src/core/worker.rs)).
- **Honest Absence:** Missing sensors return `None`/empty, never synthesized zeros. Unsupported platforms use `UnavailableHardwareProvider` to state zero capabilities honestly. Windows thermal sensors return `(None, None)` without kernel ring-0 drivers.
- **Thread-Affine Windows Tray:** Dedicated Win32 thread (`zenomen-win32-tray-pump`) handles `Shell_NotifyIconW` message loops to avoid message queue blocking or starvation, and notification window handle binding ensures notifications bind to the active tray icon.
- **Physical Validation Safety Gate:** Firmware mutations on Windows are blocked unless explicitly authorized against a physical `BoardValidation` record, keeping unknown or unverified motherboards strictly read-only.

---

## Data Flow

### 1. Primary Telemetry Path (1 Hz Sample → UI Render)
1. `spawn_monitoring_worker` starts the monitoring loop ([`src/core/worker.rs`](file:///home/omprakash/Documents/zenomen/src/core/worker.rs)).
2. Every 1 second (or immediately on `RefreshTelemetry`), the worker invokes `HardwareProvider::sample_telemetry()`.
3. **Sampling:**
   - On Linux: `LinuxHardwareProvider` parses `/proc/stat`, cpufreq, hwmon temperatures/fans, and DRM/NVIDIA GPU nodes.
   - On Windows: `WindowsHardwareProvider` samples `GetSystemTimes`, `GlobalMemoryStatusEx`, DXGI adapters, persistent PDH `\GPU Engine(*)\Utilization Percentage` via `GpuPerformanceCollector`, and `GetSystemPowerStatus`.
4. Telemetry is packed into an immutable `TelemetrySnapshot` and sent via `mpsc::Sender<AppEvent>` to the event pump.
5. `ZenOmenApp::handle_event` updates `AppState` under a mutex, pushes points into `TelemetryHistory`, and regenerates SVG sparklines.
6. `apply_to_ui` schedules property updates on the UI thread via `slint::invoke_from_event_loop`.

### 2. Hardware Mutation Path (User Click → Hardware Write)
1. User activates a control (fan mode, power profile, RGB preset) in the UI.
2. Callback queues an `AppCommand` into the bounded depth-8 command channel.
3. Worker receives command and enforces `AtomicBool` concurrency check via `InFlightGuard`.
4. **IPC Execution:**
   - **On Linux:** Worker invokes blocking `zbus` call to `com.zenomen.Hardware1`. Privileged daemon checks Polkit (`com.zenomen.hardware.control`) and writes sysfs with exponential backoff.
   - **On Windows:** Worker invokes `WindowsServiceClient::send_request`. Named pipe server verifies DACL, verifies console session ID, checks board profile authorization and `BoardValidation` physical verification flags, and executes HP WMI `hpqBMtdx` method.
5. Confirmation event (`FanModeChanged`, `PowerProfileChanged`, `RgbProfileApplied`) returns over IPC, updates `AppState`, and reflects in the UI.

### 3. Startup Path
1. `logging::init()` initializes non-blocking rotating file logging (Linux: `~/.local/state/zenomen/logs`; Windows: `%LOCALAPPDATA%\ZenOmen\logs`).
2. Single-instance check runs:
   - Linux: Session D-Bus `com.zenomen.App` request.
   - Windows: Win32 session named Mutex (`Local\ZenOmen.SingleInstance`).
3. User configuration loads from platform config directory.
4. Tray icon registers (Linux: SNI/DBusMenu; Windows: dedicated `zenomen-win32-tray-pump` Win32 message pump thread).
5. `HardwareProvider` selected based on target OS (`LinuxHardwareProvider`, `WindowsHardwareProvider`, or mock/unavailable).
6. Initial capabilities probed, window displayed, and background worker threads spawned.

---

## IPC Contracts

### 1. Linux D-Bus Contract (`com.zenomen.Hardware1`)
- **Bus:** System Bus
- **Path:** `/com/zenomen/Hardware1`
- **Interface:** `com.zenomen.Hardware1`
- **Polkit Action:** `com.zenomen.hardware.control`
- **Methods:** `GetCapabilities()`, `GetFanStatus()`, `SetFanMode(s)`, `GetPowerProfiles()`, `GetPowerProfile()`, `SetPowerProfile(s)`, `ApplyPreset(s)`, `ApplyRgbProfile(s)`, `SetRgbEnabled(b)`.

### 2. Windows Named Pipe Contract (`\\.\pipe\ZenOmen.Hardware.v1`)
- **Endpoint:** Local Named Pipe `\\.\pipe\ZenOmen.Hardware.v1`
- **Security:** DACL SDDL `D:(A;;GA;;;SY)(A;;GA;;;BA)(A;;GRGW;;;IU)` + active console session matching (`WTSGetActiveConsoleSessionId`).
- **Framing:** Synchronous length-prefixed (4-byte BE) JSON frames ([`src/service/windows/protocol.rs`](file:///home/omprakash/Documents/zenomen/src/service/windows/protocol.rs)) capped at 64 KB (`MAX_PAYLOAD_SIZE = 65536`).
- **Messages:**
  - `HardwareRequest`: `GetCapabilities`, `GetFanStatus`, `SetFanMode { mode }`, `GetPowerProfiles`, `GetPowerProfile`, `SetPowerProfile { profile }`, `ApplyPreset { preset }`, `ApplyRgbProfile { profile_json }`, `SetRgbEnabled { enabled }`.
  - `HardwareResponse`: `Capabilities`, `FanStatus { mode, rpms }`, `FanModeSet`, `PowerProfiles`, `PowerProfile`, `PowerProfileSet`, `PresetApplied`, `RgbProfileApplied`, `RgbEnabledSet`, `Error(String)`.
- **Validation Gate:** Evaluated through `authorize_mutation(req, board, validation)` against `BoardProfile` and physical `BoardValidation` record.

---

## Architectural Constraints

- **No Async in Desktop GUI:** The GUI remains strictly on `std::thread` + `mpsc` channels and synchronous IPC clients to avoid runtime bloat and keep memory within the 40 MB RSS budget.
- **Strict Seam Isolation:** Windows headers and Win32 calls are completely excluded from Linux builds; Linux D-Bus, sysfs, and Polkit code are completely excluded from Windows builds.
- **Single-Writer Discipline:** State mutations must originate exclusively from `AppState::apply(&AppEvent)`.
- **Privilege Separation:** GUI never runs as administrator or root. Daemon performs no GUI rendering or telemetry polling.
- **Fail-Safe Motherboard Safety:** Unrecognized motherboards on Windows default to read-only operation to prevent hardware damage, and recognized motherboards require physical `BoardValidation` evidence before mutating firmware.

---

*Architecture analysis: 2026-11-08*
