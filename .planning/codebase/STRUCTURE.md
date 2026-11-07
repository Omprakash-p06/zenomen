---
last_mapped_commit: 206cdc2634e0a9d0a07a90f0580927ad5b1c554e
last_mapped_at: 2026-11-07
---
# Codebase Structure

**Analysis Date:** 2026-11-07

## Directory Layout

```text
zenomen/
├── Cargo.toml                  # Single crate; two [[bin]] targets; `ui` feature; Windows dependencies
├── Cargo.lock                  # Committed lockfile (application dependency pinning & audit)
├── build.rs                    # Slint compilation under `ui` feature; Win32 resource embedding
├── AGENTS.md                   # Non-negotiable engineering rules & architectural invariants
├── README.md                   # User-facing installation, security, and architecture guide
├── src/
│   ├── lib.rs                  # Module tree; feature-gated `app` & `ui` modules
│   ├── main.rs                 # `zenomen` desktop GUI entry point and thread orchestration
│   ├── app.rs                  # Slint ↔ core event bridge (only file touching Slint AppWindow)
│   ├── logging.rs              # Early non-blocking rolling file appender (used by both bins)
│   ├── bin/
│   │   └── zenomen-hw.rs       # Dual-OS `zenomen-hw` privileged hardware daemon entry point
│   ├── core/
│   │   ├── mod.rs              # Re-exports the core application surface
│   │   ├── events.rs           # AppCommand, AppEvent, ScreenId message contracts
│   │   ├── state.rs            # AppState single-writer state machine + apply(&AppEvent)
│   │   ├── worker.rs           # 1 Hz monitoring worker + InFlightGuard atomic mutation slot
│   │   ├── telemetry.rs        # Bounded telemetry history (120 points) + SVG path generation
│   │   ├── diagnostics.rs      # System diagnostics report assembly + atomic JSON export
│   │   └── benchmark.rs        # Process RSS/PSS memory instrumentation and checkpoints
│   ├── hardware/
│   │   ├── mod.rs              # Hardware abstraction boundary re-exports
│   │   ├── provider.rs         # HardwareProvider trait, HardwareCapabilities, TelemetrySnapshot
│   │   ├── registry.rs         # Linux canonical sensor discovery (hwmon/ACPI/battery)
│   │   ├── mock.rs             # MockHardware provider + MockScenario fixtures
│   │   ├── unavailable.rs      # Honest zero-capability fallback stub for unsupported OSes
│   │   ├── rgb.rs              # RGB interface detection (read side)
│   │   ├── linux/              # Linux-specific native sensor implementations
│   │   │   ├── mod.rs          # Linux hardware module tree
│   │   │   ├── provider.rs     # LinuxHardwareProvider
│   │   │   ├── cpu.rs          # /proc/stat, cpufreq, and /proc/cpuinfo parsers
│   │   │   ├── gpu.rs          # DRM sysfs enumeration, AMD sensors, NVIDIA watcher
│   │   │   ├── memory.rs       # /proc/meminfo parser
│   │   │   └── hwmon.rs        # /sys/class/hwmon discovery and sensor readers
│   │   └── windows/            # Windows native telemetry engine (Win32, DXGI 1.3, PDH, WMI)
│   │       ├── mod.rs          # Windows hardware module tree
│   │       ├── provider.rs     # WindowsHardwareProvider
│   │       ├── cpu.rs          # Monotonic FILETIME GetSystemTimes calculation & registry model
│   │       ├── memory.rs       # GlobalMemoryStatusEx RAM telemetry
│   │       ├── gpu.rs          # DXGI 1.3 EnumAdapters1 & dedicated VRAM query
│   │       ├── battery.rs      # GetSystemPowerStatus battery metrics
│   │       ├── sensors.rs      # MSAcpi_ThermalZoneTemperature WMI query
│   │       └── device.rs       # BaseBoard identification & DMI extraction
│   ├── service/
│   │   ├── mod.rs              # Control & IPC subsystem re-exports
│   │   ├── contract.rs         # Linux D-Bus #[interface] com.zenomen.Hardware1
│   │   ├── backend.rs          # Linux verified sysfs writes, retries, and presets
│   │   ├── client.rs           # Linux blocking zbus client for the desktop worker
│   │   ├── polkit.rs           # Linux PolicyKit1 CheckAuthorization integration
│   │   ├── protocol.rs         # Shared IPC protocol primitives
│   │   ├── types.rs            # FanMode, PresetResult, HardwareControlError
│   │   └── windows/            # Windows privileged hardware control service & Named Pipe IPC
│   │       ├── mod.rs          # Windows service module tree
│   │       ├── lifecycle.rs    # SCM service lifecycle (ZenOmenHardwareService, register/run)
│   │       ├── pipe.rs         # \\.\pipe\ZenOmen.Hardware.v1 server pipe creation & framing
│   │       ├── server.rs       # Named pipe request handler and dispatch
│   │       ├── client.rs       # WindowsServiceClient named-pipe client for the GUI
│   │       ├── protocol.rs     # Length-prefixed (4-byte BE) JSON frame codec (max 64 KB)
│   │       ├── security.rs     # DACL SDDL security descriptor & active session authentication
│   │       ├── authorization.rs # Board profile safety validation & mutation gating
│   │       ├── hp_wmi.rs       # HP ACPI BIOS WMI driver (hpqBIntM / hpqBMtdx)
│   │       └── profiles.rs     # BoardProfile database and motherboard matching
│   ├── desktop/
│   │   ├── mod.rs              # Cross-platform desktop integration re-exports
│   │   ├── single_instance.rs  # Linux session D-Bus single instance
│   │   ├── tray.rs             # Linux StatusNotifierItem + DBusMenu tray
│   │   ├── autostart.rs        # Linux XDG autostart file management
│   │   ├── releases.rs         # Linux browser release opener (xdg-open)
│   │   ├── system_integration.rs # Linux root installer embedding system assets
│   │   └── windows/            # Windows native desktop integration
│   │       ├── mod.rs          # Windows desktop module tree
│   │       ├── single_instance.rs # Win32 named Mutex single-instance enforcement
│   │       ├── tray.rs         # Shell_NotifyIconW with dedicated Win32 message pump thread
│   │       ├── autostart.rs    # HKCU Run registry autostart management
│   │       ├── notifications.rs # Win32 native balloon/toast notifications
│   │       ├── releases.rs     # Win32 ShellExecuteW release page launch
│   │       ├── paths.rs        # Windows known folder paths (RoamingAppData, LocalAppData)
│   │       └── installer.rs    # Windows service installation wrappers
│   ├── config/
│   │   ├── mod.rs              # TOML config load, atomic save, and corrupt-file backup
│   │   └── schema.rs           # AppConfig, GeneralConfig, TelemetryConfig validation & bounds
│   ├── rgb/
│   │   ├── mod.rs              # RGB domain re-exports
│   │   ├── schema.rs           # RgbColor & RgbProfile strict hex validation
│   │   └── migration.rs        # OpenOMEN profile migration & built-in presets
│   └── ui/                     # Target GPUI-CE cross-platform presentation architecture
│       ├── mod.rs              # GPUI scaffold module tree
│       ├── app.rs              # GpuiApp root element tree and view-model bridge
│       ├── theme/              # Design tokens and theme styling
│       │   ├── mod.rs          # Theme definitions
│       │   ├── colors.rs       # Canonical RGB/RGBA color tokens
│       │   ├── spacing.rs      # Padding, margin, corner radius tokens
│       │   └── typography.rs   # Type scale, font weights, line heights
│       ├── components/         # Reusable presentation widgets
│       │   ├── mod.rs          # Component re-exports
│       │   ├── action_button.rs # Primary and ghost action buttons
│       │   ├── metric_card.rs  # Live sensor cards with status indicators
│       │   ├── sidebar.rs      # 6-screen navigation sidebar
│       │   ├── status_strip.rs # Service health and degradation banner
│       │   ├── telemetry_chart.rs # Multi-series historical chart with SVG rendering
│       │   └── value_readout.rs # Formatted metric readouts
│       └── screens/            # Screen view models
│           ├── mod.rs          # Screen re-exports
│           ├── overview.rs     # Dashboard overview view model
│           ├── cooling.rs      # Fan curves & manual/max mode view model
│           ├── performance.rs  # Power profile switching view model
│           ├── lighting.rs     # 4-zone RGB keyboard control view model
│           ├── settings.rs     # User preferences & intervals view model
│           └── diagnostics.rs  # Hardware report & export view model
├── ui/                         # Active production Slint 1.9 / 1.17 presentation layer
│   ├── app.slint               # AppWindow declarative shell, callbacks, screen router
│   ├── screens/                # Slint screens (overview, cooling, performance, lighting, settings, diagnostics)
│   ├── components/             # Slint components (sidebar, status-strip, telemetry-chart, metric-card)
│   └── theme/                  # Slint visual tokens and bundled offline Inter & JetBrains fonts
├── packaging/
│   └── windows/                # WiX Toolset v4/v5 Windows installer packaging
│       ├── ZenOmen.wxs         # WiX source manifest for GUI, SCM service, shortcuts
│       └── wix/                # WiX project build configuration
│           └── ZenOmen.wxs
├── tests/                      # 18 subsystem integration test suites
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
│   ├── windows_service_tests.rs # Windows named pipe protocol, board profiles, authorization
│   └── windows_telemetry_tests.rs # Windows native CPU, RAM, GPU, Battery calculation tests
├── data/                       # Packaged system integration assets
│   ├── applications/zenomen.desktop
│   ├── dbus/com.zenomen.Hardware1.{conf,service}
│   ├── polkit/com.zenomen.hardware.policy
│   ├── systemd/zenomen-hw.service
│   └── icons/                  # Multi-resolution icons (hicolor 16..512 + SVG + zenomen.ico)
├── docs/                       # Technical architecture, decisions, and testing protocols
│   ├── PACKAGING.md            # Cross-platform distribution & package formats
│   ├── PARITY.md               # Linux vs Windows feature parity matrix
│   ├── RESOURCE-BUDGET.md      # Memory budget verification (<=40 MB RSS)
│   ├── architecture/           # System architecture specifications
│   │   ├── CROSS_PLATFORM_ARCHITECTURE.md # Three-tier cross-platform design
│   │   ├── HP_WMI_PROTOCOL.md  # HP ACPI BIOS WMI firmware protocol specification
│   │   ├── IPC_SECURITY.md     # Cross-platform threat model & DACL security
│   │   └── WINDOWS_HARDWARE.md # Windows native telemetry subsystem details
│   ├── decisions/              # Architecture Decision Records
│   │   ├── ADR-001-ui-framework.md # Initial Slint evaluation
│   │   ├── ADR-002-gpui-ce-migration.md # Decision to migrate production UI to GPUI-CE
│   │   ├── ADR-002-windows-hardware-interface.md # Direct WMI/Win32 over WinRing0
│   │   ├── ADR-003-windows-service-security.md # Named pipe DACL SDDL & session isolation
│   │   └── ADR-004-cross-platform-ui.md # GPUI-CE multi-platform strategy
│   └── testing/                # Quality assurance and validation documents
│       ├── WINDOWS_TEST_MATRIX.md # Windows 10/11 build support & automated suite
│       ├── HP_BOARD_VALIDATION.md # Motherboard onboarding and safety verification
│       └── GEMINI_TESTING_HANDOFF.md # QA verification handoff and pipeline
├── scripts/                    # Asset automation & measurement scripts
│   ├── generate-icons.sh
│   ├── validate-icons.py
│   └── measure-budget.sh
├── logo/                       # Source vector SVGs
├── .github/                    # CI/CD workflows, CODEOWNERS, PR templates
└── .planning/                  # Project planning, roadmaps, and codebase documentation
```

---

## Directory Purposes

**`src/hardware/windows/`:**
- Purpose: Windows native telemetry engine and hardware discovery.
- Contains: Non-elevated sensor polling using Win32, DirectX 1.3, PDH, and WMI.
  - `provider.rs`: `WindowsHardwareProvider` implementing `HardwareProvider`.
  - `cpu.rs`: Monotonic `FILETIME` delta calculation from `GetSystemTimes` and registry processor string.
  - `memory.rs`: Total and available physical RAM via `GlobalMemoryStatusEx`.
  - `gpu.rs`: Adapter enumeration via DXGI 1.3 `EnumAdapters1` and dedicated VRAM reporting.
  - `battery.rs`: Battery percentage, AC status, and battery flags via `GetSystemPowerStatus`.
  - `sensors.rs`: Motherboard thermal zone queries via WMI.
  - `device.rs`: Board model and manufacturer extraction for board profile matching.
- Rule: strictly unprivileged; never requires administrator rights or kernel drivers.

**`src/service/windows/`:**
- Purpose: Elevated hardware control service and secure named pipe IPC on Windows.
- Contains:
  - `lifecycle.rs`: SCM service dispatch, service registration, and service uninstallation (`SERVICE_NAME = "ZenOmenHardwareService"`).
  - `pipe.rs`: Win32 named pipe creation at `\\.\pipe\ZenOmen.Hardware.v1` with explicit DACL security descriptors.
  - `server.rs`: Named pipe message loop dispatching requests to board profiles and HP WMI driver.
  - `client.rs`: `WindowsServiceClient` used by the unprivileged GUI to send IPC commands.
  - `protocol.rs`: 64 KB bounded length-prefixed JSON serialization for `HardwareRequest` and `HardwareResponse`.
  - `security.rs`: SDDL DACL construction (`D:(A;;GA;;;SY)(A;;GA;;;BA)(A;;GRGW;;;IU)`) and active console session verification (`WTSGetActiveConsoleSessionId`).
  - `authorization.rs`: Motherboard safety validation ensuring fan and power mutations match board capabilities.
  - `hp_wmi.rs`: HP ACPI BIOS WMI driver communicating through `hpqBIntM` class and `hpqBMtdx` method.
  - `profiles.rs`: Motherboard profile database defining supported modes and thermal limits.
- Rule: all mutating hardware commands must cross this boundary and undergo safety authorization.

**`src/desktop/windows/`:**
- Purpose: Windows-specific desktop shell and OS lifecycle integration.
- Contains:
  - `single_instance.rs`: Win32 named Mutex (`Local\ZenOmen_SingleInstance_Mutex_{username}`) with window restoration.
  - `tray.rs`: System tray notification icon (`Shell_NotifyIconW`) running on a dedicated thread (`zenomen-win32-tray-pump`) to satisfy Win32 message loop affinity.
  - `autostart.rs`: Registry autostart management under `HKCU\Software\Microsoft\Windows\CurrentVersion\Run`.
  - `notifications.rs`: Native desktop notifications.
  - `releases.rs`: Release URL launching via `ShellExecuteW`.
  - `paths.rs`: Known folder path resolution (`SHGetKnownFolderPath`).
  - `installer.rs`: Service registration helper commands.
- Rule: Win32 message pumps must never block core worker threads or the main GUI loop.

**`src/ui/`:**
- Purpose: Target GPUI-CE cross-platform presentation architecture scaffold ([ADR-002](file:///home/omprakash/Documents/zenomen/docs/decisions/ADR-002-gpui-ce-migration.md)).
- Contains:
  - `app.rs`: `GpuiApp` view-model container and event bridge.
  - `theme/`: Standardized color tokens, typography scales, and spacing units.
  - `components/`: Modular widgets (action buttons, metric cards, sidebar, status strip, telemetry charts, value readouts).
  - `screens/`: Typed screen view models (overview, cooling, performance, lighting, settings, diagnostics).
- Rule: pure presentation; decoupled from hardware drivers, communicating only via `AppCommand` and `AppEvent`.

**`packaging/windows/`:**
- Purpose: WiX Toolset v4/v5 installer authoring for Windows deployment.
- Contains:
  - `ZenOmen.wxs`: Comprehensive MSI package definition installing `zenomen.exe` (per-user/per-machine desktop app), `zenomen-hw.exe` (SCM Windows Service with auto-start and uninstall lifecycle), start menu shortcuts, and application icon.

**`docs/architecture/`:**
- Purpose: Cross-platform architecture specifications and technical whitepapers.
- Contains:
  - `CROSS_PLATFORM_ARCHITECTURE.md`: High-level multi-tier platform decoupling and invariants.
  - `HP_WMI_PROTOCOL.md`: Clean-room specification of the HP ACPI BIOS WMI communication protocol (`hpqBIntM`).
  - `IPC_SECURITY.md`: Threat model, DACL SDDL security, Polkit rules, and session impersonation.
  - `WINDOWS_HARDWARE.md`: Win32, DXGI 1.3, PDH, and WMI telemetry architecture.

**`docs/decisions/`:**
- Purpose: Architecture Decision Records (ADRs) tracking foundational design choices.
- Contains:
  - `ADR-001-ui-framework.md`: Initial Slint evaluation.
  - `ADR-002-gpui-ce-migration.md`: Ratified decision to adopt GPUI-CE as the unified cross-platform UI.
  - `ADR-002-windows-hardware-interface.md`: Direct ACPI WMI over third-party kernel drivers.
  - `ADR-003-windows-service-security.md`: Local named pipe security, SDDL DACL, and session verification.
  - `ADR-004-cross-platform-ui.md`: GPUI-CE multi-platform strategy.

**`docs/testing/`:**
- Purpose: Test matrices, motherboard onboarding procedures, and QA verification handoff protocols.
- Contains:
  - `WINDOWS_TEST_MATRIX.md`: Supported Windows 10/11 versions and automated test definitions.
  - `HP_BOARD_VALIDATION.md`: Step-by-step verification protocol for onboarding new HP board IDs.
  - `GEMINI_TESTING_HANDOFF.md`: Independent QA testing pipeline and verification commands.

---

## Key File Locations

**Entry Points:**
- `src/main.rs`: Desktop GUI binary `zenomen` (requires `ui` feature).
- `src/bin/zenomen-hw.rs`: Privileged hardware daemon binary `zenomen-hw` (D-Bus on Linux, SCM service on Windows).
- `build.rs`: Compiles Slint UI definitions and embeds Win32 resources.
- `src/lib.rs`: Root crate module tree.

**Windows Subsystems:**
- `src/hardware/windows/provider.rs`: `WindowsHardwareProvider` implementation.
- `src/service/windows/lifecycle.rs`: SCM service control and lifecycle.
- `src/service/windows/pipe.rs`: `\\.\pipe\ZenOmen.Hardware.v1` endpoint.
- `src/service/windows/security.rs`: SDDL DACL and console session verification.
- `src/service/windows/authorization.rs`: Motherboard profile mutation authorization.
- `src/service/windows/hp_wmi.rs`: HP BIOS WMI firmware driver.
- `src/desktop/windows/tray.rs`: Win32 system tray message pump (`zenomen-win32-tray-pump`).

**GPUI-CE Presentation Scaffold:**
- `src/ui/app.rs`: Root `GpuiApp` view model.
- `src/ui/theme/colors.rs`: Canonical color tokens.
- `src/ui/components/telemetry_chart.rs`: Historical metric SVG renderer.
- `docs/decisions/ADR-002-gpui-ce-migration.md`: GPUI-CE migration decision and release gates.

---

## Testing Layout

The `tests/` directory contains 18 comprehensive integration and unit test suites:

| Test File | Target Subsystem & Purpose |
|-----------|---------------------------|
| `benchmark_tests.rs` | RSS/PSS memory instrumentation, budget checkpoints, and memory limits |
| `config_tests.rs` | Config schema bounds, TOML load/save, and corrupt file backup recovery |
| `degradation_tests.rs` | Honest absence handling and zero/partial capability behaviour |
| `desktop_integration_tests.rs` | Desktop actions, autostart, and tray asset integration |
| `diagnostics_tests.rs` | System diagnostics assembly, DMI parsing, and JSON report export |
| `fan_control_tests.rs` | Fan mode mapping and control IPC verification |
| `hardware_service_tests.rs` | Linux backend sysfs writes, retries, and RGB variants against temp trees |
| `linux_gpu_tests.rs` | Linux DRM enumeration, AMD sysfs parsing, and NVIDIA watcher |
| `linux_procfs_tests.rs` | Linux `/proc/stat`, `/proc/meminfo`, and cpufreq parsers |
| `logging_tests.rs` | Non-blocking rotating file appender and retention policies |
| `mock_hardware_tests.rs` | Mock scenario execution (`nominal`, `zero-capability`, `overheating`) |
| `packaging_tests.rs` | Embedded packaging assets match disk files; root installer gating |
| `power_preset_tests.rs` | Power profile mapping, presets, and partial success semantics |
| `rgb_tests.rs` | 4-zone RGB validation, hex parsing, and profile migration |
| `sensor_registry_tests.rs` | Linux hwmon/ACPI canonical sensor discovery priority |
| `telemetry_history_tests.rs` | Bounded telemetry history, gap handling, and SVG geometry |
| `windows_service_tests.rs` | Windows named pipe protocol roundtrip, 64 KB bounds, and board profile authorization |
| `windows_telemetry_tests.rs` | Windows CPU `FILETIME` monotonic math, RAM calculations, GPU discovery, and battery flags |

---

## Where to Add New Code

**New Windows telemetry metric:**
1. Implement the reader in [`src/hardware/windows/`](file:///home/omprakash/Documents/zenomen/src/hardware/windows/) using Win32 or DXGI APIs.
2. Add the field to `TelemetrySnapshot` ([`src/hardware/provider.rs`](file:///home/omprakash/Documents/zenomen/src/hardware/provider.rs)). Use `Option` for missing sensors.
3. Incorporate the metric into `WindowsHardwareProvider::sample_telemetry` ([`src/hardware/windows/provider.rs`](file:///home/omprakash/Documents/zenomen/src/hardware/windows/provider.rs)).
4. Add verification unit tests to [`tests/windows_telemetry_tests.rs`](file:///home/omprakash/Documents/zenomen/tests/windows_telemetry_tests.rs).

**New Windows hardware mutation or board model:**
1. Onboard the board ID and supported modes into [`src/service/windows/profiles.rs`](file:///home/omprakash/Documents/zenomen/src/service/windows/profiles.rs) following [`docs/testing/HP_BOARD_VALIDATION.md`](file:///home/omprakash/Documents/zenomen/docs/testing/HP_BOARD_VALIDATION.md).
2. Implement WMI command dispatch in [`src/service/windows/hp_wmi.rs`](file:///home/omprakash/Documents/zenomen/src/service/windows/hp_wmi.rs).
3. Ensure authorization rules in [`src/service/windows/authorization.rs`](file:///home/omprakash/Documents/zenomen/src/service/windows/authorization.rs) validate requests.
4. Add contract tests in [`tests/windows_service_tests.rs`](file:///home/omprakash/Documents/zenomen/tests/windows_service_tests.rs).

**New GPUI-CE presentation component:**
1. Define component struct and element tree under [`src/ui/components/`](file:///home/omprakash/Documents/zenomen/src/ui/components/).
2. Use theme tokens from [`src/ui/theme/`](file:///home/omprakash/Documents/zenomen/src/ui/theme/).
3. Connect view models in [`src/ui/app.rs`](file:///home/omprakash/Documents/zenomen/src/ui/app.rs) to `AppEvent` and `AppCommand`.

---

*Structure analysis: 2026-11-07*
