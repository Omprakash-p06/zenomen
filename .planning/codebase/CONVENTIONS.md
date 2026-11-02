---
last_mapped_commit: e50af8f9ea61300a1c10d36dc6954e319da7b6e2
last_mapped_at: 2026-11-02
---
# Coding Conventions

**Analysis Date:** 2026-11-02

This document describes the conventions of the **Rust + Slint** codebase as it exists on disk. Sources of truth: `src/**/*.rs`, `ui/**/*.slint`, `tests/**/*.rs`, `build.rs`, `Cargo.toml`, `.github/workflows/ci.yml`, `.github/workflows/release.yml`.

## Naming Patterns

**Rust source files:**
- `snake_case.rs`, one module per file: `src/core/telemetry.rs`, `src/hardware/linux/hwmon.rs`, `src/desktop/single_instance.rs`.
- Directories are modules with `mod.rs`: `src/core/mod.rs`, `src/config/mod.rs`, `src/service/mod.rs`, `src/hardware/linux/mod.rs`.
- Two binaries: `src/main.rs` (GUI shell) and `src/bin/zenomen-hw.rs` (privileged D-Bus daemon). The library is `src/lib.rs`.
- Integration test files are flat in `tests/` and suffixed `_tests.rs`: `tests/sensor_registry_tests.rs`, `tests/hardware_service_tests.rs`.

**Slint files (kebab-case, no `_`):**
- Components: `ui/components/metric-card.slint`, `ui/components/unavailable-panel.slint`, `ui/components/status-strip.slint`.
- Screens: `ui/screens/overview.slint`, `ui/screens/cooling.slint`, `ui/screens/lighting.slint`.
- Theme tokens: `ui/theme/colors.slint`, `ui/theme/typography.slint`, `ui/theme/metrics.slint`.
- Exported component names are `PascalCase` and match the file in kebab form: `MetricCard`, `UnavailablePanel`, `AppWindow`.
- Slint properties and callbacks are `kebab-case`: `active-screen`, `has-cpu-metrics`, `status-text`, `sparkline-commands`, `callback open-diagnostics()`.
- Theme globals are `PascalCase` with `Theme` prefix: `ThemeColors`, `ThemeTypography`, `ThemeMetrics` (`ui/theme/colors.slint:8`).

**Types and traits:**
- `PascalCase`, acronyms normalized to one capital + lowercase: `RgbProfile`, `RgbColor`, `RgbError`, `RgbInterfaceVariant` — **not** `RGBProfile` (`src/rgb/schema.rs:28`, `src/hardware/rgb.rs`).
- Data carriers are `*Snapshot`, `*Capabilities`, `*Report`, `*Descriptor`, `*Result`, `*Status`: `TelemetrySnapshot`, `HardwareCapabilities`, `DiagnosticsReport`, `SensorDescriptor`, `PresetResult`, `ServiceConnectionStatus`.
- Errors are `*Error` enums (`HardwareControlError`, `RgbError`, `SystemIntegrationError`) and implement `std::error::Error` through `thiserror`.
- Traits name the role, not the implementation: `HardwareProvider` (`src/hardware/provider.rs:232`). Implementations are prefixed by their nature: `MockHardware`, `LinuxHardwareProvider`.

**Functions and methods (snake_case, verb-first):**
- Constructors/parsers: `new`, `from_sysfs`, `from_hex`, `from_name`, `from_env`, `parse_stat_line`, `parse_nvidia_smi_output`.
- Accessors and predicates: `capabilities`, `sample_telemetry`, `has_fan_control`, `is_connected`, `is_degraded`, `reported_sensor_count`, `sample_count`.
- Mutators/actions: `set_fan_mode`, `apply_preset`, `save_atomic`, `install_system_integration`, `spawn_monitoring_worker`.
- Conversions: `as_str`, `as_sysfs_value`, `to_hex`, `to_slint_color`, `rss_mb`, `name`.
- **Path-injectable variants are suffixed, never configured by a global:** `_at` (`is_autostart_enabled_at`, `set_autostart_enabled_at`), `_roots` (`SensorRegistry::scan_roots`, `collect_diagnostics_from_roots`), `_from_*` (`collect_diagnostics_from_roots`, `read_cpu_freq_from_cpuinfo`, `parse_nvidia_smi_output`), `_to_prefix`/`_from_prefix` (`install_to_prefix`, `uninstall_from_prefix`).
- Free helpers in `src/hardware/linux/*` are plain `read_*`/`parse_*` functions with no `self`, and are the testable seam for parsers (`src/hardware/linux/cpu.rs:156`, `src/hardware/linux/gpu.rs:175`).

**Fields and locals:**
- `snake_case`; units are part of the name: `cpu_temperature_c`, `cpu_frequency_mhz`, `ram_total_mb`, `rss_kb`, `max_retries`, `initial_delay`.
- Collections are plural (`fan_rpms`, `power_profiles`, `discovered_devices`); optional readings use `Option` and the name says so implicitly (`gpu_name: Option<String>`).

**Constants:**
- `SCREAMING_SNAKE_CASE` with unit or meaning encoded: `MIN_POLLING_INTERVAL_MS`, `MAX_POLLING_INTERVAL_MS`, `DEFAULT_POLLING_INTERVAL_MS` (`src/config/schema.rs:10`), `HISTORY_WINDOW` (`src/core/state.rs:23`), `MAX_SAMPLES`, `GAP_PERIOD_TICKS`, `SAMPLING_INTERVAL`, `EXPECTED_SENSOR_COUNT`.
- Path constants are absolute strings named `*_TARGET`: `DESKTOP_ENTRY_TARGET`, `DBUS_CONF_TARGET`, `POLKIT_POLICY_TARGET` (`src/desktop/system_integration.rs:13`).
- Embedded assets use `EMBEDDED_*` plus `include_str!` (`src/desktop/system_integration.rs:21`).

**Test names:**
- `snake_case`, descriptive assertion-style sentences: `missing_sensors_return_none_without_synthesizing_zero`, `history_strictly_bounds_samples_to_120_on_overflow`, `degradation_07_unauthorized_fan_write_preserves_hardware_state`.
- Some older suites retain a `test_` prefix (`tests/desktop_integration_tests.rs`, `tests/packaging_tests.rs`, `tests/hardware_service_tests.rs`). Match the file you are editing; new test files should use the descriptive sentence form without `test_` (the majority and the newer style).
- Numbered degradation scenarios: `degradation_01_...` through `degradation_08_...` (`tests/degradation_tests.rs`).

## Code Style

**Formatting:**
- `rustfmt` with **default settings** — there is no `rustfmt.toml` or `.rustfmt.toml`. 4-space indent, 100-column default, trailing commas, one import per line where rustfmt decides.
- CI enforces it: `cargo fmt --all -- --check` (`.github/workflows/ci.yml`, `.github/workflows/release.yml`). Run `cargo fmt --all` before committing.
- Blank line between the import block and the first item; module docs (`//!`) come first, before imports.

**Import organization (rustfmt default ordering):**
1. `std::*` imports (`use std::fs;`, `use std::sync::Arc;`)
2. blank line
3. external crates (`use serde::{Deserialize, Serialize};`, `use slint::{...};`, `use thiserror::Error;`)
4. blank line
5. crate-local imports (`use crate::core::events::{AppCommand, AppEvent};`, `use crate::hardware::HardwareProvider;`)

Example: `src/core/worker.rs:17`–`24`, `src/app.rs:18`–`26`. Binary entry points import the library as `zenomen::...` (`src/main.rs:9`–`16`), never as `crate::`.

**Edition-2024 idioms in use:**
- `let ... else` for early exit: `let Some(ui) = ui.upgrade() else { ... };` (`src/app.rs:344`), `let Ok(content) = fs::read_to_string(...) else { ... };` (`src/core/diagnostics.rs:163`).
- Let-chains: `if hp_wmi_hwmon.is_dir() && let Ok(entries) = fs::read_dir(&hp_wmi_hwmon) { ... }` (`src/service/backend.rs:81`).
- Inline format args: `format!("Write to {} failed: {e}", path.display())`, not positional `{0}` where avoidable.
- `Self { .. }` in constructors; `impl Default` written explicitly rather than always derived when defaults carry meaning (e.g. `HardwareCapabilities::default() == none()`, `src/hardware/provider.rs:207`).

## Linting

- Clippy is a **blocking gate with zero warnings**: `cargo clippy --all-targets --all-features -- -D warnings` (`.github/workflows/ci.yml`), and `--locked` in release (`.github/workflows/release.yml`).
- No `clippy.toml`; no `pedantic`/`nursery` groups enabled. Write code that passes default clippy without suppressions.
- Suppressions must be narrow, justified, and rare. The only one in the tree is `#[allow(clippy::large_enum_variant)]` on `AppEvent` (`src/core/events.rs:113`). If you add one, put a `//` comment above it explaining why the alternative is worse.
- No `todo!()`, `unimplemented!()`, or `#[ignore]` anywhere. Do not introduce them.

## Error Handling

The pattern depends on the boundary:

**1. Library/service boundaries → typed `thiserror` enums.**
- `HardwareControlError` (`src/service/types.rs:101`) with variants `IoError(String)`, `VerificationFailed { target, actual, retries }`, `UnsupportedMode(String)`, `PolkitDenied(String)`, `InvalidParameter(String)`, `InterfaceMissing(String)`.
- `RgbError` (`src/rgb/schema.rs:13`), `SystemIntegrationError` (`src/desktop/system_integration.rs:29`).
- Every variant carries a human-readable `#[error("...")]` message with named or positional fields. Structured data is kept in fields (e.g. `VerificationFailed`), not flattened into one string.
- Use `#[from]` for transparent wrapping: `#[error("I/O error while writing system files: {0}")] Io(#[from] std::io::Error)`.
- Cross-boundary conversion lives in `impl From<...> for ...`, e.g. `HardwareControlError -> zbus::fdo::Error` (`src/service/types.rs:132`). Do not hand-map D-Bus errors at each call site.

**2. Core ↔ UI and D-Bus client boundary → `Result<T, String>`.**
- The UI must display the reason verbatim, so the message is the payload: `Result<TelemetrySnapshot, String>` (`src/hardware/provider.rs:238`), all `HardwareServiceClient` methods (`src/service/client.rs:101`), and `AppEvent` result variants (`src/core/events.rs:132`).
- Build messages with `.map_err(|e| format!("GetFanStatus failed: {e}"))?` — prefix the operation, keep the source. Never discard the cause.

**3. Absence is `Option`, never an error and never a zero.**
- Missing sensors return `None` (individual value) or an empty `Vec`/`String` (collection): `read_cpu_temp() -> Option<f32>`, `read_active_power_profile() -> Option<String>`, `TelemetrySnapshot::gpu_utilization: Option<f32>`.
- `HardwareCapabilities::default()` is `none()` — a machine with nothing detected still launches and explains itself (`src/hardware/provider.rs:207`).
- This is a hard project rule: a sensor that is not present reads **Unavailable**, not `0`. Tests enforce it (`tests/mock_hardware_tests.rs`, `tests/degradation_tests.rs`).

**4. Binaries → `Result<(), Box<dyn Error>>`.**
- `src/main.rs:19`, `src/bin/zenomen-hw.rs:12`. `anyhow` is declared in `Cargo.toml` but **not used anywhere**; do not start using it without a project decision.

**5. No panics in library paths.**
- `unwrap()`/`expect()` in `src/` are confined to `#[cfg(test)]` modules and three thread-spawn sites in binaries (`src/main.rs:151`, `src/main.rs:200`, `src/core/worker.rs:326`), each with a `"... must succeed"` message.
- External reads use `.ok()`, `unwrap_or_default()`, or `?` with a typed error. A failed read of an optional sensor must not abort the sample.

## Unsafe Code

- `unsafe` appears in exactly one place: the `geteuid` FFI declaration and call in `src/desktop/system_integration.rs:42`–`50`.
- Policy: unsafe is acceptable only for a tiny, audited libc call with a `#[cfg(unix)]` guard and a `#[cfg(not(unix))]` fallback. Any new `unsafe` needs a `// SAFETY:` comment stating the invariant; the existing block does not yet have one — add it if you touch that function.
- No `unsafe` in tests, UI code, or hardware readers.

## Logging (`tracing`)

- Use the `tracing` macros directly at the call site — no wrapper module, no `log` crate.
- Subscriber setup: `tracing_subscriber::fmt::init()` in the GUI (`src/main.rs:75`), `EnvFilter` with `info` default in the daemon (`src/bin/zenomen-hw.rs:13`). `RUST_LOG` is the user-facing switch; never hardcode a filter.
- **Structured fields, not interpolation:** `tracing::warn!(path = %path.display(), error = %e, "could not read config file; using defaults")` (`src/config/mod.rs:42`). Use `%` for `Display` values, `?` for `Debug` values (`?dropped`, `?Self::ALL`).
- Message text is a lowercase sentence fragment describing what happened; fields carry the data. Avoid trailing periods.
- Levels:
  - `error!` — an operation the user asked for failed: export failure, config backup failure (`src/app.rs:251`, `src/config/mod.rs:60`).
  - `warn!` — degraded but recoverable: provider sampling failure, corrupt config fallback, unknown mock scenario, dropped command (`src/core/worker.rs:340`, `src/config/mod.rs:73`, `src/hardware/mock.rs:78`).
  - `info!` — lifecycle and successful mutations: startup, worker start/stop, fan mode/preset applied, config saved, profile migration report (`src/main.rs:77`, `src/core/worker.rs:223`).
  - `debug!` — per-tick internals, channel disconnects, queue drops, worker command handling (`src/core/worker.rs:106`, `src/app.rs:363`).
- Logging is for the journal only; **user-facing failures also travel as `AppEvent` error variants** and are rendered in the status strip/banner. A `tracing::error!` alone is not a user notification.
- `println!` is allowed only in binary `main` paths (`src/main.rs` install/uninstall messages) and the opt-in benchmark line gated by `ZENOMEN_BENCHMARK=1` (`src/core/benchmark.rs:242`). Never `println!` from library code or per-tick loops.

## Doc Comments

**Module headers (`//!`):**
- Every non-trivial module opens with a `//!` block explaining **why the module exists, what it deliberately does not do, and which project requirement it serves**.
- Requirement IDs are cited in parentheses: `(FOUND-05)`, `(PERF-03)`, `(MON-11)`, `(RGB-06)`, `(SYS-06)`, `(REL-03)`. Preserve this: it is how plans and tests stay traceable.
- Use `[`Type`]` intra-doc links for cross-references: `[`provider::HardwareProvider`]`, `[`AppState::apply`]`.
- Good examples: `src/app.rs:1`–`16` (the two-direction bridge and the `Weak` rationale), `src/hardware/provider.rs:1`–`17` (the no-fabricated-zero contract), `src/core/state.rs:1`–`11`.

**Item docs (`///`):**
- Every `pub` item (struct, field, enum, variant, trait method, function) gets a `///` sentence. Private helpers get one when the intent is not obvious.
- Describe the contract and the failure mode, not the syntax: "`Err` carries a human-readable reason the UI can show in the status strip; it must never be a panic." (`src/hardware/provider.rs:236`).
- Doc the units and ranges: "Aggregate CPU utilization in percent (0–100)." (`src/hardware/provider.rs:156`).
- No `# Arguments`/`# Returns` sections and no runnable doctests; this crate has none and GUI/hardware code is not doctest-friendly. Keep the prose sentence form.
- Public error enums document each variant with a `///` line (`src/service/types.rs`).

**Slint comments:**
- `//` header block at the top of each component explaining the layout source (`01-UI-SPEC.md`) and the invariant it enforces.
- `///` on properties whose semantics are not obvious: "Empty string means \"no pill\" — a nominal card carries no state chrome." (`ui/components/metric-card.slint:15`).
- Inline `//` explains non-obvious unit conversions, e.g. `letter-spacing: 0.5px; // UI-SPEC 0.04em at 12px; Slint has no em unit`.

## Representing "Unavailable" / Missing Sensors

This is the codebase's defining convention — follow it in every new feature:

- **Data layer:** `Option<T>` per reading, empty `Vec`/`String` for collections. `TelemetrySnapshot` is the contract (`src/hardware/provider.rs:146`).
- **Capability layer:** one flag per interface in `HardwareCapabilities`, defaulting to `false` (`cpu_metrics`, `gpu_metrics`, `ram_metrics`, `fan_rpm`, `fan_control`, `power_profiles`, `rgb_lighting`, `battery_metrics`). Query per subsystem; do not collapse to one boolean.
- **Counting:** `reported_sensor_count()` measures presence from model/struct fields, not from numeric values — an idle CPU at 0 % still counts as reported (`src/hardware/provider.rs:183`).
- **Service layer:** `ServiceConnectionStatus::Unavailable(String)` carries the reason (`src/service/client.rs:15`).
- **UI:** the literal string `"Unavailable"` is the default value of value-bearing properties (`ui/components/metric-card.slint:12`), and `UnavailablePanel` is the only "cannot act here" treatment (`ui/components/unavailable-panel.slint`). Screens name the exact missing interface in the reason line.
- **Never** substitute `0`, `0.0`, `"Unknown"`, or a plausible-looking value for a missing reading.

## Function and Module Design

**Functions:**
- Small and single-purpose; pure parsing helpers take `&str`/`&Path` and return `Option`/`Result`, so they are unit-testable without hardware (`src/hardware/linux/cpu.rs`, `src/hardware/linux/gpu.rs`).
- Prefer 0–2 parameters; pass a struct when more are needed (e.g. `collect_diagnostics_from_roots` takes explicit roots because tests drive it).
- Returning `()` + `tracing` is acceptable for lifecycle functions; returning `Self` chains constructors (`RgbColor::new`).
- Blocking work never runs on the UI thread: sampling lives in `src/core/worker.rs`, and anything that sleeps documents it: "NOTE: Blocks for `window` duration. MUST NEVER be called from Slint UI thread." (`src/core/benchmark.rs:225`).

**Modules:**
- `src/lib.rs` exposes the seven public modules (`app`, `config`, `core`, `desktop`, `hardware`, `rgb`, `service`) plus `slint::include_modules!()`.
- Each directory `mod.rs` re-exports its public surface so callers use short paths: `pub use provider::{HardwareCapabilities, HardwareProvider, TelemetrySnapshot};` (`src/hardware/mod.rs:18`), `pub use events::{AppCommand, AppEvent, ScreenId};` (`src/core/mod.rs:18`). Add new public types to the `mod.rs` re-export, not just the leaf file.
- Dependency direction is one-way: `ui` ← `app` ← `core` ← `hardware`; `service` is the privileged side and must not depend on Slint. `src/core` never touches hardware directly, only through the `HardwareProvider` trait (`src/core/mod.rs:1`–`6`).
- Barrel/re-export cycles are avoided by keeping `hardware/provider.rs` data-and-trait only.

**Derives and serde:**
- Data types derive `Debug, Clone` at minimum; value-like types add `PartialEq, Eq` when they contain no floats (`HardwareCapabilities`, `FanMode`, `PresetResult`); snapshot types with `f32` derive `PartialEq` but not `Eq` (`TelemetrySnapshot`).
- Serde types add `Serialize, Deserialize`; enum wire format is normalized with `#[serde(rename_all = "lowercase")]` (`src/service/types.rs:8`).
- Config structs use `#[serde(default)]` on every field plus an explicit `impl Default` so a partial TOML always loads (`src/config/schema.rs`), and call `sanitize()` on load and save (`src/config/mod.rs:48`, `src/config/mod.rs:87`).

**Slint/UI:**
- No hardware access in `.slint`; no literal colors (every color comes from `ThemeColors` — `ui/theme/colors.slint:7`); spacing/sizes come from `ThemeMetrics`; fonts from `ThemeTypography`.
- Every out-of-range screen index falls back to Overview (`current-screen` in `ui/app.slint`); a bad value must never blank the window.
- New screens go in `ui/screens/`, reusable pieces in `ui/components/`, tokens in `ui/theme/`; register the screen import in `ui/app.slint`.

## Anti-Patterns (do not introduce)

- **Fabricating a reading** (`0`, default string) when a sensor is absent — return `None` and let the UI render `Unavailable`.
- **`unwrap()`/`expect()` in non-test library code** — the only permitted sites are thread spawns in binaries with an explicit `must succeed` message.
- **`anyhow`** — it is an unused declared dependency; errors crossing boundaries are typed `thiserror` enums or `Result<_, String>` for the UI.
- **`#[allow(...)]` without a reason comment**, or widening clippy lints to get a build green.
- **`println!` outside `main`/benchmark opt-in** — use `tracing` with structured fields.
- **Hardware I/O on the UI thread** or blocking calls inside Slint callbacks — queue an `AppCommand` instead (`src/app.rs:1`–`16`).
- **New third-party crates** for small tasks — the stack is deliberately minimal (`Cargo.toml` comment on Slint features); prefer std.
- **Literal hex colors or ad-hoc spacing in `.slint` files** — add a token to `ui/theme/*.slint` instead.
- **Changing a write path without read-back verification** — `write_and_verify` (`src/service/backend.rs:12`) is the required pattern for every hardware write.

---

*Convention analysis: 2026-11-02*
