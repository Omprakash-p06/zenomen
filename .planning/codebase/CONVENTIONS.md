---
last_mapped_commit: 206cdc2634e0a9d0a07a90f0580927ad5b1c554e
last_mapped_at: 2026-11-07
---
# Coding Conventions

**Analysis Date:** 2026-11-07

## Naming Patterns

**Files:**
- Rust source files are `snake_case.rs`: `src/core/state.rs`, `src/hardware/linux/hwmon.rs`, `src/desktop/system_integration.rs`, `src/hardware/windows/cpu.rs`, `src/service/windows/pipe.rs`.
- Modules that own a subsystem use a directory plus `mod.rs` (this repo does not use `foo/foo.rs`): `src/core/mod.rs`, `src/hardware/mod.rs`, `src/service/mod.rs`, `src/rgb/mod.rs`, `src/desktop/mod.rs`, `src/config/mod.rs`, `src/hardware/linux/mod.rs`, `src/hardware/windows/mod.rs`, `src/service/windows/mod.rs`, `src/desktop/windows/mod.rs`.
- Integration test suites are `tests/<area>_tests.rs` — 18 files (17 integration test suites and 1 benchmark suite, totaling 110 integration tests), e.g. `tests/config_tests.rs`, `tests/hardware_service_tests.rs`, `tests/mock_hardware_tests.rs`, `tests/logging_tests.rs`, `tests/windows_service_tests.rs`, `tests/windows_telemetry_tests.rs`.
- Binaries: `src/main.rs` builds `zenomen`; `src/bin/zenomen-hw.rs` builds `zenomen-hw` (`Cargo.toml` lines 40-47).

**Functions:**
- `snake_case`, verb-first, intent in the prefix:
  - Parsers take `&str` or byte slices and return `Option`/`Result`: `parse_proc_meminfo`, `parse_nvidia_smi_output`, `read_cpu_model`, `read_cpu_freq_from_cpuinfo`, `parse_battery_status`, `calculate_cpu_utilization`, `decode_frame` (`src/hardware/linux/memory.rs`, `src/hardware/linux/gpu.rs`, `src/hardware/linux/cpu.rs`, `src/hardware/windows/battery.rs`, `src/hardware/windows/cpu.rs`, `src/service/protocol.rs`).
  - Scanners/discovery: `scan_hwmon_devices`, `scan_roots`, `enumerate_drm_gpus`, `detect_rgb_interface`, `resolve_board_profile`, `enumerate_gpu_adapters` (`src/hardware/linux/hwmon.rs`, `src/hardware/registry.rs`, `src/hardware/linux/gpu.rs`, `src/hardware/rgb.rs`, `src/service/windows/profiles.rs`, `src/hardware/windows/gpu.rs`).
  - Predicates: `has_cpu_metrics`, `is_degraded`, `is_connected`, `is_card_device_name`, `is_full_success` (`src/hardware/provider.rs`, `src/core/state.rs`, `src/service/client.rs`, `src/service/types.rs`).
  - Constructors: `new`, `default`, `from_env`; builder-style options use `with_` (`with_connection`, `with_polkit_required` in `src/service/contract.rs`).
  - Conversions follow `from_*` / `as_*` / `to_*`: `FanMode::from_sysfs`, `as_sysfs_value`, `RgbColor::to_hex`, `to_slint_color`, `encode_frame`.
- Free functions are preferred for pure parsing; they never touch global paths.

**Variables:**
- `snake_case`, with units encoded in the name: `cpu_temperature_c`, `cpu_frequency_mhz`, `ram_used_mb`, `polling_interval_ms`, `vram_used_mb`, `fan_rpms` (`src/hardware/provider.rs`, `src/config/schema.rs`).
- Option-valued readings are named plainly and documented as `None` when unreadable — never defaulted to a fabricated zero: `gpu_utilization: Option<f32>`, `gpu_name: Option<String>` (`src/hardware/provider.rs`).
- Error/unavailable state uses explicit names: `last_sampling_error`, `last_control_error`, `pending_fan_mode`, `config_corrupt_notice` (`src/core/state.rs`).
- Loop variables that are intentionally unused are prefixed `_`: `for _attempt in 1..=max_retries` (`src/service/backend.rs`).

**Types:**
- `PascalCase` noun phrases: `HardwareCapabilities`, `TelemetrySnapshot`, `SensorRegistry`, `TelemetryHistory`, `MockScenario`, `PresetResult`, `BoardProfile`, `WindowsCpuTimes`.
- Enums use PascalCase variants and carry payloads where the failure/state is richer than a string: `FanMode::Manual(u8)`, `FanMode::Unknown(String)`, `MockScenario::ZeroCapabilities`, `GpuVendor::Unknown(u32)` (`src/service/types.rs`, `src/hardware/mock.rs`, `src/hardware/windows/gpu.rs`).
- Error enums always end in `Error`: `HardwareControlError`, `RgbError`, `SystemIntegrationError`, `ReleasesError`, `DesktopError`.
- Traits name a capability, not an implementation: `HardwareProvider` (`src/hardware/provider.rs`), `HardwareServiceClient` (`src/service/client.rs`).
- Domain structs derive `Debug, Clone, PartialEq, Eq` where possible and add `Copy` for small enums/flags. Types holding `f32` omit `Eq` (`TelemetrySnapshot` in `src/hardware/provider.rs`).
- Generic constructors accept `impl Into<String>` or `P: Into<PathBuf>` rather than concrete `String`/`PathBuf`: `RgbProfile::new`, `HardwareBackend::new` (`src/rgb/schema.rs`, `src/service/backend.rs`).

**Constants:**
- `SCREAMING_SNAKE_CASE`, declared near the concept they bound, with a doc comment that states the unit and the reason:
  - `SAMPLING_INTERVAL`, `HISTORY_WINDOW`, `MAX_SAMPLES`, `EXPECTED_SENSOR_COUNT`, `COMMAND_QUEUE_DEPTH` (`src/core/worker.rs`, `src/core/state.rs`, `src/core/telemetry.rs`, `src/hardware/provider.rs`, `src/app.rs`).
  - `PIPE_NAME`, `PIPE_BUFFER_SIZE`, `PIPE_TIMEOUT_MS`, `PIPE_SDDL`, `MAX_PAYLOAD_SIZE`, `PROTOCOL_VERSION` (`src/service/windows/pipe.rs`, `src/service/windows/security.rs`, `src/service/protocol.rs`).
  - Bounds are exported when callers/tests need them: `MIN_POLLING_INTERVAL_MS`, `MAX_POLLING_INTERVAL_MS`, `DEFAULT_POLLING_INTERVAL_MS` (`src/config/schema.rs`).
  - Magic numbers are extracted into named constants, e.g. `GAP_PERIOD_TICKS`, `GAP_TICKS`, `RAM_TOTAL_MB` (`src/hardware/mock.rs`).

**Test function names:**
- 58 of 150 verified automated tests use a `test_` prefix (common in integration suites such as `desktop_integration_tests.rs`, `hardware_service_tests.rs`, `fan_control_tests.rs`, `packaging_tests.rs`, `power_preset_tests.rs`, `windows_service_tests.rs`, and `windows_telemetry_tests.rs`). Unit tests in `src/` and newer domain suites use descriptive sentences: `a_partial_sample_is_degraded_but_still_counted` (`src/core/state.rs`), `nominal_provider_emits_valid_values_across_sensors` (`tests/mock_hardware_tests.rs`), `test_filetime_delta_to_cpu_utilization_nominal` (`tests/windows_telemetry_tests.rs`).
- Follow the descriptive sentence or domain-focused style for new tests; do not rename existing tests just to normalize.
- Requirement-oriented suites use a stable numeric prefix: `degradation_01_...` through `degradation_08_...` (`tests/degradation_tests.rs`).

## Code Style

**Formatting:**
- `rustfmt` with default settings. There is no `rustfmt.toml` or `.rustfmt.toml` in the repository.
- Edition 2024 is used (`Cargo.toml` line 4), so edition-2024 idioms appear:
  - let-chains: `if hp_wmi_hwmon.is_dir() && let Ok(entries) = fs::read_dir(&hp_wmi_hwmon)` (`src/service/backend.rs:80`), also `src/main.rs:30`, `src/core/diagnostics.rs:112`.
  - `unsafe { std::env::set_var(...) }` where tests mutate the environment (`tests/logging_tests.rs:54`).
- 4-space indentation, no tabs, no trailing whitespace, LF endings.
- Formatting is a hard CI gate: `cargo fmt --all -- --check` accompanied by `git diff --check`.
- Standard verification pipeline: `cargo fmt --all -- --check`, `git diff --check`, `cargo clippy --all-targets --all-features -- -D warnings`, `xvfb-run -a cargo test --all`, and `./scripts/validate-icons.py`.

**Linting:**
- Clippy runs with all targets and all features, warnings denied: `cargo clippy --all-targets --all-features -- -D warnings` (`.github/workflows/ci.yml:60`; the release workflow adds `--locked`).
- `Cargo.toml` has no `[lints]` table; lints are enforced solely by the CI command line.
- Suppress narrowly with explicit justification comments: `#[allow(clippy::large_enum_variant)]` on `AppEvent` (`src/core/events.rs`). Do not add file-level allows.

**Derives and serde:**
- Standard derive order observed: `#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]` (`src/hardware/provider.rs:27`), `#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]` (`src/config/schema.rs`).
- Config/RGB structs use `#[serde(default)]` on every newly-added optional field so older files still deserialize (`src/config/schema.rs`, `src/rgb/schema.rs`).
- Enums exposed over serde use `#[serde(rename_all = \"lowercase\")]` (`FanMode`, `src/service/types.rs:8`).
- Custom conversions are implemented instead of leaking wire formats: `Serialize`/`Deserialize` for `RgbColor` serialize as `#RRGGBB` strings (`src/rgb/schema.rs:94-110`).

**Feature and Platform Separation (ARCH-01):**
- **Compile-time OS separation (ARCH-01):** Platform selection is strictly compile-time via `#[cfg(target_os = ...)]`. Pure Linux builds contain no Win32 or WMI code and link no Windows libraries; pure Windows builds contain no sysfs, `/proc`, or systemd code and link no `zbus` libraries.
- The GUI is an optional default feature: `#[cfg(feature = "ui")] pub mod app;` and `#[cfg(feature = "ui")] slint::include_modules!();` (`src/lib.rs`). UI-only methods are gated the same way: `#[cfg(feature = "ui")] pub fn to_slint_color` (`src/rgb/schema.rs`).
- Linux hardware implementation lives under `src/hardware/linux/` and is gated by `#[cfg(target_os = "linux")]` (`src/hardware/mod.rs`).
- Windows hardware implementation lives under `src/hardware/windows/` and is gated by `#[cfg(target_os = "windows")]` (`src/hardware/mod.rs`).
- Non-Linux/non-Windows builds or unsupported targets fallback cleanly to `UnavailableHardwareProvider`, reporting zero capabilities and an honest error on sampling — never speculative reads (`src/hardware/unavailable.rs`).
- Cross-platform shared protocols reside in `src/service/protocol.rs`, utilizing endian-safe length-prefixed framing and JSON payloads for both Unix domain and Windows named pipe transport.

**Win32 FFI & System Programming Guidelines:**
- **RAII for DLL handles (FreeLibrary):** Every dynamically loaded DLL handle acquired via `LoadLibraryA` or `LoadLibraryW` must be released with `FreeLibrary(module)` before function exit or cached statically using `std::sync::OnceLock`. Leaking module handles in 1-second telemetry loops causes reference counter overflow (`src/hardware/windows/cpu.rs:166`).
- **Handle closure:** Always release Win32 kernel handles with `CloseHandle(handle)` and registry keys with `RegCloseKey(hkey)` in all control flow paths, including early returns.
- **Thread affinity for Win32 message pumps:** Window handles (`HWND`) created with `CreateWindowExW` and their corresponding message loops (`GetMessageW` / `DispatchMessageW`) are strictly thread-affine under Win32. A dedicated message-pump thread (`zenomen-win32-tray-pump`) must create the window within its own thread context before entering `GetMessageW`, and communicate the resulting `HWND` back via synchronization channels (`std::sync::mpsc`). Window teardown must be posted asynchronously to the pump thread via `PostMessageW(hwnd, WM_CLOSE, 0, 0)` rather than calling `DestroyWindow` from an alien thread (`src/desktop/windows/tray.rs`).
- **Explicit SDDL DACL for named pipes:** Never pass a NULL security descriptor to `CreateNamedPipeW`. Named pipe instances must attach an explicit Discretionary Access Control List (DACL) using canonical SDDL: `D:(A;;GA;;;SY)(A;;GA;;;BA)(A;;GRGW;;;IU)` (Generic All for SYSTEM and BUILTIN\Administrators; Generic Read/Write for Interactive Users) converted via `ConvertStringSecurityDescriptorToSecurityDescriptorW` and cleaned up with `LocalFree(p_sd)` (`src/service/windows/security.rs`, `src/service/windows/pipe.rs`).
- **Wide string and null-termination safety:** All Win32 `*W` functions must receive wide UTF-16 strings with an explicit null terminator via `.chain(std::iter::once(0))`. When reading string values from the Windows registry, buffer decoding must locate the first null byte (`buf[..len].iter().position(|&b| b == 0)`) to guard against uninitialized or trailing buffer junk (`src/hardware/windows/device.rs`, `src/hardware/windows/cpu.rs`).
- **Non-intrusive named pipe probing:** Do not check service availability by opening and immediately closing pipe handles with `CreateFileW`, as this consumes server connection slots and triggers `ERROR_BROKEN_PIPE` in server worker loops. Use `WaitNamedPipeW` or non-destructive status checks instead (`src/service/windows/client.rs`).

## Import Organization

**Order (dominant pattern):**
1. `std::...` imports.
2. Blank line.
3. External crates, alphabetical (`serde`, `thiserror`, `slint`, `windows_sys`, `zbus`, `tracing_*`).
4. Blank line.
5. `crate::...` imports.

Examples: `src/app.rs:16-24`, `src/hardware/registry.rs:7-13`, `src/core/worker.rs:20-27`, `src/service/backend.rs:3-7`.

**Path aliases:**
- None. There are no `[workspace]` members and no path aliases; use `crate::...` for internal paths and `zenomen::...` from integration tests.

**Re-exports:**
- Each subsystem `mod.rs` declares its modules first, then re-exports the public surface with `pub use` in alphabetical order: `src/core/mod.rs`, `src/hardware/mod.rs`, `src/desktop/mod.rs`, `src/service/mod.rs`.
- Callers import from the curated boundary, not deep modules, where a boundary exists: integration tests use `zenomen::hardware::{HardwareProvider, MockHardware, MockScenario}` (`tests/mock_hardware_tests.rs`). Deep imports are acceptable for parsers that are deliberately testable, e.g. `zenomen::hardware::linux::cpu::{CpuSampler, read_cpu_model}` or `zenomen::hardware::windows::battery::parse_battery_status`.
- Glob imports appear only as `use super::*;` inside `#[cfg(test)] mod tests` blocks.

## Error Handling

**Core rule — absence is not an error and is never a zero:**
- Unreadable sensors are `None`/empty collections, not `0`: `gpu_utilization: Option<f32>`, `fan_rpms: Vec<u32>` (`src/hardware/provider.rs:147-181`). Sysfs and Win32 readers return `Option` via `.ok()?` or explicit checks: `read_temp_millidegrees`, `read_fan_rpm`, `read_cpu_frequency`, `parse_battery_status`.
- Operation failure returns `Result`; the UI-displayable reason crosses the provider boundary as `Result<TelemetrySnapshot, String>` with “a human-readable reason … it must never be a panic” (`src/hardware/provider.rs:239-241`).

**Typed errors:**
- Domain errors use `thiserror` and are `Clone + PartialEq + Eq` so they can be asserted on and moved through events: `HardwareControlError` (`src/service/types.rs`), `RgbError` (`src/rgb/schema.rs`), `SystemIntegrationError` (`src/desktop/system_integration.rs`), `ReleasesError` (`src/desktop/releases.rs`), `DesktopError` (`src/desktop/windows/tray.rs`).
- `#[error("...")]` messages are user-facing sentences and include context fields.
- Conversions to external error types are explicit: `impl From<HardwareControlError> for zbus::fdo::Error` (`src/service/types.rs`).
- Filesystem config APIs use `std::io::Result<()>` where no domain error is needed: `save_atomic` (`src/config/mod.rs:85`).

**Context preservation:**
- Add context with `map_err`, including the path: `fs::write(path, ...).map_err(|e| HardwareControlError::IoError(format!("Write to {} failed: {e}", path.display())))` (`src/service/backend.rs:22-24`).
- Use `?` for propagation and `let ... else` for graceful early exits: `let Ok(entries) = std::fs::read_dir(hwmon_root) else { return devices; }` (`src/hardware/linux/hwmon.rs:36-38`).

**Panics and poisoning:**
- Library/runtime code does not `panic!`, `unwrap()`, or `expect()`. The only `panic!` calls in `src/` are inside `#[cfg(test)] mod tests`.
- Startup thread spawns are the sole runtime `expect`s and carry messages: `"spawning desktop action listener must succeed"` (`src/main.rs:168`), `"spawning event pump must succeed"` (`src/main.rs:254`).
- Mutex poisoning is recovered rather than propagated: `.lock().unwrap_or_else(|p| p.into_inner())` (`src/app.rs`).
- Failures degrade: `load_or_default` backs up a corrupt config and returns defaults plus a notice (`src/config/mod.rs`); `logging::init` falls back to stderr-only (`src/logging.rs`); unsupported hardware yields `UnavailableHardwareProvider` (`src/hardware/unavailable.rs`).

## Logging

**Framework:** `tracing` + `tracing-subscriber` with `EnvFilter`; stderr layer plus a daily rotating file layer (`src/logging.rs`). `RUST_LOG` selects the filter; default level is `info`.

**Patterns:**
- Structured fields, not string interpolation: `tracing::warn!(path = %path.display(), error = %e, "could not read config file; using defaults")` (`src/config/mod.rs:42`), `tracing::info!(path = %path.display(), "configuration atomically saved")` (`src/config/mod.rs:114`).
- Message text is lowercase, imperative/declarative, and has no trailing period: `"monitoring worker started"`, `"ignored unknown mock scenario"` (`src/core/worker.rs:52`, `src/hardware/mock.rs:81`).
- `%` for `Display` values (paths), `?` for `Debug` values (e.g. scenario lists).
- Log-and-degrade on recoverable failure; `error!` only when a fallback action itself failed.
- `println!`/`eprintln!` are confined to CLI entry points (`src/main.rs`, `src/bin/zenomen-hw.rs`) and the logging fallback warnings in `src/logging.rs`.
- Never log secrets, environment dumps, or raw sensor file contents.

## Comments and Documentation

**Module docs (`//!`):**
- Every module starts with a purpose statement plus the design constraint it encodes. Examples: `src/hardware/provider.rs` (why absence is `None`), `src/core/state.rs` (single source of truth), `src/app.rs` (two directions only), `src/hardware/unavailable.rs` (do not add speculative reads), `src/service/windows/mod.rs` (Windows named-pipe service).
- Requirement IDs are cited in parentheses so intent stays traceable: `(FOUND-04)`, `(FOUND-05)`, `(MON-04)`, `(MON-11)`, `(SYS-06)`, `(RGB-02)`, `(PERF-02)`, `(WIN-01)`, `(WIN-03)`, `(WIN-04)`, `(WIN-07)`, `(ARCH-01)`.
- Module docs state *why*, including rejected alternatives and failure contracts.

**Item docs (`///`):**
- Public structs, fields, enums, variants, traits, and functions are documented. Field docs state units and `None` semantics: `src/hardware/provider.rs`, `src/core/state.rs`.
- Intra-doc links are used for related items: `[`AppState::apply`]`, `[`TelemetrySnapshot::reported_sensor_count`]`.

**Inline comments:**
- Explain rationale, ordering, safety, and trade-offs rather than restating the code:
  - `// SAFETY: setting LC_CTYPE before starting any worker threads...` (`src/main.rs:34`).
  - `// Win32 message pump must run in the creating thread...` (`src/desktop/windows/tray.rs`).
  - `// Relaxed is sufficient: the counter only has to be unique...` (`src/hardware/mock.rs:135-136`).

## Function and API Design

**Small, injected, testable:**
- IO roots and external program paths are constructor parameters, never hardcoded inside logic: `HardwareBackend::new(root)` (`src/service/backend.rs`), `SensorRegistry::scan_roots(hwmon_root, acpi_root)` (`src/hardware/registry.rs`), `NvidiaSampler::new(program)` (`src/hardware/linux/gpu.rs`), `set_autostart_enabled_at` (`src/desktop/autostart.rs`).
- Production wrappers delegate to injectable `*_from_roots` / `*_at` functions: `collect_diagnostics` → `collect_diagnostics_from_roots` (`src/core/diagnostics.rs`).
- Parsers accept `&str` or byte slices so they can be unit-tested with fixture text; filesystem readers accept `&Path`.
- Prefer returning owned plain data (`TelemetrySnapshot`, `DiagnosticsReport`, `BoardProfile`) over borrowed views across threads/modules.
- Public functions that can fail return `Result`; functions that answer “is it there?” return `bool`/`Option`.

**Threading:**
- No heavy async runtime in the product (no Tokio; `Cargo.toml` has no async runtime dependency). Background work uses `std::thread`, named threads (`thread::Builder::new().name("zenomen-monitor-worker".to_string())`), and `std::sync::mpsc` channels.
- The zbus interface methods (`src/service/contract.rs`) are the only `async fn`s on Linux; callers use zbus’s blocking API outside them. On Windows, communication uses synchronous framed named pipes (`src/service/windows/client.rs`).
- Shared mutable state uses `Arc<Mutex<_>>` / atomics, with a documented reason for the ordering.

## Module and Architecture Conventions

- **Layer boundaries are enforced by imports:** `src/core/` never touches hardware directly; `src/app.rs` owns the only Slint bridge; `src/hardware/` owns sensors, sysfs, Win32 APIs, and external tools.
- **Single writer for UI state:** `AppState` is mutated only through `AppState::apply(&AppEvent)` (`src/core/state.rs`). Add new state via a new `AppEvent` variant + `apply` arm, not by assigning fields from callers.
- **Message types:** UI → core is `AppCommand`, core → UI is `AppEvent` (`src/core/events.rs`). Variants carry domain types, never sysfs paths or shell commands.
- **UI callbacks are non-blocking:** they enqueue an `AppCommand` through the bounded channel (`COMMAND_QUEUE_DEPTH = 8`) and drop/log if full (`src/app.rs`).
- **Capabilities are per-subsystem booleans,** not one aggregate flag (`HardwareCapabilities`, `src/hardware/provider.rs`).
- **Platform separation (ARCH-01):** only `src/hardware/linux/**` may read `/sys` or `/proc`; only `src/hardware/windows/**` may call Win32 telemetry APIs; unsupported targets stay honest with zero capabilities.
- **Dependency discipline:** no new heavy frameworks; the RSS budget (≤40 MB) is a design constraint encoded in `Cargo.toml` comments and AGENTS.md rule 4.

## Documentation Synchronization

- When interfaces, test counts, or build requirements change, update `README.md` and the relevant docstrings in the same change (AGENTS.md rule 10).
- Total test count: 150 automated tests verified passing across 18 integration test suites and unit tests (40 unit tests in `src/lib.rs` and 110 integration tests in `tests/`). Ensure docs and tests remain synchronized when touching the test suite.

---

*Convention analysis: 2026-11-07*
