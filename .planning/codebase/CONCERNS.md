<!-- refreshed: 2026-11-11 -->
# Codebase Concerns

**Analysis Date:** 2026-11-11
**Scope:** full repository at `86a33bb` (branch `fix/windows-8a3d-authorization-and-restore`)

---

## Tech Debt

### 1. ~900 lines of unshipped GPUI scaffold compiled into the library
- Files: `src/ui/app.rs`, `src/ui/components/*.rs`, `src/ui/screens/*.rs`, `src/ui/theme/*.rs` (897 lines total).
- `src/lib.rs` compiles `pub mod ui` under the `ui` feature, but neither `src/main.rs` nor `src/bin/zenomen-hw.rs` constructs `GpuiApp`.
- It is not dead in the compiler sense (it is `pub`), so Clippy cannot flag it, and it silently ages against the real Slint UI.
- It also violates AGENTS.md rule 6: `GpuiApp::handle_event` (`src/ui/app.rs`) assigns `self.state.customer_field = ...` directly instead of routing through `AppState::apply`, creating a second, divergent writer of domain state.
- Decision record: `docs/decisions/ADR-002-gpui-ce-migration.md`. Either finish or delete it; keeping it cost-free is not true.

### 2. Documentation drift in `profiles.rs`
- `src/service/windows/profiles.rs` module docstring says board `8A3D` has "Manual PWM and power mutations remain blocked", and the `resolve_board_validation` doc says "Board 8A3D ... is explicitly unvalidated pending physical testing".
- The actual arm for `8A3D` on BIOS `F.22` returns `power_write_authorized: true` and `fan_write_verified: true`, with the F.22 validation comment block above it.
- Risk: a future editor trusting the prose could "restore" the block, or a reviewer could misread the current safety posture. This is exactly the drift AGENTS.md rule 10 is meant to prevent.

### 3. Stale "70/70 passing" claim in planning state
- `.planning/STATE.md` records "unit tests 70/70 passing" for Phase 07; the tree now contains 322 test attributes and a new 8A3D authorization surface.
- Risk: phase verification claims that outlive the code they described.

### 4. Windows capability reporting is inferred from the board catalog, not probed
- `WindowsHardwareProvider::capabilities` (`src/hardware/windows/provider.rs`) sets `fan_rpm`, `fan_control`, `power_profiles`, and `rgb_lighting` from `BoardProfile` alone.
- The board catalog describes protocol support, not whether `ZenOmenHardwareService` is installed, running, or WMI-reachable.
- Result: the GUI can advertise fan/power controls on a machine where every mutation will fail with "named pipe unavailable" or `InterfaceMissing`.
- Contrast with `hp_wmi::query_hp_capabilities`, which does probe firmware but is only reached through the service.

### 5. Two overlapping UI stacks and two D-Bus/pipe client layers are documented as one
- `src/service/client.rs` is the façade, but Windows-specific behavior lives in `src/service/windows/client.rs` with its own retry loop; the facade just forwards. Acceptable, but every new method must be added in both places plus `HardwareRequest`/`HardwareResponse`, `contract.rs` (Linux) and `backend.rs`.

### 6. No `TODO`/`FIXME` markers
- A repo-wide search finds zero `TODO`, `FIXME`, `XXX`, or `HACK` comments. That is tidy but means deferred work is only recorded in `.planning/` and `docs/`, not at the code site.

---

## Known Bugs / Behavioral Defects

### 1. Windows provider fabricates `power_profile` and `fan_mode`
`src/hardware/windows/provider.rs::sample_telemetry`:
```rust
let (fan_mode, power_profile, power_profiles) = if let Some(ref board) = self.board_profile {
    let (p_first, p_list) = ... board.supported_power_profiles.first() ...
    ("auto".to_string(), p_first, p_list)
};
```
- `fan_mode` is hardcoded `"auto"` even when the service is offline and the real mode is `max`.
- `power_profile` is the *first supported* profile, presented as the *active* profile, with no read-back. On `8A3D` the active profile lives in EC `0x95` and cannot be read from userland — the honest value is "unavailable", not "quiet".
- Because `TelemetrySnapshot::reported_sensor_count` counts non-empty `fan_mode`/`power_profile`, the status strip can claim "11 of 11 sensors reported" while two of those values were invented.
- `AppState::apply` only backfills these when empty (`if self.fan_mode.is_empty()`), so the fabricated values persist even after a verified `FanModeChanged`.

### 2. Production fallback installs a test fake
`src/service/windows/server.rs::run_pipe_server`, WMI connect failure path:
```rust
hp_wmi::connect_hp_wmi().unwrap_or_else(|_| {
    HpWmiConnection::from_adapter(crate::service::windows::hp_wmi::FakeWmiAdapter::new(false))
})
```
- It retries the connection that just failed, then places `FakeWmiAdapter` (a test double) into a live service. It reports `is_available = false`, so reads fail honestly, but the production path depending on a test-only type is a maintenance and audit hazard. A dedicated `UnavailableWmiAdapter` would express the intent.

### 3. `8A3D` shutdown restore always picks `Balanced`
- `server.rs` calls `set_fan_mode_with_target(..., SafeFallback(ThermalProfile::Balanced))` when `requires_thermal_profile_restore` is true.
- The runbook path requires a *known* active profile; shutdown uses the fallback because the profile is unreadable. This is deliberate and logged, but it means stopping the service always writes "Balanced" on `8A3D`, changing a user's Quiet/Performance selection. Docs should state this as user-visible behavior.

### 4. Combined-preset `display_applied = true` on Linux without a display change
- `src/core/profile_orchestrator.rs` sets `(display_applied, confirmed_refresh_rate, display_message) = (true, None, None)` for non-Windows.
- `PresetResult::is_full_success` tolerates this because `confirmed_refresh_rate.is_none()`, but the field literally reads "display applied" when nothing display-related happened.

### 5. Migrated preset name is not normalized in one path
- `HardwareBackend::apply_preset` (Linux) accepts `gaming` and maps it to `performance`; `profile_orchestrator` resolves `gaming` to a target named `performance`, so `PresetResult.preset_name` differs between platforms.
- `AppState::apply` compensates by checking `preset_name == "gaming" || "performance"` when guessing the fan mode, which is a symptom rather than a fix.

---

## Security Considerations

### Strengths
- Strict two-process separation; GUI never writes hardware and never invokes `sudo`.
- Linux: every mutating D-Bus method calls `authorize` with the caller's unique bus name and a Polkit action (`com.zenomen.hardware.control`); interactive flag is on for mutations, off for diagnostics.
- Windows: pipe DACL restricts to SYSTEM/Administrators/Interactive Users (`D:(A;;GA;;;SY)(A;;GA;;;BA)(A;;GRGW;;;IU)`); client must be in the active console session; ASLR-friendly `asInvoker` GUI manifest.
- Board validation is a second, hardware-specific authorization gate: unknown board, wrong BIOS, or unverified capability blocks writes before firmware.
- sysfs input validation rejects path separators and control characters in power-profile names (`HardwareBackend::set_power_profile`).
- IPC framing caps payloads at 64 KiB on both encode and decode.

### Risks
1. **Session check degrades silently in console mode.** `verify_client_session` (`src/service/windows/security.rs`) skips token impersonation when `ImpersonateNamedPipeClient` fails, logging at `debug`, and relies solely on `GetNamedPipeClientSessionId`. In a LocalSystem SCM service this branch should not trigger, but a failure there would weaken the check without an alert. Consider `warn` and/or failing closed when running as a service.
2. **`RevertToSelf` failure calls `std::process::exit(1)`** mid-request from deep inside the security module. Correct for safety, but it is a hard process kill reachable from the request path; worth documenting as an intentional last resort.
3. **`unsafe` surface is large and hand-written.** ~181 lines reference `unsafe` across `wmi_ffi.rs`, `hp_wmi.rs`, `security.rs`, `tray.rs`, `display.rs`, `single_instance.rs`. Hand-rolled COM vtables in `wmi_ffi.rs` (681 lines) are the highest-risk code in the repo; they are covered by 7 unit tests plus protocol tests, but not by any live-COM test.
4. **Firmware authorization is conflated at the transport layer.** `RealWmiAdapter` maps `WBEM_E_ACCESS_DENIED` to `AuthorizationFailed`, which the D-Bus mapping treats as `AccessDenied`. That is correct for Linux, but on Windows it mixes "needs elevation" with "board refused" in user-facing text.
5. **Preset/RGB payloads are JSON strings over IPC** (`ApplyRgbProfile { profile_json }`). They are parsed with `serde_json` and then re-validated; keep the validation in `set_rgb_profile` (which currently always refuses) rather than assuming the struct fields are sane.
6. **Polkit dependency is not pinned to a minimum version** in the policy file review; the action allows active local users. `data/polkit/com.zenomen.hardware.policy` should be reviewed whenever `allow_active` changes.

---

## Performance Bottlenecks

### 1. `nvidia-smi` polling is process-based
- `NvidiaSampler` (`src/hardware/linux/gpu.rs`) spawns `nvidia-smi` on a watcher thread. The 1 Hz tick never blocks, but each invocation costs CPU and can be tens of milliseconds. On hybrid laptops this is the dominant sampling cost.
- No sysfs/NVML fallback; if `nvidia-smi` is absent, NVIDIA telemetry is simply unavailable (honest, but limited).

### 2. Two-level enrichment on Windows couples telemetry to IPC
- Every tick where the provider returns empty fan/power data, `sample_and_dispatch` performs additional pipe calls (`GetFanStatus`, `GetPowerProfiles`), each opening a new pipe handle (`CreateFileW` + `WaitNamedPipeW` loop).
- At 1 Hz with a 500 ms floor, that is up to 3 pipe connections per sample; `WindowsServiceClient` has no connection pooling.

### 3. `AppState::apply` regenerates SVG paths on every sample
- `generate_chart_paths_cached` and `generate_sparkline_paths_cached` run per `TelemetryUpdated` and on every config/history change. Bounded (≤300 points) but non-trivial string formatting on the caller thread (the event pump, not the UI thread).
- `history.set_duration` is recomputed with `config.telemetry.polling_interval_ms` — a config that can change independently of the window.

### 4. Service memory sampling every 2 ticks
- `sample_current_process_memory` + a pipe call every ~2 s is small but unnecessary when the diagnostics screen is not visible; it runs regardless of `active_screen`.

### 5. Budget enforcement is manual
- `scripts/measure-budget.sh` is not wired into CI. The 40 MB / 15 MB budgets are verified only when a human runs the script; `tests/benchmark_tests.rs` asserts a *headless baseline*, which is not the GUI peak.

---

## Fragile Areas

### 1. `hp_wmi.rs` protocol sequencing (2048 lines)
- Fan writes depend on exact ordering: `0x10` arm → `0x27=0` → `0x2F` validate → `0x2E {cpu,gpu}`; restore depends on `0x27=0` → `0x10` → `0x1A`.
- Small changes (moving the `get_fan_count` call, reordering the 150 ms sleep, or sending `0x2E` before `0x27=0`) can stall fans or trip the firmware's 120 s window. `tests/windows_service_tests.rs` covers this, but only against `FakeWmiAdapter`.

### 2. `8A3D` EC thermal profile handling
- `0x2E {0,0}` freezes this board's fans; the code path that avoids it (`restore_automatic_fan_control_with_target`) is load-bearing safety logic.
- Any new caller of `set_victus_fan_levels` must respect the guard that rejects `(AUTOMATIC, AUTOMATIC)` on `8A3D`.

### 3. Fan-table parsing (`gpu_offset`)
- Row 0 can have `gpu_rpm == 0`; offset must come from the first non-zero GPU row. `parse_fan_table` and `FanTable::gpu_level_for_cpu` implement this, and `tests/windows_service_tests.rs` exercises it, but manual-level writes are still `manual_fan_write_verified: false`, so this math is currently unshipped on real hardware.

### 4. Platform `#[cfg]` duplication
- `src/service/client.rs` (~559 lines) contains parallel Linux and Windows bodies for the same nine methods. A missed arm compiles on one platform and fails at runtime on the other; CI covers both, but only compile+tests, not live IPC.

### 5. `main.rs` teardown ordering
- Clean shutdown depends on dropping `command_sender`, `ui`, `app`, then joining worker and event pump, and finally the desktop-action thread. Reordering drops can deadlock or lose the refresh-rate restore (`restore_default_refresh_rate` runs inside the `Shutdown` arm).

### 6. `src/app.rs` string formatting as the only UI contract
- 620 lines of `set_*` properties; a renamed Slint property is a compile error (good), but a *semantic* mismatch (e.g. empty string vs `0`) is silent. `fan_speed` intentionally returns empty for absence; `gpu_utilization` maps `None` to `0` — inconsistent absence encoding.

### 7. Logging as a side channel for support
- `is_running_as_service()` infers service identity from the executable file name containing `zenomen-hw`. Renaming/installing under another name silently sends service logs to `%LOCALAPPDATA%` and changes the log location users are told to check.

---

## Scaling Limits

- Hardcoded two-fan assumption in UI (`fan-1-value`, `fan-2-value` in `ui/app.slint`) and in `read_fan_rpms` enrichment; `TelemetrySnapshot.fan_rpms` is a `Vec`, but only two are displayed.
- Six screens are a compile-time set (`ScreenId::ALL`); adding a screen touches the enum, `index()`, `from_index()`, `title()`, `ui/app.slint`, and `src/app.rs` routing.
- RGB is fixed at four zones (`[RgbColor; 4]`) across schema, migration, backend and UI. Boards with 1 zone map to `Legacy` but the type still carries 4.
- History window is limited to 30/60/120/300 seconds by `AppConfig::sanitize`; arbitrary values silently reset to 120. Buffer capacity is derived from the polling interval (`capacity_for_duration`), so the same window holds 60–600 samples depending on cadence.
- Board catalog is a hardcoded match in `resolve_board_profile`; every new model requires a code change and a re-release.
- Pipe server handles one connection at a time (accept → read → respond → disconnect). Fine for one GUI, but two concurrent clients serialize.

---

## Dependencies at Risk

| Dependency | Risk | Mitigation |
|---|---|---|
| `slint 1.17` (pinned) | Large transitive tree; renderer/backend features are load-bearing for the RSS budget; upgrades can change rendering or inflation size | Pinned version + `default-features = false`; budget script exists but is not in CI |
| `zbus 5` | Pulls an async executor stack into an otherwise synchronous app; API churn between 4.x/5.x | Isolated behind `src/service/contract.rs` and `client.rs`; `blocking` feature used deliberately |
| `windows-sys 0.59` | Raw, unsafe bindings; feature-flag creep; version bumps can rename modules | Features are explicitly enumerated in `Cargo.toml`; usage confined to `#[cfg(windows)]` modules |
| `directories 6.0` | Config/log paths differ per platform and version | Centralized in `config::project_dirs` / `logging::log_directory` / `desktop::windows::paths` |
| `tracing-appender 0.2` | Non-blocking writer requires the guard to live for the process lifetime | `LoggingGuard` held in both `main`s |
| `winres 0.1` | Unmaintained build dependency; failure is downgraded to a `cargo:warning` | Build script tolerates failure |

No `cargo-deny`, no license audit beyond `cargo audit` in CI. GPL-3.0-only licensing of the project is declared in `Cargo.toml`, `LICENSE`, and `NOTICE`.

---

## Missing Critical Features

- **Windows RGB is unimplemented**: `hp_wmi::set_rgb_profile` always returns `UnsupportedMode`; `server.rs`'s `SetRgbEnabled` is a no-op ack. The UI still exposes a Lighting screen on Windows boards with `rgb_zone_count > 0` (none of the current catalog boards), so this is latent.
- **Manual fan curves / per-fan PWM** are parsed and validated but gated off (`manual_fan_write_verified: false` everywhere).
- **Linux display refresh-rate control** does not exist; combined presets are Windows-only for that leg.
- **No live end-to-end test** of the Linux D-Bus service (no test bus fixture), so method signature changes are verified only by compile-time `zbus` macros.
- **No CI budget gate**; no automated soak test of the 120 s firmware fan window or the 30 s observation state machine.
- **No update mechanism**; the Releases page is opened in a browser and the user installs manually.

---

## Uncommitted Work & Release Readiness

- Working tree is clean at `86a33bb`; branch is `fix/windows-8a3d-authorization-and-restore`, 14 commits ahead of the last codebase-map refresh.
- Version is `0.1.0` for both binaries; release workflow enforces tag == `Cargo.toml` version.
- Physical validation for `8A3D`/F.22 is recorded in code (`profiles.rs`) and in `.planning/STATE.md`, but the evidence file lives in `docs/testing/HP_BOARD_VALIDATION.md` — confirm it names the same BIOS and measurements before shipping.
- `README.md` now documents the packaging paths and test counts reported by `Cargo.toml` and the suite inventory; keep it in step when either changes.

---

## Test Coverage Gaps

1. **Live COM/WMI**: `RealWmiAdapter`'s 400+ lines of COM marshalling are only exercised through `FakeWmiAdapter`. `wmi_diag` exists but is manual.
2. **Live D-Bus service**: no integration test boots `HardwareService` on a private bus and calls the nine methods; only pure Polkit signature serialization is tested.
3. **`src/ui/` scaffold**: zero tests, while it mutates state outside `AppState::apply`.
4. **GUI rendering**: `benchmark_tests` asserts headless safety, not visual correctness; no screenshot/visual regression.
5. **Pipe server loop**: protocol framing and `handle_request` are tested, but the `ConnectNamedPipe`/`ReadFile` loop and `verify_client_session` are not.
6. **Degradation on Windows**: `degradation_tests.rs` focuses on Linux provider injection; Windows "service offline" behavior is covered indirectly via `ServiceConnectionStatus`.
7. **Concurrency edge cases**: the in-flight guard is tested; simultaneous `UpdateConfig` + sampling, or a `Shutdown` while a mutation RPC is outstanding, are not.
8. **Config migration**: corrupt-file recovery is tested; forward/backward-compatible field addition (unknown keys, removed keys) is not.

---

*Concerns analysis: 2026-11-11*
