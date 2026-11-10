<!-- refreshed: 2026-11-11 -->
# Architecture

**Analysis Date:** 2026-11-11

## System Overview

ZenOmen is a two-process, layered hardware control center. The unprivileged GUI renders telemetry and queues intent; the privileged service owns every hardware mutation. A typed message boundary (D-Bus on Linux, named pipe on Windows) sits between them, and a validated-board policy sits between the service and firmware.

```
┌────────────────────────────────────────────────────────────┐
│ zenomen (GUI, unprivileged, Slint)                         │
│  ui/app.slint ──src/app.rs (Slint bridge)──► AppState       │
│                       ▲                     │              │
│        AppEvent ◄─────┘                     ▼ AppCommand    │
│  src/core/worker.rs (1 Hz loop, mutation executor)          │
│  src/hardware/* (read-only telemetry providers)             │
└───────────────┬───────────────────────────┬─────────────────┘
                │ AppCommand → service       │ hardware reads
                ▼                            ▼
   src/service/client.rs                 sysfs/procfs (Linux)
   D-Bus  |  named pipe                  Win32/DXGI/PDH (Windows)
                │
┌───────────────▼────────────────────────────────────────────┐
│ zenomen-hw (privileged service)                            │
│  Linux:  src/service/contract.rs + polkit.rs                │
│  Windows: lifecycle.rs → server.rs → handle_request         │
│  authorize_mutation → BoardValidation gate → driver         │
│  Linux:  src/service/backend.rs (write_and_verify)          │
│  Windows: hp_wmi.rs / victus_s.rs (WMI firmware)            │
└────────────────────────────────────────────────────────────┘
```

**Core value encoded in types:** a missing sensor is `None`/empty, never `0` (`src/hardware/provider.rs`); every write is followed by a read-back (`src/service/backend.rs`, `hp_wmi::set_fan_mode_with_target`); every visible UI change is an `AppEvent` folded through `AppState::apply` (`src/core/events.rs`, `src/core/state.rs`).

## Component Responsibilities

- `src/core/` — domain contract. `events.rs` (typed `AppCommand`/`AppEvent`, `ScreenId`), `state.rs` (`AppState`, the only mutator), `worker.rs` (threading, command loop, mutation executor), `telemetry.rs` (bounded history + SVG paths), `diagnostics.rs` (report + atomic JSON export), `benchmark.rs` (RSS/CPU instrumentation), `profile_orchestrator.rs` (combined display+power+fan presets).
- `src/hardware/` — read-only boundary. `provider.rs` (`HardwareProvider` trait, `TelemetrySnapshot`, `HardwareCapabilities`), `registry.rs` (canonical sensor discovery), `linux/`, `windows/`, `mock.rs`, `unavailable.rs`, `process_memory.rs`, `rgb.rs`.
- `src/service/` — privileged control contract. `types.rs` (`FanMode`, `PresetResult`, `PowerProfileResult`, `HardwareControlError`), `client.rs` (platform-agnostic client facade), `backend.rs` (Linux sysfs writes), `contract.rs` + `polkit.rs` (Linux D-Bus/Polkit), `windows/` (protocol, pipe, security, server, board profiles, HP WMI driver, runbooks).
- `src/desktop/` — OS integration: single instance, tray, autostart, releases link, system-integration installer; Windows submodule mirrors each concern.
- `src/config/` — TOML schema, atomic persistence, corrupt-file recovery.
- `src/rgb/` — profile schema, built-in presets, OpenOMEN migration.
- `src/app.rs` + `ui/` — the Slint bridge and the live declarative UI.
- `src/ui/` — dormant GPUI-CE view-model scaffold, compiled but not used by either binary.

## Process Model & Security Boundaries

### Process 1: Unprivileged GUI (`zenomen`, `src/main.rs`)
- Runs as the logged-in user, `asInvoker` manifest on Windows (`build.rs`).
- No `sudo`, no raw sysfs writes, no network.
- Owns: Slint event loop, tray, single-instance, telemetry providers, config, logs, diagnostics export.
- Talks to the service only through `HardwareServiceClient`.

### Process 2: Privileged Service (`zenomen-hw`, `src/bin/zenomen-hw.rs`)
- Linux: system D-Bus service run by systemd (root), name `com.zenomen.Hardware1`.
- Windows: SCM service `ZenOmenHardwareService` (LocalSystem) with console fallback.
- Owns: Polkit checks (Linux), pipe DACL/session checks (Windows), board validation, firmware/sysfs writes, shutdown restore.

Boundary rules:
- The GUI can *request* but never *perform* a mutation; authorization happens in the privileged process.
- On Windows the board gate (`authorize_mutation`) runs before every request dispatch, including presets and RGB.
- On Linux the Polkit check runs per mutating method (`HardwareService::authorize`).
- Capability discovery is honest: an unrecognized Windows board yields a generic profile with no fan/power/RGB capability; an unsupported OS yields `UnavailableHardwareProvider`.

## Threading Model & Background Workers

| Thread | Created in | Purpose |
|---|---|---|
| main / Slint event loop | `src/main.rs` | UI thread; never samples hardware, never blocks on I/O |
| `zenomen-monitor-worker` | `src/core/worker.rs` | 1 Hz sampling deadline loop; receives `AppCommand`s via `recv_timeout`; broadcasts events |
| `zenomen-event-pump` | `src/main.rs` | Drains `AppEvent` receiver and calls `app.handle_event` |
| `zenomen-desktop-actions` | `src/main.rs` | Converts tray/D-Bus actions into commands or Slint invocations |
| `zenomen-ctrl-<name>` + `zenomen-rpc-<name>` | `src/core/worker.rs` (`execute_mutating_action`) | Offload each mutation so a Polkit prompt cannot stall the sampling loop; 30 s timeout |
| `zenomen-diag-export` | `src/app.rs` | Collects and writes the JSON diagnostics report off the UI thread |
| `zenomen-nvidia-watch` | `src/hardware/linux/gpu.rs` | Owns `nvidia-smi` polling; ticks read the latest sample |
| zbus executor | `src/bin/zenomen-hw.rs` | Serves D-Bus methods; Polkit calls are awaited inside the executor |
| pipe accept loop | `src/service/windows/server.rs` | Blocks in `ConnectNamedPipe`, verifies session, dispatches one request at a time |

Concurrency invariants:
- Exactly one hardware mutation may be in flight (`AtomicBool` + `InFlightGuard` in `src/core/worker.rs`). A second request is rejected with "Another hardware change is still in progress"; the guard is released by `Drop`, so a timeout or panic cannot leave hardware locked out.
- The UI is updated only inside `slint::invoke_from_event_loop`; the window is held as a `Weak` so background threads cannot keep it alive (`src/app.rs`).
- Sampling pauses while the UI is hidden (`SetUiVisible(false)` switches the loop from timed to blocking receive), preserving the idle CPU budget.
- A shared `Arc<Mutex<AppState>>` is used as the single source of truth; poisoned locks are recovered (`unwrap_or_else(|p| p.into_inner())`) because the payload is plain data.

## Data Flow Walkthroughs

### 1. Telemetry Sampling Flow (Read-Only)
1. `spawn_monitoring_worker` sends `CapabilitiesLoaded(provider.capabilities())` immediately.
2. Cold-start sample runs before the first tick so the window is not blank for a second.
3. On each tick `sample_and_dispatch` calls `provider.sample_telemetry()`.
4. If the provider reports no fan tachometers or no power profiles and the service is connected, the worker *enriches* the snapshot from `GetFanStatus` / `GetPowerProfiles` (this is how Windows — where fans live behind the service — reports them).
5. `AppEvent::TelemetryUpdated(snapshot)` travels to the event pump, which calls `AppState::apply` and then `apply_to_ui`.
6. `AppState::apply` pushes a `TelemetryPoint`, regenerates cached chart/sparkline SVG paths, and backfills fan mode / power profile only when unknown.
7. Every 5 ticks: service status + Polkit status refresh. Every 2 ticks: GUI and service process memory.

### 2. User Command Flow (UI → Core)
1. A Slint callback (`src/app.rs::install_callbacks`) converts a click into an `AppCommand` and `try_send`s it on a depth-8 `sync_channel`.
2. A full queue drops the click and logs at debug level — back-pressure, never a blocked UI.
3. The worker matches the command: navigation emits `ScreenChanged`; refresh samples immediately and resets the deadline; settings commands mutate `current_config`, persist atomically, and echo `ConfigLoaded`.

### 3. Privileged Mutation Flow (Write & Verification)
1. `AppCommand::SetFanMode/SetPowerProfile/ApplyPreset/ApplyRgbProfile/SetRgbPower`.
2. Worker announces intent where relevant (`FanModeRequested`) and calls `execute_mutating_action`, which spawns a short-lived RPC thread and waits up to 30 s.
3. `HardwareServiceClient` performs the platform call:
   - Linux: D-Bus method → `authorize` (Polkit) → `HardwareBackend` → `write_and_verify` (3 attempts, exponential backoff) → read-back confirmation.
   - Windows: frame over the named pipe → `handle_request` → `authorize_mutation` (board ID + BIOS + per-capability flags) → WMI driver → pre/post firmware read-back.
4. Result becomes `FanModeChanged` / `PowerProfileChanged` / `PresetApplied` / `RgbProfileApplied` / `RgbPowerChanged`; `AppState::apply` clears or records `last_control_error`, and `apply_to_ui` updates the banner and verified flags.
5. On failure the typed error text is shown verbatim; nothing is reported as applied unless the read-back agreed (or is explicitly marked `ProfileUnverified`).

### 4. Combined System Profile Flow
`ApplyPreset(name)` → `resolve_preset_targets` (`src/core/profile_orchestrator.rs`) maps `quiet/balanced/auto/performance|gaming` to refresh rate, power profile and fan mode → display change on Windows (verified via GDI read-back) → service `ApplyPreset` (power then fan, with per-leg success flags) → merged `PresetResult` with `partial_success`, optional `PowerProfileStatus`, and a message that names exactly what failed.
- `PresetResult::is_full_success` requires power, fan, and (when a display target exists) display success.
- `AppState` only sets `active_preset` when the preset fully succeeded; a partial result clears it.
- On shutdown the worker restores the configured refresh rate (`restore_default_refresh_rate`).

## Key Abstractions

### 1. `HardwareProvider` (`src/hardware/provider.rs`)
```rust
pub trait HardwareProvider: Send + Sync {
    fn capabilities(&self) -> HardwareCapabilities;
    fn sample_telemetry(&self) -> Result<TelemetrySnapshot, String>;
}
```
- Implementations: `LinuxHardwareProvider`, `WindowsHardwareProvider`, `MockHardware`, `UnavailableHardwareProvider`.
- `TelemetrySnapshot::EXPECTED_SENSOR_COUNT = 11`; `reported_sensor_count` derives presence from data presence, and `rgb_profile` is deliberately excluded because it is presentation state.

### 2. `SensorRegistry` (`src/hardware/registry.rs`)
- Single shared binding store for both dashboard and diagnostics (MON-12).
- CPU temp priority `k10temp` → `coretemp` → `acpitz`; fans from `fan*_input`; PWM only from HP/platform hwmon devices.
- Constructed with injectable roots (`scan_roots`) so tests never touch live hardware.

### 3. `HardwareServiceClient` (`src/service/client.rs`)
- One API for both platforms, `#[cfg]`-split internally.
- Linux caches a `zbus::blocking::Connection` and resets it on communication failure.
- Windows delegates to `WindowsServiceClient` (one connection per request).
- Converts "service missing" errors into instructions ("start with `sudo systemctl start zenomen-hw`").

### 4. `HardwareRequest` / `HardwareResponse` (`src/service/windows/protocol.rs`)
- Length-prefixed JSON enums capped at 64 KiB.
- Read-only vs mutating is encoded structurally and consumed by `authorize_mutation`.
- `PowerProfileSet(PowerProfileResult)` and `PresetApplied(PresetResult)` carry verification status rather than a bare string.

### 5. `BoardProfile` + `BoardValidation` (`src/service/windows/profiles.rs`)
- `BoardProfile` = what the board claims (fan count, Max Fan, custom curves, thermal source, power choices, RGB zones).
- `BoardValidation` = what has been physically proven, keyed by board **and** BIOS version.
- `BoardProfile::capabilities(validation)` joins both; no validation record means every write flag is false.
- `8A3D` is `ThermalProfileSource::EmbeddedController` (EC `0x95`) with `has_custom_curves = true` as a *protocol* capability.

### 6. `WmiAdapter` (`src/service/windows/hp_wmi.rs`)
```rust
pub trait WmiAdapter: Send + Sync {
    fn bios_call(&self, command: u32, command_type: u32, payload: &[u8], out_size: u32)
        -> Result<Vec<u8>, HardwareControlError>;
    fn is_available(&self) -> bool;
}
```
- `RealWmiAdapter` performs COM/WMI calls; `FakeWmiAdapter` replays scripted responses and records every call.
- Envelope construction is planned separately (`plan_hpq_input_envelope` / `populate_hpq_envelope`) so property types can be unit-tested without COM.

### 7. `AppState` (`src/core/state.rs`)
- One struct per session; one mutator (`apply`).
- Answers "what is true" and "what should the status strip say"; never formats screen strings (that is `src/app.rs`).
- `status_text()` and `is_degraded()` refuse to invent numbers before the first sample.

## Architectural Constraints & Invariants

1. **Single writer for UI state:** only `AppState::apply(&AppEvent)` mutates visible state; `src/app.rs::apply_to_ui` only formats. (`src/ui/app.rs` is the exception — see CONCERNS.)
2. **No hardware in the UI thread:** providers are sampled on `zenomen-monitor-worker`; mutations run on dedicated RPC threads with a 30 s cap.
3. **No fabrication:** absence is `None`/empty/`Unavailable`, never `0`; capability flags are derived from readable interfaces (`src/hardware/registry.rs`, `src/hardware/provider.rs`).
4. **Verified writes:** Linux `write_and_verify`; Windows pre-write read, write, 150 ms settle, post-write read; mismatch is `VerificationFailed`.
5. **Board-gated Windows mutations:** unrecognized board, mismatched BIOS, or unverified capability blocks the write before any firmware call.
6. **Platform isolation:** all Linux hardware paths under `src/hardware/linux/` and `src/service/backend.rs`; all Windows driver code under `src/hardware/windows/` and `src/service/windows/`. Other OSes report zero capabilities.
7. **Honest combined results:** `PresetResult.partial_success` and `PowerProfileStatus::ProfileUnverified` exist so a partial or unreadable outcome is never presented as full success.
8. **Safe 8A3D restoration:** never send `0x2E {0,0}` on `8A3D`; require a known profile or an explicit safe fallback, otherwise refuse before writing.
9. **Bounded resources:** `panic = "abort"`, size-optimized release profile, no Qt/Skia, sampling paused when hidden, bounded history (30/60/120/300 samples).
10. **Graceful teardown:** `Shutdown` command → restore refresh rate → drop senders → join worker/event pump/desktop-action threads; Windows service restores fan mode only on validated boards.

---

*Architecture analysis: 2026-11-11*
