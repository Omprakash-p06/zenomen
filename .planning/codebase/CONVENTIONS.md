# Coding Conventions

**Analysis Date:** 2026-11-09

## Naming Patterns

**Files:**
- Rust modules are `snake_case.rs`; directories are singular, lower-case domain areas: `src/core/`, `src/hardware/`, `src/service/`, `src/desktop/`, `src/config/`, `src/rgb/`, `src/ui/`.
- One concept per file, and each directory has a `mod.rs` that declares submodules and re-exports the public surface (see `src/core/mod.rs`, `src/service/mod.rs`, `src/hardware/mod.rs`).
- Platform-specific implementations live in a platform directory: `src/hardware/linux/*.rs`, `src/hardware/windows/*.rs`, `src/service/windows/*.rs`, `src/desktop/windows/*.rs`.
- Slint files are `kebab-case.slint` under `ui/`: `ui/components/status-strip.slint`, `ui/screens/cooling.slint`, `ui/theme/colors.slint`.
- Integration test files are `<area>_tests.rs` under `tests/`: `tests/fan_control_tests.rs`, `tests/windows_service_tests.rs`, `tests/hardware_service_tests.rs`.

**Functions:**
- `snake_case`, verb-first for actions (`spawn_monitoring_worker`, `write_and_verify`, `encode_frame`, `resolve_board_profile`), noun-ish for pure accessors (`capabilities`, `status_text`, `as_sysfs_value`).
- Fallible operations return `Result<_, _>` and are named for the operation, not with `try_` (`load_or_default`, `parse_requested`, `sample_telemetry`, `read_fan_status`).
- Predicate accessors use `is_`/`has_` prefixes: `is_degraded`, `has_fan_control`, `has_any_reading`, `is_full_success`, `is_available`, `has_cpu_metrics`.
- Constructor variants: `new()` for the normal path, `from_*` for derived construction (`from_sysfs`, `from_snapshot`, `from_adapter`, `from_env`), `default()` via `impl Default` where a safe zero value exists.

**Variables:**
- `snake_case`; booleans read as assertions (`close_to_tray`, `require_polkit`, `has_battery`, `available`, `allow_fan_writes`).
- Channel/thread locals name their role: `command_sender`, `event_receiver`, `mutation_in_flight`, `command_receiver`.
- Named OS threads use kebab-case, prefixed with the product: `"zenomen-monitor-worker"`, `"zenomen-event-pump"`, `"zenomen-desktop-actions"`, `"zenomen-win32-tray-pump"`.

**Types:**
- `PascalCase`; traits are capability nouns (`HardwareProvider`, `WmiAdapter`, `HardwareBackend`), enums name the concept (`FanMode`, `AppCommand`, `AppEvent`, `MockScenario`, `ServiceError`, `BoardValidation`).
- Error enums end in `Error` (`HardwareControlError`, `ServiceError`, `ProtocolError`, `RgbError`, `SystemIntegrationError`); operation results that aggregate an outcome use `*Result` (`PresetResult`).
- Domain types are re-exported from their module `mod.rs` so callers use one import hop (for example `use zenomen::service::types::{FanMode, PresetResult, HardwareControlError}` via `src/service/mod.rs`).

**Constants:**
- `SCREAMING_SNAKE_CASE`, declared near the top of the file, accompanied by a `///` doc comment explaining the *why*, not just the value: `HISTORY_WINDOW`, `SUBSYSTEM_COUNT`, `MAX_FRAME_BYTES`, `COMMAND_QUEUE_DEPTH`, `SAMPLING_INTERVAL`, `GAP_PERIOD_TICKS`, `GAP_TICKS` (`src/core/state.rs`, `src/service/protocol.rs`, `src/app.rs`, `src/core/worker.rs`, `src/hardware/mock.rs`).
- Associated constants live on the type they constrain, e.g. `TelemetrySnapshot::EXPECTED_SENSOR_COUNT`, `MockScenario::ALL`, and `ScreenId::ALL` (`src/hardware/provider.rs`, `src/hardware/mock.rs`, `src/core/events.rs`).

**Test names:**
- Unit tests inside `#[cfg(test)] mod tests` use full descriptive sentences, no `test_` prefix: `starts_waiting_for_a_reading_and_reports_nothing_supported`, `oversized_frame_is_rejected_before_allocation` (`src/core/state.rs`, `src/service/protocol.rs`).
- Integration tests in `tests/` use the `test_` prefix and a verb phrase: `test_fan_mode_state_transitions`, `test_named_pipe_protocol_roundtrip_all_requests`, `test_set_fan_mode_returns_authorization_failed_when_access_denied`.
- Requirement-matrix suites number cases so failures map directly to the specification: `degradation_01_fan_hwmon_absent_leaves_cpu_temp_working` (`tests/degradation_tests.rs`).

## Code Style

**Formatting:**
- `rustfmt` with **default settings** — there is no `rustfmt.toml`, no `.editorconfig`, and no `.gitattributes` in the repository root. Match `cargo fmt` output exactly; CI enforces `cargo fmt --all -- --check` (`.github/workflows/ci.yml`).
- Rust **edition 2024** (`Cargo.toml`). Idioms from the edition are used freely: `let ... && let ...` chains (`src/main.rs`, `tests/logging_tests.rs`), `unsafe` blocks with explicit `// SAFETY:` comments, and inline `format!` captures (`{e}`, `{path:?}`).
- Keep lines within rustfmt's default 100-column width; the codebase wraps long calls and `match` arms to satisfy it.
- Larger files are divided with box-drawing section headers, e.g. `// ───────── WmiAdapter trait ─────────` in `src/service/windows/hp_wmi.rs` and numbered banners in `tests/windows_service_tests.rs`.
- Operator precedence is made explicit with parentheses where it aids readability (`(body.len() as u32).to_le_bytes()` in `src/service/protocol.rs`).

**Linting and Clippy Zero-Warning Policy:**
- CI gate: `cargo clippy --all-targets --all-features -- -D warnings` on both Linux and Windows (`.github/workflows/ci.yml`).
- Zero warnings are permitted across all build targets and feature combinations (`--all-targets --all-features`).
- There is no `clippy.toml`; standard Rust 2024 lint suites are active.
- `#[allow(...)]` is used sparingly and **always justified in a comment or doc note**:
  - `#[allow(clippy::large_enum_variant)]` on `AppEvent` with a rationale about one-sample-per-tick movement (`src/core/events.rs`).
  - `#[allow(unreachable_code)]` after platform-conditional early returns (`src/core/benchmark.rs`).
  - `#[allow(unused_mut)]` / `#[allow(unused_variables)]` where a value is mutated or used on one OS target (`src/hardware/windows/provider.rs`, `src/service/windows/security.rs`).
- Windows FFI module deliberately opts out of naming/style lints for C/COM ABI fidelity: `#![cfg(windows)]`, `#![allow(non_snake_case, non_camel_case_types, non_upper_case_globals, unused)]` (`src/service/windows/wmi_ffi.rs`).
- No `TODO`, `FIXME`, `HACK`, or `XXX` markers are allowed in `src/` or `tests/`; future work items are strictly managed in `.planning/`.

## Import Organization

**Order (strictly observed):**
1. `std::` imports first (alphabetical by crate path, grouped in rustfmt order).
2. External crate imports (`directories`, `serde`, `slint`, `thiserror`, `toml`, `tracing`, `windows_sys`, `zbus`).
3. A blank line.
4. Crate-local imports beginning with `crate::` (or `zenomen::` in integration tests).

Example from `src/service/protocol.rs`:
```rust
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::hardware::HardwareCapabilities;
use crate::rgb::RgbProfile;
use crate::service::types::{FanMode, HardwareControlError, PresetResult};
```
Example from `src/main.rs`:
```rust
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::sync::mpsc;

use slint::ComponentHandle;
use zenomen::AppWindow;
use zenomen::app::ZenOmenApp;
```

**Path Aliases:**
- None. There are no Cargo aliases or `extern crate` shims; always spell the complete module path (`crate::core::events`, `crate::service::types`).
- Platform-only imports are gated with `#[cfg(target_os = "linux")]` / `#[cfg(windows)]` directly on the `use` item (`src/main.rs`, `src/bin/zenomen-hw.rs`).

**Re-exports / Barrels:**
- Each directory's `mod.rs` re-exports the types callers should use (`src/core/mod.rs`, `src/service/mod.rs`, `src/hardware/mod.rs`, `src/ui/mod.rs`). Add a re-export there rather than making callers reach into a private submodule.
- Slint-generated modules are re-exported once at the crate root: `slint::include_modules!()` in `src/lib.rs` behind `#[cfg(feature = "ui")]`.

## Error Handling

**Strategy:** Typed, explicit errors with `thiserror`; no `anyhow`, and no error-string boxing except at top-level binary entry points.

**Patterns:**
- Domain errors derive `thiserror::Error` with `#[error("...")]` messages carrying contextual details:
  - `HardwareControlError` (`src/service/types.rs`): `DeviceNotFound`, `PermissionDenied`, `InterfaceMissing`, `VerificationFailed`, `CommandFailed`, `InvalidValue`, `Unsupported`, `CommunicationFailed`, `AuthorizationFailed`.
  - `ServiceError` and `ProtocolError` (`src/service/protocol.rs`): wire-serializable failure representations.
  - `RgbError` (`src/rgb/schema.rs`): configuration parsing, hex color syntax, zone range validation.
  - `SystemIntegrationError` (`src/desktop/system_integration.rs`): autostart, tray, and SCM errors.
- Error enums implement `Clone + PartialEq + Eq` when crossing IPC boundaries (`ServiceError` is `Serialize + Deserialize`, `src/service/protocol.rs`).
- Cross-layer mapping uses explicit `From` implementations:
  - `impl From<HardwareControlError> for ServiceError` (`src/service/protocol.rs`)
  - `impl From<HardwareControlError> for zbus::fdo::Error` (`src/service/types.rs`)
- Hardware provider boundary uses a human-readable error: `fn sample_telemetry(&self) -> Result<TelemetrySnapshot, String>` (`src/hardware/provider.rs`); worker propagates this as `AppEvent::TelemetrySamplingFailed` (`src/core/worker.rs`).
- Mutation results surface to the UI as `Result<String, String>` or `Result<PresetResult, String>` variants of `AppEvent` (`src/core/events.rs`); `AppState::apply` stores the failure message in `last_control_error` (`src/core/state.rs`).
- Binary entry points return `Result<(), Box<dyn std::error::Error>>` (`src/main.rs`, `src/bin/zenomen-hw.rs`).
- Mutex poisoning is recovered safely rather than panicked: `.lock().unwrap_or_else(|p| p.into_inner())` (`src/app.rs`, `src/service/windows/hp_wmi.rs`, `src/hardware/linux/gpu.rs`).
- `unwrap()` and `expect()` are confined to test code and startup invariants. Runtime `expect` strings explain why failure is impossible and conclude with "must succeed" (`.expect("spawning monitoring worker thread must succeed")` in `src/core/worker.rs`).

## Logging

**Framework:** `tracing` + `tracing-subscriber` with `EnvFilter` and daily-rotating file appender (`src/logging.rs`, `Cargo.toml`).

**Patterns:**
- Structured key-value fields over string interpolation: `tracing::warn!(error = %e, "fan mode change error recorded in state")`, `tracing::info!(version = env!("CARGO_PKG_VERSION"), "starting ZenOmen")`, `tracing::warn!(?dropped, "command receiver is gone; request dropped")`.
- Field syntax: `%value` for `Display`, `?value` for `Debug`, `path = %path.display()` for filesystem paths.
- Message text is lower-case, sentence fragments without trailing periods ("telemetry sampling failed", "monitoring worker stopped", "loaded fallback configuration").
- Log levels:
  - `info!`: Major lifecycle milestones (startup, provider selection, tray hide/show, graceful shutdown).
  - `warn!`: Recoverable degradation (missing sysfs nodes, offline daemon, dropped channel messages, sampling hiccups).
  - `error!`: Unrecoverable errors or security rejections (`src/service/windows/security.rs`).
  - `debug!`/`trace!`: Kept minimal to avoid polluting release log files.
- `eprintln!` is strictly reserved for pre-logging bootstrapping (`src/logging.rs`) and CLI invocation errors (`src/main.rs`, `src/bin/zenomen-hw.rs`).
- Zero secrets or raw hardware buffers may ever be logged. Error messages reaching logs are typed `Display` strings guaranteed safe to log.

## Comments and Documentation

**Commentary Rules:**
- Comments explain the *why*, safety invariants, and architectural trade-offs, never restating obvious code. Examples: the weak window reference rationale in `src/app.rs`, the bound queue capacity preventing UI freezes in `src/core/worker.rs`, the sine-phase math in `src/hardware/mock.rs`.
- Protocol workflows use numbered step comments matching firmware and IPC contracts: `// 1. Initialise COM for this thread.`, `// 2. Query WbemLocator...` (`src/service/windows/hp_wmi.rs`).
- Requirement and specification IDs are cited inline: `FOUND-05`, `MON-11`, `SYS-06`, `WIN-03`, `WIN-04`, `WIN-07`, `T-01-04`, `PERF-01`, `RGB-02`, `WR-11`.
- `// SAFETY:` comments are mandatory before every `unsafe` block, explicitly describing the pointer validity, thread exclusivity, or OS invariant (`src/main.rs`, `src/service/windows/hp_wmi.rs`, `src/service/windows/wmi_ffi.rs`).

**Rustdoc:**
- Every module file begins with `//!` module documentation describing responsibilities, architecture, and constraints (`src/core/state.rs`, `src/hardware/provider.rs`, `src/service/protocol.rs`, `src/service/windows/hp_wmi.rs`).
- Public functions and structs have `///` documentation with concise imperative summaries, argument notes, unit indicators (`BatteryTelemetry.charge_percent`), and error conditions.
- Intra-doc links (`[`AppState::apply`]`, `[`HardwareProvider`]`) are used consistently.

## Module and Architecture Layering

- `src/core/`: Platform-agnostic core logic. Commands, events, state machine, telemetry history ring buffer, and the 1 Hz monitoring worker loop. Zero imports from `crate::hardware::linux` or platform Win32/libc APIs (`src/core/mod.rs`).
- `src/hardware/`: Sensor abstraction boundary. Defines `HardwareProvider`, `TelemetrySnapshot`, and `HardwareCapabilities`. Submodules `linux/`, `windows/`, `mock/`, and `unavailable/` implement provider instances.
- `src/service/`: Privileged hardware control contracts. `protocol.rs` defines cross-platform wire frames; `backend.rs` implements Linux sysfs control; `windows/` implements the Windows named pipe service, WMI dispatch, security authorization, and board validation.
- `src/app.rs`: The sole application bridge between Slint UI and the core worker/state engine.
- `src/ui/`: Non-wired GPUI scaffold (retained as experimental/historical scaffold). Active UI is strictly built with Slint in `ui/**/*.slint`.

## Mock Hardware Conventions

ZenOmen provides an in-tree mock hardware provider (`src/hardware/mock.rs`) supporting three distinct operational scenarios:
1. `MockScenario::Nominal`: All hardware subsystems present (`HardwareCapabilities::all_supported()`). Values oscillate within plausible ranges (CPU 15–45%, GPU 30–70%, fans 2400–3800 RPM, battery 85% discharging) to keep live charts moving.
2. `MockScenario::ZeroCapabilities`: Simulates unsupported hardware or missing drivers (`HardwareCapabilities::none()`). Emits `TelemetrySnapshot::default()` with zero reported sensors, asserting the honest absence principle.
3. `MockScenario::SensorDataGap`: Evaluates sensor failure recovery. Oscillates nominally but drops GPU utilization, GPU temperature, and battery metrics for 3 out of every 10 ticks (`GAP_PERIOD_TICKS = 10`, `GAP_TICKS = 3`), asserting chart break rendering without dropping series identities.

**Configuration:** Controlled via `ZENOMEN_MOCK_SCENARIO` environment variable (`nominal`, `zero`, `gap`), defaulting to `nominal` with a warning on unrecognised input.

## Domain Model Conventions

- **Single writer:** `AppState` is strictly mutated through `AppState::apply(&AppEvent)` (`src/core/state.rs`). UI callbacks never alter state directly.
- **Event-driven UI:** User actions dispatch `AppCommand`; state updates emit `AppEvent`.
- **Honest absence:** Missing or unsupported sensors are represented as `None` or empty vectors; never synthesize `0` as a placeholder (`src/hardware/provider.rs`, `src/core/state.rs`).
- **Capabilities flags:** Subsystems track capability bits (`has_cpu_metrics`, `has_fan_control`, `has_rgb_lighting`). `SUBSYSTEM_COUNT` (8) is kept synchronized with `TelemetrySnapshot::EXPECTED_SENSOR_COUNT`.
- **`FanMode` enum:** Canonical fan modes (`Auto`, `Max`, `Manual(u8)`, `Unknown(String)`). Mode string parsing is centralized in `FanMode::parse_requested` (`src/service/types.rs`).
- **Granular `PresetResult`:** Carries individual action statuses, confirmed values, and a `partial_success` flag to ensure honest user feedback.

---

*Convention analysis: 2026-11-09*
