# Codebase Structure

**Analysis Date:** 2026-11-09

## Directory Layout

```text
zenomen/
├── .github/
│   └── workflows/             # GitHub Actions CI/CD (ci.yml, release.yml)
├── .planning/                 # GSD planning documents and codebase maps
│   ├── codebase/              # System architecture, stack, structure, conventions
│   └── phases/                # Milestone and wave planning documentation
├── data/                      # Linux system assets (D-Bus, Polkit, systemd, desktop)
│   ├── dbus/                  # com.zenomen.Hardware1.conf
│   ├── polkit/                # com.zenomen.hardware.control.policy
│   ├── systemd/               # zenomen-hw.service unit file
│   └── icons/                 # Application icon assets in standard resolutions
├── docs/                      # Architecture notes, ADRs, test audits, resource budget
│   ├── architecture/          # HP WMI protocol and Windows function contracts
│   └── testing/               # Board validation reports and Windows audit
├── packaging/
│   └── windows/               # Windows WiX installer (.wxs) & deployment assets
├── scripts/                   # Icon generation, resource budget measurements, checks
├── tests/                     # Subsystem integration tests (18 test suites)
│   ├── benchmark_tests.rs
│   ├── config_tests.rs
│   ├── degradation_tests.rs
│   ├── desktop_integration_tests.rs
│   ├── diagnostics_tests.rs
│   ├── fan_control_tests.rs
│   ├── hardware_service_tests.rs
│   ├── linux_gpu_tests.rs
│   ├── linux_procfs_tests.rs
│   ├── logging_tests.rs
│   ├── mock_hardware_tests.rs
│   ├── packaging_tests.rs
│   ├── power_preset_tests.rs
│   ├── rgb_tests.rs
│   ├── sensor_registry_tests.rs
│   ├── telemetry_history_tests.rs
│   ├── windows_service_tests.rs
│   └── windows_telemetry_tests.rs
├── ui/                        # LIVE Declarative Slint UI definitions (compiled by build.rs)
│   ├── app.slint              # AppWindow shell, properties, callbacks, sidebar router
│   ├── screens/               # Screen views
│   │   ├── overview.slint     # Main dashboard with telemetry overview
│   │   ├── cooling.slint      # Fan mode toggles, tachometer cards, curves
│   │   ├── performance.slint  # ACPI power profiles and presets
│   │   ├── lighting.slint     # 4-zone keyboard RGB controls and presets
│   │   ├── diagnostics.slint  # Sensor registry dump and report export
│   │   └── settings.slint     # Polling interval, history window, autostart
│   ├── components/            # Reusable Slint UI widgets
│   │   ├── card.slint         # MetricCard, BaseCard, SectionCard
│   │   ├── header.slint       # ScreenHeader
│   │   ├── sidebar.slint      # AppSidebar with navigation buttons
│   │   ├── status-strip.slint # Bottom status bar and error notification banners
│   │   ├── telemetry-chart.slint # Canvas-based SVG path telemetry line chart
│   │   ├── pill.slint         # Status badges and pill indicators
│   │   └── button.slint       # Themed interactive buttons
│   └── theme/                 # Design tokens and fonts
│       ├── colors.slint       # ThemeColors palette
│       ├── typography.slint   # ThemeTypography styles
│       ├── metrics.slint      # ThemeMetrics dimensions and radii
│       └── fonts/             # Bundled local fonts (Inter, JetBrains Mono)
├── src/
│   ├── main.rs                # Unprivileged GUI binary entry (feature = "ui")
│   ├── lib.rs                 # Library root; exports modules and feature gates
│   ├── app.rs                 # Slint bridge: binds Slint callbacks to AppCommand & applies AppState
│   ├── logging.rs             # Tracing subscriber setup (stderr + daily rotating log file)
│   ├── bin/
│   │   └── zenomen-hw.rs      # Privileged hardware control daemon entry point
│   ├── core/                  # Core domain logic (pure Rust, zero direct hardware access)
│   │   ├── mod.rs             # Core module exports
│   │   ├── events.rs          # AppCommand, AppEvent, and ScreenId message contracts
│   │   ├── state.rs           # AppState single-writer reducer (AppState::apply)
│   │   ├── worker.rs          # 1 Hz monitoring worker loop and mutation dispatch
│   │   ├── telemetry.rs       # Bounded 120-sample history and SVG chart generation
│   │   ├── diagnostics.rs     # DiagnosticsReport aggregation and atomic JSON export
│   │   └── benchmark.rs       # RSS/PSS memory and cold-start timing instrumentation
│   ├── hardware/              # Read-only telemetry abstraction
│   │   ├── mod.rs             # Hardware module exports
│   │   ├── provider.rs        # HardwareProvider trait, Capabilities, TelemetrySnapshot
│   │   ├── registry.rs        # SensorRegistry: canonical sensor discovery & priority
│   │   ├── rgb.rs             # RgbInterfaceVariant detection
│   │   ├── mock.rs            # MockHardware provider with deterministic test scenarios
│   │   ├── unavailable.rs     # UnavailableHardwareProvider for unsupported platforms
│   │   ├── linux/             # Linux telemetry samplers (procfs, sysfs, DRM)
│   │   │   ├── mod.rs
│   │   │   ├── provider.rs    # LinuxHardwareProvider implementation
│   │   │   ├── cpu.rs         # /proc/stat, /proc/cpuinfo, scaling_cur_freq
│   │   │   ├── gpu.rs         # DRM card enumeration, AMD/NVIDIA telemetry
│   │   │   ├── memory.rs      # /proc/meminfo parsing
│   │   │   └── hwmon.rs       # /sys/class/hwmon device scanning and sensor parsing
│   │   └── windows/           # Windows telemetry samplers (PDH, DXGI, WMI, Win32)
│   │       ├── mod.rs
│   │       ├── provider.rs    # WindowsHardwareProvider implementation
│   │       ├── cpu.rs         # PDH CPU utilization and frequency queries
│   │       ├── gpu.rs         # DXGI adapter enumeration and memory counters
│   │       ├── memory.rs      # GlobalMemoryStatusEx memory sampling
│   │       ├── sensors.rs     # WMI thermal and fan readings
│   │       ├── battery.rs     # GetSystemPowerStatus battery metrics
│   │       └── device.rs      # Motherboard board ID and BIOS version discovery
│   ├── service/               # Privileged hardware service and IPC boundary
│   │   ├── mod.rs             # Service module exports
│   │   ├── client.rs          # HardwareServiceClient facade (D-Bus or Named Pipe)
│   │   ├── contract.rs        # com.zenomen.Hardware1 D-Bus interface definition (Linux)
│   │   ├── backend.rs         # Verified sysfs mutations with read-back verification (Linux)
│   │   ├── polkit.rs          # PolkitVerifier authorization checking (Linux)
│   │   ├── protocol.rs        # Unified versioned protocol contract (ServiceRequest/Response)
│   │   ├── types.rs           # FanMode, PresetResult, HardwareControlError types
│   │   └── windows/           # Windows privileged service implementation
│   │       ├── mod.rs
│   │       ├── server.rs      # Named pipe listener loop and request dispatcher
│   │       ├── client.rs      # WindowsServiceClient named-pipe IPC client
│   │       ├── protocol.rs    # Windows HardwareRequest and HardwareResponse (BE framing)
│   │       ├── pipe.rs        # CreateNamedPipeW helper and pipe name constants
│   │       ├── security.rs    # Named-pipe SDDL DACL and console session verification
│   │       ├── authorization.rs # Board profile mutation authorization rules
│   │       ├── profiles.rs    # BoardProfile catalog and BoardValidation records
│   │       ├── hp_wmi.rs      # HP BIOS WMI driver (hpqBIntM) and WmiAdapter trait
│   │       ├── wmi_ffi.rs     # Raw COM/WMI FFI bindings (Windows-only)
│   │       └── lifecycle.rs   # SCM service dispatcher, registration, and console runner
│   ├── config/                # Persistent application configuration
│   │   ├── mod.rs             # load_or_default, save_atomic, default_config_path
│   │   └── schema.rs          # AppConfig, GeneralConfig, TelemetryConfig, sanitization
│   ├── desktop/               # OS integration (single-instance, tray, autostart)
│   │   ├── mod.rs             # Cross-platform desktop exports and DesktopAction
│   │   ├── single_instance.rs # Session D-Bus single-instance mutex (Linux)
│   │   ├── tray.rs            # StatusNotifierItem system tray integration (Linux)
│   │   ├── autostart.rs       # XDG autostart desktop entry management (Linux)
│   │   ├── system_integration.rs # Root asset installer/uninstaller (Linux)
│   │   ├── releases.rs        # Browser launcher for release downloads
│   │   └── windows/           # Win32 desktop integration
│   │       ├── mod.rs
│   │       ├── single_instance.rs # Win32 Named Mutex single-instance check
│   │       ├── tray.rs        # Shell_NotifyIconW Win32 system tray icon
│   │       ├── autostart.rs   # HKCU\Software\Microsoft\Windows\CurrentVersion\Run
│   │       ├── paths.rs       # KnownFolder LocalAppData path resolution
│   │       ├── notifications.rs # Toast notification wrappers
│   │       ├── installer.rs   # Service installation helpers
│   │       └── releases.rs    # ShellExecuteW release link launcher
│   ├── rgb/                   # RGB lighting domain
│   │   ├── mod.rs             # RGB exports and builtin presets
│   │   ├── schema.rs          # RgbProfile, RgbColor, 4-zone schemas
│   │   └── migration.rs       # OpenOMEN legacy JSON profile migration parser
│   └── ui/                    # DORMANT: GPUI-CE migration scaffold (not used by live binaries)
│       ├── mod.rs
│       ├── app.rs             # GpuiApp scaffold (bypasses AppState::apply; do not extend)
│       ├── components/
│       ├── screens/
│       └── theme/
├── build.rs                   # Build script: compiles ui/app.slint & embeds Windows manifest/icons
├── Cargo.toml                 # Cargo manifest: features ("ui"), binaries, profile configurations
└── AGENTS.md                  # Repository rules and architectural guidelines
```

---

## Directory Purposes & Module Boundaries

### `src/core/` — Domain Core (Pure Logic & State)
- **Role:** Pure application logic, message definitions, scheduling, and state aggregation.
- **Invariants:**
  - Zero direct hardware I/O. Hardware reads are injected via `Arc<dyn HardwareProvider>`.
  - Zero Slint UI dependencies. Core code can compile without the `ui` feature.
  - `AppState::apply` is the **single writer** mutator for application state.
  - Telemetry history is strictly bounded (`MAX_SAMPLES = 120`).

### `src/hardware/` — Read-Only Hardware Telemetry
- **Role:** Implements the `HardwareProvider` trait to provide unprivileged telemetry sampling.
- **Submodules:**
  - `linux/`: Reads procfs and sysfs directly. Bound to `SensorRegistry` to avoid duplicate sysfs scans.
  - `windows/`: Calls native Win32/COM APIs (PDH, DXGI, `GlobalMemoryStatusEx`, WMI).
  - `registry.rs`: Enforces strict temperature priority matrix (AMD `k10temp` > Intel `coretemp` > ACPI).
  - `mock.rs`: Deterministic mock provider for tests.
  - `unavailable.rs`: Safe zero-capability fallback.
- **Invariants:** Unimplemented sensors are represented as `None` or empty vectors, never fabricated zeros.

### `src/service/` — Privileged Control & IPC Seam
- **Role:** Handles elevated hardware control (fan modes, power profiles, RGB lighting) across the privilege boundary.
- **Submodules:**
  - `client.rs`: `HardwareServiceClient` facade used by GUI background threads.
  - `contract.rs`, `backend.rs`, `polkit.rs`: Linux D-Bus `com.zenomen.Hardware1` server and verified sysfs writes.
  - `windows/`: Windows named pipe server (`\\.\pipe\ZenOmen.Hardware.v1`), SCM dispatcher, client security validation, board validation catalogs, and HP BIOS WMI driver.
- **Invariants:**
  - All hardware mutations must be verified with read-back checks before reporting success.
  - Linux mutations require Polkit authorization (`com.zenomen.hardware.control`).
  - Windows mutations require active console session ID and validated `BoardProfile`.

### `ui/` — Live Declarative Slint UI
- **Role:** Authoritative user interface declarations.
- **Submodules:**
  - `app.slint`: Top-level window, global properties, callbacks, and router.
  - `screens/`: Individual views (`overview`, `cooling`, `performance`, `lighting`, `diagnostics`, `settings`).
  - `components/`: Modular widgets (`card`, `header`, `sidebar`, `status-strip`, `telemetry-chart`, `pill`, `button`).
  - `theme/`: Theme tokens (`colors`, `typography`, `metrics`) and bundled fonts.
- **Invariants:** Purely declarative. Zero hardware access, zero system calls, zero math algorithms.

### `src/ui/` — Dormant GPUI Scaffold
- **Role:** Legacy GPUI-CE experimental migration scaffold.
- **Status:** Unused by `src/main.rs` and `src/bin/zenomen-hw.rs`.
- **Warning:** Directly assigns to `self.state.*`, violating the single-writer invariant. Do not add new code here.

### `src/config/` & `src/desktop/` — System & Persistence
- **Role:** Configuration management and OS integration.
- **Submodules:**
  - `src/config/`: TOML configuration with atomic persistence and auto-recovery from corrupted files.
  - `src/desktop/`: Single-instance enforcement, system tray, and autostart registration for Linux and Windows.

---

## Key File Locations

| Concern | File Path |
|---------|-----------|
| GUI Binary Entry | `src/main.rs` |
| Daemon Binary Entry | `src/bin/zenomen-hw.rs` |
| Build Script | `build.rs` |
| Slint Bridge | `src/app.rs` |
| Declarative UI Shell | `ui/app.slint` |
| Message Contract | `src/core/events.rs` |
| State Reducer | `src/core/state.rs` |
| Monitoring Worker | `src/core/worker.rs` |
| Hardware Seam | `src/hardware/provider.rs` |
| Sensor Priority Matrix | `src/hardware/registry.rs` |
| Service Client Facade | `src/service/client.rs` |
| Linux D-Bus Contract | `src/service/contract.rs` |
| Linux Sysfs Writes | `src/service/backend.rs` |
| Windows Named Pipe Server | `src/service/windows/server.rs` |
| Windows IPC Protocol | `src/service/windows/protocol.rs` |
| HP BIOS WMI Driver | `src/service/windows/hp_wmi.rs` |
| Windows Board Catalog | `src/service/windows/profiles.rs` |
| Windows Security Gate | `src/service/windows/security.rs` |
| Configuration Schema | `src/config/schema.rs` |
| Windows Service Tests | `tests/windows_service_tests.rs` |

---

## Naming Conventions & Code Style

- **Files & Modules:**
  - Rust files use `snake_case.rs` (e.g., `hp_wmi.rs`, `single_instance.rs`).
  - Slint files use `kebab-case.slint` (e.g., `telemetry-chart.slint`, `metric-card.slint`).
  - Integration tests use `<subsystem>_tests.rs` (e.g., `fan_control_tests.rs`).
- **Types & Enums:** `PascalCase` (e.g., `AppState`, `TelemetrySnapshot`, `HardwareControlError`).
- **Slint Types & Properties:**
  - Component names: `PascalCase` (e.g., `AppWindow`, `MetricCard`).
  - Properties & callbacks: `kebab-case` (e.g., `has-fan-control`, `set-fan-mode`).
- **Functions & Methods:** `snake_case`, verb-first (e.g., `sample_telemetry`, `execute_mutating_action`).
- **Constants:** `SCREAMING_SNAKE_CASE` (e.g., `MAX_PAYLOAD_SIZE`, `PROTOCOL_VERSION`).
- **Unit Tests:** Behavior-driven test names (e.g., `a_second_mutation_is_rejected_while_one_is_in_flight`).

---

## Where to Add New Code

### 1. Adding a New Hardware Sensor
1. Declare the metric in `TelemetrySnapshot` or `HardwareCapabilities` in `src/hardware/provider.rs` (use `Option<T>` for optional metrics).
2. Implement sampling in `src/hardware/linux/` (for Linux) and `src/hardware/windows/` (for Windows).
3. If Linux requires canonical path binding, update `SensorRegistry` in `src/hardware/registry.rs`.
4. Update `MockHardware` in `src/hardware/mock.rs` and `UnavailableHardwareProvider` in `src/hardware/unavailable.rs`.
5. Expose formatted properties in `src/app.rs:apply_to_ui` and bind them in `ui/screens/*.slint`.
6. Add unit/integration tests in `tests/sensor_registry_tests.rs` and `tests/windows_telemetry_tests.rs`.

### 2. Adding a New UI Screen
1. Create `ui/screens/<new_screen>.slint`.
2. Add a variant to `ScreenId` in `src/core/events.rs`.
3. Import and route the screen inside `ui/app.slint`.
4. Add sidebar navigation buttons in `ui/components/sidebar.slint`.
5. Update `apply_to_ui` in `src/app.rs` to publish state required by the new screen.

### 3. Adding a User Action / UI Command
1. Add an `AppCommand` variant to `src/core/events.rs`.
2. Declare callback in `ui/app.slint` (or component) and wire in `src/app.rs:install_callbacks`.
3. Handle the command in `src/core/worker.rs`.
4. If an asynchronous result is produced, declare a matching `AppEvent` variant in `src/core/events.rs`.
5. Add the reducer transition in `src/core/state.rs:apply`.
6. Add unit tests for state transition in `src/core/state.rs`.

### 4. Adding a Privileged Mutation / IPC Operation
1. Add variants to `HardwareRequest` and `HardwareResponse` in `src/service/windows/protocol.rs` and `ServiceRequest`/`ServiceResponse` in `src/service/protocol.rs`.
2. **Linux:** Add method to `com.zenomen.Hardware1` in `src/service/contract.rs` (calling `authorize(&hdr)` first) and implement verified sysfs write in `src/service/backend.rs`.
3. **Windows:** Add handler to `handle_request` in `src/service/windows/server.rs`, mutation authorization in `src/service/windows/authorization.rs`, and BIOS WMI implementation in `src/service/windows/hp_wmi.rs`.
4. Expose the client method in `HardwareServiceClient` (`src/service/client.rs`).
5. Wire the call through `execute_mutating_action` in `src/core/worker.rs`.
6. Add integration tests in `tests/windows_service_tests.rs` and `tests/hardware_service_tests.rs`.

### 5. Adding a Configuration Option
1. Add the field to `AppConfig` or sub-structs in `src/config/schema.rs`.
2. Implement default values and clamping in `AppConfig::sanitize`.
3. Add a UI setting control in `ui/screens/settings.slint` and callback in `src/app.rs`.
4. Handle update command in `src/core/worker.rs` and save via `config::save_atomic`.
5. Add test coverage in `tests/config_tests.rs`.

---

*Structure analysis: 2026-11-09*
