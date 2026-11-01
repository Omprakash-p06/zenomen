---
phase: 02-linux-telemetry-capability-detection
fixed_at: 2026-11-01T23:19:00Z
iteration: 1
scope: all
findings_in_scope: 6
fixed: 6
skipped: 0
status: all_fixed
---

# Phase 02: Code Review Fix Report

**Fixed at:** 2026-11-01T23:19:00Z  
**Scope:** all (3 Warnings, 3 Info)  
**Iteration:** 1  
**Status:** all_fixed  

## Summary

All 6 findings (3 warnings and 3 informational findings) identified in the Phase 02 code review have been resolved and verified across compiler, linter, and unit/integration test suites.

## Fixes Applied

### WR-01: `JoinHandle::join()` after timeout in `sample_nvidia_smi` blocks worker thread if `nvidia-smi` hangs
- **Files modified:** `src/hardware/linux/gpu.rs`
- **Resolution:** Modified `sample_nvidia_smi` to conditionally join `join_handle` only when `rx.recv_timeout` yields `Ok(...)`. When `recv_timeout` times out, `join()` is not called and the thread is allowed to detach, preserving non-blocking monitoring worker loop guarantees.

### WR-02: Synchronous sysfs and file I/O inside Slint UI callback `on_export_diagnostics`
- **Files modified:** `src/app.rs`
- **Resolution:** Offloaded `collect_diagnostics` and `export_diagnostics_to_file` to a dedicated background thread spawned via `std::thread::Builder::new().name("zenomen-diag-export")`. The resulting export status message is dispatched back to the UI thread non-blockingly using `slint::invoke_from_event_loop`.

### WR-03: Unchecked 1 Hz thread spawning for `sample_nvidia_smi` on systems without NVIDIA hardware
- **Files modified:** `src/hardware/linux/provider.rs`
- **Resolution:** Added `has_nvidia: bool` cached during `LinuxHardwareProvider` initialization (checking DRM card vendor for `GpuVendor::Nvidia` or presence of `/usr/bin/nvidia-smi` / `/bin/nvidia-smi`). Guarded 1 Hz `sample_nvidia_smi` behind `self.has_nvidia`, eliminating thread creation churn on non-NVIDIA systems.

### IN-01: CPU `guest` and `guest_nice` jiffies double-counted in `CpuJiffies::total()`
- **Files modified:** `src/hardware/linux/cpu.rs`
- **Resolution:** Removed `virtual_all` from `total()` calculation because Linux `/proc/stat` already accounts for guest execution within `user` and `nice` fields.

### IN-02: Fan tachometer discovery `resolve_fans` potential duplicate index handling across fallback devices
- **Files modified:** `src/hardware/registry.rs`
- **Resolution:** Added `dedup_by` filtering on `index` and `input_path` to ensure fallback fan enumeration never exposes duplicate sensors.

### IN-03: Hwmon device sorting uses lexicographical ordering rather than natural numeric sorting
- **Files modified:** `src/hardware/linux/hwmon.rs`
- **Resolution:** Updated `scan_hwmon_devices` to sort device IDs by parsing the numeric suffix of `hwmon<N>`, ensuring true numerical ordering (`hwmon0`, `hwmon1`, ..., `hwmon10`).

## Skipped Findings

None. All 6 findings were fixed.

## Verification

- `cargo check`: Clean (0 errors)
- `cargo clippy --all-targets -- -D warnings`: Clean (0 warnings)
- `cargo fmt --check`: Clean formatting
- `cargo test --all`: 52 tests passed (0 failed, 0 ignored)
