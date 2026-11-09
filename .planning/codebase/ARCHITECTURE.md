<!-- refreshed: 2026-11-09 -->
# Architecture

**Analysis Date:** 2026-11-09

## System Overview

ZenOmen is an open-source, lightweight hardware control center for HP OMEN and Victus laptops running Linux and Windows. It follows a **two-process, unidirectional-data-flow** architecture designed for strict privilege separation, minimal resource overhead (<=40 MB RSS), and fail-safe operation:

1. **Unprivileged Desktop GUI (`zenomen`):** Runs in the user's desktop session. Contains the declarative Slint UI, an event-driven state bridge, a single-writer application state reducer (`AppState`), and a background monitoring worker thread (`zenomen-monitor-worker`). It performs all hardware telemetry *reads* without elevated privileges.
2. **Privileged Hardware Daemon (`zenomen-hw`):** Runs as `root` (Linux) or `LocalSystem` (Windows SCM service). Owns every hardware *mutation* (fan mode switches, ACPI platform profiles, RGB lighting writes). Exposes an authenticated IPC interface (system D-Bus on Linux; local named pipe on Windows).
3. **Cross-Platform Abstraction:** Hardware telemetry reads are abstracted behind the `HardwareProvider` trait and `SensorRegistry`, while privileged control mutations are abstracted through `HardwareServiceClient` communicating via typed request/response contracts (`HardwareRequest`/`HardwareResponse` on Windows named pipes, and `com.zenomen.Hardware1` on Linux D-Bus).

```text
┌────────────────────────────────────────────────────────────────────────┐
│ GUI binary: zenomen  (src/main.rs, feature = "ui")                     │
│                                                                        │
│  ui/app.slint (declarative shell/router: 6 screens, theme tokens)       │
│        │ callbacks                            ▲ formatted properties   │
│        ▼                                      │                        │
│  src/app.rs  ZenOmenApp  (the ONLY Slint bridge: Weak<AppWindow>)      │
└────────┬──────────────────────────────────────────────────▲────────────┘
         │ AppCommand (SyncSender, depth 8, try_send)        │ AppEvent (mpsc)
         ▼                                                   │ via event pump
┌───────────────────────────────────────────────────────────┴────────────┐
│ src/core/  (pure logic & scheduling, no direct hardware I/O)           │
│  worker.rs      zenomen-monitor-worker (1 Hz deadline loop, idle gate) │
│  events.rs      AppCommand / AppEvent / ScreenId                       │
│  state.rs       AppState::apply(&AppEvent)  ← SINGLE WRITER            │
│  telemetry.rs   bounded 120-sample history + SVG chart path generation │
│  diagnostics.rs DiagnosticsReport snapshot & atomic JSON export        │
└────────┬───────────────────────────────────────────────┬───────────────┘
         │ Arc<dyn HardwareProvider>                     │ HardwareServiceClient
         ▼ (read-only telemetry)                         ▼ (privileged writes)
┌─────────────────────────────────┐        ┌─────────────────────────────┐
│ src/hardware/                   │        │ src/service/                │
│  provider.rs   HardwareProvider │        │  client.rs  ServiceClient   │
│  registry.rs   SensorRegistry   │        │  contract.rs D-Bus (Linux)  │
│  linux/        sysfs & procfs   │        │  backend.rs  sysfs verify   │
│  windows/      PDH, DXGI, WMI   │        │  windows/   Named pipe      │
│  mock.rs       MockHardware     │        │  windows/   HP WMI driver   │
│  unavailable.rs Zero caps       │        │  windows/   Board validation│
└─────────────────────────────────┘        └──────────────┬──────────────┘
                                                          │ IPC (D-Bus / Named Pipe)
                                           ┌──────────────▼──────────────┐
                                           │ zenomen-hw (root/LocalSystem)│
                                           │ src/bin/zenomen-hw.rs        │
                                           └─────────────────────────────┘
```

---

## Component Responsibilities

| Component | Responsibility | Key Files |
|-----------|----------------|-----------|
| **GUI Entry Point** | CLI argument parsing, logging initialization, single-instance enforcement, tray registration, provider selection, thread spawning, Slint event loop run, ordered teardown | `src/main.rs` |
| **Slint UI Bridge** | Connects Slint callbacks to `AppCommand` channel; folds `AppEvent` into `AppState`; updates Slint properties on UI thread via `slint::invoke_from_event_loop` | `src/app.rs` |
| **Declarative UI** | Window layout, navigation sidebar router, 6 screen definitions, reusable UI components, typography/color tokens; strictly zero hardware access or math logic | `ui/app.slint`, `ui/screens/*.slint`, `ui/components/*.slint`, `ui/theme/*.slint` |
| **Core Message Contract** | Type-safe enum definitions for user actions (`AppCommand`), internal domain results (`AppEvent`), and screen identifiers (`ScreenId`) | `src/core/events.rs` |
| **Single-Writer State** | Holds entire UI state in `AppState`; all updates go through pure reducer `AppState::apply(&AppEvent)`; computes status indicators and degradation flags | `src/core/state.rs` |
| **Monitoring Worker** | Manages 1 Hz telemetry sampling loop using `recv_timeout`; sleeps during window minimization; executes mutation requests on short-lived worker threads | `src/core/worker.rs` |
| **Telemetry History** | Bounded `VecDeque` (max 120 samples) for CPU/GPU utilization and temperatures; builds gap-aware SVG path strings for UI charts | `src/core/telemetry.rs` |
| **Hardware Abstraction** | Declares `HardwareProvider` trait, `HardwareCapabilities`, and `TelemetrySnapshot` types | `src/hardware/provider.rs` |
| **Linux Telemetry** | Reads procfs (`/proc/stat`, `/proc/meminfo`), sysfs hwmon, and DRM subsystem for AMD/Intel/NVIDIA telemetry; binds to `SensorRegistry` | `src/hardware/linux/*.rs` |
| **Windows Telemetry** | Samples CPU metrics via PDH, GPU metrics via DXGI, memory via `GlobalMemoryStatusEx`, battery via `GetSystemPowerStatus`, and thermal data via WMI | `src/hardware/windows/*.rs` |
| **Sensor Registry** | Discovers and canonicalizes CPU temperature, fan tachometers, and ACPI paths using strict driver priority (AMD `k10temp` vs Intel `coretemp`) | `src/hardware/registry.rs` |
| **Mock Provider** | Deterministic simulation of hardware states (`Nominal`, `ZeroCapabilities`, `SensorDataGap`) for testing without hardware dependencies | `src/hardware/mock.rs` |
| **Unavailable Provider** | Failsafe provider reporting zero capabilities for unsupported operating systems or virtual environments | `src/hardware/unavailable.rs` |
| **Service Client Facade** | Platform-branching facade exposing unified methods for fan control, power profiles, RGB lighting, and daemon connection checks | `src/service/client.rs` |
| **Linux Service Daemon** | Implements `com.zenomen.Hardware1` D-Bus interface, validates Polkit credentials, writes and verifies sysfs paths | `src/service/contract.rs`, `src/service/backend.rs`, `src/service/polkit.rs` |
| **Windows Service Daemon** | Runs SCM service dispatcher or console mode, accepts named-pipe connections, verifies client console tokens, enforces board profile authorization, executes HP BIOS WMI calls | `src/service/windows/*.rs` |
| **Privileged Binary** | Entry point for `zenomen-hw`; initializes logging and runs D-Bus event loop (Linux) or SCM dispatcher/console server (Windows) | `src/bin/zenomen-hw.rs` |
| **Configuration** | Persistent TOML configuration schema, range clamping/sanitization, atomic writes via temporary files, automatic corruption recovery | `src/config/mod.rs`, `src/config/schema.rs` |
| **Desktop Integration** | Single-instance mutual exclusion (session D-Bus on Linux, Win32 Named Mutex on Windows), system tray icon, autostart entries, service installer/uninstaller | `src/desktop/*`, `src/desktop/windows/*` |
| **Diagnostics System** | Gathers snapshot of `AppState` and `SensorRegistry`, exports redacted diagnostic reports as atomic JSON files | `src/core/diagnostics.rs` |
| **RGB Domain** | RGB lighting profile representation, 4-zone color definitions, builtin presets, and migration parser for legacy OpenOMEN profiles | `src/rgb/*.rs` |

---

## Process Model & Security Boundaries

ZenOmen operates strictly across two processes with divergent privilege levels and security boundaries:

```text
┌────────────────────────────────────────────────────────────────────────┐
│ PROCESS 1: zenomen (Unprivileged Desktop GUI)                          │
│ User Desktop Session (Desktop User / Medium Integrity)                 │
│                                                                        │
│ - Slint UI & presentation logic                                        │
│ - State reduction & history buffers                                    │
│ - Read-only hardware polling (sysfs/procfs on Linux; PDH/DXGI on Win) │
│ - Never runs as root/Administrator                                     │
│ - Drops privileges by design                                           │
└───────────────────────────────────┬────────────────────────────────────┘
                                    │ IPC BOUNDARY
    Linux: System D-Bus             │ Windows: Local Named Pipe
    Destination: com.zenomen.Hardware1│ Pipe: \\.\pipe\ZenOmen.Hardware.v1
    Polkit: com.zenomen.hardware.control│ DACL: SYSTEM(GA) + Admin(GA) + User(GRGW)
                                    │ Console Session ID token verification
                                    ▼
┌────────────────────────────────────────────────────────────────────────┐
│ PROCESS 2: zenomen-hw (Privileged Hardware Service)                    │
│ System Context (root / LocalSystem / High Integrity)                   │
│                                                                        │
│ - No GUI libraries linked (--no-default-features)                      │
│ - Authorized hardware mutations only (Auto/Max fan, ACPI profiles, RGB)│
│ - Verified writes: read-back verification after every hardware mutation│
│ - Board profile authorization: rejects unvalidated motherboard models  │
│ - Failsafe teardown: restores fan to Auto on graceful shutdown         │
└────────────────────────────────────────────────────────────────────────┘
```

### 1. Process 1: Unprivileged Desktop GUI (`zenomen`)
- Built from `src/main.rs` with `default-features = ["ui"]`.
- Runs under the active desktop user's standard credentials.
- Performs read-only telemetry sampling. Reading sysfs files like `/sys/class/hwmon` or `/proc/stat` on Linux, and calling PDH / DXGI / `GlobalMemoryStatusEx` on Windows requires no administrative rights.
- Does not contain any code capable of raw hardware writes or elevated driver calls.

### 2. Process 2: Privileged Hardware Service (`zenomen-hw`)
- Built from `src/bin/zenomen-hw.rs` with `--no-default-features`. Does not link Slint, font engines, or graphics drivers.
- **Linux:** Managed by `systemd` as a system service. Activates on D-Bus request for `com.zenomen.Hardware1`. Each mutating method (`SetFanMode`, `SetPowerProfile`, `ApplyPreset`, `ApplyRgbProfile`, `SetRgbEnabled`) queries Polkit for action `com.zenomen.hardware.control` using the caller's D-Bus credentials (`src/service/polkit.rs`).
- **Windows:** Managed by Windows Service Control Manager (SCM) under the service name `ZenOmenHardwareService` (running as `NT AUTHORITY\LocalSystem`). Can also run via `--console` for testing.
  - **Named Pipe Security:** The pipe `\\.\pipe\ZenOmen.Hardware.v1` is created with a strict SDDL DACL (`D:(A;;GA;;;SY)(A;;GA;;;BA)(A;;GRGW;;;IU)`), allowing read/write only to Interactive Users, Administrators, and SYSTEM (`src/service/windows/security.rs`).
  - **Session Verification:** On every client connection, `verify_client_security` impersonates the client, retrieves their process token, and asserts that the client's token session ID matches `WTSGetActiveConsoleSessionId`. Network and secondary logon sessions are immediately rejected (`src/service/windows/security.rs`).
  - **Board Authorization:** All hardware mutations are validated against `BoardProfile` and `BoardValidation` catalogs (`src/service/windows/profiles.rs`, `src/service/windows/authorization.rs`). If a board ID (e.g., HP Victus 15 `8A3D`) has not been physically validated for a specific mutation, the service rejects the request with `HardwareControlError::BoardNotSupported`.
  - **Verified Mutations:** Hardware mutations are applied via HP BIOS WMI (`hpqBIntM`, GUID `5FB7F034-2C63-45e9-BE91-3D44E2C707E4`) and verified with read-back queries before returning success (`src/service/windows/hp_wmi.rs`).

---

## Threading Model & Background Workers

The GUI binary (`zenomen`) uses standard synchronous Rust threads (`std::thread`) and channels (`std::sync::mpsc`). It deliberately does **not** pull in an asynchronous runtime (Tokio/async-std) to strictly preserve its <=40 MB RSS budget.

```text
┌────────────────────────────────────────────────────────────────────────┐
│ THREAD 1: Main Thread (Slint UI Event Loop)                            │
│  - Executes `slint::run_event_loop_until_quit()`                       │
│  - Receives UI input events, paints declarative UI at display refresh  │
│  - Receives UI updates via `slint::invoke_from_event_loop()`           │
└───────────────────▲───────────────────────────────▲────────────────────┘
                    │ invoke_from_event_loop        │ invoke_from_event_loop
                    │                               │
┌───────────────────┴─────────────┐   ┌─────────────┴────────────────────┐
│ THREAD 2: zenomen-event-pump    │   │ THREAD 3: zenomen-desktop-actions│
│  - Loops over mpsc Receiver     │   │  - Loops over action channel     │
│  - Calls ZenOmenApp::handle_event│  │  - Receives PresentWindow, Quit, │
│  - Folds events into AppState   │   │    Tray fan actions              │
└───────────────────▲─────────────┘   └──────────────────────────────────┘
                    │
                    │ mpsc::Sender<AppEvent>
                    │
┌───────────────────┴────────────────────────────────────────────────────┐
│ THREAD 4: zenomen-monitor-worker (Long-lived Background Worker)        │
│  - 1 Hz deadline loop: recv_timeout(deadline - now) on AppCommand     │
│  - Wakes immediately on AppCommand::RefreshTelemetry                   │
│  - Pauses polling when AppCommand::SetUiVisible(false)                 │
│  - Calls provider.sample_telemetry() without blocking the UI          │
│  - Checks HardwareServiceClient connection status periodically         │
└───────────────────┬────────────────────────────────────────────────────┘
                    │ spawns on mutating AppCommand
                    ▼
┌────────────────────────────────────────────────────────────────────────┐
│ EPHEMERAL THREADS 5 & 6: zenomen-ctrl-* & zenomen-rpc-*                │
│  - Spawned per mutating command (SetFanMode, SetPowerProfile, etc.)    │
│  - Guarded by AtomicBool `mutation_in_flight` (rejects concurrent writes)
│  - zenomen-ctrl-* sets up 30-second timeout channel                    │
│  - zenomen-rpc-* performs blocking IPC client call                     │
│  - Automatically cleans up and releases in-flight guard via RAII       │
└────────────────────────────────────────────────────────────────────────┘
```

### Detailed Thread Descriptions

1. **Slint UI Event Loop (`main` thread):**
   - Owns the window handle `AppWindow`.
   - Handles mouse, keyboard, and render events.
   - Installs Slint callbacks (`install_callbacks` in `src/app.rs`) which wrap UI interactions into `AppCommand` values and send them through a bounded `SyncSender` (capacity: 8).
   - Never performs I/O, disk writes, IPC calls, or sleeps.

2. **Monitoring Worker (`zenomen-monitor-worker`):**
   - Spawned in `src/core/worker.rs:spawn_monitoring_worker`.
   - Owns the 1 Hz cadence using `command_receiver.recv_timeout(deadline - now)`.
   - When a user clicks "Refresh", `AppCommand::RefreshTelemetry` arrives on the receiver, instantly breaking the timeout wait, sampling hardware, sending `AppEvent::TelemetryUpdated`, and resetting the deadline.
   - **Idle Optimization (SYS-04):** When the window is closed to tray, `AppCommand::SetUiVisible(false)` switches the worker loop to a blocking `recv()`, completely eliminating background hardware polling and reducing CPU wakeups to zero until the window is restored (`AppCommand::SetUiVisible(true)`).

3. **Event Pump (`zenomen-event-pump`):**
   - Spawned in `src/main.rs:327`.
   - Continuously receives `AppEvent` items from an unbounded `mpsc::channel`.
   - Calls `ZenOmenApp::handle_event(event)`, which locks `Arc<Mutex<AppState>>`, invokes the single-writer reducer `state.apply(&event)`, formats the state into Slint types, and schedules UI updates on the main thread via `slint::invoke_from_event_loop`.

4. **Desktop Actions Listener (`zenomen-desktop-actions`):**
   - Spawned in `src/main.rs:202`.
   - Listens on `action_receiver` for desktop events emitted by session D-Bus (Linux) or Win32 tray messages (Windows).
   - Handles `DesktopAction::PresentWindow` (restores minimized window), `DesktopAction::SetFanModeAuto`, `DesktopAction::SetFanModeMax`, and `DesktopAction::Quit`.

5. **Mutation Controller & RPC Threads (`zenomen-ctrl-*` & `zenomen-rpc-*`):**
   - Spawned in `src/core/worker.rs:execute_mutating_action`.
   - Guarded by `mutation_in_flight: Arc<AtomicBool>`. If a user spams buttons while a mutation is running, subsequent requests are dropped immediately with an error event.
   - Spawns a controller thread that spawns an inner RPC thread to call `HardwareServiceClient`. Uses a 30-second channel timeout to ensure that an unresponsive IPC daemon never locks up the monitoring loop.
   - Holds an `InFlightGuard` RAII struct that clears the atomic flag on drop, even on timeout or panic.

6. **Diagnostics Exporter Thread (`zenomen-diag-export`):**
   - Spawned in `src/app.rs:227` upon user request.
   - Snapshots the current `AppState` and `SensorRegistry`, serializes to JSON, and writes atomically using a temporary file (`atomic_write`) so file system I/O never delays UI rendering.

---

## Data Flow Walkthroughs

### 1. Telemetry Sampling Flow (Read-Only)

```text
[zenomen-monitor-worker]
        │ 1. 1 Hz deadline expires (or AppCommand::RefreshTelemetry received)
        ▼
[HardwareProvider::sample_telemetry()]
        │ 2. Linux: reads /sys/class/hwmon, /proc/stat, /proc/meminfo
        │    Windows: queries PDH CPU query, DXGI GPU adapter, GlobalMemoryStatusEx
        ▼
[Result<TelemetrySnapshot, String>]
        │ 3. Packaged into AppEvent::TelemetryUpdated or AppEvent::TelemetrySamplingFailed
        ▼
[mpsc::Sender<AppEvent>] ──► [zenomen-event-pump]
                                     │ 4. Calls ZenOmenApp::handle_event(event)
                                     ▼
                             [AppState::apply(&event)]
                                     │ 5. Single writer updates AppState:
                                     │    - Appends to TelemetryHistory (max 120 samples)
                                     │    - Generates SVG path strings for charts
                                     │    - Recalculates status & degradation indicators
                                     ▼
                             [slint::invoke_from_event_loop]
                                     │ 6. Dispatches to Slint UI thread
                                     ▼
                             [apply_to_ui(ui, &state)]
                                     │ 7. Assigns formatted properties to Slint window
                                     ▼
                                [Slint UI Frame Rendered]
```

### 2. User Command Flow (UI → Core)

```text
[User clicks Fan Mode "Max"]
        │
        ▼
[ui/screens/cooling.slint]
        │ emits callback: root.set-fan-mode("max")
        ▼
[src/app.rs: install_callbacks]
        │ invokes on_set_fan_mode closure
        ▼
[send_command(&commands, AppCommand::SetFanMode("max"))]
        │ calls SyncSender::try_send
        │ (Queue depth: 8. Saturated queue drops command with debug log, preventing UI lockup)
        ▼
[mpsc::Receiver<AppCommand>] in [zenomen-monitor-worker]
        │ wakes recv_timeout immediately
        ▼
[src/core/worker.rs: dispatch command]
        │ executes command branch
```

### 3. Privileged Mutation Flow (Write & Verification)

```text
[zenomen-monitor-worker]
        │ 1. Receives AppCommand::SetFanMode("max")
        ▼
[src/core/worker.rs: execute_mutating_action]
        │ 2. Checks mutation_in_flight AtomicBool; acquires InFlightGuard
        ▼
[Spawns zenomen-ctrl-fan-mode & zenomen-rpc-fan-mode]
        │ 3. Executes client RPC call with 30-second timeout channel
        ▼
[HardwareServiceClient::set_fan_mode("max")]
        │
        ├── Linux: D-Bus call to com.zenomen.Hardware1.SetFanMode("max")
        │          - zenomen-hw verifies Polkit action com.zenomen.hardware.control
        │          - backend.rs executes write_and_verify on pwm1_enable
        │
        └── Windows: Named pipe write \\.\pipe\ZenOmen.Hardware.v1
                     - HardwareRequest::SetFanMode { mode: "max" } (BE framed JSON)
                     - zenomen-hw verifies client console session ID
                     - authorize_mutation checks BoardProfile & BoardValidation
                     - hp_wmi::set_fan_mode writes BIOS WMI (hpqBIntM)
                     - Performs read-back verification of fan state
        ▼
[IPC Response Returned]
        │
        ├── Linux: Ok(()) or D-Bus error
        └── Windows: HardwareResponse::FanModeSet("max") or HardwareResponse::Error(msg)
        ▼
[Mapped to AppEvent::FanModeChanged(Result<String, String>)]
        │ 4. Sent over event channel to zenomen-event-pump
        ▼
[AppState::apply(&event)]
        │ 5. On success: state.fan_mode = "max", state.last_control_error = None
        │    On error: state.last_control_error = Some(err_msg)
        ▼
[apply_to_ui] ──► [Slint UI reflects updated fan mode or displays error banner]
```

---

## Key Abstractions

### 1. `HardwareProvider` Trait (`src/hardware/provider.rs`)
The single polymorphic seam separating the core monitoring loop from platform-specific hardware telemetry:
```rust
pub trait HardwareProvider: Send + Sync {
    fn capabilities(&self) -> HardwareCapabilities;
    fn sample_telemetry(&self) -> Result<TelemetrySnapshot, String>;
}
```
- **Type Honesty:** If a metric is missing, `TelemetrySnapshot` fields evaluate to `None` or empty vectors (`Vec::new()`), never synthetic zeros.
- **Implementations:**
  - `LinuxHardwareProvider` (`src/hardware/linux/provider.rs`): Samples procfs, sysfs, and DRM using `SensorRegistry`.
  - `WindowsHardwareProvider` (`src/hardware/windows/provider.rs`): Samples PDH CPU counters, DXGI adapters, Win32 memory, and WMI sensors.
  - `MockHardware` (`src/hardware/mock.rs`): Deterministic test provider (`Nominal`, `ZeroCapabilities`, `SensorDataGap`).
  - `UnavailableHardwareProvider` (`src/hardware/unavailable.rs`): Safe zero-capability fallback for unsupported platforms.

### 2. `SensorRegistry` (`src/hardware/registry.rs`)
The canonical hardware sensor binding engine for Linux systems:
- Resolves CPU temperature via strict priority hierarchy: AMD Zen (`k10temp` → `Tctl` / `Tdie` / `temp1_input`) > Intel (`coretemp` → `Package id 0` / `temp1_input`) > ACPI thermal zones.
- Discovers fan tachometers and detects `pwm1_enable` sysfs control paths.
- Shared between `LinuxHardwareProvider` and `collect_diagnostics` so dashboard telemetry and diagnostics reports can never disagree.

### 3. `HardwareServiceClient` (`src/service/client.rs`)
The platform-agnostic client facade used by the core worker to communicate with `zenomen-hw`:
- Compiles conditionally using `#[cfg(target_os = ...)]`.
- On Linux, wraps a cached `zbus::blocking::Connection` targeting `com.zenomen.Hardware1`.
- On Windows, delegates to `WindowsServiceClient` (`src/service/windows/client.rs`) communicating over named pipe `\\.\pipe\ZenOmen.Hardware.v1`.
- Exposes uniform methods returning `Result<T, String>`.

### 4. `HardwareRequest` & `HardwareResponse` (`src/service/windows/protocol.rs`)
The live IPC wire protocol for the Windows named-pipe service:
- Serialized as JSON with a 4-byte big-endian length prefix, capped at 64 KiB (`MAX_PAYLOAD_SIZE`).
- Commands: `GetCapabilities`, `GetFanStatus`, `SetFanMode`, `GetPowerProfiles`, `GetPowerProfile`, `SetPowerProfile`, `ApplyPreset`, `ApplyRgbProfile`, `SetRgbEnabled`.
- Accompanied by the target shared protocol in `src/service/protocol.rs` (`ServiceRequest`/`ServiceResponse`, 4-byte little-endian framing with `PROTOCOL_VERSION = 1`).

### 5. `BoardProfile` & `BoardValidation` (`src/service/windows/profiles.rs`)
The safety gate for physical hardware mutations on Windows:
- Maintains a registry of known HP motherboard models (e.g., Victus 15 `8A3D`, OMEN 16 `8A13`).
- `authorize_mutation` checks whether a requested mutation is supported by the hardware profile and backed by an explicit `BoardValidation` record before issuing WMI commands.

### 6. `WmiAdapter` Trait (`src/service/windows/hp_wmi.rs`)
An injectable firmware transport abstraction for Windows HP BIOS WMI:
- Enables integration testing without physical HP hardware by allowing `tests/windows_service_tests.rs` to substitute `FakeWmiAdapter`.

---

## Architectural Constraints & Invariants

1. **Unidirectional Control Flow:** Slint UI callbacks NEVER access hardware, NEVER invoke IPC directly, and NEVER block. They only emit `AppCommand` values.
2. **Single-Writer UI State:** `AppState` is mutated ONLY inside `AppState::apply(&AppEvent)`. No other function or thread is permitted to assign to `AppState` fields.
3. **Privilege Separation:** All hardware writes must cross the IPC boundary to `zenomen-hw`. The GUI binary must never require or request root or administrative privileges.
4. **Verified Hardware Writes:** Any hardware mutation performed by `zenomen-hw` must verify actual hardware state via read-back before acknowledging success.
5. **Fail-Safe Shutdown:** On Windows service shutdown, fan mode is explicitly restored to Auto.
6. **Bounded Queues & Memory Safety:**
   - Command channel: depth 8 (`try_send` drops on saturation).
   - Telemetry history: max 120 samples.
   - IPC message frame: hard limit of 64 KiB.
   - Mutation RPC timeout: hard limit of 30 seconds.
7. **Resource Budget:** Memory consumption must stay under 40 MB RSS. Avoid heavy runtimes (no Tokio). Builds compile with `opt-level = "s"`, `lto = true`, and `panic = "abort"`.
8. **Feature Gating:** Slint UI code is feature-gated behind `default = ["ui"]`. The `zenomen-hw` daemon builds with `--no-default-features`, completely excluding the UI framework.

---

*Architecture analysis: 2026-11-09*
