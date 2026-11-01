---
phase: 01-foundation-gui-shell-on-mock-hardware
plan: "03"
subsystem: telemetry/core/benchmarks
tags:
  - rust
  - slint
  - telemetry-history
  - mock-hardware
  - worker-thread
  - channel-timeout
  - multi-series-chart
  - memory-benchmarking
  - headless-testing

requires:
  - "01-01: project scaffold, CI gates, fonts, theme tokens"
  - "01-02: GUI shell, sidebar navigation, screen routing, typed event bridge, unavailable panels"
provides:
  - "Configurable mock hardware provider (Nominal, ZeroCapabilities, SensorDataGap) selectable via ZENOMEN_MOCK_SCENARIO"
  - "Bounded TelemetryHistory holding at most 120 samples with pre-push pop_front() eviction"
  - "Multi-series SVG path generator supporting gaps across None sensor readings and 'Waiting for first reading' empty state"
  - "TelemetryChart with three distinct series (CPU Gold #C6A15B, GPU Secondary Text #9BA1AC, RAM Neutral Tint #5D6470)"
  - "Dedicated non-blocking monitoring worker running on a 1 Hz deadline schedule with recv_timeout for immediate on-demand refresh"
  - "Memory benchmarking instrumentation parsing /proc/self/smaps_rollup with fallback to /proc/self/status"
  - "Comprehensive headless test suite verifying hardware scenarios, ring buffer eviction, SVG generation, and RSS constraints"
affects:
  - 02 (Linux sysfs hardware providers replace MockHardware behind the HardwareProvider trait)
  - 03 (privileged writes route through D-Bus while worker sampling continues on background cadence)
  - 04 (configurable telemetry history window setting integrates with TelemetryHistory::MAX_SAMPLES)

actuals:
  tokens: 38200
  tasks: 4
  commits: 4

tech-stack:
  added: []
  patterns:
    - "Decoupled sampling worker: worker runs on a dedicated thread with recv_timeout, never calling thread::sleep"
    - "Immediate refresh responsiveness: AppCommand::RefreshTelemetry resets the deadline and triggers instantaneous sampling"
    - "Bounded VecDeque ring buffer: capacity pre-sized to 120 with explicit pop_front() on overflow guarantees flat memory usage"
    - "Resilient SVG path generation: pure functions produce M/L command strings, emitting M breaks across None sensor gaps"
    - "Memory measurement via Linux smaps_rollup: reads true process RSS, PSS, and Private_Dirty metrics from the kernel"

key-files:
  created:
    - src/hardware/mock.rs
    - src/core/telemetry.rs
    - src/core/worker.rs
    - src/core/benchmark.rs
    - src/lib.rs
    - tests/mock_hardware_tests.rs
    - tests/telemetry_history_tests.rs
    - tests/benchmark_tests.rs
  modified:
    - src/core/mod.rs
    - src/core/state.rs
    - src/hardware/mod.rs
    - src/hardware/provider.rs
    - src/app.rs
    - src/main.rs
    - ui/components/sparkline.slint
    - ui/components/telemetry-chart.slint
    - ui/screens/overview.slint

key-decisions:
  - "MockHardware is selectable via ZENOMEN_MOCK_SCENARIO env var (nominal, zero, gap) allowing all degradation states to be tested in the real binary without special compile flags."
  - "TelemetryHistory enforces a strict 120-sample bound via pop_front() before push_back(), preventing memory expansion even in long-running desktop sessions (MON-09, T-01-05)."
  - "generate_svg_path maps None to pen-up (M command on resume), visually representing sensor gaps rather than drawing lines to zero."
  - "spawn_monitoring_worker utilizes command_receiver.recv_timeout(deadline - now) instead of thread::sleep, ensuring immediate response to user refresh requests without delay (MON-10, 01-REVIEWS.md:57)."
  - "zenomen is organized as both a library (src/lib.rs) and binary (src/main.rs) so integration tests in tests/ cleanly access the public core API without display server requirements."
  - "Memory benchmarking instrumentation parses /proc/self/smaps_rollup (Rss, Pss, Private_Dirty) with fallback to /proc/self/status, enabling accurate empirical measurement of cold start (<30 MB) and warm dashboard (<38 MB) RSS."

requirements-completed:
  - FOUND-03
  - MON-01
  - MON-09
  - MON-10
  - PERF-01
  - PERF-02
  - PERF-03

verification:
  - "cargo check: zero errors"
  - "cargo fmt --all -- --check: passed"
  - "cargo clippy --all-targets --all-features -- -D warnings: passed with 0 warnings"
  - "cargo test --all: 20 tests passed across lib, main, mock_hardware_tests, telemetry_history_tests, and benchmark_tests"
  - "Memory benchmark test: process RSS successfully parsed and verified <30 MB"
---

# Plan 01-03: Telemetry Engine, Mock Scenarios, Multi-Series Charts & Benchmarking

Delivered the telemetry pipeline for Phase 1:
1. **Mock Hardware Provider**: Implemented `MockHardware` with `Nominal`, `ZeroCapabilities`, and `SensorDataGap` scenarios selectable at runtime via `ZENOMEN_MOCK_SCENARIO`.
2. **Bounded Telemetry History**: Built `TelemetryHistory` capping history at 120 samples via pre-push eviction.
3. **Multi-Series Chart & Sparklines**: Completed `TelemetryChart` and `Sparkline` Slint components bound to SVG path strings with pen-up gap handling across `None` readings.
4. **Non-Blocking Monitoring Worker**: Implemented `spawn_monitoring_worker` with channel timeout scheduling (`recv_timeout`), guaranteeing on-demand refresh responsiveness without `thread::sleep`.
5. **Memory & Performance Benchmarks**: Created `/proc/self/smaps_rollup` parser and headless test suite verifying memory footprint stays well under the 40 MB threshold.

