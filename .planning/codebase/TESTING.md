---
last_mapped_commit: e50af8f9ea61300a1c10d36dc6954e319da7b6e2
last_mapped_at: 2026-11-02
---
# Testing Patterns

**Analysis Date:** 2026-11-02

## Test Framework

**Runner:**
- `cargo test` (built-in Rust test harness, stable toolchain, edition 2024).
- No test-runner config file and **no dev-dependencies** in `Cargo.toml` — no `mockall`, no `rstest`, no `proptest`, no `tokio`, no `serial_test`.
- Integration tests are native `tests/*.rs` targets; unit tests are `#[cfg(test)] mod tests` blocks inside `src/**/*.rs`.

**Assertion Library:**
- Standard library `assert!`, `assert_eq!`, `assert_ne!`, and `matches!` — no external assertion crate.
- `expect("...")` is used for fixture setup and for values the test asserts must exist; plain `unwrap()` appears mostly in older fixture code.
- Float comparisons use a tolerance: `assert!((util - 75.0).abs() < 0.01, "Expected 75.0%, got {util}")` (`src/hardware/linux/cpu.rs:227`).

**Run Commands:**

```bash
cargo test                                      # all tests
cargo test --all                                # all workspace targets (CI form)
xvfb-run --auto-servernum cargo test --all      # headless CI form (required in CI)
cargo test --lib                                # unit tests only (src/**)
cargo test --test hardware_service_tests        # one integration suite
cargo test fan_mode                             # filter by name substring
cargo test --locked --all                       # release workflow form (Cargo.lock enforced)
```

There is no watch, coverage, or benchmark harness wired up (no `cargo-watch`, `llvm-cov`, `tarpaulin`, or `criterion`).

## Test Suite Inventory

**Totals:** 15 integration test files, 6 in-source unit-test modules, **108 `#[test]` functions** (79 integration + 29 unit).

| File | Tests | Covers |
|------|------:|--------|
| `tests/benchmark_tests.rs` | 5 | RSS instrumentation, worker tick latency, headless worker dispatch (PERF-01, MON-10) |
| `tests/config_tests.rs` | 6 | TOML roundtrip, atomic save, corrupt-file backup, polling clamp (SYS-06/07/09) |
| `tests/degradation_tests.rs` | 8 | Degradation matrix: fan/pwm/temp/nvidia-smi/RGB/service absence, unauthorized write, corrupt config (PERF-05) |
| `tests/desktop_integration_tests.rs` | 8 | Autostart XDG entry, single-instance D-Bus dispatch, StatusNotifierItem, desktop/icon assets |
| `tests/diagnostics_tests.rs` | 2 | `DiagnosticsReport` from injected roots + live-system smoke test |
| `tests/fan_control_tests.rs` | 3 | Fan-mode state transitions, verification-error preservation, service status mapping |
| `tests/hardware_service_tests.rs` | 9 | `zenomen-hw` backend, `write_and_verify`, fan parsing, presets, RGB writes, polkit contract |
| `tests/linux_gpu_tests.rs` | 3 | DRM card enumeration, nvidia-smi CSV parsing |
| `tests/linux_procfs_tests.rs` | 4 | `/proc/stat` CPU deltas, `/proc/cpuinfo` model/freq, `/proc/meminfo` |
| `tests/mock_hardware_tests.rs` | 3 | `Nominal`, `ZeroCapabilities`, `SensorDataGap` scenarios and the no-fabricated-zero rule |
| `tests/packaging_tests.rs` | 5 | Embedded assets vs `data/`, install/uninstall to a mock prefix, root enforcement, Cargo.toml packaging metadata |
| `tests/power_preset_tests.rs` | 5 | ACPI profile selection, combined presets, partial success, mock backend |
| `tests/rgb_tests.rs` | 8 | Hex validation, builtin presets, three interface variants, OpenOMEN migration, sysfs writes |
| `tests/sensor_registry_tests.rs` | 6 | Shared registry and CPU-temp priority matrix (k10temp > coretemp > acpitz) |
| `tests/telemetry_history_tests.rs` | 4 | 120-sample bound, SVG path generation, `None` gap breaks |

| Unit module | Tests | Covers |
|-------------|------:|--------|
| `src/core/state.rs:293` | 10 | `AppState::apply` transitions, status text, degradation, history resizing |
| `src/core/telemetry.rs:247` | 6 | `TelemetryHistory` capacity/eviction, SVG path rendering |
| `src/hardware/linux/cpu.rs:188` | 5 | `CpuJiffies::parse_stat_line`, delta utilization, cpuinfo parsing |
| `src/hardware/linux/gpu.rs:249` | 3 | nvidia-smi parsing edge cases |
| `src/hardware/linux/memory.rs:75` | 2 | `/proc/meminfo` arithmetic |
| `src/hardware/linux/hwmon.rs:88` | 3 | Temperature/fan/label sysfs readers |

**Split rule:** parsers and pure state machines get in-source unit tests; anything that needs a fake filesystem root, a fake provider, or a channel gets an integration test in `tests/`.

## Test File Organization

**Integration tests:**
- Flat directory, no subfolders: `tests/<domain>_tests.rs`.
- Naming `<subsystem>_<kind>_tests.rs` or `<domain>_tests.rs`: `sensor_registry_tests.rs`, `desktop_integration_tests.rs`, `hardware_service_tests.rs`.
- Each file is its own crate, so it may only use the **public** API via `use zenomen::...`. If a test needs a private helper, either make it `pub` on the module boundary or add the test to the in-source `#[cfg(test)]` module.

**Unit tests:**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_stat_line_correctly_extracts_jiffies() { ... }
}
```

Always at the bottom of the file; `use super::*;` plus explicit `std` imports when needed (e.g. `use std::fs;` inside `src/hardware/linux/hwmon.rs:90`).

## Test Structure

**File header:** every integration file starts with a `//!` block naming the requirement IDs it verifies:

```rust
//! Integration tests for fan mode control, read-back verification state, and UI state mapping.
```

```rust
//! Degradation matrix integration tests (PERF-05).
//!
//! Asserts that removing any sensor, interface, external tool, or daemon
//! causes zero crashes and cleanly degrades to an explained unavailable state.
```

**Test body:** arrange → act → assert, with numbered comments for multi-step state transitions:

```rust
#[test]
fn test_fan_mode_state_transitions() {
    let mut state = AppState::default();
    assert_eq!(state.fan_mode, "");

    // 1. Success event updates fan mode and clears errors
    state.apply(&AppEvent::FanModeChanged(Ok("max".to_string())));
    assert_eq!(state.fan_mode, "max");

    // 2. Failure event preserves active mode and sets error
    state.apply(&AppEvent::FanModeChanged(Err("...".to_string())));
    assert_eq!(state.fan_mode, "max"); // preserved
    assert!(state.last_control_error.is_some());
}
```

(`tests/fan_control_tests.rs:8`)

**Patterns:**
- **Assertion messages explain the contract**, not just the value: `assert!(caps.has_fan_control(), "Nominal must report fan control")`, `assert_eq!(state, default, "Default configuration must be restored")`.
- **Abbreviated scenario naming:** numbered tests for the degradation matrix (`degradation_01_...` … `degradation_08_...`) so the matrix is auditable against the plan.
- **Structural byte-equality for hardware safety:** after a rejected write, the test re-reads the sysfs file and asserts it is unchanged: `assert_eq!(content, "2\n", "File content must not be corrupted on error")` (`tests/degradation_tests.rs`).
- **Fixture struct literals with `..AppState::default()`** rather than builders for state-level tests (`tests/diagnostics_tests.rs:83`).
- **`matches!` for error variants:** `assert!(matches!(RgbColor::from_hex(""), Err(RgbError::EmptyColor)))` (`tests/rgb_tests.rs`).

## Mocking

**Framework:** none. Fakes are hand-written at the trait and filesystem boundaries.

**1. Trait injection — `MockHardware`:**

```rust
pub trait HardwareProvider: Send + Sync {
    fn capabilities(&self) -> HardwareCapabilities;
    fn sample_telemetry(&self) -> Result<TelemetrySnapshot, String>;
}
```

`MockHardware::new(MockScenario::Nominal)` implements it in `src/hardware/mock.rs`; the worker and `main` accept `Arc<dyn HardwareProvider>`, so tests pass the fake without touching sysfs (`tests/benchmark_tests.rs:44`, `src/main.rs:172`).
- Scenarios: `Nominal` (all sensors, oscillating values), `ZeroCapabilities` (every flag false, empty snapshot), `SensorDataGap` (GPU readings vanish for 3 of every 10 ticks).
- Runtime selection in the real binary: `ZENOMEN_MOCK_SCENARIO=zero|gap|nominal cargo run` (`src/hardware/mock.rs:76`, `src/main.rs:164`).

**2. Filesystem-root injection — the dominant hardware fake:**
- `SensorRegistry::scan_roots(&hwmon_root, &acpi_root)` — tests create a temp tree with `hwmon0/name`, `temp1_input`, `fan1_input`, `pwm1_enable`, `platform_profile*` and point the scan at it (`tests/degradation_tests.rs:16`, `tests/sensor_registry_tests.rs`).
- `HardwareBackend::new(&root)` prefixes every sysfs path (`src/service/backend.rs:63`); `HardwareBackend::default()` is `new("/")`. Tests build `root/sys/devices/platform/hp-wmi/hwmon/hwmon5/...` under a temp dir (`tests/hardware_service_tests.rs:11`).
- `detect_rgb_interface(&sysfs_root)`, `install_to_prefix(&temp_dir)`, `is_autostart_enabled_at(&path)`, `set_autostart_enabled_at(&path, bool)`, `collect_diagnostics_from_roots(...)`.
- There is **no global env-var path override** (besides the mock-scenario selector); prefer adding an `_at`/`_roots`/`_from_prefix` variant over reading env vars.

**3. Temp-directory fixtures with `Drop` cleanup:**

```rust
struct MockSysfs { root: PathBuf }

impl MockSysfs {
    fn new(test_name: &str) -> Self {
        let root = std::env::temp_dir().join(format!(
            "zenomen_test_hw_{test_name}_{}", std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);   // idempotent start
        fs::create_dir_all(&root).unwrap();
        Self { root }
    }
}

impl Drop for MockSysfs {
    fn drop(&mut self) { let _ = fs::remove_dir_all(&self.root); }
}
```

(`tests/hardware_service_tests.rs:12`; same shape as `TestDir` in `tests/diagnostics_tests.rs:12` and `src/hardware/linux/hwmon.rs:92`.)
- Unique names use `std::process::id()` or `SystemTime::now().duration_since(UNIX_EPOCH).as_nanos()` — tests run in parallel threads of one process, so the prefix must include the test name.
- Always clean at the start *and* in `Drop`; never assume the directory is fresh.

**4. Channel and D-Bus fakes:**
- Worker tests use `mpsc::channel()` for events and `mpsc::sync_channel(8)` for commands, asserting with `recv_timeout(Duration::from_millis(500))` (`tests/benchmark_tests.rs:46`).
- Desktop/tray tests call the zbus interface methods directly on the service object and read the action channel instead of starting a bus (`tests/desktop_integration_tests.rs:56`).
- The absent-daemon test tolerates both environments: it only asserts errors when `status.is_connected()` is false (`tests/degradation_tests.rs`).

**What to mock:**
- Hardware reads (filesystem roots, `HardwareProvider`).
- External commands (`nvidia-smi` output is passed as a `&str` to `parse_nvidia_smi_output`, never spawned).
- Privileged installation targets (`install_to_prefix`).
- Absence scenarios: delete a file/dir from the fake tree rather than returning an error.

**What NOT to mock:**
- Pure parsers (`parse_stat_line`, `parse_proc_meminfo`, `parse_nvidia_smi_output`, `RgbColor::from_hex`) — call them directly with literal input.
- `AppState` / `TelemetryHistory` — plain structs, construct and assert.
- The filesystem itself via a mocking crate — use a temp root instead.

## Fixtures and Factories

- No `fixtures/` directory and no factory crate. Fixtures are constructed inline.
- Full snapshot fixture pattern (`src/core/state.rs:427`):

```rust
fn full_snapshot() -> TelemetrySnapshot {
    TelemetrySnapshot {
        cpu_name: "AMD Ryzen 7 5800H".to_string(),
        cpu_temperature_c: Some(68.0),
        gpu_utilization: Some(62.0),
        ram_total_mb: 16_384,
        fan_rpms: vec![2_450, 2_400],
        power_profile: "balanced".to_string(),
        ..TelemetrySnapshot::default()
    }
}
```

- Capability fixture: `HardwareCapabilities::all_supported()` for "fully equipped", then mutate one flag for the degraded case (`src/core/state.rs:318`).
- Mock sysfs fixture: a `MockSysfs`/`TestDir` struct with `new(name)` + `Drop`; add files with `fs::write(dir.join("temp1_input"), "55000\n")`.
- Embedded-resource fixtures: `tests/packaging_tests.rs` compares `EMBEDDED_*` constants against `data/**` on disk, so edits to `data/` must be mirrored in `src/desktop/system_integration.rs`.

## Coverage

**Requirements:** none enforced. No coverage tool, config, threshold, or CI coverage step.
- CI enforces **pass/fail only** plus the clippy/format/audit gates.
- To measure locally you would need to add a tool (e.g. `cargo llvm-cov`); none is installed or documented.
- Practical coverage expectation comes from the requirement IDs in test-file headers and plan acceptance criteria, not from a percentage.

## Test Types

**Unit tests:**
- In-source `#[cfg(test)] mod tests`, 29 tests across 6 modules.
- Scope: pure parsing (`CpuJiffies`, nvidia-smi CSV, meminfo), state folding (`AppState::apply`), history bounds/SVG paths, and the three hwmon readers against a temp file.

**Integration tests:**
- `tests/*.rs`, 79 tests across 15 crates.
- Scope: public API behavior across modules — registry priority, degradation matrix, backend write/read-back verification, presets, RGB variants/migration, config persistence, desktop integration, packaging assets.

**Headless/GUI tests:**
- CI runs the whole suite through `xvfb-run --auto-servernum cargo test --all` because the test binaries link the Slint/winit/femtovg stack and a future GUI test needs a display (`.github/workflows/ci.yml`, `.github/workflows/release.yml`).
- **No current test constructs `AppWindow`** (`grep AppWindow tests/` returns nothing). The suite is deliberately headless: `tests/benchmark_tests.rs` states "Headless CI safety (no GUI server requirement or panics)". The window is exercised only by launching the binary under `xvfb-run` in manual/packaging verification (`docs/PACKAGING.md`, `.planning/phases/05-*/05-01-PLAN.md`).
- If you add a GUI test, put it in `tests/ui_*_tests.rs`, construct `zenomen::AppWindow::new()` on the test thread, keep it short, and run under `xvfb-run`; never leave a window shown or block on the event loop.

**Performance tests:**
- `tests/benchmark_tests.rs` asserts a worker tick is under ~5 ms, RSS is < 500 MB (sanity bound for the 40 MB budget), and the worker answers `RefreshTelemetry` within 500 ms without waiting for the 1 s tick.
- Long-duration budget measurement is a shell harness, not a test: `scripts/measure-budget.sh` (with `--soak`), documented in `docs/RESOURCE-BUDGET.md`.

## Common Patterns

**Async/channel testing (no async runtime):**

```rust
let (event_sender, event_receiver) = mpsc::channel();
let (command_sender, command_receiver) = mpsc::sync_channel(8);
let worker = spawn_monitoring_worker(provider, event_sender, command_receiver);

let event = event_receiver
    .recv_timeout(Duration::from_millis(500))
    .expect("Worker must send initial capabilities immediately");
match event {
    AppEvent::CapabilitiesLoaded(caps) => assert!(caps.has_cpu_metrics()),
    other => panic!("Expected CapabilitiesLoaded, got: {other:?}"),
}
```

Never `thread::sleep` to wait for a result; use `recv_timeout` and fail with a contract message.

**Error testing:**

```rust
assert!(matches!(RgbColor::from_hex("#FFF"), Err(RgbError::InvalidLength(_))));
assert!(RgbColor::from_hex("#GGGGGG").is_err());

match install() {
    Err(SystemIntegrationError::NotRoot) => {}
    other => panic!("Expected NotRoot error when non-root, got: {other:?}"),
}
```

- Assert the **variant**, not just `is_err()`, whenever the variant carries meaning.
- For absence, assert `Option::is_none()` and explicitly no fabricated zero: `assert!(sample.gpu_utilization.is_none(), "Non-numeric utilization degrades cleanly to None")`.

**Filesystem assertions:**
- Read back the file to prove hardware safety (`content == "2\n"` after a rejected write).
- Assert side-effect files exist (`config.toml.bak.<timestamp>`), and that temp files do **not** linger (`config_atomic_write_leaves_no_lingering_temp_file`).
- Parse exported JSON back into the typed struct and assert equality (`tests/diagnostics_tests.rs`).

**Live-system tests (small, non-assertive):**
- `collect_diagnostics_live_system_does_not_panic` and `linux_hardware_provider_samples_cleanly_on_linux` may read the real `/sys` and `/proc`.
- They assert only invariants that hold on every Linux host (non-empty strings, no panic). They must not assert a specific CPU model, fan count, or GPU — CI runners have none of these.

## CI Quality Gate

Both workflows run the same four gates; release adds packaging. Order matters — packaging cannot start if a gate fails.

**`.github/workflows/ci.yml` (push/PR to `main`):**
1. `cargo fmt --all -- --check`
2. `cargo clippy --all-targets --all-features -- -D warnings`
3. `xvfb-run --auto-servernum cargo test --all`
4. `cargo audit` (audits the committed `Cargo.lock`)

**`.github/workflows/release.yml` (tag `v*` or manual):**
1. Tag must equal `Cargo.toml` version (`cargo metadata` + `jq`)
2. `cargo fmt --all -- --check`
3. `cargo clippy --locked --all-targets --all-features -- -D warnings`
4. `xvfb-run --auto-servernum cargo test --locked --all`
5. `cargo audit`
6. `cargo packager --release --formats deb,appimage`
7. Artifact names must contain the version; generate `SHA256SUMS`; publish release

**Implications for new tests:** they must be warning-free under clippy (`-D warnings`), rustfmt-clean, pass with `--locked` (do not add a dependency just for a test), not require network access, and clean up temp files.

## Conventions Checklist for New Tests

- [ ] File-level `//!` doc naming the requirement ID(s) verified.
- [ ] Integration file named `tests/<domain>_tests.rs`; unit tests in `#[cfg(test)] mod tests` at the bottom of the source file.
- [ ] Test name is a snake_case sentence describing the expected behavior (`sensor_data_gap_scenario_periodically_emits_none`).
- [ ] Fake hardware through `MockHardware` or a temp filesystem root; no real `/sys` writes.
- [ ] Temp dir name includes the test name plus PID/nanos; cleanup on start and in `Drop`.
- [ ] Assert with an explanatory message; assert error variants with `matches!`.
- [ ] Absence asserted as `None`/empty — never assert or accept a fabricated `0`.
- [ ] Channels use `recv_timeout`; no fixed `sleep` for synchronization.
- [ ] No new dev-dependency; std only.
- [ ] Runs green under `xvfb-run --auto-servernum cargo test --all`, `cargo fmt --all -- --check`, and `cargo clippy --all-targets --all-features -- -D warnings`.

---

*Testing analysis: 2026-11-02*
