<!-- refreshed: 2026-11-11 -->
# Testing Patterns

**Analysis Date:** 2026-11-11

## Test Framework

**Runner:**
- Built-in Rust test harness (`#[test]`), `cargo test`. No external test framework, no `proptest`, no snapshot library.
- Two layers:
  1. Co-located unit tests in `#[cfg(test)] mod tests` blocks inside `src/`.
  2. Integration suites in `tests/*.rs` that use the `zenomen` library crate (`use zenomen::...`).

**Assertion style:**
- `assert!`, `assert_eq!`, `assert_matches!`-equivalent via `matches!` + `assert!`.
- Failure messages are supplied when the value is not self-explanatory: `assert!(msg.contains("still in progress"), "message: {msg}")`.
- Tests assert domain invariants, not implementation details: no firmware write happens, a number is not fabricated, an authorization is refused, a frame round-trips.

**Run commands (from `README.md` and `.github/workflows/ci.yml`):**
```bash
# Full suite (GUI-rendering tests need a headless X server)
xvfb-run --auto-servernum cargo test --all

# Unit tests only
cargo test --lib

# One integration suite
cargo test --test fan_control_tests

# One test by name
cargo test --test windows_service_tests test_8a3d

# Quality gates (CI)
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo audit
```

**Counts (counted from `#[test]` attributes on 2026-11-11):**

| Layer | Files | Tests |
|---|---|---|
| Unit tests in `src/` | 15 | **84** |
| Integration suites in `tests/` | 19 | **238** |
| **Total** | | **322** |

> These counts are mirrored in `README.md`; update both whenever the suite inventory changes.

## Test Organization & Suite Inventory

### Co-located unit tests (`src/`)
| File | Tests | Focus |
|---|---|---|
| `src/service/windows/hp_wmi.rs` | 20 | envelope property planning, firmware status mapping, method selection, fake-adapter sequencing |
| `src/service/windows/victus_s.rs` | 15 | fan-table parsing, GPU offset, level validation, `pwm_to_level`, observation state machine |
| `src/core/state.rs` | 10 | `AppState::apply`, status text, degradation rules, sensor counting |
| `src/service/windows/wmi_ffi.rs` | 7 | VARIANT/SAFEARRAY/BSTR conversion and cleanup |
| `src/service/protocol.rs` | 6 | protocol helpers |
| `src/core/telemetry.rs` | 6 | bounded history, resize, SVG path generation |
| `src/hardware/linux/cpu.rs` | 5 | `/proc/stat` and `/proc/cpuinfo` parsing |
| `src/hardware/linux/hwmon.rs` | 3 | hwmon scanning and label reading |
| `src/hardware/linux/gpu.rs` | 3 | DRM/AMD sysfs parsing |
| `src/service/windows/wmi_diag.rs` | 2 | diagnostic probe formatting |
| `src/hardware/linux/memory.rs` | 2 | `MemTotal - MemAvailable` |
| `src/core/worker.rs` | 2 | single in-flight mutation, guard lifetime across timeout |
| `src/service/polkit.rs` | 1 | Polkit `(bba{ss})` response signature |
| `src/hardware/unavailable.rs` | 1 | zero capabilities, honest absence |
| `src/desktop/system_integration.rs` | 1 | daemon binary install/copy/overwrite |

### Integration suites (`tests/`)
| Suite | Tests | Focus |
|---|---|---|
| `windows_service_tests.rs` | 127 | pipe framing, board profiles/validation, `authorize_mutation`, WMI protocol sequences, 8A3D fan/power behavior, runbook state machine |
| `desktop_integration_tests.rs` | 12 | single instance, tray, autostart, icon/desktop assets |
| `sensor_registry_tests.rs` | 11 | canonical registry, CPU temp priority, fan discovery |
| `rgb_tests.rs` | 9 | variant discovery, presets, hex validation, migration, write verification |
| `hardware_service_tests.rs` | 9 | `HardwareBackend` read/write, `write_and_verify`, Polkit contract surface |
| `windows_telemetry_tests.rs` | 8 | CPU utilization math, RAM math, GPU vendor mapping, battery status parsing |
| `degradation_tests.rs` | 8 | missing sensor/interface/tool/daemon → explained, non-crashing state |
| `linux_gpu_tests.rs` | 7 | DRM enumeration, AMD telemetry, NVIDIA sample parsing (Linux-only) |
| `power_preset_tests.rs` | 6 | ACPI profile switching, combined presets, diagnostics service status |
| `config_tests.rs` | 6 | TOML roundtrip, atomic save, corrupt-file backup/recovery, clamping |
| `telemetry_history_tests.rs` | 5 | window bounds, eviction, path stability |
| `profile_orchestrator_tests.rs` | 5 | preset target resolution, refresh-rate helpers |
| `packaging_tests.rs` | 5 | embedded desktop entry/D-Bus/Polkit assets, install/uninstall under a prefix |
| `fan_control_tests.rs` | 5 | mode parsing, verified state transitions, UI state mapping |
| `benchmark_tests.rs` | 5 | RSS ceiling, tick duration, headless safety |
| `linux_procfs_tests.rs` | 4 | procfs parsers and provider sampling (Linux-only) |
| `mock_hardware_tests.rs` | 3 | `nominal`, `ZeroCapabilities`, `SensorDataGap` scenarios |
| `diagnostics_tests.rs` | 2 | report collection from injected roots, atomic JSON export |
| `logging_tests.rs` | 1 | single-subscriber setup (`#![cfg(not(target_os = "windows"))]`, intentionally one test) |

### Platform filtering
- Whole-file gates: `#![cfg(target_os = "linux")]` (`linux_gpu_tests.rs`, `linux_procfs_tests.rs`), `#![cfg(not(target_os = "windows"))]` (`logging_tests.rs`).
- Module gates: Windows-only modules (`service/windows/*`) and their tests compile everywhere only where the logic is platform-neutral; `wmi_ffi` is `#[cfg(windows)]` and its tests follow it.
- Windows service tests are designed to run on any host because they never touch COM or real firmware.

## Mocking Strategies & Hardware Doubles

### 1. WMI adapter double (`FakeWmiAdapter`)
- Location: `src/service/windows/hp_wmi.rs` (shared by `tests/windows_service_tests.rs`).
- Scripted queue: `expect_call(FakeResponse::Success { payload })`, `FirmwareError { status }`, `WmiError { message }`.
- Records every call (`RecordedCall { command, command_type, payload, out_size }`) so tests assert the exact command family (`HPWMI_GM`), opcode (`0x27`, `0x2D`, `0x2E`, `0x1A`) and payload bytes.
- `assert_all_consumed()` prevents a test from silently not exercising a path.
- Convention for mutations: pre-load **read → write → read** (pre-write read-back, write ack, post-write verification).

### 2. In-tree mock provider (`MockHardware`)
- Location: `src/hardware/mock.rs`; scenarios `Nominal`, `ZeroCapabilities`, `SensorDataGap`.
- Selected in production only via `ZENOMEN_MOCK_SCENARIO`; constructed directly in tests.

### 3. Filesystem root injection
- `SensorRegistry::scan_roots(hwmon_root, acpi_root)` — fixture trees.
- `LinuxHardwareProvider::with_all_roots(drm, cpufreq, hwmon, acpi)`.
- `HardwareBackend::new(root)` — sysfs prefix for write/verify tests.
- `collect_diagnostics_from_roots(...)` — `/etc/os-release`, kernel release, DMI root.
- `install_to_prefix(prefix)` / `uninstall_from_prefix(prefix)` — system integration without root.
- `resolve_app_data_paths_from(...)` — Windows path resolution.

### 4. Pure parser doubles
- Fan tables and fan levels are byte vectors (`table_payload` helper in `victus_s.rs` tests).
- `/proc/stat`, `/proc/meminfo`, `/proc/cpuinfo`, hwmon attributes, and DRM sysfs contents are injected as strings/temp files.
- Display and WMI FFI surfaces are tested through pure planning functions (`plan_hpq_input_envelope`, `method_for_output_size`, `map_disp_change_code`-adjacent logic) rather than real Win32 calls.

### 5. Time and concurrency
- Timeouts are shortened in tests (`execute_mutating_action_with_timeout(..., Duration::from_millis(30), ...)`).
- Thread coordination uses `AtomicBool` gates and `recv_timeout`, never fixed long sleeps beyond small settle delays.
- `InFlightGuard` is tested to stay active after a caller timeout and to release only when the operation truly finishes.

## Test Structure & Patterns

**Setup pattern (temp dir):**
```rust
let base_dir = std::env::temp_dir().join(format!("zenomen_test_install_{}", std::process::id()));
let _ = fs::remove_dir_all(&base_dir);
fs::create_dir_all(&base_dir).unwrap();
// ... exercise ...
let _ = fs::remove_dir_all(&base_dir);
```
Tests never write to `/sys`, `/usr`, or require root; destructive steps (install, uninstall) run against a prefixed temp tree.

**Behavioral assertion pattern:**
```rust
#[test]
fn a_second_mutation_is_rejected_while_one_is_in_flight() {
    // hold the first action open, submit a second, assert the rejection text,
    // release, assert the first completes, then assert the slot is reusable.
}
```

**Honesty assertions:**
- `unavailable_connection_reports_honest_absence` and `starts_waiting_for_a_reading_and_reports_nothing_supported` fail if a fabricated `0` or optimistic capability appears.
- `test_8a3d_*` tests assert that `0x2E {0,0}` is never sent and that power `ProfileUnverified` is reported rather than `ProfileVerified`.
- `degradation_tests.rs` removes each interface in turn and asserts the app still explains itself.

**Error-path testing:**
- Firmware statuses `2/3/4/5` map to specific `HardwareControlError` variants and are asserted directly.
- Oversized frames (`> 64 KiB`) are rejected on encode **and** decode.
- Polkit denial, unvalidated boards, mismatched BIOS versions, and unverified capabilities are each asserted to block before any write.

## Adding New Tests

1. Pick the layer:
   - Pure logic (parsers, state, protocol) → co-located `#[cfg(test)] mod tests`.
   - Cross-module behavior, filesystem fixtures, or public API → `tests/<area>_tests.rs`.
2. Reuse the existing doubles:
   - Windows firmware → `FakeWmiAdapter` with pre-loaded read/write/read.
   - Linux sysfs → fixture directory + injected root.
   - Provider behavior → `MockHardware` scenario.
3. Assert the invariant and the absence case, not just the happy path.
4. Keep global-state tests singular (see `logging_tests.rs`, which has exactly one test because the subscriber is process-wide).
5. Run `cargo fmt --all -- --check`, `cargo clippy --all-targets --all-features -- -D warnings`, and the affected suite before committing.
6. Update the counts and suite inventory in this document and in `README.md` when test totals change (AGENTS.md rule 10).

## Coverage Notes

- There is no `cargo-tarpaulin`/`llvm-cov` coverage job in CI; coverage is asserted by requirement traceability instead.
- Strongest coverage: Windows WMI protocol and board authorization (127 tests), state machine and status honesty, config recovery, packaging assets.
- Weakest coverage: end-to-end Linux D-Bus round-trip against a live bus, real COM paths in `RealWmiAdapter`, GUI rendering beyond "does not panic", and the dormant `src/ui/` scaffold (no behavioral tests).
- Hardware-in-the-loop evidence is out of band: `docs/testing/HP_BOARD_VALIDATION.md`, `zenomen-hw --validate-8a3d-fan`, `--validate-8a3d-power`, `--wmi-diag`.

---

*Testing analysis: 2026-11-11*
