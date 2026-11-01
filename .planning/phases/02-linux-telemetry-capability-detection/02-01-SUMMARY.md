---
phase: 02-linux-telemetry-capability-detection
plan: "01"
subsystem: hardware/linux
tags:
  - rust
  - procfs
  - sysfs
  - drm
  - nvidia-smi
  - meminfo
  - telemetry

requires:
  - 01-03
provides:
  - "Linux procfs CPU parser computing aggregate and per-core deltas without sleeps"
  - "Accurate RAM telemetry via /proc/meminfo MemAvailable math (used = total - available)"
  - "Dynamic DRM GPU sysfs enumeration without card0 hardcoding"
  - "Non-blocking NVIDIA fallback with timeout and graceful degradation"
  - "LinuxHardwareProvider implementing the HardwareProvider trait"
affects:
  - 02-02 (wires SensorRegistry for temperature priority and fan tachometers)
  - 02-03 (wires live charts, history window controls, and diagnostics export)

actuals:
  tasks: 4
  tests: 7 added (39 total passing)

tech-stack:
  added:
    - "src/hardware/linux/cpu.rs (CpuSampler, CpuJiffies delta math, cpufreq reader)"
    - "src/hardware/linux/memory.rs (MemAvailable math parser, read_memory_snapshot)"
    - "src/hardware/linux/gpu.rs (dynamic DRM card enumeration, AMD sysfs, non-blocking nvidia-smi)"
    - "src/hardware/linux/provider.rs (LinuxHardwareProvider)"
    - "tests/linux_procfs_tests.rs"
    - "tests/linux_gpu_tests.rs"
  patterns:
    - "Previous-tick delta tracking on worker thread with zero sleep calls"
    - "MemAvailable calculation prevents naive MemFree false memory alarms"
    - "DRM card path enumeration accepts card[0-9]+ and skips subconnectors"
    - "NVIDIA query runs with non-blocking channel timeout and fails safely to None"
---

# Plan 02-01: Linux Hardware Provider — procfs, hwmon, DRM & NVIDIA Summary

`LinuxHardwareProvider` has been implemented and verified with zero-warning clippy and full test coverage:
1. **CPU Parsing & Delta Tracking (MON-01, MON-02, MON-03):**
   - Implemented `CpuJiffies` and `CpuSampler` to calculate CPU utilization deltas between 1 Hz worker ticks without blocking sleeps.
   - Extracts CPU model name and frequency via cpufreq sysfs or `/proc/cpuinfo`.
2. **RAM Telemetry via MemAvailable (MON-07):**
   - Parses `/proc/meminfo` and calculates `used = MemTotal - MemAvailable`.
   - Never uses naive `MemFree`, avoiding false ~95% RAM warnings caused by Linux page cache.
3. **Dynamic DRM & NVIDIA GPU Telemetry (MON-05, MON-06, MON-08):**
   - Dynamically searches `/sys/class/drm/card*` without hardcoding `card0`.
   - Queries AMD sysfs (`gpu_busy_percent`, `mem_info_vram_*`, linked `hwmon`).
   - Queries NVIDIA via `nvidia-smi` with a 200ms non-blocking timeout, cleanly returning `None` if the driver is suspended or communication fails.
4. **Integration & Tests:**
   - Implemented `LinuxHardwareProvider` conforming to `HardwareProvider`.
   - Added unit and integration tests in `tests/linux_procfs_tests.rs` and `tests/linux_gpu_tests.rs`. All 39 tests pass cleanly.
