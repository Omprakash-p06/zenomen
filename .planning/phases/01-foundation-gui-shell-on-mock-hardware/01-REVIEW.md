---
phase: 01-foundation-gui-shell-on-mock-hardware
reviewed: 2026-11-01T15:35:00Z
depth: standard
files_reviewed: 49
files_reviewed_list:
  - .github/workflows/ci.yml
  - .gitignore
  - Cargo.lock
  - Cargo.toml
  - build.rs
  - src/app.rs
  - src/core/benchmark.rs
  - src/core/events.rs
  - src/core/mod.rs
  - src/core/state.rs
  - src/core/telemetry.rs
  - src/core/worker.rs
  - src/hardware/mock.rs
  - src/hardware/mod.rs
  - src/hardware/provider.rs
  - src/lib.rs
  - src/main.rs
  - tests/benchmark_tests.rs
  - tests/mock_hardware_tests.rs
  - tests/telemetry_history_tests.rs
  - ui/app.slint
  - ui/components/action-button.slint
  - ui/components/capability-notice.slint
  - ui/components/icons.slint
  - ui/components/metric-card.slint
  - ui/components/screen-header.slint
  - ui/components/section-card.slint
  - ui/components/sidebar.slint
  - ui/components/sparkline.slint
  - ui/components/status-pill.slint
  - ui/components/status-strip.slint
  - ui/components/telemetry-chart.slint
  - ui/components/unavailable-panel.slint
  - ui/components/value-readout.slint
  - ui/screens/cooling.slint
  - ui/screens/diagnostics.slint
  - ui/screens/lighting.slint
  - ui/screens/overview.slint
  - ui/screens/performance.slint
  - ui/screens/settings.slint
  - ui/theme/colors.slint
  - ui/theme/fonts/Inter-OFL.txt
  - ui/theme/fonts/Inter-Regular.ttf
  - ui/theme/fonts/Inter-SemiBold.ttf
  - ui/theme/fonts/JetBrainsMono-OFL.txt
  - ui/theme/fonts/JetBrainsMono-Regular.ttf
  - ui/theme/fonts/README.md
  - ui/theme/metrics.slint
  - ui/theme/typography.slint
findings:
  critical: 0
  warning: 3
  info: 3
  total: 6
status: issues_found
---

# Phase 01: Code Review Report

**Reviewed:** 2026-11-01T15:35:00Z
**Depth:** standard
**Files Reviewed:** 49
**Status:** issues_found

## Summary

Phase 1 establishes the Rust + Slint application foundation, event-driven architecture (`ZenOmenApp` bridge with MPSC channels), bounded telemetry history (`TelemetryHistory` ring buffer producing SVG path commands), dedicated background sampling thread (`spawn_monitoring_worker`), mock hardware scenarios (`Nominal`, `ZeroCapabilities`, `SensorDataGap`), comprehensive Slint design system tokens, 6 responsive screens, and headless CI verification.

All 49 files were reviewed at standard depth. Full compilation (`cargo check`), strict linter pass (`cargo clippy --all-targets -- -D warnings`), and 20 headless unit/integration tests pass with 0 failures. No critical security vulnerabilities or memory unsafety issues were identified. 3 warnings and 3 informational findings were surfaced regarding thread teardown semantics, provider error reporting contracts, and SVG numerical edge cases.

## Warnings

### WR-01: Background worker and event pump threads detached via drop(JoinHandle) rather than cleanly joined

**File:** `src/main.rs:56-62`
**Issue:** In `src/main.rs`, teardown after `ui.run()?` is performed via:
```rust
drop(app);
drop(worker);
drop(event_pump);
```
In Rust, dropping a `JoinHandle<()>` detaches the thread rather than terminating or joining it. Furthermore, `command_receiver` in the worker thread does not disconnect upon `drop(app)` because `ui: AppWindow` (still active on the stack) retains callback closures (`on_navigate`, `on_refresh_telemetry`) that hold clones of `commands: SyncSender<AppCommand>`. Consequently, the monitoring worker and event pump continue looping in the background until OS process termination. If state persistence or resource teardown is added in subsequent phases, detached threads risk concurrent execution during shutdown.
**Fix:** Explicitly drop `ui` before dropping channels or dispatch an explicit shutdown command/flag and join the worker thread:
```rust
drop(ui);
drop(app);
let _ = worker.join();
let _ = event_pump.join();
```

### WR-02: HardwareProvider sample_telemetry() error dropped and never surfaced to UI

**File:** `src/core/worker.rs:113-119`
**Issue:** The `HardwareProvider` trait contract (`src/hardware/provider.rs:186-189`) specifies:
> `/// Take one telemetry sample. Err carries a human-readable reason the UI can show in the status strip; it must never be a panic.`

However, in `sample_and_dispatch` (`src/core/worker.rs:113`):
```rust
match provider.sample_telemetry() {
    Ok(snapshot) => event_sender
        .send(AppEvent::TelemetryUpdated(snapshot))
        .is_ok(),
    Err(err) => {
        tracing::warn!(%err, "telemetry sampling failed");
        true
    }
}
```
`AppEvent` contains no error variant (e.g. `SamplingFailed(String)`), so when sampling returns an `Err`, no event reaches `AppState`. The UI remains on its prior reading or "Waiting for first reading" without displaying the error reason in the status strip as promised by the trait documentation.
**Fix:** Add `SamplingFailed(String)` or wrap `TelemetryUpdated(Result<TelemetrySnapshot, String>)` in `AppEvent`, update `AppState::apply` to record the error reason, and reflect it in `AppState::status_text()` and the status strip.

### WR-03: Potential NaN propagation in SVG path coordinate formatting

**File:** `src/core/telemetry.rs:214-222`
**Issue:** In `generate_svg_path_windowed`:
```rust
let x = start + step * index as f32;
let y = height * (1.0 - value.clamp(0.0, 100.0) / 100.0);
```
In Rust standard library, `f32::clamp(min, max)` returns `NaN` if `value` is `NaN`. If any sensor in future hardware backends produces `NaN`, `format!(" {x:.1} {y:.1}")` emits literal `NaN` strings into the SVG path commands (e.g., `L 100.0 NaN`). Slint's path parser will fail to parse the malformed command, causing dropped path rendering.
**Fix:** Filter non-finite numbers before computing coordinates:
```rust
let Some(value) = sample.filter(|v| v.is_finite()) else {
    pen_down = false;
    continue;
};
```

## Info

### IN-01: Asymmetric sensor modeling for CPU vs GPU utilization

**File:** `src/hardware/provider.rs:125-147`
**Issue:** `gpu_utilization`, `cpu_temperature_c`, and `gpu_temperature_c` are modeled as `Option<f32>` to support sensor gaps (`None`), whereas `cpu_utilization` is a non-optional `f32` (defaulting to `0.0`). When CPU reading cannot be sampled, it cannot express a sensor gap (`None`) without also resetting `cpu_name`.
**Fix:** Consider modeling `cpu_utilization: Option<f32>` across the telemetry pipeline for consistency with GPU metrics when implementing the Linux sysfs provider in Phase 2.

### IN-02: Obsolete dead_code allow attribute on TelemetryUpdated

**File:** `src/core/events.rs:88-89`
**Issue:** `AppEvent::TelemetryUpdated` carries:
```rust
#[allow(dead_code, reason = "sampling worker arrives in plan 01-03")]
TelemetryUpdated(TelemetrySnapshot),
```
Plan 01-03 is complete and the sampling worker actively produces this variant. The attribute and reasoning are now obsolete.
**Fix:** Remove the obsolete attribute.

### IN-03: Process-wide RSS assertion in benchmark tests

**File:** `tests/benchmark_tests.rs:25-30`
**Issue:** `memory_instrumentation_reads_valid_process_rss` asserts `metrics.rss_mb() < 30.0`. In a multi-threaded test runner or environments with heavy allocator overhead, measuring process-level RSS can introduce test flakiness if other tests run concurrently.
**Fix:** Document runner expectations (e.g. `--test-threads=1`) or calibrate headroom for multi-threaded test execution.

---

_Reviewed: 2026-11-01T15:35:00Z_  
_Reviewer: Antigravity (gsd-code-reviewer)_  
_Depth: standard_
