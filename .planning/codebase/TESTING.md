---
last_mapped_commit: 4baf4bdbfc56a9755e48c37893d7515f1ffa172d
last_mapped_at: 2026-11-07
---
# Testing Patterns

**Analysis Date:** 2026-11-07

## Test Framework

**Runner:**
- Rust’s built-in test harness (`#[test]`), stable toolchain. There is no `tokio`, `rstest`, `proptest`, `mockall`, `tempfile`, or `nextest`; `Cargo.toml` declares **no `[dev-dependencies]` at all**.
- Config: none beyond `Cargo.toml` (`[profile.test] debug = 0, incremental = false`, `Cargo.toml:57-59`).

**Assertion Library:**
- Standard `assert!`, `assert_eq!`, `assert_ne!`, plus `assert!(matches!(...))` for error-variant checks and explicit `match`/`panic!` when a readable failure message is needed.
- Floating-point comparisons use an explicit tolerance: `assert!((util - 75.0).abs() < 0.01, "Expected 75.0%, got {util}")` (`src/hardware/linux/cpu.rs:231`). There are 9 such tolerance assertions.

**Run Commands (as defined by this repo):**

```bash
# Standard verification commands pipeline
cargo fmt --all -- --check
git diff --check
cargo clippy --all-targets --all-features -- -D warnings
xvfb-run -a cargo test --all
./scripts/validate-icons.py

# CI security audit (.github/workflows/ci.yml)
cargo audit

# Release workflow (.github/workflows/release.yml)
xvfb-run --auto-servernum cargo test --locked --all

# Single suite
cargo test --test benchmark_tests
cargo test --test degradation_tests
cargo test --test fan_control_tests
cargo test --test power_preset_tests
cargo test --test desktop_integration_tests
cargo test --test linux_gpu_tests
cargo test --test logging_tests
cargo test --test sensor_registry_tests

# Unit tests only
cargo test --lib
```

Helper scripts in `scripts/` include:
- `measure-budget.sh`: measures RSS and CPU budget consumption
- `generate-icons.sh`: generates multi-resolution icon assets via Inkscape and ImageMagick
- `validate-icons.py`: validates all 8 hicolor PNG sizes (16x16 to 256x256), scalable SVG, and multi-resolution ICO (7 embedded sizes)

## Test Inventory

**Total test count: 123 verified automated tests across 15 integration test suites and unit tests** (plus 1 benchmark/performance suite, totaling 124 `#[test]` functions: 34 unit tests in `src/` and 90 integration tests across 16 files under `tests/`). There are no `#[ignore]`, no `#[should_panic]`, and no async tests.

### Key Test Suites & Recent Additions
- `tests/logging_tests.rs` (1 test): verifies rotating file sink directory creation, log line emission to state directory, and process-global double initialization safety.
- `tests/linux_gpu_tests.rs` (7 tests including NvidiaSampler watcher and expiry): DRM enumeration, nvidia-smi parsing, `NvidiaSampler` background watcher thread (spawning external tool, handling failures/missing binaries), and stale reading expiry (falling back to `None` after timeout).
- `tests/fan_control_tests.rs` (5 tests including pending fan mode in-flight tracking): `AppEvent` → `AppState` fan-mode transitions, offline daemon handling, and pending fan mode in-flight tracking (marking writes in-flight and clearing upon backend confirmation or failed verification).
- `tests/power_preset_tests.rs` (6 tests including dual-leg partial success honesty): dynamic ACPI profile enumeration, combined presets full workflow, dual-leg partial success honesty (reporting partial success when either fan mode or platform profile succeeds while the other fails), and diagnostics reporting.
- `tests/sensor_registry_tests.rs` (9 tests including sysfs battery presence): sensor priority matrix (`k10temp` and `coretemp` selected over `acpitz`, falling back to `acpitz` when CPU driver missing), fan/pwm discovery on HP WMI, sysfs battery presence detection (measured dynamically via `/sys/class/power_supply`, not hardcoded; reports false when absent), and non-synthesizing absence guarantees.

**Integration suites (`tests/`):**

| File | Tests | Lines | Focus |
|------|------:|------:|-------|
| `tests/benchmark_tests.rs` | 5 | 234 | Worker headless behavior, RSS/tick performance, visibility gating, shutdown |
| `tests/config_tests.rs` | 6 | 181 | Defaults, atomic save, corrupt-file recovery, polling clamp |
| `tests/degradation_tests.rs` | 8 | 209 | One test per degradation case (fan absent, pwm absent, no CPU temp, nvidia-smi missing, RGB absent, service absent, unauthorized write, corrupt config) |
| `tests/desktop_integration_tests.rs` | 8 | 179 | Autostart, single-instance D-Bus object, tray SNI/DBusMenu, icon assets, releases URL |
| `tests/diagnostics_tests.rs` | 2 | 179 | Diagnostics from synthetic roots + live-system smoke test |
| `tests/fan_control_tests.rs` | 5 | 117 | `AppEvent` → `AppState` fan-mode transitions, offline daemon behavior, pending fan mode in-flight tracking |
| `tests/hardware_service_tests.rs` | 9 | 280 | `write_and_verify`, mock sysfs backend, presets, polkit/config assets |
| `tests/linux_gpu_tests.rs` | 7 | 175 | DRM enumeration, nvidia-smi parsing, NvidiaSampler watcher thread, stale reading expiry |
| `tests/linux_procfs_tests.rs` | 4 | 89 | `/proc/stat`, `/proc/meminfo`, `/proc/cpuinfo` parsing, live provider smoke |
| `tests/logging_tests.rs` | 1 | 91 | Rotating file sink + double-init safety |
| `tests/mock_hardware_tests.rs` | 3 | 128 | `MockScenario::{Nominal, ZeroCapabilities, SensorDataGap}` |
| `tests/packaging_tests.rs` | 5 | 172 | Embedded assets match disk, root enforcement, prefix install/uninstall |
| `tests/power_preset_tests.rs` | 6 | 256 | ACPI profile selection, combined presets, dual-leg partial success honesty, diagnostics |
| `tests/rgb_tests.rs` | 8 | 365 | Hex validation, presets, OpenOMEN migration, 3 interface variants, verified writes |
| `tests/sensor_registry_tests.rs` | 9 | 262 | Sensor priority matrix, fan discovery, sysfs battery presence detection |
| `tests/telemetry_history_tests.rs` | 4 | 89 | 120-sample bound, SVG path generation and gap handling |

**Unit test modules in `src/` (`#[cfg(test)] mod tests`):**

| File | Tests | Focus |
|------|------:|-------|
| `src/core/state.rs` | 10 | `AppState::apply`, status copy, degradation, history resize |
| `src/core/telemetry.rs` | 6 | History eviction, SVG path commands/gaps/NaN |
| `src/core/worker.rs` | 2 | Single in-flight mutation gate, guard lifetime |
| `src/service/polkit.rs` | 1 | Polkit response tuple serialization `(bba{ss})` |
| `src/desktop/system_integration.rs` | 1 | Daemon binary install/overwrite behavior |
| `src/hardware/unavailable.rs` | 1 | Zero capabilities, sampling refused |
| `src/hardware/linux/cpu.rs` | 5 | `/proc/stat` parsing, delta utilization, model/freq parsing |
| `src/hardware/linux/gpu.rs` | 3 | Card-name filter, nvidia-smi parse, PCI vendor IDs |
| `src/hardware/linux/memory.rs` | 2 | MemAvailable-based used memory, missing-key behavior |
| `src/hardware/linux/hwmon.rs` | 3 | Millidegree conversion, fan RPM, label trimming |

**Documentation synchronization:** `README.md:226` documents 123 automated unit and integration tests across integration test suites. Update these counts when adding or removing tests (AGENTS.md rule 10).

## Test File Organization

**Location:**
- Pure parsing/state logic is unit-tested co-located with the code in `#[cfg(test)] mod tests` at the bottom of the same file (all 10 modules above).
- Cross-module, filesystem, thread, and binary-level behavior lives in `tests/<area>_tests.rs` integration suites.
- No `tests/common/` shared module and no `tests/fixtures/` directory. Helpers are duplicated per suite by design so each suite is self-contained.

**Naming:**
- Files: `<area>_tests.rs` (16 suites).
- Unit test functions: descriptive sentences, no `test_` prefix (`a_full_sample_reports_every_expected_sensor`, `reports_nothing_rather_than_guessing`).
- Integration test functions: mixed. Older files use `test_` (38 of 90 integration tests), newer files use descriptive names (`mock_hardware_tests.rs`, `rgb_tests.rs`, `sensor_registry_tests.rs`, `config_tests.rs`). Prefer descriptive names for new tests.
- Requirement-driven suites use ordinal prefixes: `degradation_01_...` … `degradation_08_...` (`tests/degradation_tests.rs`).

**Structure:**

```
tests/
├── benchmark_tests.rs
├── config_tests.rs
├── ...
└── telemetry_history_tests.rs

src/core/state.rs
└── #[cfg(test)]
    mod tests { use super::*; ... }
```

## Test Structure

**Unit module pattern (`src/`):**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_partial_sample_is_degraded_but_still_counted() {
        let mut snapshot = full_snapshot();
        snapshot.gpu_name = None;
        let mut state = AppState::default();

        state.apply(&AppEvent::CapabilitiesLoaded(fully_capable()));
        state.apply(&AppEvent::TelemetryUpdated(snapshot));

        assert_eq!(
            state.status_text(),
            "Mock provider · 10 of 11 sensors reported"
        );
        assert!(state.is_degraded());
    }

    fn full_snapshot() -> TelemetrySnapshot { /* local builder */ }
}
```

(Source: `src/core/state.rs:313-468`.)

**Integration pattern (`tests/`):**

```rust
//! Tests for mock hardware provider scenarios (`Nominal`, `ZeroCapabilities`, `SensorDataGap`).

use zenomen::hardware::{HardwareProvider, MockHardware, MockScenario};

#[test]
fn nominal_provider_emits_valid_values_across_sensors() {
    let mock = MockHardware::new(MockScenario::Nominal);
    let caps = mock.capabilities();
    assert!(caps.has_cpu_metrics(), "Nominal must report CPU metrics");
    // ...
}
```

(Source: `tests/mock_hardware_tests.rs`.)

**Patterns:**
- Every integration suite starts with a `//!` module doc listing what it verifies (15 of 16; `tests/diagnostics_tests.rs` is the exception).
- Arrange–act–assert with numbered comments for multi-step state transitions: `// 1. Success event ... // 2. Failure event ...` (`tests/fan_control_tests.rs:8-54`).
- Builders are local helper functions/structs at the top of the file, below imports (`fn full_snapshot()`, `impl MockSysfs`, `fn fake_nvidia_smi`).
- Setup/teardown is RAII, not `setup()`/`teardown()` functions: `Drop for TestDir` or explicit `let _ = fs::remove_dir_all(&dir);` at the end.
- Assertions almost always carry a message (132 message-bearing assertions across the suite), especially when asserting absence or hardware facts.

## Mocking

**Framework:** None. Hand-written test doubles and dependency injection only.

**Primary provider double — `MockHardware`** (`src/hardware/mock.rs`):

```rust
let mock = MockHardware::new(MockScenario::Nominal);
let sample = mock.sample_telemetry().expect("Nominal sample must succeed");
```

- `Nominal`: all capabilities, sine-wave readings.
- `ZeroCapabilities`: zero flags, `TelemetrySnapshot::default()`, no fabricated numbers.
- `SensorDataGap`: nominal, but GPU utilization/temperature become `None` for 3 of every 10 ticks.
- `MockScenario::from_env()` reads `ZENOMEN_MOCK_SCENARIO` (`nominal`, `zero`/`zerocapabilities`, `gap`/`sensor-data`); `src/main.rs:170-185` also accepts it to launch the real binary in a degraded mode. Tests should construct scenarios directly; the env var is for manual/binary runs.

**Filesystem/sysfs doubles — inject a root:**

```rust
let root = std::env::temp_dir().join(format!("zenomen_test_hw_{test_name}_{}", std::process::id()));
let hwmon_dir = root.join("sys/devices/platform/hp-wmi/hwmon/hwmon5");
fs::create_dir_all(&hwmon_dir).expect("failed to create mock hwmon dir");
fs::write(hwmon_dir.join("name"), "hp\n").unwrap();
fs::write(hwmon_dir.join("pwm1_enable"), "2\n").unwrap();

let backend = HardwareBackend::new(&root);
```

(Source: `tests/hardware_service_tests.rs:9-53`; same pattern in `tests/power_preset_tests.rs:10-50` and `tests/rgb_tests.rs:253-350`.)

Other injection seams used by tests:
- `SensorRegistry::scan_roots(hwmon_root, acpi_root)` (`src/hardware/registry.rs:52`).
- `collect_diagnostics_from_roots(state, registry, os_release, kernel_release, dmi_root, drm_root)` (`src/core/diagnostics.rs:74`).
- `set_autostart_enabled_at(...)` / `is_autostart_enabled_at(...)` (`src/desktop/autostart.rs`).
- `open_releases_page_with("nonexistent-xdg-open-command-for-testing")` (`src/desktop/releases.rs`, exercised in `tests/desktop_integration_tests.rs:171`).

**External command double — fake executable:**

```rust
fn fake_nvidia_smi(dir: &std::path::Path, body: &str) -> PathBuf {
    let path = dir.join("fake-nvidia-smi");
    fs::write(&path, format!("#!/bin/sh\n{body}\n")).expect("write fake tool");
    fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).expect("chmod fake tool");
    path
}

let sampler = NvidiaSampler::new(fake_nvidia_smi(dir.path(), "echo '...'")); 
```

(Source: `tests/linux_gpu_tests.rs:87-124`.) Use this instead of invoking a real `nvidia-smi`.

**What to mock:**
- All hardware reads (sysfs, procfs-dependent logic, DRM, hwmon, nvidia-smi).
- The D-Bus service boundary when testing UI state: drive `AppState` with `AppEvent` values instead of a live bus (`tests/fan_control_tests.rs`, `tests/power_preset_tests.rs`).
- Time-sensitive worker behavior with channels and short timeouts, not real hardware.

**What NOT to mock:**
- Pure parsers: pass fixture text directly (`parse_proc_meminfo`, `parse_nvidia_smi_output`, `read_cpu_model`).
- Serde/serialization behavior: use real types (`RgbColor::from_hex`, `to_bytes` for polkit tuples).
- `AppState` logic: construct the real state and call `apply`.

## Fixtures and Temp Directories

There is no `tempfile` dependency. Three local patterns coexist:

**1. RAII `TestDir` (preferred for new tests)** — unique by nanoseconds under `std::env::temp_dir()`, cleaned in `Drop`:

```rust
struct TestDir(PathBuf);

impl TestDir {
    fn new(prefix: &str) -> Self {
        let nanos = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let path = std::env::temp_dir().join(format!("{prefix}-{nanos}"));
        fs::create_dir_all(&path).expect("create test dir");
        Self(path)
    }
    fn path(&self) -> &Path { &self.0 }
}

impl Drop for TestDir {
    fn drop(&mut self) { let _ = fs::remove_dir_all(&self.0); }
}
```

Used in `src/hardware/linux/hwmon.rs:94-113`, `tests/linux_gpu_tests.rs:12-34`, `tests/diagnostics_tests.rs:13-38`, `tests/sensor_registry_tests.rs:11-35`, `tests/logging_tests.rs:11-37`.

**2. `temp_test_dir(name)` helper + manual cleanup** — same nanosecond uniqueness, caller does `let _ = fs::remove_dir_all(&dir);` (`tests/config_tests.rs:17-25`, `tests/rgb_tests.rs:20-28`).

**3. `MockSysfs` builder with PID-based root** — removes a stale directory before creating, cleans up in `Drop` (`tests/hardware_service_tests.rs:13-53`, `tests/power_preset_tests.rs:11-50`).

**Rules observed:**
- One isolated directory per test; never share a directory between tests.
- Never write to `/sys`, `/etc`, `/usr`, or the user’s real config/state dirs. The root is always a temp prefix passed into `HardwareBackend`, `SensorRegistry`, diagnostics, autostart, or install helpers.
- Synthetic trees mirror the real sysfs layout (`sys/class/hwmon/hwmon0`, `sys/devices/platform/hp-wmi/hwmon/hwmon5`, `sys/firmware/acpi/platform_profile*`, `sys/class/drm/card1`, `sys/class/dmi/id/*`).
- Tests that inspect committed assets read them relative to the repository root: `data/applications/zenomen.desktop`, `data/icons/hicolor/{size}x{size}/apps/zenomen.png` (`tests/packaging_tests.rs`, `tests/desktop_integration_tests.rs`). `cargo test` runs with CWD at the package root, which these tests rely on.
- Environment variables are mutated in exactly one test (`XDG_STATE_HOME` in `tests/logging_tests.rs:51-56`), with a `// SAFETY:` comment and a single-test binary because the subscriber is process-global.

## Coverage

**Requirements:** None enforced. No coverage tool, config, threshold, or CI step (`tarpaulin`, `llvm-cov`, `codecov` are absent).

**View Coverage:**
- Not configured. If coverage is needed, add it as a local-only tool; do not add a dev-dependency solely for it without justification (AGENTS.md rule 4).

## Test Types

**Unit Tests:**
- Scope: pure parsers, state transitions, SVG generation, serialization, single-module edge cases.
- Location: bottom-of-file `#[cfg(test)] mod tests`; 34 tests in 10 modules.
- Approach: direct calls, fixture strings, no filesystem except the hwmon reader tests (temp dir).

**Integration Tests:**
- Scope: multi-module flows (worker + provider + channels), filesystem/sysfs trees, fake executables, desktop/autostart behavior, packaging assets, degradation matrix.
- Location: `tests/*.rs`; 90 tests in 16 suites.
- Approach: public `zenomen::` API, injected roots, `MockHardware`, `AppState::apply`.

**E2E Tests:**
- Not used. There is no test that starts the Slint window or a real D-Bus session. `tests/desktop_integration_tests.rs` calls the D-Bus interface methods directly (`ApplicationService::present_window`, `StatusNotifierItem::activate`, `TrayDbusMenu`) rather than talking to a bus.

**Async Tests:**
- None (`#[tokio::test]` / async test count is 0). The only async code (`src/service/contract.rs` zbus methods) is exercised indirectly through the sync backend and through polkit tuple serialization (`src/service/polkit.rs:74-100`).

## Common Patterns

**Concurrency and timing:**

```rust
let event = event_receiver
    .recv_timeout(Duration::from_millis(500))
    .expect("Worker must send initial capabilities immediately");
```

- Use `mpsc::Receiver::recv_timeout` with short budgets (100–500 ms) for worker events (`tests/benchmark_tests.rs`).
- Use deadline loops instead of a single sleep when waiting for a background thread:

```rust
let deadline = Instant::now() + Duration::from_secs(5);
let sample = loop {
    if let Some(sample) = sampler.latest() { break sample; }
    assert!(Instant::now() < deadline, "watcher never produced a sample");
    std::thread::sleep(Duration::from_millis(10));
};
```

(`tests/linux_gpu_tests.rs:109-119`.)
- Fixed sleeps are used only where the behavior itself is time-based: stale GPU sample expiry (`3_100 ms`, `tests/linux_gpu_tests.rs:170`) and fake-tool startup (10–200 ms).

**Error testing:**

```rust
assert!(sample_telemetry().is_err(), "an unimplemented platform must not sample");

match result {
    Err(SystemIntegrationError::NotRoot) => {}
    other => panic!("Expected NotRoot error when non-root, got: {:?}", other),
}
```

- Assert the error variant when the variant is the contract (`tests/packaging_tests.rs:51-58`); otherwise assert `is_err()` and, when relevant, that the message contains the reason (`src/core/worker.rs` rejection test).
- For “no fabricated data” contracts, assert `is_none()`/`is_empty()`/`0` explicitly (`tests/mock_hardware_tests.rs:88-98`, `src/hardware/unavailable.rs:40-50`).

**State-machine testing:**
- Drive `AppState` only with events and assert both confirmed and in-flight fields after each step (`tests/fan_control_tests.rs`, `tests/power_preset_tests.rs`).

**Asset/embedded-content testing:**
- Assert `include_str!` constants equal the on-disk files so packaged assets cannot drift (`tests/packaging_tests.rs:12-49`).

## CI Checks

Single `quality` job on `ubuntu-latest` (`.github/workflows/ci.yml`), least-privilege `contents: read`, concurrency cancellation, all gates blocking:
1. `cargo fmt --all -- --check`
2. `git diff --check`
3. `cargo clippy --all-targets --all-features -- -D warnings`
4. `xvfb-run -a cargo test --all` (or `xvfb-run --auto-servernum cargo test --all`)
5. `./scripts/validate-icons.py`
6. `cargo audit` against the committed `Cargo.lock`

Native packages installed for the build: `libxkbcommon-dev`, `libfontconfig1-dev`, `libgl1-mesa-dev`, `xvfb`. Release workflow repeats fmt/clippy/tests with `--locked`, audits, then packages `.deb` and `.AppImage` via `cargo-packager` (`.github/workflows/release.yml:57-79`).

Note: no test source currently imports `slint` or constructs `AppWindow` (grep across `tests/` and `src/**/tests`); `xvfb-run` is retained as the documented/CI invocation. Keep using the CI command so the gate stays identical to what runs remotely.

## Known Gaps

- **Slint bridge untested:** `src/app.rs` (545 lines) has no `#[cfg(test)]` module and no integration test. Callback wiring, command queue saturation, and `invoke_from_event_loop` publication are unverified.
- **Entry points untested:** `src/main.rs` and `src/bin/zenomen-hw.rs` have no tests (startup, provider selection, CLI flags, daemon connection).
- **D-Bus interface methods untested:** `src/service/contract.rs` (214 lines) has no tests; only the backend and polkit serialization are covered.
- **Live-system assumptions:** `tests/linux_procfs_tests.rs:74-89` and `tests/benchmark_tests.rs:18-36` assert against the real `/proc`; there are no `#[cfg(target_os = "linux")]` gates on the suite, so the tests are Linux-only in practice.
- **Timing sensitivity:** the GPU stale-expiry test sleeps 3.1 s and worker tests use wall-clock budgets; slow CI runners could flake. Prefer deadline loops over single sleeps in new tests.
- **Count synchronization:** `README.md:226` documents 123 tests (15 integration test suites plus unit tests), while the repo has 124 test functions including the benchmark suite. Keep docs synchronized when adding or altering tests.

**Checklist for new tests (per AGENTS.md rule 7):**
1. Add a unit test in the same file for pure logic; add/extend a `tests/<area>_tests.rs` suite for cross-module behavior.
2. Use `MockHardware` or inject a temp root; never touch live hardware, `/sys`, or root paths.
3. Create a uniquely named temp dir and clean it via `Drop` (copy the `TestDir` pattern).
4. Assert with a message that explains the hardware/state contract being protected.
5. Run `cargo fmt --all -- --check`, `git diff --check`, `cargo clippy --all-targets --all-features -- -D warnings`, `xvfb-run -a cargo test --all`, and `./scripts/validate-icons.py`.
6. Update the test counts in `README.md` if the total changes.

---

*Testing analysis: 2026-11-07*
