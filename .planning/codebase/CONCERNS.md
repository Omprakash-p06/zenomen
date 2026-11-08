---
last_mapped_commit: 4691619f4e741bd5944f2085fa455b135ff873af
last_mapped_at: 2026-11-08
---
<!-- refreshed: 2026-11-08 -->

# Codebase Concerns

**Analysis Date:** 2026-11-08
**Branch:** `fix/zenomen-reliability`
**HEAD Commit:** `4691619f4e741bd5944f2085fa455b135ff873af`

**Validation basis:** Static review of Rust sources, data files, build scripts, tests, and architecture documentation at commit `4691619f4e741bd5944f2085fa455b135ff873af`. Test coverage verified across 18 test suites (154 tests passing: 40 unit, 114 integration) including Linux sysfs/procfs providers, Windows PDH/WMI/registry providers, Windows named-pipe service RPC, and the GPUI-CE architecture scaffold.

---

## Recently Resolved Concerns

The reliability and platform hardening updates on branch `fix/zenomen-reliability` across Phase 6 and recent reliability commits (`464de29` through `4691619`) resolved critical architectural, concurrency, platform-specific stability, and telemetry honesty concerns:

| Resolved Concern | Severity / ID | Location | Resolution Summary |
|---|---|---|---|
| **Windows SCM Service Dispatcher** | Critical (`WIN-03`) | `src/service/windows/lifecycle.rs:140-310`, `src/bin/zenomen-hw.rs:27-38` | Implemented full Windows Service Control Manager (SCM) integration using `StartServiceCtrlDispatcherW`, `service_main`, `RegisterServiceCtrlHandlerExW`, and `report_service_status` (`SERVICE_START_PENDING`, `SERVICE_RUNNING`, `SERVICE_STOP_PENDING`, `SERVICE_STOPPED`). Added `--console` / `-c` CLI options for standalone execution and automatic fallback when invoked outside the service controller (`ERROR_FAILED_SERVICE_CONTROLLER_CONNECT`). |
| **Windows Named Pipe Server & Synchronous I/O** | Critical (`WIN-03`) | `src/service/windows/server.rs:115-280`, `src/service/windows/pipe.rs:45-55` | Implemented the production named-pipe server connection and request handling loop in `run_pipe_server`. Removed `FILE_FLAG_OVERLAPPED` from `CreateNamedPipeW` in `pipe.rs` to guarantee synchronous, blocking I/O compatibility. Integrated active session verification (`verify_client_session`) via `ImpersonateNamedPipeClient` and `WTSGetActiveConsoleSessionId` to reject cross-session and RDP requests before processing frames. |
| **Hardware Mutation Safety & Board Validation** | Critical (`WIN-07`) | `src/service/windows/profiles.rs:25-75`, `src/service/windows/authorization.rs:20-135` | Introduced `BoardValidation` evidence struct tracking granular write verification flags (`fan_write_verified`, `power_write_verified`, `rgb_write_verified`). Gated all hardware mutations (fan modes, power profiles, presets, RGB lighting) in `authorize_mutation` until an explicit validation record matching `board_id` and `bios_version` is provided, restricting unvalidated motherboards strictly to safe read-only monitoring. |
| **HP WMI Simulation Removal & Honest State** | Warning (`WIN-04`) | `src/service/windows/hp_wmi.rs:40-120` | Replaced simulated 2400/2500 fan RPM values and simulated sleep loops with honest status reporting and real connection verification in `hp_wmi.rs`. Fan status reports `("auto", Vec::new())` when physical tachometers are unmeasured rather than fabricating fake RPM values. |
| **Windows GPU Utilization Telemetry** | Warning (`WIN-01`) | `src/hardware/windows/gpu.rs:190-280`, `src/hardware/windows/provider.rs:140-165` | Implemented persistent `GpuPerformanceCollector` using Windows Performance Data Helper (PDH) querying `\GPU Engine(*)\Utilization Percentage` via `PdhOpenQueryW`, `PdhAddEnglishCounterW`, and `PdhGetFormattedCounterValue` with `PDH_FMT_DOUBLE`. Establishes delta baselines on initialization and handles query failures cleanly. |
| **Desktop Notification Window Anchor** | Warning (`SYS-01`) | `src/desktop/windows/notifications.rs:15-50`, `src/desktop/windows/tray.rs:60-75` | Added `show_notification_with_hwnd` binding `NOTIFYICONDATAW.hWnd` to the active tray window handle (`TrayHandle.hwnd`), resolving notification drops caused by zeroed `hWnd` when invoking `Shell_NotifyIconW(NIM_MODIFY)`. Provided convenience method `TrayHandle::show_notification`. |
| **Telemetry Capability Honesty** | Info (`MON-01`, `ARCH-03`) | `src/hardware/windows/sensors.rs:10-85` | Enforced honest sensor absence policy across Windows telemetry. Added `TemperatureSource` enum; CPU and GPU temperatures report honest `None` (categorized with `TemperatureSource::Unavailable`) because unprivileged Windows lacks thermal APIs without ring-0 drivers; unmeasured fans return an empty vector rather than dummy tachometer counts. |
| **Win32 System Tray Thread Affinity** | Critical (`CR-01`) | `src/desktop/windows/tray.rs:80-185` | In Win32, message queues are strictly thread-affine; calling `CreateWindowExW` on the main thread while running `GetMessageW` on a background pump thread caused `ERROR_INVALID_WINDOW_HANDLE` (1400) and dropped tray events. Resolved by creating the hidden message window (`ZenOmenTrayMsgWindow`) directly inside the background pump thread (`zenomen-win32-tray-pump`), synchronizing `HWND` back via an `mpsc` channel, and dispatching `PostMessageW(hwnd, WM_CLOSE, 0, 0)` during teardown. |
| **Non-Intrusive Named Pipe Probing** | Warning (`WARN-01`) | `src/service/windows/client.rs:20-45` | Replaced intrusive connection probe (`CreateFileW` followed by immediate `CloseHandle`), which consumed server connection slots and generated spurious `ERROR_BROKEN_PIPE` disconnect errors, with non-intrusive `WaitNamedPipeW` (500ms timeout / 0ms probe), checking for existence without establishing an incomplete duplex connection. |
| **Dynamic Module Leak Prevention** | Warning (`WARN-02`) | `src/hardware/windows/cpu.rs:145-175`, `src/hardware/windows/gpu.rs:65-85` | Addressed kernel handle and module reference counter growth caused by calling `LoadLibraryA` on `powrprof.dll` and `dxgi.dll` during 1 Hz telemetry sampling without freeing. Resolved with deterministic `FreeLibrary(module)` calls, preventing memory pressure during continuous background tray operation. |
| **Windows Service Name Consistency** | Warning (`WARN-03`) | `src/service/windows/lifecycle.rs:6-10`, `packaging/windows/wix/ZenOmen.wxs:28-36` | Aligned `SERVICE_NAME` in `lifecycle.rs` from `"ZenOmenHw"` to `"ZenOmenHardwareService"`, matching the WiX MSI installer service specification and eliminating SCM query mismatches during uninstallation or CLI service control. |
| **WiX Installer Desktop & ARP Icon** | Warning (`WARN-04`) | `packaging/windows/ZenOmen.wxs:18-28`, `packaging/windows/wix/ZenOmen.wxs:18-28` | Added top-level `<Icon Id="zenomen.ico" SourceFile="data/icons/zenomen.ico" />` and `<Property Id="ARPPRODUCTICON" Value="zenomen.ico" />` to WiX manifests, resolving compiler identifier errors during `wix build` and ensuring correct application icons in Windows Add/Remove Programs. |
| **Named Pipe Security & Explicit DACL** | Warning (`WARN-05`) | `src/service/windows/pipe.rs:35-55`, `src/service/windows/security.rs:65-102` | Replaced default `NULL` security attributes with an explicit SDDL Discretionary Access Control List: `D:(A;;GA;;;SY)(A;;GA;;;BA)(A;;GRGW;;;IU)` via `ConvertStringSecurityDescriptorToSecurityDescriptorW`. Enforces that only Local System, Administrators, and Interactive Users can access the privileged named pipe (`\\.\pipe\ZenOmen.Hardware.v1`). |
| **GPUI Event Channel Drain** | Warning (`WARN-06`) | `src/ui/app.rs:88-125` | Resolved event-loop error dropping in GPUI-CE event bridge. Pattern matching in `AppEvent` processing now explicitly handles both `Ok` and `Err` arms for fan mode, power profile, and RGB changes, properly setting `self.state.last_control_error` and preventing event channel starvation. |
| **Windows Telemetry Boundary Guards** | Info (`INFO-01`, `INFO-02`) | `src/hardware/windows/battery.rs:18-26`, `src/hardware/windows/device.rs:32-38, 75-81` | Guarded against false-positive charging states when the battery flag is 255 (unknown status), and implemented safe NUL-delimiter position scanning (`.iter().position(|&b| b == 0)`) for registry string reading to avoid one-byte truncation. |
| **NVIDIA CLI Overhead & Cold-Start Latency** | Linux Performance | `src/hardware/linux/gpu.rs:180-265`, `src/hardware/linux/provider.rs:27-172` | Replaced per-tick process execution with a persistent background `NvidiaSampler` thread (`zenomen-nv-watch`). Ticks query a bounded sync channel (`capacity=1`) that coalesces rapid requests. Telemetry is cached with a 3-second freshness window, returning `None` instead of stale or fabricated data if the dGPU is suspended in D3cold. |
| **Battery Capability Dynamic Scanning** | Linux Hardening | `src/hardware/registry.rs:27-77, 146-168` | Eliminated hardcoded battery capability flags. `SensorRegistry::scan_roots` performs dynamic sysfs battery presence scanning via `scan_battery_present()`, discovering `/sys/class/power_supply/BAT*` devices with readable `capacity` files. Correctly reports `battery_metrics = has_battery`. |
| **Worker Mutation Concurrency & Timeout** | Concurrency / Safety | `src/core/worker.rs:26-30, 390-465` | Implemented `InFlightGuard(Arc<AtomicBool>)` in `execute_mutating_action_with_timeout`. Only one hardware mutation is permitted in flight; concurrent requests are rejected immediately. Even if the caller times out waiting on authorization or hardware I/O, the RAII guard remains held in the background RPC worker until physical completion or unwinding. |
| **Logging & Diagnostics in Headless/GUI** | Diagnostics | `src/logging.rs:1-103`, `src/main.rs:24`, `src/bin/zenomen-hw.rs:13` | Replaced ad-hoc stdout/stderr logging with `zenomen::logging::init()`. Configures a non-blocking daily rotating file appender targeting the platform state directory (`~/.local/state/zenomen/logs` or `%LOCALAPPDATA%\zenomen\logs`). Degrades gracefully to stderr/journald if directory creation fails. |
| **Unsupported Platform Compile Failures** | Cross-Platform | `src/hardware/unavailable.rs:1-52`, `src/hardware/mod.rs:15-20` | Introduced `UnavailableHardwareProvider` for non-Linux/non-Windows fallback targets (`#[cfg(not(any(target_os = "linux", target_os = "windows")))]`). Reports honest non-implemented errors on sample attempts, enabling clean cross-compilation across platforms. |

---

## Active & Future Concerns (Prioritized)

### 1. High Priority: Hardware-in-the-Loop Physical Validation on HP OMEN Laptops (Windows 10/11)
- **Status:** Software Safety Gate Active (`BoardValidation`); Physical Hardware-in-the-Loop Validation Pending
- **Impact Area:** `src/service/windows/hp_wmi.rs`, `src/service/windows/profiles.rs`, `docs/testing/HP_BOARD_VALIDATION.md`
- **Description:** ZenOmen commands Embedded Controller (EC) fan speeds, power presets, and 4-zone lighting via ACPI WMI execution (`hpqBIntM`, GUID `{5FB7F034-2C63-45E9-BE91-3D44E2C707E4}`). The introduction of `BoardValidation` in `profiles.rs` and enforcement in `authorization.rs` ensures unverified motherboards are strictly locked to read-only monitoring, preventing dangerous blind writes. However, physical testing across representative HP OMEN and Victus chassis (15, 16, 17, and Transcend models) is required to:
  1. Verify ACPI WMI buffer byte packing (`0x20008` query / `0x20009` write) against real HP UEFI BIOS revisions.
  2. Confirm fast command response times (<500ms) without EC bus contention under heavy CPU/GPU thermal load.
  3. Validate and ratify board profiles (`BoardValidation::new_verified`) following the protocol in `docs/testing/HP_BOARD_VALIDATION.md`.
- **Mitigation / Next Step:** Execute Phase 4 of `HP_BOARD_VALIDATION.md` on physical test laptops running Windows 10 22H2 and Windows 11 23H2/24H2. `BoardValidation::new_unvalidated` baseline keeps all mutation requests safely blocked until verified.

### 2. Medium Priority: Windows Binary and MSI Code Signing (EV Cert / Azure Trusted Signing)
- **Status:** Unsigned Builds in CI
- **Impact Area:** `packaging/windows/ZenOmen.wxs`, `.github/workflows/release.yml`
- **Description:** Windows SmartScreen and Defender vigorously flag unsigned `.exe` and `.msi` installers, particularly those installing background Windows Services running as `LocalSystem`.
- **Impact:** Users downloading ZenOmen on Windows 10/11 encounter prominent SmartScreen warnings requiring manual override ("Run anyway"), reducing user trust and complicating automated deployment.
- **Mitigation / Next Step:** Procure an Authenticode Extended Validation (EV) code-signing certificate or integrate Microsoft Azure Trusted Signing into `.github/workflows/release.yml` to sign `zenomen.exe`, `zenomen-hw.exe`, and `ZenOmen.msi`.

### 3. Medium Priority: GPUI-CE UI Migration & Slint Retirement Roadmap
- **Status:** Architecture Scaffold Complete; Slint is Active v1 GUI (ADR-001)
- **Impact Area:** `src/ui/app.rs`, `src/ui/screens/`, `Cargo.toml`, `docs/decisions/ADR-001-ui-framework.md`
- **Description:** Slint 1.18.1 remains the active production GUI engine (`default = ["ui"]`). The GPUI-CE scaffold (`src/ui/`) provides a decoupled presentation architecture via unidirectional `AppCommand`/`AppEvent` messaging per ADR-001 and ADR-004. Retiring Slint requires bringing GPUI-CE to complete feature parity:
  1. Wiring complex interactive widgets: multi-node custom fan curve editor, real-time SVG telemetry canvas, and RGB zone color picker.
  2. Native system tray context menu integration on both Windows (`Shell_NotifyIconW`) and Linux (`ksni` / StatusNotifierItem).
  3. Window vibrancy/acrylic effects support on Windows 11.
  4. Memory consumption validation against the <= 40 MB RSS budget before toggling default feature flags.
- **Mitigation / Next Step:** Complete widget migration in subsequent development phases, validating memory consumption against the <= 40 MB RSS budget before toggling default feature flags.

### 4. Low Priority: D-Bus Systemd Daemon Sandboxing on Linux
- **Status:** Operational with Unit Activation (`SystemdService=zenomen-hw.service`)
- **Impact Area:** `data/dbus/com.zenomen.Hardware1.service`, `data/systemd/zenomen-hw.service`
- **Description:** Linux systemd activation is configured with `SystemdService=zenomen-hw.service`, ensuring activation requests start under systemd unit sandboxing (`ProtectSystem=strict`, `NoNewPrivileges=true`, `ProtectHome=true`).
- **Mitigation / Next Step:** Maintain and audit fine-grained `ReadWritePaths` across new kernel versions, keeping paths strictly bounded to required sysfs nodes (`pwm1_enable`, `platform_profile`, `rgb_zones`).

---

## Tech Debt

**History-window unit mismatch (UI says seconds, worker stores samples):**
- **Issue:** The settings UI labels chart history options as `30s`, `60s`, `120s`, `300s` and the callback parameter is named `seconds`, but `SetHistoryWindow` is treated as a raw sample count in `HardwareWorker`. The displayed window duration changes whenever `polling_interval_ms` changes.
- **Files:** `ui/screens/settings.slint:348`, `src/app.rs:120-124`, `src/core/worker.rs:156-172, 190-201`
- **Impact:** At the default 1s interval, "120s" holds 120 samples (~120s), but at a 2s interval it holds 240s of data. Persisted config values remain ambiguous.
- **Fix approach:** Convert seconds to samples at the command boundary using the active `sampling_interval` (`samples = seconds / interval`), or store a duration in config and recompute capacity when the polling interval changes.

**`SensorRegistry::detect_capabilities` hardcodes the RGB sysfs root on Linux:**
- **Issue:** Capability detection calls `detect_rgb_interface(Path::new("/"))` instead of deriving the root from the registry's scan roots or the provider's configured root.
- **Files:** `src/hardware/registry.rs:139`, `src/hardware/rgb.rs:33-53`
- **Impact:** Tests using synthetic fixture roots still probe host `/sys`, breaking test isolation and causing custom-root providers to report capabilities they cannot control.
- **Fix approach:** Carry `sysfs_root` (or an explicit RGB root) in `SensorRegistry`, defaulting to `/` only in production `scan()`.

**Fan-control capability divergence between registry and daemon (Linux):**
- **Issue:** `SensorRegistry::resolve_fans` accepts `pwm1_enable` on any hwmon device (including non-HP devices) and reports `has_fan_control = true`. The privileged backend only accepts `hp-wmi`/`hp` devices and returns `InterfaceMissing` otherwise.
- **Files:** `src/hardware/registry.rs:256-315`, `src/service/backend.rs:77-115`
- **Impact:** The UI can advertise fan control on unsupported machines, and subsequent mutations fail with `InterfaceMissing`.
- **Fix approach:** Align the registry's predicate with the daemon's hp/hp-wmi filter or consume the daemon's resolved capability directly.

**Autostart entry is PATH-dependent and deletion is destructive (Linux):**
- **Issue:** The generated Linux `.desktop` entry uses `Exec=zenomen --minimized` without an absolute path. Disabling autostart unconditionally deletes the file rather than setting `Hidden=true` or `X-GNOME-Autostart-enabled=false`, discarding user customizations.
- **Files:** `src/desktop/autostart.rs:17-28, 78-93`
- **Impact:** Autostart silently fails if the session's `$PATH` omits the binary directory. Disabling and re-enabling wipes custom flags.
- **Fix approach:** Write absolute binary paths resolved at runtime, update `Hidden=true` on disable, and reconcile startup state with config.

**Offline guidance hardcodes developer build paths on Linux:**
- **Issue:** When `zenomen-hw` is offline, client errors instruct users to run `sudo target/debug/zenomen-hw`.
- **Files:** `src/service/client.rs:232-248`
- **Impact:** End users on packaged installations receive guidance referencing nonexistent debug target paths.
- **Fix approach:** Emit packaged guidance (`sudo systemctl start zenomen-hw` or check service status) based on build profile.

**RGB client methods bypass the offline pre-check (Linux):**
- **Issue:** `apply_rgb_profile` and `set_rgb_enabled` call `connect()` directly rather than routing through `connect_service()`, which formats human-friendly offline messages.
- **Files:** `src/service/client.rs:301-344`
- **Impact:** RGB operations surface raw D-Bus errors (`ServiceUnknown`) instead of standard offline guidance.
- **Fix approach:** Route all client mutations through `connect_service()` and `format_service_error()`.

**`USER_HZ` hardcoded to 100 in Linux benchmark math:**
- **Issue:** `cpu_percent_between` divides jiffies by `USER_HZ = 100`, but `CLK_TCK` is kernel-dependent and not guaranteed to be 100.
- **Files:** `src/core/benchmark.rs:142-143, 232`
- **Impact:** CPU percentages reported by benchmarks can deviate on custom or non-standard kernel configurations.
- **Fix approach:** Query `sysconf(_SC_CLK_TCK)` dynamically via libc with fallback to 100.

**Diagnostics export temp-name collision and durability barrier:**
- **Issue:** Temp path uses `target.with_extension("tmp.<pid>")`, risking collision if two exports run sequentially in the same process, and lacks a `sync_all` barrier before rename.
- **Files:** `src/core/diagnostics.rs:144-160`
- **Impact:** Rapid exports can collide or leave truncated reports on crash.
- **Fix approach:** Include monotonic timestamps in temp filenames and invoke `sync_all()` prior to renaming.

---

## Known Bugs

**Shutdown hangs when session D-Bus is unavailable (Bare TTY / Headless Linux):**
- **Symptoms:** Closing the app never exits when `ensure_single_instance` returns `Disabled` (no session bus).
- **Files:** `src/main.rs:77-92, 138-168, 270-278`
- **Cause:** In the `Disabled` branch, `action_sender` remains referenced in `main`, so the receiver loop never observes disconnect. `desktop_action_thread.join()` blocks indefinitely.
- **Workaround:** Terminate process via `SIGKILL`.
- **Fix approach:** Explicitly drop `action_sender` after instance setup so the receiver disconnects when the window closes.

**`SetRgbEnabled(true)` resets active profile to default white on Linux:**
- **Symptoms:** Toggling RGB back on illuminates all zones in solid white, losing previously active color profiles.
- **Files:** `src/service/contract.rs:195-213`, `src/core/state.rs:223-241`
- **Cause:** D-Bus method has no profile state persistence; `enabled == true` unconditionally sends `RgbColor::new(255, 255, 255)`.
- **Fix approach:** Persist the last applied profile in the service daemon and restore it upon re-enabling.

**Legacy RGB parsing can panic on non-ASCII content:**
- **Symptoms:** Malformed `keyboardleds` content causes a byte slice panic at UTF-8 boundaries, aborting `zenomen-hw`.
- **Files:** `src/service/backend.rs:409-427`
- **Cause:** Validates string length as 24 bytes, then blindly slices at 6-byte offsets without checking ASCII character boundaries.
- **Fix approach:** Parse as raw ASCII hex bytes and reject invalid characters with `HardwareControlError::InvalidParameter`.

**History capacity not recomputed on interval changes:**
- **Symptoms:** Changing polling interval alters the chart's total time duration.
- **Files:** `src/core/worker.rs:156-201`
- **Fix approach:** Store duration in seconds and recompute sample buffer size dynamically on interval updates.

---

## Security Considerations

**Privileged Local Named Pipe IPC Security (Windows):**
- **Architecture:** The Windows hardware service (`zenomen-hw.exe`) runs as `LocalSystem` and exposes `\\.\pipe\ZenOmen.Hardware.v1`.
- **Mitigation Applied:**
  1. **Session Boundary Validation (`verify_client_session`):** Uses `ImpersonateNamedPipeClient`, `OpenThreadToken`, and `WTSGetActiveConsoleSessionId` to reject calls from remote desktop sessions, SSH sessions, or secondary background users.
  2. **Explicit SDDL DACL (`PIPE_SDDL`):** Configured with `D:(A;;GA;;;SY)(A;;GA;;;BA)(A;;GRGW;;;IU)` ensuring only Local System, Administrators, and the active Interactive User can connect.
  3. **Strict Payload Framing & Deserialization:** 4-byte big-endian length prefix with bounded 64 KB JSON requests/responses; unparseable packets or oversized buffers are rejected immediately without memory reallocation.
  4. **Synchronous I/O Guarantee:** `CreateNamedPipeW` omits `FILE_FLAG_OVERLAPPED` and specifies `PIPE_WAIT`, guaranteeing deterministic blocking read/write execution.

**Privileged D-Bus IPC Security & Polkit Authorization (Linux):**
- **Architecture:** The Linux daemon runs as root and exposes `com.zenomen.Hardware1` on the system bus.
- **Mitigation Applied:** Mutating operations require Polkit authorization (`org.zenomen.hardware.control`). Read-only operations (`GetCapabilities`, `GetFanStatus`) are unauthenticated by design to allow non-privileged dashboard monitoring.
- **Systemd Sandboxing:** `SystemdService=zenomen-hw.service` declared in D-Bus activation file guarantees daemon executes under unit sandboxing (`ProtectSystem=strict`, `NoNewPrivileges=true`, `ProtectHome=true`, scoped `ReadWritePaths`).

**Firmware & Embedded Controller Protection:**
- **Architecture:** Direct WMI ACPI calls (`hpqBIntM`) and sysfs writes.
- **Mitigation Applied:**
  1. **Board Profile Allowlisting (`src/service/windows/profiles.rs`):** Unknown board IDs default to `None`, restricting operations to safe read-only monitoring.
  2. **Hardware Validation Records (`BoardValidation`):** Granular write flags (`fan_write_verified`, `power_write_verified`, `rgb_write_verified`) gate mutations in `authorize_mutation`. Unvalidated motherboards reject mutations with `HardwareControlError::UnsupportedMode`.

---

## Performance Bottlenecks

**Blocking filesystem I/O inside async D-Bus handlers (Linux):**
- **Problem:** Mutating methods in `HardwareService` (`src/service/contract.rs`) are `async`, but backend functions execute synchronous `fs::write`, `fs::read_to_string`, and `thread::sleep` retry loops on the zbus executor thread.
- **Impact:** A slow sysfs write or lock stalls all D-Bus requests, including concurrent telemetry reads.
- **Improvement path:** Wrap backend sysfs calls in `tokio::task::spawn_blocking` or a dedicated worker channel.

**RGB migration runs synchronously before window creation:**
- **Problem:** `migrate_openomen_profiles` executes on the main thread prior to creating the UI window (`src/main.rs:97-112`).
- **Impact:** Large legacy profile trees can delay first window paint during cold start.
- **Improvement path:** Defer migration to a background worker after window display.

**History memory growth and per-frame chart regeneration:**
- **Problem:** Up to 300 telemetry samples are evaluated and converted into SVG paths on every frame.
- **Impact:** Path generation recalculates all points even when data points have not changed.
- **Improvement path:** Cache path geometries and invalidate only when new telemetry points arrive or dimensions change.

---

## Fragile Areas

**HP WMI ACPI Method Buffer Contracts (`hpqBIntM`):**
- **Files:** `src/service/windows/hp_wmi.rs`, `src/service/windows/protocol.rs`, `src/service/windows/profiles.rs`
- **Why fragile:** HP BIOS firmware across different OMEN generations (Intel 10th-14th Gen, AMD Zen 3-5) may interpret byte flags or return codes differently. Writing an unverified buffer could cause EC lockups requiring hard power cycling.
- **Safe modification:** Follow `docs/testing/HP_BOARD_VALIDATION.md`. Enforce read-first verification (`0x20008`) and keep unratified boards read-only via `BoardValidation::new_unvalidated`.

**Linux Sysfs Layout Assumptions for HP Hardware:**
- **Files:** `src/hardware/rgb.rs:33-53`, `src/service/backend.rs:77-115`, `src/hardware/registry.rs:256-315`
- **Why fragile:** Expects specific HP sysfs layouts (`rgb_zones/zone00`, `keyboardleds`, `leds/hp_omen::kbd_backlight/zone_colors`, `hp-wmi`). Upstream kernel revisions or non-HP test environments can alter paths.
- **Safe modification:** Always route through discovery helpers with fallback checks; test against synthetic fixture directories.

**Live-host tests in default test suite:**
- **Files:** `tests/sensor_registry_tests.rs`, `tests/linux_procfs_tests.rs`, `tests/degradation_tests.rs`
- **Why fragile:** Test cases reading host `/sys` and `/proc` directly fluctuate depending on whether the runner is AMD, Intel, a VM, or a container.
- **Safe modification:** Gate live-host tests behind `--ignored` or a `live-hardware` cargo feature flag; default CI runs to fixture roots.

---

## Scaling Limits

**Sequential Windows Named-Pipe Server:**
- **Current capacity:** `run_pipe_server` processes client requests sequentially in a single connection loop with synchronous blocking I/O.
- **Limit:** Adequate for single-user desktop operation (ZenOmen GUI client + system tray), but multiple simultaneous client requests queue.
- **Scaling path:** Transition to threaded worker pool or overlapped I/O completion ports (IOCP) if multi-client concurrent throughput is required.

**Memory Budget Verification:**
- **Current capacity:** `docs/RESOURCE-BUDGET.md` documents `Private_Dirty` of 7.15–28.50 MB (PASS vs ≤40 MB target). Measured `VmRSS` under Xvfb is 132.71–141.20 MB due to mesa/llvmpipe allocations.
- **Limit:** Software rendering in virtual display environments inflates shared memory.
- **Scaling path:** Enforce automated memory budget regression gating in CI.

---

## Dependencies at Risk

**Slint 1.18.1 UI Framework:**
- **Risk:** Pinned to `renderer-femtovg` for software/OpenGL compatibility; migration to GPUI-CE is planned per ADR-001 and ADR-004.
- **Mitigation:** Modular `HardwareWorker` architecture isolates Slint strictly to the presentation shell.

**`windows-sys` 0.59 Raw Win32 FFI:**
- **Risk:** Requires explicit unsafe pointer management and manual struct zeroing across Windows APIs.
- **Mitigation:** Wrap raw Win32 calls in dedicated safe abstractions with RAII wrappers (`FreeLibrary`, handle guards).

---

## Missing Critical Features

- **Physical Hardware Verification:** End-to-end testing and BIOS verification on physical HP OMEN laptops running Windows 10/11.
- **Windows Binary & MSI Code Signing:** Authenticode / Azure Trusted Signing pipeline in CI for SmartScreen suppression.
- **GPUI-CE Full Widget Suite & Migration:** Complete dashboard, fan curves, and tray integration replacing active Slint v1 GUI (ADR-001).
- **End-to-End Daemon Authorization Integration Tests:** Automated testing of Polkit denials on Linux and session mismatch rejection on Windows.

---

## Test Coverage Gaps

**Physical ACPI WMI Execution:**
- **What's not tested:** Live BIOS method execution via `hpqBMtdx` on physical HP hardware.
- **Priority:** High

**Polkit & Windows Session Challenge Paths:**
- **What's not tested:** Live authorization rejection when calling across unauthenticated boundaries.
- **Priority:** High

**Headless Application Lifecycle on Bare Display Servers:**
- **What's not tested:** Clean shutdown under non-standard display environments without D-Bus session bus.
- **Priority:** Medium

---

*Concerns audit: 2026-11-08 — Commit `4691619f4e741bd5944f2085fa455b135ff873af`*
