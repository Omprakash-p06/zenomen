---
last_mapped_commit: 4691619f4e741bd5944f2085fa455b135ff873af
last_mapped_at: 2026-11-08
---
# Codebase Structure

**Analysis Date:** 2026-11-08

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
│   │       ├── gpu.rs          # DXGI 1.3 EnumAdapters1, dedicated VRAM, and PDH GpuPerformanceCollector
│   │       ├── battery.rs      # GetSystemPowerStatus battery metrics and charging state
│   │       ├── sensors.rs      # TemperatureSource, TemperatureReading, FanReading, honest absence
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
│   │       ├── lifecycle.rs    # SCM service lifecycle (ZenOmenHardwareService, dispatcher, --console)
│   │       ├── pipe.rs         # \\.\pipe\ZenOmen.Hardware.v1 server pipe creation & framing
│   │       ├── server.rs       # Synchronous named pipe request handler loop & graceful shutdown
│   │       ├── client.rs       # WindowsServiceClient named-pipe client for the GUI
│   │       ├── protocol.rs     # Length-prefixed (4-byte BE) JSON frame codec (max 64 KB)
│   │       ├── security.rs     # DACL SDDL security descriptor & active session authentication
│   │       ├── authorization.rs # Board profile safety validation & BoardValidation mutation gating
│   │       ├── hp_wmi.rs       # HP ACPI BIOS WMI driver (hpqBIntM / hpqBMtdx)
│   │       └── profiles.rs     # BoardProfile database and BoardValidation physical verification struct
│   ├── desktop/
│   │   ├── mod.rs              # Cross-platform desktop integration re-exports
│   │   ├── single_instance.rs  # Linux session D-Bus single instance
│   │   ├── tray.rs             # Linux StatusNotifierItem + DBusMenu tray
│   │   ├── autostart.rs        # Linux XDG autostart file management
│   │   ├── releases.rs         # Linux browser release opener (xdg-open)
│   │   ├── system_integration.rs # Linux root installer embedding system assets
│   │   └── windows/            # Windows native desktop integration
│   │       ├── mod.rs          # Windows desktop module tree
│   │       ├── single_instance.rs # Session-scoped Win32 named Mutex (Local\ZenOmen.SingleInstance)
│   │       ├── tray.rs         # Shell_NotifyIconW with dedicated Win32 message pump thread
│   │       ├── autostart.rs    # HKCU Run registry autostart management
│   │       ├── notifications.rs # Win32 native notifications bound via show_notification_with_hwnd
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
│   ├── windows_service_tests.rs # Windows named pipe protocol, board profiles, BoardValidation gating
│   └── windows_telemetry_tests.rs # Windows native CPU, RAM, GPU, Battery, and sensor calculation tests
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
│   │   ├── WINDOWS_FUNCTION_CONTRACTS.md # Win32 API contracts, SCM dispatcher, and validation contracts
│   │   └── WINDOWS_HARDWARE.md # Windows native telemetry subsystem details
│   ├── decisions/              # Architecture Decision Records
│   │   ├── ADR-001-ui-framework.md # Initial Slint evaluation
│   │   ├── ADR-002-gpui-ce-migration.md # Decision to migrate production UI to GPUI-CE
│   │   ├── ADR-002-windows-hardware-interface.md # Direct WMI/Win32 over WinRing0
│   │   ├── ADR-003-windows-service-security.md # Named pipe DACL SDDL & session isolation
│   │   └── ADR-004-cross-platform-ui.md # GPUI-CE multi-platform strategy
│   └── testing/                # Quality assurance and validation documents
│       ├── WINDOWS_TEST_MATRIX.md # Windows 10/11 build support & automated suite
│       ├── WINDOWS_IMPLEMENTATION_AUDIT.md # Systematic implementation audit and verification log
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
  - `provider.rs`: `WindowsHardwareProvider` implementing `HardwareProvider`, holding persistent CPU and PDH GPU samplers.
  - `cpu.rs`: Monotonic `FILETIME` delta calculation from `GetSystemTimes` (`(delta_kernel + delta_user) - delta_idle`) and registry processor string.
  - `memory.rs`: Total and available physical RAM via `GlobalMemoryStatusEx`.
  - `gpu.rs`: Adapter enumeration via DXGI 1.3 `EnumAdapters1`, dedicated VRAM reporting, and persistent `GpuPerformanceCollector` querying PDH `\GPU Engine(*)\Utilization Percentage`.
  - `battery.rs`: Battery percentage, AC status, and battery flags via `GetSystemPowerStatus`.
  - `sensors.rs`: Structured `TemperatureReading`, `TemperatureSource` (`AcpiThermalZone`, `CpuPackage`, `GpuCore`, `Unavailable`), and `FanReading` models; honest absence telemetry returning `(None, None)` and empty fan vectors.
  - `device.rs`: Board model and manufacturer extraction from `HARDWARE\DESCRIPTION\System\BIOS\BaseBoardProduct`.
- Rule: strictly unprivileged; never requires administrator rights or kernel drivers.

**`src/service/windows/`:**
- Purpose: Elevated hardware control service and secure named pipe IPC on Windows.
- Contains:
  - `lifecycle.rs`: SCM service dispatch via `StartServiceCtrlDispatcherW`, `service_main`, `RegisterServiceCtrlHandlerExW`, status reporting, registration/uninstallation, and automatic/explicit `--console` fallback mode.
  - `pipe.rs`: Win32 synchronous named pipe creation at `\\.\pipe\ZenOmen.Hardware.v1` (`PIPE_ACCESS_DUPLEX | PIPE_TYPE_MESSAGE | PIPE_READMODE_MESSAGE | PIPE_WAIT`) with explicit SDDL DACL.
  - `server.rs`: Synchronous named pipe request handler loop (`run_pipe_server`) with graceful atomic stop signal, session verification, 64 KB framed payload I/O, and `handle_request`.
  - `client.rs`: `WindowsServiceClient` named-pipe client used by the unprivileged GUI.
  - `protocol.rs`: 64 KB bounded length-prefixed (4-byte BE) JSON frame codec (`MAX_PAYLOAD_SIZE = 65536`).
  - `security.rs`: SDDL DACL construction (`D:(A;;GA;;;SY)(A;;GA;;;BA)(A;;GRGW;;;IU)`) and active console session verification (`verify_client_session` using `ImpersonateNamedPipeClient` + `WTSGetActiveConsoleSessionId`).
  - `authorization.rs`: Motherboard safety validation ensuring fan, power, and RGB mutations match board capabilities and physical `BoardValidation` evidence.
  - `hp_wmi.rs`: HP ACPI BIOS WMI driver communicating through `hpqBIntM` class and `hpqBMtdx` method.
  - `profiles.rs`: Motherboard profile database defining supported modes, and `BoardValidation` physical verification evidence struct.
- Rule: all mutating hardware commands must cross this boundary and undergo safety authorization.

**`src/desktop/windows/`:**
- Purpose: Windows-specific desktop shell and OS lifecycle integration.
- Contains:
  - `single_instance.rs`: Win32 named Mutex (`Local\ZenOmen.SingleInstance`) with window restoration via `FindWindowW`.
  - `tray.rs`: System tray notification icon (`Shell_NotifyIconW`) running on a dedicated thread (`zenomen-win32-tray-pump`) hosting invisible `WNDCLASSW` message window to satisfy Win32 message loop affinity.
  - `autostart.rs`: Registry autostart management under `HKCU\Software\Microsoft\Windows\CurrentVersion\Run`.
  - `notifications.rs`: Win32 native balloon/toast notifications via `Shell_NotifyIconW(NIM_MODIFY)` with `show_notification_with_hwnd` window handle binding.
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
- Rule: pure presentation; decoupled from hardware drivers, communicating only via `AppCommand` and `AppEvent`.\n\n**`packaging/windows/`:**\n- Purpose: WiX Toolset v4/v5 installer authoring for Windows deployment.\n- Contains:\n  - `ZenOmen.wxs`: Comprehensive MSI package definition installing `zenomen.exe` (per-user/per-machine desktop app), `zenomen-hw.exe` (SCM Windows Service with auto-start and uninstall lifecycle), start menu shortcuts, and application icon.\n\n**`docs/architecture/`:**\n- Purpose: Cross-platform architecture specifications and technical whitepapers.\n- Contains:\n  - `CROSS_PLATFORM_ARCHITECTURE.md`: High-level multi-tier platform decoupling and invariants.\n  - `HP_WMI_PROTOCOL.md`: Clean-room specification of the HP ACPI BIOS WMI communication protocol (`hpqBIntM`).\n  - `IPC_SECURITY.md`: Threat model, DACL SDDL security, Polkit rules, and session impersonation.\n  - `WINDOWS_FUNCTION_CONTRACTS.md`: Win32 API contracts, SCM service dispatcher, synchronous pipe protocol, physical board validation, telemetry absence guarantees, and desktop window binding.\n  - `WINDOWS_HARDWARE.md`: Win32, DXGI 1.3, PDH, and WMI telemetry architecture.\n\n**`docs/decisions/`:**\n- Purpose: Architecture Decision Records (ADRs) tracking foundational design choices.\n- Contains:\n  - `ADR-001-ui-framework.md`: Initial Slint evaluation.\n  - `ADR-002-gpui-ce-migration.md`: Ratified decision to adopt GPUI-CE as the unified cross-platform UI.\n  - `ADR-002-windows-hardware-interface.md`: Direct ACPI WMI over third-party kernel drivers.\n  - `ADR-003-windows-service-security.md`: Local named pipe security, SDDL DACL, and session verification.\n  - `ADR-004-cross-platform-ui.md`: GPUI-CE multi-platform strategy.\n\n**`docs/testing/`:**\n- Purpose: Test matrices, motherboard onboarding procedures, QA verification handoff protocols, and implementation audits.\n- Contains:\n  - `WINDOWS_TEST_MATRIX.md`: Supported Windows 10/11 versions and automated test definitions.\n  - `WINDOWS_IMPLEMENTATION_AUDIT.md`: Systematic verification audit across Windows subsystems.\n  - `HP_BOARD_VALIDATION.md`: Step-by-step verification protocol for onboarding new HP board IDs.\n  - `GEMINI_TESTING_HANDOFF.md`: Independent QA testing pipeline and verification commands.\n\n---\n\n## Key File Locations\n\n**Entry Points:**\n- `src/main.rs`: Desktop GUI binary `zenomen` (requires `ui` feature).\n- `src/bin/zenomen-hw.rs`: Privileged hardware daemon binary `zenomen-hw` (D-Bus on Linux, SCM service or standalone `--console` mode on Windows).\n- `build.rs`: Compiles Slint UI definitions and embeds Win32 resources.\n- `src/lib.rs`: Root crate module tree.\n\n**Windows Subsystems:**\n- `src/hardware/windows/provider.rs`: `WindowsHardwareProvider` implementation.\n- `src/hardware/windows/gpu.rs`: `GpuPerformanceCollector` persistent PDH utilization query and DXGI 1.3 adapter discovery.\n- `src/hardware/windows/sensors.rs`: `TemperatureSource`, `TemperatureReading`, `FanReading`, and honest absence functions.\n- `src/service/windows/lifecycle.rs`: SCM service dispatcher (`StartServiceCtrlDispatcherW`), handler, and status reporting.\n- `src/service/windows/pipe.rs`: `\\\\.\\pipe\\ZenOmen.Hardware.v1` synchronous server endpoint.\n- `src/service/windows/server.rs`: Synchronous request loop (`run_pipe_server`) and graceful atomic shutdown.\n- `src/service/windows/security.rs`: SDDL DACL and console session verification (`verify_client_session`).\n- `src/service/windows/profiles.rs`: Motherboard profile catalogue and `BoardValidation` physical verification evidence struct.\n- `src/service/windows/authorization.rs`: Motherboard profile mutation authorization and `BoardValidation` write gating.\n- `src/service/windows/hp_wmi.rs`: HP BIOS WMI firmware driver.\n- `src/desktop/windows/single_instance.rs`: Session-scoped Win32 mutex (`Local\\ZenOmen.SingleInstance`).\n- `src/desktop/windows/tray.rs`: Win32 system tray message pump (`zenomen-win32-tray-pump`).\n- `src/desktop/windows/notifications.rs`: Balloon/toast notifications with window handle binding (`show_notification_with_hwnd`).\n- `docs/architecture/WINDOWS_FUNCTION_CONTRACTS.md`: Formal contracts and Win32 API specifications.\n\n**GPUI-CE Presentation Scaffold:**\n- `src/ui/app.rs`: Root `GpuiApp` view model.\n- `src/ui/theme/colors.rs`: Canonical color tokens.\n- `src/ui/components/telemetry_chart.rs`: Historical metric SVG renderer.\n- `docs/decisions/ADR-002-gpui-ce-migration.md`: GPUI-CE migration decision and release gates.\n\n---\n\n## Testing Layout\n\nThe `tests/` directory contains 18 comprehensive integration and unit test suites:\n\n| Test File | Target Subsystem & Purpose |\n|-----------|---------------------------|\n| `benchmark_tests.rs` | RSS/PSS memory instrumentation, budget checkpoints, and memory limits |\n| `config_tests.rs` | Config schema bounds, TOML load/save, and corrupt file backup recovery |\n| `degradation_tests.rs` | Honest absence handling and zero/partial capability behaviour |\n| `desktop_integration_tests.rs` | Desktop actions, autostart, and tray asset integration |\n| `diagnostics_tests.rs` | System diagnostics assembly, DMI parsing, and JSON report export |\n| `fan_control_tests.rs` | Fan mode mapping and control IPC verification |\n| `hardware_service_tests.rs` | Linux backend sysfs writes, retries, and RGB variants against temp trees |\n| `linux_gpu_tests.rs` | Linux DRM enumeration, AMD sysfs parsing, and NVIDIA watcher |\n| `linux_procfs_tests.rs` | Linux `/proc/stat`, `/proc/meminfo`, and cpufreq parsers |\n| `logging_tests.rs` | Non-blocking rotating file appender and retention policies |\n| `mock_hardware_tests.rs` | Mock scenario execution (`nominal`, `zero-capability`, `overheating`) |\n| `packaging_tests.rs` | Embedded packaging assets match disk files; root installer gating |\n| `power_preset_tests.rs` | Power profile mapping, presets, and partial success semantics |\n| `rgb_tests.rs` | 4-zone RGB validation, hex parsing, and profile migration |\n| `sensor_registry_tests.rs` | Linux hwmon/ACPI canonical sensor discovery priority |\n| `telemetry_history_tests.rs` | Bounded telemetry history, gap handling, and SVG geometry |\n| `windows_service_tests.rs` | Windows named pipe protocol roundtrip, 64 KB bounds, board profile authorization, and `BoardValidation` mutation gating |\n| `windows_telemetry_tests.rs` | Windows CPU `FILETIME` monotonic math, RAM calculations, GPU discovery, PDH collector, VRAM honest absence, battery status, and sensor readings |\n\n---\n\n## Where to Add New Code\n\n**New Windows telemetry metric:**\n1. Implement the reader in [`src/hardware/windows/`](file:///home/omprakash/Documents/zenomen/src/hardware/windows/) using Win32, DXGI, or PDH APIs.\n2. Add the field to `TelemetrySnapshot` ([`src/hardware/provider.rs`](file:///home/omprakash/Documents/zenomen/src/hardware/provider.rs)). Use `Option` for missing sensors to preserve honest absence.\n3. Incorporate the metric into `WindowsHardwareProvider::sample_telemetry` ([`src/hardware/windows/provider.rs`](file:///home/omprakash/Documents/zenomen/src/hardware/windows/provider.rs)).\n4. Add verification unit tests to [`tests/windows_telemetry_tests.rs`](file:///home/omprakash/Documents/zenomen/tests/windows_telemetry_tests.rs).\n\n**New Windows hardware mutation or board model:**\n1. Onboard the board ID and supported modes into [`src/service/windows/profiles.rs`](file:///home/omprakash/Documents/zenomen/src/service/windows/profiles.rs) following [`docs/testing/HP_BOARD_VALIDATION.md`](file:///home/omprakash/Documents/zenomen/docs/testing/HP_BOARD_VALIDATION.md).\n2. Add physical validation records via `BoardValidation`.\n3. Implement WMI command dispatch in [`src/service/windows/hp_wmi.rs`](file:///home/omprakash/Documents/zenomen/src/service/windows/hp_wmi.rs).\n4. Ensure authorization rules in [`src/service/windows/authorization.rs`](file:///home/omprakash/Documents/zenomen/src/service/windows/authorization.rs) validate requests.\n5. Add contract tests in [`tests/windows_service_tests.rs`](file:///home/omprakash/Documents/zenomen/tests/windows_service_tests.rs).\n\n**New GPUI-CE presentation component:**\n1. Define component struct and element tree under [`src/ui/components/`](file:///home/omprakash/Documents/zenomen/src/ui/components/).\n2. Use theme tokens from [`src/ui/theme/`](file:///home/omprakash/Documents/zenomen/src/ui/theme/).\n3. Connect view models in [`src/ui/app.rs`](file:///home/omprakash/Documents/zenomen/src/ui/app.rs) to `AppEvent` and `AppCommand`.\n\n---\n\n*Structure analysis: 2026-11-08*\n
