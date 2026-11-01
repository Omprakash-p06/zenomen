---
phase: 03-verified-hardware-control
reviewed: 2026-11-02T00:23:00Z
depth: standard
files_reviewed: 27
files_reviewed_list:
  - Cargo.lock
  - Cargo.toml
  - data/dbus/com.zenomen.Hardware1.conf
  - data/polkit/com.zenomen.hardware.policy
  - data/systemd/zenomen-hw.service
  - src/app.rs
  - src/bin/zenomen-hw.rs
  - src/core/diagnostics.rs
  - src/core/events.rs
  - src/core/mod.rs
  - src/core/state.rs
  - src/core/worker.rs
  - src/lib.rs
  - src/service/backend.rs
  - src/service/client.rs
  - src/service/contract.rs
  - src/service/mod.rs
  - src/service/polkit.rs
  - src/service/types.rs
  - tests/benchmark_tests.rs
  - tests/fan_control_tests.rs
  - tests/hardware_service_tests.rs
  - tests/power_preset_tests.rs
  - ui/app.slint
  - ui/screens/cooling.slint
  - ui/screens/diagnostics.slint
  - ui/screens/performance.slint
findings:
  critical: 0
  warning: 3
  info: 3
  total: 6
status: issues_found
---

# Phase 03: Code Review Report

**Reviewed:** 2026-11-02T00:23:00Z  
**Depth:** standard  
**Files Reviewed:** 27  
**Status:** issues_found

## Summary

Phase 3 introduces verified hardware control across HP OMEN/Victus systems. It establishes a two-process security architecture with privilege separation: an unprivileged Slint desktop GUI (`zenomen`) communicating over system D-Bus with a dedicated background daemon (`zenomen-hw`, `com.zenomen.Hardware1`). All mutating methods enforce mandatory Polkit authorization (`com.zenomen.hardware.control`). Hardware writes to fan mode (`pwm1_enable`) and ACPI power profiles (`platform_profile`) execute an exponential backoff write-and-verify engine that detects sticky or rejected registers and fails with `VerificationFailed` rather than reporting false success. Combined presets (Quiet, Balanced, Gaming) report granular execution results via `PresetResult`.

All 66 tests pass across 11 test suites. No memory unsafety or root privilege escape vulnerabilities were detected. 3 warnings and 3 informational findings were identified, primarily regarding synchronous D-Bus RPC execution on the Slint UI thread in `apply_to_ui`, strict clippy lint failures under `-D warnings`, and redundant D-Bus connection creation on every client call.

---

## Warnings

### WR-01: Synchronous D-Bus call to PolicyKit in Slint UI thread loop (`apply_to_ui`)

**File:** [`src/app.rs:374-386`](file:///home/omprakash/Documents/zenomen/src/app.rs#L374-L386)  
**Issue:** `apply_to_ui` runs synchronously on the Slint GUI rendering thread for every event (including 1 Hz telemetry ticks). When `state.service_status` is `Connected`, `apply_to_ui` calls:
```rust
let polkit_desc = match &state.service_status {
    crate::service::client::ServiceConnectionStatus::Connected => {
        let client = crate::service::client::HardwareServiceClient::new();
        client.check_polkit_status().unwrap_or_else(|e| format!("Check failed: {e}"))
    }
    ...
};
ui.set_polkit_status_text(polkit_desc.into());
```
`client.check_polkit_status()` performs a blocking synchronous D-Bus method call (`CheckAuthorization`) to `org.freedesktop.PolicyKit1.Authority`. If D-Bus or PolicyKit stalls or has an I/O delay, the Slint UI thread blocks and drops frames. This directly conflicts with the project's core UI thread isolation guarantee:
> *"Callbacks never read a sensor, never block, and never wait for an answer (FOUND-05, threat T-01-03)."*

**Fix:** Move the `check_polkit_status()` query to the background monitoring worker thread (e.g. alongside the 5-second service status check in `src/core/worker.rs`). Store the resulting string in `AppState::polkit_status` so `apply_to_ui` reads it with zero I/O:
```rust
ui.set_polkit_status_text(state.polkit_status.as_str().into());
```

---

### WR-02: Clippy linter failures under `-D warnings`

**Files:** [`src/core/worker.rs:145`](file:///home/omprakash/Documents/zenomen/src/core/worker.rs#L145), [`src/service/backend.rs:77, 90, 97, 141, 142`](file:///home/omprakash/Documents/zenomen/src/service/backend.rs)  
**Issue:** Running `cargo clippy --all-targets -- -D warnings` fails with 6 linter errors:
1. `src/core/worker.rs:145:28`: `status_check_tick % 5 == 0` triggers `clippy::manual-is-multiple-of`.
2. `src/service/backend.rs:77`: `if hp_wmi_hwmon.is_dir() { if let Ok(entries) = ... }` triggers `clippy::collapsible-if`.
3. `src/service/backend.rs:90`: `if class_hwmon.is_dir() { if let Ok(entries) = ... }` triggers `clippy::collapsible-if`.
4. `src/service/backend.rs:97`: `if pwm_path.exists() { if let Ok(name) = ... }` triggers `clippy::collapsible-if`.
5. `src/service/backend.rs:141`: `if fan_input.is_file() { if let Ok(content) = ... }` triggers `clippy::collapsible-if`.
6. `src/service/backend.rs:142`: `if let Ok(content) = ... { if let Ok(rpm) = ... }` triggers `clippy::collapsible-if`.

**Fix:** Apply idiomatic Rust 2024 let-chaining and `.is_multiple_of(5)`.

---

### WR-03: Redundant D-Bus system bus connection creation on every client call

**File:** [`src/service/client.rs:48-50`](file:///home/omprakash/Documents/zenomen/src/service/client.rs#L48-L50)  
**Issue:** `HardwareServiceClient::connect()` establishes a new system D-Bus connection via `Connection::system()` every time a method is called (`get_fan_status`, `set_fan_mode`, `get_power_profiles`, `set_power_profile`, `apply_preset`, `check_polkit_status`). Establishing a system D-Bus connection involves UNIX socket initialization, SASL authentication, and registration. While acceptable for occasional clicks, calling this repeatedly or during background polling creates unnecessary socket churn and allocations.  
**Fix:** Cache the active `Connection` in a thread-safe `Arc<std::sync::Mutex<Option<Connection>>>`, returning the existing connection if alive or reconnecting on error.

---

## Informational Findings

### IN-01: Blocking `thread::sleep` in async D-Bus interface `write_and_verify`

**File:** [`src/service/backend.rs:25`](file:///home/omprakash/Documents/zenomen/src/service/backend.rs#L25)  
**Issue:** `HardwareBackend::write_and_verify` uses `std::thread::sleep(delay)` during exponential backoff. When invoked from `HardwareService`'s async interface methods, this blocks the runtime executor thread handling D-Bus requests. Because `zenomen-hw` processes sequential hardware operations with small backoff delays (50ms - 200ms), this is acceptable in v1, but could be made non-blocking via `async` or offloaded to a worker pool in future phases.

---

### IN-02: Hardcoded sysfs fan input index scan limit (1..=4)

**File:** [`src/service/backend.rs:136`](file:///home/omprakash/Documents/zenomen/src/service/backend.rs#L136)  
**Issue:** `get_fan_rpms()` checks `fan{idx}_input` for `idx in 1..=4`. Laptops like OMEN have 2 fans (CPU, GPU), so 4 is sufficient for v1. Dynamic globbing or prefix checking for `fan*_input` would be slightly more flexible for multi-fan desktop platforms.

---

### IN-03: Systemd unit path assumption `/usr/local/bin/zenomen-hw`

**File:** [`data/systemd/zenomen-hw.service:10`](file:///home/omprakash/Documents/zenomen/data/systemd/zenomen-hw.service#L10)  
**Issue:** `ExecStart` points to `/usr/local/bin/zenomen-hw`. When packaged into a Debian package (`.deb`) in Phase 5, the binary is conventionally installed to `/usr/bin/zenomen-hw`. A packaging build step will need to substitute the install prefix.

---

## Verification Plan

1. Fix `WR-01`: Move polkit status check to worker thread, store in `AppState::polkit_status`, and read without I/O in `apply_to_ui`.
2. Fix `WR-02`: Resolve all 6 clippy linter warnings using `.is_multiple_of(5)` and collapsible let-chains.
3. Fix `WR-03`: Implement connection reuse in `HardwareServiceClient`.
4. Re-run `cargo clippy --all-targets -- -D warnings` and verify 0 warnings.
5. Re-run `cargo test --all` and verify all 66 tests pass cleanly.
