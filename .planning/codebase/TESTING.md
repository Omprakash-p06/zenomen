# Testing Patterns

**Analysis Date:** 2026-11-09

## Test Framework

**Runner:**
- Standard Rust built-in `libtest` via `cargo test` (no external runners such as `nextest` or custom harnesses).
- Rust **edition 2024**, stable toolchain.
- Headless execution: Linux CI runs within `xvfb-run` to satisfy Slint/Winit dependencies (`.github/workflows/ci.yml`).

**Assertion Library:**
- Standard Rust assertion macros: `assert!`, `assert_eq!`, `assert_ne!`, `matches!`.
- Diagnostic unpacking via `Result::expect`, `Result::expect_err`, and `Option::expect`.
- Assertions state the requirement or invariant in message strings ("Nominal must report CPU metrics", "Fan RPMs must be empty (no tachometer on this platform)").

**Key Test Commands:**
```bash
# Run all passing tests across unit and integration suites
cargo test --all

# Run unit tests only (co-located #[cfg(test)] in src/lib.rs)
cargo test --lib

# Run a specific integration test suite
cargo test --test windows_service_tests
cargo test --test hardware_service_tests
cargo test --test mock_hardware_tests

# Run a specific test by function name
cargo test test_fan_mode_state_transitions

# Quality and lint gates (enforced in CI)
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo audit
```

**CI Gates (`.github/workflows/ci.yml`):**
- `linux-quality` (Ubuntu runner): `cargo fmt`, `cargo clippy --all-targets --all-features -- -D warnings`, `xvfb-run --auto-servernum cargo test --all`, `cargo audit`.
- `windows-quality` (Windows runner): `cargo fmt`, `cargo clippy --all-targets --all-features -- -D warnings`, `cargo test --all`.
- **Clippy Zero-Warning Policy:** Both CI pipelines enforce `-D warnings` across `--all-targets` and `--all-features`. Zero unapproved warnings are tolerated.

## Test Organization & Suite Inventory

The ZenOmen repository defines **182 total test functions** across unit and integration test locations:
- **40 Unit Tests:** Co-located in `src/` inside `#[cfg(test)] mod tests` across 11 modules.
- **142 Integration Tests:** Distributed across **18 integration test files** in `tests/`.

### Execution Counts and Platform Filtering
Because Linux-specific suites gate on `#[cfg(target_os = "linux")]` and certain host-dependent tests check environment preconditions, test execution counts vary predictably by platform:
- **Windows Host:** **156 passed** (40 unit + 116 integration; 0 failed). Linux procfs, GPU, and logging suites compile to 0 tests via target attributes.
- **Linux Host / CI:** **154–155 passing tests** typical across unit and the 18 integration suites when host daemon guards (`tests/fan_control_tests.rs`) or environment conditions skip, extending up to 173 tests when all sysfs and mock suites run on a headless runner.

### Co-located Unit Tests (`src/lib.rs` - 40 tests)
Unit tests reside in their corresponding source files and test private helper functions and state transitions:
1. `src/core/state.rs` (10 tests): `AppState::apply` event folding, full snapshot processing, partial sample degradation, capability flag tracking, status text formatting, history resize.
2. `src/core/telemetry.rs` (6 tests): History capacity bounds, dynamic resizing, oldest sample eviction, SVG sparkline path rendering, gap detection on `None`/`NaN`/`Inf`.
3. `src/core/worker.rs` (2 tests): Worker mutation concurrency guard (rejecting second in-flight mutation), timeout guard persistence.
4. `src/service/protocol.rs` (6 tests): Frame encoding/decoding, request roundtrips, typed error preservation, RGB profile serialization, oversized frame rejection (>64 KB), truncated frame rejection, zero-request ID validation.
5. `src/hardware/linux/cpu.rs` (5 tests): `/proc/stat` parsing, jiffies calculation, delta utilization percentage, CPU model and frequency extraction.
6. `src/hardware/linux/gpu.rs` (3 tests): `nvidia-smi` XML/text parser, PCI vendor ID recognition, connector name filtering.
7. `src/hardware/linux/hwmon.rs` (3 tests): Fan RPM integer parsing, millidegree to Celsius conversion, sensor label string trimming.
8. `src/hardware/linux/memory.rs` (2 tests): `/proc/meminfo` parsing, computing used memory from `MemAvailable`.
9. `src/hardware/unavailable.rs` (1 test): `UnavailableHardwareProvider` reports honest absence rather than guessing.
10. `src/desktop/system_integration.rs` (1 test): Daemon binary installation and file copy verification.
11. `src/service/polkit.rs` (1 test): Polkit D-Bus response signature validation.

### Integration Test Suites (`tests/*.rs` - 18 suites, 142 tests)

| Suite File | Tests (Def / Win) | Primary Responsibilities & Target Contracts |
|:---|:---:|:---|
| `benchmark_tests.rs` | 5 / 4 | Headless worker tick duration (<5ms), memory RSS readings (`read_memory_metrics`), UI visibility gating (pausing samples when hidden), clean shutdown on `AppCommand::Shutdown`. (*1 test is Linux-only*). |
| `config_tests.rs` | 6 / 6 | Config serialization/deserialization, atomic file saves (`save_atomic`), defaults fallback on missing file, corrupted config file backup and recovery. |
| `degradation_tests.rs` | 8 / 8 | Specification requirement matrix (degradation cases 01–08): missing fan hwmon, absent `pwm_enable`, missing CPU sensor, missing `nvidia-smi`, absent RGB zones, missing hardware daemon, unauthorized fan writes, corrupt configuration. |
| `desktop_integration_tests.rs` | 12 / 12 | XDG autostart `.desktop` generation, Windows Registry Run key escaping, LocalAppData resolution, application icon asset validation, release URL safety, StatusNotifierItem & DBusMenu properties, single-instance dispatch. |
| `diagnostics_tests.rs` | 2 / 2 | Diagnostics collection on live host without panicking, and diagnostics bundle generation against synthetic mock roots. |
| `fan_control_tests.rs` | 5 / 5 | Fan mode state transitions (`Auto`, `Max`), in-flight pending fan mode indicators, clearing pending mode on verification failure, graceful handling of offline daemon. |
| `hardware_service_tests.rs` | 9 / 9 | Sysfs verified writes (`write_and_verify`), mismatch detection, IO error handling, mock backend power profile switching, fan mode control, combined preset execution. |
| `linux_gpu_tests.rs` | 7 / 0 | *Gated with `#![cfg(target_os = "linux")]`*. Tests GPU vendor ID mapping, NVIDIA SMI parser outputs, sysfs card naming, connector filtering. |
| `linux_procfs_tests.rs` | 4 / 0 | *Gated with `#![cfg(target_os = "linux")]`*. `/proc/stat` and `/proc/meminfo` synthetic string parsing, jiffies math, memory calculation. |
| `logging_tests.rs` | 1 / 0 | *Gated with `#![cfg(not(target_os = "windows"))]`*. Log file creation, daily rotation, and isolation under custom `XDG_STATE_HOME`. |
| `mock_hardware_tests.rs` | 3 / 3 | In-tree mock provider scenarios: Nominal provider sensor ranges, ZeroCapabilities empty flags, SensorDataGap periodic `None` emissions. |
| `packaging_tests.rs` | 5 / 5 | Root privilege enforcement, Debian package file mappings in `Cargo.toml`, embedded asset synchronization against physical `data/` files, atomic file writer. |
| `power_preset_tests.rs` | 6 / 6 | Combined power and fan presets, partial success reporting, dynamic enumeration of platform profiles, error handling when hardware interface is missing. |
| `rgb_tests.rs` | 9 / 7 | Four-zone sysfs writes, legacy keyboard LED writes, kernel patch interface, non-destructive OpenOmen migration, hex color validation, built-in color presets. (*2 tests Linux-only*). |
| `sensor_registry_tests.rs` | 11 / 10 | Hwmon driver priority matrix (selecting `k10temp` or `coretemp` over `acpitz`), HP-WMI fan control detection, battery presence measurement, custom root confinement. (*1 test Linux-only*). |
| `telemetry_history_tests.rs` | 5 / 5 | Ring buffer capacity bounding (strictly 120 samples), dynamic time-window resizing, SVG sparkline path construction with `M` break commands across gaps. |
| `windows_service_tests.rs` | 36 / 36 | Complete cross-platform Windows service test suite using `FakeWmiAdapter`: motherboard profile resolution (8A3D/8A3E read-only restriction vs validated boards), request authorization, WMI query error propagation, read-back verification failures, named pipe protocol roundtrips, 64 KB frame limits, truncated frame rejection. |
| `windows_telemetry_tests.rs` | 8 / 8 | Cross-platform mathematical unit tests for Windows telemetry: `FILETIME` delta to CPU % utilization, battery status parsing, RAM usage calculation, DXGI GPU vendor detection. |

## Mocking Strategies & Hardware Doubles

ZenOmen strictly avoids heavy third-party mocking crates (`mockall`, `wiremock`). All doubles are hand-written, deterministic, and safe for headless CI.

### 1. The WMI Adapter Double (`FakeWmiAdapter`)
Located in `src/service/windows/hp_wmi.rs`, `FakeWmiAdapter` is compiled into the core library (not feature-gated to `#[cfg(test)]`) so integration tests in `tests/` can drive the Windows WMI client and server stacks without Windows hardware:

```rust
pub trait WmiAdapter: Send + Sync {
    fn exec_method(&self, sign: u32, req_type: u32, in_buf: &[u8; 140])
        -> Result<Vec<u8>, HardwareControlError>;
    fn is_available(&self) -> bool;
}

// In tests:
let adapter = FakeWmiAdapter::new(true);
adapter.expect_call(fan_mode_response(0x00)); // Step 1: Pre-write read
adapter.expect_call(write_ack());              // Step 2: Firmware write
adapter.expect_call(fan_mode_response(0x01));  // Step 3: Post-write verify read
let conn = HpWmiConnection::from_adapter(adapter);
```

`FakeResponse` models three outcomes:
- `FakeResponse::Success { payload }`: Successful firmware execution with a 136-byte output buffer.
- `FakeResponse::FirmwareError { status }`: Emulates BIOS firmware rejection codes:
  - `0x01`: Unsupported / unknown command.
  - `0x04`: Access denied / authorization failed (BIOS password locked).
- `FakeResponse::WmiError { message }`: Emulates COM/WMI transport failures.

**Three-Step Mutation Protocol:**
Any test testing hardware mutation (such as `set_fan_mode`) must queue three consecutive expectations to satisfy the read-write-verify sequence. If readback disagrees with the requested mode, `HardwareControlError::VerificationFailed` is asserted.

### 2. In-Tree Mock Hardware Provider (`MockHardware`)
Defined in `src/hardware/mock.rs`, `MockHardware` implements `HardwareProvider` for developer machines and integration testing:
- **`MockScenario::Nominal`:** Reports all capabilities (`HardwareCapabilities::all_supported()`). Telemetry values oscillate in smooth sine waves (CPU 15–45%, GPU 30–70%, fans 2400–3800 RPM, battery 85% discharging) to exercise chart renderers.
- **`MockScenario::ZeroCapabilities`:** Reports no capabilities (`HardwareCapabilities::none()`). Emits `TelemetrySnapshot::default()` with empty collections and `None`, verifying that UI screens render honest unavailability without fabricating zeros.
- **`MockScenario::SensorDataGap`:** Simulates transient sensor dropouts. Follows nominal values, but drops GPU utilization, GPU temperature, and battery metrics (`None`) for 3 ticks out of every 10 (`GAP_PERIOD_TICKS = 10`, `GAP_TICKS = 3`).

Directly verified in `tests/mock_hardware_tests.rs` and selectable at runtime via `ZENOMEN_MOCK_SCENARIO` (`nominal`, `zero`, `gap`).

### 3. Filesystem Root Injection
Linux hardware backends accept custom root paths rather than hardcoding `/sys`:
- `HardwareBackend::new(&root)` accepts an arbitrary path where tests build simulated sysfs trees.
- `SensorRegistry::scan_roots(&hwmon_root, &acpi_root)` scans injected temp roots.
- RAII cleanup structs (`MockSysfs`, `TestHwmonTree`, `TestDir`) ensure directories under `std::env::temp_dir()` are cleanly unlinked on test completion.

### 4. Pure In-Memory Parser Doubles
Parsing routines take synthetic string slices or byte buffers:
- `/proc/stat` and `/proc/meminfo` synthetic buffers in `tests/linux_procfs_tests.rs`.
- `nvidia-smi` text outputs in `tests/linux_gpu_tests.rs`.
- Simulated Windows `FILETIME` and `SYSTEM_POWER_STATUS` structures in `tests/windows_telemetry_tests.rs`.

## Test Structure & Patterns

**State-Machine Assertions:**
Tests drive `AppState::apply` with concrete `AppEvent` sequences and assert every intermediate transition:
```rust
let mut state = AppState::default();
state.apply(&AppEvent::FanModeRequested("max".to_string()));
assert_eq!(state.pending_fan_mode.as_deref(), Some("max"));

// Post-write verification failure clears pending state
state.apply(&AppEvent::FanModeChanged(Err("verification mismatch".to_string())));
assert_eq!(state.pending_fan_mode, None);
assert_eq!(state.last_control_error.as_deref(), Some("verification mismatch"));
```

**Typed Error Assertions:**
Tests assert explicit error enum variants rather than inspecting error message strings:
```rust
let err = zenomen::service::windows::hp_wmi::set_fan_mode(&conn, "max")
    .expect_err("access denied must propagate");
assert!(
    matches!(err, HardwareControlError::AuthorizationFailed(_)),
    "expected AuthorizationFailed, got {err:?}"
);
```

**Board Validation and Authorization Testing:**
`tests/windows_service_tests.rs` asserts that unvalidated boards (such as `8A3D` or `8A3E`) only receive read capability flags, and that any mutation attempt is rejected before reaching WMI dispatch:
```rust
let validation = resolve_board_validation(Some("8A3D"));
assert!(!validation.allow_fan_writes);
assert!(!validation.allow_power_writes);

let err = authorize_mutation(&validation, "fan_mode")
    .expect_err("mutation on 8A3D must be blocked");
assert!(matches!(err, HardwareControlError::PermissionDenied(_)));
```

**Thread and Worker Testing:**
ZenOmen has no async executor. Concurrency is tested with `std::thread` and `std::sync::mpsc`:
- Worker threads are spawned, driven with commands over bounded channels, and joined cleanly.
- Worker latency and prompt shutdown are verified in `tests/benchmark_tests.rs`.

## Adding New Tests

1. **Unit tests:** Place in the same file under `#[cfg(test)] mod tests { use super::*; ... }` for internal parsers, state folding, and math.
2. **Integration tests:** Place in `tests/<area>_tests.rs` using only the public `zenomen::` crate API.
3. **Hardware mocking:** Always use `FakeWmiAdapter` for Windows or temporary directory trees for Linux sysfs. Never communicate with physical hardware.
4. **Platform gating:** Use `#[cfg(target_os = "linux")]` or `#[cfg(windows)]` when relying on OS-specific types. Where possible, write tests to be cross-platform using fakes (as done in `windows_service_tests.rs` and `windows_telemetry_tests.rs`).
5. **Zero warnings:** Run `cargo clippy --all-targets --all-features -- -D warnings` and `cargo fmt --all -- --check` before completing any changes.

---

*Testing analysis: 2026-11-09*
