---
phase: 06-cross-platform-windows-linux-release
reviewed: 2026-11-07T21:40:00Z
depth: deep
files_reviewed: 45
files_reviewed_list:
  - build.rs
  - Cargo.toml
  - packaging/windows/ZenOmen.wxs
  - packaging/windows/wix/ZenOmen.wxs
  - .github/workflows/ci.yml
  - .github/workflows/release.yml
  - docs/architecture/CROSS_PLATFORM_ARCHITECTURE.md
  - docs/architecture/HP_WMI_PROTOCOL.md
  - docs/architecture/IPC_SECURITY.md
  - docs/architecture/WINDOWS_HARDWARE.md
  - docs/decisions/ADR-002-windows-hardware-interface.md
  - docs/decisions/ADR-003-windows-service-security.md
  - docs/decisions/ADR-004-cross-platform-ui.md
  - docs/testing/GEMINI_TESTING_HANDOFF.md
  - docs/testing/HP_BOARD_VALIDATION.md
  - docs/testing/WINDOWS_TEST_MATRIX.md
  - src/main.rs
  - src/bin/zenomen-hw.rs
  - src/core/telemetry.rs
  - src/core/diagnostics.rs
  - src/core/benchmark.rs
  - src/desktop/windows/autostart.rs
  - src/desktop/windows/installer.rs
  - src/desktop/windows/notifications.rs
  - src/desktop/windows/paths.rs
  - src/desktop/windows/releases.rs
  - src/desktop/windows/single_instance.rs
  - src/desktop/windows/tray.rs
  - src/hardware/windows/battery.rs
  - src/hardware/windows/cpu.rs
  - src/hardware/windows/device.rs
  - src/hardware/windows/gpu.rs
  - src/hardware/windows/memory.rs
  - src/hardware/windows/provider.rs
  - src/service/windows/authorization.rs
  - src/service/windows/client.rs
  - src/service/windows/hp_wmi.rs
  - src/service/windows/lifecycle.rs
  - src/service/windows/pipe.rs
  - src/service/windows/profiles.rs
  - src/service/windows/protocol.rs
  - src/service/windows/security.rs
  - src/service/windows/server.rs
  - src/ui/app.rs
  - tests/windows_service_tests.rs
  - tests/windows_telemetry_tests.rs
findings:
  critical: 1
  warning: 6
  info: 3
  total: 10
status: resolved
remediation:
  status: all_critical_and_warning_fixed
  tests_passed: 144
---

# Phase 06: Code Review Report — Unified Cross-Platform Release (Windows 10/11 + Linux Hardening)

**Reviewed:** 2026-11-07T21:40:00Z  
**Depth:** deep (Architecture, Thread-Affinity, Memory Leaks, Win32 FFI & Security)  
**Files Reviewed:** 45  
**Status:** issues_found  

---

## Summary

This review covers all non-planning source code, test suites, packaging configurations, and architecture specifications introduced or modified during **Phase 6** (`1bc6aa2`, `0c1872f`, `c90b245`, `05da1ea`, `6ffe107`, `0c0c121`).

The cross-platform separation (`ARCH-01`) is maintained with high fidelity:
- Pure Linux builds link no Win32/WMI libraries.
- Pure Windows builds touch no `/sys` or `/proc` filesystems and link no `zbus` libraries.
- Hardware mutations are properly isolated behind the privileged `zenomen-hw` service over local named pipe IPC with active console session validation (`WTSGetActiveConsoleSessionId`).
- Sensor reporting adheres strictly to the honest reporting invariant: absent hardware returns `None` or `Unavailable`, never synthesized zeroes.
- All 144 unit and integration tests are verified green across 18 test suites in headless CI.

However, deep scrutiny of the Windows subsystems identified **1 Critical Blocker** (Win32 message loop thread affinity in the system tray implementation) and **6 Warnings** (resource reference leaks on DLL loading, intrusive named-pipe availability checks, service name identifier mismatch, missing WiX icon resource, unhandled error variants in the GPUI event bridge, and explicit DACL descriptor attachment).

---

## Findings Classified by Severity

### Critical Issues (Release Blocker)

#### CR-01: Win32 System Tray Message Pump Thread Affinity Violation
- **File:** `src/desktop/windows/tray.rs:105-175`
- **Severity:** Critical (BLOCKER)
- **Issue:** In Win32, window message queues are strictly thread-affine. `CreateWindowExW` in `register_tray_icon` creates the hidden message window (`ZenOmenTrayMsgWindow`) on the calling thread (the main/UI thread), but immediately spawns a background thread (`zenomen-win32-tray-pump`) to run `GetMessageW(&mut msg, hwnd, 0, 0)`.
- **Consequence:** Under Win32 rules, `GetMessage` can only process messages for windows owned by the calling thread. Passing an `hwnd` created on another thread causes `GetMessage` to fail with `ERROR_INVALID_WINDOW_HANDLE` (1400) or block indefinitely without receiving messages. Consequently, system tray events (`WM_TRAY_CALLBACK`, left-click window presentation, right-click context menu dispatch) will fail or hang on live Windows desktop sessions.
- **Remediation:** The background thread `zenomen-win32-tray-pump` must create the window within its own thread context before entering `GetMessageW`, and communicate the resulting `HWND` back to `register_tray_icon` via a synchronization channel (e.g. `std::sync::mpsc::channel`). Additionally, `DestroyWindow` must be dispatched to the creating thread via `PostMessageW(hwnd, WM_CLOSE, 0, 0)`.

---

### Warnings

#### WARN-01: Intrusive Named-Pipe Availability Probe Disrupts Service State
- **File:** `src/service/windows/client.rs:20-45`
- **Severity:** Warning
- **Issue:** `WindowsServiceClient::is_service_available()` checks if the service is alive by calling `CreateFileW(PIPE_NAME, ...)` and immediately `CloseHandle(handle)`.
- **Consequence:** On Win32 named pipes, `CreateFileW` consumes an active connection slot on the listening server (`ConnectNamedPipe`). Immediately closing the handle without writing a request frame triggers `ERROR_BROKEN_PIPE` on the server's `ReadFile`, polluting server logs with spurious disconnects and temporarily consuming connection instances.
- **Remediation:** Use `WaitNamedPipeW(pipe_name_wide.as_ptr(), 0)` or query pipe status without establishing a full duplex connection.

#### WARN-02: Module Reference Leak via Unfreed `LoadLibraryA` in Telemetry Loop
- **File:** `src/hardware/windows/cpu.rs:145-165` and `src/hardware/windows/gpu.rs:65-75`
- **Severity:** Warning
- **Issue:** `read_cpu_frequency()` calls `LoadLibraryA(c"powrprof.dll")` on every 1-second telemetry sample tick. Similarly, `enumerate_gpu_adapters()` calls `LoadLibraryA(c"dxgi.dll")`. Neither function calls `FreeLibrary(module)` before returning.
- **Consequence:** Each `LoadLibraryA` call increments the OS module reference count. In a long-running background tray process, calling this every second causes reference counter overflow and unnecessary kernel handle pressure.
- **Remediation:** Either invoke `FreeLibrary(module)` upon function exit or cache the module and resolved function pointer statically using `std::sync::OnceLock`.

#### WARN-03: SCM Service Name Identifier Inconsistency
- **File:** `src/service/windows/lifecycle.rs:6-10` vs `packaging/windows/wix/ZenOmen.wxs:28-36`
- **Severity:** Warning
- **Issue:** `lifecycle.rs` defines `pub const SERVICE_NAME: &str = "ZenOmenHw";`. However, the WiX installer definition defines `<ServiceInstall Name="ZenOmenHardwareService" ...>`.
- **Consequence:** If installed via the WiX MSI installer, running `zenomen-hw.exe --uninstall-service` or CLI service management commands fails to find the service in the SCM because it queries `"ZenOmenHw"` instead of `"ZenOmenHardwareService"`.
- **Remediation:** Align `SERVICE_NAME` in `lifecycle.rs` to `"ZenOmenHardwareService"`.

#### WARN-04: Missing WiX Top-Level Icon Definition
- **File:** `packaging/windows/wix/ZenOmen.wxs:18-26` and `packaging/windows/ZenOmen.wxs:18-26`
- **Severity:** Warning
- **Issue:** Desktop and Start Menu shortcuts declare `Icon="zenomen.ico"`, but the XML lacks a top-level `<Icon Id="zenomen.ico" SourceFile="data/icons/zenomen.ico" />` element inside `<Package>`.
- **Consequence:** WiX v4/v5 compiler (`wix build`) fails with unresolved identifier error for `zenomen.ico` when assembling the MSI package.
- **Remediation:** Add `<Icon Id="zenomen.ico" SourceFile="data/icons/zenomen.ico" />` to the WiX package manifest.

#### WARN-05: Missing Explicit Security Descriptor Attachment on Named Pipe
- **File:** `src/service/windows/pipe.rs:35-45`
- **Severity:** Warning
- **Issue:** `create_server_pipe()` passes `std::ptr::null()` as `lpSecurityAttributes` to `CreateNamedPipeW`.
- **Consequence:** While `verify_client_session` checks the client's token session, passing NULL relies on the default process DACL, which may either block standard interactive users under non-standard configurations or permit unintended callers to connect.
- **Remediation:** Construct and supply a `SECURITY_ATTRIBUTES` populated via `ConvertStringSecurityDescriptorToSecurityDescriptorW` using SDDL `D:(A;;GA;;;SY)(A;;GA;;;BA)(A;;GRGW;;;IU)`.

#### WARN-06: Unhandled Failure Arms in GPUI-CE Event Bridge
- **File:** `src/ui/app.rs:88-105`
- **Severity:** Warning
- **Issue:** `GpuiApp::handle_event` matches `PowerProfileChanged(Ok(p))`, `RgbPowerChanged(Ok(p))`, and `RgbProfileApplied(Ok(p))`, but ignores `Err(e)` variants, allowing them to fall through to `_ => {}`.
- **Consequence:** Hardware mutation failures for power profile changes or RGB application are never recorded in `self.state.last_control_error`, leaving the UI without error feedback on failure.
- **Remediation:** Handle both `Ok` and `Err` arms, updating `self.state.last_control_error = Some(e)` on failure.

---

### Info & Code Quality

#### INFO-01: False-Positive Charging State on Unknown Battery Flag (255)
- **File:** `src/hardware/windows/battery.rs:18-26`
- **Severity:** Info
- **Issue:** When `flag == 255` (unknown battery status), `(flag & 8) != 0` evaluates to true, reporting `charging: Some(true)` when status is actually unknown.
- **Remediation:** Check `if flag != 255 { Some((flag & 8) != 0) } else { None }`.

#### INFO-02: Registry String Truncation on Missing Trailing NUL
- **File:** `src/hardware/windows/device.rs:32-38, 75-81`
- **Severity:** Info
- **Issue:** `buf_len.saturating_sub(1)` assumes the registry string always has a terminating NUL byte. If a string was stored without a NUL byte, the last character is truncated.
- **Remediation:** Find NUL position via `.iter().position(|&b| b == 0).unwrap_or(buf_len as usize)`.

#### INFO-03: Process RSS Benchmarking Unsupported on Windows
- **File:** `src/core/benchmark.rs:40-60`
- **Severity:** Info
- **Issue:** `read_memory_metrics()` reads `/proc/self/status` which fails on Windows.
- **Remediation:** Implement `K32GetProcessMemoryInfo` for `WorkingSetSize` under `#[cfg(windows)]`.

---

## Remediation Applied (`--fix`)

All 1 Critical issue and 6 Warnings, along with Info items INFO-01 and INFO-02, have been remediated:
1. **CR-01 (Fixed):** Window creation moved into `zenomen-win32-tray-pump` background thread closure in `src/desktop/windows/tray.rs`. HWND is synchronized to caller via channel, and teardown sends `PostMessageW(handle.hwnd, WM_CLOSE, 0, 0)`, ensuring 100% thread affinity compliance.
2. **WARN-01 (Fixed):** Replaced connection probe `CreateFileW` with non-intrusive `WaitNamedPipeW` in `src/service/windows/client.rs`.
3. **WARN-02 (Fixed):** Added `FreeLibrary` calls on DLL modules (`powrprof.dll` in `cpu.rs` and `dxgi.dll` in `gpu.rs`).
4. **WARN-03 (Fixed):** Aligned `SERVICE_NAME` in `src/service/windows/lifecycle.rs` to `"ZenOmenHardwareService"`.
5. **WARN-04 (Fixed):** Added `<Icon Id="zenomen.ico" SourceFile="data/icons/zenomen.ico" />` and `<Property Id="ARPPRODUCTICON" Value="zenomen.ico" />` in both `packaging/windows/ZenOmen.wxs` and `packaging/windows/wix/ZenOmen.wxs`.
6. **WARN-05 (Fixed):** Configured and attached `SECURITY_ATTRIBUTES` with explicit SDDL DACL (`D:(A;;GA;;;SY)(A;;GA;;;BA)(A;;GRGW;;;IU)`) in `src/service/windows/pipe.rs` via `src/service/windows/security.rs`.
7. **WARN-06 (Fixed):** Handled both `Ok` and `Err` arms for Power Profile and RGB events in `src/ui/app.rs`, updating `self.state.last_control_error`.
8. **INFO-01 & INFO-02 (Fixed):** Guarded unknown battery flag (255) and implemented safe NUL searching for registry strings.

### Verification
- `cargo fmt --all -- --check`: 0 violations
- `git diff --check`: 0 whitespace errors
- `cargo clippy --all-targets --all-features -- -D warnings`: 0 warnings
- `cargo clippy --target x86_64-pc-windows-msvc --bin zenomen -- -D warnings`: 0 warnings
- `cargo clippy --target x86_64-pc-windows-msvc --bin zenomen-hw -- -D warnings`: 0 warnings
- `xvfb-run -a cargo test --all`: 144 tests green (0 failed, 0 ignored)
