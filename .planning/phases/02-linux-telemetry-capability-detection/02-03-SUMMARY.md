---
phase: 02-linux-telemetry-capability-detection
plan: "03"
subsystem: core/diagnostics
tags:
  - rust
  - slint
  - telemetry-history
  - dynamic-resizing
  - diagnostics-report
  - file-export
  - linux-provider

requires:
  - 02-02
provides:
  - "Dynamic telemetry history window configuration (30s, 60s, 120s, 300s) without restarting"
  - "DiagnosticsReport gathering OS, kernel, DMI, capabilities, sensor paths, and latest readings"
  - "Atomic diagnostics JSON export to local disk (~/.config/zenomen/diagnostics/)"
  - "Default runtime initialization of LinuxHardwareProvider on Linux host"
affects:
  - Phase 3 (Fan & Thermal Control)
  - Phase 4 (Configuration & Profiles persistence)

actuals:
  tasks: 3
  tests: 4 added (52 total passing)

tech-stack:
  added:
    - "src/core/diagnostics.rs (DiagnosticsReport, collect_diagnostics, export_diagnostics_to_file)"
    - "tests/diagnostics_tests.rs (Report generation & atomic JSON serialization integration tests)"
  modified:
    - "src/core/telemetry.rs (Dynamic set_capacity on ring buffer and windowed SVG path generation)"
    - "src/core/events.rs & src/core/state.rs (HistoryWindowChanged event and state handling)"
    - "src/app.rs (Wiring history window setting & diagnostics export callbacks)"
    - "src/main.rs (Default LinuxHardwareProvider initialization with mock fallback)"
    - "ui/screens/settings.slint (History window duration selector buttons)"
    - "ui/screens/diagnostics.slint (System detection readout and Export Diagnostics button)"
  patterns:
    - "Dynamic ring buffer resizing evicting oldest samples on downsize without loss of recent data"
    - "Atomic export file writing via temporary file replacement"
    - "Unified sensor inspection between runtime sampling and diagnostics report"
---

# Plan 02-03: Live Charts, Refresh & History Controls, Diagnostics Report & Export Summary

Plan 02-03 has completed all remaining deliverables for Phase 2:
1. **Dynamic Telemetry History Window Resizing (MON-11):**
   - Implemented `TelemetryHistory::set_capacity(&mut self, new_capacity: usize)` in `src/core/telemetry.rs`.
   - Downsizing evicts oldest samples using `pop_front()`, keeping buffer size bounded.
   - SVG path rendering scales cleanly across the dynamically configured capacity.
   - Wired `AppCommand::SetHistoryWindow(usize)` and `AppEvent::HistoryWindowChanged(usize)` across core worker, state, and UI.
   - Added interactive duration selector options (30s, 60s, 120s, 300s) to `ui/screens/settings.slint`.
2. **Diagnostics Detection Report & JSON File Export (DIAG-01, DIAG-02):**
   - Implemented `src/core/diagnostics.rs` defining `DiagnosticsReport` containing:
     - `app_version`, `os_name` (from `/etc/os-release`), `kernel_version` (from `/proc/sys/kernel/osrelease`).
     - DMI `product_name` and `sys_vendor` (from `/sys/class/dmi/id`).
     - Subsystem `capabilities` (`HardwareCapabilities`).
     - Canonical `cpu_sensor`, `fan_sensors`, and `gpu_sensors` from shared `SensorRegistry`.
     - `latest_telemetry` snapshot and `recent_error` message.
     - ISO 8601 UTC `timestamp`.
   - Implemented atomic file exporter `export_diagnostics_to_file` to save formatted JSON reports.
   - Wired "Export Diagnostics" button on `ui/screens/diagnostics.slint` exporting to `~/.config/zenomen/diagnostics/`.
3. **Default Linux Hardware Provider (MON-10):**
   - Updated `src/main.rs` to default to `LinuxHardwareProvider::new()` on Linux.
   - Preserves `ZENOMEN_MOCK_SCENARIO` environment variable override for automated testing.
   - Shares `SensorRegistry` with `ZenOmenApp` ensuring identical sensor identity across telemetry and diagnostics.
4. **Verification:**
   - 52 tests passing across 8 test suites. Zero clippy warnings with `-D warnings`.
