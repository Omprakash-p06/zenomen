---
phase: 02-linux-telemetry-capability-detection
plan: "02"
subsystem: hardware/registry
tags:
  - rust
  - hwmon
  - sensor-registry
  - driver-priority
  - tachometers
  - capabilities

requires:
  - 02-01
provides:
  - "Canonical SensorRegistry unifying sensor path bindings across dashboard and diagnostics"
  - "Strict CPU temperature driver and label priority resolution (k10temp > coretemp > acpitz)"
  - "Dynamic hwmon tachometer discovery and PWM control interface detection"
  - "Accurate HardwareCapabilities reflecting detected host interfaces"
affects:
  - 02-03 (diagnostics report and export uses SensorRegistry to detail exact sensor bindings)

actuals:
  tasks: 3
  tests: 9 added (48 total passing)

tech-stack:
  added:
    - "src/hardware/linux/hwmon.rs (hwmon scanner, millidegree parser, fan rpm reader)"
    - "src/hardware/registry.rs (SensorRegistry, resolve_cpu_temp, resolve_fans)"
    - "tests/sensor_registry_tests.rs"
  patterns:
    - "Single shared SensorRegistry prevents heuristic drift between dashboard and diagnostics"
    - "Driver priority matrix strictly prioritizes k10temp/coretemp over acpitz"
    - "Absent hardware sensors yield None / Unavailable without fabricating zero values"
---

# Plan 02-02: Capability Detection & Single Shared SensorRegistry Summary

Plan 02-02 has delivered the canonical `SensorRegistry` and integrated it with `LinuxHardwareProvider`:
1. **Hwmon Scanner & Readers (MON-04, MON-12):**
   - Implemented `scan_hwmon_devices`, `read_temp_millidegrees`, `read_fan_rpm`, and `read_sensor_label` in `src/hardware/linux/hwmon.rs`.
2. **Strict CPU Temperature Priority Matrix (MON-04):**
   - Implemented in `src/hardware/registry.rs`:
     1. Priority 1: `k10temp` (AMD Zen) with `Tctl` / `Tdie` (or fallback `temp1_input`).
     2. Priority 2: `coretemp` (Intel) with `Package id 0` (or fallback `temp1_input`).
     3. Priority 3: `acpitz` (ACPI Thermal Zone fallback).
   - Never relies on arbitrary hwmon indices (`hwmon0`, `hwmon1`, etc.).
3. **Fan Tachometers & Hardware Capabilities (MON-08, MON-12):**
   - Enumerates fan tachometers from platform devices (`hp`, `hp-wmi`), discovering multiple fans sorted by index.
   - Detects `pwm1_enable` to establish `has_fan_control`.
   - Populates `HardwareCapabilities` accurately based on discovered interfaces.
4. **Provider Integration & Verification:**
   - Integrated `SensorRegistry` into `LinuxHardwareProvider`. Telemetry sampling and diagnostics now read identical paths from the shared registry.
   - Author comprehensive unit and integration tests in `tests/sensor_registry_tests.rs`. All 48 tests pass cleanly.
