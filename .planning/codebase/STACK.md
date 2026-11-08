---
last_mapped_commit: 4691619f4e741bd5944f2085fa455b135ff873af
last_mapped_at: 2026-11-08
---
# Technology Stack

**Analysis Date:** 2026-11-08
**Branch:** `fix/zenomen-reliability`
**HEAD Commit:** `4691619f4e741bd5944f2085fa455b135ff873af`

## Summary

ZenOmen is a high-efficiency native hardware control center for HP OMEN and Victus laptops, engineered for both Linux and Windows 10/11. The project produces two Rust binaries from a single Cargo package:

- `zenomen` — Unprivileged desktop GUI application (`src/main.rs`, `src/app.rs`, `ui/`), gated by `required-features = ["ui"]`. Runs under user privileges with no administrative elevation required. Active GUI is Slint 1.9 / 1.18.1 with optional cargo feature gating, with a staged GPUI-CE migration path (`src/ui/` scaffold, ADR-002, ADR-004).
- `zenomen-hw` — Privileged, headless hardware control daemon / Windows Service (`src/bin/zenomen-hw.rs`, `src/service/`), compiled without linking Slint via `--no-default-features`. On Linux, executes as a systemd root service exposing system D-Bus (`com.zenomen.Hardware1`) and Unix domain socket (`/run/zenomen-hw/hw.sock`). On Windows, executes under `NT AUTHORITY\LocalSystem` as SCM service `ZenOmenHardwareService` (`src/service/windows/lifecycle.rs`) communicating via Named Pipe (`\\.\pipe\ZenOmen.Hardware.v1`).

The stack strictly enforces a zero-bloat philosophy: no webview, no JavaScript runtime (Electron/Tauri avoided), no async runtime (Tokio is not a direct runtime dependency), no database engine, and no HTTP client. Mutating hardware calls are strictly verified, bounded, and decoupled from the presentation layer.

## Target Platforms

### Linux Platform
- **Target OS:** Ubuntu 22.04 LTS+, modern systemd-based Linux distributions.
- **Kernel Requirements:** Linux Kernel 5.15+ (x86_64).
- **Core Interfaces:** systemd (service lifecycle, sandboxing), D-Bus (zbus 4 / zbus 5, resolved to `5.19.0` with `blocking` feature; system bus `com.zenomen.Hardware1`, session bus `com.zenomen.App` / `StatusNotifierItem`), Unix domain socket (`/run/zenomen-hw/hw.sock`), Polkit (`org.freedesktop.PolicyKit1`).
- **Hardware Telemetry & Control:** `procfs` (`/proc/stat`, `/proc/meminfo`, `/proc/cpuinfo`), `sysfs` (`/sys/class/hwmon`, `/sys/devices/platform/hp-wmi`, `/sys/firmware/acpi`), DMI/SMBIOS, DRM (`/sys/class/drm`).
- **GPU Telemetry:** AMD/Intel DRM sysfs; NVIDIA proprietary driver sampled via dedicated `NvidiaSampler` thread invoking `nvidia-smi` / NVML on a decoupled channel (`mpsc::sync_channel(1)`).

### Windows Platform
- **Target OS:** Windows 10 (22H2 19045+) and Windows 11 (24H2 26100+).
- **Compilation Target:** `x86_64-pc-windows-msvc`.
- **Win32 API Bindings:** `windows-sys` 0.59 with explicit features:
  - `Win32_System_Performance` (for PDH performance counters)
  - `Win32_System_Services` (for SCM lifecycle and service control dispatcher)
  - `Win32_System_Pipes` (for named pipe server and client IPC)
  - `Win32_Storage_FileSystem` (for pipe access flags and file operations)
  - `Win32_Security` & `Win32_Security_Authorization` (for SDDL DACL parsing and caller token impersonation)
  - `Win32_UI_Shell` (for `Shell_NotifyIconW` tray and notification dispatch)
  - `Win32_System_Wmi` & `Win32_System_Com` (for HP WMI ACPI BIOS communication)
  - `Win32_Foundation`, `Win32_System_SystemInformation`, `Win32_System_Power`, `Win32_System_Threading`, `Win32_System_Memory`, `Win32_System_Registry`, `Win32_System_LibraryLoader`, `Win32_System_RemoteDesktop`, `Win32_System_IO`, `Win32_UI_WindowsAndMessaging`, `Win32_Graphics_Gdi`.
- **Telemetry Engines:**
  - CPU: `GetSystemTimes` for monotonic delta utilization; `CallNtPowerInformation` for frequency; PDH fallback.
  - Memory: `GlobalMemoryStatusEx` (`ullTotalPhys`, `ullAvailPhys`).
  - GPU: DXGI 1.3 (`CreateDXGIFactory1`, `EnumAdapters1`, `QueryVideoMemoryInfo`) for adapter info and dedicated VRAM; persistent `GpuPerformanceCollector` querying `\GPU Engine(*)\Utilization Percentage` via PDH (Performance Data Helper).
  - Battery: `GetSystemPowerStatus` (`SYSTEM_POWER_STATUS`).
- **Hardware Control:** Direct ACPI BIOS invocation via WMI Fast Command (`hpqBIntM`, GUID `{5FB7F034-2C63-45E9-BE91-3D44E2C707E4}`, method `hpqBMtdx`). Zero third-party ring-0 kernel drivers required (ADR-002). Enforces `BoardValidation` gating before mutations.

## Languages

**Primary:**
- **Rust (edition 2024):** Core logic, hardware providers, services, IPC, and tests across Linux and Windows (`Cargo.toml:4`).
- **Slint DSL 1.x:** 23 `.slint` files under `ui/` (`ui/app.slint`, `ui/screens/`, `ui/components/`, `ui/theme/`), compiled to native Rust at build time via `slint-build` in `build.rs` when the `ui` feature is active.

**Secondary / Packaging:**
- **WiX Toolset XML (v4/v5):** Windows MSI installer definition (`packaging/windows/wix/ZenOmen.wxs`).
- **TOML:** Application settings (`~/.config/zenomen/config.toml` or Windows `%APPDATA%\ZenOmen\config.toml`) and Cargo manifests.
- **JSON:** Wire protocol payloads, RGB profiles, and diagnostics exports.
- **XML:** Linux D-Bus bus policy and Polkit authorization policy (`data/dbus/`, `data/polkit/`).
- **PowerShell / Bash:** Packaging scripts and Linux budget benchmarks.

## Runtime

**Environment:**
- **Native OS Execution:** Unmanaged binary execution; no virtual machine, JIT, or garbage collector.
- **Concurrency & Threads:**
  - GUI main thread runs the Slint windowing event loop (`winit`).
  - Dedicated background worker thread `HardwareWorker` (`src/core/worker.rs`) isolates 1000ms hardware sampling ticks from UI rendering.
  - On Windows, a dedicated thread runs `zenomen-win32-tray-pump` (`src/desktop/windows/tray.rs`) to maintain strict Win32 thread-affinity for `Shell_NotifyIconW` and hidden message windows (`ZenOmenTrayMsgWindow`).
  - Non-blocking logging thread managed by `tracing-appender` (`src/logging.rs`).
  - On Linux, dedicated `NvidiaSampler` thread `zenomen-nv-watch` (`src/hardware/linux/gpu.rs`) prevents GPU D3cold sleep states from blocking telemetry cycles.
  - Daemon uses `zbus::block_on` (Linux) or Windows SCM dispatch (`StartServiceCtrlDispatcherW`) / blocking named pipe server loop with synchronous I/O.
- **Process Memory Profile:** Release profile enforces `opt-level = "s"`, `lto = true`, `codegen-units = 1`, `panic = "abort"`, `strip = true` (`Cargo.toml:57-64`) to uphold the $\le 40\text{ MB}$ GUI RSS budget and $\le 15\text{ MB}$ daemon idle RSS budget.

**Package Management:**
- Cargo with committed authoritative `Cargo.lock` (lockfile version 4).
- CI runs on latest stable Rust with `clippy`, `rustfmt`, and `cargo-audit`.

## Frameworks & UI Architecture

### UI Framework
- **Slint 1.9 / 1.18.1 (Active Production GUI):**
  - Dependency: `slint = "1.17"` (caret requirement resolving to 1.18.1 in `Cargo.lock`), gated behind optional default feature `ui = ["dep:slint"]`.
  - Backends: `backend-winit` with `renderer-femtovg` (OpenGL 2.1+ / Direct3D). Qt and Skia backends are explicitly excluded to conserve memory.
  - Slint compiler: `slint-build` 1.18.1 in `build.rs` compiles `ui/app.slint` with Fluent styling when `ui` feature is enabled.
- **GPUI-CE Migration Path (ADR-002, ADR-004, `src/ui/` scaffold):**
  - Complete domain-decoupled UI view-model scaffold established under `src/ui/` (`src/ui/app.rs`, `src/ui/screens/`, `src/ui/components/`, `src/ui/theme/`).
  - Communicates with hardware exclusively through strongly typed `AppCommand` / `AppEvent` channels and `HardwareProvider` trait contracts, enabling seamless future migration to GPUI-CE without touching core domain or hardware modules.

### IPC & Privileged Services
- **Linux IPC:**
  - System D-Bus: `com.zenomen.Hardware1` (`zbus` 4 / `zbus` 5 with blocking feature, resolved to `5.19.0`).
  - Unix Domain Socket: `/run/zenomen-hw/hw.sock`.
  - Session D-Bus: `com.zenomen.App` for single-instance; `StatusNotifierItem` / `DBusMenu` for tray.
- **Windows IPC & SCM Service:**
  - Named Pipe: `\\.\pipe\ZenOmen.Hardware.v1`.
  - Wire Protocol Framing: **4-byte big-endian length prefix** (`u32::to_be_bytes`), followed by UTF-8 JSON payload strictly capped at 64 KB (`MAX_PAYLOAD_SIZE = 65536`). Synchronous blocking I/O (no `FILE_FLAG_OVERLAPPED`).
  - Security Descriptor & DACL: Explicit `PIPE_SDDL` access control (`D:(A;;GA;;;SY)(A;;GA;;;BA)(A;;GRGW;;;IU)`) granting Full Control to `LocalSystem` (`SY`) and `Builtin\Administrators` (`BA`), and Read/Write to `Interactive Users` (`IU`).
  - Caller Session Verification: Caller session token verified against `WTSGetActiveConsoleSessionId()` via `ImpersonateNamedPipeClient` / `GetTokenInformation(TokenSessionId)`.
  - Windows Service Control Manager (SCM): `ZenOmenHardwareService` registered in SCM via `src/service/windows/lifecycle.rs`, running under `NT AUTHORITY\LocalSystem` account with delayed auto-start support, lifecycle managed via `StartServiceCtrlDispatcherW`, `RegisterServiceCtrlHandlerExW`, and `SetServiceStatus` (with fallback to standalone console mode when run outside SCM).

## Key Dependencies

**Critical Dependencies:**
| Crate | Manifest Requirement | Resolved (`Cargo.lock`) | Purpose / Architectural Role |
|---|---|---|---|
| `slint` | `1.17` (caret) | `1.18.1` | Active production desktop GUI; optional behind `ui` feature. |
| `slint-build` | `1.17` (caret) | `1.18.1` | Build-time `.slint` compiler for `ui/app.slint`. |
| `windows-sys` | `0.59` | `0.59.0` | Comprehensive Win32/WMI/DXGI bindings for Windows telemetry, IPC, service, tray, and autostart (`Win32_System_Performance`, `Win32_System_Services`, `Win32_System_Pipes`, `Win32_Storage_FileSystem`, `Win32_Security`, `Win32_UI_Shell`, `Win32_System_Wmi`, `Win32_System_Com`). |
| `zbus` | `5` (D-Bus zbus 4/5) | `5.19.0` | Linux D-Bus hardware service, client, Polkit integration, single instance, and tray. |
| `serde` | `1.0` + `derive` | `1.0.229` | Serde serialization for wire protocols, configuration, diagnostics, and RGB profiles. |
| `serde_json` | `1.0` | `1.0.154` | JSON payload serialization for D-Bus and Named Pipe framing. |
| `toml` | `0.8` | `0.8.23` | TOML parsing and serialization for `config.toml`. |
| `thiserror` | `2.0` | `2.0.21` | Strongly typed domain errors across hardware, service, desktop, and IPC layers. |
| `directories` | `6.0` | `6.0.0` | Cross-platform standard directories (XDG on Linux, `%APPDATA%` on Windows). |
| `tracing` | `0.1` | `0.1.44` | Structured application logging across GUI and daemon processes. |
| `tracing-subscriber` | `0.3` + `env-filter` | `0.3.23` | Logging configuration and runtime filter dispatch (`RUST_LOG`). |
| `tracing-appender` | `0.2` | `0.2.5` | Non-blocking daily rolling log file writer with bounded history. |
| `libc` | `0.2` | `0.2.191` | Unix-specific platform primitives (`cfg(unix)`). |

**Windows Build Dependencies:**
- `winres` **0.1.12** (`[target.'cfg(windows)'.build-dependencies]`): Embeds `data/icons/zenomen.ico` and application manifest into Windows executables (`build.rs`).

## Explicitly Absent Dependencies

- **No Web / Async Runtime:** No `tokio` direct dependency; no `actix`, no `hyper`, no `reqwest`.
- **No Webview / Chromium:** No Electron, Tauri, WRY, or CEF.
- **No Embedded Databases:** No SQLite, RocksDB, or SQLx.
- **No Ring-0 Kernel Drivers on Windows:** No WinRing0, InpOut32, or unsigned kernel drivers; all BIOS control uses ACPI WMI `hpqBIntM`.
- **No Third-Party Telemetry Daemons:** No dependency on Open Hardware Monitor or proprietary HP DLLs.

## Desktop Integrations

| Feature | Linux Implementation | Windows Implementation |
|---|---|---|
| **System Tray** | D-Bus `org.kde.StatusNotifierItem` + `com.canonical.dbusmenu` | Win32 `Shell_NotifyIconW` with dedicated message pump thread (`zenomen-win32-tray-pump`) and `hwnd` binding |
| **Autostart** | XDG autostart desktop entry (`~/.config/autostart/zenomen.desktop`) | Windows Registry Run key (`HKCU\Software\Microsoft\Windows\CurrentVersion\Run`) |
| **Single Instance** | Session D-Bus well-known name acquisition (`com.zenomen.App`) | Win32 Named Mutex (`Local\ZenOmen.SingleInstance`) + `FindWindowW` / `SetForegroundWindow` |
| **Elevation / Privileges** | Polkit (`com.zenomen.hardware.control`) via system D-Bus | Windows Service (`ZenOmenHardwareService`) under `NT AUTHORITY\LocalSystem` via Named Pipe DACL |
| **Update Notification** | Browser launch via `xdg-open` | Browser launch via `ShellExecuteW` (`src/desktop/windows/releases.rs`) |

## Packaging & CI/CD

### Packaging Formats
- **Linux:**
  - Debian package (`.deb`): Built via `cargo-packager`.
  - AppImage (`.AppImage`): Built via `cargo-packager`.
- **Windows:**
  - Windows Installer (`.msi`): WiX Toolset v4/v5 configuration (`packaging/windows/wix/ZenOmen.wxs`), installing `zenomen.exe` and configuring `ZenOmenHardwareService` with auto-start.
  - Portable Archive (`.zip`): Standalone release archive containing `zenomen.exe`, `zenomen-hw.exe`, `LICENSE`, and `README.md`.

### CI/CD Pipelines
- **`.github/workflows/ci.yml`:** Dual-OS matrix validating Ubuntu 22.04 (`ubuntu-latest`) and Windows 10/11 (`windows-latest`):
  - Code formatting (`cargo fmt --check`).
  - Zero-warning linter (`cargo clippy --all-targets --all-features -- -D warnings`).
  - Unit and integration tests (Linux headless via `xvfb-run`; Windows native test harness).
  - Dependency vulnerability scan (`cargo audit`).
- **`.github/workflows/release.yml`:** Automated multi-platform release pipeline building `.deb`, `.AppImage`, `.msi`, and `.zip` artifacts, validating SHA-256 checksums, and creating GitHub Releases.

---

*Stack analysis updated: 2026-11-08 (HEAD `4691619f4e741bd5944f2085fa455b135ff873af`)*
