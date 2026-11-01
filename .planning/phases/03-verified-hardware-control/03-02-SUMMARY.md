---
phase: 03-verified-hardware-control
plan: "02"
subsystem: ui/cooling
tags:
  - rust
  - slint
  - fan-control
  - dbus-client
  - verification
  - error-reporting

requires:
  - 03-01
provides:
  - "HardwareServiceClient D-Bus client layer for zenomen GUI"
  - "Verified fan mode control (Auto / Max) via AppCommand::SetFanMode"
  - "Cooling screen interactive Auto and Max buttons with verified state readout"
  - "Read-back mismatch and polkit denial error reporting banner with dismiss action"
  - "Capability and service connection gating with clear explanatory notices"
affects:
  - 03-03 (ACPI power profiles, combined presets, Performance screen, diagnostics)

actuals:
  tasks: 4
  tests: 3 added (64 total passing)

tech-stack:
  added:
    - "src/service/client.rs (HardwareServiceClient, ServiceConnectionStatus)"
    - "tests/fan_control_tests.rs"
  modified:
    - "src/core/events.rs (AppCommand::SetFanMode, AppEvent::FanModeChanged, AppEvent::ServiceStatusChanged)"
    - "src/core/state.rs (AppState fan_mode, pending_fan_mode, service_status, last_control_error)"
    - "src/core/worker.rs (HardwareServiceClient instantiation, periodic status check, SetFanMode handler)"
    - "src/app.rs (Slint bridge for set_fan_mode, dismiss_control_error, and property bindings)"
    - "ui/screens/cooling.slint (Interactive Auto/Max mode buttons, error notification banner, status pill)"
    - "ui/app.slint (AppWindow fan control properties, callbacks, and screen routing)"
  patterns:
    - "UI displays verified read-back state, never optimistic assumption (COOL-04)"
    - "Manual mode strictly rejected/omitted in v1 without custom curves (COOL-03)"
    - "Gated controls show explanatory notices when interface missing or service offline (COOL-05, SVC-06)"
---

# Plan 03-02: Verified Fan Control — Mode Read/Write, Read-Back Verification & Cooling Screen Summary

Verified fan mode control and interactive Cooling screen UI have been implemented and verified:
1. **D-Bus Client Layer (SVC-06):**
   - Implemented `HardwareServiceClient` in `src/service/client.rs` using `zbus::blocking::Connection`.
   - Tracks `ServiceConnectionStatus` (`Connected`, `Disconnected`, `Unavailable`).
   - Degrades gracefully without blocking the UI thread or panicking when `zenomen-hw` is offline.
2. **Core Events & Worker Integration (COOL-01, COOL-02, COOL-04):**
   - Added `AppCommand::SetFanMode` and `AppCommand::DismissControlError`.
   - Added `AppEvent::FanModeChanged`, `AppEvent::ServiceStatusChanged`, and `AppEvent::ControlErrorDismissed`.
   - Worker handles `SetFanMode` asynchronously, verifies mode from D-Bus service, and updates `AppState`.
   - Unprivileged telemetry continues to sample continuously without elevated privilege requirements (COOL-06).
3. **Cooling Screen UI & Verification Reporting (COOL-03, COOL-04, COOL-05):**
   - Updated `ui/screens/cooling.slint` with interactive buttons for `Auto` (`pwm1_enable = 2`) and `Max` (`pwm1_enable = 0`).
   - Manual mode is omitted from target options in v1.
   - Highlights confirmed active mode read back from hardware.
   - Renders a prominent `Hardware Control Verification Failed` banner when a write fails to stick or polkit is denied.
   - Gated with explanatory notices when `has-fan-control` is false or service is disconnected.
4. **Testing & Verification:**
   - Authored unit and integration tests in `tests/fan_control_tests.rs` verifying mode transitions, error persistence, error dismissal, and offline client handling.
   - All 64 tests pass cleanly across 10 test suites.
