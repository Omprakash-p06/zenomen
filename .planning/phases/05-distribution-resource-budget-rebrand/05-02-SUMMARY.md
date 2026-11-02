# Plan 05-02: Resource Budget & Degradation Matrix Summary

**Status:** Completed
**Commit:** `(Pending commit)`

## Overview

Plan 05-02 implements performance instrumentation, visibility-gated sampling, empirical resource measurements, and comprehensive degradation testing across all hardware input failure scenarios (PERF-01, PERF-02, PERF-03, PERF-04, PERF-05, PERF-06).

## Delivered Artifacts

1. **`src/core/benchmark.rs`**:
   - Added `CpuSample`, `read_cpu_sample()` parsing fields 14/15 (`utime`, `stime`) from `/proc/self/stat` safely handling parentheses and spaces in process `comm`.
   - Added `cpu_percent_between()` calculating CPU percentage clamped at 0.0 with clock tick normalization.
   - Added `USER_HZ = 100`, `StageReport`, and `measure_stage()` for non-UI measurement.
2. **Visibility-Gated Sampling (`src/core/events.rs`, `src/core/worker.rs`, `src/main.rs`)**:
   - Added `AppCommand::SetUiVisible(bool)`.
   - In `worker.rs`, when `ui_visible == false`, periodic polling is paused and the worker thread blocks on `command_receiver.recv()`.
   - On transition `false -> true` upon window restoration from system tray (`DesktopAction::PresentWindow`), an immediate sample refresh is taken to avoid stale metrics, followed by resuming periodic sampling.
   - In `main.rs`, wired `SetUiVisible(false)` on `--minimized` startup and window close-to-tray.
   - Empirical outcome: **0 delta jiffies over 10 seconds (0.0000% CPU)** when minimized in tray (target: ≤ 0.02%).
3. **Stage Matrix Instrumentation (`src/main.rs`)**:
   - Emits stage benchmarks (`cold-start`, `warm-dashboard`, `tray-minimized`, `dashboard-restored`) at info level with machine-readable format `zenomen-benchmark stage=<name> rss_mb=<f64>` when `ZENOMEN_BENCHMARK=1`.
4. **Reproducible Measurement Harness (`scripts/measure-budget.sh`)**:
   - Automated sampling script measuring `/proc/<pid>/status` `VmRSS` and `/proc/<pid>/stat` CPU jiffies. Supports `--soak <seconds>` mode with 5-minute warm-up and RSS growth tracking.
5. **Session Bus Mode for Daemon Measurement (`src/bin/zenomen-hw.rs`)**:
   - Added `ZENOMEN_HW_SESSION=1` opt-in to allow non-root headless benchmarking of `zenomen-hw` on user session bus.
   - Measured idle RSS: **3.43 MB** (target: ≤ 15 MB).
6. **Degradation Test Suite (`tests/degradation_tests.rs`)**:
   - 8 integration tests covering every sensor/driver degradation path:
     - Missing fan hwmon directory.
     - Missing `pwm1_enable` sysfs node.
     - Absent CPU temp sensor.
     - Missing or malformed `nvidia-smi` output.
     - Absent RGB sysfs directory.
     - Offline hardware D-Bus service daemon.
     - Unauthorized/rejected Polkit fan write.
     - Corrupted configuration JSON.
7. **Resource Budget Documentation (`docs/RESOURCE-BUDGET.md`)**:
   - Complete empirical record covering cold start (7.15 MB), warm dashboard (16.56 MB), tray minimized (16.59 MB), hardware service (3.43 MB), dashboard idle CPU (0.0914%), tray CPU (0.0000%), degradation matrix, and explicit "NOT MEASURED" marker for the 2-hour soak.
   - Explicit separation statement between UI budget and hardware service budget.

## Verification

- `cargo fmt --all -- --check`: Clean.
- `cargo clippy --all-targets -- -D warnings`: 0 warnings.
- `cargo test --test benchmark_tests`: 4 passed.
- `cargo test --test degradation_tests`: 8 passed.
- `cargo test --all`: 72 passed, 0 failed.
