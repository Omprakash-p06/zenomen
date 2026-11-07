# Plan 06-02: Cross-Platform Domain Contracts & Seams Summary

**Status:** Completed
**Phase:** 06-cross-platform-windows-linux-release
**Wave:** 2

## Overview
Plan 06-02 formalized and decoupled the cross-platform telemetry contracts, expanded `TelemetrySnapshot` with real battery metrics, enforced strict root isolation in sensor capability detection, aligned fan control discovery with the privileged daemon, and resolved the settings history window duration calculation and dynamic scaling.

## Delivered Artifacts & Fixes

1. **Cross-Platform Battery Telemetry Contract (`src/hardware/provider.rs` & `src/hardware/mod.rs`)**:
   - Added `BatteryTelemetry` struct with `present`, `charge_percent: Option<u8>`, `charging: Option<bool>`, and `remaining_seconds: Option<u32>`.
   - Added `battery: Option<BatteryTelemetry>` field to `TelemetrySnapshot` (defaulting to `None`).
   - Added `has_battery_metrics(&self) -> bool` helper asserting presence and measured charge level.
   - Re-exported `BatteryTelemetry` in `crate::hardware`.
2. **Linux Battery Telemetry Sampling (`src/hardware/linux/provider.rs`)**:
   - Added `power_supply_root: PathBuf` to `LinuxHardwareProvider` allowing path injection in tests.
   - Implemented `sample_battery_telemetry(&Path)` to enumerate `/sys/class/power_supply/BAT*`, reading `capacity` (clamped 0..=100), `status` (`Charging` -> `Some(true)`), and `time_to_empty_now`/`time_to_full_now` (converted to seconds).
   - If no battery device is found, returns `None` without fabricating dummy values.
3. **Mock Provider Battery Coverage (`src/hardware/mock.rs`)**:
   - Nominal scenario emits 85% battery, non-charging, 10800s remaining.
   - ZeroCapabilities scenario emits `battery: None`.
   - SensorDataGap scenario alternates battery availability during gap intervals.
4. **SensorRegistry Root Isolation & Fan Predicate Alignment (`src/hardware/registry.rs`)**:
   - Stored `sysfs_root: PathBuf` in `SensorRegistry` derived from the scanned root hierarchy.
   - Added `pub fn sysfs_root(&self) -> &Path` accessor.
   - Updated `detect_capabilities` to pass `&self.sysfs_root` to `detect_rgb_interface`, ensuring isolated tests never probe the host `/sys`.
   - Aligned `resolve_fans` to strictly gate `has_fan_control` to HP platform interfaces (`hp`, `hp-wmi`, or `platform`), preventing generic hwmon chips (`coretemp`) with `pwm1_enable` from enabling fan control.
5. **History Window Dynamic Scaling & SVG Path Caching (`src/core/telemetry.rs` & `src/core/state.rs`)**:
   - Added `duration_seconds: u32` tracking to `TelemetryHistory`.
   - Added `capacity_for_duration(duration_seconds, interval_ms)`: calculates `(duration_seconds * 1000 / interval_ms).max(2)` so capacity dynamically preserves the visible time window across polling rate changes.
   - Added `new_with_duration` and `update_interval`.
   - Added `generate_chart_paths_cached` and `generate_sparkline_paths_cached` caching SVG paths until samples are appended or capacity changes.
   - Updated `AppState` on `TelemetryUpdated`, `HistoryWindowChanged`, and `ConfigLoaded` to use cached path rendering and preserve duration.

## Verification & Test Results

1. **New Tests Added**:
   - `tests/sensor_registry_tests.rs`:
     - `test_rgb_capability_detection_stays_within_custom_root`: Asserts custom root without HP RGB files reports `rgb_lighting: false` regardless of host hardware.
     - `test_generic_hwmon_pwm_does_not_enable_hp_fan_control`: Asserts generic hwmon `coretemp` device with `pwm1_enable` reports `fan_control: false`.
   - `tests/telemetry_history_tests.rs`:
     - `test_history_capacity_adjusts_dynamically_to_preserve_time_window`: Asserts 120s duration scales to 120 samples at 1000ms, 240 samples at 500ms, and 60 samples at 2000ms.
   - `tests/mock_hardware_tests.rs`:
     - Verified nominal battery telemetry values (85%, non-charging, 10800s).
     - Verified zero capabilities emits `None` without panic.
     - Verified gap scenario emits `None` during gap ticks.
2. **Full Pipeline Verification**:
   - `cargo fmt --all -- --check`: Passed with 0 discrepancies.
   - `git diff --check`: Passed with 0 whitespace errors.
   - `cargo clippy --all-targets --all-features -- -D warnings`: Passed with 0 warnings.
   - `xvfb-run -a cargo test --all`: All 128 automated tests passed across 16 test suites.
