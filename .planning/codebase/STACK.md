# Technology Stack

**Analysis Date:** 2026-11-09

## Languages

**Primary:**
- **Rust (Edition 2024):** Core application codebase across both targets (Linux and Windows). Powers the desktop GUI (`src/main.rs`), the privileged hardware service daemon (`src/bin/zenomen-hw.rs`), hardware telemetry providers (`src/hardware/`), service transport abstractions (`src/service/`), desktop environment integrations (`src/desktop/`), and configuration subsystem (`src/config/`). Built using Rust stable toolchain (1.85+).
- **Slint Markup (`ui/app.slint`):** Declarative UI markup compiled into native Rust code ahead-of-time by `slint-build` in `build.rs` using the `"fluent"` style.

**Secondary:**
- **TOML:** Configuration serialization and deserialization for user preferences (`~/.config/zenomen/config.toml` on Linux, `%LOCALAPPDATA%\ZenOmen\config.toml` on Windows) via `toml` 0.8 in `src/config/mod.rs` and `src/config/schema.rs`.
- **JSON:** Inter-process communication payload framing (`serde_json` in `src/service/windows/protocol.rs` and `src/service/protocol.rs`), RGB lighting profile serialization (`src/rgb/schema.rs`), and sanitized diagnostics report export (`src/core/diagnostics.rs`).
- **Bash:** Linux resource budget measurement harness (`scripts/measure-budget.sh`) validating RSS footprint against the ≤40 MB UI / ≤15 MB daemon memory constraints.
- **XML:**
  - D-Bus system service activation and bus policies (`data/dbus/com.zenomen.Hardware1.service`, `data/dbus/com.zenomen.Hardware1.conf`).
  - Polkit authorization policy definition (`data/polkit/com.zenomen.hardware.policy`).
  - WiX Toolset installer source (`packaging/windows/ZenOmen.wxs`) for Windows MSI packaging.
  - Windows application manifest embedded in PE resources (`build.rs`).
- **PowerShell / Batch:** Windows installer orchestration and packaging automation in CI (`.github/workflows/release.yml`).

## Runtime

**Environment:**
- Native desktop processes; no embedded virtual machine, JavaScript runtime, or heavy async runtime (deliberately zero Tokio or async-std dependencies).
- **Two-Process Model:**
  - `zenomen` (GUI Client): Unprivileged desktop process running in the user's interactive session. Manifest specifies `level="asInvoker"` (UAC elevation never requested).
  - `zenomen-hw` (Hardware Daemon): Privileged service process providing hardware read and mutation capabilities.
    - *Linux:* Runs as a systemd system service under `root` account (`data/systemd/zenomen-hw.service`).
    - *Windows:* Runs as a Windows Service under `LocalSystem` account (`ZenOmenHardwareService`) managed by the Service Control Manager (SCM), with console fallback (`--console`) for development.
- **Concurrency Architecture:**
  - Standard library threads (`std::thread`) and channels (`std::sync::mpsc`) exclusively (`src/core/worker.rs`, `src/app.rs`).
  - UI event loop marshalled via `slint::invoke_from_event_loop`.
  - Non-blocking telemetry ingestion worker runs on a dedicated background thread with thread-safe atomics (`std::sync::atomic`).

**Package Manager:**
- Cargo (Rust package manager).
- Lockfile: `Cargo.lock` is committed to version control; CI strictly uses `cargo build --locked` and `cargo test --locked`.

## Frameworks

**Core Frameworks:**
- **Slint (`slint`):** Declared `"1.17"` (caret requirement), locked to `1.18.1` in `Cargo.lock`.
  - Configured with `default-features = false`.
  - Enabled features: `std`, `compat-1-2`, `backend-winit`, `renderer-femtovg`.
  - Qt (`backend-qt`) and Skia (`renderer-skia`) backends are intentionally excluded to maintain the strict ≤40 MB peak RSS budget (`docs/RESOURCE-BUDGET.md`).
  - Transitive graphics/windowing crates: `winit` 0.30.13, `femtovg` 0.27.0.
- **zbus (`zbus`):** Declared `"5"`, locked to `5.19.0`. Configured with `features = ["blocking"]` only.
  - Powers Linux D-Bus system service (`com.zenomen.Hardware1` at `/com/zenomen/Hardware1` in `src/service/contract.rs`).
  - Powers Linux session D-Bus desktop integration (`src/desktop/single_instance.rs`, `src/desktop/tray.rs`).
- **windows-sys (`windows-sys`):** Declared `"0.59"`, locked to `0.59.0` (with 21 explicit Win32 feature modules).
  - Used on Windows for direct Win32 API interactions: SCM services, message-mode named pipes, security descriptors, token inspection, DXGI GPU enumeration, PDH counters, registry queries, and tray notification icons.
- **thiserror (`thiserror`):** Declared `"2.0"`, locked to `2.0.21` (transitive 1.0.69 also present).
  - Provides strongly-typed error enums across all subsystem layers (`src/service/types.rs`, `src/hardware/mod.rs`, `src/config/mod.rs`, `src/rgb/mod.rs`).

**Testing Framework:**
- Built-in `cargo test` test harness; no dev-dependencies or external mock frameworks declared in `Cargo.toml`.
- Deterministic fakes and in-memory test doubles:
  - `MockHardware` / `MockScenario` (`src/hardware/mock.rs`) for nominal, zero, and gap telemetry testing.
  - `FakeWmiAdapter` (`src/service/windows/hp_wmi.rs`) for deterministic queue-driven WMI BIOS simulation.
  - Extensive integration suites located under `tests/` (18 test files covering service lifecycle, D-Bus, named pipes, telemetry, degradation, and RGB).

**Build & Tooling:**
- `slint-build` (`1.17`, locked `1.18.1`): Build script dependency invoked in `build.rs` to compile `ui/app.slint` into Rust code when the `ui` feature is active.
- `winres` (`0.1`, locked `0.1.12`): Windows build dependency embedding application icons (`data/icons/zenomen.ico`), version metadata, and PE execution manifest (`asInvoker`, PerMonitorV2 DPI awareness) into `zenomen.exe`.
- `cargo-packager`: Packaging tool configured in `Cargo.toml` (`[package.metadata.packager]`) generating Linux `.deb` and `.AppImage` distribution bundles.
- WiX Toolset: Windows Installer XML toolset compiling `packaging/windows/ZenOmen.wxs` into MSI installer packages.

## Key Dependencies

**Production & Build Dependencies:**
| Crate | Declared | Locked | Purpose | Architecture / Platform |
|---|---|---|---|---|
| `slint` | `1.17` | 1.18.1 | Desktop UI presentation framework | All (GUI client only; behind `ui` feature) |
| `slint-build` | `1.17` | 1.18.1 | Slint markup compiler in `build.rs` | Build-dependency |
| `zbus` | `5` (`blocking`) | 5.19.0 | D-Bus system service and desktop IPC | Linux |
| `windows-sys` | `0.59` | 0.59.0 | Win32, COM, SCM, Named Pipes, DXGI, PDH | Windows (`cfg(windows)`) |
| `winres` | `0.1` | 0.1.12 | Windows icon, version, and manifest compilation | Windows build-dependency |
| `libc` | `0.2` | 0.2.191 | Unix POSIX calls (signals, process IDs) | Unix (`cfg(unix)`) |
| `serde` | `1.0` (`derive`) | 1.0.229 | Struct serialization and deserialization | All |
| `serde_json` | `1.0` | 1.0.154 | Named pipe JSON framing & diagnostics export | All |
| `toml` | `0.8` | 0.8.23 | Configuration file parsing and generation | All |
| `thiserror` | `2.0` | 2.0.21 | Typed error enums and domain error wrapping | All |
| `tracing` | `0.1` | 0.1.44 | Structured logging facade | All |
| `tracing-subscriber` | `0.3` (`env-filter`) | 0.3.23 | Console and file logging layer formatting | All |
| `tracing-appender` | `0.2` | 0.2.5 | Non-blocking daily rolling log file appender | All |
| `directories` | `6.0` | 6.0.0 | Standard XDG and OS base directory discovery | All |

**windows-sys Feature Modules Enabled (`Cargo.toml:56-78`):**
- `Win32_Foundation` (handles, errors, basic types)
- `Win32_System_SystemInformation` (system times, basic OS metrics)
- `Win32_System_Power` (`GetSystemPowerStatus`)
- `Win32_System_Performance` (PDH query and counter collection)
- `Win32_System_Threading` (mutexes, thread tokens)
- `Win32_System_Memory` (`GlobalMemoryStatusEx`)
- `Win32_System_Registry` (registry queries for CPU and BIOS info)
- `Win32_System_LibraryLoader` (`LoadLibraryA`, `GetProcAddress`, `FreeLibrary`)
- `Win32_System_Pipes` (named pipe creation, connection, message mode)
- `Win32_System_Services` (SCM service registration, control handler, dispatcher)
- `Win32_System_RemoteDesktop` (`WTSGetActiveConsoleSessionId`)
- `Win32_System_IO` (overlapped I/O primitives)
- `Win32_System_Wmi` (WMI constants and types)
- `Win32_Storage_FileSystem` (`CreateFileW`, `ReadFile`, `WriteFile`)
- `Win32_Security` (security attributes, tokens, impersonation)
- `Win32_Security_Authorization` (SDDL conversion to security descriptors)
- `Win32_UI_WindowsAndMessaging` (message pumps, window messaging)
- `Win32_UI_Shell` (`Shell_NotifyIconW`, tray management)
- `Win32_System_Com` (COM initialization and runtime)
- `Win32_Graphics_Gdi` (GDI resources for tray icons)
- `Win32_System_ProcessStatus` (process memory metrics)

**Hand-Written Infrastructure & Zero External Crate Stacks:**
- **No WMI / COM crate:** HP BIOS WMI is implemented via direct custom COM FFI (`src/service/windows/wmi_ffi.rs`) defining vtables for `IWbemLocator`, `IWbemServices`, `IWbemClassObject`, and `IEnumWbemClassObject`.
- **No HTTP client crate:** No `reqwest`, `ureq`, or `curl`. Update checks delegate entirely to the host's default web browser (`src/desktop/releases.rs`, `src/desktop/windows/releases.rs`).
- **No Database crate:** No SQLite, RocksDB, or embedded DB. State is maintained in memory or persisted as human-readable TOML/JSON.

## Configuration

**Runtime Environment & Storage:**
- **Configuration Path Resolution:**
  - Linux: `~/.config/zenomen/config.toml` (via `directories::ProjectDirs::from("com", "zenomen", "zenomen")`).
  - Windows: `%LOCALAPPDATA%\ZenOmen\config.toml` (via `src/desktop/windows/paths.rs`).
- **Service Data Path:**
  - Windows privileged service stores state/logs in `%PROGRAMDATA%\ZenOmen`.
- **Configuration Schema (`src/config/schema.rs`):**
  - `general`: `polling_interval_ms` (clamped between 500 ms and 5000 ms, default 1000 ms), `start_minimized`, `minimize_to_tray`, `theme` (`"dark"`), `active_preset` (`"balanced"`).
  - `telemetry`: `history_retention_minutes` (default 5 min), `enable_gpu_telemetry` (default true).
- **Atomic Persistence & Backup:**
  - Atomic writing via temporary files with sync and rename.
  - Corrupt configuration files are preserved as `config.toml.bak.<timestamp>` before defaults are cleanly restored.
- **Environment Variables:**
  - `RUST_LOG`: Log filter directive parsed by `tracing-subscriber::EnvFilter` (default: `"info"`).
  - `ZENOMEN_MOCK_SCENARIO`: Selects hardware mock scenario: `"nominal"`, `"zero"`, or `"gap"` (`src/hardware/mock.rs`).
  - `ZENOMEN_HW_SESSION`: Instructs Linux `zenomen-hw` daemon to register on the session bus instead of the system bus for development/testing (`src/bin/zenomen-hw.rs`).
  - `ZENOMEN_BENCHMARK`: Enables benchmark timing paths (`src/core/benchmark.rs`).
  - `XDG_CONFIG_HOME`, `XDG_STATE_HOME`, `HOME`: Standard Linux directory overrides.
  - `LOCALAPPDATA`, `PROGRAMDATA`: Windows directory overrides.

**Cargo Build Configuration:**
- **Features (`Cargo.toml:10-15`):**
  - `default = ["ui"]`
  - `ui = ["dep:slint"]`
  - Binary `zenomen` requires feature `ui`.
  - Binary `zenomen-hw` builds with `--no-default-features` (excluding Slint and UI libraries completely).
- **Compilation Profiles (`Cargo.toml:80-97`):**
  - Dev/Test: `debug = 0`, `incremental = false` (prevents multi-gigabyte build artifacts).
  - Release: `opt-level = "s"` (optimize for binary size and cache efficiency), `lto = true`, `codegen-units = 1`, `panic = "abort"`, `strip = true`.

## Platform Requirements

**Development Environment:**
- Rust stable toolchain supporting edition 2024 (1.85.0+).
- Linux: `libxkbcommon-dev`, `libfontconfig1-dev`, `libgl1-mesa-dev`, `libdbus-1-dev`, `xvfb` (for headless test execution).
- Windows: MSVC toolchain (Visual Studio Build Tools), Windows SDK (with `rc.exe` and `mt.exe` for resource compilation), WiX Toolset v3/v4.

**Target Deployments:**
- **Linux (x86_64):**
  - Distributions: Debian, Ubuntu, Fedora, Arch, and compatible systemd-based distributions.
  - Packaging: Debian package (`.deb`) and AppImage bundle (`.AppImage`) via `cargo-packager`.
  - Runtime Dependencies: `libxkbcommon0`, `libfontconfig1`, `libgl1`, `libdbus-1-3`.
- **Windows (x64):**
  - Versions: Windows 10 (version 1809+) and Windows 11.
  - Packaging: WiX MSI per-machine installer (`packaging/windows/ZenOmen.wxs`, UpgradeCode `{4E1A9D5B-3B9B-4D78-9B92-7E5A2A1A7D10}`).
  - Installs `zenomen.exe` (unprivileged GUI) and registers `zenomen-hw.exe` as `ZenOmenHardwareService` (auto-start LocalSystem service).

---

*Stack analysis verified: 2026-11-09*
