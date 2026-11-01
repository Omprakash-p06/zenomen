---
phase: "01-foundation-gui-shell-on-mock-hardware"
plan: "03"
type: implementation
prefix: "01-03"
wave: 3
depends_on:
  - "01-02"
requirements:
  - FOUND-03
  - MON-09
  - PERF-03
files_modified:
  - src/hardware/mod.rs
  - src/hardware/provider.rs
  - src/hardware/mock.rs
  - src/core/telemetry.rs
  - src/core/worker.rs
  - src/core/benchmark.rs
  - ui/components/sparkline.slint
  - ui/components/telemetry-chart.slint
  - ui/screens/overview.slint
  - tests/mock_hardware_tests.rs
  - tests/telemetry_history_tests.rs
  - tests/benchmark_tests.rs
  - src/main.rs
autonomous: true
---

# Plan 01-03: Mock Provider, Monitoring Worker, Bounded History Charts & Benchmark Instrumentation

## Objective
Deliver a runnable, end-to-end ZenOmen application driven entirely by a configurable mock hardware provider (**FOUND-03**), with a non-blocking background monitoring worker (**PERF-03**), rolling telemetry charts backed by a bounded 120-sample history ring buffer (**MON-09**), and empirical memory/frame-time benchmarking instrumentation.

## Context & Design Rules
- Follow [`01-UI-SPEC.md`](file:///home/omprakash/Documents/zenomen/.planning/phases/01-foundation-gui-shell-on-mock-hardware/01-UI-SPEC.md):
  - `Sparkline`: 28px height, 2px stroke `Path` with no axes or labels.
  - `TelemetryChart`: 160px height plot, 3 horizontal grid lines at 1px outline, gold series strokes (2px), legend row.
  - Fewer than 2 samples renders "Waiting for first reading" text instead of a flat 0 line.
  - Sensor data gaps (missing samples) render as path breaks (`M` commands) rather than connecting across gaps.
  - Absent sensors render no sparkline.
- Strictly bounded memory (**PERF-04** preparation):
  - `VecDeque<TelemetryPoint>` capped at 120 entries; `pop_front()` before `push_back()`.
  - Update Slint model properties in place rather than rebuilding arrays on every tick.
- Non-blocking execution (**PERF-03**):
  - Monitoring worker runs in a separate `std::thread`, scheduled on deadlines.
  - Computes CPU deltas without inline sleeps; Slint main thread never waits on sampling.
- Benchmark instrumentation:
  - Parses `/proc/self/smaps_rollup` (RSS, PSS, Private Dirty) to measure cold start, warm dashboard, minimized tray, and peak RSS.

## Tasks

### Task 1: Hardware Provider Trait & Mock Provider (`src/hardware/`)
1. Create `src/hardware/provider.rs` defining:
   - `HardwareCapabilities` struct.
   - `TelemetrySnapshot` struct (CPU model, utilization, temp, freq, GPU name, util, temp, RAM total/used, Fan RPMs, Fan Mode, ACPI profile).
   - `HardwareProvider` trait: `fn capabilities(&self) -> HardwareCapabilities;` and `fn sample_telemetry(&self) -> Result<TelemetrySnapshot, String>;`.
2. Create `src/hardware/mock.rs` implementing `MockHardware`:
   - Configurable scenarios: `Nominal`, `ZeroCapabilities`, `SensorDataGap` (drops a sensor for 3 ticks to verify UI trace gap handling).
   - Simulates realistic oscillating metrics: CPU 15–45%, GPU 30–70%, RAM 8192/16384 MB (using `MemAvailable` math), Fans 2200–4200 RPM.

### Task 2: Bounded Telemetry Buffer & SVG Path Generator (`src/core/telemetry.rs`)
1. Create `src/core/telemetry.rs`:
   - `TelemetryHistory`: wraps `VecDeque<TelemetryPoint>` with `MAX_SAMPLES = 120`.
   - Pure function `generate_svg_path(samples: &[Option<f32>], width: f32, height: f32) -> String`:
     - Normalizes values to plot dimensions (inverting Y for SVG coordinates).
     - Returns empty string when `< 2` valid points exist.
     - Emits `M` (move) commands across `None` values to create visible breaks in plotted traces.
2. Create `ui/components/sparkline.slint`: 28px height `Path` binding `commands` property.
3. Create `ui/components/telemetry-chart.slint`: 160px height container with gridlines, series `Path`, legend items, and "Waiting for first reading" fallback.
4. Integrate Sparkline into `Overview` metric cards and `TelemetryChart` into the overview wide chart slot.

### Task 3: Deadline-Scheduled Monitoring Worker (`src/core/worker.rs`)
1. Create `src/core/worker.rs`:
   - Spawns a background `std::thread`.
   - Executes on a 1-second deadline schedule (`Instant::now() + Duration::from_secs(1)`).
   - Samples `MockHardware`, pushes to `TelemetryHistory`, generates SVG path strings, and schedules UI updates via `slint::invoke_from_event_loop`.
   - Handles `AppCommand::RefreshTelemetry` to trigger immediate on-demand sampling.

### Task 4: Benchmark & Memory Instrumentation (`src/core/benchmark.rs`)
1. Create `src/core/benchmark.rs`:
   - Helper parsing `/proc/self/smaps_rollup` (or `/proc/self/status` if rollup unavailable) for RSS, PSS, and Private_Dirty.
   - Logs memory benchmarks distinguishing:
     - Cold start RSS (initial window map).
     - Warm dashboard RSS (after 120 samples).
   - Measures frame times to ensure UI rendering remains steady at 60 FPS while the worker pushes updates.

### Task 5: Automated Test Suite (`tests/`)
1. `tests/mock_hardware_tests.rs`: verify all mock scenarios produce valid, non-zero telemetry.
2. `tests/telemetry_history_tests.rs`: verify `TelemetryHistory` maintains exactly 120 samples on overflow and SVG path generator handles breaks properly.
3. `tests/benchmark_tests.rs`: verify memory instrumentation reads process RSS accurately and stays well within the 40 MB threshold.

## Verification
- Running `cargo run` launches ZenOmen displaying dynamic, plausible readings and live rolling charts in the dark theme.
- Triggering `ZeroCapabilities` mock mode launches cleanly and shows `UnavailablePanel` across screens without crashing (**FOUND-04**).
- Triggering `SensorDataGap` produces a visible break in the chart trace as specified in `01-UI-SPEC.md`.
- Window frame time stays stable during telemetry updates (**PERF-03**).
- Process RSS starts <30 MB and stays <38 MB with 120 samples populated.
- `cargo test --all` passes 100%.

