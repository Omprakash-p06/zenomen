---
phase: 03-verified-hardware-control
plan: "01"
subsystem: service/dbus
tags:
  - rust
  - dbus
  - zbus
  - polkit
  - sysfs
  - hardware-service
  - systemd

requires:
  - 02-03
provides:
  - "Privileged system daemon zenomen-hw on D-Bus com.zenomen.Hardware1"
  - "Mandatory Polkit authorization com.zenomen.hardware.control on mutating methods"
  - "Verified sysfs write loop with exponential backoff (50ms, 100ms, 200ms) and read-back validation"
  - "Strict parameter validation rejecting traversal, arbitrary strings, and unsupported manual mode"
  - "Structured PresetResult reporting granular execution without false atomic rollback claims"
  - "Sandboxed systemd unit, D-Bus system policy, and polkit action configuration"
affects:
  - 03-02 (D-Bus client, fan mode control, Cooling screen)
  - 03-03 (ACPI power profiles, combined presets, Performance screen, diagnostics)

actuals:
  tasks: 5
  tests: 9 added (61 total passing)

tech-stack:
  added:
    - "zbus v5.19 with blocking and async support"
    - "src/service/types.rs (FanMode, PresetResult, HardwareControlError)"
    - "src/service/backend.rs (HardwareBackend, write_and_verify)"
    - "src/service/polkit.rs (PolkitVerifier, com.zenomen.hardware.control)"
    - "src/service/contract.rs (HardwareService with #[zbus::interface(name = com.zenomen.Hardware1)])"
    - "src/bin/zenomen-hw.rs (Privileged daemon binary)"
    - "data/dbus/com.zenomen.Hardware1.conf (D-Bus security policy)"
    - "data/polkit/com.zenomen.hardware.policy (Polkit policy)"
    - "data/systemd/zenomen-hw.service (Sandboxed systemd service)"
    - "tests/hardware_service_tests.rs"
  patterns:
    - "Two-process privilege separation: unprivileged GUI + sandboxed D-Bus daemon"
    - "Mandatory polkit check on all mutating methods (SVC-03)"
    - "Verified write backoff loop detects sticky registers and fails with VerificationFailed (COOL-04, PWR-04)"
    - "Granular PresetResult reports partial success honestly (PWR-06)"
---

# Plan 03-01: `zenomen-hw` Service — Formalized D-Bus Contract, Mandatory Polkit & Verified Writes Summary

`zenomen-hw` privileged service and core verified hardware control engine have been implemented and verified:
1. **Privilege Separation & Sandboxed Daemon (SVC-01, SVC-02, SVC-05):**
   - The GUI executable `zenomen` remains unprivileged.
   - Privileged operations live in `src/bin/zenomen-hw.rs` connecting to the system D-Bus at `com.zenomen.Hardware1`.
   - Packaged with security policies: D-Bus configuration `data/dbus/com.zenomen.Hardware1.conf` and hardened systemd service `data/systemd/zenomen-hw.service` (`ProtectSystem=strict`, `ProtectHome=true`, `ReadOnlyPaths=/`, `ReadWritePaths=/sys/devices/platform/hp-wmi /sys/firmware/acpi`).
2. **Mandatory Polkit Authorization (SVC-03):**
   - Implemented `PolkitVerifier` querying `org.freedesktop.PolicyKit1.Authority` for action `com.zenomen.hardware.control`.
   - Every mutating method (`SetFanMode`, `SetPowerProfile`, `ApplyPreset`, `ApplyRgbProfile`, `SetRgbEnabled`) enforces polkit authorization before dispatching to hardware.
3. **Formalized Contract & Whitelist Input Validation (SVC-04):**
   - Exposes `com.zenomen.Hardware1` endpoints: `GetCapabilities`, `GetFanStatus`, `SetFanMode`, `GetPowerProfiles`, `GetPowerProfile`, `SetPowerProfile`, `ApplyPreset`, `ApplyRgbProfile`, `SetRgbEnabled`.
   - Whitelist validation rejects directory traversal, null bytes, newlines, and unsupported manual fan modes without curves (`COOL-03`).
4. **Verified Sysfs Write Engine (COOL-04, PWR-04, PWR-06):**
   - Implemented `write_and_verify` with exponential backoff (50ms, 100ms, 200ms) comparing read-back value.
   - Fails honestly with typed `VerificationFailed` on mismatch rather than optimistic success.
   - Presets return structured `PresetResult` reporting `power_applied`, `fan_applied`, `partial_success`, and explanatory message.
5. **Testing & Verification:**
   - Authored 9 unit and integration tests in `tests/hardware_service_tests.rs` with simulated sysfs environments.
   - All 61 tests pass cleanly across 9 test suites.
