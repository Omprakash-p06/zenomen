---
phase: 03-verified-hardware-control
plan: "03"
subsystem: ui/performance
tags:
  - rust
  - slint
  - power-profiles
  - presets
  - diagnostics
  - polkit
  - verification

requires:
  - 03-02
provides:
  - "Interactive ACPI power profile selection with verified read-back"
  - "Combined presets (Quiet, Balanced, Gaming) with granular PresetResult banner"
  - "Hardware service connection and polkit permission diagnostics in UI and export (DIAG-04)"
affects:
  - phase-04 (Desktop integration, tray, configuration persistence, RGB)

actuals:
  tasks: 5
  tests: 5 added (66 total passing across 11 suites)

tech-stack:
  added:
    - "tests/power_preset_tests.rs"
  modified:
    - "src/service/client.rs (check_polkit_status)"
    - "src/core/diagnostics.rs (HardwareServiceDiagnostic, collect_diagnostics_from_roots)"
    - "src/core/mod.rs (re-exported HardwareServiceDiagnostic, collect_diagnostics_from_roots)"
    - "src/app.rs (set_power_profile, apply_preset callbacks and UI properties)"
    - "ui/screens/performance.slint (Interactive profile choice cards, combined presets, PresetResult banner)"
    - "ui/screens/diagnostics.slint (Hardware Control Service diagnostics section)"
    - "ui/app.slint (AppWindow performance, presets, and diagnostic properties and bindings)"
  patterns:
    - "Dynamic enumeration of ACPI profiles from platform_profile_choices (PWR-01)"
    - "Verified write backoff loop on power profiles (PWR-04)"
    - "Granular PresetResult reports partial success honestly without false atomic rollback (PWR-06)"
    - "DIAG-04 surfaces daemon connection status and polkit authorization readiness"
---

# Plan 03-03: ACPI Power Profiles, Combined Presets & Hardware Service Diagnostics Summary

Interactive ACPI power profile switching, combined presets with `PresetResult` partial-apply reporting, and hardware service diagnostics (`DIAG-04`) have been implemented and verified:
1. **Dynamic Profile Enumeration & Verified Switching (PWR-01, PWR-02, PWR-03, PWR-04):**
   - Choices are populated dynamically from `/sys/firmware/acpi/platform_profile_choices` (no hardcoded lists).
   - Interactive profile choice selector in `ui/screens/performance.slint` allows applying advertised profiles.
   - Every write is verified via read-back with exponential backoff in `zenomen-hw`; invalid profiles, traversal, or injection attempts are rejected.
2. **Combined Presets with Granular Reporting (PWR-05, PWR-06):**
   - Implemented Quiet (`quiet`/`cool` + Auto fan), Balanced (`balanced` + Auto fan), and Gaming (`performance` + Max fan).
   - Results are presented through a granular `PresetResult` banner showing `power_applied`, `fan_applied`, `partial_success`, and explanatory message.
   - Partial success (e.g., power profile succeeds but fan mode fails) is reported honestly without claiming false atomic rollback.
3. **Hardware Service Diagnostics (DIAG-04):**
   - Added `HardwareServiceDiagnostic` reporting D-Bus service status (`Connected`, `Disconnected`, `Unavailable`) and polkit authorization readiness (`Authorized`, `Authentication challenge required`, or `Authorization not granted`).
   - Displayed in `ui/screens/diagnostics.slint` and included in the exported JSON diagnostic report.
4. **Testing & Verification:**
   - Authored 5 integration tests in `tests/power_preset_tests.rs` covering dynamic profile choices, verified switching, combined presets, partial failure reporting, and diagnostics export.
   - All 66 tests pass cleanly across all 11 test suites (`cargo test --all`).
