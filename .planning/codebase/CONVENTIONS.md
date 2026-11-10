<!-- refreshed: 2026-11-11 -->
# Coding Conventions

**Analysis Date:** 2026-11-11

## Naming Patterns

**Files & modules:**
- One responsibility per module; names are singular nouns (`core/state.rs`, `service/backend.rs`, `hardware/registry.rs`).
- Platform subtrees mirror each other by filename: `hardware/{linux,windows}/provider.rs`, `desktop/windows/tray.rs`, `service/windows/client.rs`.
- `mod.rs` is used as the directory surface (no `lib.rs`-per-directory style).
- Test suites are `<area>_tests.rs` (`fan_control_tests.rs`, `power_preset_tests.rs`, `windows_service_tests.rs`).

**Rust items:**
- Functions/methods: `snake_case`, verb-first (`sample_telemetry`, `apply_preset`, `resolve_board_validation`, `authorize_mutation`).
- Types/traits/enums: `UpperCamelCase`; error enums end in `Error`; results end in `Result` (`PresetResult`, `PowerProfileResult`, `DisplayChangeResult`).
- Constants: `SCREAMING_SNAKE_CASE`, often exported as the module's contract (`PIPE_NAME`, `MAX_PAYLOAD_SIZE`, `POLKIT_ACTION_CONTROL`, `EXPECTED_SENSOR_COUNT`, `HPWMI_GM`).
- Boolean-returning methods read as predicates: `is_connected`, `has_fan_control`, `is_fully_success`, `is_readback_verified`, `requires_thermal_profile_restore`.
- Optional data is `Option<...>` or empty `Vec`/`String`; there is no sentinel `0` convention.

**Domain identifiers (wire-stable strings):**
- Fan mode: `"auto"`, `"max"`, `"manual"`.
- Power profile: `"quiet"`, `"balanced"`, `"performance"`, `"gaming"` (preset alias), `"auto"`.
- Mock scenarios: `nominal`, `zero`, `gap`.
- Process roles: `gui`, `hardware_service` (`#[serde(rename_all = "snake_case")]`).
These strings cross D-Bus, the named pipe, and `AppCommand` unchanged; do not invent synonyms.

**Slint:**
- Components `UpperCamelCase` (`AppWindow`, `StatusStrip`, `UnavailablePanel`).
- Properties and callbacks `kebab-case` (`has-fan-control`, `pending-fan-mode`, `on-apply-preset`).
- Rust setters follow Slint's generated `set_<snake_case>` / `on_<snake_case>`.

**Tests:**
- Descriptive sentence names stating the guarantee, not the method:
  - `a_second_mutation_is_rejected_while_one_is_in_flight`
  - `test_8a3d_fan_table_gpu_offset_uses_first_nonzero_gpu_row`
  - `unavailable_connection_reports_honest_absence`

## Code Style

- Default `rustfmt` (no `rustfmt.toml`) and default Clippy; CI runs `cargo fmt --all -- --check` and `cargo clippy --all-targets --all-features -- -D warnings`.
- Edition 2024 idioms are used freely: `if let ... && ...` chains (`if let Some(path) = ... && let Err(e) = ...`), `let-else`, `is_multiple_of()`, `is_some_and()`.
- Prefer `?` and explicit error mapping; avoid `unwrap()` in production paths. Existing `unwrap()`s are confined to test helpers, `Default`/`None` fallbacks, or cases already proven non-empty (e.g. `p_res.as_ref().unwrap()` after an `is_ok()` check in `server.rs`).
- `#[allow(clippy::large_enum_variant)]` appears exactly once (`src/core/events.rs`, `AppEvent`) with a comment justifying it (a sample is moved once per tick; boxing would add an allocation per sample).
- Line length is not mechanically enforced; comments wrap near 100 columns.
- Unsafe blocks are localized and annotated: every `unsafe` FFI call in `hp_wmi.rs`, `wmi_ffi.rs`, `security.rs`, `display.rs`, `tray.rs` carries either a `// SAFETY:` comment or a function-level `# Safety` doc section.
- `#[cfg(...)]` is preferred over runtime platform branching; each platform function has a non-matching stub returning an honest error (`InterfaceMissing`) or `None`.

## Import Organization

Observed order (rustfmt default grouping):
1. `std::...` (`std::sync::Arc`, `std::path::Path`)
2. External crates (`slint`, `zbus`, `serde`, `windows_sys`, `tracing`)
3. Crate items (`crate::core::...`, `crate::service::...`)

Conventions:
- Platform-gated imports are annotated individually: `#[cfg(target_os = "linux")] use zbus::blocking::Connection;`.
- Re-exports live in `mod.rs` files so consumers import from the layer, not the leaf: `crate::hardware::{HardwareProvider, TelemetrySnapshot}`, `crate::service::{FanMode, PresetResult}`.
- `pub use` is used to define a module's public surface (`src/core/mod.rs`, `src/service/mod.rs`, `src/hardware/mod.rs`).
- `build.rs` and `src/lib.rs` keep `slint::include_modules!()` behind `#[cfg(feature = "ui")]` so the daemon never links Slint.

## Error Handling

**Typed errors, not strings, at boundaries:**
- `HardwareControlError` (`src/service/types.rs`) — `IoError`, `VerificationFailed { target, actual, retries }`, `UnsupportedMode`, `PolkitDenied`, `InvalidParameter`, `InterfaceMissing`, `ConnectionFailed`, `ProtocolError`, `AccessDenied`, `AuthorizationFailed`.
- `DisplayError` (`src/hardware/windows/display.rs`), `SystemIntegrationError` (`src/desktop/system_integration.rs`), `DesktopError` (`src/desktop/mod.rs`), `RgbError` (`src/rgb/schema.rs`), `MigrationReport` entries.
- D-Bus mapping is centralized in `impl From<HardwareControlError> for zbus::fdo::Error`.

**Strings at the UI boundary:**
- The worker intentionally converts to `String` for `AppEvent` (`Result<String, String>`, `Result<PresetResult, String>`) because the message is displayed verbatim.
- Never convert a failure into `Ok` with a message containing "failed": `HardwareBackend::apply_preset` returns `Err(VerificationFailed)` when both legs fail.

**Absence vs failure:**
- Missing hardware → `Err(InterfaceMissing)` or `None`/empty, never a zero reading.
- Sampling failure → `AppEvent::TelemetrySamplingFailed(String)` and `AppState::last_sampling_error`.
- Mutations → `AppState::last_control_error`, surfaced in the UI banner and cleared by `ControlErrorDismissed`.

**Poisoned locks:**
- `state.lock().unwrap_or_else(|p| p.into_inner())` is the standard pattern (8 occurrences); the guarded data has no invariants to violate.

## Logging

- `tracing` macros only: `tracing::info!/warn!/error!/debug!`.
- Structured fields first, message last: `tracing::info!(mode = %normalized, "Fan mode confirmed by firmware read-back")`.
- Levels: `info` for lifecycle changes and confirmed writes; `warn` for recoverable degradation (missing sensor, rejected client, skipped restore); `error` for failed mutations and protocol violations; `debug` for per-tick detail.
- File logging is a feature, not an afterthought: `src/logging.rs` installs stderr plus a daily rotating file (7 files max) and degrades to stderr-only if the directory cannot be created. Logging never blocks startup.
- Sensitive content is not logged (no tokens exist; paths and HRESULTs are logged for support).

## Comments and Documentation

- Every module starts with `//!` explaining its contract and the invariant it enforces (see `src/core/state.rs`, `src/service/windows/hp_wmi.rs`, `src/hardware/provider.rs`).
- Comments explain *why*, not *what*: e.g. `// Command 0x10 re-arms the firmware's 120 s manual-fan window, so telemetry must not resend it.`
- Safety-critical reasoning is inline and cites evidence (upstream `hp-wmi` behavior, patch dates, register offsets) — `victus_s.rs` and `profiles.rs` are the canonical examples.
- Public structs/enums document each field's meaning and absence semantics.
- Requirement IDs from `.planning/REQUIREMENTS.md` are referenced in comments (`MON-04`, `WIN-07`, `RGB-02`, `SYS-07`, `PERF-01`) to keep traceability.
- Docs under `docs/architecture/` and `docs/decisions/` are the long-form counterparts (`HP_WMI_PROTOCOL.md`, `IPC_SECURITY.md`, ADR-001..004); code changes that alter those contracts are expected to update them (AGENTS.md rule 10).

## Module and Architecture Layering

- Dependency direction: `app/ui → core → hardware`, and `core/worker → service::client → service::{backend, windows}`. `hardware` never imports `core`; `service` imports `hardware` types only for shared data (`ProcessMemoryInfo`, `HardwareCapabilities`).
- `src/core/` must not touch hardware, filesystem sysfs, or Slint. The Slint bridge lives only in `src/app.rs`.
- Each platform keeps its privileged code isolated: Linux under `src/service/{backend,contract,polkit}.rs`, Windows under `src/service/windows/`. `src/hardware/linux/` and `src/hardware/windows/` never call each other.
- Third platforms must resolve to `UnavailableHardwareProvider` with zero capabilities (AGENTS.md rule 5).

## Mock Hardware Conventions

- `FakeWmiAdapter` (`src/service/windows/hp_wmi.rs`) is the canonical hardware double:
  - `new(available: bool)` marks transport availability.
  - `expect_call(FakeResponse::Success/FirmwareError/WmiError)` pre-loads one response per call, in order.
  - `recorded_calls()` returns `RecordedCall { command, command_type, payload, out_size }` for protocol assertions.
  - `assert_all_consumed()` is called in teardown so an unexercised response fails the test.
- Mutation tests must pre-load read → write → read, mirroring the real verification sequence documented at the top of `tests/windows_service_tests.rs`.
- `MockHardware` (`src/hardware/mock.rs`) provides three scenarios and is selected only by the `ZENOMEN_MOCK_SCENARIO` environment variable; tests construct it directly.
- Filesystem tests use `tempfile`-style unique directories via `std::env::temp_dir().join(format!("zenomen_..._{}", std::process::id()))` or an equivalent suffix, and remove them at the end; no test writes to `/sys` or requires root.
- Injectable roots are the pattern: `LinuxHardwareProvider::with_all_roots`, `SensorRegistry::scan_roots`, `HardwareBackend::new(root)`, `collect_diagnostics_from_roots`, `install_to_prefix`, `resolve_app_data_paths_from`.

## Domain Model Conventions

- `HardwareCapabilities` has one boolean per interface plus `rgb_variant`; `Default` is `none()` so an unfilled capability set degrades rather than lies.
- `TelemetrySnapshot` uses `Option` for sensors that may be absent; `EXPECTED_SENSOR_COUNT` and `reported_sensor_count` define the "X of 11 sensors" status line.
- `PresetResult` always reports per-leg success (`power_applied`, `fan_applied`, `display_applied`), `partial_success`, and an explanatory `message`; `power_profile_status` distinguishes verified from accepted-but-unverified.
- `BoardProfile` = claimed capability, `BoardValidation` = proven capability. Never set a validation write flag from board ID alone; `new_verified` is reserved for documented hardware validation.
- `FanMode` parsing is strict (`auto`/`max` only; `manual` is explicitly rejected with `UnsupportedMode`).
- `AppState` is the only mutable UI truth; every change arrives as an `AppEvent`.

---

*Convention analysis: 2026-11-11*
