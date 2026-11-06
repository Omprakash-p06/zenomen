---
last_mapped_commit: 30d62de2cdf6f507822944e5602bdd907539ce38
last_mapped_at: 2026-11-06
---
# Coding Conventions

**Analysis Date:** 2026-11-06

## Scope and Ground Truth

This document describes the **Rust + Slint implementation** in `src/` and `ui/`.

`AGENTS.md` still embeds a conventions block for the retired Python `openomen`
CLI (extensionless script, snake_case module functions, no type hints). That
code no longer exists in the tree. **Do not follow the Python section** — the
rules below come from the actual Rust and Slint sources.

Supporting configuration:

- `Cargo.toml` — edition 2024, no `[lints]` table, no `[dev-dependencies]`
- `.github/workflows/ci.yml` — the enforced style gates
- No `rustfmt.toml`, `.rustfmt.toml`, `clippy.toml`, `deny.toml`,
  `.editorconfig`, `rust-toolchain.toml`, or `.cargo/config.toml` exist.
  **Tool defaults are the project's settings.**

## Naming Patterns

**Rust files:**
- `snake_case`, one concept per file: `src/hardware/linux/hwmon.rs`,
  `src/core/telemetry.rs`, `src/rgb/migration.rs`
- A directory always has `mod.rs` that declares submodules and re-exports the
  public surface: `src/core/mod.rs`, `src/service/mod.rs`,
  `src/hardware/mod.rs`, `src/desktop/mod.rs`
- Binaries: `src/main.rs` (the `zenomen` GUI) and `src/bin/zenomen-hw.rs` (the
  privileged daemon). Never add another top-level `main` outside these two.
- Integration tests: `tests/<area>_tests.rs` (see `TESTING.md`)

**Slint files:**
- Lowercase, role-based names: `ui/screens/cooling.slint`,
  `ui/components/section-card.slint`, `ui/theme/colors.slint`
- One exported component per screen/component file
- Theme token files live only in `ui/theme/`

**Types:**
- `PascalCase`, noun-first, no prefixes: `HardwareCapabilities`,
  `TelemetrySnapshot`, `SensorRegistry`, `TelemetryHistory`, `PresetResult`
- Enums use bare variant names, not type-prefixed ones:
  `GpuVendor::Amd`, `MockScenario::ZeroCapabilities`, `ScreenId::Overview`
- Error enums end in `Error`: `HardwareControlError` (`src/service/types.rs:107`),
  `RgbError` (`src/rgb/schema.rs:13`), `SystemIntegrationError`
  (`src/desktop/system_integration.rs:29`), `ReleasesError`
  (`src/desktop/releases.rs:17`)
- Traits are nouns/agents: `HardwareProvider` (`src/hardware/provider.rs:232`)

**Functions:**
- `snake_case`, verb-first: `sample_telemetry`, `read_cpu_temp`,
  `write_and_verify`, `apply_preset`, `collect_diagnostics`, `load_or_default`,
  `save_atomic`
- Constructors: `new()`; alternate construction uses `with_*`:
  `TelemetryHistory::with_capacity`, `LinuxHardwareProvider::with_all_roots`,
  `HardwareService::with_connection` (`src/core/telemetry.rs:115`,
  `src/hardware/linux/provider.rs:55`, `src/service/contract.rs:38`)
- Predicates: `has_*` / `is_*`: `has_fan_control`, `is_connected`,
  `is_full_success`, `is_off`, `has_any_reading`
- Conversions: `from_*` / `to_*`: `FanMode::from_sysfs`,
  `RgbColor::from_hex`, `RgbColor::to_slint_color`,
  `TelemetryPoint::from_snapshot`, `ScreenId::from_index`
- Test functions: newer suites use descriptive snake_case sentences
  (`history_evicts_oldest_beyond_max_samples`); older suites keep a `test_`
  prefix (`test_fan_mode_state_transitions`). The degradation matrix uses a
  numbered prefix: `degradation_01_...` (`tests/degradation_tests.rs:23`).
  **For new tests, prefer the descriptive form without `test_`.**

**Variables:**
- `snake_case`, unit suffixes are part of the name:
  `cpu_temperature_c`, `ram_total_mb`, `polling_interval_ms`, `rss_kb`,
  `cpu_frequency_mhz`, `initial_delay`
- Established abbreviations only: `cpu`, `gpu`, `ram`, `rpm`, `rgb`, `hwmon`,
  `sysfs`, `procfs`, `dmi`, `drm`, `polkit`, `ui`, `ms`, `mb`, `kb`
- Short names are allowed for loop/closure locals: `ui`, `e`, `err`, `res`,
  `guard`, `snap`
- Prefix intentionally unused bindings with `_`: `_session_conn`,
  `_per_core`, `_attempt` (`src/main.rs:73`, `src/hardware/linux/provider.rs:127`)

**Constants:**
- Module-level `SCREAMING_SNAKE_CASE`: `MAX_SAMPLES`, `COMMAND_QUEUE_DEPTH`,
  `SAMPLING_INTERVAL`, `POLKIT_ACTION_CONTROL`, `HISTORY_WINDOW`, `USER_HZ`
- Public API constants are associated consts:
  `TelemetrySnapshot::EXPECTED_SENSOR_COUNT` (`src/hardware/provider.rs:188`),
  `ScreenId::ALL` (`src/core/events.rs:27`), `MockScenario::ALL`
  (`src/hardware/mock.rs:56`)
- Numeric literals use `_` separators: `16_384`, `3_200`, `2_450`, `0x1002`,
  `0o755`

**Slint identifiers:**
- Components `PascalCase`: `AppWindow`, `SectionCard`, `UnavailablePanel`,
  `StatusPill`
- Globals: `ThemeColors`, `ThemeTypography`, `ThemeMetrics`
- Properties and callbacks are **kebab-case**: `active-screen`,
  `has-fan-control`, `polling-interval-ms`, `rgb-profile-desc`,
  `set-fan-mode`, `open-releases-page`, `dismiss-control-error`
  (`ui/app.slint:44`, `ui/app.slint:137-149`)
- Read-only inputs use `in`, two-way use `in-out`, derived state uses
  `private property` (`ui/app.slint:151-192`)

## Code Style

**Formatting:**
- `cargo fmt` with **default rustfmt settings**; CI runs
  `cargo fmt --all -- --check` (`.github/workflows/ci.yml`)
- 4-space indentation, LF newlines, UTF-8, 100-column default width
- Trailing commas in multi-line calls/structs/arrays
- Imports are alphabetized within a group by default rustfmt

**Linting:**
- CI runs `cargo clippy --all-targets --all-features -- -D warnings`; warnings
  are errors. Never leave a clippy warning in a PR.
- There is exactly **one** lint suppression in the crate:
  `#[allow(clippy::large_enum_variant)]` on `AppEvent`
  (`src/core/events.rs:113`), accompanied by a comment explaining the
  allocation trade-off. Any new allow needs the same justification inline.

**Edition 2024 idioms used deliberately:**
- Let-chains instead of nested `if let`:
  `src/main.rs:22-27`, `src/service/backend.rs:80-88`,
  `src/hardware/registry.rs:109-112`, `src/core/worker.rs:171-175`
- `is_multiple_of` instead of `% n == 0`: `src/core/worker.rs:301`
- `unsafe` is required and used for `std::env::set_var` (`src/main.rs:29`) and
  the `geteuid` FFI declaration (`src/desktop/system_integration.rs:44-48`).
  Every `unsafe` block carries a `// SAFETY:` comment.

**Control-flow style:**
- Prefer `let ... else` for early returns:
  `src/core/telemetry.rs:226-231`, `src/core/diagnostics.rs:163-165`,
  `src/hardware/registry.rs:109-113`
- Prefer `Option` combinators (`?`, `.ok()?`, `.map`, `.and_then`,
  `.unwrap_or_default`) in parsers:
  `src/hardware/linux/hwmon.rs:67-86`, `src/hardware/linux/gpu.rs:117-120`
- Use saturating arithmetic for hardware counters and deltas:
  `src/hardware/linux/cpu.rs:65-97`, `src/core/telemetry.rs:220`
- Avoid `else { if ... }`; use `match` or let-chains

**Derives (observed order):**
`Debug, Clone, Copy, PartialEq, Eq, Default` for plain value types;
`Debug, Clone, PartialEq, Eq, Serialize, Deserialize` for config/D-Bus types;
`Debug, Error` (thiserror) for error enums. Put `Default` last.

## Import Organization

**Order (blank line between groups):**
1. `std::...`
2. External crates: `serde::`, `slint::`, `thiserror::`, `tracing::`, `zbus::`
3. `crate::...` (library) or `zenomen::...` (binaries/tests)
4. In `#[cfg(test)] mod tests`: `use super::*;` first, then local `use`

Example: `src/app.rs:16-24`, `src/main.rs:6-17`,
`tests/rgb_tests.rs:11-18`.

**Path rules:**
- Inside the library always use `crate::` paths — never `zenomen::`
- Binaries and integration tests use the public crate name `zenomen::`
  (`src/main.rs:11-13`, `tests/hardware_registry…` style imports)
- Import from the module root where possible (`zenomen::hardware::HardwareProvider`)
  rather than reaching into implementation modules, unless testing internals
- Glob imports only as `use super::*;` inside inline test modules
- Re-export the public API from each `mod.rs` (`src/core/mod.rs:15-23`,
  `src/service/mod.rs`, `src/rgb/mod.rs`)

## Error Handling

**Three tiers — pick the lowest one that works:**

1. **Domain errors** — `thiserror` enums with a message per variant:
   - `HardwareControlError` (`src/service/types.rs:106-131`) covering
     `IoError`, `VerificationFailed { target, actual, retries }`,
     `UnsupportedMode`, `PolkitDenied`, `InvalidParameter`, `InterfaceMissing`
   - `RgbError` (`src/rgb/schema.rs:12-28`)
   - `SystemIntegrationError` (`src/desktop/system_integration.rs:28-38`)
   - `ReleasesError` (`src/desktop/releases.rs:16-20`)
   - Convert to D-Bus with an explicit `impl From<...> for zbus::fdo::Error`
     (`src/service/types.rs:133-145`). Add the mapping when adding a variant.

2. **Message-only boundaries** — `Result<T, String>` where the caller only
   displays the text:
   - `HardwareProvider::sample_telemetry` (`src/hardware/provider.rs:238`)
   - `HardwareServiceClient` methods (`src/service/client.rs`)
   - `read_memory_snapshot` (`src/hardware/linux/memory.rs:68-73`)
   - `export_diagnostics_to_file` (`src/core/diagnostics.rs:145-160`)

3. **Absence** — `Option<T>` for a sensor that was not read. This is the
   project's core value encoded in the type system: `None`, never `0` or `0.0`
   (`src/hardware/provider.rs:9-17`, `src/hardware/linux/hwmon.rs:67-77`).

**Rules:**
- Never `unwrap()` on filesystem, D-Bus, or parse results in library code
- Mutex poisoning is tolerated, not propagated:
  `.unwrap_or_else(|p| p.into_inner())` (`src/app.rs:376-380`,
  `src/service/client.rs:60`)
- `expect()` is reserved for programmer invariants where continuing is
  impossible: thread spawn (`src/main.rs:165`, `src/main.rs:229`,
  `src/core/worker.rs:326`), Slint compilation (`build.rs:10`)
- Errors shown to users are actionable and name the next step. The canonical
  example is the offline-daemon guidance in `src/service/client.rs:232-249` —
  change the message there, not at every call site.
- Validation happens at the point of use, before any write:
  `set_power_profile` rejects newlines/slashes/NUL and unadvertised profiles
  (`src/service/backend.rs:234-258`); `RgbColor::from_hex` rejects malformed
  hex (`src/rgb/schema.rs:47-75`).
- `anyhow` is declared in `Cargo.toml` but **unused**. Do not introduce it
  without a project-level decision; the established pattern is `thiserror` +
  `String` boundaries.

**Write verification is mandatory** for anything that mutates hardware:
`write_and_verify` writes, sleeps, reads back, retries with exponential
backoff, then returns `VerificationFailed`
(`src/service/backend.rs:12-48`). Never add a direct `fs::write` to a sysfs
control file without a read-back.

## Logging

- Use the `tracing` macros exclusively: `tracing::info!`, `warn!`, `error!`,
  `debug!`
- `println!` appears only for CLI-facing output of the system-integration flags
  (`src/main.rs:38`, `src/main.rs:51`) and the machine-readable benchmark line
  (`src/core/benchmark.rs:247`). Do not add `println!` to library code.
- Structured fields first, message second; message is a lowercase sentence with
  no trailing period:
  ```rust
  tracing::warn!(path = %path.display(), error = %e, "could not read config file; using defaults");
  tracing::info!(version = env!("CARGO_PKG_VERSION"), "starting ZenOmen");
  ```
- `%value` for `Display`, `?value` for `Debug`, bare `field` for scalars
  (`src/app.rs:351-367`, `src/core/worker.rs:223`)
- Level meanings:
  - `error!` — the user-visible operation failed and needs attention
  - `warn!` — degraded but continuing (fallback config, missing hardware,
    dropped command)
  - `info!` — lifecycle changes (start, stop, migration report, config saved)
  - `debug!` — per-tick internals and queue behaviour. **Never log per tick at
    info** (1 Hz spam).
- Subscriber setup: `tracing_subscriber::fmt::init()` in the GUI
  (`src/main.rs:89`), `EnvFilter` defaulting to `info` in the daemon
  (`src/bin/zenomen-hw.rs:12-16`). `RUST_LOG` is the user knob.

## Comments and Documentation

- **Every module starts with `//!`** — a topic sentence plus the reason the
  module exists: `src/hardware/provider.rs:1-17`,
  `src/core/worker.rs:1-16`, `src/core/telemetry.rs:1-16`
- **Every public item has `///`** explaining *why* and any constraint, not a
  restatement of the name:
  ```rust
  /// `pop_front()` runs *before* `push_back()`, so the deque never holds more
  /// than `capacity` points even for an instant (T-01-05).
  ```
  (`src/core/telemetry.rs:139-142`)
- Requirement/threat IDs are cited inline and should be preserved when editing:
  `FOUND-05`, `MON-04`, `MON-11`, `RGB-06`, `SYS-07`, `PERF-03`, `T-01-03`,
  `T-01-04`, `WR-01`. Examples: `src/app.rs:7-11`, `src/core/events.rs:12-14`,
  `src/service/backend.rs:9-11`
- Inline `//` comments explain hardware quirks and ordering constraints:
  numbered discovery steps (`src/service/backend.rs:78-91`), the CPU sensor
  priority matrix (`src/hardware/registry.rs:46-49`), and the deadline loop
  (`src/core/worker.rs:83-99`)
- `// SAFETY:` is required above every `unsafe` block
- Slint files use `//` for the component contract (what the component may and
  may not do) and `///` for properties that map to Rust setters
  (`ui/app.slint:43`, `ui/app.slint:124-149`)
- Do **not** write JSDoc-style `Args:` / `Returns:` blocks. Prose paragraphs
  and lists are the style.

## Function Design

- Small and single-purpose. Separate pure parsing from I/O:
  `parse_proc_meminfo` vs `read_memory_snapshot`
  (`src/hardware/linux/memory.rs:30` and `:68`);
  `parse_nvidia_smi_output` vs `sample_nvidia_smi`
  (`src/hardware/linux/gpu.rs`)
- Testability comes from **path injection**, not traits-for-everything:
  `SensorRegistry::scan_roots(hwmon_root, acpi_root)`,
  `LinuxHardwareProvider::with_all_roots(...)`,
  `HardwareBackend::new(root)`,
  `collect_diagnostics_from_roots(...)`,
  `open_releases_page_with(launcher)`,
  `install_to_prefix(prefix)` (`src/hardware/registry.rs:42`,
  `src/service/backend.rs:64`, `src/core/diagnostics.rs:74`,
  `src/desktop/releases.rs:26`, `src/desktop/system_integration.rs`)
- Typical arity is 0–3. Above that, prefer a struct or document the parameter
  order (see `with_all_roots`).
- Return owned data and let the caller decide presentation. Core functions do
  not print; only `src/main.rs` and `src/core/benchmark.rs` print.
- Generic parameters use inline bounds:
  `pub fn new<P: Into<PathBuf>>(root: P)` (`src/service/backend.rs:64`),
  `label: impl Into<String>` (`src/app.rs:276`)
- Higher-order helpers take closures for event mapping:
  `execute_mutating_action(name, sender, f, event_builder)`
  (`src/core/worker.rs:350-380`)
- Series selection uses `fn` pointers, not boxed closures:
  `fn path(&self, pick: fn(&TelemetryPoint) -> Option<f32>, ...)`
  (`src/core/telemetry.rs:180`)

## Module Design

**Layer boundaries (do not invert):**
- `src/hardware/**` — the only code allowed to read `/sys`, `/proc`, or spawn
  `nvidia-smi`. Exposes plain data through `HardwareProvider`.
- `src/core/**` — state, events, telemetry math, diagnostics. Touches no
  hardware and no Slint.
- `src/app.rs` — the single Slint bridge. Turns callbacks into `AppCommand`s
  and `AppEvent`s into UI properties.
- `src/service/**` — privileged D-Bus daemon, verified writes, polkit. The GUI
  reaches it only through `HardwareServiceClient`.
- `src/main.rs` — orchestration only: config, single instance, tray, worker
  spawn, event loop, teardown.

**Exports:**
- Re-export the intended public surface from `mod.rs`; submodules stay `pub` so
  integration tests can reach internals.
- There is currently **no** `pub(crate)` or `pub(super)` usage. When adding
  helpers that are not part of the public API, prefer `pub(crate)` over
  widening the surface.
- `src/lib.rs` declares the modules and ends with `slint::include_modules!();`
  — generated `AppWindow` is available as `crate::AppWindow` /
  `zenomen::AppWindow`.

**File size:**
- Most files are under 300 lines. The two large files are `src/app.rs` (545)
  and `src/service/backend.rs` (502). A new file approaching 300 lines is a
  signal to split by responsibility (e.g. schema vs migration in `src/rgb/`).

## Slint Conventions

- **No literal colors or spacing in screens.** Reference the token globals:
  `ThemeColors.*`, `ThemeMetrics.*`, `ThemeTypography.*`
  (`ui/theme/colors.slint:1-6`, `ui/theme/metrics.slint`, `ui/theme/typography.slint`)
- Component inputs are `in property`; two-way bindings are `in-out property`.
  Do not expose raw sensor values — the core sends preformatted strings
  (`ui/app.slint:72-98`, `src/app.rs:521-539`)
- Callbacks are re-emitted upward; a component never talks to the core:
  ```slint
  set-fan-mode(mode) => { root.set-fan-mode(mode); }
  ```
  (`ui/app.slint:310-312`)
- The router keys on `current-screen` with an explicit fallback to Overview so
  a bad index cannot blank the window (`ui/app.slint:191-192`)
- Containers that accept children use `@children`
  (`ui/components/section-card.slint:32`)
- Conditional UI uses `if condition : Component { ... }` — no visibility hacks
- Animations are short and eased: `animate x { duration: 150ms; easing:
  ease-out; }` (`ui/screens/settings.slint:24-34`)
- Import `std-widgets.slint` sparingly; the shell imports only `ScrollView`
  (`ui/app.slint:17`)
- New screens must be instantiated in `ui/app.slint` so the compiler
  type-checks them, and wired to properties in `src/app.rs` (`apply_to_ui`)

## Git and Tooling

- Commit messages follow Conventional Commits with a scope:
  `feat(05-03): ...`, `fix(service): ...`, `fix(ui, concerns): ...`,
  `docs(codebase): ...`, `ci: ...` (`git log`)
- `Cargo.lock` is committed on purpose (application, not library) so
  `cargo audit` audits the shipped graph — see `.gitignore:5-8`
- Comments in `Cargo.toml` explain dependency and profile choices
  (`Cargo.toml:11-18`, `:37-44`). Keep that habit.
- Before committing, the project expects all four gates to pass: `cargo fmt
  --all -- --check`, `cargo clippy --all-targets --all-features -- -D warnings`,
  `xvfb-run -a cargo test --all`, `cargo audit`
  (`.github/workflows/ci.yml`, `README.md:274-296`)

---

*Convention analysis: 2026-11-06*
