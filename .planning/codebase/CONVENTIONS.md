# Coding Conventions

**Analysis Date:** 2026-11-08

## Naming Patterns

**Files:**
- Modules use `snake_case` and one concern per file: `src/hardware/provider.rs`, `src/core/telemetry.rs`, `src/service/protocol.rs`, `src/desktop/single_instance.rs`.
- Platform code lives under a platform directory and mirrors the shared module name: `src/hardware/linux/hwmon.rs`, `src/hardware/windows/battery.rs`, `src/service/windows/server.rs`, `src/desktop/windows/tray.rs`.
- Every subsystem directory has a `mod.rs` façade (`src/core/mod.rs`, `src/hardware/mod.rs`, `src/service/mod.rs`, `src/desktop/mod.rs`, `src/rgb/mod.rs`, `src/ui/mod.rs`).
- Tests are `snake_case` with a `_tests.rs` suffix in `tests/` (e.g. `tests/hardware_service_tests.rs`); inline unit-test modules are named `tests`.

**Functions:**
- `snake_case`, verb-first and specific: `sample_telemetry`, `load_or_default`, `save_atomic`, `write_and_verify`, `collect_diagnostics_from_roots`, `migrate_openomen_profiles`.
- Predicates are `has_*` / `is_*`: `has_cpu_metrics`, `has_fan_control`, `is_degraded`, `is_connected`, `is_full_success`.
- Conversions follow `from_*` / `as_*` / `to_*`: `FanMode::from_sysfs`, `parse_requested`, `as_sysfs_value`, `as_str`, `TelemetryPoint::from_snapshot`.
- Test functions read as behavioral sentences, usually without a `test_` prefix in newer code: `a_partial_sample_is_degraded_but_still_counted` (`src/core/state.rs`), `priority_matrix_selects_k10temp_over_acpitz` (`tests/sensor_registry_tests.rs`). Older integration suites use `test_` prefixes (`tests/desktop_integration_tests.rs`, `tests/hardware_service_tests.rs`); do not rename existing tests, follow the local file style.

**Variables:**
- Plain `snake_case`, no type suffixes or Hungarian notation: `pending_fan_mode`, `history_window`, `last_control_error`, `service_status`.
- Handles and guards are named for their lifetime obligation: `_logging_guard`, `_session_conn`, `_instance_guard`, `_tray_handle` in `src/main.rs`.

**Types:**
- Types, traits and enum variants are `PascalCase`: `HardwareProvider`, `TelemetrySnapshot`, `FanMode`, `PresetResult`, `AppEvent`, `MockScenario`, `RgbProfile`, `ServiceError`.
- Domain enums serialize with explicit casing where they cross a boundary: `#[serde(rename_all = "lowercase")]` on `FanMode` (`src/service/types.rs`).
- Constants are `SCREAMING_SNAKE_CASE` with a unit/meaning in the name: `MAX_SAMPLES`, `HISTORY_WINDOW`, `EXPECTED_SENSOR_COUNT`, `CHART_WIDTH`, `PROTOCOL_VERSION`, `MAX_FRAME_BYTES`, `SUBSYSTEM_COUNT`.
- Struct fields are documented when their meaning or `None` semantics are non-obvious (`src/hardware/provider.rs`, `src/core/state.rs`).

## Code Style

**Formatting:**
- `rustfmt` defaults. There is **no** `rustfmt.toml` or `.rustfmt.toml`; rely on `cargo fmt`.
- Edition 2024 (`Cargo.toml`), which permits let-chains; the codebase uses them, e.g. `if let Ok(dir) = fs::File::open(parent_dir) && ...` in `src/config/mod.rs` and `if let Some(path) = ... && let Err(e) = ...` in `src/core/worker.rs`.
- Default line width (100 columns); code is wrapped in a narrower, readable style rather than packed to the limit.
- CI enforces formatting exactly: `cargo fmt --all -- --check` in `.github/workflows/ci.yml` and `.github/workflows/release.yml`.
- No trailing whitespace or stray files; `git diff --check` is part of the required review loop (`AGENTS.md`, `docs/testing/GEMINI_TESTING_HANDOFF.md`).

**Linting:**
- Zero clippy warnings is a merge gate: `cargo clippy --all-targets --all-features -- -D warnings` (`.github/workflows/ci.yml`).
- There is no `clippy.toml` and no `[lints]` table in `Cargo.toml`; do not add global lint suppression.
- Inline `#[allow(...)]` is rare and must carry a justifying comment. Existing examples:
  - `#[allow(clippy::large_enum_variant)]` with a comment explaining it in `src/core/events.rs`.
  - `#[allow(unreachable_code)]`, `#[allow(unused_mut)]`, `#[allow(unused_variables)]` only in platform/FFI shims (`src/core/benchmark.rs`, `src/hardware/windows/provider.rs`, `src/service/windows/security.rs`).
- `unsafe` blocks require a `// SAFETY:` justification, e.g. `src/main.rs:34` and `tests/logging_tests.rs:54`.

**Profiles:**
- `[profile.dev]` and `[profile.test]` use `debug = 0` and `incremental = false` to keep `target/` and RSS small (`Cargo.toml`).
- `[profile.release]` uses `opt-level = "s"`, `lto = true`, `codegen-units = 1`, `panic = "abort"`, `strip = true` to protect the ≤40 MB RSS budget.

## Import Organization

**Order (rustfmt default):**
1. `std::*`
2. External crates (`serde`, `slint`, `tracing`, `zbus`, `thiserror`, ...)
3. Crate-local paths (`crate::core::...`, `crate::hardware::...`)

Blank lines separate the groups, as in `src/service/protocol.rs`, `src/core/worker.rs`, and `src/app.rs`. Within a group, imports are alphabetized by rustfmt.

**Path Aliases:**
- No custom aliases. Use absolute `crate::` paths, not `super::` (except `use super::*;` inside `#[cfg(test)] mod tests`).
- Prefer re-exports through each subsystem's `mod.rs` so callers name the boundary in one hop: `use zenomen::core::{AppCommand, AppEvent, AppState};`, `use zenomen::hardware::{HardwareProvider, MockHardware};`.
- Crate-internal modules are referenced by full path at first use when it improves locality: `crate::service::types::PresetResult`, `crate::config::AppConfig`.

## Error Handling

**Strategy:** Typed errors at domain boundaries; human-readable `String` errors at the hardware/UI seam; never panic on expected failure.

**Patterns:**
- The `HardwareProvider` trait returns `Result<TelemetrySnapshot, String>`; the `Err` is a message the status strip can display and must never be a panic (`src/hardware/provider.rs`).
- Domain errors use `thiserror` with `#[error("...")]` messages:
  - `HardwareControlError` in `src/service/types.rs`
  - `ServiceError` and `ProtocolError` in `src/service/protocol.rs`
  - `RgbError` in `src/rgb/schema.rs`
  - `DesktopError` in `src/desktop/mod.rs`, `ReleasesError`, `SystemIntegrationError`
- Manual `From` impls translate between layers instead of leaking platform detail: `impl From<HardwareControlError> for zbus::fdo::Error` (`src/service/types.rs`) and `impl From<HardwareControlError> for ServiceError` (`src/service/protocol.rs`).
- Error messages are lowercase, actionable, and state the failed target and observed value (e.g. `VerificationFailed { target, actual, retries }`).
- "Fallback is a normal result" is modeled as data, not `Err`: `config::load_or_default` returns `(AppConfig, Option<String>)` so corruption still yields usable defaults plus a notice (`src/config/mod.rs`).
- I/O functions return `std::io::Result<()>`: `save_atomic`, `atomic_write_file`.
- `main` uses `Result<(), Box<dyn std::error::Error>>` (`src/main.rs`); CLI admin branches print to `stderr` and exit non-zero.
- Production code avoids `unwrap()`; use `unwrap_or_else`, `map`, `?`, or explicit fallbacks. `unwrap`/`expect` are acceptable in tests and for documented programmer invariants such as spawning named threads in `src/main.rs`.
- Recoverable degradation must not abort startup: logging falls back to stderr-only (`src/logging.rs`), hardware absence returns `None`/empty collections (`src/hardware/provider.rs`), and unsupported platforms return `UnavailableHardwareProvider` (`src/hardware/unavailable.rs`).

## Logging

**Framework:** `tracing` + `tracing-subscriber` with `EnvFilter` (`RUST_LOG`, default `info`). Initialized once in `zenomen::logging::init()` and held via `LoggingGuard` for the process lifetime (`src/logging.rs`, used at the top of `src/main.rs`).

**Patterns:**
- Use structured fields, not string interpolation: `tracing::info!(version = env!("CARGO_PKG_VERSION"), "starting ZenOmen");`, `tracing::warn!(error = %e, "fan mode change error recorded in state");`.
- Display formatting uses `%` (`error = %e`, `path = %path.display()`), debug formatting uses `?`.
- Levels:
  - `info!` for lifecycle and successful mutations (startup, provider selection, config save, clean shutdown).
  - `warn!` for recoverable degradation (missing D-Bus, corrupt config, sampling error, failed write verification).
  - `error!` for persistence failures or invariant violations (`src/config/mod.rs`, `src/core/worker.rs`).
  - `debug!` for per-tick, navigation, and visibility churn (`src/core/worker.rs`).
- `println!`/`eprintln!` are reserved for CLI admin commands and for logging setup failures before a subscriber exists (`src/main.rs`, `src/bin/zenomen-hw.rs`, `src/logging.rs`). Runtime logging is always `tracing`.
- Two sinks: stderr (`with_target(false)`) and a daily rotating file (`with_ansi(false)`, `max_log_files(7)`). The file path is platform-resolved in `log_directory()`.

## Comments

**When to Comment:**
- Explain *why* and the invariant enforced, not what the line does. Module headers (`//!`) are the primary design record.
- Module docs routinely name the requirement or threat the code satisfies: `FOUND-05`, `SYS-06`, `SYS-07`, `MON-11`, `PERF-02`, `RGB-04`, `T-01-04`, `WIN-08`. Preserve these references in new code.
- Non-obvious decisions get an adjacent rationale (e.g. why an event is large, why `rgb_profile` is excluded from sensor count, why `None` is not `0`).

**JSDoc/TSDoc:** Not applicable. Rustdoc `///` is used on public items and struct fields; module-level `//!` docs lead every non-trivial file. Documented examples include environment variables in a fenced block (`src/hardware/mock.rs`) and fallible contract notes on trait methods (`src/hardware/provider.rs`).

**Test comments:** Scenario tests number their steps (`// 1. Success event updates...`) in `tests/fan_control_tests.rs`; parser tests annotate the arithmetic (`tests/linux_procfs_tests.rs`).

## Function Design

**Size:** Small, single-purpose. Parsing, arithmetic, and SVG generation are separated from I/O so they are unit-testable without hardware: `parse_proc_meminfo`, `parse_nvidia_smi_output`, `calculate_cpu_utilization`, `generate_svg_path`, `parse_battery_status`.

**Parameters:** Prefer borrowed inputs (`&str`, `&Path`, `&AppConfig`). Root paths are injected so tests can point at temp directories: `SensorRegistry::scan_roots(hwmon_root, acpi_root)`, `HardwareBackend::new(root)`, `collect_diagnostics_from_roots(...)`, `set_autostart_enabled_at(path, bool)`.

**Return Values:** Return domain types and `Option` for "not measured" — never sentinel zeros. Use `Result` for fallible work and `bool` only for predicates.

## Module Design

**Exports:** Each `mod.rs` re-exports its public surface with `pub use` (`src/core/mod.rs`, `src/hardware/mod.rs`, `src/service/mod.rs`, `src/desktop/mod.rs`). Add new public types to the relevant façade rather than making callers reach into submodules.

**Barrel Files:** `mod.rs` acts as the barrel/façade; there are no `prelude` modules.

**Feature gating:** The GUI is the default `ui` feature (`Cargo.toml`). `src/lib.rs` gates `app`, `ui`, and `slint::include_modules!()` behind `#[cfg(feature = "ui")]` so `zenomen-hw` never links Slint.

## Domain & Architecture Conventions

**Single-writer UI state:** Mutate `AppState` only through `AppState::apply(&AppEvent)` (`src/core/state.rs`). `src/app.rs` locks the state and calls `apply`; screens read formatted values and never write state directly.

**Typed message flow:** `AppCommand` is UI → core; `AppEvent` is core → UI. UI callbacks queue commands and never sample hardware or block (`src/app.rs`, `src/core/events.rs`).

**Reuse domain types:** Use `FanMode`, `PresetResult`, `TelemetrySnapshot`, `BatteryTelemetry`, `HardwareCapabilities`, `AppEvent`, `AppCommand`, `RgbProfile` rather than introducing parallel structs (`src/service/types.rs`, `src/hardware/provider.rs`, `src/core/events.rs`, `src/rgb/schema.rs`).

**Platform separation:** Linux-only hardware code stays under `src/hardware/linux/`; Windows-only code under `src/hardware/windows/`, `src/service/windows/`, `src/desktop/windows/`. Unsupported platforms use `UnavailableHardwareProvider` with zero capabilities — never fabricate readings or guess sysfs paths (`src/hardware/unavailable.rs`).

**No async runtime:** Threads + `std::sync::mpsc` only (`src/core/worker.rs`, `src/main.rs`). Do not introduce Tokio or another async runtime.

**Honest absence:** Missing sensors are `None` or empty collections, never `0`. `TelemetrySnapshot::reported_sensor_count` and `AppState::status_text` derive the display from measured presence (`src/hardware/provider.rs`, `src/core/state.rs`).

---

*Convention analysis: 2026-11-08*
