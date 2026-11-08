<!-- refreshed: 2026-11-08 -->
# Architecture

**Analysis Date:** 2026-11-08

## System Overview

ZenOmen is a two-binary Rust (edition 2024) workspace-in-one-crate: an unprivileged Slint GUI (`zenomen`) and a privileged hardware daemon (`zenomen-hw`). The GUI never writes hardware. All mutations cross a typed IPC boundary to the daemon, which performs verified write + read-back cycles.

```text
┌──────────────────────────────────────────────────────────────────────────┐
│  PRESENTATION (unprivileged, `ui` feature)                               │
│  ┌────────────────────────┐   ┌──────────────────────────────────────┐   │
│  │ Slint declarations     │   │ Bridge `src/app.rs` (ZenOmenApp)     │   │
│  │ `ui/app.slint`         │◄─►│ callbacks -> AppCommand              │   │
│  │ `ui/screens/*.slint`   │   │ AppEvent -> AppState -> properties   │   │
│  │ `ui/components/*.slint`│   │                                      │   │
│  └────────────────────────┘   └───────────────┬──────────────────────┘   │
└───────────────────────────────────────────────┼──────────────────────────┘
                     SyncSender<AppCommand> (cap 8) │ Sender<AppEvent>
                     (UI -> core, try_send)          │ (core -> UI, unbounded)
┌───────────────────────────────────────────────▼──────────────────────────┐
│  CORE (`src/core/`) — no hardware access                                 │
│  `events.rs` (AppCommand / AppEvent / ScreenId)                          │
│  `state.rs` (AppState::apply — single writer)                            │
│  `worker.rs` (1 Hz deadline loop + mutation threads)                     │
│  `telemetry.rs` (bounded 120-sample history + SVG paths)                 │
│  `diagnostics.rs` · `benchmark.rs`                                       │
└───────────────┬───────────────────────────────────┬──────────────────────┘
                │ read-only sampling                 │ mutating RPC (30 s cap)
┌───────────────▼──────────────────┐   ┌────────────▼──────────────────────┐
│  HARDWARE ABSTRACTION            │   │  SERVICE CLIENT (`src/service/`)  │
│  `hardware/provider.rs` trait    │   │  Linux: system D-Bus              │
│  `hardware/linux/*`              │   │    `com.zenomen.Hardware1`        │
│  `hardware/windows/*`            │   │  Windows: named pipe              │
│  `hardware/mock.rs`              │   │    `\\.\pipe\ZenOmen.Hardware.v1` │
│  `hardware/unavailable.rs`       │   └────────────┬──────────────────────┘
│  `hardware/registry.rs` (sysfs)  │                │
└──────────────────────────────────┘                │
┌────────────────────────────────────────────────────▼─────────────────────┐
│  PRIVILEGED DAEMON `src/bin/zenomen-hw.rs` (no Slint feature)            │
│  `service/contract.rs` (zbus interface) · `service/polkit.rs`            │
│  `service/backend.rs` (verified sysfs write + read-back retry)           │
│  `service/windows/{lifecycle,server,hp_wmi,security,authorization,...}`  │
└──────────────────────────────────────────────────────────────────────────┘
┌──────────────────────────────────────────────────────────────────────────┐
│  SUPPORTING DOMAINS                                                      │
│  `src/config/` TOML + atomic save   `src/rgb/` schema + migration        │
│  `src/desktop/` single-instance, tray, autostart, releases               │
│  `src/logging.rs` tracing + rotating file                                │
└──────────────────────────────────────────────────────────────────────────┘
```

## Component Responsibilities

| Component | Responsibility | File |
|-----------|----------------|------|
| GUI entry point | Process bootstrap, provider selection, lifecycle, shutdown ordering | `src/main.rs` |
| Slint bridge | Only path into the UI: callbacks → `AppCommand`, `AppEvent` → properties | `src/app.rs` |
| App state | Single mutable record of what the UI shows; `apply()` is the only mutator | `src/core/state.rs` |
| Message contract | `AppCommand` (UI→core), `AppEvent` (core→UI), `ScreenId` enum | `src/core/events.rs` |
| Monitoring worker | 1 Hz deadline loop, initial sample, service probing, mutation offload | `src/core/worker.rs` |
| Telemetry history | Bounded `VecDeque` sample buffer and SVG path generation | `src/core/telemetry.rs` |
| Diagnostics | Report assembly, JSON export, platform system metadata | `src/core/diagnostics.rs` |
| Budget instrumentation | RSS/PSS/Private-Dirty sampling and benchmark checkpoints | `src/core/benchmark.rs` |
| Provider contract | `HardwareProvider` trait, `HardwareCapabilities`, `TelemetrySnapshot` | `src/hardware/provider.rs` |
| Linux provider | procfs/sysfs/DRM/nvidia-smi sampling | `src/hardware/linux/provider.rs` |
| Windows provider | PDH/DXGI/WMI battery/sensor sampling (read-only) | `src/hardware/windows/provider.rs` |
| Mock provider | Deterministic scenarios (`nominal`, `zero-capabilities`, `sensor-data-gap`) | `src/hardware/mock.rs` |
| Unavailable provider | Zero capabilities on unsupported platforms | `src/hardware/unavailable.rs` |
| Sensor registry | Canonical CPU-temp / fan / profile bindings, shared by telemetry and diagnostics | `src/hardware/registry.rs` |
| D-Bus contract | `com.zenomen.Hardware1` interface + Polkit gate | `src/service/contract.rs` |
| sysfs backend | `write_and_verify` retry/read-back writes, presets, RGB | `src/service/backend.rs` |
| Service client | Platform-switching RPC client (`HardwareServiceClient`) | `src/service/client.rs` |
| Windows service | SCM lifecycle, named-pipe server loop, DACL/session verification | `src/service/windows/{lifecycle,server,security}.rs` |
| HP WMI driver | `hpqBIntM` mailbox command set | `src/service/windows/hp_wmi.rs` |
| Board policy | Per-board capability catalog + validation evidence gating | `src/service/windows/{profiles,authorization}.rs` |
| GUI-side view models | Parallel GPUI-CE scaffold mirroring Slint screens (**not on render path**) | `src/ui/` |
| Config | TOML load, sanitize, atomic save, corrupt-file backup | `src/config/mod.rs`, `src/config/schema.rs` |
| RGB domain | Strict `#RRGGBB` schema, 4-zone profiles, OpenOMEN migration | `src/rgb/{schema,migration}.rs` |
| Desktop integration | Single-instance, tray, autostart, releases, root installer | `src/desktop/` |
| Logging | stderr + daily rotating file, `LoggingGuard` lifetime | `src/logging.rs` |

## Pattern Overview

**Overall:** Layered hexagonal-style architecture with a single UI bridge, a threaded producer/consumer core, and a platform-abstracted hardware boundary. All UI mutation is funneled through one event queue; all privileged mutation crosses one IPC seam.

**Key Characteristics:**
- **Single-writer UI state** — `AppState` changes only through `AppState::apply(&AppEvent)` (`src/core/state.rs:109`).
- **One direction in, one direction out** — UI → `AppCommand` (queued, lossy); core → `AppEvent` (applied, then published inside `slint::invoke_from_event_loop`).
- **No hardware in core/UI** — sampling lives behind `HardwareProvider`; writes live behind `HardwareService`/`HardwareServiceClient`.
- **Honest absence** — missing sensors are `None`/empty, never `0`; capabilities are per-subsystem booleans.
- **Strict platform isolation** — Linux sysfs in `src/hardware/linux/` and `src/service/backend.rs`; Windows in `src/hardware/windows/` and `src/service/windows/`; everything else gets `UnavailableHardwareProvider`.
- **Two binaries, one frontend feature** — `ui` is a Cargo feature; the root daemon never links Slint (`Cargo.toml`, `src/lib.rs`).

## Layers

**Presentation (Slint):**
- Purpose: Declare the window, screens, components, theme tokens; emit callbacks only.
- Location: `ui/app.slint`, `ui/screens/`, `ui/components/`, `ui/theme/`
- Contains: `AppWindow` properties, `callback` declarations, pure layout/styling.
- Depends on: `std-widgets.slint` and theme tokens only.
- Used by: `src/app.rs` through `slint::include_modules!()` (`src/lib.rs`).

**Presentation bridge (`src/app.rs`):**
- Purpose: Translate callbacks into `AppCommand`; fold `AppEvent` into state and format into Slint properties.
- Contains: `ZenOmenApp`, `install_callbacks`, `initialize`, `handle_event`, `apply_to_ui`.
- Depends on: `crate::core`, `crate::hardware` data types, `slint`.
- Used by: `src/main.rs` only.

**Core (`src/core/`):**
- Purpose: Own the truth of the session, the sampling cadence, and the message contract.
- Location: `src/core/{events,state,worker,telemetry,diagnostics,benchmark}.rs`
- Contains: no `std::fs` hardware reads, no Win32, no sysfs paths.
- Depends on: `crate::hardware` data types, `crate::service::HardwareServiceClient`, `crate::config`, `crate::desktop::autostart`.
- Used by: `src/main.rs` (`spawn_monitoring_worker`), `src/app.rs`, `src/ui/*` (read-only scaffold).

**Hardware abstraction (`src/hardware/`):**
- Purpose: Provide telemetry + capabilities behind one trait; isolate OS quirks.
- Location: `src/hardware/provider.rs` (contract), `linux/`, `windows/`, `mock.rs`, `unavailable.rs`, `registry.rs`, `rgb.rs`
- Contains: `LinuxHardwareProvider`, `WindowsHardwareProvider`, `MockHardware`, `UnavailableHardwareProvider`, `SensorRegistry`.
- Depends on: `std::fs`, `libc`, `windows-sys`, std threads for `nvidia-smi` watcher.
- Used by: `src/main.rs` (provider selection), worker sampling.

**Privileged control (`src/service/`):**
- Purpose: Execute verified hardware mutations under authorization.
- Location: `src/service/{contract,backend,client,polkit,types,protocol}.rs`, `src/service/windows/`
- Contains: D-Bus interface, Polkit verification, retry+read-back writes, SCM service loop, WMI driver, DACL security.
- Depends on: `zbus`, `windows-sys`, `serde`, `serde_json`.
- Used by: `zenomen-hw` binary (server), `src/core/worker.rs` (client).

**Desktop integration (`src/desktop/`):**
- Purpose: Single-instance enforcement, tray, close-to-tray, autostart, releases, root installer.
- Location: `src/desktop/` (XDG/D-Bus) and `src/desktop/windows/` (Win32).
- Contains: `ensure_single_instance`, `register_tray`, `DesktopAction`, `set_autostart_enabled`, `install_system_integration`.
- Depends on: `zbus` blocking, `windows-sys`, `directories`.
- Used by: `src/main.rs`, `src/core/worker.rs` (autostart persistence).

**Supporting domains:**
- `src/config/` — `AppConfig`, `load_or_default`, `save_atomic`, `sanitize`.
- `src/rgb/` — `RgbColor`, `RgbProfile`, `builtin_presets`, `migrate_openomen_profiles`.
- `src/logging.rs` — `init()` returning `LoggingGuard`.

## Data Flow

### Primary Request Path (telemetry)

1. UI callback fires in `ui/app.slint` (e.g. `navigate`, `set-fan-mode`) and is bridged by `ZenOmenApp::install_callbacks` into an `AppCommand` (`src/app.rs:94`).
2. `send_command` uses `SyncSender::try_send` on a depth-8 queue; a full queue drops the click with a `tracing::debug` (`src/app.rs:391`).
3. The worker thread `zenomen-monitor-worker` wakes from `recv_timeout(deadline - now)`, or immediately on `RefreshTelemetry` (`src/core/worker.rs:34`).
4. `sample_and_dispatch` calls `provider.sample_telemetry()` and sends `AppEvent::TelemetryUpdated` or `TelemetrySamplingFailed` over an unbounded `mpsc::Sender<AppEvent>` (`src/core/worker.rs:345`).
5. The `zenomen-event-pump` thread drains events and calls `ZenOmenApp::handle_event` (`src/main.rs:327`).
6. `handle_event` locks `Arc<Mutex<AppState>>`, calls `state.apply(&event)`, then queues `apply_to_ui` via `slint::invoke_from_event_loop` (`src/app.rs:361`).
7. `apply_to_ui` formats values into `SharedString`/`ModelRc` and writes Slint properties (`src/app.rs:420`). The UI never receives raw sensor numbers to interpret.

### Mutating Command Path (fan / power / preset / RGB)

1. `AppCommand::SetFanMode` arrives at the worker loop.
2. `AppEvent::FanModeRequested` is emitted first so the UI can show an in-flight write (`src/core/worker.rs`).
3. `execute_mutating_action` acquires a single-flight `AtomicBool`; a concurrent request is rejected immediately; otherwise it spawns `zenomen-ctrl-{name}` and `zenomen-rpc-{name}` threads with a 30 s timeout (`src/core/worker.rs:370`).
4. The RPC closure calls `HardwareServiceClient`, which talks to `com.zenomen.Hardware1` over system D-Bus (Linux) or `\\.\pipe\ZenOmen.Hardware.v1` (Windows) (`src/service/client.rs`).
5. The daemon authorizes (Polkit `com.zenomen.hardware.control` on Linux; board validation + DACL/session checks on Windows), writes, reads back, and returns the confirmed value (`src/service/contract.rs`, `src/service/backend.rs`).
6. `InFlightGuard` releases the slot on every exit path; the result is mapped to `AppEvent::FanModeChanged(Ok|Err)` and folded into `AppState` (`src/core/worker.rs:467`).

### Desktop Action Path (tray / second instance)

1. Primary instance owns session-bus name `com.zenomen.App` and the tray (`src/desktop/single_instance.rs`, `src/desktop/tray.rs`, or `src/desktop/windows/{single_instance,tray}.rs`).
2. Remote `PresentWindow`/`Quit`/`SetFanMode*` arrives as `DesktopAction` on a `sync_channel(16)` (`src/main.rs:109`).
3. The `zenomen-desktop-actions` thread converts it either into `slint::invoke_from_event_loop` window operations or into `AppCommand` (`src/main.rs:206`).

### Startup Sequence

1. `logging::init()` returns `LoggingGuard` held for process lifetime (`src/main.rs:22`).
2. Root-only integration CLI flags are handled before any windowing (`src/main.rs:55`).
3. `config::load_or_default` reads TOML and reports corruption notice (`src/main.rs:96`).
4. Single-instance + tray registration; non-primary instances exit immediately (`src/main.rs:111`).
5. `AppWindow::new()` → `ZenOmenApp::new()` → `install_callbacks()` (`src/main.rs:175`).
6. Provider selection: `ZENOMEN_MOCK_SCENARIO` override, else `LinuxHardwareProvider` / `WindowsHardwareProvider` / `UnavailableHardwareProvider` (`src/main.rs:240`).
7. `app.initialize(provider.capabilities())` writes the first frame honestly, then `ConfigCorrupted`/`ConfigLoaded` events (`src/main.rs:317`).
8. Event pump thread and monitoring worker start; `run_event_loop_until_quit()` blocks (`src/main.rs:327`).

**State Management:**
- `AppState` is behind `Arc<Mutex<AppState>>`; lock poisoning is tolerated by `lock_state` (`src/app.rs:410`).
- Telemetry history is capped at `MAX_SAMPLES = 120` (`src/core/telemetry.rs:27`); `set_duration` recomputes capacity from polling interval.
- Shutdown is explicit: `AppCommand::Shutdown`, then drop senders in dependency order, then join worker/event-pump/desktop threads (`src/main.rs:350`).

## Key Abstractions

**`HardwareProvider` (trait):**
- Purpose: The single read-only seam between core and OS.
- Examples: `src/hardware/linux/provider.rs`, `src/hardware/windows/provider.rs`, `src/hardware/mock.rs`, `src/hardware/unavailable.rs`
- Pattern: Two methods — `capabilities()` and `sample_telemetry() -> Result<TelemetrySnapshot, String>`; requires `Send + Sync` because the worker owns it.

**`AppState` + `AppEvent::apply`:**
- Purpose: Single source of truth for every displayed value.
- Examples: `src/core/state.rs:31`, `src/core/state.rs:109`
- Pattern: Event-sourced reducer. All other code may read state; only `apply` mutates.

**`ZenOmenApp`:**
- Purpose: The only bridge into the Slint window.
- Examples: `src/app.rs:37`
- Pattern: Holds `Weak<AppWindow>` so workers cannot keep a closed window alive; `Arc<Mutex<AppState>>` for core truth; `SyncSender<AppCommand>` for outbound commands.

**`SensorRegistry`:**
- Purpose: Canonical hardware bindings shared by telemetry and diagnostics.
- Examples: `src/hardware/registry.rs:17`
- Pattern: Scan-once structure; CPU temp resolved by strict priority (`k10temp` → `coretemp` → `acpitz`); fan control restricted to HP/platform hwmon devices.

**`HardwareService` / `HardwareServiceClient`:**
- Purpose: The privileged mutation seam.
- Examples: `src/service/contract.rs:14`, `src/service/client.rs:44`
- Pattern: Typed operations only — no shell commands, no arbitrary paths, no raw payloads. On Linux the daemon is a zbus interface; on Windows it is a length-prefixed JSON frame protocol over a DACL-restricted named pipe.

**`write_and_verify`:**
- Purpose: Make every hardware write observable.
- Examples: `src/service/backend.rs:12`
- Pattern: Write → exponential-backoff sleep → read back → compare; returns `VerificationFailed { target, actual, retries }`.

**Board policy (`BoardProfile` + `BoardValidation` + `authorize_mutation`):**
- Purpose: Windows mutations are blocked unless the exact board and BIOS have recorded physical validation.
- Examples: `src/service/windows/profiles.rs:6`, `src/service/windows/authorization.rs:16`
- Pattern: Catalog lookup by board ID; validation record must match board ID and set the specific `*_write_verified` flag.

**`RgbProfile` / `RgbColor`:**
- Purpose: Strict, typed four-zone lighting model.
- Examples: `src/rgb/schema.rs`
- Pattern: `#RRGGBB` parsing with descriptive errors; unsupported animation fields preserved for forward compatibility.

**GPUI-CE scaffold (`src/ui/`):**
- Purpose: Parallel view-model mirror of the Slint screens for a future toolkit migration.
- Examples: `src/ui/app.rs`, `src/ui/screens/`, `src/ui/components/`, `src/ui/theme/`
- Pattern: `GpuiApp::handle_event` re-derives screen models from `AppState`. It is compiled but **not instantiated by `src/main.rs`**; the active render path is Slint.

## Entry Points

**GUI (`zenomen`):**
- Location: `src/main.rs` (requires `ui` feature; declared in `Cargo.toml` `[[bin]]`).
- Triggers: User launch, desktop entry, tray restore.
- Responsibilities: logging, locale fix, root-only CLI flags, config load, single-instance, tray, app bridge, provider selection, event loop, teardown.

**Privileged daemon (`zenomen-hw`):**
- Location: `src/bin/zenomen-hw.rs` (no `ui` feature required).
- Linux: binds `com.zenomen.Hardware1` on system D-Bus (or session bus if `ZENOMEN_HW_SESSION` is set), serves `HardwareService` over `HardwareBackend`.
- Windows: `--install-service` / `--uninstall-service` for SCM registration, `--console` for foreground testing, default path runs the SCM dispatcher which falls back to console mode.

**CLI integration flags (`zenomen`):**
- Linux: `--install-system-integration`, `--uninstall-system-integration` (root; writes systemd unit, D-Bus config, Polkit policy, desktop entry) — `src/desktop/system_integration.rs`.
- Windows: `--install-service`, `--uninstall-service`, `--install-system-integration`, `--uninstall-system-integration` — `src/desktop/windows/installer.rs`.

**Environment overrides:**
- `ZENOMEN_MOCK_SCENARIO` — `nominal` / `zero` / `gap` selects `MockHardware` instead of the real provider (`src/main.rs:240`).
- `ZENOMEN_HW_SESSION` — serves the daemon on the session bus instead of system bus (`src/bin/zenomen-hw.rs`).
- `RUST_LOG` — `tracing_subscriber::EnvFilter` (`src/logging.rs`).

## Architectural Constraints

- **Threading:** GUI runs a single Slint event loop. Hardware sampling runs only on `zenomen-monitor-worker`. Hardware mutation runs on short-lived `zenomen-ctrl-*` / `zenomen-rpc-*` threads. Desktop actions run on `zenomen-desktop-actions`. Diagnostics export runs on `zenomen-diag-export`. No async runtime is used in the GUI; `zbus` is used in blocking mode; the daemon uses `zbus::block_on`.
- **Global state:** No module-level mutable singletons. Shared state is confined to `Arc<Mutex<AppState>>`, `Arc<Mutex<Option<Arc<SensorRegistry>>>>` (`src/app.rs`), and per-worker `Arc<AtomicBool>` single-flight flags (`src/core/worker.rs`).
- **Circular imports:** `core` names hardware data types; `hardware` never imports `core`. `service::windows` imports `hardware` types for the wire protocol; `hardware::windows` imports `service::windows::profiles` for board capabilities. This is an intentional type-sharing edge, not a runtime cycle.
- **Memory budget:** 120-sample cap (`src/core/telemetry.rs:27`), release profile `opt-level = "s"`, `lto`, `codegen-units = 1`, `strip`, `panic = "abort"` (`Cargo.toml`), Slint pinned without Qt/Skia backends.
- **Feature gating:** `ui = ["dep:slint"]` and `default = ["ui"]`; `src/app.rs` and `src/ui` are `#[cfg(feature = "ui")]` in `src/lib.rs`; `build.rs` skips Slint compilation when `ui` is off.
- **Queue back-pressure:** Command queue depth 8; full queue drops the request instead of blocking the UI (`src/app.rs:30`).
- **Mutation serialization:** Only one hardware write in flight; `InFlightGuard` releases the slot even on timeout or panic (`src/core/worker.rs:370-475`).
- **Absence semantics:** `None`/empty means unread; `0` means a real zero reading. Enforced in `TelemetrySnapshot::reported_sensor_count` and `apply_to_ui` (`src/hardware/provider.rs`, `src/app.rs:420`).
- **Reserved contract:** `src/service/protocol.rs` defines a platform-neutral versioned request/response protocol, but the live D-Bus path uses the `zbus` interface and the live Windows path uses `src/service/windows/protocol.rs`. Do not assume `service::protocol` is on the active path.

## Anti-Patterns

### Mutating Slint properties from a worker thread

**What happens:** A background thread calls `ui.set_cpu_utilization(...)` directly.
**Why it's wrong:** Slint properties are only safe to write on the event loop; cross-thread writes race rendering and can panic. The architecture guarantees workers only send `AppEvent`.
**Do this instead:** Send an `AppEvent` to the event pump and let `ZenOmenApp::handle_event` publish through `slint::invoke_from_event_loop` (`src/app.rs:361`).

### Formatting or interpreting sensor numbers inside `.slint` files

**What happens:** A `.slint` screen computes `root.cpu-utilization + "%"` or divides bytes into MB.
**Why it's wrong:** It puts domain logic in the presentation layer and breaks the "UI receives display strings" contract (`apply_to_ui` formats `percent`/`degrees`/`fan_speed`).
**Do this instead:** Format in `src/app.rs` (`percent`, `degrees`, `fan_speed`, `string_model`) and pass a display-ready string (`src/app.rs:518`).

### Adding a hardware read to `src/core/` or `src/ui/`

**What happens:** A new sensor is read with `std::fs::read_to_string("/sys/...")` inside the worker or a view model.
**Why it's wrong:** It bypasses the `HardwareProvider` boundary and the platform separation rule, and makes testing require real hardware.
**Do this instead:** Add the read to `src/hardware/linux/` and expose it via `TelemetrySnapshot`/`SensorRegistry`; add a mock scenario for tests.

### Writing `AppState` fields outside `apply()`

**What happens:** Code does `state.fan_mode = ...` directly after cloning the mutex guard.
**Why it's wrong:** It breaks the single-writer invariant and lets the core and UI disagree about what is displayed. Note: `src/ui/app.rs` (GPUI scaffold) does direct field assignment; that file is not on the active render path and must not be used as a template.
**Do this instead:** Introduce or reuse an `AppEvent` variant and call `state.apply(&event)` (`src/core/state.rs:109`).

### Fabricating zeros for missing sensors

**What happens:** A provider returns `0.0` or `0` when a sensor file cannot be read.
**Why it's wrong:** The status strip and charts would report "0 °C" or "0 of 11 sensors" as measured facts; the project's core value forbids invented readings.
**Do this instead:** Return `None`/empty and let `has_any_reading()`/`reported_sensor_count()` drive "waiting for first reading" or per-panel `Unavailable` states (`src/hardware/provider.rs`, `src/core/state.rs:286`).

### Adding platform code without a cfg gate

**What happens:** A `windows-sys` call or `/sys/` path is referenced unconditionally.
**Why it's wrong:** It breaks the other platform build and violates the strict separation rule in `AGENTS.md`.
**Do this instead:** Put code under `#[cfg(target_os = "windows")]` in `src/hardware/windows/` or `src/service/windows/`, and mirror Linux in `src/hardware/linux/` / `src/service/backend.rs`. Unsupported platforms use `UnavailableHardwareProvider`.

## Error Handling

**Strategy:** Typed errors at boundaries, human-readable strings toward the UI, never a panic for a hardware/IO failure.

**Patterns:**
- Provider sampling returns `Result<TelemetrySnapshot, String>`; the worker converts `Err` into `AppEvent::TelemetrySamplingFailed` (`src/core/worker.rs:345`).
- Privileged control uses `HardwareControlError` (`thiserror`) with variants `IoError`, `VerificationFailed`, `UnsupportedMode`, `PolkitDenied`, `InvalidParameter`, `InterfaceMissing`, `ConnectionFailed`, `ProtocolError`, `AccessDenied` (`src/service/types.rs`).
- `HardwareControlError` maps to `zbus::fdo::Error` so callers receive structured D-Bus faults (`src/service/types.rs`).
- Mutating actions get a 30 s timeout and return `Err("Operation timed out ...")`; the mutation slot stays held until the underlying RPC actually finishes (`src/core/worker.rs:390`).
- UI control failures become `AppState.last_control_error` and the dismissible banner (`AppState::apply`, `AppEvent::ControlErrorDismissed`).
- Corrupt config is backed up to `config.toml.bak.<timestamp>` and reported as `AppEvent::ConfigCorrupted` (`src/config/mod.rs`).
- Logging is best-effort: if the log directory cannot be created, logging degrades to stderr-only rather than blocking startup (`src/logging.rs`).
- Poisoned mutexes are recovered via `unwrap_or_else(|p| p.into_inner())` (`src/app.rs:410`).

## Cross-Cutting Concerns

**Logging:** `tracing` macros throughout; `src/logging.rs` installs a stderr layer plus a daily rotating file (`tracing_appender`), returns `LoggingGuard` that must live for the process. Log locations: Linux `~/.local/state/zenomen/logs`; Windows `%LOCALAPPDATA%\ZenOmen\logs` for the GUI and `%PROGRAMDATA%\ZenOmen\logs` for the service.

**Validation:** Screen indices via `ScreenId::from_index` (unknown → rejected, not clamped); fan mode via `FanMode::parse_requested` (only `auto`/`max` in v1); color via `RgbColor::from_hex`; config via `AppConfig::sanitize` clamping polling to 500–5000 ms and history window bounds; IPC frames capped at 64 KiB.

**Authentication:** Linux — Polkit action `com.zenomen.hardware.control` checked before every mutating D-Bus method (`src/service/contract.rs`, `src/service/polkit.rs`). Windows — named-pipe DACL `D:(A;;GA;;;SY)(A;;GA;;;BA)(A;;GRGW;;;IU)`, interactive-console-session verification, and board validation records (`src/service/windows/{security,authorization}.rs`).

**Process lifetime:** Single-instance enforced by session D-Bus name (Linux) or named mutex (Windows). Close-to-tray is controlled by `Arc<AtomicBool>` shared with the Slint close handler. Shutdown is explicit and ordered (`src/main.rs:350`).

---

*Architecture analysis: 2026-11-08*
