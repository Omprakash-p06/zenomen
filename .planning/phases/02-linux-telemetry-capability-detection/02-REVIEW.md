---
phase: 02-linux-telemetry-capability-detection
reviewed: 2026-11-01T23:14:00Z
depth: standard
files_reviewed: 26
files_reviewed_list:
  - Cargo.lock
  - Cargo.toml
  - src/app.rs
  - src/core/diagnostics.rs
  - src/core/events.rs
  - src/core/mod.rs
  - src/core/state.rs
  - src/core/telemetry.rs
  - src/core/worker.rs
  - src/hardware/linux/cpu.rs
  - src/hardware/linux/gpu.rs
  - src/hardware/linux/hwmon.rs
  - src/hardware/linux/memory.rs
  - src/hardware/linux/mod.rs
  - src/hardware/linux/provider.rs
  - src/hardware/mod.rs
  - src/hardware/provider.rs
  - src/hardware/registry.rs
  - src/main.rs
  - tests/diagnostics_tests.rs
  - tests/linux_gpu_tests.rs
  - tests/linux_procfs_tests.rs
  - tests/sensor_registry_tests.rs
  - ui/app.slint
  - ui/screens/diagnostics.slint
  - ui/screens/settings.slint
findings:
  critical: 0
  warning: 3
  info: 3
  total: 6
status: resolved
---

# Phase 02: Code Review Report

**Reviewed:** 2026-11-01T23:14:00Z  
**Depth:** standard  
**Files Reviewed:** 26  
**Status:** issues_found

## Summary

Phase 2 transitions ZenOmen from a mock-driven foundation to real Linux hardware telemetry and capability detection. It introduces procfs CPU delta calculation without sleeps (`src/hardware/linux/cpu.rs`), memory calculations using `MemTotal - MemAvailable` (`src/hardware/linux/memory.rs`), dynamic DRM card enumeration without hardcoded `card0` and non-blocking NVIDIA query fallback (`src/hardware/linux/gpu.rs`), hwmon sensor scanning (`src/hardware/linux/hwmon.rs`), the unified canonical `SensorRegistry` with strict driver and label priority (`src/hardware/registry.rs`), dynamic telemetry history window resizing (`src/core/telemetry.rs`), and system diagnostics report generation and atomic JSON export (`src/core/diagnostics.rs`).

Full compilation (`cargo check`), strict linter pass (`cargo clippy --all-targets -- -D warnings`), and 52 unit/integration tests pass with 0 failures. No critical memory unsafety vulnerabilities were found. 3 warnings and 3 informational findings were identified, primarily regarding thread joining semantics on timeouts, synchronous filesystem I/O within a Slint UI callback, and redundant 1 Hz thread spawning on non-NVIDIA systems.

---

## Warnings

### WR-01: `JoinHandle::join()` after timeout in `sample_nvidia_smi` blocks the worker thread if `nvidia-smi` hangs

**File:** [`src/hardware/linux/gpu.rs:197-200`](file:///home/omprakash/Documents/zenomen/src/hardware/linux/gpu.rs#L197-L200)  
**Issue:** `sample_nvidia_smi` queries `nvidia-smi` in a spawned thread and uses `rx.recv_timeout(timeout)` with a 200ms timeout to avoid hanging the monitoring worker thread. However, immediately after matching the result, it calls:
```rust
let _ = join_handle.join();
result
```
In Rust's standard library, `JoinHandle::join()` blocks the calling thread until the child thread terminates. If the NVIDIA GPU is in D3cold power suspension, or the kernel driver stalls for several seconds during PCIe wake-up/bus recovery, `recv_timeout` returns `Err(RecvTimeoutError::Timeout)` at 200ms, but `join_handle.join()` immediately blocks the monitoring worker thread until `Command::output()` finishes. This defeats the non-blocking safeguard and violates **MON-06** ("Missing or sleeping GPUs degrade to None without blocking or hanging the worker thread").  
**Fix:** Only call `join()` if `rx.recv_timeout` succeeded (`Ok(...)`). If it timed out (`Err(RecvTimeoutError::Timeout)`), do not call `join()`; allow the handle to drop so the thread runs detached without stalling the worker:
```rust
let (result, should_join) = match rx.recv_timeout(timeout) {
    Ok(Ok(output)) if output.status.success() => {
        let stdout = String::from_utf8_lossy(&output.stdout);
        (parse_nvidia_smi_output(&stdout), true)
    }
    Ok(_) => (None, true),
    Err(_) => (None, false), // Timed out: do not block worker on join()
};

if should_join {
    let _ = join_handle.join();
}
result
```

---

### WR-02: Synchronous sysfs and file I/O inside Slint UI callback `on_export_diagnostics`

**File:** [`src/app.rs:114-142`](file:///home/omprakash/Documents/zenomen/src/app.rs#L114-L142)  
**Issue:** The Slint callback `on_export_diagnostics` is executed directly on the Slint UI thread. Inside the callback closure, `collect_diagnostics` and `export_diagnostics_to_file` perform synchronous filesystem and sysfs I/O:
- Reading `/etc/os-release`
- Reading `/proc/sys/kernel/osrelease`
- Reading `/sys/class/dmi/id/product_name` and `sys_vendor`
- Iterating `/sys/class/drm` and reading vendor files
- Formatting and writing pretty JSON to disk via `std::fs::write` and `std::fs::rename`

This conflicts with the core architectural principle in `src/app.rs`:
> "Callbacks never read a sensor, never block, and never wait for an answer (FOUND-05, threat T-01-03)."

If the target filesystem stalls (e.g. slow storage, I/O backlog, or networked home directories), the entire UI event loop hitches and drops frames.  
**Fix:** Offload the diagnostic collection and file export to a detached background thread (`std::thread::spawn`), then dispatch the resulting status string back to the UI thread via `slint::invoke_from_event_loop`:
```rust
let ui_weak = self.ui.clone();
let state = Arc::clone(&self.state);
let registry_slot = Arc::clone(&self.registry);

ui.on_export_diagnostics(move || {
    let current_state = lock_state(&state).clone();
    let reg_guard = registry_slot.lock().unwrap_or_else(|p| p.into_inner());
    let reg = reg_guard.clone();
    drop(reg_guard);
    let ui_weak = ui_weak.clone();

    std::thread::spawn(move || {
        let report = crate::core::collect_diagnostics(&current_state, reg.as_deref());
        let export_dir = directories::ProjectDirs::from("com", "zenomen", "zenomen")
            .map(|dirs| dirs.config_dir().join("diagnostics"))
            .unwrap_or_else(|| std::path::PathBuf::from("diagnostics"));

        let filename = format!("zenomen-diagnostics-{}.json", report.timestamp.replace(':', "-"));
        let export_path = export_dir.join(filename);

        let status_msg = match crate::core::export_diagnostics_to_file(&report, &export_path) {
            Ok(()) => format!("Exported to {}", export_path.display()),
            Err(err) => format!("Export failed: {err}"),
        };

        let _ = slint::invoke_from_event_loop(move || {
            if let Some(ui) = ui_weak.upgrade() {
                ui.set_export_status(status_msg.into());
            }
        });
    });
});
```

---

### WR-03: Unchecked 1 Hz thread spawning for `sample_nvidia_smi` on systems without NVIDIA hardware

**File:** [`src/hardware/linux/provider.rs:158`](file:///home/omprakash/Documents/zenomen/src/hardware/linux/provider.rs#L158) and [`src/hardware/linux/gpu.rs:170-184`](file:///home/omprakash/Documents/zenomen/src/hardware/linux/gpu.rs#L170-L184)  
**Issue:** `LinuxHardwareProvider::sample_telemetry()` calls `sample_nvidia_smi(Duration::from_millis(200))` on every 1 Hz sampling tick. On machines without an NVIDIA GPU or where `nvidia-smi` is not installed, `sample_nvidia_smi` spawns a new OS thread (`std::thread::Builder::new().name("zenomen-nv-query").spawn(...)`) that immediately fails to launch the binary (`ENOENT`). Spawning and destroying an OS thread every single second indefinitely introduces unnecessary kernel scheduling churn and memory allocation, conflicting with the lightweight efficiency requirements.  
**Fix:** In `LinuxHardwareProvider::new()`, check whether an NVIDIA GPU was detected during DRM enumeration or verify `Path::new("/usr/bin/nvidia-smi").exists()`. If false, store a boolean flag `has_nvidia` on `LinuxHardwareProvider` and skip `sample_nvidia_smi` entirely during 1 Hz sampling.

---

## Informational Findings

### IN-01: CPU `guest` and `guest_nice` jiffies double-counted in `CpuJiffies::total()`

**File:** [`src/hardware/linux/cpu.rs:65-79`](file:///home/omprakash/Documents/zenomen/src/hardware/linux/cpu.rs#L65-L79)  
**Issue:** In Linux kernel `/proc/stat`, `guest` is already accounted for within `user` time, and `guest_nice` is already accounted for within `nice` time (see `man 5 proc`). Adding `virtual_all` (`guest + guest_nice`) to `busy` counts guest execution time twice in `total()`. On typical bare-metal systems, guest time is 0 so this has no effect, but under KVM virtualization workloads it slightly inflates the `total` denominator.  
**Suggestion:** Remove `virtual_all` from `total()` calculation since `user` and `nice` are already added.

---

### IN-02: Fan tachometer discovery `resolve_fans` potential duplicate index handling across fallback devices

**File:** [`src/hardware/registry.rs:218-245`](file:///home/omprakash/Documents/zenomen/src/hardware/registry.rs#L218-L245)  
**Issue:** If no platform device (`hp` or `hp-wmi`) is found, `search_pool` defaults to all discovered hwmon devices. If two different devices both expose a `fan1_input` (for instance a chassis sensor and an auxiliary controller), both are parsed with `index: 1`.  
**Suggestion:** When searching fallback generic devices, de-duplicate tachometers by input path and/or device ID.

---

### IN-03: Hwmon device sorting uses lexicographical ordering rather than natural numeric sorting

**File:** [`src/hardware/linux/hwmon.rs:48`](file:///home/omprakash/Documents/zenomen/src/hardware/linux/hwmon.rs#L48)  
**Issue:** `devices.sort_by(|a, b| a.id.cmp(&b.id))` sorts `"hwmon10"` before `"hwmon2"`. Priority resolution correctly inspects driver names rather than hardware indices so this has no functional impact on CPU temperature priority, but natural numeric sorting (`a.id.strip_prefix("hwmon")...`) is cleaner for diagnostic listings.  
**Suggestion:** Parse the numeric suffix when sorting hwmon IDs.

---

## Verification

- `cargo check`: Pass (0 errors)
- `cargo clippy --all-targets -- -D warnings`: Pass (0 warnings)
- `cargo fmt --check`: Pass (clean)
- `cargo test --all`: 52 tests passed (0 failed, 0 ignored)
