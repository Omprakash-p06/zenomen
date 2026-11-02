---
last_mapped_commit: e50af8f9ea61300a1c10d36dc6954e319da7b6e2
last_mapped_at: 2026-11-02
---
<!-- refreshed: 2026-11-02 -->

# Architecture

**Analysis Date:** 2026-11-02

## System Overview

ZenOmen is a two-process system: an unprivileged Slint desktop UI (`zenomen`) and a privileged system D-Bus daemon (`zenomen-hw`). The GUI reads telemetry directly from Linux `procfs`/`sysfs`; every hardware *write* crosses the D-Bus boundary into the daemon, which re-validates the request, checks polkit, writes sysfs, and reads the value back to confirm it stuck.

```text
┌──────────────────────────────────────────────────────────────────────────┐
│  UI THREAD (unprivileged process `zenomen`)                               │
│  `ui/app.slint` → `AppWindow` (compiled by `build.rs`)                    │
│  screens: overview, cooling, performance, lighting, settings, diagnostics │
│  `ui/screens/*.slint`, `ui/components/*.slint`, `ui/theme/*.slint`        │
└───────────────┬──────────────────────────────────────────▲───────────────┘
                │ Slint callbacks (navigate, set-fan-mode,  │ properties
                │ set-power-profile, apply-rgb-preset, …)   │ (formatted strings)
                ▼                                          │
┌──────────────────────────────────────────────────────────┴───────────────┐
│  BRIDGE  `src/app.rs` — `ZenOmenApp`                                      │
│  UI → core: `AppCommand` via bounded `SyncSender` (try_send, depth 8)    │
│  core → UI: `AppEvent` → `AppState::apply` → `invoke_from_event_loop`    │
└───────┬───────────────────────────────────────────────────────▲──────────┘
        │ command channel (mpsc)                                │ event channel (mpsc)
        ▼                                                       │
┌───────────────────────────────┐                 ┌───────────────┴─────────┐
│ MONITOR WORKER                │                 │ EVENT PUMP              │
│ `src/core/worker.rs`          │                 │ `src/main.rs`           │
│ 1 Hz deadline loop            │                 │ drains AppEvent, folds  │
│ visibility-gated sampling     │                 │ into `AppState`, queues │
│ service probe every 5 ticks   │                 │ UI update               │
└───────┬───────────────────────────────┬─────────────────────────────────┘
        │ read path                     │ write path
        ▼                               ▼
┌───────────────────────────┐   ┌──────────────────────────────────────────┐
│ READ BOUNDARY             │   │ CONTROL BOUNDARY                         │
│ `src/hardware/`           │   │ `src/service/client.rs` (zbus blocking)  │
│ `HardwareProvider` trait  │   │            │ system D-Bus                 │
│ `LinuxHardwareProvider`   │   │            ▼                             │
│ `SensorRegistry`          │   │ `src/service/contract.rs`                │
│ linux/{cpu,gpu,hwmon,mem} │   │   authorize() → polkit                   │
│ `MockHardware`            │   │            ▼                             │
│ /proc, /sys (read-only)   │   │ `src/service/backend.rs`                 │
└───────────────────────────┘   │   write_and_verify() → read-back         │
                                └──────────────────┬───────────────────────┘
                                                   │ sysfs writes (root)
                                                   ▼
                                ┌──────────────────────────────────────────┐
                                │ PRIVILEGED DAEMON `zenomen-hw`           │
                                │ `src/bin/zenomen-hw.rs`                  │
                                │ systemd: ProtectSystem=strict, no caps   │
                                └──────────────────────────────────────────┘
```

## Component Responsibilities

| Component | Responsibility | File |
|-----------|----------------|------|
| GUI entry point | Parse flags, load config, single-instance, tray, spawn worker + event pump, run Slint loop, ordered teardown | `src/main.rs` |
| Slint shell / router | Window properties, screen routing by `current-screen`, global callbacks | `ui/app.slint` |
| Slint screens | Render only properties handed to them; emit callbacks; never sample | `ui/screens/*.slint` |
| Slint components | Reusable presentation primitives (metric card, status pill, unavailable panel, chart, sidebar) | `ui/components/*.slint` |
| Slint theme | Colors, typography, metrics + bundled fonts | `ui/theme/*.slint`, `ui/theme/fonts/` |
| UI ↔ core bridge | Callbacks → `AppCommand`; `AppEvent` → state → window properties | `src/app.rs` (`ZenOmenApp`) |
| Message contract | `AppCommand`, `AppEvent`, `ScreenId` | `src/core/events.rs` |
| Core state | Single source of truth; `apply()` is the only mutator; status/degraded logic | `src/core/state.rs` |
| Monitoring worker | 1 Hz sampling, visibility gating, on-demand refresh, service probes, mutation offload | `src/core/worker.rs` |
| Telemetry history | Bounded `VecDeque`, SVG path generation, gaps for missing samples | `src/core/telemetry.rs` |
| Diagnostics | Build/export JSON report, read OS/DMI/kernel metadata | `src/core/diagnostics.rs` |
| Benchmark instrumentation | RSS/PSS/private-dirty reads and cold-start timing | `src/core/benchmark.rs` |
| Provider contract | `HardwareProvider` trait, `HardwareCapabilities`, `TelemetrySnapshot` | `src/hardware/provider.rs` |
| Linux provider | Sample CPU/RAM/fan/power profile/GPU from kernel interfaces | `src/hardware/linux/provider.rs` |
| Sensor registry | Canonical CPU-temp and fan-tach bindings shared by dashboard + diagnostics | `src/hardware/registry.rs` |
| Linux parsers | `/proc/stat`, `/proc/meminfo`, `/proc/cpuinfo`, cpufreq, hwmon, DRM/`nvidia-smi` | `src/hardware/linux/{cpu,memory,hwmon,gpu}.rs` |
| Mock provider | Deterministic scenarios (`Nominal`, `ZeroCapabilities`, `SensorDataGap`) | `src/hardware/mock.rs` |
| RGB schema / migration | Strict `#RRGGBB`, 4-zone profiles, OpenOMEN import | `src/rgb/{schema,migration}.rs` |
| D-Bus client | Blocking zbus calls to `com.zenomen.Hardware1` | `src/service/client.rs` |
| D-Bus contract | Interface definition, polkit gate, parameter validation | `src/service/contract.rs` |
| Hardware backend | Verified sysfs writes with retry/backoff and read-back | `src/service/backend.rs` |
| Service types | `FanMode`, `PresetResult`, `HardwareControlError` and zbus error mapping | `src/service/types.rs` |
| Polkit verifier | `CheckAuthorization` against `org.freedesktop.PolicyKit1.Authority` | `src/service/polkit.rs` |
| Privileged daemon entry | Own `com.zenomen.Hardware1` on system bus, serve interface | `src/bin/zenomen-hw.rs` |
| Desktop integration | SNI tray, single instance, autostart, releases link, system installer | `src/desktop/*.rs` |
| Configuration | TOML schema, atomic save, corrupt-file backup and defaults | `src/config/{mod,schema}.rs` |
| Library facade | Module exports + `slint::include_modules!()` | `src/lib.rs` |
| Build script | Compiles `ui/app.slint` with a pinned `fluent` style; Windows icon | `build.rs` |

## Pattern Overview

**Overall:** Layered hexagonal-ish architecture with a typed message bus between UI and core, a trait-based read boundary, and a fixed-method RPC boundary for writes.

**Key Characteristics:**
- **Unidirectional data flow per direction.** UI emits commands; core emits events. There is no shared mutable UI object and no direct hardware access from a callback.
- **The UI is a pure projection.** Window properties are already-formatted strings, lists and booleans; the Slint layer performs no measurement, no unit conversion and no capability inference.
- **Two distinct hardware paths.** Telemetry *reads* stay in-process (unprivileged `procfs`/`sysfs`). All *writes* are RPCs to a root daemon.
- **Honest absence encoded in types.** Missing sensors are `None`/empty collections, not zero. `TelemetrySnapshot::reported_sensor_count()` and `AppState::status_text()` measure presence rather than assuming it.
- **Verification at the last mile.** `write_and_verify()` writes, sleeps, reads back, retries with exponential backoff, and returns `VerificationFailed { target, actual, retries }` instead of claiming success.
- **One long-lived sampling thread.** The UI thread never blocks on sensor I/O; sampling pauses entirely while the window is hidden.

## Layers

**1. Presentation (Slint):**
- Purpose: Draw the window and route six screens; emit callbacks for user intent.
- Location: `ui/app.slint`, `ui/screens/`, `ui/components/`, `ui/theme/`.
- Contains: Declarative components, properties, callbacks, keyboard focus handling.
- Depends on: Nothing in Rust at compile time; `build.rs` compiles it, `src/lib.rs` includes the generated `AppWindow`.
- Used by: `src/main.rs` (window creation), `src/app.rs` (property writes, callback installation).

**2. Bridge (`src/app.rs`):**
- Purpose: Be the *only* path between Slint and the core.
- Location: `src/app.rs`.
- Contains: `ZenOmenApp` (holds `Weak<AppWindow>`, `SyncSender<AppCommand>`, `Arc<Mutex<AppState>>`, optional registry, close-to-tray atomic), `install_callbacks()`, `initialize()`, `handle_event()`, `apply_to_ui()`, `send_command()`.
- Depends on: `crate::core` types, `crate::hardware` data types, Slint.
- Used by: `src/main.rs`, and the event-pump thread.

**3. Core:**
- Purpose: Own command/event semantics, application state, sampling cadence, bounded history, diagnostics.
- Location: `src/core/{events,state,worker,telemetry,diagnostics,benchmark}.rs`.
- Contains: `AppCommand`/`AppEvent`/`ScreenId`, `AppState`, `spawn_monitoring_worker`, `TelemetryHistory`, `DiagnosticsReport`.
- Depends on: `crate::hardware` (data types + provider trait), `crate::config`, `crate::service` client, `crate::desktop` (autostart), `crate::rgb`.
- Used by: `src/app.rs`, `src/main.rs`.

**4. Hardware read boundary:**
- Purpose: Turn kernel interfaces into one immutable `TelemetrySnapshot` per tick.
- Location: `src/hardware/`.
- Contains: `HardwareProvider` trait, `LinuxHardwareProvider`, `SensorRegistry`, `linux/{hwmon,cpu,memory,gpu}`, `MockHardware`, `RgbInterfaceVariant` detection (read-only).
- Depends on: `std::fs`, `std::process::Command` (`nvidia-smi`), `std::thread` (timeout probe).
- Used by: the monitor worker only.

**5. Control boundary / privileged service:**
- Purpose: Accept only predefined operations, authorize the caller, validate parameters, write sysfs, verify the result.
- Location: `src/service/{client,contract,backend,polkit,types}.rs`, `src/bin/zenomen-hw.rs`.
- Contains: blocking client, `#[zbus::interface]` contract, `HardwareBackend` rooted at a configurable sysfs prefix, polkit gate, typed errors.
- Depends on: `zbus`, `serde_json`, `thiserror`, `libc` `geteuid` via `extern` in the installer.
- Used by: the monitor worker (via client), tests (backend with a temp root), and the daemon binary.

**6. Desktop integration:**
- Purpose: Make the GUI a well-behaved desktop citizen without a heavy toolkit.
- Location: `src/desktop/{single_instance,tray,autostart,releases,system_integration}.rs`.
- Contains: session-bus `com.zenomen.App` single instance, hand-written StatusNotifierItem + dbusmenu, XDG autostart, `xdg-open` releases link, root-only installer.
- Depends on: `zbus` (session bus), `directories`, `include_str!` of `data/*`.
- Used by: `src/main.rs`, `src/app.rs` (releases/export), worker (autostart).

**7. Configuration:**
- Purpose: Persist UI/telemetry preferences safely.
- Location: `src/config/{mod,schema}.rs`.
- Contains: `AppConfig { general, telemetry }`, `sanitize()`, `load_or_default()`, `save_atomic()`, `project_dirs()`.
- Depends on: `toml`, `serde`, `directories`.
- Used by: `src/main.rs`, worker, `src/app.rs`.

**8. RGB:**
- Purpose: Model lighting looks and import legacy profiles.
- Location: `src/rgb/{schema,migration}.rs`, `src/hardware/rgb.rs`.
- Contains: `RgbColor`, `RgbProfile`, `RgbError`, built-in presets, migration report, interface variant detection.
- Depends on: `serde`, `thiserror`, `slint` (for `Color` conversion in `schema.rs`).
- Used by: bridge (preset lookup), worker (apply command), service backend (sysfs writes), contract (JSON parsing).

## Data Flow

### Primary Read Path (telemetry tick)

1. Worker wakes on its deadline or an `AppCommand::RefreshTelemetry` (`src/core/worker.rs:99-143`).
2. `sample_and_dispatch()` calls `provider.sample_telemetry()` (`src/core/worker.rs:331-346`).
3. `LinuxHardwareProvider::sample_telemetry()` reads `/proc/stat`, cpufreq or `/proc/cpuinfo`, `SensorRegistry` CPU temp, `/proc/meminfo`, registry fan RPMs, ACPI `platform_profile*`, then GPU via `nvidia-smi` (200 ms budget) or DRM sysfs (`src/hardware/linux/provider.rs:115-198`).
4. On success the worker sends `AppEvent::TelemetryUpdated(snapshot)`; on error `TelemetrySamplingFailed(err)`.
5. The event-pump thread (`zenomen-event-pump`) calls `ZenOmenApp::handle_event` (`src/main.rs:191-200`).
6. `AppState::apply` stores the snapshot, pushes a `TelemetryPoint`, regenerates six SVG path strings (`src/core/state.rs:116-133`).
7. `slint::invoke_from_event_loop` runs `apply_to_ui`, which formats numbers to strings, maps capability flags to booleans and sets window properties (`src/app.rs:342-353`, `386-519`).
8. Slint re-renders the active screen; an absent value stays an empty string and renders as `Unavailable` / `Waiting for first reading`.

### Primary Write Path (`set fan max`)

1. Cooling screen emits `set-fan-mode("max")`; `ZenOmenApp::on_set_fan_mode` turns it into `AppCommand::SetFanMode` and `try_send`s it (`src/app.rs:129-133`, `357-369`).
2. Worker receives the command and calls `execute_mutating_action("fan", …)` (`src/core/worker.rs:222-232`).
3. A short-lived `zenomen-ctrl-fan` thread spawns `zenomen-rpc-fan`, waits up to 30 s, and always reports an `AppEvent::FanModeChanged` (`src/core/worker.rs:350-380`).
4. `HardwareServiceClient::set_fan_mode` calls `SetFanMode` on the system bus (`src/service/client.rs:123-141`).
5. `HardwareService::set_fan_mode` takes the D-Bus header, calls `authorize()` → polkit `CheckAuthorization` with `allow_user_interaction = true` (`src/service/contract.rs:55-79`, `116-127`).
6. `HardwareBackend::set_fan_mode` parses the requested mode (only `auto`/`max`), resolves `pwm1_enable`, and calls `write_and_verify(path, "0", 3, 50 ms)` (`src/service/backend.rs:12-48`, `162-176`).
7. The backend re-reads the mode and returns the confirmed string; the client deserializes it; the worker emits `FanModeChanged(Ok(confirmed))`.
8. `AppState::apply` clears `pending_fan_mode`, stores `fan_mode`, clears `last_control_error`; the UI is updated on the event loop.
9. On failure the error string lands in `last_control_error` and renders in the screen's error banner.

### Startup Flow

1. `main()` intercepts `--install-system-integration` / `--uninstall-system-integration` before any GUI work (`src/main.rs:20-45`).
2. Loads `~/.config/zenomen/config.toml` via `load_or_default`; a corrupt file is backed up as `config.toml.bak.<timestamp>` and a notice is queued (`src/config/mod.rs:34-78`).
3. Resolves `--minimized`/`-m` and `config.general.start_minimized` (`src/main.rs:52-54`).
4. Requests `com.zenomen.App` on the session bus; a second instance invokes `PresentWindow` and exits; no session bus means standalone mode (`src/main.rs:56-72`, `src/desktop/single_instance.rs`).
5. Initialises tracing, logs `cold-start`, migrates OpenOMEN RGB profiles non-destructively (`src/main.rs:74-95`).
6. Creates `AppWindow`, builds `ZenOmenApp`, installs callbacks, wires close-to-tray, starts the desktop-action thread (`src/main.rs:97-151`).
7. Chooses provider: `MockHardware::from_env()` when `ZENOMEN_MOCK_SCENARIO` is set, otherwise `LinuxHardwareProvider` plus its `SensorRegistry` (`src/main.rs:153-180`).
8. `app.initialize(provider.capabilities())` writes capabilities, diagnostics identity and config directly on the UI thread *before* the loop so the first frame is honest (`src/app.rs:292-320`).
9. Starts the event pump, then `spawn_monitoring_worker`, then shows the window or starts minimized (`src/main.rs:190-214`).

### Shutdown Flow

1. Event loop exits (`quit_event_loop` from tray/close or `Quit` desktop action).
2. `AppCommand::Shutdown` is `try_send`-ed, then `command_sender` is dropped (`src/main.rs:216-218`).
3. `ui` and `app` are dropped (releases the strong window reference).
4. Worker is joined; it exits on `Shutdown` or on command-channel disconnect and drops its `event_sender` clone.
5. Event pump is joined after its senders are gone.
6. Session connection is dropped, then the desktop-action thread is joined (`src/main.rs:219-224`).

### Desktop Interaction Flow

1. Tray `Activate` or a second instance's `PresentWindow` sends `DesktopAction::PresentWindow` into a depth-16 sync channel (`src/desktop/tray.rs`, `src/desktop/single_instance.rs`).
2. `zenomen-desktop-actions` receives it, calls `slint::invoke_from_event_loop` on the UI thread, shows/unminimizes the window and sends `AppCommand::SetUiVisible(true)` (`src/main.rs:121-151`).
3. The worker immediately samples once and resets its deadline (`src/core/worker.rs:122-134`).

**State Management:**
- `AppState` lives in `Arc<Mutex<AppState>>` inside `ZenOmenApp`; only `apply(&AppEvent)` mutates it. Lock poisoning is tolerated by recovering the inner guard (`src/app.rs:376-380`).
- The UI holds no authoritative values — only formatted properties written from `apply_to_ui`.
- Persistent state: `~/.config/zenomen/config.toml`, migrated RGB profiles under the ZenOmen config dir, diagnostics exports in `config_dir/diagnostics/`.
- In-memory history: `TelemetryHistory` (`VecDeque`, default 120, configurable to 30/60/120/300 via `TelemetryConfig::history_window`).

## Key Abstractions

**`HardwareProvider` (`src/hardware/provider.rs:232-239`):**
- Purpose: The single seam between core and any telemetry backend.
- Examples: `LinuxHardwareProvider`, `MockHardware`.
- Pattern: `Send + Sync` trait with `capabilities()` and `sample_telemetry() -> Result<TelemetrySnapshot, String>`; the error is a human-readable string, never a panic.

**`TelemetrySnapshot` + `HardwareCapabilities` (`src/hardware/provider.rs`):**
- Purpose: Plain, serialisable data crossing threads and the diagnostics export.
- Pattern: Optional fields mean "not read this tick"; `reported_sensor_count()` and `EXPECTED_SENSOR_COUNT = 12` define honesty; capabilities are one flag per subsystem (`SUBSYSTEM_COUNT = 8` in `src/core/state.rs:23`).

**`SensorRegistry` (`src/hardware/registry.rs`):**
- Purpose: Bind each logical sensor to exactly one sysfs path once, so dashboard and diagnostics cannot disagree.
- Pattern: Strict driver priority (`k10temp` → `coretemp` → `acpitz`), label preference (`Tctl`/`Tdie`, `Package id 0`), preferred `hp`/`hp-wmi` devices for fans, then `read_*` methods used by the provider and diagnostics.

**`AppCommand` / `AppEvent` (`src/core/events.rs`):**
- Purpose: The typed, exhaustive contract in both directions.
- Pattern: Nothing carries a path or a shell command; `ScreenId` is a compile-time set where `from_index` is the only numeric entry point and unknown indices are refused.

**`AppState::apply` (`src/core/state.rs:107-246`):**
- Purpose: Fold every event into one authoritative snapshot.
- Pattern: One match over `AppEvent`, no I/O, no formatting beyond status text; `is_degraded()` compares capability count and per-sample report count.

**`write_and_verify` (`src/service/backend.rs:12-48`):**
- Purpose: Make "write succeeded" mean "the kernel now reports the value".
- Pattern: Write → sleep → read back → compare case-insensitively → retry with doubled delay → `VerificationFailed` with target, actual, retries. Used by fan, power profile and four-zone/legacy RGB; the kernel-patch RGB path does its own byte-exact read-back (`src/service/backend.rs:437-472`).

**`HardwareBackend` sysfs root (`src/service/backend.rs:50-74`):**
- Purpose: Same code runs against `/` in production and a temp directory in tests.
- Pattern: `resolve_path()` prefixes every absolute sysfs path with a configurable root; tests build `sys/...` trees under `std::env::temp_dir()`.

**`TelemetryHistory` (`src/core/telemetry.rs`):**
- Purpose: Bounded, allocation-stable chart history.
- Pattern: `VecDeque::with_capacity`, `pop_front` before `push_back`; `None` is a gap, so `generate_svg_path` lifts the pen instead of drawing through missing data.

**`ScreenId` (`src/core/events.rs:16-67`):**
- Purpose: Keep sidebar order, routing indices, titles and the UI in lockstep.
- Pattern: `ALL`, `index()`, `from_index()`, `title()` — four match arms that must be updated together.

**`PresetResult` (`src/service/types.rs`):**
- Purpose: Report combined fan+power application without claiming false atomicity.
- Pattern: `power_applied`, `fan_applied`, `partial_success`, `message`; surfaces granular failures in the Performance screen.

## Entry Points

**GUI binary:**
- Location: `src/main.rs:19` (`fn main`), package binary `zenomen` (`Cargo.toml`).
- Triggers: desktop launcher, `zenomen`, `zenomen --minimized|-m`, `zenomen --install-system-integration`, `zenomen --uninstall-system-integration`.
- Responsibilities: Flags, config, single instance, tray, provider selection, thread orchestration, event loop, ordered teardown.

**Privileged daemon:**
- Location: `src/bin/zenomen-hw.rs:11`.
- Triggers: `zenomen-hw.service` (`data/systemd/zenomen-hw.service`, `Type=dbus`) or D-Bus activation (`data/dbus/com.zenomen.Hardware1.service`, `User=root`).
- Responsibilities: Connect to system bus (or session bus when `ZENOMEN_HW_SESSION` is set, for tests), own `com.zenomen.Hardware1`, serve `HardwareService` at `/com/zenomen/Hardware1`, keep the future pending.
- Environment: `RUST_LOG`, `ZENOMEN_HW_SESSION` (testing only).

**Slint definition:**
- Location: `ui/app.slint` (`export component AppWindow`), compiled by `build.rs` into `src/lib.rs` via `slint::include_modules!()`.
- Triggers: every GUI launch.
- Responsibilities: Window shell, screen router by `current-screen`, global callback declarations, keyboard focus contract.

**Environment-selected provider:**
- Location: `src/hardware/mock.rs` (`from_env`), consumed in `src/main.rs:158-166`.
- Triggers: `ZENOMEN_MOCK_SCENARIO=nominal|zero|gap`.
- Responsibilities: Reproducible zero-capability and sensor-gap runs against the real binary.

**Installer mode:**
- Location: `src/desktop/system_integration.rs`; invoked by `src/main.rs:20-45`.
- Triggers: `sudo zenomen --install-system-integration`.
- Responsibilities: Atomically install desktop entry, systemd unit, D-Bus service/conf, polkit policy, daemon binary; refuses when not root.

## Architectural Constraints

- **Threading:** One Slint UI thread; one long-lived monitor worker; one event-pump thread; one desktop-action thread. Mutating RPCs are offloaded to a per-operation `zenomen-ctrl-*` thread with a nested `zenomen-rpc-*` thread and a 30 s timeout so a polkit dialog can never stall sampling (`src/core/worker.rs:348-380`). Two additional short-lived threads exist: the `nvidia-smi` timeout probe (`src/hardware/linux/gpu.rs:188-190`) and diagnostics export (`src/app.rs:227-229`). No thread-per-sensor design.
- **UI thread purity:** Callbacks only enqueue commands; `try_send` drops on a full queue (depth 8) rather than blocking (`src/app.rs:30`, `357-369`). UI mutation happens exclusively inside `slint::invoke_from_event_loop`.
- **Window lifetime:** `ZenOmenApp` holds `Weak<AppWindow>`; worker threads must never own a strong window reference (`src/app.rs:36-43`).
- **Visibility gating:** When `ui_visible == false`, the worker blocks on `recv()` and performs no sampling, which is what holds idle CPU near zero (`src/core/worker.rs:99-118`). Restoring visibility samples immediately.
- **Privilege boundary:** The GUI process is unprivileged and only reads `/proc` and `/sys`. Every write is a D-Bus call to a root daemon. The interface accepts only predefined methods with scalar/JSON arguments — no path, no command, no shell. Parameter validation is duplicated deliberately at the service edge (`FanMode::parse_requested`, profile-choice membership, illegal-character rejection, strict RGB JSON parse).
- **Daemon hardening:** `ProtectSystem=strict`, `ProtectHome=true`, `PrivateTmp`, `NoNewPrivileges`, empty `CapabilityBoundingSet`, `SystemCallFilter=@system-service`, `MemoryDenyWriteExecute`; sysfs write carve-outs are scoped to `hp-wmi`, ACPI, hwmon and LEDs (`data/systemd/zenomen-hw.service`).
- **Polkit:** Only `com.zenomen.hardware.control` exists; `allow_any`/`allow_inactive`/`allow_active` are `auth_admin` / `auth_admin_keep` (`data/polkit/com.zenomen.hardware.policy`). The service checks authorization on every mutating method using the message sender, and permits interactive authentication.
- **Memory ceiling:** Slint pinned to `1.17` with `backend-winit` + `renderer-femtovg` and *without* Qt/Skia (`Cargo.toml`); release profile uses `opt-level = "s"`, LTO, `codegen-units = 1`, `panic = "abort"`, `strip`.
- **Bounded history:** `TelemetryHistory` never grows past the configured window; `src/core/telemetry.rs` is the only place chart geometry is computed, so it is testable headlessly.
- **D-Bus naming:** system bus `com.zenomen.Hardware1` at `/com/zenomen/Hardware1`; session bus `com.zenomen.App` at `/com/zenomen/App`; tray `org.kde.StatusNotifierItem` at `/StatusNotifierItem` with `com.canonical.dbusmenu` at `/MenuBar`.
- **Read path asymmetry (accepted):** fan *mode* and polkit/service status come from the daemon client, while CPU/GPU/RAM/fan RPM/power-profile choices come from the local provider. `TelemetrySnapshot::fan_mode` is populated only by the mock; on Linux the UI uses `AppState::fan_mode` from `FanModeChanged`.

## Anti-Patterns

### Reading hardware in a UI callback

**What happens:** Adding a `std::fs::read_to_string("/sys/...")` or a client call directly inside a Slint callback or screen component.
**Why it's wrong:** The UI thread blocks on sensor I/O or a polkit round-trip; the window freezes, violating the responsiveness guarantee and the "UI never touches hardware" constraint.
**Do this instead:** Add an `AppCommand` in `src/core/events.rs`, queue it from `src/app.rs`, and let `src/core/worker.rs` do the work. Return the result as an `AppEvent`.

### Fabricating a reading for a missing sensor

**What happens:** Defaulting a temperature to `0.0`, an RPM list to `[0, 0]`, or RAM usage to `"0 MB"` when the sensor was absent.
**Why it's wrong:** The project's core value forbids numbers the app did not measure; `0 °C` and `0 RPM` are plausible-looking lies that break diagnostics and user trust.
**Do this instead:** Use `Option`/empty `Vec`/`String::new()` in `TelemetrySnapshot`, and render `Unavailable` (see `src/app.rs:444-461` and `fan_speed()` at `src/app.rs:533-539`).

### Adding a D-Bus method that takes a path or command

**What happens:** Exposing `write_file(path, value)` or shelling out from `HardwareService` to "support one more model".
**Why it's wrong:** It turns a root daemon into an arbitrary-write/arbitrary-exec primitive for any bus caller; the fixed contract is the entire security argument.
**Do this instead:** Add a named operation (`SetFanMode`-style) to `src/service/contract.rs`, validate its arguments in `src/service/backend.rs`, and let the backend resolve the sysfs path itself.

### Writing sysfs without read-back

**What happens:** Calling `fs::write(path, value)` and returning `Ok(())`.
**Why it's wrong:** The kernel may silently ignore or clamp a write (mode not supported, firmware race); the UI would report success that did not happen.
**Do this instead:** Route through `write_and_verify()` (`src/service/backend.rs:12-48`) and re-read the confirmed state before responding; return `HardwareControlError::VerificationFailed` with target/actual/retries.

### Treating "sampling failed" as a crash or a silent zero

**What happens:** `unwrap()` on a sensor read, or swallowing the error and emitting a default snapshot.
**Why it's wrong:** A missing GPU or a transient hwmon read must degrade, not panic and not invent data; the status strip explicitly distinguishes sampling error from waiting-for-first-reading (`src/core/state.rs:252-268`).
**Do this instead:** Return `Err(String)` from the provider, emit `AppEvent::TelemetrySamplingFailed`, and keep the last good snapshot plus an explicit error line.

### Duplicating RGB interface discovery

**What happens:** Interface probing exists twice: read-only `detect_rgb_interface()` in `src/hardware/rgb.rs` and writable `HardwareBackend::find_rgb_interface()` in `src/service/backend.rs`.
**Why it's wrong:** The two lists of sysfs paths can drift, so capabilities can claim lighting while the daemon cannot find a writable path (or vice versa).
**Do this instead:** When adding a fourth RGB interface, update both functions and `tests/rgb_tests.rs` in the same change, or refactor the path table into one shared constant list.

### Adding a sensor without updating the honesty counters

**What happens:** Adding a telemetry field but leaving `TelemetrySnapshot::EXPECTED_SENSOR_COUNT` (12) or `SUBSYSTEM_COUNT` (8) untouched.
**Why it's wrong:** The status strip and degraded logic silently under-report, and `tests/sensor_registry_tests.rs` / `src/core/state.rs` tests stop meaning what they claim.
**Do this instead:** Update `EXPECTED_SENSOR_COUNT`, the `reported_sensor_count()` array, any new `HardwareCapabilities` flag plus `SUBSYSTEM_COUNT`, the mock provider, and the corresponding tests.

## Error Handling

**Strategy:** Typed errors at boundaries, human-readable strings across threads, verified writes at the hardware edge, and graceful degradation everywhere.

**Patterns:**
- Provider sampling returns `Result<TelemetrySnapshot, String>`; the worker converts failures into `AppEvent::TelemetrySamplingFailed` and never panics (`src/core/worker.rs:331-346`).
- Service errors are `HardwareControlError` (`IoError`, `VerificationFailed`, `UnsupportedMode`, `PolkitDenied`, `InvalidParameter`, `InterfaceMissing`) mapped to `zbus::fdo::Error` (`src/service/types.rs:120-141`).
- The client wraps every call in `map_err(|e| format!("<Method> failed: {e}"))` so the UI gets an actionable string (`src/service/client.rs`).
- Mutating operations get a 30 s timeout with an explicit "timed out waiting for authorization or hardware response" message (`src/core/worker.rs:368-376`).
- Config corruption is non-fatal: timestamped backup, safe defaults, `AppEvent::ConfigCorrupted` notice (`src/config/mod.rs:52-77`).
- Optional reads use `let Ok(...) = ... else { return None/Vec::new() }`, preserving absence rather than erroring (`src/hardware/registry.rs`, `src/hardware/linux/provider.rs`).
- `panic = "abort"` in release means a panic terminates the process; there is no unwinding recovery path.

## Cross-Cutting Concerns

**Logging:** `tracing` + `tracing-subscriber` with `RUST_LOG` env filter; structured fields (`screen`, `mode`, `profile`, `error`, `path`), `info` for lifecycle and successful writes, `debug` for worker cadence, `warn`/`error` for degraded paths. Benchmarks are logged via `log_benchmark()` (`src/core/benchmark.rs`).

**Validation:** Argparse-equivalent is Clap-free; CLI flags are matched by `std::env::args().any`. Config is clamped in `AppConfig::sanitize()`. Service arguments are validated in the backend, not trusted from the client. RGB hex is validated strictly in `RgbColor::from_hex`.

**Authentication/Authorization:** Unix euid only for the installer (`is_root()` via `geteuid`); polkit for hardware control. No accounts, no network auth, no cloud.

**Performance:** Fixed 1 Hz default cadence (configurable 500–5000 ms), hidden-window sampling pause, no thread per sensor, bounded history, pre-allocated ring buffer, SVG strings generated once per tick, release profile tuned for size/RSS.

**Privacy:** All data local; the only network action is opening the GitHub Releases page in the user's browser (`src/desktop/releases.rs`). No telemetry upload, no HTTP client in the tree.

**Packaging:** Cargo Packager metadata in `Cargo.toml` ships both binaries plus `data/` assets; `src/desktop/system_integration.rs` embeds the same assets with `include_str!` for AppImage/standalone installs, and `tests/packaging_tests.rs` asserts the embedded copies match disk.

---

*Architecture analysis: 2026-11-02*
