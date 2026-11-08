---
last_mapped_commit: 4691619f4e741bd5944f2085fa455b135ff873af
last_mapped_at: 2026-11-08
---
# Testing Patterns

**Analysis Date:** 2026-11-08

## Test Framework

**Runner:**
- Rust’s built-in test harness (`#[test]`), stable toolchain. There is no `tokio`, `rstest`, `proptest`, `mockall`, `tempfile`, or `nextest`; `Cargo.toml` declares **no `[dev-dependencies]` at all**.
- Config: none beyond `Cargo.toml` (`[profile.test] debug = 0, incremental = false`, `Cargo.toml:57-59`).

**Assertion Library:**
- Standard `assert!`, `assert_eq!`, `assert_ne!`, plus `assert!(matches!(...))` for error-variant checks and explicit `match`/`panic!` when a readable failure message is needed.
- Floating-point comparisons use an explicit tolerance: `assert!((util - 75.0).abs() < 0.01, "Expected 75.0%, got {util}")` (`src/hardware/linux/cpu.rs:231`), `assert!((util - 66.6667).abs() < 0.01)` (`tests/windows_telemetry_tests.rs:33`).

**Run Commands (as defined by this repo):**

```bash
# Standard verification commands pipeline
git diff --check
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo check --target x86_64-pc-windows-msvc --no-default-features
cargo check --target x86_64-pc-windows-msvc --bin zenomen-hw
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
cargo test --test windows_service_tests
cargo test --test windows_telemetry_tests

# Unit tests only
cargo test --lib
```

Helper scripts in `scripts/` include:
- `measure-budget.sh`: measures RSS and CPU budget consumption
- `generate-icons.sh`: generates multi-resolution icon assets via Inkscape and ImageMagick
- `validate-icons.py`: validates all 8 hicolor PNG sizes (16x16 to 256x256), scalable SVG, and multi-resolution ICO (7 embedded sizes)

## Test Inventory

**Total test count: 153 verified automated tests across 18 test suites and unit tests (all passing)** (40 unit tests in `src/lib.rs` across 11 modules and 113 integration tests across 18 files under `tests/`). There are no `#[ignore]`, no `#[should_panic]`, and no async tests.

### Key Test Suites & Recent Additions
- `tests/windows_service_tests.rs` (8 tests): named pipe protocol roundtrip, 64 KB payload rejection, board profile resolution and unknown board rejection, fan mode authorization, unvalidated hardware mutation blocking (BoardValidation gating), mock client-server named pipe communication, and non-Windows lifecycle/server interface missing guards.
- `tests/windows_telemetry_tests.rs` (8 tests): GetSystemTimes CPU delta utilization math, clock warp / zero delta guards, GlobalMemoryStatusEx RAM math, battery status parsing (charging, discharging, unknown flag 255, desktop flag 128), DXGI vendor ID recognition, GpuPerformanceCollector & VRAM distinction, and structured sensor telemetry guarantees.
- `tests/desktop_integration_tests.rs` (12 tests): verifies Linux and Windows desktop integration contracts, including autostart lifecycles, single-instance D-Bus service dispatch, SNI/DBusMenu status notifier items, icon assets, releases URL formatting, launcher failure fallback, Windows local AppData path resolution, Windows release URL safety, and registry autostart key escaping.
- `tests/sensor_registry_tests.rs` (11 tests): verifies sensor priority matrix (`k10temp` and `coretemp` selected over `acpitz`, falling back to `acpitz` when CPU driver missing), fan/pwm discovery on HP WMI, sysfs battery presence detection (measured dynamically via `/sys/class/power_supply`), non-synthesizing absence guarantees, and custom root isolation for generic PWM fans.
- `tests/rgb_tests.rs` (9 tests): verifies hex color validation, built-in presets, OpenOMEN migration safety, all three interface variants (four-zone, legacy, kernel patch), verified hardware writes, and restoring the previous profile when re-enabling RGB instead of hardcoding white.
- `tests/telemetry_history_tests.rs` (5 tests): verifies 120-sample bound on overflow, SVG path generation and gap handling, NaN/infinity filtering, and dynamic capacity resizing to preserve the target time window when polling intervals change.
- `tests/linux_gpu_tests.rs` (7 tests): DRM enumeration, nvidia-smi parsing, `NvidiaSampler` background watcher thread (spawning external tool, handling failures/missing binaries), and stale reading expiry (falling back to `None` after timeout).
- `tests/fan_control_tests.rs` (5 tests): `AppEvent` → `AppState` fan-mode transitions, offline daemon handling, and pending fan mode in-flight tracking (marking writes in-flight and clearing upon backend confirmation or failed verification).
- `tests/power_preset_tests.rs` (6 tests): dynamic ACPI profile enumeration, combined presets full workflow, dual-leg partial success honesty (reporting partial success when either fan mode or platform profile succeeds while the other fails), and diagnostics reporting.

**Integration suites (`tests/`):**

| File | Tests | Lines | Focus |
|------|------:|------:|-------|
| `tests/benchmark_tests.rs` | 5 | 234 | Worker headless behavior, RSS/tick performance, visibility gating, shutdown |
| `tests/config_tests.rs` | 6 | 181 | Defaults, atomic save, corrupt-file recovery, polling clamp |
| `tests/degradation_tests.rs` | 8 | 209 | One test per degradation case (fan absent, pwm absent, no CPU temp, nvidia-smi missing, RGB absent, service absent, unauthorized write, corrupt config) |
| `tests/desktop_integration_tests.rs` | 12 | 272 | Autostart, single-instance D-Bus, tray SNI/DBusMenu, icon assets, releases URL, Windows AppData/registry autostart |
| `tests/diagnostics_tests.rs` | 2 | 181 | Diagnostics from synthetic roots + live-system smoke test |
| `tests/fan_control_tests.rs` | 5 | 118 | `AppEvent` → `AppState` fan-mode transitions, offline daemon behavior, pending fan mode in-flight tracking |
| `tests/hardware_service_tests.rs` | 9 | 280 | `write_and_verify`, mock sysfs backend, presets, polkit/config assets |
| `tests/linux_gpu_tests.rs` | 7 | 175 | DRM enumeration, nvidia-smi parsing, NvidiaSampler watcher thread, stale reading expiry |
| `tests/linux_procfs_tests.rs` | 4 | 89 | `/proc/stat`, `/proc/meminfo`, `/proc/cpuinfo` parsing, live provider smoke |
| `tests/logging_tests.rs` | 1 | 91 | Rotating file sink + double-init safety |
| `tests/mock_hardware_tests.rs` | 3 | 155 | `MockScenario::{Nominal, ZeroCapabilities, SensorDataGap}` |
| `tests/packaging_tests.rs` | 5 | 172 | Embedded assets match disk, root enforcement, prefix install/uninstall |
| `tests/power_preset_tests.rs` | 6 | 256 | ACPI profile selection, combined presets, dual-leg partial success honesty, diagnostics |
| `tests/rgb_tests.rs` | 9 | 422 | Hex validation, presets, OpenOMEN migration, 3 interface variants, verified writes, restore-previous |
| `tests/sensor_registry_tests.rs` | 11 | 299 | Sensor priority matrix, fan discovery, sysfs battery presence detection, generic PWM guard |
| `tests/telemetry_history_tests.rs` | 5 | 111 | 120-sample bound, SVG path generation/gaps, dynamic capacity window resizing |
| `tests/windows_service_tests.rs` | 8 | 291 | Named pipe framing, 64 KB payload guard, OMEN/Victus board profiles, fan mode authorization, mutation gating, mock client-server, non-Windows guards |
| `tests/windows_telemetry_tests.rs` | 8 | 190 | WindowsCpuTimes delta math, clock warp / zero delta guards, RAM math, battery status parsing, DXGI vendors, GPU/VRAM distinction, sensor guarantees |

**Unit test modules in `src/` (`#[cfg(test)] mod tests`):**

| File | Tests | Focus |
|------|------:|-------|
| `src/core/state.rs` | 10 | `AppState::apply`, status copy, degradation, history resize |
| `src/core/telemetry.rs` | 6 | History eviction, dynamic capacity, SVG path commands/gaps/NaN |
| `src/core/worker.rs` | 2 | Single in-flight mutation gate, in-flight guard lifetime across timeouts |
| `src/service/polkit.rs` | 1 | Polkit response tuple serialization `(bba{ss})` |
| `src/service/protocol.rs` | 6 | Length-prefixed framing, oversized/truncated frame rejection, zero request ID rejection, typed errors |
| `src/desktop/system_integration.rs` | 1 | Daemon binary install/overwrite behavior |
| `src/hardware/unavailable.rs` | 1 | Zero capabilities, sampling refused |
| `src/hardware/linux/cpu.rs` | 5 | `/proc/stat` parsing, delta utilization, model/freq parsing |
| `src/hardware/linux/gpu.rs` | 3 | Card-name filter, nvidia-smi parse, PCI vendor IDs |
| `src/hardware/linux/memory.rs` | 2 | MemAvailable-based used memory, missing-key behavior |
| `src/hardware/linux/hwmon.rs` | 3 | Millidegree conversion, fan RPM, label trimming |

## Test File Organization

**Location:**
- Pure parsing, protocol serialization, and state logic is unit-tested co-located with the code in `#[cfg(test)] mod tests` at the bottom of the same file (all 11 modules above).
- Cross-module, filesystem, thread, protocol round-trip, and binary-level behavior lives in `tests/<area>_tests.rs` integration suites.
- No `tests/common/` shared module and no `tests/fixtures/` directory. Helpers are duplicated per suite by design so each suite is self-contained.

**Naming:**
- Files: `<area>_tests.rs` (18 suites).
- Unit test functions: descriptive sentences, no `test_` prefix (`a_full_sample_reports_every_expected_sensor`, `reports_nothing_rather_than_guessing`, `oversized_frame_is_rejected_before_allocation`).
- Integration test functions: mixed. Suites use descriptive sentences or domain `test_` prefixes (`tests/windows_service_tests.rs`, `tests/windows_telemetry_tests.rs`, `tests/desktop_integration_tests.rs`). Prefer descriptive names for new tests.
- Requirement-driven suites use ordinal prefixes: `degradation_01_...` … `degradation_08_...` (`tests/degradation_tests.rs`).

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

(Source: `src/core/state.rs:337-349`.)

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
- Every integration suite starts with a `//!` module doc listing what it verifies.
- Arrange–act–assert with numbered comments for multi-step state transitions.
- Builders are local helper functions/structs at the top of the file, below imports.
- Setup/teardown is RAII, not `setup()`/`teardown()` functions: `Drop for TestDir` or explicit cleanup at the end.
- Assertions almost always carry a message, especially when asserting absence or hardware facts.

## Mocking & Testing Strategies

**Framework:** None. Hand-written test doubles and dependency injection only.

### Primary Provider Double — `MockHardware`
```rust
let mock = MockHardware::new(MockScenario::Nominal);
let sample = mock.sample_telemetry().expect("Nominal sample must succeed");
```
- `Nominal`: all capabilities, sine-wave readings.
- `ZeroCapabilities`: zero flags, `TelemetrySnapshot::default()`, no fabricated numbers.
- `SensorDataGap`: nominal, but GPU utilization/temperature become `None` for 3 of every 10 ticks.
- `MockScenario::from_env()` reads `ZENOMEN_MOCK_SCENARIO` (`nominal`, `zero`/`zerocapabilities`, `gap`/`sensor-data`).

### Filesystem & Sysfs Doubles — Inject a Root
```rust
let root = std::env::temp_dir().join(format!("zenomen_test_hw_{test_name}_{}", std::process::id()));
let hwmon_dir = root.join("sys/devices/platform/hp-wmi/hwmon/hwmon5");
fs::create_dir_all(&hwmon_dir).expect("failed to create mock hwmon dir");
fs::write(hwmon_dir.join("name"), "hp\n").unwrap();
fs::write(hwmon_dir.join("pwm1_enable"), "2\n").unwrap();

let backend = HardwareBackend::new(&root);
```

### Windows Testing Strategy
Windows platform code is rigorously tested in headless Linux environments without requiring a Windows kernel or live Win32 APIs:
- **Mock Named Pipe IPC:** Framing and transport semantics are tested via in-memory byte streams using `encode_frame` and `decode_frame`. Full request-response lifecycles (`HardwareRequest` → `handle_request` → `HardwareResponse`) execute in `tests/windows_service_tests.rs` without opening OS pipe handles.
- **Mock Security Descriptors & Buffer Guards:** Protocol tests verify length-prefixed payload safety, rejecting payloads exceeding `MAX_PAYLOAD_SIZE` (64 KB) and truncated headers before memory allocation (`src/service/protocol.rs`, `tests/windows_service_tests.rs`). Security descriptors are validated against the canonical SDDL string `PIPE_SDDL`.
- **Board Validation & Mutation Gating:**
  - `resolve_board_profile` tested against known HP motherboard IDs (`88F8` OMEN 15/16 AMD, `8A24` Intel case-insensitivity, `8A3E` Victus) and rejecting unknown/empty strings (`tests/windows_service_tests.rs`).
  - Request authorization matrix (`authorize_mutation`) validated to permit auto/max fan modes and reject unauthorized modes (`manual`, `extreme_turbo`) with typed `HardwareControlError` variants.
  - Unvalidated hardware mutation blocking strictly enforces `BoardValidation` gating, preventing unvalidated writes or mismatched board mutations.
- **Mock Telemetry Math & Sensor Guarantees:**
  - **GetSystemTimes CPU Delta Utilization Math & Clock Warp Guards:** `tests/windows_telemetry_tests.rs` feeds synthetic `WindowsCpuTimes` (kernel, user, idle) into `calculate_cpu_utilization`, verifying nominal calculation (66.67%), 100% busy, 0% idle, zero delta guards (`None`), and backward clock warp protection.
  - **GlobalMemoryStatusEx RAM Math:** `calculate_ram_usage` tested with 16 GB total / 6 GB available, zero values, and saturation when available exceeds total.
  - **Battery Status Parsing:** `tests/windows_telemetry_tests.rs` verifies `parse_battery_status`, testing charging, discharging, desktop missing battery (flag 128 -> `None`), unknown lifetime (`0xFFFFFFFF`), and guarding against battery flag 255 (marking charging as `None` rather than a false positive `Some(true)`).
  - **DXGI Vendor ID Recognition:** `tests/windows_telemetry_tests.rs` asserts PCI vendor ID mapping (`0x10DE` -> Nvidia, `0x1002` -> AMD, `0x8086` -> Intel, unknown -> `GpuVendor::Unknown`).
  - **GpuPerformanceCollector & VRAM Distinction:** Dedicated VRAM total is reported honestly, while used VRAM returns honest `None` when unqueried; absent adapter handled cleanly.
  - **Structured Sensor Telemetry Guarantees:** Validates sensor source enumeration (`Win32`, `DXGI`, `PDH`) and temperature/fan sensor fallback structures ensuring honest absence reporting without fabricating fake metrics.
- **Non-Windows Lifecycle & Server Interface Missing Guards:** Validates non-Windows lifecycle functions (`register_service`, `uninstall_service`, `run_service`, `run_console_service`) and server pipe entry points (`create_server_pipe`, `run_pipe_server`) return `HardwareControlError::InterfaceMissing` instead of failing unsafely (`tests/windows_service_tests.rs`).

### External Command Double — Fake Executable
```rust
fn fake_nvidia_smi(dir: &std::path::Path, body: &str) -> PathBuf {
    let path = dir.join("fake-nvidia-smi");
    fs::write(&path, format!("#!/bin/sh\n{body}\n")).expect("write fake tool");
    fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).expect("chmod fake tool");
    path
}

let sampler = NvidiaSampler::new(fake_nvidia_smi(dir.path(), "echo '...'"));
```

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

Used in `src/hardware/linux/hwmon.rs`, `tests/linux_gpu_tests.rs`, `tests/diagnostics_tests.rs`, `tests/sensor_registry_tests.rs`, `tests/logging_tests.rs`.

**2. `temp_test_dir(name)` helper + manual cleanup** — nanosecond uniqueness, caller cleans via `let _ = fs::remove_dir_all(&dir);` (`tests/config_tests.rs`, `tests/rgb_tests.rs`).

**3. `MockSysfs` builder with PID-based root** — removes stale directory before creating, cleans up in `Drop` (`tests/hardware_service_tests.rs`, `tests/power_preset_tests.rs`).

## Coverage

**Requirements:** None enforced. No coverage tool, config, threshold, or CI step (`tarpaulin`, `llvm-cov`, `codecov` are absent).

## Test Types

**Unit Tests:**
- Scope: pure parsers, protocol frames, state transitions, SVG generation, serialization, single-module edge cases.
- Location: bottom-of-file `#[cfg(test)] mod tests`; 40 tests across 11 modules.
- Approach: direct calls, fixture strings, synthetic times, no live filesystem except hwmon reader tests (temp dir).

**Integration Tests:**
- Scope: multi-module flows (worker + provider + channels), filesystem/sysfs trees, fake executables, desktop/autostart behavior, packaging assets, Windows protocol round-trips, degradation matrix.
- Location: `tests/*.rs`; 113 tests across 18 suites.
- Approach: public `zenomen::` API, injected roots, `MockHardware`, `AppState::apply`.

**E2E Tests:**
- Not used. There is no test that starts the Slint window or a real system-wide D-Bus session. Interface methods are called directly.

**Async Tests:**
- None (`#[tokio::test]` / async test count is 0). Async code is confined to Linux zbus contract methods and tested through sync backends and serialization tests.

## Common Patterns

**Concurrency and timing:**
- Use `mpsc::Receiver::recv_timeout` with short budgets (100–500 ms) for worker events (`tests/benchmark_tests.rs`).
- Use deadline loops instead of a single sleep when waiting for a background thread.
- Fixed sleeps are used only where behavior is explicitly time-based: stale GPU sample expiry (`3_100 ms`, `tests/linux_gpu_tests.rs`) and fake-tool startup (10–200 ms).

**Error testing:**
- Assert the error variant when the variant is the contract (`tests/packaging_tests.rs`, `tests/windows_service_tests.rs`); otherwise assert `is_err()` and inspect the message.
- For “no fabricated data” contracts, assert `is_none()`/`is_empty()`/`0` explicitly.

**State-machine testing:**
- Drive `AppState` only with events and assert both confirmed and in-flight fields after each step (`tests/fan_control_tests.rs`, `tests/power_preset_tests.rs`).

## CI Checks

Single `quality` job on `ubuntu-latest` (`.github/workflows/ci.yml`), least-privilege `contents: read`, concurrency cancellation, all gates blocking:
1. `git diff --check`
2. `cargo fmt --all -- --check`
3. `cargo clippy --all-targets --all-features -- -D warnings`
4. `cargo check --target x86_64-pc-windows-msvc --no-default-features`
5. `cargo check --target x86_64-pc-windows-msvc --bin zenomen-hw`
6. `xvfb-run -a cargo test --all` (or `xvfb-run --auto-servernum cargo test --all`)
7. `./scripts/validate-icons.py`
8. `cargo audit` against the committed `Cargo.lock`

## Known Gaps

- **Slint bridge untested:** `src/app.rs` has no `#[cfg(test)]` module and no integration test. Callback wiring, command queue saturation, and `invoke_from_event_loop` publication are unverified.
- **Entry points untested:** `src/main.rs` and `src/bin/zenomen-hw.rs` have no tests (startup, provider selection, CLI flags, daemon connection).
- **D-Bus interface methods untested:** `src/service/contract.rs` has no integration test; only the backend, polkit serialization, and protocol framing are tested.
- **Live-system assumptions:** `tests/linux_procfs_tests.rs` and `tests/benchmark_tests.rs` assert against the real `/proc`; tests run in Linux CI.
- **Count synchronization:** Total test count must be kept synchronized across `README.md`, `CONVENTIONS.md`, and `TESTING.md` when altering test suites (AGENTS.md rule 10).

**Checklist for new tests (per AGENTS.md rule 7):**
1. Add a unit test in the same file for pure logic; add/extend a `tests/<area>_tests.rs` suite for cross-module behavior.
2. Use `MockHardware` or inject a temp root; never touch live hardware, `/sys`, or root paths.
3. Create a uniquely named temp dir and clean it via `Drop` (copy the `TestDir` pattern).
4. Assert with a message that explains the hardware/state contract being protected.
5. Run `git diff --check`, `cargo fmt --all -- --check`, `cargo clippy --all-targets --all-features -- -D warnings`, `cargo check --target x86_64-pc-windows-msvc --no-default-features`, `cargo check --target x86_64-pc-windows-msvc --bin zenomen-hw`, `xvfb-run -a cargo test --all`, and `./scripts/validate-icons.py`.
6. Update the test counts in documentation if the total changes.

---

*Testing analysis: 2026-11-08*
