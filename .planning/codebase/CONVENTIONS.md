---
last_mapped_commit: 761e5f975d7c22d1c48bf777a0bcaa1f2db582ba
last_mapped_at: 2026-11-07
---
# Coding Conventions

**Analysis Date:** 2026-11-07

## Naming Patterns

**Files:**
- Rust source files are `snake_case.rs`: `src/core/state.rs`, `src/hardware/linux/hwmon.rs`, `src/desktop/system_integration.rs`.
- Modules that own a subsystem use a directory plus `mod.rs` (this repo does not use `foo/foo.rs`): `src/core/mod.rs`, `src/hardware/mod.rs`, `src/service/mod.rs`, `src/rgb/mod.rs`, `src/desktop/mod.rs`, `src/config/mod.rs`, `src/hardware/linux/mod.rs`.
- Integration test suites are `tests/<area>_tests.rs` — 16 files, e.g. `tests/config_tests.rs`, `tests/hardware_service_tests.rs`, `tests/mock_hardware_tests.rs`.
- Binaries: `src/main.rs` builds `zenomen`; `src/bin/zenomen-hw.rs` builds `zenomen-hw` (`Cargo.toml` lines 40-47).

**Functions:**
- `snake_case`, verb-first, intent in the prefix:
  - Parsers take `&str` and return `Option`/`Result`: `parse_proc_meminfo`, `parse_nvidia_smi_output`, `read_cpu_model`, `read_cpu_freq_from_cpuinfo` (`src/hardware/linux/memory.rs`, `src/hardware/linux/gpu.rs`, `src/hardware/linux/cpu.rs`).
  - Scanners/discovery: `scan_hwmon_devices`, `scan_roots`, `enumerate_drm_gpus`, `detect_rgb_interface` (`src/hardware/linux/hwmon.rs`, `src/hardware/registry.rs`, `src/hardware/linux/gpu.rs`, `src/hardware/rgb.rs`).
  - Predicates: `has_cpu_metrics`, `is_degraded`, `is_connected`, `is_card_device_name` (`src/hardware/provider.rs`, `src/core/state.rs`, `src/service/client.rs`).
  - Constructors: `new`, `default`, `from_env`; builder-style options use `with_` (`with_connection`, `with_polkit_required` in `src/service/contract.rs`).
  - Conversions follow `from_*` / `as_*` / `to_*`: `FanMode::from_sysfs`, `as_sysfs_value`, `RgbColor::to_hex`, `to_slint_color`.
- Free functions are preferred for pure parsing; they never touch global paths.

**Variables:**
- `snake_case`, with units encoded in the name: `cpu_temperature_c`, `cpu_frequency_mhz`, `ram_used_mb`, `polling_interval_ms`, `vram_used_mb`, `fan_rpms` (`src/hardware/provider.rs`, `src/config/schema.rs`).
- Option-valued readings are named plainly and documented as `None` when unreadable — never defaulted to a fabricated zero: `gpu_utilization: Option<f32>`, `gpu_name: Option<String>` (`src/hardware/provider.rs`).
- Error/unavailable state uses explicit names: `last_sampling_error`, `last_control_error`, `pending_fan_mode`, `config_corrupt_notice` (`src/core/state.rs`).
- Loop variables that are intentionally unused are prefixed `_`: `for _attempt in 1..=max_retries` (`src/service/backend.rs`).

**Types:**
- `PascalCase` noun phrases: `HardwareCapabilities`, `TelemetrySnapshot`, `SensorRegistry`, `TelemetryHistory`, `MockScenario`, `PresetResult`.
- Enums use PascalCase variants and carry payloads where the failure/state is richer than a string: `FanMode::Manual(u8)`, `FanMode::Unknown(String)`, `MockScenario::ZeroCapabilities` (`src/service/types.rs`, `src/hardware/mock.rs`).
- Error enums always end in `Error`: `HardwareControlError`, `RgbError`, `SystemIntegrationError`, `ReleasesError`.
- Traits name a capability, not an implementation: `HardwareProvider` (`src/hardware/provider.rs`).
- Domain structs derive `Debug, Clone, PartialEq, Eq` where possible and add `Copy` for small enums/flags. Types holding `f32` omit `Eq` (`TelemetrySnapshot` in `src/hardware/provider.rs`).
- Generic constructors accept `impl Into<String>` or `P: Into<PathBuf>` rather than concrete `String`/`PathBuf`: `RgbProfile::new`, `HardwareBackend::new` (`src/rgb/schema.rs`, `src/service/backend.rs`).

**Constants:**
- `SCREAMING_SNAKE_CASE`, declared near the concept they bound, with a doc comment that states the unit and the reason:
  - `SAMPLING_INTERVAL`, `HISTORY_WINDOW`, `MAX_SAMPLES`, `EXPECTED_SENSOR_COUNT`, `COMMAND_QUEUE_DEPTH` (`src/core/worker.rs`, `src/core/state.rs`, `src/core/telemetry.rs`, `src/hardware/provider.rs`, `src/app.rs`).
  - Bounds are exported when callers/tests need them: `MIN_POLLING_INTERVAL_MS`, `MAX_POLLING_INTERVAL_MS`, `DEFAULT_POLLING_INTERVAL_MS` (`src/config/schema.rs`).
  - Magic numbers are extracted into named constants, e.g. `GAP_PERIOD_TICKS`, `GAP_TICKS`, `RAM_TOTAL_MB` (`src/hardware/mock.rs`).

**Test function names:**
- Mixed across the repo: 38 of 124 tests use a `test_` prefix, concentrated in older suites (`tests/desktop_integration_tests.rs` 8/8, `tests/fan_control_tests.rs` 5/5, `tests/hardware_service_tests.rs` 9/9, `tests/packaging_tests.rs` 5/5, `tests/power_preset_tests.rs` 6/6). Newer suites and all unit modules use descriptive sentences: `a_partial_sample_is_degraded_but_still_counted` (`src/core/state.rs`), `sensor_data_gap_scenario_periodically_emits_none` (`tests/mock_hardware_tests.rs`).
- Follow the descriptive style for new tests; do not rename existing tests just to normalize.
- Requirement-oriented suites use a stable numeric prefix: `degradation_01_...` through `degradation_08_...` (`tests/degradation_tests.rs`).

## Code Style

**Formatting:**
- `rustfmt` with default settings. There is no `rustfmt.toml` or `.rustfmt.toml` in the repository.
- Edition 2024 is used (`Cargo.toml` line 4), so edition-2024 idioms appear:
  - let-chains: `if hp_wmi_hwmon.is_dir() && let Ok(entries) = fs::read_dir(&hp_wmi_hwmon)` (`src/service/backend.rs:80`), also `src/main.rs:30`, `src/core/diagnostics.rs:112`.
  - `unsafe { std::env::set_var(...) }` where tests mutate the environment (`tests/logging_tests.rs:54`).
- 4-space indentation, no tabs, no trailing whitespace, LF endings.
- Default 100-column width; a few expressions that rustfmt cannot split remain slightly longer (14 lines in `src/`, 5 in `tests/`).
- Formatting is a hard CI gate: `cargo fmt --all -- --check` (`.github/workflows/ci.yml:57`; release workflow at `.github/workflows/release.yml:57`).

**Linting:**
- Clippy runs with all targets and all features, warnings denied: `cargo clippy --all-targets --all-features -- -D warnings` (`.github/workflows/ci.yml:60`; the release workflow adds `--locked`).
- `Cargo.toml` has no `[lints]` table; lints are enforced solely by the CI command line.
- There is exactly one lint suppression in `src/`, and it carries a justification comment: `#[allow(clippy::large_enum_variant)]` on `AppEvent` (`src/core/events.rs:110-113`). Follow that pattern — suppress narrowly, comment why, do not add file-level allows.

**Derives and serde:**
- Standard derive order observed: `#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]` (`src/hardware/provider.rs:27`), `#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]` (`src/config/schema.rs`).
- Config/RGB structs use `#[serde(default)]` on every newly-added optional field so older files still deserialize (`src/config/schema.rs`, `src/rgb/schema.rs`).
- Enums exposed over serde use `#[serde(rename_all = "lowercase")]` (`FanMode`, `src/service/types.rs:8`).
- Custom conversions are implemented instead of leaking wire formats: `Serialize`/`Deserialize` for `RgbColor` serialize as `#RRGGBB` strings (`src/rgb/schema.rs:94-110`).

**Feature/platform gating:**
- The GUI is an optional default feature: `#[cfg(feature = "ui")] pub mod app;` and `#[cfg(feature = "ui")] slint::include_modules!();` (`src/lib.rs:8-19`). UI-only methods are gated the same way: `#[cfg(feature = "ui")] pub fn to_slint_color` (`src/rgb/schema.rs:93`).
- Linux hardware code is isolated under `src/hardware/linux/` and selected with `#[cfg(target_os = "linux")]` / `#[cfg(not(target_os = "linux"))]` re-exports (`src/hardware/mod.rs:16-22`).
- Non-Linux builds must use `UnavailableHardwareProvider`, which reports zero capabilities and an error on sampling — never speculative sysfs reads (`src/hardware/unavailable.rs:1-34`).

## Import Organization

**Order (dominant pattern):**
1. `std::...` imports.
2. Blank line.
3. External crates, alphabetical (`serde`, `thiserror`, `slint`, `zbus`, `tracing_*`).
4. Blank line.
5. `crate::...` imports.

Examples: `src/app.rs:16-24`, `src/hardware/registry.rs:7-13`, `src/core/worker.rs:20-27`, `src/service/backend.rs:3-7`.

**Exceptions (do not copy):**
- `src/service/contract.rs:3-10` places `std::sync` after the `crate::` imports. Use the std-first order above for new code.

**Path aliases:**
- None. There are no `[workspace]` members and no path aliases; use `crate::...` for internal paths and `zenomen::...` from integration tests.

**Re-exports:**
- Each subsystem `mod.rs` declares its modules first, then re-exports the public surface with `pub use` in alphabetical order: `src/core/mod.rs:6-20`, `src/hardware/mod.rs:12-22`, `src/desktop/mod.rs:6-18`.
- Callers import from the curated boundary, not deep modules, where a boundary exists: integration tests use `zenomen::hardware::{HardwareProvider, MockHardware, MockScenario}` (`tests/mock_hardware_tests.rs:3`). Deep imports are acceptable for parsers that are deliberately testable, e.g. `zenomen::hardware::linux::cpu::{CpuSampler, read_cpu_model}` (`tests/linux_procfs_tests.rs:3`).
- Glob imports appear only as `use super::*;` inside `#[cfg(test)] mod tests` blocks.

## Error Handling

**Core rule — absence is not an error and is never a zero:**
- Unreadable sensors are `None`/empty collections, not `0`: `gpu_utilization: Option<f32>`, `fan_rpms: Vec<u32>` (`src/hardware/provider.rs:147-181`). Sysfs readers return `Option` via `.ok()?`: `read_temp_millidegrees`, `read_fan_rpm`, `read_cpu_freq_mhz` (`src/hardware/linux/hwmon.rs:67-77`, `src/hardware/linux/cpu.rs:170-174`).
- Operation failure returns `Result`; the UI-displayable reason crosses the provider boundary as `Result<TelemetrySnapshot, String>` with “a human-readable reason … it must never be a panic” (`src/hardware/provider.rs:239-241`).

**Typed errors:**
- Domain errors use `thiserror` and are `Clone + PartialEq + Eq` so they can be asserted on and moved through events: `HardwareControlError` (`src/service/types.rs:105-131`), `RgbError` (`src/rgb/schema.rs:11-29`), `SystemIntegrationError` (`src/desktop/system_integration.rs:28`), `ReleasesError` (`src/desktop/releases.rs:16`).
- `#[error("...")]` messages are user-facing sentences and include context fields.
- Conversions to external error types are explicit: `impl From<HardwareControlError> for zbus::fdo::Error` (`src/service/types.rs:133-145`).
- Filesystem config APIs use `std::io::Result<()>` where no domain error is needed: `save_atomic` (`src/config/mod.rs:85`).

**Context preservation:**
- Add context with `map_err`, including the path: `fs::write(path, ...).map_err(|e| HardwareControlError::IoError(format!("Write to {} failed: {e}", path.display())))` (`src/service/backend.rs:22-24`).
- Use `?` for propagation and `let ... else` for graceful early exits: `let Ok(entries) = std::fs::read_dir(hwmon_root) else { return devices; }` (`src/hardware/linux/hwmon.rs:36-38`).

**Panics and poisoning:**
- Library/runtime code does not `panic!`, `unwrap()`, or `expect()`. The only `panic!` calls in `src/` are inside `#[cfg(test)] mod tests` (`src/core/worker.rs:489,548,573`).
- Startup thread spawns are the sole runtime `expect`s and carry messages: `"spawning desktop action listener must succeed"` (`src/main.rs:168`), `"spawning event pump must succeed"` (`src/main.rs:254`).
- Mutex poisoning is recovered rather than propagated: `.lock().unwrap_or_else(|p| p.into_inner())` (`src/app.rs:88`, `src/app.rs:376-380`).
- Failures degrade: `load_or_default` backs up a corrupt config and returns defaults plus a notice (`src/config/mod.rs:34-78`); `logging::init` falls back to stderr-only (`src/logging.rs:76-102`); unsupported hardware yields `UnavailableHardwareProvider` (`src/hardware/unavailable.rs`).

## Logging

**Framework:** `tracing` + `tracing-subscriber` with `EnvFilter`; stderr layer plus a daily rotating file layer (`src/logging.rs`). `RUST_LOG` selects the filter; default level is `info`.

**Patterns:**
- Structured fields, not string interpolation: `tracing::warn!(path = %path.display(), error = %e, "could not read config file; using defaults")` (`src/config/mod.rs:42`), `tracing::info!(path = %path.display(), "configuration atomically saved")` (`src/config/mod.rs:114`).
- Message text is lowercase, imperative/declarative, and has no trailing period: `"monitoring worker started"`, `"ignored unknown mock scenario"` (`src/core/worker.rs:52`, `src/hardware/mock.rs:81`).
- `%` for `Display` values (paths), `?` for `Debug` values (e.g. scenario lists).
- Log-and-degrade on recoverable failure; `error!` only when a fallback action itself failed (backup copy failure, `src/config/mod.rs:60`).
- `println!`/`eprintln!` are confined to CLI entry points (`src/main.rs`, `src/bin/zenomen-hw.rs`) and the logging fallback warnings in `src/logging.rs:43-63`.
- Never log secrets, environment dumps, or raw sensor file contents.

## Comments and Documentation

**Module docs (`//!`):**
- Every module starts with a purpose statement plus the design constraint it encodes. Examples: `src/hardware/provider.rs:1-17` (why absence is `None`), `src/core/state.rs:1-10` (single source of truth), `src/app.rs:1-14` (two directions only), `src/hardware/unavailable.rs:1-11` (do not add speculative reads).
- Requirement IDs are cited in parentheses so intent stays traceable: `(FOUND-04)`, `(FOUND-05)`, `(MON-04)`, `(MON-11)`, `(SYS-06)`, `(RGB-02)`, `(PERF-02)`. Examples: `src/hardware/provider.rs:24-25`, `src/core/state.rs:25-27`, `src/rgb/schema.rs:4-6`.
- Module docs state *why*, including rejected alternatives and failure contracts: `src/logging.rs:1-8`, `src/hardware/mock.rs:3-19`.

**Item docs (`///`):**
- Public structs, fields, enums, variants, traits, and functions are documented. Field docs state units and `None` semantics: `src/hardware/provider.rs:147-181`, `src/core/state.rs:29-77`.
- Intra-doc links are used for related items: `[`AppState::apply`]`, `[`TelemetrySnapshot::reported_sensor_count`]` (`src/core/state.rs:5`, `src/hardware/provider.rs:129`).
- Doctest fences are not used for executable examples; the only fence is a `text` block documenting env-var launches (`src/hardware/mock.rs:13-16`).

**Inline comments:**
- Explain rationale, ordering, safety, and trade-offs rather than restating the code:
  - `// SAFETY: setting LC_CTYPE before starting any worker threads...` (`src/main.rs:34`).
  - `// 1. Direct path check...` / `// 2. Fallback: ...` for discovery order (`src/service/backend.rs:78-91`).
  - `// Relaxed is sufficient: the counter only has to be unique...` (`src/hardware/mock.rs:135-136`).
- Comments that describe a past state or a plan (“Phase 1 has no detector yet”, `src/hardware/mock.rs:3`) should not be added for new work; describe the current behavior.

## Function and API Design

**Small, injected, testable:**
- IO roots and external program paths are constructor parameters, never hardcoded inside logic: `HardwareBackend::new(root)` (`src/service/backend.rs:62-68`), `SensorRegistry::scan_roots(hwmon_root, acpi_root)` (`src/hardware/registry.rs:52`), `NvidiaSampler::new(program)` (`src/hardware/linux/gpu.rs:208`), `set_autostart_enabled_at` (`src/desktop/autostart.rs`).
- Production wrappers delegate to injectable `*_from_roots` / `*_at` functions: `collect_diagnostics` → `collect_diagnostics_from_roots` (`src/core/diagnostics.rs:59-81`).
- Parsers accept `&str` so they can be unit-tested with fixture text; filesystem readers accept `&Path`.
- Prefer returning owned plain data (`TelemetrySnapshot`, `DiagnosticsReport`) over borrowed views across threads/modules.
- Public functions that can fail return `Result`; functions that answer “is it there?” return `bool`/`Option`.

**Threading:**
- No async runtime in the product (no Tokio; `Cargo.toml` has no async dependency beyond zbus). Background work uses `std::thread`, named threads (`thread::Builder::new().name("zenomen-monitor-worker".to_string())`, `src/core/worker.rs:42-45`), and `std::sync::mpsc` channels.
- The zbus interface methods (`src/service/contract.rs`) are the only `async fn`s; callers use zbus’s blocking API outside them.
- Shared mutable state uses `Arc<Mutex<_>>` / atomics, with a documented reason for the ordering (`Ordering::Relaxed` for the mock tick, `src/hardware/mock.rs:133-139`).

## Module and Architecture Conventions

- **Layer boundaries are enforced by imports:** `src/core/` never touches hardware directly; `src/app.rs` owns the only Slint bridge; `src/hardware/` owns sensors, sysfs, and external tools (`src/core/mod.rs:1-6`, `src/hardware/mod.rs:1-10`).
- **Single writer for UI state:** `AppState` is mutated only through `AppState::apply(&AppEvent)` (`src/core/state.rs:107-109`). Add new state via a new `AppEvent` variant + `apply` arm, not by assigning fields from callers.
- **Message types:** UI → core is `AppCommand`, core → UI is `AppEvent` (`src/core/events.rs:69-153`). Variants carry domain types, never sysfs paths or shell commands.
- **UI callbacks are non-blocking:** they enqueue an `AppCommand` through the bounded channel (`COMMAND_QUEUE_DEPTH = 8`) and drop/log if full (`src/app.rs:1-14`, `src/app.rs:30`, `send_command`).
- **Capabilities are per-subsystem booleans,** not one aggregate flag (`HardwareCapabilities`, `src/hardware/provider.rs:21-47`). Add a new `has_*` accessor and update `reported_sensor_count`/`SUBSYSTEM_COUNT` (`src/core/state.rs:23`) and tests in the same change.
- **Platform separation:** only `src/hardware/linux/**` may read `/sys` or `/proc`; the non-Linux provider stays honest with zero capabilities.
- **Dependency discipline:** no new frameworks or async runtimes; the RSS budget (≤40 MB) is a design constraint encoded in `Cargo.toml` comments (`Cargo.toml:17-24,52-68`) and AGENTS.md rule 4.

## Documentation Synchronization

- When interfaces, test counts, or build requirements change, update `README.md` and the relevant docstrings in the same change (AGENTS.md rule 10).
- Current drift to be aware of: `README.md:226` states “123 automated … tests”; the source currently contains 124 `#[test]` functions. `docs/PARITY.md:83` states 109. Update these counts when touching the test suite rather than propagating the old number.

---

*Convention analysis: 2026-11-07*
