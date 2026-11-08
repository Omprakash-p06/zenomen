# Testing Patterns

**Analysis Date:** 2026-11-08

## Test Framework

**Runner:**
- Rust built-in `libtest` (stable toolchain). No external test runner or harness crate.
- `Cargo.toml` sets `[profile.test] debug = 0` and `incremental = false` for fast, low-disk test builds.
- No `rstest`, `proptest`, `mockall`, `criterion`, or async test runtime is used. The workspace contains no Tokio dependency.

**Assertion Library:**
- Standard library `assert!`, `assert_eq!`, `assert_ne!`, `matches!`, `.expect(...)`, `.unwrap_err()` / `expect_err`.
- Assertions usually include a descriptive message stating the invariant, e.g. `assert_eq!(snapshot.used_mb, ..., "naive MemFree must not be used")` in `tests/linux_procfs_tests.rs`.

**Run Commands:**
```bash
cargo test --all                       # Windows and general host run (README/CI)
xvfb-run -a cargo test --all           # Linux CI: required for any Slint surface tests
cargo test --test <suite_name>         # Run one integration suite, e.g. cargo test --test config_tests
cargo fmt --all -- --check             # Formatting gate
cargo clippy --all-targets --all-features -- -D warnings   # Zero-warning gate
bash scripts/measure-budget.sh         # RSS/CPU budget harness (Linux, PERF-01/02/06)
```

**CI enforcement:**
- `.github/workflows/ci.yml` runs `cargo fmt --check`, clippy with `-D warnings`, `xvfb-run --auto-servernum cargo test --all` on Ubuntu, and `cargo test --all` on Windows, plus `cargo audit` on Linux.
- `.github/workflows/release.yml` repeats `--locked` fmt/clippy/tests before packaging.
- `docs/testing/GEMINI_TESTING_HANDOFF.md` lists the full pre-merge pipeline, including cross-compilation clippy checks for the Windows targets and `python3 ./scripts/validate-icons.py`.

**Current suite size:** 155 `#[test]` functions — 40 inline unit tests across 12 source files plus 115 integration tests across 18 files in `tests/`. The README currently states "123 tests across 16 integration test suites" (`README.md:226`), which lags the actual tree; treat the source as authoritative.

## Test File Organization

**Location:**
- **Inline unit tests:** a `#[cfg(test)] mod tests { use super::*; ... }` block at the bottom of the module under test. Present in:
  - `src/core/state.rs` (10 tests), `src/core/telemetry.rs` (6), `src/core/worker.rs` (2)
  - `src/hardware/linux/cpu.rs` (5), `src/hardware/linux/gpu.rs` (3), `src/hardware/linux/hwmon.rs` (3), `src/hardware/linux/memory.rs` (2)
  - `src/hardware/unavailable.rs` (1)
  - `src/service/protocol.rs` (6), `src/service/polkit.rs` (1)
  - `src/desktop/system_integration.rs` (1)
- **Integration tests:** top-level `tests/` files, importing the public crate as `zenomen::...`:
  - `tests/benchmark_tests.rs` (5), `tests/config_tests.rs` (6), `tests/degradation_tests.rs` (8), `tests/desktop_integration_tests.rs` (12), `tests/diagnostics_tests.rs` (2), `tests/fan_control_tests.rs` (5), `tests/hardware_service_tests.rs` (9), `tests/linux_gpu_tests.rs` (7), `tests/linux_procfs_tests.rs` (4), `tests/logging_tests.rs` (1), `tests/mock_hardware_tests.rs` (3), `tests/packaging_tests.rs` (5), `tests/power_preset_tests.rs` (6), `tests/rgb_tests.rs` (9), `tests/sensor_registry_tests.rs` (11), `tests/telemetry_history_tests.rs` (5), `tests/windows_service_tests.rs` (9), `tests/windows_telemetry_tests.rs` (8).

**Naming:**
- Files: `snake_case` + `_tests.rs` under `tests/`.
- Test behavior names are full sentences in newer modules (`a_full_sample_reports_every_expected_sensor`); older integration suites prefix with `test_` (`test_fan_mode_parsing_and_conversion`). Match the surrounding file.
- Platform-only suites use an inner attribute at the top of the file: `#![cfg(target_os = "linux")]` in `tests/linux_procfs_tests.rs` and `tests/linux_gpu_tests.rs`; `#![cfg(not(target_os = "windows"))]` in `tests/logging_tests.rs`. Individual tests use `#[cfg(target_os = "linux")]` / `#[cfg(target_os = "windows")]` where only a portion is platform-specific (`tests/benchmark_tests.rs:118`, `tests/fan_control_tests.rs:118`, `tests/sensor_registry_tests.rs:244`).

**Structure:**
```
tests/                       # integration suites against the public `zenomen` crate
  <area>_tests.rs
src/<module>.rs              # inline `#[cfg(test)] mod tests` for private helpers
src/hardware/linux/<x>.rs    # Linux parser tests co-located with the parser
```

## Test Structure

**Suite Organization:**
```rust
//! Tests for bounded telemetry history and SVG path generation.

use zenomen::core::telemetry::{MAX_SAMPLES, TelemetryHistory, TelemetryPoint, generate_svg_path};

#[test]
fn history_strictly_bounds_samples_to_120_on_overflow() {
    let mut history = TelemetryHistory::new();
    for i in 0..150 {
        history.push(TelemetryPoint {
            timestamp: Instant::now(),
            cpu: Some(i as f32),
            gpu: Some((i * 2) as f32),
            ram: Some(50.0),
            fan: None,
        });
    }
    assert_eq!(history.sample_count(), MAX_SAMPLES);
}
```

**Patterns:**
- Every integration file opens with a `//!` module doc naming what it verifies and the requirement IDs it covers (e.g. `tests/config_tests.rs` documents SYS-06/SYS-07/SYS-09).
- Inline unit tests import `use super::*;` and test private helpers directly.
- Setup is explicit and local to each test — no global fixtures, no test ordering assumptions.
- Scenario tests number the steps in comments and assert after each transition (`tests/fan_control_tests.rs`, `tests/power_preset_tests.rs`).
- Teardown is RAII (`Drop`) or an explicit `let _ = fs::remove_dir_all(...)`; prefer the RAII fixture for new tests.

## Mocking

**Framework:** None (no `mockall`). Fakes are hand-written implementations of the existing traits, and shell/OS boundaries are replaced by path injection.

**Patterns:**
```rust
// src/hardware/mock.rs — deterministic provider used by unit + integration tests
let mock = MockHardware::new(MockScenario::SensorDataGap);
let sample = mock.sample_telemetry().expect("sampling must succeed");

// tests/hardware_service_tests.rs — mock sysfs tree behind an injectable root
struct MockSysfs { root: PathBuf }
let backend = HardwareBackend::new(&self.root);
let res = write_and_verify(&temp_file, "updated", 3, Duration::from_millis(5));

// tests/sensor_registry_tests.rs — fixture builds hwmon/<id>/name, temp*_input, fan*_input, pwm1_enable
let registry = SensorRegistry::scan_roots(tree.path(), &empty_acpi);
```

**What to Mock:**
- Hardware reads/writes: use `MockHardware` / `MockScenario` (`src/hardware/mock.rs`) or a temporary filesystem tree passed into `SensorRegistry::scan_roots` (`src/hardware/registry.rs`), `HardwareBackend::new` (`src/service/backend.rs`), `collect_diagnostics_from_roots` (`src/core/diagnostics.rs`).
- External commands: parse their output with a pure function instead of executing them (`parse_nvidia_smi_output` in `src/hardware/linux/gpu.rs`, tested in `tests/degradation_tests.rs`; `parse_battery_status`, `calculate_cpu_utilization` in `src/hardware/windows/`).
- Filesystem/env: redirect via temp dirs and `*_at`/`*_from_roots` helpers (`set_autostart_enabled_at`, `is_autostart_enabled_at`, `resolve_app_data_paths_from`, `XDG_STATE_HOME` in `tests/logging_tests.rs`).

**What NOT to Mock:**
- Do not mock the `HardwareProvider` trait inside `AppState` tests — `AppState::apply` is fed real `AppEvent` values and asserted directly (`src/core/state.rs`, `tests/fan_control_tests.rs`).
- Do not mock serialization: protocol tests round-trip the real `encode_frame`/`decode_frame` (`tests/windows_service_tests.rs`, `src/service/protocol.rs`).
- Do not touch live `/sys`, `/proc`, `/dev`, WMI, D-Bus system bus, or root-only paths. Tests that need a location must have a root/path parameter.

## Fixtures and Factories

**Test Data:**
```rust
// src/core/state.rs — named factory for a fully-populated snapshot
fn full_snapshot() -> TelemetrySnapshot { /* all 11 sensors populated */ }
fn fully_capable() -> HardwareCapabilities { HardwareCapabilities::all_supported() }

// tests/sensor_registry_tests.rs — unique temp tree with Drop cleanup
struct TestHwmonTree(PathBuf);
impl TestHwmonTree {
    fn new(prefix: &str) -> Self {
        let nanos = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let path = std::env::temp_dir().join(format!("{prefix}-{nanos}"));
        fs::create_dir_all(&path).expect("create test dir");
        Self(path)
    }
}
impl Drop for TestHwmonTree { fn drop(&mut self) { let _ = fs::remove_dir_all(&self.0); } }
```

**Location:**
- Fixtures are defined inline in the test file that owns them, not shared across suites. Repeated shapes:
  - `TestDir(PathBuf)` in `src/hardware/linux/hwmon.rs`, `tests/linux_gpu_tests.rs`, `tests/logging_tests.rs`, `tests/diagnostics_tests.rs`.
  - `TestHwmonTree` in `tests/sensor_registry_tests.rs`.
  - `MockSysfs` in `tests/hardware_service_tests.rs`.
- Temp directory names include `SystemTime::now()` nanoseconds and/or `std::process::id()` to avoid collisions between parallel tests: `zenomen_config_test_{name}_{nanos}`, `zenomen_test_hw_{name}_{pid}`.
- Static text fixtures (procfs/cpuinfo/meminfo/nvidia-smi/WMI output) are inline `&str` constants with expected arithmetic in comments.
- No `tests/fixtures/` directory exists; do not add shared mutable fixtures without cleaning them up.

## Coverage

**Requirements:** No numeric coverage tool or threshold is configured. The practical gate is a green `cargo test --all` plus zero-warning clippy in CI.

**View Coverage:**
- Not configured. To inspect locally, use an external tool (e.g. `cargo llvm-cov`) — do not add it to `Cargo.toml` without justification, per the dependency budget.

**What is covered (by area):**
- **Core state machine:** event folding, degradation/status text, history window, sampling-error state (`src/core/state.rs`, `tests/degradation_tests.rs`, `tests/fan_control_tests.rs`).
- **Telemetry chart math:** 120-sample bound, SVG `M`/`L` generation, gap handling, sparklines (`src/core/telemetry.rs`, `tests/telemetry_history_tests.rs`).
- **Worker lifecycle:** visibility-gated sampling pause/resume, prompt shutdown, tick latency (`src/core/worker.rs`, `tests/benchmark_tests.rs`).
- **Mock provider contract:** all eight subsystem flags and the three scenarios (`tests/mock_hardware_tests.rs`).
- **Linux parsers and registry:** CPU jiffies, `/proc/meminfo` (uses `MemAvailable`), `/proc/cpuinfo`, hwmon priority (`k10temp`/`coretemp` over `acpitz`), fan/PWM discovery, DRM GPU enumeration, nvidia-smi parsing (`src/hardware/linux/*`, `tests/linux_procfs_tests.rs`, `tests/linux_gpu_tests.rs`, `tests/sensor_registry_tests.rs`).
- **Verified control path:** write-and-verify retries/mismatch/I/O error, `FanMode` parsing and rejection, mock backend fan/profile/preset flows, PresetResult partial-success honesty (`tests/hardware_service_tests.rs`, `tests/power_preset_tests.rs`).
- **Service protocol:** version validation, frame size limits, request/response round-trips, polkit status handling (`src/service/protocol.rs`, `src/service/polkit.rs`, `tests/hardware_service_tests.rs`).
- **Windows pure logic:** CPU filetime deltas, RAM math, battery status parsing, GPU vendor recognition, named-pipe framing, board-profile resolution, authorization rules (`tests/windows_telemetry_tests.rs`, `tests/windows_service_tests.rs`). These tests run on any host because the Win32 calls are behind internal `#[cfg(windows)]` shims.
- **Config:** defaults, atomic save, no leftover temp file, clamp to `[500, 5000]` ms, corrupt-file backup and notice (`tests/config_tests.rs`).
- **Desktop/packaging:** single-instance D-Bus dispatch, StatusNotifierItem/menu, XDG autostart preserve-and-toggle, app-data path resolution, embedded asset parity with `data/`, icon validity, Cargo packaging mappings (`tests/desktop_integration_tests.rs`, `tests/packaging_tests.rs`, `src/desktop/system_integration.rs`).
- **Logging:** a real line reaches the XDG state directory and a second `init()` degrades instead of panicking (`tests/logging_tests.rs`).
- **RGB:** profile schema/serialization, capability detection within a custom root, enable restores the previous profile (`tests/rgb_tests.rs`).
- **Diagnostics:** mock-root report collection and export (`tests/diagnostics_tests.rs`).

**What is not covered (gaps):**
- **Slint UI layer:** `src/app.rs` and `src/ui/{app,components,screens,theme}` have no direct tests. The UI is exercised only indirectly through state assertions under `xvfb-run`.
- **Live hardware:** no test reads real `/sys`, `/proc`, hwmon, DRM, WMI, or battery hardware; on-device validation is manual (`docs/testing/HP_BOARD_VALIDATION.md`, `docs/testing/WINDOWS_TEST_MATRIX.md`).
- **True Windows system calls:** DXGI/PDH/registry/named-pipe/server lifecycle are compiled for Windows but can only be exercised on Windows CI or physical hardware.
- **Privileged operations:** real root installation, polkit prompts, systemd/D-Bus service activation, and service install/uninstall are mocked at the path level; `tests/packaging_tests.rs` skips root-requiring branches when not root.
- **GUI rendering/lifecycle:** window open/close, tray restore, and event-loop teardown are not automated.

## Test Types

**Unit Tests:**
- Private and pure helpers, inline in the module: parsers, math, state folding, frame codec. Fast, no filesystem or only a temp dir.

**Integration Tests:**
- Public crate surface in `tests/`, using `MockHardware`, temp trees, and injected roots. These cover cross-module contracts (provider → state → event, client → backend → protocol).

**E2E / GUI Tests:**
- Not automated. No Playwright/Selenium/robot equivalent. Manual procedures live in `docs/testing/WINDOWS_TEST_MATRIX.md` and `docs/testing/HP_BOARD_VALIDATION.md`.

**Performance/Budget Tests:**
- `tests/benchmark_tests.rs` asserts headless safety, valid RSS instrumentation (<500 MB sanity bound), worker tick duration, and prompt shutdown. `scripts/measure-budget.sh` is the manual RSS/CPU harness (Linux `/proc` based).

## Common Patterns

**Thread and Channel Testing (no async):**
```rust
let (tx, rx) = mpsc::channel();
let worker = spawn_monitoring_worker(Arc::new(MockHardware::new(MockScenario::Nominal)), tx, command_rx);

let event = rx.recv_timeout(Duration::from_secs(2)).expect("event within deadline");
assert!(matches!(event, AppEvent::TelemetryUpdated(_)));

let _ = command_tx.try_send(AppCommand::Shutdown);
worker.join().expect("worker exits cleanly");
```
Use `recv_timeout`/`Instant` deadlines rather than fixed sleeps where the assertion depends on a message (`src/core/worker.rs` tests, `tests/benchmark_tests.rs`). A bounded `sleep` + poll loop is acceptable only for asynchronous filesystem writers (`tests/logging_tests.rs`).

**Error Testing:**
```rust
let err = FanMode::parse_requested("turbo").unwrap_err();
assert!(matches!(err, HardwareControlError::InvalidParameter(_)));

let mis = write_and_verify(&file, "wrong", 2, Duration::from_millis(1));
assert!(matches!(mis, Err(HardwareControlError::VerificationFailed { .. })));
```
Assert the *variant*, not the full message, so text changes do not churn tests. Use `matches!` for structured errors.

**Degradation Testing:**
```rust
// tests/degradation_tests.rs — remove one interface at a time; assert no panic and clean None/empty
let registry = SensorRegistry::scan_roots(&hwmon_root, &acpi_root);
assert!(registry.cpu_temp.is_none());          // unavailable, not fabricated 0.0
assert_eq!(registry.fan_tachometers.len(), 1);
```

**Environment Mutation:**
```rust
// SAFETY: this test binary has exactly one test, so no other thread reads the environment.
unsafe { std::env::set_var("XDG_STATE_HOME", dir.path()); }
```
Env mutation is only acceptable in a suite with a single test (see `tests/logging_tests.rs`), and must carry a `// SAFETY:` comment.

**Platform Gating:**
```rust
#![cfg(target_os = "linux")]              // whole file
#[cfg(target_os = "windows")]              // single test
#[cfg(not(target_os = "windows"))]
```
New platform-specific tests must be gated so both Windows and Linux CI stay green, and must not import the other platform's native APIs.

**Numeric Tolerance:**
```rust
assert!((agg2 - 75.0).abs() < 0.1, "Expected 75%, got {agg2}");
```
Use epsilon comparisons for floats; exact equality is reserved for integers and enum values.

---

*Testing analysis: 2026-11-08*
