---
last_mapped_commit: 761e5f975d7c22d1c48bf777a0bcaa1f2db582ba
last_mapped_at: 2026-11-07
---
<!-- refreshed: 2026-11-07 -->

# Architecture

**Analysis Date:** 2026-11-07

## System Overview

ZenOmen is a two-process Rust (edition 2024) application. The **unprivileged GUI** (`zenomen`) owns all rendering, telemetry sampling, and configuration; the **privileged daemon** (`zenomen-hw`) owns every hardware mutation and is reached only over system D-Bus through Polkit-gated calls.

```text
┌──────────────────────────────────────────────────────────────────────────────┐
│ Process A — `zenomen`  (regular user; built only with `--features ui`)      │
│                                                                              │
│  ui/app.slint + ui/screens/* + ui/components/*   ← pure declarative Slint    │
│                     ▲ Slint properties (strings only)                        │
│  src/app.rs  (Slint ↔ core bridge; callbacks→AppCommand, AppEvent→properties)│
│                     ▲ AppState::apply / invoke_from_event_loop               │
│  src/core/  (events, state, worker, telemetry, diagnostics, benchmark)       │
│  src/hardware/  (HardwareProvider trait, SensorRegistry, Linux/mock/         │
│                  unavailable backends — read-only sysfs/procfs)              │
│  src/config, src/logging, src/rgb, src/desktop                               │
└───────────┬──────────────────────────────────────┬───────────────────────────┘
            │ session D-Bus                        │ system D-Bus (control only)
            ▼                                      ▼
  com.zenomen.App (/com/zenomen/App)      com.zenomen.Hardware1
  org.kde.StatusNotifierItem (/StatusNotifierItem)   /com/zenomen/Hardware1
  com.canonical.dbusmenu (/MenuBar)       src/service/client.rs (zbus::blocking)
            │                                      │
            │                              ┌───────▼───────────────────────────┐
            │                              │ Process B — `zenomen-hw` (root)   │
            │                              │  src/service/contract.rs          │
            │                              │   #[interface] + polkit gate      │
            │                              │  src/service/backend.rs           │
            │                              │   verified sysfs writes           │
            │                              │  src/service/polkit.rs            │
            │                              │  systemd: ProtectSystem=strict    │
            │                              └───────────────────────────────────┘
            ▼
  desktop actions (PresentWindow / Quit) → src/main.rs action thread
```

There is exactly one process-level split. The GUI never escalates privileges, never imports `src/service/backend.rs`, and never writes sysfs. The daemon is designed to build without Slint (`--no-default-features`; `src/lib.rs:8` gates `app`, and `build.rs:11` skips compiling `ui/app.slint`); it never samples telemetry and never runs a window event loop.

## Component Responsibilities

| Component | Responsibility | File |
|-----------|----------------|------|
| GUI entry point | Logging, config load, single instance, provider selection, thread orchestration, shutdown order | `src/main.rs` |
| Slint bridge | Queue UI callbacks as `AppCommand`; fold `AppEvent` into `AppState`; publish properties on the UI thread | `src/app.rs` |
| UI shell / router | Window layout, sidebar, 6-screen router keyed by `current-screen`; no hardware access | `ui/app.slint` |
| Event contract | `AppCommand` (UI→core), `AppEvent` (core→UI), compile-time `ScreenId` | `src/core/events.rs` |
| Single-writer state | `AppState` + `apply(&AppEvent)` + status/degradation derivation | `src/core/state.rs` |
| Monitoring worker | 1 Hz deadline loop, on-demand refresh, config persistence, single in-flight mutation | `src/core/worker.rs` |
| Telemetry history | Bounded `VecDeque` (120 points) and SVG path generation (pure) | `src/core/telemetry.rs` |
| Diagnostics | Report assembly from state + registry + DMI/procfs; atomic JSON export | `src/core/diagnostics.rs` |
| Hardware contract | `HardwareProvider` trait, `HardwareCapabilities`, `TelemetrySnapshot` | `src/hardware/provider.rs` |
| Sensor registry | Canonical CPU-temp/fan/ACPI/battery bindings discovered once, shared by dashboard and diagnostics | `src/hardware/registry.rs` |
| Linux backend | procfs/sysfs/DRM/nvidia-smi sampling; all Linux-specific paths live here | `src/hardware/linux/*.rs` |
| Mock / unavailable backends | Deterministic scenarios and an honest zero-capability platform stub | `src/hardware/mock.rs`, `src/hardware/unavailable.rs` |
| D-Bus server contract | `com.zenomen.Hardware1` methods; Polkit gate on all mutating methods | `src/service/contract.rs` |
| Verified hardware writes | sysfs write + read-back + exponential-backoff retry; presets; RGB variants | `src/service/backend.rs` |
| Polkit verifier | `CheckAuthorization` against `org.freedesktop.PolicyKit1.Authority` | `src/service/polkit.rs` |
| D-Bus client | Blocking zbus calls, connection caching, actionable offline errors | `src/service/client.rs` |
| Daemon entry point | Build/serve `com.zenomen.Hardware1` on system bus (or session when `ZENOMEN_HW_SESSION` set) | `src/bin/zenomen-hw.rs` |
| Desktop integration | Single instance, SNI tray, close-to-tray, XDG autostart, release link | `src/desktop/*` |
| Config | `~/.config/zenomen/config.toml` with atomic save and corrupt-file recovery | `src/config/mod.rs`, `src/config/schema.rs` |
| RGB domain | Profile schema, strict hex validation, OpenOMEN migration, built-in presets | `src/rgb/*` |
| Logging | stderr + daily rotating file under the XDG state dir | `src/logging.rs` |

## Pattern Overview

**Overall:** Layered hexagonal-ish architecture with a typed message bus at the centre and a privilege boundary implemented as a D-Bus service.

**Key Characteristics:**
- **Single writer:** `AppState` changes through `AppState::apply(&AppEvent)` (`src/core/state.rs:109`); the UI only receives formatted strings.
- **Message-driven UI:** every visible change is the result of one `AppEvent`, published inside `slint::invoke_from_event_loop` (`src/app.rs:327-353`).
- **Read/write split:** telemetry reads happen in-process via `HardwareProvider`; all writes cross the D-Bus + Polkit boundary.
- **Honest absence:** missing sensors are `None`/empty/zero-length, never fabricated zeros; `UnavailableHardwareProvider` reports zero capabilities.
- **No async runtime in the GUI:** `std::thread` + `std::sync::mpsc`; zbus is used in `blocking` mode (`Cargo.toml:34`). Only the daemon uses zbus's async executor.
- **Feature-gated UI:** `ui` is a default feature; `zenomen` requires it, `zenomen-hw` builds without it.

## Layers

**Presentation — Slint:**
- Purpose: Declarative rendering; property bindings only.
- Location: `ui/app.slint`, `ui/screens/`, `ui/components/`, `ui/theme/`
- Contains: `AppWindow`, six screens (Overview, Cooling, Performance, Lighting, Settings, Diagnostics), shared cards/charts/status strip, theme tokens and bundled Inter/JetBrains Mono fonts.
- Depends on: nothing executable — imports only other `.slint` files and `std-widgets.slint`.
- Used by: `src/app.rs` (generated `AppWindow` via `slint::include_modules!()` in `src/lib.rs:18-19`).

**Bridge:**
- Purpose: The only bidirectional path between UI and core.
- Location: `src/app.rs`
- Contains: `ZenOmenApp` (`Weak<AppWindow>`, `SyncSender<AppCommand>`, `Arc<Mutex<AppState>>`, registry slot, close-to-tray flag), `install_callbacks`, `handle_event`, `initialize`, `apply_to_ui`.
- Depends on: `src/core`, `src/hardware` data types, `src/rgb`, `src/config`, `src/desktop::releases`.
- Used by: `src/main.rs` only.

**Application core:**
- Purpose: Own the truth about what is displayed and the sampling cadence.
- Location: `src/core/`
- Contains: `AppCommand`/`AppEvent`/`ScreenId`, `AppState`, `spawn_monitoring_worker`, `TelemetryHistory`/SVG paths, diagnostics, RSS benchmarking.
- Depends on: `src/hardware` types and `src/service` types (client status, `PresetResult`).
- Used by: `src/app.rs`, `src/main.rs`, tests.

**Hardware abstraction (read side):**
- Purpose: One seam for every sensor read.
- Location: `src/hardware/`
- Contains: `HardwareProvider` trait (`src/hardware/provider.rs:235`), `SensorRegistry`, `LinuxHardwareProvider`, `MockHardware`, `UnavailableHardwareProvider`, RGB interface detection.
- Depends on: std only (+ serde for snapshot/capability serialization).
- Used by: the monitoring worker and diagnostics; never by Slint code.

**Privileged control (write side):**
- Purpose: Validate, authorize, write, verify hardware state.
- Location: `src/service/`
- Contains: D-Bus interface (`contract.rs`), sysfs backend (`backend.rs`), Polkit (`polkit.rs`), blocking client (`client.rs`), domain types (`types.rs`).
- Depends on: `zbus`, `serde_json`, `thiserror`, `crate::rgb`, `crate::hardware::rgb`.
- Used by: `src/bin/zenomen-hw.rs` (server) and `src/core/worker.rs` (client).

**OS/desktop integration:**
- Purpose: Session-bus lifecycle and user-scoped files.
- Location: `src/desktop/`, `src/config/`, `src/logging.rs`, `src/rgb/migration.rs`
- Contains: single instance, SNI tray, autostart, release link, system integration installer, config TOML, log rotation, profile migration.
- Depends on: `zbus::blocking`, `directories`, `toml`, `tracing`.
- Used by: `src/main.rs`, the worker (config persistence), and the Settings screen callbacks.

**Packaging/data:**
- Purpose: Ship the systemd unit, D-Bus policy, Polkit action, desktop entry, icons, and fonts.
- Location: `data/`, `Cargo.toml` (`[package.metadata.packager]`), `.github/workflows/`
- Contains: `data/systemd/zenomen-hw.service`, `data/dbus/com.zenomen.Hardware1.{conf,service}`, `data/polkit/com.zenomen.hardware.policy`, `data/applications/zenomen.desktop`, `data/icons/**`, `ui/theme/fonts/**`.
- Note: there is **no** on-disk D-Bus introspection XML; zbus generates introspection from `#[interface]` attributes in `src/service/contract.rs`, `src/desktop/single_instance.rs`, and `src/desktop/tray.rs`.

## Data Flow

### Primary Request Path — telemetry tick → pixels

1. `spawn_monitoring_worker` opens with an initial capabilities event and an immediate sample (`src/core/worker.rs:49-64`).
2. Loop waits with `command_receiver.recv_timeout(deadline - now)`; `RefreshTelemetry` breaks the wait immediately (`src/core/worker.rs:103-148`).
3. On timeout, `sample_and_dispatch` calls `HardwareProvider::sample_telemetry()` (`src/core/worker.rs:345-360`).
4. `LinuxHardwareProvider::sample_telemetry` reads `/proc/stat`, cpufreq (fallback `/proc/cpuinfo`), the canonical CPU-temp binding, `/proc/meminfo`, fan tachometers, `platform_profile*`, then NVIDIA watcher or DRM GPU sysfs (`src/hardware/linux/provider.rs:117-205`).
5. Snapshot → `AppEvent::TelemetryUpdated` → `mpsc::Sender<AppEvent>` → `zenomen-event-pump` thread (`src/main.rs:245-254`).
6. `ZenOmenApp::handle_event` applies the event to `AppState` under the mutex, then queues `apply_to_ui` via `slint::invoke_from_event_loop` (`src/app.rs:327-353`).
7. `AppState::apply` pushes a `TelemetryPoint`, regenerates chart + sparkline SVG paths, fills first-seen fan/power values (`src/core/state.rs:116-142`).
8. `apply_to_ui` formats values into `SharedString`s and sets ~50 Slint properties (`src/app.rs:386-519`).

### Control Path — fan/power/RGB mutation

1. Slint callback (`set-fan-mode`, `set-power-profile`, `apply-preset`, `apply-rgb-preset`, `set-rgb-power`) is installed as an `AppCommand` sender (`src/app.rs:94-213`).
2. `send_command` uses `try_send` on a depth-8 `SyncSender`; a full queue drops the click and logs (`src/app.rs:357-369`).
3. Worker matches the command and calls `execute_mutating_action` (`src/core/worker.rs:227-297`).
4. One mutation at a time: `AtomicBool` swap rejects concurrent writes with an immediate error event; an `InFlightGuard` releases the slot even if the RPC thread panics/times out (`src/core/worker.rs:390-446`).
5. A `zenomen-ctrl-*` thread spawns a nested `zenomen-rpc-*` thread and waits at most 30 s (`src/core/worker.rs:413-436`).
6. `HardwareServiceClient` performs blocking zbus calls on the system bus (`src/service/client.rs:59-98`, `123-141`).
7. Daemon `HardwareService` method calls `authorize()` → `PolkitVerifier::check_authorization` with `com.zenomen.hardware.control` and `allow_user_interaction=true` (`src/service/contract.rs:56-80`).
8. `HardwareBackend` writes the sysfs target, sleeps, reads back, retries with exponential backoff (3 attempts, 50 ms initial for fan/power, 10 ms for RGB), and returns the confirmed value (`src/service/backend.rs:12-48`, `179-265`).
9. `PresetResult` is returned as JSON for combined presets; every leg is reported independently and a total failure becomes a D-Bus error (`src/service/backend.rs:269-355`).
10. Result → `AppEvent::FanModeChanged` / `PowerProfileChanged` / `PresetApplied` / `RgbProfileApplied` / `RgbPowerChanged` → state → UI.
11. Before the call, `AppEvent::FanModeRequested` is emitted so the UI shows an in-flight state for the duration of the Polkit prompt (`src/core/events.rs:131-136`, `src/core/worker.rs:232`).

### Startup Path

1. `logging::init()` installs stderr + rotating file layers, never aborting on failure (`src/main.rs:24`, `src/logging.rs:76`).
2. Root-only `--install-system-integration` / `--uninstall-system-integration` are handled before any UI (`src/main.rs:41-65`).
3. Config loads from `directories::ProjectDirs` (`~/.config/zenomen/config.toml`), with corrupt-file backup + defaults (`src/config/mod.rs:34-78`).
4. `ensure_single_instance` requests `com.zenomen.App` on the session bus with `DO_NOT_QUEUE`; a secondary instance invokes `PresentWindow` and exits (`src/desktop/single_instance.rs:67-114`).
5. Tray SNI + DBusMenu objects are registered; a missing `StatusNotifierWatcher` is non-fatal (`src/desktop/tray.rs:186-220`).
6. Short-lived OpenOMEN RGB migration runs non-destructively (`src/main.rs:98-112`, `src/rgb/migration.rs`).
7. `AppWindow::new()`, `ZenOmenApp::new`, callbacks installed, close-to-tray interceptor installed (`src/main.rs:114-136`).
8. Provider selection: `ZENOMEN_MOCK_SCENARIO` → mock; else Linux provider on Linux; else `UnavailableHardwareProvider` (`src/main.rs:186-211`).
9. `initialize(capabilities)` applies capabilities and writes the first frame directly (no event loop yet), including OS/kernel/DMI diagnostics (`src/app.rs:292-320`).
10. Config events are applied; the event pump, monitoring worker, and desktop-action thread start; window shows or starts hidden (`src/main.rs:237-268`).
11. Teardown sends `Shutdown`, drops senders/UI/app in dependency order, joins the worker, event pump, and action thread (`src/main.rs:270-278`).

### Secondary Flows

**Config changes:** Settings callbacks → `AppCommand::UpdateConfig` / `SetPollingInterval` / `SetAutostart` / `SetStartMinimized` / `SetCloseToTray` → worker sanitizes, atomically saves TOML (`src/config/mod.rs:85-116`), writes/removes `~/.config/autostart/zenomen.desktop` (`src/desktop/autostart.rs:67-92`), then emits `AppEvent::ConfigLoaded`.

**Diagnostics export:** `on_export_diagnostics` snapshots state + registry and spawns `zenomen-diag-export` (`src/app.rs:218-263`); `collect_diagnostics` reads `/etc/os-release`, `/proc/sys/kernel/osrelease`, `/sys/class/dmi/id`, `/sys/class/drm`; JSON is written atomically to `~/.config/zenomen/diagnostics/` (`src/core/diagnostics.rs:59-160`).

**Desktop actions:** SNI/menu and `com.zenomen.App` methods push `DesktopAction::{PresentWindow,Quit}` into a depth-16 channel; a dedicated thread calls `invoke_from_event_loop` to show/quit (`src/main.rs:138-168`, `src/desktop/tray.rs:122-183`).

**NVIDIA sampling:** `NvidiaSampler` owns a `zenomen-nv-watch` thread with a capacity-1 wake channel; samples older than 3 s are reported unavailable, so a hung driver cannot leave stale numbers on screen (`src/hardware/linux/gpu.rs:191-246`).

**State Management:**
- Canonical state: `Arc<Mutex<AppState>>` inside `ZenOmenApp`; poisoned locks are recovered (`src/app.rs:376-380`).
- Every event is applied before the UI callback is queued, so the core is always first to know.
- Navigation is only pushed to Slint on `AppEvent::ScreenChanged`; the sidebar updates immediately on click and the command confirms it asynchronously (`src/app.rs:391-393`).
- History is bounded at 120 points; `history_window` from config is one of 30/60/120/300 (`src/core/telemetry.rs`, `src/config/schema.rs:98-100`).

## Key Abstractions

**`HardwareProvider`** (`src/hardware/provider.rs:235-242`)
- Purpose: single seam for all sensor reads.
- Implementations: `LinuxHardwareProvider` (`src/hardware/linux/provider.rs`), `MockHardware` (`src/hardware/mock.rs`), `UnavailableHardwareProvider` (`src/hardware/unavailable.rs`).
- Pattern: `Send + Sync`, `capabilities()` + `sample_telemetry() -> Result<TelemetrySnapshot, String>`; errors are human-readable and land in the status strip.

**`HardwareCapabilities`** (`src/hardware/provider.rs:28-145`)
- Purpose: per-subsystem truth (8 booleans + RGB variant), never an aggregate.
- Pattern: `Default`/`none()` = nothing supported; `all_supported()` for tests; `reported_sensor_count()` counts subsystem flags, not sensors.

**`TelemetrySnapshot`** (`src/hardware/provider.rs:152-223`)
- Purpose: one immutable sample; optional fields mean “not read this tick”.
- Pattern: `EXPECTED_SENSOR_COUNT = 11`; presence is derived from model names/paths, not from zero values, so idle CPUs still count as reported.

**`AppEvent` / `AppCommand`** (`src/core/events.rs:71-153`)
- Purpose: the only two directions across the bridge.
- Pattern: commands carry user intent only; events carry plain data only (no sysfs paths, no commands to run).

**`AppState::apply`** (`src/core/state.rs:109-266`)
- Purpose: the single mutation point and the definition of “what is true now”.
- Pattern: exhaustive match over `AppEvent`; derives status text and degradation from capabilities + latest sample.

**`SensorRegistry`** (`src/hardware/registry.rs:17-152`)
- Purpose: discover canonical CPU-temp binding (k10temp → coretemp → acpitz), fan tachometers, `pwm1_enable`, `platform_profile`, battery presence once, then share between dashboard and diagnostics.
- Pattern: `scan_roots(hwmon_root, acpi_root)` for testable filesystem trees; read methods never invent values.

**`HardwareService` / `HardwareBackend`** (`src/service/contract.rs:14`, `src/service/backend.rs:52`)
- Purpose: D-Bus surface + verified sysfs writer.
- Pattern: server methods are thin; authorization is centralized in `authorize()`; writes go through `write_and_verify`; the backend is rooted at `/` by default and at a temp dir in tests.

**`FanMode` / `PresetResult`** (`src/service/types.rs:9-103`)
- Purpose: domain vocabulary shared by daemon, client, and UI state.
- Pattern: strict request parsing (`auto`/`max` only), sysfs mapping `0=max`, `1=manual`, `2=auto`; presets map to `(platform_profile, fan_mode)` tuples with granular reporting.

**`RgbProfile` / `RgbInterfaceVariant`** (`src/rgb/schema.rs`, `src/hardware/rgb.rs:13-54`)
- Purpose: 4-zone colour model with strict `#RRGGBB` validation and three kernel interface variants (`FourZone`, `Legacy`, `KernelPatch`).

**`MockScenario`** (`src/hardware/mock.rs:41-87`)
- Purpose: reproduce nominal, zero-capability, and sensor-gap behaviour without the target laptop.
- Pattern: selected by `ZENOMEN_MOCK_SCENARIO`; unknown values warn and fall back to real Linux provider selection (in `src/main.rs`) / nominal (in `MockScenario::from_env`).

## Entry Points

**GUI binary — `zenomen`:**
- Location: `src/main.rs` (`[[bin]]` requires `ui`, `Cargo.toml:40-43`)
- Triggers: desktop entry `Exec=zenomen %U`, terminal launch, or XDG autostart `zenomen --minimized`.
- Responsibilities: logging, locale fix-up, root-only install commands, config, single instance, tray, provider selection, worker/pump/action threads, Slint event loop, ordered shutdown.

**Daemon binary — `zenomen-hw`:**
- Location: `src/bin/zenomen-hw.rs:10-46`
- Triggers: systemd `Type=dbus` unit `data/systemd/zenomen-hw.service`, D-Bus activation via `data/dbus/com.zenomen.Hardware1.service`, or manual `sudo zenomen-hw`.
- Responsibilities: logging, `HardwareBackend::default()` (root `/`), request `com.zenomen.Hardware1`, serve at `/com/zenomen/Hardware1`, attach the connection for Polkit checks, then `future::pending()` forever.

**Session D-Bus service — `com.zenomen.App`:**
- Location: `src/desktop/single_instance.rs`
- Triggers: second GUI launch, desktop `Activate`.
- Responsibilities: `PresentWindow`, `Activate`, `Quit` → `DesktopAction` channel.

**Tray services:**
- Location: `src/desktop/tray.rs`
- Objects: `/StatusNotifierItem` (`org.kde.StatusNotifierItem`), `/MenuBar` (`com.canonical.dbusmenu`).
- Responsibilities: activation and two menu items (`Open ZenOmen`, `Quit ZenOmen`).

**Build entry — `build.rs`:**
- Location: `build.rs:7-25`
- Triggers: every build.
- Responsibilities: compile `ui/app.slint` with the pinned `fluent` style only when `CARGO_FEATURE_UI` is set; emit Windows resources under `cfg(windows)`.

## Architectural Constraints

- **Threading:** Slint runs on the main thread and is mutated only inside `slint::invoke_from_event_loop`. Named threads in the GUI: `zenomen-monitor-worker`, `zenomen-event-pump`, `zenomen-desktop-actions`, short-lived `zenomen-ctrl-<action>` + nested `zenomen-rpc-<action>`, `zenomen-nv-watch` (provider lifetime), `zenomen-diag-export`. The daemon runs a single zbus async executor via `zbus::block_on`.
- **Global state:** no module-level mutable `static`s. Shared state is held in `Arc`s: `Arc<Mutex<AppState>>`, `Arc<AtomicBool>` (in-flight mutation and close-to-tray), `Arc<SensorRegistry>`, `Arc<Mutex<Option<Connection>>>` (client cache), `Mutex<CpuSampler>`, `Arc<Mutex<Option<(Instant, GpuTelemetry)>>>`.
- **Single-writer discipline:** add a new `AppState` field only if it can be changed from `AppState::apply`; the one existing exception is `ZenOmenApp::set_provider_label`, which writes `state.provider_label` directly (`src/app.rs:276-283`).
- **Privilege boundary:** no `std::fs::write` to sysfs outside `src/service/backend.rs`; the GUI must only read. Root-only install paths live in `src/desktop/system_integration.rs` and are gated by `is_root()`.
- **Platform separation:** all Linux-specific reads are under `src/hardware/linux/`. `src/hardware/mod.rs:8` compiles that module on every target, but only re-exports `LinuxHardwareProvider` under `cfg(target_os = "linux")` (`src/hardware/mod.rs:16-17`); non-Linux selects `UnavailableHardwareProvider` (`src/main.rs:201-210`). Never add speculative paths to `src/hardware/unavailable.rs`.
- **No async runtime in the GUI:** `std` threads + `mpsc`; zbus is `blocking`. Do not add Tokio.
- **Bounded resources:** 120-point history, depth-8 command queue, 30 s mutation timeout, 7 rotating log files, ≤40 MB RSS release profile (`opt-level="s"`, LTO, `panic="abort"`, `strip`).
- **D-Bus contract is a public API:** bus name `com.zenomen.Hardware1`, object path `/com/zenomen/Hardware1`, interface `com.zenomen.Hardware1`. Changing a method signature requires updating `contract.rs`, `client.rs`, and the daemon/systemd/polkit packaging together.
- **Feature gating:** anything referencing `crate::AppWindow` or `slint` must be behind `#[cfg(feature = "ui")]` or live in `src/app.rs` (module gated in `src/lib.rs:8-9`).

## D-Bus Contract (GUI ↔ daemon)

**System bus, destination/path/interface `com.zenomen.Hardware1`.** The daemon binds the session bus instead when `ZENOMEN_HW_SESSION` is set (`src/bin/zenomen-hw.rs:23-27`).

| Method | Signature | Polkit | Implementation |
|--------|-----------|--------|----------------|
| `GetCapabilities` | `() -> s` (JSON: `has_fan_control`, `has_power_profiles`, `has_rgb_lighting`, `rgb_variant`, `service_version`) | No | `src/service/contract.rs:86-102` |
| `GetFanStatus` | `() -> (s, au)` mode + RPMs | No | `src/service/contract.rs:105-113` |
| `SetFanMode` | `(s) -> s` confirmed mode | Yes | `src/service/contract.rs:117-128` |
| `GetPowerProfiles` | `() -> as` | No | `src/service/contract.rs:131-135` |
| `GetPowerProfile` | `() -> s` | No | `src/service/contract.rs:138-142` |
| `SetPowerProfile` | `(s) -> s` confirmed profile | Yes | `src/service/contract.rs:146-157` |
| `ApplyPreset` | `(s) -> s` JSON `PresetResult` | Yes | `src/service/contract.rs:161-172` |
| `ApplyRgbProfile` | `(s) -> s` JSON `RgbProfile` payload | Yes | `src/service/contract.rs:176-191` |
| `SetRgbEnabled` | `(b) -> s` | Yes | `src/service/contract.rs:195-213` |

Authorization uses action `com.zenomen.hardware.control` (`data/polkit/com.zenomen.hardware.policy`); the bus policy allows any local user to send to the name, and the daemon enforces Polkit per call (`data/dbus/com.zenomen.Hardware1.conf`). `zbus::fdo::Error` mapping is centralized in `impl From<HardwareControlError> for zbus::fdo::Error` (`src/service/types.rs:133-145`).

## Error Handling

**Strategy:** typed errors inside the service; human-readable `String` at provider and client boundaries; no panics on the sampling path; the UI shows errors in the status strip or a dismissible control-error banner.

**Patterns:**
- `HardwareControlError` (`thiserror`) for I/O, verification, unsupported mode, Polkit denial, invalid parameter, missing interface (`src/service/types.rs:106-131`).
- Provider sampling returns `Result<TelemetrySnapshot, String>`; a failure becomes `AppEvent::TelemetrySamplingFailed` and marks the app degraded (`src/core/worker.rs:353-358`, `src/core/state.rs:143-145`).
- Mutating operations are wrapped with a 30 s timeout; timeout/disconnect/panic all become an error event, and the in-flight slot is released by `Drop` (`src/core/worker.rs:425-446`).
- Client turns `ServiceUnknown` into an actionable message telling the user to start or install `zenomen-hw` (`src/service/client.rs:232-249`).
- Config corruption is non-fatal: backup + defaults + `AppEvent::ConfigCorrupted` notice (`src/config/mod.rs:52-76`).
- Logging failure degrades to stderr only (`src/logging.rs:39-89`).

## Cross-Cutting Concerns

**Logging:** `tracing` with `EnvFilter` (`RUST_LOG`, default `info`), stderr layer plus a daily rotating file layer in the XDG state dir (`~/.local/state/zenomen/logs`), 7 files retained (`src/logging.rs`). The `LoggingGuard` must live for the process lifetime.

**Validation:** `AppConfig::sanitize` clamps polling to 500–5000 ms and history to 30/60/120/300 (`src/config/schema.rs:88-106`); `FanMode::parse_requested` permits only `auto`/`max`; power profiles must appear in `platform_profile_choices` and reject `\n`, `\r`, `/`, `\0` (`src/service/backend.rs:234-265`); RGB colours use strict hex parsing (`src/rgb/schema.rs`); `ScreenId::from_index` refuses unknown indices.

**Authentication:** Polkit on every mutating D-Bus method; getters are unauthenticated for diagnostics; the GUI can query readiness without prompting (`flags = 0`, `src/service/client.rs:251-299`).

**Configuration:** TOML at `~/.config/zenomen/config.toml`, atomic temp-file + fsync + rename + parent-dir sync (`src/config/mod.rs:85-116`). The worker is the only writer during a session.

**Memory/perf instrumentation:** `src/core/benchmark.rs` reads `/proc/self/smaps_rollup` (fallback `/proc/self/status`) and logs cold-start, warm-dashboard, tray-minimized, dashboard-restored checkpoints (`src/main.rs:95`, `129`, `155`, `242`).

## Anti-Patterns

### Direct `AppState` mutation outside `apply`

**What happens:** `ZenOmenApp::set_provider_label` locks the state and assigns `state.provider_label` directly (`src/app.rs:276-283`), because no `AppEvent` variant carries a provider label.
**Why it's wrong:** it is the only write path that bypasses the single-writer invariant; adding more exceptions erodes the guarantee that every visible value is reproducible from an event stream.
**Do this instead:** add an `AppEvent::ProviderLabelChanged(String)` (handled in `src/core/state.rs::apply`) or document the exception at the field and keep it to provider-selection metadata only.

### Publishing order gated on the first snapshot

**What happens:** `apply_to_ui` returns early when `state.latest_snapshot` is `None` (`src/app.rs:444-446`), before setting `service_status_text`, `polkit_status_text`, RGB profile fields, and preset results (all set after the gate).
**Why it's wrong:** service/Polkit/RGB properties are independent of telemetry, so on a machine with no sample (zero-capability or sampling failure) they keep their `.slint` defaults even though `AppState` knows better.
**Do this instead:** move the snapshot-independent publishers above the `let Some(snapshot) = ... else { return; }` gate, and only gate the readings that actually need a snapshot.

### Duplicated fan/RGB discovery in read and write paths

**What happens:** the read path resolves fans and RGB in `SensorRegistry::resolve_fans` / `detect_rgb_interface` (`src/hardware/registry.rs:256-320`, `src/hardware/rgb.rs:34-53`), while the daemon independently re-scans `pwm1_enable`, fan inputs, and the three RGB paths in `HardwareBackend::find_fan_control_path` / `get_fan_rpms` / `find_rgb_interface` (`src/service/backend.rs:77-175`, `358-388`).
**Why it's wrong:** the two implementations can disagree about priority (hp-wmi first vs class-wide fallback, dedup rules), producing “telemetry says one fan, control says another” behaviour.
**Do this instead:** when touching discovery, make one side delegate to shared helpers (the registry types are already in the same crate) or add a contract test that asserts both find the same paths for the same fixture tree.

### Public `AppState` fields

**What happens:** every field is `pub` (`src/core/state.rs:31-77`), so any module can assign to it without an event.
**Why it's wrong:** the type system does not enforce the single-writer rule; only review does.
**Do this instead:** keep new fields private with `pub(crate)` read accessors, or at minimum add them with a comment naming `apply` as the only writer.

### Hard-coded screen cardinality in three places

**What happens:** six screens are encoded in `ScreenId::ALL` (`src/core/events.rs:27-34`), the router bounds `current-screen <= 5` (`ui/app.slint:191-192`), and the `screen-titles` array (`ui/app.slint:196-203`).
**Why it's wrong:** adding a seventh screen silently falls back to Overview or indexes past the title array unless all three are edited together.
**Do this instead:** when adding a screen, update `ScreenId::ALL`/`index`/`title`, the `.slint` bound, the title array, and the router in the same commit; add a test that asserts `ScreenId::ALL.len()` equals the UI bound.

---

*Architecture analysis: 2026-11-07*
