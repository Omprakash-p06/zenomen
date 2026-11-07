# Plan 06-01: Linux Concerns & Security Remediation Summary

**Status:** Completed
**Phase:** 06-cross-platform-windows-linux-release
**Wave:** 1

## Overview
Plan 06-01 remediated all unresolved security vulnerabilities, technical debt, and fragile areas in the Linux codebase as documented in `.planning/codebase/CONCERNS.md` and Section 6 of the cross-platform specification. This establishes a rock-solid, audited Linux baseline before introducing Windows native code.

## Delivered Artifacts & Fixes

1. **D-Bus Service Activation Binding (`data/dbus/com.zenomen.Hardware1.service`)**:
   - Added `SystemdService=zenomen-hw.service` to delegate D-Bus activation directly to systemd instead of launching the binary directly outside the sandboxed cgroup.
2. **Systemd Service Sandboxing (`data/systemd/zenomen-hw.service`)**:
   - Restricted write access with `ReadWritePaths=-/sys/devices/platform/hp-wmi -/sys/firmware/acpi -/sys/class/hwmon -/sys/class/leds`.
   - Hardened system security with `ProtectProc=invisible`, `ProcSubset=pid`, `PrivateDevices=true`, `RestrictSUIDSGID=true`, `RemoveIPC=true`, `UMask=0077`, and `LogsDirectory=zenomen`.
3. **Non-Destructive RGB Enable/Disable State Preservation (`src/service/contract.rs` & `src/service/client.rs`)**:
   - Added `last_active_rgb` profile zone caching in `HardwareService`.
   - `apply_rgb_profile_internal` caches the active 4-zone profile colors.
   - `set_rgb_enabled_internal(false)` powers off RGB to `#000000`.
   - `set_rgb_enabled_internal(true)` restores the previously active profile rather than destructively overwriting with `#FFFFFF` white.
   - Routed client RGB operations through `connect_service()` and `format_service_error()` to ensure offline daemon errors emit actionable guidance.
4. **Session D-Bus Absence Shutdown Safety (`src/main.rs`)**:
   - Explicitly dropped the main thread's `action_sender` clone immediately after single instance setup.
   - When running without session D-Bus, closing the window now cleanly drops the remaining sender, unblocking `action_receiver.recv()` and preventing `desktop_action_thread.join()` from deadlocking.
5. **Non-Destructive Autostart Lifecycle (`src/desktop/autostart.rs`)**:
   - Implemented `update_desktop_entry()` to update `Hidden=true`/`false` and `X-GNOME-Autostart-enabled=false`/`true` in place.
   - Disabling autostart updates flags without deleting the file, preserving user comments, custom keys, and environment variables.
6. **Dynamic System Clock Tick Math (`src/core/benchmark.rs` & `Cargo.toml`)**:
   - Added `libc = "0.2"` under `[target.'cfg(unix)'.dependencies]` (resolving to existing locked version 0.2.191).
   - Added `get_clock_ticks_per_second()` dynamically querying `sysconf(_SC_CLK_TCK)` on Linux instead of hardcoding `USER_HZ = 100`.
7. **Durable Atomic Diagnostics Export (`src/core/diagnostics.rs`)**:
   - Added monotonic sequence counter `static EXPORT_SEQ: AtomicU64` to eliminate temporary file collisions.
   - Added `File::sync_all()` on the temporary file before rename, and directory `sync_all()` on the parent directory to guarantee on-disk durability.

## Verification & Test Results

1. **New Tests Added**:
   - `tests/rgb_tests.rs`: `test_rgb_enable_restores_previous_profile_instead_of_hardcoded_white` verifying profile application, disable, and non-destructive color restoration.
   - `tests/desktop_integration_tests.rs`: `test_autostart_toggle_preserves_custom_file_lines` verifying that custom comments and keys survive disable/enable cycles.
2. **Offline Daemon Error Guidance**:
   - `tests/fan_control_tests.rs`: Verified offline error contains `"sudo systemctl start zenomen-hw"`.
3. **Full Suite**:
   - `cargo fmt --all -- --check`: Passed with 0 discrepancies.
   - `git diff --check`: Passed with 0 whitespace errors.
   - `cargo clippy --all-targets --all-features -- -D warnings`: Passed with 0 warnings.
   - `xvfb-run -a cargo test --all`: All 125 automated tests passed across all 16 integration suites.
