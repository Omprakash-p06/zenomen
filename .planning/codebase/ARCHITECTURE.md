---
last_mapped_commit: 30d62de2cdf6f507822944e5602bdd907539ce38
last_mapped_at: 2026-11-06
---
<!-- refreshed: 2026-11-06 -->

# Architecture

**Analysis Date:** 2026-11-06

## System Overview

ZenOmen is a two-process system built from one Cargo package (`zenomen`) that produces two binaries:

1. `zenomen` — the unprivileged Slint desktop GUI (`src/main.rs`).
2. `zenomen-hw` — the privileged, headless system D-Bus daemon (`src/bin/zenomen-hw.rs`).

The GUI reads telemetry **in-process** from procfs/sysfs and sends every hardware **mutation** over the system bus to the daemon, which authorizes the caller with Polkit and performs verified sysfs writes.

```text
┌──────────────────────────────────────────────────────────────────────────┐
│ Presentation — Slint, UI thread, unprivileged                             │
│  `ui/app.slint` router  ·  `ui/screens/*`  ·  `ui/components/*`  ·  theme │
└───────────────────────────────┬──────────────────────────────────────────┘
                                │ properties in (formatted strings)
                                │ callbacks out (user intents only)
┌───────────────────────────────▼──────────────────────────────────────────┐
│ Bridge — `src/app.rs` (`ZenOmenApp`)                                      │
│  callback → `AppCommand` → `SyncSender` (depth 8, drop on full)           │
│  `AppEvent` → `AppState::apply` → `invoke_from_event_loop` → `apply_to_ui`│
└──────────────┬───────────────────────────────────────────┬───────────────┘
               │ command channel                           │ event channel
┌──────────────▼──────────────────┐          ┌─────────────▼───────────────┐
│ Core — worker thread            │          │ Event pump thread           │
│ `core/worker.rs` 1 Hz deadline  │          │ `core/worker.rs` events →   │
│ `core/state.rs` single writer   │          │ `ZenOmenApp::handle_event`  │
│ `core/telemetry.rs` 120-point   │          └─────────────────────────────┘
└──────────────┬──────────────────┘
               │ `HardwareProvider` trait (`Send + Sync`)
┌──────────────▼──────────────────┐          ┌─────────────────────────────┐
│ Hardware adapters (GUI process) │          │ Privileged daemon           │
│ `hardware/linux/provider.rs`    │          │ `src/bin/zenomen-hw.rs`     │
│ `hardware/registry.rs` canonical│          │ `service/contract.rs`       │
│ `hardware/linux/{cpu,gpu,...}`  │          │ `service/polkit.rs`         │
│ `hardware/mock.rs` scenarios    │          │ `service/backend.rs`        │
└─────────────────────────────────┘          │  `write_and_verify` → sysfs │
                                             └──────────────▲──────────────┘
                                                            │ system D-Bus
                                             `service/client.rs` (GUI side)
                                             `com.zenomen.Hardware1`
```

Desktop shell services also live in the GUI process on the **session** bus: single-instance ownership of `com.zenomen.App` and a StatusNotifierItem tray (`src/desktop/`).

## Component Responsibilities

| Component | Responsibility | File |
|-----------|----------------|------|
| GUI entry point | CLI flags, config load, single instance, tray, provider selection, thread wiring, teardown | `src/main.rs` |
| Library root | Module tree plus `slint::include_modules!()` (generated `AppWindow`) | `src/lib.rs` |
| Slint bridge | Queue UI callbacks as commands; fold events into state; publish formatted strings to Slint properties | `src/app.rs` |
| Typed contracts | `ScreenId`, `AppCommand`, `AppEvent` | `src/core/events.rs` |
| Core state | Single source of truth; exhaustive `apply(&AppEvent)`; status/degradation logic | `src/core/state.rs` |
| Monitoring worker | 1 Hz deadline loop, visibility gating, command dispatch, off-thread mutations | `src/core/worker.rs` |
| Telemetry history | Bounded `VecDeque` (120), SVG chart/sparkline path generation | `src/core/telemetry.rs` |
| Diagnostics | Report assembly, DMI/OS/kernel collection, JSON export | `src/core/diagnostics.rs` |
| Benchmark instrumentation | RSS/PSS reading and stage timing (`ZENOMEN_BENCHMARK`) | `src/core/benchmark.rs` |
| Configuration | `AppConfig` schema, clamping, atomic TOML save, corruption backup | `src/config/{mod,schema}.rs` |
| Provider contract | `HardwareProvider`, `HardwareCapabilities`, `TelemetrySnapshot` | `src/hardware/provider.rs` |
| Sensor registry | One canonical scan for CPU temp, fan tachometers, fan control, ACPI profiles | `src/hardware/registry.rs` |
| Linux adapters | procfs CPU/RAM, DRM/NVIDIA GPU, hwmon scan, provider assembly | `src/hardware/linux/` |
| Mock provider | Deterministic nominal/zero/gap scenarios | `src/hardware/mock.rs` |
| D-Bus contract | `com.zenomen.Hardware1` methods and Polkit gate | `src/service/contract.rs` |
| D-Bus client | Blocking proxy used by the worker; status/polkit probes | `src/service/client.rs` |
| Daemon backend | Verified sysfs writes, retries, presets, RGB variants | `src/service/backend.rs` |
| Polkit verifier | `CheckAuthorization` against `com.zenomen.hardware.control` | `src/service/polkit.rs` |
| Daemon entry | Connects to system (or session in tests) bus, serves the interface | `src/bin/zenomen-hw.rs` |
| Desktop integration | Single instance, SNI tray, XDG autostart, releases link, installer | `src/desktop/` |
| RGB model/migration | Color validation, profiles, OpenOMEN import | `src/rgb/` |
| UI shell | Window, sidebar/header/status strip, screen router | `ui/app.slint` |
| UI screens | Overview, Cooling, Performance, Lighting, Settings, Diagnostics | `ui/screens/*.slint` |
| UI components | Reusable cards, charts, pills, buttons, notices | `ui/components/*.slint` |
| Theme tokens | Colors, typography, metrics, bundled fonts | `ui/theme/` |

## Pattern Overview

**Overall:** Ports-and-adapters hardware boundary plus a unidirectional, message-driven UI bridge, split across a privilege boundary.

**Key Characteristics:**
- One trait (`HardwareProvider`) is the only seam between core logic and hardware (`src/hardware/provider.rs:232`).
- Exactly two message types cross the core/UI boundary: `AppCommand` (UI → core) and `AppEvent` (core → UI) (`src/core/events.rs:71`, `src/core/events.rs:115`).
- `AppState::apply` is the only mutator of view state (`src/core/state.rs:109`).
- Hardware **reads** happen in the unprivileged process; hardware **writes** happen only in the daemon.
- Absence is modeled as `Option`/empty collections/empty strings, never as `0` (`src/hardware/provider.rs:12-14`, `src/app.rs:444-458`).
- All UI mutation happens on the Slint event loop via `slint::invoke_from_event_loop` (`src/app.rs:342`).
- Memory is bounded by construction: 120-sample ring buffer plus regenerated path strings (`src/core/telemetry.rs:27`, `src/core/telemetry.rs:141-158`).

## Layers

**Presentation (Slint):**
- Purpose: Render properties and emit user intents; no hardware access and no formatting of raw values.
- Location: `ui/app.slint`, `ui/screens/`, `ui/components/`, `ui/theme/`.
- Contains: window shell, six screens, reusable components, design tokens, bundled Inter/JetBrains Mono fonts.
- Depends on: nothing in Rust except the generated property/callback surface.
- Used by: the bridge only.

**Bridge (`src/app.rs`):**
- Purpose: Translate callbacks into queued commands; translate events into state plus Slint property writes.
- Location: `src/app.rs:36-354` (type), `src/app.rs:94-273` (callbacks), `src/app.rs:386-519` (rendering).
- Contains: `ZenOmenApp`, `send_command`, `lock_state`, `apply_to_ui`, formatting helpers.
- Depends on: `core` types, `hardware` data types, one `Arc<SensorRegistry>` for diagnostics labels.
- Used by: `src/main.rs` and the event pump thread.

**Core (state and orchestration):**
- Purpose: Own app state, sampling cadence, bounded history, diagnostics, and the command/event vocabulary.
- Location: `src/core/`.
- Contains: `AppState`, `AppEvent`, `AppCommand`, `spawn_monitoring_worker`, `TelemetryHistory`, `DiagnosticsReport`, benchmark helpers.
- Depends on: `hardware` (data types and the provider trait), `config`, `service` (client, result types), `rgb`.
- Used by: the bridge and `src/main.rs`.

**Hardware adapters (unprivileged reads):**
- Purpose: Convert kernel interfaces into `TelemetrySnapshot` values.
- Location: `src/hardware/`.
- Contains: `LinuxHardwareProvider`, `SensorRegistry`, hwmon/CPU/GPU/memory parsers, mock provider, RGB interface detection.
- Depends on: std only (plus `serde` for data types).
- Used by: the monitoring worker and diagnostics.

**Service (privileged write path):**
- Purpose: Expose a narrow, Polkit-gated D-Bus API and execute verified sysfs writes.
- Location: `src/service/`.
- Contains: `HardwareService` (`contract.rs`), `HardwareBackend` (`backend.rs`), `PolkitVerifier` (`polkit.rs`), blocking `HardwareServiceClient` (`client.rs`), `FanMode`/`PresetResult`/`HardwareControlError` (`types.rs`).
- Depends on: `zbus`, std fs, `rgb`/`hardware::rgb` types.
- Used by: the daemon binary (server side) and the worker (client side).

**Desktop shell (session bus + XDG):**
- Purpose: Single instance, tray, autostart, releases link, system installer.
- Location: `src/desktop/`.
- Contains: `ensure_single_instance`, `register_tray`, `set_autostart_enabled`, `open_releases_page`, `install`/`uninstall`.
- Depends on: `zbus` (session bus), std fs/process, `directories`.
- Used by: `src/main.rs` and the worker (autostart toggles).

**Persistence:**
- Purpose: Durable user configuration and diagnostics output.
- Location: `src/config/mod.rs:20-116`, `src/core/diagnostics.rs:145`, `src/rgb/migration.rs`.
- Contains: `~/.config/zenomen/config.toml`, backups `config.toml.bak.<epoch>`, diagnostics JSON under `<config_dir>/diagnostics/`, OpenOMEN RGB migration into the ZenOmen config dir.
- Depends on: `directories`, `toml`, `serde_json`, atomic write helpers.

## Data Flow

### Primary Flow — telemetry sample to rendered pixel

1. The worker thread wakes on its deadline or an immediate refresh (`src/core/worker.rs:98-118`).
2. `sample_and_dispatch` calls `provider.sample_telemetry()` (`src/core/worker.rs:331-346`).
3. `LinuxHardwareProvider` reads `/proc/stat`, cpufreq or `/proc/cpuinfo`, the canonical registry temperature/fans, `/proc/meminfo`, ACPI profiles, and GPU data (`src/hardware/linux/provider.rs:115-201`).
4. `SensorRegistry` supplies CPU temperature, fan RPMs, fan mode and power profiles from the paths discovered once at startup (`src/hardware/registry.rs:69-114`).
5. A `AppEvent::TelemetryUpdated(snapshot)` is sent on the event channel (`src/core/worker.rs:336-338`).
6. The event pump thread calls `app.handle_event(event)` for every event (`src/main.rs:220-229`).
7. `handle_event` folds the event into `AppState` under a mutex (`src/app.rs:335-338`, `src/core/state.rs:109-142`), appending a `TelemetryPoint` and regenerating SVG paths (`src/core/state.rs:116-141`).
8. `slint::invoke_from_event_loop` schedules `apply_to_ui` on the UI thread (`src/app.rs:340-352`).
9. `apply_to_ui` formats strings/colors and writes `AppWindow` properties (`src/app.rs:386-519`); Slint re-renders.
10. `ui/app.slint` routes on `current-screen` and passes properties to the active screen (`ui/app.slint:260-425`).

### Secondary Flow — mutating hardware control

1. A screen callback reaches the bridge (e.g. `set-fan-mode`), which queues `AppCommand::SetFanMode` (`src/app.rs:129-133`).
2. The worker receives the command (`src/core/worker.rs:222-232`).
3. `execute_mutating_action` spawns a named `zenomen-ctrl-*` thread and an inner `zenomen-rpc-*` thread, waiting at most 30 s (`src/core/worker.rs:350-380`).
4. The inner thread calls `HardwareServiceClient::set_fan_mode` (`src/service/client.rs:123-141`), a **blocking** zbus call to `com.zenomen.Hardware1` (`src/service/client.rs:59-68`).
5. The daemon's `HardwareService::set_fan_mode` calls `authorize(&hdr)`, which asks Polkit for `com.zenomen.hardware.control` with user interaction allowed (`src/service/contract.rs:56-80`, `src/service/contract.rs:117-128`).
6. `HardwareBackend::set_fan_mode` maps the mode to its sysfs value and calls `write_and_verify` with 3 attempts and exponential backoff (`src/service/backend.rs:12-48`, `src/service/backend.rs:179-193`).
7. The confirmed value or an error string becomes `AppEvent::FanModeChanged` (`src/core/worker.rs:226-231`).
8. `AppState::apply` clears `pending_fan_mode` and records either the confirmed mode or `last_control_error` (`src/core/state.rs:162-182`); `apply_to_ui` publishes the banner and mode (`src/app.rs:421-433`).

### Tertiary Flow — visibility-gated sampling

1. Window close with close-to-tray enabled sends `AppCommand::SetUiVisible(false)` and hides the window (`src/main.rs:122-133`).
2. Start-minimized does the same before running the loop (`src/main.rs:235-243`).
3. The worker switches from `recv_timeout(deadline)` to blocking `recv()`; **no periodic sampling occurs while hidden** (`src/core/worker.rs:99-118`).
4. A tray/desktop `PresentWindow` action shows the window and sends `SetUiVisible(true)` (`src/main.rs:145-155`).
5. The worker samples immediately and resets its deadline (`src/core/worker.rs:122-134`).

### Quaternary Flow — configuration

1. `main` loads TOML via `load_or_default`; a parse failure backs up the file and returns a notice (`src/main.rs:62-64`, `src/config/mod.rs:34-78`).
2. `ConfigLoaded`/`ConfigCorrupted` events populate `AppState` and the Settings screen (`src/main.rs:213-216`, `src/app.rs:397-406`).
3. Settings callbacks queue `AppCommand::Set*` / `UpdateConfig`; the worker mutates a local `AppConfig`, calls `sanitize()`, and `save_atomic()` (temp file + fsync + rename + directory fsync) (`src/core/worker.rs:151-221`, `src/config/mod.rs:85-116`).
4. The worker echoes `ConfigLoaded` so the core and UI converge on the sanitized values (`src/core/worker.rs:166`, `183`, `195`).

### Diagnostics export Flow

1. `export-diagnostics` callback locks a snapshot of `AppState` and clones the registry, then spawns `zenomen-diag-export` (`src/app.rs:220-262`).
2. `collect_diagnostics` reads OS/kernel/DMI/DRM and formats registry bindings (`src/core/diagnostics.rs:59-100`).
3. The report is written atomically as JSON (`src/core/diagnostics.rs:145`) and the path is published back with `invoke_from_event_loop` (`src/app.rs:256-260`).

### Daemon Startup Flow

1. `zenomen-hw` initializes `tracing` (`src/bin/zenomen-hw.rs:12-16`).
2. `zbus::block_on` creates `HardwareBackend::default()` rooted at `/` (`src/bin/zenomen-hw.rs:23-24`, `src/service/backend.rs:56-60`).
3. It connects to the system bus, or to the session bus when `ZENOMEN_HW_SESSION` is set for tests (`src/bin/zenomen-hw.rs:26-30`).
4. It owns `com.zenomen.Hardware1` and serves at `/com/zenomen/Hardware1`, then stores the connection for Polkit queries (`src/bin/zenomen-hw.rs:31-40`).
5. Production activation is via systemd (`data/systemd/zenomen-hw.service`, `Type=dbus`) or D-Bus activation (`data/dbus/com.zenomen.Hardware1.service`).

**State Management:**
- `AppState` lives behind `Arc<Mutex<AppState>>`; poisoned locks are recovered with `into_inner()` (`src/app.rs:376-380`).
- Mutations occur on the pump thread before the UI closure is queued, so the core is always first to know (`src/app.rs:322-353`).
- The UI is stateless except for Slint properties; it never owns a hardware value.
- The command channel is `sync_channel(COMMAND_QUEUE_DEPTH = 8)` and uses `try_send`; a full queue drops the request and logs rather than blocking the UI (`src/app.rs:30`, `src/app.rs:356-369`).
- The window is held as `slint::Weak<AppWindow>` so background threads cannot keep it alive (`src/app.rs:13-14`, `src/app.rs:38`).

## Key Abstractions

**`HardwareProvider` (port):**
- Purpose: Make every backend interchangeable and keep hardware out of `core` and `app`.
- Examples/implementations: `src/hardware/linux/provider.rs:110`, `src/hardware/mock.rs:142`.
- Pattern: `Send + Sync` trait with `capabilities()` and `sample_telemetry() -> Result<TelemetrySnapshot, String>` (`src/hardware/provider.rs:232-239`). Selected in `src/main.rs:183-195`.

**`HardwareCapabilities` (per-subsystem truth):**
- Purpose: Let each screen name the exact missing interface instead of one coarse unsupported state.
- Location: `src/hardware/provider.rs:28-145`.
- Pattern: eight booleans + optional RGB variant; `Default` is `none()`, so an unfilled value degrades the UI rather than inventing hardware.

**`TelemetrySnapshot` (plain data):**
- Purpose: One immutable sample crossing thread and process boundaries; `Option` marks absence.
- Location: `src/hardware/provider.rs:151-220`.

**`SensorRegistry` (canonical sensor bindings):**
- Purpose: Resolve CPU temperature by driver/label priority and enumerate tachometers/PWM/ACPI once, so dashboard, fan reads, and diagnostics all use the same paths.
- Location: `src/hardware/registry.rs:17-136`; resolution rules at `:139-222` (k10temp → coretemp → acpitz) and `:225-289` (prefer hp/hp-wmi/platform, then any hwmon).
- Pattern: scan once at provider construction (`src/hardware/linux/provider.rs:73`), then cheap reads per tick.

**`AppCommand` / `AppEvent` (message vocabulary):**
- Purpose: The only two directions across the UI boundary.
- Location: `src/core/events.rs:71-104`, `src/core/events.rs:115-147`.
- Pattern: Commands are intents; events are facts. `AppState::apply` and the worker's `match` are exhaustive, so adding a variant forces the author to handle it.

**`AppState::apply` (single writer):**
- Purpose: Guarantee the UI cannot show something the core does not know.
- Location: `src/core/state.rs:107-260`.
- Pattern: one exhaustive `match`; derived questions (`status_text`, `is_degraded`) live beside the data (`src/core/state.rs:262-304`).

**`HardwareService` / `HardwareServiceClient` (privilege boundary):**
- Purpose: Narrow API for all mutations: `GetCapabilities`, `GetFanStatus`, `SetFanMode`, `GetPowerProfiles`, `GetPowerProfile`, `SetPowerProfile`, `ApplyPreset`, `ApplyRgbProfile`, `SetRgbEnabled`.
- Location: server `src/service/contract.rs:83-213`; client `src/service/client.rs:34-343`.
- Pattern: every mutating method takes `#[zbus(header)]` and calls `authorize` before touching the backend.

**`HardwareBackend` (verified writer):**
- Purpose: Keep all sysfs writes in one place with read-back verification.
- Location: `src/service/backend.rs:52-501`.
- Pattern: `write_and_verify(path, target, retries, initial_delay)`; RGB has a byte-level variant for the kernel-patch interface (`src/service/backend.rs:464-499`).

**`RgbProfile` / `RgbInterfaceVariant`:**
- Purpose: One profile model over three incompatible kernel interfaces.
- Location: `src/rgb/schema.rs:114-150`, `src/hardware/rgb.rs:13-34`.
- Pattern: detect variant (`FourZone`, `Legacy`, `KernelPatch`) then branch in `apply_rgb_hardware` (`src/service/backend.rs:348-501`).

**`MockScenario`:**
- Purpose: Exercise nominal, zero-capability, and sensor-gap paths through the real binary.
- Location: `src/hardware/mock.rs:41-69`; selected by `ZENOMEN_MOCK_SCENARIO` in `src/main.rs:168-195`.

## Entry Points

**GUI binary (`zenomen`):**
- Location: `src/main.rs:19`.
- Triggers: desktop launcher, `zenomen`, autostart (`zenomen --minimized`), or a second launch that delegates to the primary instance.
- Responsibilities: locale fix, root-only installer flags, config, single instance/tray, logging/migration, UI construction, bridge installation, provider selection, initial capabilities, event pump, worker, event loop, orderly teardown (`src/main.rs:245-253`).

**Root-only installer flags:**
- Location: `src/main.rs:35-59`.
- Triggers: `--install-system-integration`, `--uninstall-system-integration`; handled before any GUI work.
- Responsibilities: delegate to `src/desktop/system_integration.rs:122-176`, print result, exit 0/1.

**Daemon binary (`zenomen-hw`):**
- Location: `src/bin/zenomen-hw.rs:11`.
- Triggers: systemd `Type=dbus` unit, D-Bus activation, or a developer running it directly.
- Responsibilities: serve `com.zenomen.Hardware1` until killed; `ZENOMEN_HW_SESSION` switches to the session bus for integration tests.

**Build script:**
- Location: `build.rs:8-20`.
- Triggers: `cargo build`.
- Responsibilities: compile `ui/app.slint` with style pinned to `fluent`; embed Windows icon under `cfg(windows)`.

**Session D-Bus names owned by the GUI:**
- `com.zenomen.App` at `/com/zenomen/App` (`src/desktop/single_instance.rs:10-12`).
- `org.kde.StatusNotifierItem` at `/StatusNotifierItem` and `com.canonical.dbusmenu` at `/MenuBar` (`src/desktop/tray.rs:14-15`, `:186-220`).

## Architectural Constraints

- **Threading:** The Slint UI thread never touches hardware or D-Bus. Dedicated threads are: `zenomen-monitor-worker` (`src/core/worker.rs:39`), `zenomen-event-pump` (`src/main.rs:222`), `zenomen-desktop-actions` (`src/main.rs:138`), per-request `zenomen-ctrl-*` + `zenomen-rpc-*` (`src/core/worker.rs:359-367`), and `zenomen-diag-export` (`src/app.rs:227`). There is no async runtime in the GUI; the daemon uses `zbus::block_on`.
- **UI-thread affinity:** Slint properties may only be written on the UI thread. `initialize()` is the one exception because it runs before the event loop starts (`src/app.rs:285-320`). All later writes go through `invoke_from_event_loop` (`src/app.rs:342`).
- **Single-writer state:** `AppState` changes only through `apply`. Do not add setters or mutate it from UI callbacks.
- **Privilege split:** The GUI process must never open a sysfs file for writing. Mutations go through `src/service/client.rs` → `HardwareService` → `HardwareBackend`. Reads are intentionally direct and unprivileged (telemetry must work with the daemon absent).
- **Read/write asymmetry:** Fan RPM, fan mode, power profile, and capabilities are read twice by two independent implementations — `SensorRegistry` in the GUI and `HardwareBackend` in the daemon (`src/hardware/registry.rs:225-289` vs `src/service/backend.rs:77-175`). They can disagree; the GUI's snapshot is what the dashboard shows, the daemon's read-back is what a write is verified against.
- **Capability source is GUI-local:** `HardwareCapabilities` comes from `SensorRegistry::detect_capabilities` (`src/hardware/registry.rs:117-135`). The daemon's `GetCapabilities` exists (`src/service/contract.rs:86-102`) but the client only uses the call as a liveness probe and discards the body (`src/service/client.rs:78-98`). `battery_metrics` is hardcoded `false` (`src/hardware/registry.rs:134`) even though the UI exposes a battery capability flag.
- **Non-blocking worker:** Polkit prompts can take arbitrarily long, so mutating calls must go through `execute_mutating_action` (30 s timeout) rather than being awaited on the sampling loop (`src/core/worker.rs:348-380`).
- **Back-pressure by dropping:** Command sends use `try_send`; a full queue is treated as acceptable loss (`src/app.rs:356-369`). Never replace with `send()` on the UI thread.
- **Bounded memory:** History is capped by `capacity`, with `pop_front` before `push_back` (`src/core/telemetry.rs:143-149`); SVG paths are regenerated per tick instead of retaining chart models. Release profile is size/RSS oriented (`Cargo.toml:37-44`).
- **Compile-time UI coupling:** `slint::include_modules!()` lives in `src/lib.rs:14`, so the generated `AppWindow` is part of the library and the daemon binary links it too (types only; no window is ever created). Any `.slint` compile error breaks both binaries.
- **Screen set is compile-time:** `ScreenId` is an enum with `ALL: [Self; 6]` (`src/core/events.rs:16-34`), and the Slint router validates `0..=5` (`ui/app.slint:191-192`) while the sidebar declares `row-count: 6` (`ui/components/sidebar.slint:52`). Adding a screen requires all three.
- **Environment switches:** `ZENOMEN_MOCK_SCENARIO` (`src/main.rs:168`), `ZENOMEN_HW_SESSION` (`src/bin/zenomen-hw.rs:26`), `ZENOMEN_BENCHMARK=1` (`src/core/benchmark.rs:245`), `RUST_LOG` (`src/main.rs:88`), and an automatic `LC_CTYPE=C.UTF-8` fallback (`src/main.rs:20-32`).
- **No network stack:** the only outbound action is `xdg-open` to the releases page (`src/desktop/releases.rs:22-44`); there is no HTTP client dependency.

## Anti-Patterns

### Writing hardware from the GUI process

**What happens:** A handler in `src/hardware/` or `src/app.rs` opens `/sys/...` for writing.
**Why it's wrong:** It bypasses Polkit, the systemd sandbox, and `write_and_verify` read-back; the GUI runs unprivileged and will fail or, worse, succeed partially.
**Do this instead:** Add a method to `HardwareBackend` using `write_and_verify`, expose it in `src/service/contract.rs` behind `authorize`, add the blocking client call in `src/service/client.rs`, and invoke it from the worker.

### Touching Slint from a background thread

**What happens:** A worker or spawned thread calls `ui.set_*` directly.
**Why it's wrong:** Slint is not thread-safe; it panics or corrupts the render state.
**Do this instead:** Send an `AppEvent` and let `ZenOmenApp::handle_event` queue `apply_to_ui` via `slint::invoke_from_event_loop` (`src/app.rs:340-352`).

### Blocking the sampling loop on authorization

**What happens:** A D-Bus mutation is awaited inline in the worker's `match`.
**Why it's wrong:** A Polkit password dialog would stop telemetry for as long as the dialog is open.
**Do this instead:** Follow `execute_mutating_action` (`src/core/worker.rs:350-380`).

### Fabricating a reading for an absent sensor

**What happens:** A parser returns `0.0`/`0` when the file is missing, or the bridge writes `"0"` into a property.
**Why it's wrong:** It violates the project's core value; the UI must say `Unavailable`.
**Do this instead:** Return `Option`/empty `Vec`/empty `String` (`src/hardware/provider.rs:149-150`) and let `apply_to_ui` skip the property (`src/app.rs:444-458`, `src/app.rs:531-539`).

### Reading sensors in the bridge or UI

**What happens:** `src/app.rs` or a `.slint` file learns about a sysfs path.
**Why it's wrong:** It puts blocking I/O on the UI thread and breaks the single sampling cadence.
**Do this instead:** Add the read to a `hardware/linux/*` module, sample it in `LinuxHardwareProvider`, carry it in `TelemetrySnapshot`, and format it in `apply_to_ui`.

### Duplicating hwmon discovery a third time

**What happens:** A new feature scans `/sys/class/hwmon` itself.
**Why it's wrong:** CPU-temperature priority, fan ordering, and fan-control selection already live in `SensorRegistry`; a copy drifts silently.
**Do this instead:** Extend `SensorRegistry::scan_roots` (`src/hardware/registry.rs:42-66`) and read through its accessors. If the daemon needs the same binding, pass the concept through the D-Bus contract rather than re-deriving it in `contract.rs`/`client.rs`.

### Adding an event that skips state

**What happens:** A new `AppEvent` is handled only in `apply_to_ui`, or a new `AppCommand` is handled only by a side effect.
**Why it's wrong:** `AppState` then disagrees with the window, and Diagnostics/serialization lose the fact.
**Do this instead:** Add the variant, handle it in `AppState::apply` (the compiler enforces exhaustiveness), then publish it in `apply_to_ui`.

### Adding a screen to Slint only

**What happens:** A new `.slint` screen is instantiated but no enum/index/bound is updated.
**Why it's wrong:** `current-screen` clamps to `0`, so the screen is unreachable.
**Do this instead:** Update `ScreenId::ALL`, `index`, `from_index`, `title`; the router bound in `ui/app.slint:191-192`; the sidebar row count/model; and the shell's `if` chain.

## Error Handling

**Strategy:** Degrade and explain; never panic on missing hardware.

**Patterns:**
- Provider failures return `Err(String)` and become `AppEvent::TelemetrySamplingFailed`, which marks the state degraded and is shown in the status strip (`src/core/worker.rs:339-344`, `src/core/state.rs:143-145`, `src/core/state.rs:266-282`).
- Mutations return `Result<_, String>`; the bridge records `last_control_error` and the screens render a dismissible banner (`src/core/state.rs:162-235`, `src/app.rs:433`).
- The daemon uses `HardwareControlError` (`thiserror`) for interface-missing, IO, invalid parameter, verification failure, and Polkit denial (`src/service/types.rs`, `src/service/backend.rs:22-47`), mapped into `zbus::fdo::Error`.
- Verified writes retry with exponential backoff and report target/actual/retries on failure (`src/service/backend.rs:12-48`).
- Config corruption is never fatal: the bad file is copied aside and defaults load (`src/config/mod.rs:47-77`).
- Thread teardown is explicit: `Shutdown` command, drop senders, join worker and pump (`src/main.rs:245-253`).

## Cross-Cutting Concerns

**Logging:** `tracing` with `tracing_subscriber::fmt` in both binaries; `RUST_LOG`/`EnvFilter` selects verbosity (`src/main.rs:89`, `src/bin/zenomen-hw.rs:12-16`). Named threads make logs traceable.

**Validation:** `AppConfig::sanitize` clamps polling to 500–5000 ms and whitelists history windows 30/60/120/300 (`src/config/schema.rs:90-106`); the daemon validates power-profile names against `platform_profile_choices` and rejects slashes/newlines/NUL (`src/service/backend.rs:234-258`); RGB colors are strict `#RRGGBB` (`src/rgb/schema.rs:47-77`).

**Authentication/Authorization:** Polkit action `com.zenomen.hardware.control` with `auth_admin` / `auth_admin_keep` (`data/polkit/com.zenomen.hardware.policy`); D-Bus policy restricts ownership of `com.zenomen.Hardware1` to root (`data/dbus/com.zenomen.Hardware1.conf`).

**Sandboxing:** `zenomen-hw.service` sets `ProtectSystem=strict`, `ProtectHome=true`, `NoNewPrivileges=true`, empty capability bounding set, `RestrictAddressFamilies=AF_UNIX`, and read-write carve-outs for hp-wmi, ACPI, hwmon, and LEDs.

**Packaging:** Cargo Packager metadata in `Cargo.toml:46-75` ships both binaries and the desktop/D-Bus/Polkit/systemd assets; CI gates formatting, clippy, tests, and `cargo audit` (`.github/workflows/ci.yml`); tag releases verify the tag equals `Cargo.toml` version (`.github/workflows/release.yml`).

---

*Architecture analysis: 2026-11-06*
