<!-- refreshed: 2026-11-09 -->
# Codebase Concerns

**Analysis Date:** 2026-11-09

Scope: full repository, branch `fix/zenomen-reliability`, with a dirty working tree (46 modified tracked files, 3 untracked files). Windows-specific findings were verified against source on a Windows host; the daemon compiles (`cargo check --no-default-features --bin zenomen-hw --offline` passed) and `tests/windows_service_tests.rs` passes 49/49, but **all** Windows automated tests currently execute against `FakeWmiAdapter` and do not exercise live COM, WMI BIOS methods, named pipes, SCM, or security tokens.

---

## Tech Debt

**Two divergent wire protocols for the same service surface:**
- Issue: `src/service/protocol.rs` defines the versioned, little-endian, typed contract (`RequestEnvelope`/`ServiceRequest`/`ServiceResponse`, `PROTOCOL_VERSION`) that its module doc states "the Linux daemon speaks ... the Windows service speaks the same contract". The Windows service actually uses a second, incompatible protocol in `src/service/windows/protocol.rs`: 4-byte **big-endian** length prefix, no `protocol_version`, no `request_id`, and stringly typed payloads (`SetFanMode { mode: String }`, `ApplyRgbProfile { profile_json: String }`).
- Files: `src/service/protocol.rs:1-11`, `src/service/protocol.rs:22-25`, `src/service/windows/protocol.rs:9-23`, `src/service/windows/protocol.rs:41-90`, `src/service/windows/server.rs:341`, `src/service/windows/client.rs:121-185`.
- Impact: Windows IPC has no version negotiation or correlation ID; two schemas for the same nine operations must be maintained in sync by hand. `ServiceRequest::ApplyRgbProfile(RgbProfile)` (typed struct) vs `HardwareRequest::ApplyRgbProfile { profile_json: String }` (JSON nested inside JSON string) represents an architectural shape divergence.
- Fix approach: Route the Windows pipe through `src/service/protocol.rs` (`encode_frame`/`decode_frame` + `RequestEnvelope::validate`) and delete `src/service/windows/protocol.rs`, or explicitly document the Windows protocol as intentionally separate and add versioning and `request_id` fields.

**Linux-only service modules are compiled and linked into Windows binaries:**
- Issue: `src/service/mod.rs:1-4` declares `backend`, `contract`, and `polkit` unconditionally (and re-exports them at `src/service/mod.rs:9-12`). `src/service/types.rs:147` implements `From<HardwareControlError> for zbus::fdo::Error` unconditionally. `Cargo.toml:34` declares `zbus = { version = "5", features = ["blocking"] }` under top-level `[dependencies]`, rather than under `[target.'cfg(unix)'.dependencies]`.
- Files: `Cargo.toml:34`, `src/service/mod.rs:1-4`, `src/service/mod.rs:9-12`, `src/service/types.rs:147-163`, `src/service/backend.rs` (sysfs paths), `src/service/polkit.rs:28-70` (D-Bus/PolicyKit), `src/service/contract.rs` (zbus interface).
- Impact: `zenomen.exe` and `zenomen-hw.exe` on Windows link `zbus` plus its async runtime stack (`async-executor`, `async-io`, `rustix`, `uds_windows`) for code that can never run on Windows. This directly conflicts with AGENTS rule 4 (no unnecessary async runtimes, lean dependencies) and the $\le 40$ MB RSS budget; it also pulls in two versions of `windows-sys` (`0.59` and `0.61`) via `Cargo.lock`.
- Fix approach: Add `#[cfg(not(target_os = "windows"))]` to `backend`, `contract`, `polkit`, and the `zbus::fdo::Error` conversion; move `zbus` to `[target.'cfg(unix)'.dependencies]`; gate re-exports in `src/service/mod.rs`.

**Hand-rolled COM/WMI FFI instead of `windows-sys` bindings:**
- Issue: `src/service/windows/wmi_ffi.rs` (currently untracked) re-declares `GUID`, `VARIANT`, `SAFEARRAY`, and four COM vtables (`IWbemLocatorVtbl`, `IWbemServicesVtbl`, `IWbemClassObjectVtbl`, `IEnumWbemClassObjectVtbl`) using raw `usize` placeholder slots for unused methods. No unit test compiles or exercises these vtable layouts beyond basic type checking.
- Files: `src/service/windows/wmi_ffi.rs:27-322`, `src/service/windows/hp_wmi.rs:180-556`.
- Impact: Vtable slot ordering and struct layouts are hand-maintained; an offset mistake silently invokes the wrong COM method at runtime. Some declarations use raw `usize` placeholders (`src/service/windows/wmi_ffi.rs:196-241`), so compiler signature checks cannot catch mistakes.
- Fix approach: Transition to the `windows` or `windows-sys` crate WMI/COM bindings already available; if the custom FFI layer is retained, commit `wmi_ffi.rs` and add compile-time layout assertions (`size_of::<VARIANT>() == 24`) and Windows-gated mock COM vtable tests.

**Dead or unreachable command constants and sensor stubs:**
- Issue: `commands::SYSTEM_DESIGN_DATA` (`0x28`) is defined in `src/service/windows/hp_wmi.rs` but never used (`FAN_SPEEDS` and `MAX_FAN_STATUS` are now used by the telemetry path). Sensor queries `read_fan_rpms`, `read_fan_readings`, `read_temperature_readings`, `enumerate_pnp_devices`, `read_gpu_temperature`, and `sample_gpu_utilization` have no callers or return static empty data. `has_custom_curves` is now `true` for board `8A3D` (upstream `fan_table` capability) but is still never read.
- Files: `src/service/windows/hp_wmi.rs:58-66`, `src/service/windows/hp_wmi.rs:882-887`, `src/hardware/windows/sensors.rs:43-69`, `src/hardware/windows/device.rs:171-174`, `src/hardware/windows/gpu.rs:324-333`, `src/service/windows/profiles.rs:35`.
- Impact: Readers and maintainers infer capabilities that do not exist in the compiled binary; the RGB length check provides false security.
- Fix approach: Delete unused stubs/constants or mark them `#[allow(dead_code)]` with explicit phase-tracking comments; remove constant-false condition checks.

**Board `8A3D` fan write safety and auto-restore:**
- Status: Resolved. `restore_automatic_fan_control` refuses the dangerous `0x2E {0,0}` reset on `8A3D`, instead requiring a known `ThermalProfile` before writing anything (`0x1A`), halting on any step failure. The authorization bypass for `SetFanMode { mode: "auto" }` has been closed in `src/service/windows/authorization.rs`: all fan mutations strictly require `validation.fan_write_verified == true`.
- Opcode `0x4C` investigation: Upstream Linux `HPWMI_THERMAL_PROFILE_QUERY = 0x4C` is implemented as a read-only candidate probe in `--wmi-diag` / `probe_thermal_profile_query`, but is never used to assume production capabilities or auto-unlock writes on `8A3D`. Board `8A3D` remains strictly unvalidated for physical writes (`fan_write_verified: false`).
- Files: `src/service/windows/hp_wmi.rs` (`restore_automatic_fan_control`, `probe_thermal_profile_query`), `src/service/windows/victus_s.rs`, `src/service/windows/authorization.rs`, `src/service/windows/wmi_diag.rs`.

**Documentation drift regarding commands and verification:**
- Issue: `docs/architecture/HP_WMI_PROTOCOL.md` §4 specifies fan command `0x27`, power command `0x28`, and RGB command `0x1A`. The code uses `PERFORMANCE_MODE = 0x1A`, `SYSTEM_DESIGN_DATA = 0x28` (unused), and `FOUR_ZONE_RGB = 0x0002_0009` (a `ReqType` value passed as a command ID). `docs/architecture/HP_WMI_PROTOCOL.md` §6.2 claims `authorization.rs` enforces an allowlist on raw command ID/subcommand/payload bytes over IPC; `src/service/windows/authorization.rs` only matches typed enum variants, and command bytes are assembled server-side. `docs/architecture/WINDOWS_FUNCTION_CONTRACTS.md` §2.1 lists `88F8` profiles as balanced/performance, while `src/service/windows/profiles.rs:122-126` lists quiet/balanced/performance.
- Files: `docs/architecture/HP_WMI_PROTOCOL.md` §4, §6.2; `docs/architecture/WINDOWS_FUNCTION_CONTRACTS.md` §2.1, §3.2; `src/service/windows/hp_wmi.rs:58-66`, `src/service/windows/authorization.rs:16-201`.
- Impact: Engineers implementing hardware support based on ratified protocol documentation will assemble incorrect firmware command buffers; architectural contracts cannot serve as release gates until reconciled.
- Fix approach: Reconcile command IDs against a single authoritative source, update protocol documentation, and add integration tests verifying command byte construction.

**Release builds abort on panic:**
- Issue: `panic = "abort"` is set in `[profile.release]`. Combined with the unchecked FFI and raw pointers in WMI/COM and Win32 calls, any unexpected panic inside the LocalSystem daemon terminates the process immediately: no stack unwinding, no execution of the shutdown hook that restores fan mode to Auto (`src/service/windows/server.rs:371-379`), and no panic hook output beyond the abort message. The test profile (`[profile.test]`) uses unwinding, so this behavior is never exercised in CI.
- Files: `Cargo.toml:89-96`, `src/service/windows/server.rs:371-379`.
- Impact: A malformed WMI response or unexpected null pointer can abruptly crash the hardware daemon and leave the Embedded Controller locked in its last fan mode (e.g. Max fan at 100% RPM).
- Fix approach: Use `panic = "unwind"` for service binaries with a custom top-level panic hook that dispatches the Auto fan restore command before process exit.

---

## Known Bugs

**Double free of `SAFEARRAY` (memory unsafety) in the WMI execution path:**
- Symptoms: `InBuf` is wrapped into a `VARIANT` via `ScopedVariant::from_safearray(sa)`, then the same `sa` pointer is explicitly destroyed with `SafeArrayDestroy(sa)`. When `sa_var` drops at scope exit, its `Drop` implementation calls `VariantClear(&mut self.0)`, which invokes `SafeArrayDestroy` a second time on the already-freed pointer.
- Files: `src/service/windows/hp_wmi.rs:422-447`, `src/service/windows/wmi_ffi.rs:405-415`.
- Trigger: Every `RealWmiAdapter::exec_method` call (i.e. every physical fan, power profile, or RGB lighting read/write on Windows).
- Workaround: In unit tests this is masked because tests execute against `FakeWmiAdapter`. On live Windows COM, `VariantClear` will trigger heap corruption or access violation.
- Fix: Remove the explicit `SafeArrayDestroy(sa)` call and let `ScopedVariant` own and release the `SAFEARRAY`, or call `VariantClear` explicitly and `std::mem::forget(sa_var)`.

**Unchecked `SafeArray` HRESULTs risk memory corruption on malformed buffers:**
- Symptoms: `SafeArrayAccessData(sa, &mut sa_data)` is called without checking the returned HRESULT; `copy_nonoverlapping(..., sa_data as *mut u8, 140)` immediately writes 140 bytes even if `sa_data` is null. On the output side, `SafeArrayGetUBound`, `SafeArrayAccessData`, and `std::slice::from_raw_parts(out_data as *const u8, len)` are all unchecked; `ubound` defaults to `0` on failure, constructing a 1-byte slice from an invalid or null pointer.
- Files: `src/service/windows/hp_wmi.rs:433-436`, `src/service/windows/hp_wmi.rs:543-551`.
- Impact: Memory corruption or instant daemon crash if WMI returns an unexpected buffer structure or fails data locking.
- Fix: Assert every HRESULT with `win32_check`, validate `ubound >= 0` and `len <= 128`, and return `HardwareControlError::ProtocolError` instead of blindly dereferencing raw pointers.

**Live HP WMI `hpqBMtdx` physical fan execution timing gap:**
- Symptoms: `set_fan_mode` writes command `0x27` subcommand `0x01`, sleeps for 150 ms, and reads back mode via `read_fan_mode_from_firmware` (command `0x27` subcommand `0x00`). On physical HP hardware (OMEN and Victus laptops), Embedded Controller (EC) fan state transitions are asynchronous and often take 500 ms to 2000 ms to settle or update the status byte. The read-back check `after != normalized` fails prematurely, raising `HardwareControlError::VerificationFailed` even though the EC accepted the write and is actively ramping the fans.
- Files: `src/service/windows/hp_wmi.rs:722-795`, `src/service/windows/hp_wmi.rs:920-958`.
- Trigger: Selecting "Max" or "Auto" fan mode on physical hardware with realistic EC latency.
- Workaround: None in the current codebase; `FakeWmiAdapter` responds instantaneously with mock responses in tests, masking the real-world timing gap.
- Fix: Implement an adaptive polling loop for post-write verification (e.g. check every 100 ms up to a 2000 ms timeout) before failing with `VerificationFailed`.

**`SetPowerProfile` verification fails for valid aliases sent by the UI:**
- Symptoms: `set_performance_mode` accepts and normalizes `"quiet" | "eco" | "cool"` to byte `0x02` and `"default" | "balanced"` to byte `0x00`. However, the read-back function `read_performance_mode_from_firmware` only returns canonical strings `"balanced" | "performance" | "cool"`. The post-write check `after != normalized` therefore fails for `"quiet"`, `"eco"`, and `"default"`, returning `VerificationFailed { target: "quiet", actual: "cool" }` even though the firmware write succeeded.
- Files: `src/service/windows/hp_wmi.rs:829-866`, `src/service/windows/hp_wmi.rs:980-991`, `src/ui/screens/performance.rs:18-27`.
- Trigger: Selecting the "Quiet" preset on boards `88F8` or `8A24` (both advertise `"quiet"` in `src/service/windows/profiles.rs:122-138`).
- Fix: Map aliases to their canonical read-back names before write and verify against canonical names, and add automated tests covering alias verification.

**Named pipe server blocks indefinitely while idle, preventing clean SCM stop and fan restoration:**
- Symptoms: `run_pipe_server` blocks synchronously in `ConnectNamedPipe(pipe, null)`. The `stop_signal` atomic is only checked after a client connects and disconnects. When the Windows Service Control Manager sends `SERVICE_CONTROL_STOP`, `service_ctrl_handler` sets `stop_signal = true` and reports `SERVICE_STOP_PENDING` with a 3000 ms wait hint. However, the server loop remains blocked in `ConnectNamedPipe`. SCM wait hint expires, SCM forcefully kills the process (`TerminateProcess`), and `restore_fan_mode_on_shutdown` is never executed.
- Files: `src/service/windows/server.rs:239-255`, `src/service/windows/server.rs:371-379`, `src/service/windows/lifecycle.rs:202-210`, `src/service/windows/lifecycle.rs:246-258`.
- Impact: Service shutdown appears hung in Windows Services (`services.msc`), and the hardware is left running in Max fan mode indefinitely. In `--console` mode, Ctrl+C cannot stop the server cleanly.
- Fix: Use overlapped I/O with `CreateEventW` and `WaitForMultipleObjects` on both the pipe connection and a shutdown event handle, or connect a dummy local client to unblock the synchronous loop on stop.

**Mutation timeout leaves the in-flight lock permanently held:**
- Symptoms: `execute_mutating_action_with_timeout` moves `InFlightGuard` into an inner RPC thread. If the RPC call times out on the supervisor thread (30 s timeout), the guard remains alive inside the inner thread. If the inner thread is blocked on a synchronous named pipe read or hung WMI call, `in_flight` remains `true` forever, causing every subsequent mutation to be rejected with "Another hardware change is still in progress".
- Files: `src/core/worker.rs:401-463`, `src/core/worker.rs:542-583`, `src/main.rs:350-356`.
- Trigger: Windows service hangs or client connection stalls while executing a mutation.
- Impact: Complete lockout of all hardware controls until the user restarts `zenomen.exe`.
- Fix: Maintain guard ownership on the supervisor thread, release or invalidate the guard upon timeout, and impose strict bounded I/O timeouts on the client named pipe calls.

**`SetRgbEnabled` is a no-op that reports success without firmware interaction:**
- Symptoms: `handle_request` in `src/service/windows/server.rs:153-160` immediately returns `HardwareResponse::RgbEnabledSet(enabled)` without calling any firmware adapter method. The client treats this as confirmed state, violating the project rule against fabricating hardware state.
- Files: `src/service/windows/server.rs:153-160`, `src/service/windows/client.rs:405-419`.
- Impact: Toggling lighting in the UI displays success even if the physical keyboard backlight state remains unchanged.
- Fix: Implement the underlying WMI lighting toggle method or return `HardwareControlError::UnsupportedMode`.

**Unprivileged tachometer and thermal telemetry limitation on Windows:**
- Symptoms: `read_fan_rpms()` and `read_fan_readings()` in `src/hardware/windows/sensors.rs` return empty vectors (`Vec::new()`) unconditionally. `read_temperature_sensors()` returns `(None, None)` and `read_temperature_readings()` returns `Unavailable`. Even in the elevated service, `query_fan_status` in `src/service/windows/hp_wmi.rs:708-719` returns `Ok((mode, Vec::new()))` because command `FAN_SPEEDS` (`0x11`) is never called.
- Files: `src/hardware/windows/sensors.rs:34-70`, `src/service/windows/hp_wmi.rs:708-719`.
- Impact: Fan tachometer widgets and thermal graphs on Windows are permanently empty/unavailable, contrary to contracts in `docs/architecture/WINDOWS_FUNCTION_CONTRACTS.md` §5.1.
- Fix: Implement command `0x11` parsing in `RealWmiAdapter` for supported boards, and document thermal limitations honestly where kernel ring-0 drivers are unavailable.

**Registry helpers substitute invented values for read failures:**
- Symptoms: In `src/hardware/windows/device.rs:48-60`, BIOS version falls back to `"F.00"`, manufacturer to `"HP"`, and model to `"OMEN Laptop"` when registry reads fail or contain non-UTF-8 data. While `"F.00"` fails safe against board validation guards, substituting manufactured values violates AGENTS rule 9.
- Files: `src/hardware/windows/device.rs:48-60`, `src/hardware/windows/device.rs:72-116`.
- Impact: UI and diagnostics display fabricated values instead of indicating read failure.
- Fix: Return `Option<String>` for BIOS version and system identity, rendering "Unavailable" in the UI when reads fail.

---

## Security Considerations

**Board validation matrix locks out target machine while granting unbacked writes to others:**
- Risk: The board validation policy in `src/service/windows/profiles.rs:181-259` hardcodes write permissions for `88F8` (OMEN 15/16 AMD) on BIOS `F.20/F.21` and `8A24` (OMEN 16/17 Intel) on BIOS `F.10`, despite the absence of any committed diagnostic log evidence or SSDT/DSDT dumps proving physical validation. Conversely, board `8A3D` (HP Victus 15-fb0xxx, the primary hardware available to the maintainer) is hardcoded as unvalidated for writes (`fan_write_verified: false`, `power_write_verified: false`), permanently blocking fan and power mutations on the primary development machine.
- Files: `src/service/windows/profiles.rs:115-259`, `tests/windows_service_tests.rs:193-222`, `docs/testing/HP_BOARD_VALIDATION.md`.
- Impact: On `8A3D`, all mutation requests fail with `HardwareControlError::UnsupportedMode`. On `88F8` and `8A24`, writes are permitted on claims that lack recorded validation evidence. Furthermore, normal BIOS updates (e.g. `88F8` updating to `F.22`) silently revoke write permissions due to strict string matching.
- Recommendations: Complete the physical validation runbook on `8A3D` and commit evidence artifacts; replace hardcoded code arms with an evidence-backed validation record loader; provide clear user diagnostic messages when a BIOS update changes validation status.

**Named pipe security hardening and impersonation fail-open discrepancy:**
- Risk: In `src/service/windows/security.rs:48-104`, `verify_client_session` attempts `ImpersonateNamedPipeClient` to perform deep token inspection. If impersonation succeeds, it checks `TokenSessionId` and ensures `RevertToSelf` fails closed (`std::process::exit(1)`). However, if `ImpersonateNamedPipeClient` fails (e.g. running in `--console` dev mode without `SeImpersonatePrivilege`), the implementation logs a debug message and returns `Ok(())` (`security.rs:98-103`). Debug note `.planning/debug/windows-security-and-runtime-validation.md:59` explicitly claimed the server "rejects the connection immediately" on impersonation failure. The actual code falls back to `GetNamedPipeClientSessionId` only.
- Files: `src/service/windows/security.rs:48-106`, `.planning/debug/windows-security-and-runtime-validation.md:54-61`.
- Impact: If the service runs under a degraded account or if impersonation fails under SCM, the service silently drops token inspection and relies solely on the pipe session ID.
- Recommendations: Distinguish explicit dev/console mode from SCM production mode; fail closed (`AccessDenied`) in SCM mode if `ImpersonateNamedPipeClient` fails; reconcile debug documentation with code reality.

**Unprivileged interactive user can execute firmware mutations without admin elevation:**
- Risk: The named pipe DACL grants `GRGW` to `IU` (Interactive Users, `src/service/windows/security.rs:114-118`). Any process running in the active console session—including unprivileged malware, background scripts, or low-integrity applications—can connect to `\\.\pipe\zenomen-hw` and trigger fan, power, or RGB firmware mutations. On Linux, mutations are gated by PolicyKit (`src/service/polkit.rs:28-71`), which enforces administrative credentials. On Windows, no `TokenElevation` or `BUILTIN\Administrators` check is performed.
- Files: `src/service/windows/security.rs:114-118`, `src/service/windows/server.rs:22-31`, `src/service/windows/authorization.rs:16-201`.
- Impact: Weak privilege boundary compared to Linux: an unprivileged desktop process can drive hardware mutations through a SYSTEM service without UAC elevation.
- Recommendations: For mutating operations (`SetFanMode`, `SetPowerProfile`, `ApplyRgbProfile`), check client token elevation (`TokenElevation`) or require membership in the local Administrators group during impersonation; allow read-only queries for standard interactive users.

**Single-threaded synchronous blocking read enables trivial denial of service:**
- Risk: The named pipe server loop handles one connection at a time using synchronous `ReadFile` without a read timeout (`src/service/windows/server.rs:264-334`). Any local process in the interactive session can connect to the pipe, send 1 byte (or declare a 4-byte length and send nothing further), and hold the connection open. The server thread blocks permanently in `ReadFile`, blocking all other clients from connecting and preventing SCM service shutdown.
- Files: `src/service/windows/server.rs:237-334`, `src/service/windows/pipe.rs:7-13`.
- Impact: A hung or malicious client can wedge the entire hardware service indefinitely.
- Recommendations: Use overlapped I/O with a strict 5-second per-message read timeout (`WaitForSingleObject`), disconnecting and resetting the pipe if the client stalls.

**Unquoted service binary path during SCM registration (CWE-428):**
- Risk: `register_service` passes `std::env::current_exe()` directly to `CreateServiceW` as `lpBinaryPathName` without enclosing quotation marks. If the executable is installed in a path containing spaces (e.g. `C:\Program Files\ZenOmen\zenomen-hw.exe`), Windows service startup will attempt to execute `C:\Program.exe` if present.
- Files: `src/service/windows/lifecycle.rs:18-24`, `src/service/windows/lifecycle.rs:42-56`.
- Impact: Local privilege escalation to LocalSystem if an attacker has write access to the volume root.
- Recommendations: Enclose the binary path in escaped quotes (`"\"{path}\""`), and add a unit test verifying path formatting.

---

## Performance Bottlenecks

**Full WMI COM connection and query recreation on every firmware call:**
- Problem: `RealWmiAdapter::exec_method_windows` creates a new `WbemLocator`, connects to `ROOT\WMI`, opens `hpqBIntM`, calls `GetMethod`, spawns an in-parameters instance, executes `SELECT * FROM hpqBIntM`, fetches `__PATH`, and calls `ExecMethod` for every single read or write operation. The `IWbemServices` pointer obtained during `connect()` is immediately dropped.
- Files: `src/service/windows/hp_wmi.rs:161-277`, `src/service/windows/hp_wmi.rs:304-556`.
- Impact: Substantial latency penalty (50–150 ms) and heap churn on every hardware poll or write; creates dozens of short-lived COM proxies.
- Improvement path: Cache the `IWbemServices` pointer, the class object, and the instance `__PATH` BSTR in `RealWmiAdapter`, reusing them across calls with a reconnect-on-failure fallback.

**GPU utilization query uses wildcard counter with non-array PDH API:**
- Problem: `GpuPerformanceCollector` queries counter `\GPU Engine(*)\Utilization Percentage` (which matches multiple GPU engines) using `PdhGetFormattedCounterValue`. This API is designed for single-instance counters. When used with wildcards, it returns only the first engine instance or fails with `PDH_CSTATUS_INVALID_DATA`, resulting in under-reported or missing aggregate GPU utilization.
- Files: `src/hardware/windows/gpu.rs:221-229`, `src/hardware/windows/gpu.rs:272-296`.
- Impact: GPU utilization on Windows reports inaccurate or zero percentage under multi-engine workloads.
- Improvement path: Use `PdhGetFormattedCounterArray` to enumerate and aggregate utilization across all engine instances, or use DXGI/D3DKMT query interfaces.

**Synchronous atomic file persistence on the telemetry worker thread:**
- Problem: Config mutation actions (`SetPollingInterval`, `SetAutostart`, `SetStartMinimized`, etc.) invoke `save_atomic`, which writes to a temporary file, calls `file.sync_all()` (`FlushFileBuffers`), renames the file, and calls `sync_all()` on the parent directory. This runs synchronously on the worker thread, blocking the 1 Hz telemetry sampling loop.
- Files: `src/core/worker.rs:156-226`, `src/config/mod.rs:85-116`.
- Impact: UI stutter and delayed telemetry frames during settings modifications; directory sync is a no-op or error on Windows NTFS.
- Improvement path: Offload config persistence to a dedicated background task or debounced writer, and omit parent-directory sync on Windows targets.

**Synchronous sleeps during firmware writes hold the single-threaded server:**
- Problem: `set_fan_mode` sleeps 150 ms before verification and 200 ms on busy retries; `set_performance_mode` sleeps 150 ms. During these sleeps, the single-threaded server loop is completely blocked from processing any other IPC request.
- Files: `src/service/windows/hp_wmi.rs:773-792`, `src/service/windows/hp_wmi.rs:858`.
- Impact: Concurrent client calls block or timeout during hardware mode transitions.
- Improvement path: Acceptable for rare writes, but should be handled asynchronously or across a multi-threaded request pool.

---

## Fragile Areas

**`VARIANT` struct layout assumes 64-bit architecture:**
- Files: `src/service/windows/wmi_ffi.rs:68-77`.
- Why fragile: The struct is hand-coded with an explicit 24-byte layout matching 64-bit Windows. No `#[cfg(target_pointer_width = "64")]` guard or compile-time assertion exists. On 32-bit Windows targets, `VARIANT` is 16 bytes, causing instant ABI mismatch and stack corruption.
- Safe modification: Add `static_assertions::assert_eq_size!(VARIANT, [u8; 24])` and gate compilation with `#[cfg(target_arch = "x86_64")]`.
- Test coverage: Zero unit tests compile or assert struct sizes.

**Hardcoded COM vtable indices in DXGI adapter enumeration:**
- Files: `src/hardware/windows/gpu.rs:105-140` (`EnumAdapters1` at slot 12, `GetDesc1` at slot 10, `Release` at slot 2), `src/hardware/windows/gpu.rs:45-57`.
- Why fragile: Vtable offsets are hardcoded magic numbers transmuted to function pointers. Any change in COM interface definition or SDK header ordering causes calls to jump to arbitrary memory locations.
- Safe modification: Use the official `windows` crate `IDXGIFactory1` and `IDXGIAdapter1` types.
- Test coverage: Unit tests only exercise fallback logic on non-Windows targets.

**Tray icon lifetime, window proc raw pointer dereference, and leaked sender:**
- Files: `src/desktop/windows/tray.rs:64-65`, `src/desktop/windows/tray.rs:151-155`, `src/desktop/windows/tray.rs:238`.
- Why fragile: `TrayHandle` contains `unsafe impl Send` and `unsafe impl Sync` despite encapsulating a Win32 `HWND`. A raw `Box::into_raw(Box::new(event_sender))` is stored into `GWLP_USERDATA` and dereferenced in the window procedure without thread-affinity checks. On clean shutdown, the boxed sender is never freed, leaking memory and preventing the channel from closing cleanly.
- Safe modification: Wrap the tray window state in a dedicated RAII manager that reclaims the pointer on `WM_NCDESTROY` and joins the message loop thread on drop.

**Inconsistent WMI availability reporting between adapter and server:**
- Files: `src/service/windows/hp_wmi.rs:260-274`, `src/service/windows/server.rs:207-235`.
- Why fragile: When the WMI class `hpqBIntM` is missing, `RealWmiAdapter::connect` returns `Ok(Self { available: false })`. The server logs "HP WMI connection established successfully" despite the interface being unavailable, and then on query failures retries `connect_hp_wmi()` before falling back to `FakeWmiAdapter::new(false)`.
- Safe modification: Define an explicit `WmiConnectionStatus::Unavailable` return, eliminate duplicate connection attempts, and log accurate status.

---

## Scaling Limits

**Single-client serialization with short connection timeout:**
- Current capacity: The server creates a single named pipe instance (`create_server_pipe(true)` at `src/service/windows/server.rs:237`). The `max_instances` parameter of 4 in `src/service/windows/pipe.rs:54` is ineffective because only one instance handle is created.
- Limit: A second concurrent client immediately receives `ERROR_PIPE_BUSY`. `WindowsServiceClient` retries 5 times with `WaitNamedPipeW(500)` (~2.5 s total) before failing. A single blocked client blocks all communication.
- Scaling path: Maintain a pool of pipe instances or use non-blocking overlapped I/O with thread-pool completion ports.

**Unbounded event channel between worker and UI loop:**
- Current capacity: `src/main.rs:325` uses `std::sync::mpsc::channel()` (unbounded). The command channel is correctly bounded at 8 (`src/app.rs:30,50`), and tray channel at 16 (`src/main.rs:108`).
- Limit: If the UI event loop stalls, telemetry frames accumulate indefinitely, leading to memory growth.
- Scaling path: Use `sync_channel` with a fixed bound (e.g. 16) and drop stale telemetry frames in favor of fresh updates.

---

## Dependencies at Risk

**Unconditional `zbus` dependency on Windows targets:**
- Risk: `Cargo.toml:34` compiles `zbus` and its async runtime on Windows targets where it is completely unused.
- Impact: Inflates executable binary size, increases compile times, introduces duplicate transitive dependencies (`windows-sys 0.59` and `0.61`), and conflicts with project memory targets.
- Migration plan: Move `zbus` to `[target.'cfg(unix)'.dependencies]` and gate `src/service/mod.rs` modules.

**Untracked and hand-rolled WMI COM FFI:**
- Risk: `src/service/windows/wmi_ffi.rs` is hand-written, untested in CI, and currently untracked in git.
- Impact: High risk of accidental omission during packaging or build breakage across different Windows SDK versions.
- Migration plan: Commit `src/service/windows/wmi_ffi.rs` immediately; migrate to standard `windows` crate bindings in Phase 7.

**Unpinned MSRV with Edition 2024:**
- Risk: `Cargo.toml:4` specifies `edition = "2024"` without defining a `rust-version`.
- Impact: CI or contributor builds on toolchains prior to 1.85 will fail with confusing compiler diagnostics.
- Migration plan: Add `rust-version = "1.85"` to `Cargo.toml`.

---

## Missing Critical Features

**Physical fan RPM tachometer telemetry on Windows:**
- Problem: Both the unprivileged provider (`src/hardware/windows/sensors.rs:59-64`) and the elevated service (`src/service/windows/hp_wmi.rs:708-719`) return empty vectors for fan RPMs. Command `FAN_SPEEDS` (`0x11`) is unimplemented.
- Architectural reality: On HP Victus laptops (`8A3D`, `8A3E`), BIOS WMI does not expose fan tachometer readings via standard ACPI methods (returning `UnknownCommand`). Reading Victus fan speeds requires direct Embedded Controller port I/O, which is prohibited without ring-0 kernel drivers (ADR-002). Victus platforms will permanently lack tachometer telemetry on Windows. On HP OMEN laptops (`88F8`, `8A24`), command `0x11` could theoretically be implemented in the elevated service.
- Blocks: Live fan tachometer graphs and RPM displays in the Cooling screen on Windows.

**CPU and GPU temperature telemetry on Windows:**
- Problem: `read_temperature_sensors` returns `(None, None)` and `read_temperature_readings` returns `Unavailable` (`src/hardware/windows/sensors.rs:34-57`).
- Blocks: Thermal monitoring, overheating warnings, and temperature charts in the UI.

**Physical hardware validation for target machine (`8A3D`):**
- Problem: Board `8A3D` (HP Victus 15-fb0xxx, the primary hardware available) is blocked from all writes (`src/service/windows/profiles.rs:232-247`). Phase 7 validation has not been performed on live hardware.
- Blocks: Physical fan mode and power profile control on the maintainer's machine.

---

## Uncommitted Work & Release Readiness

**Working tree contains uncommitted dependencies and extensive modifications:**
- Evidence: `src/service/windows/wmi_ffi.rs` is an **untracked file**, yet `src/service/windows/mod.rs:12` imports it. Checking out `HEAD` fails Windows compilation. The working tree has 46 modified tracked files across 3,000+ lines of changes.
- Impact: Git history does not represent a compilable Windows state; work is vulnerable to data loss.
- Recommendation: Commit `wmi_ffi.rs` immediately in a dedicated commit, followed by logical commits for security hardening, profiles, and documentation.

---

## Test Coverage Gaps

**Real WMI COM FFI and `hpqBMtdx` invocation:**
- Untested: All 36 Windows service integration tests in `tests/windows_service_tests.rs` use `FakeWmiAdapter`. Zero automated tests execute `RealWmiAdapter`, COM initialization, SAFEARRAY memory management, or method execution against live WMI infrastructure.
- Priority: Critical.

**Named pipe security and impersonation edge cases:**
- Untested: Session verification across multiple interactive sessions, simulated `ImpersonateNamedPipeClient` failures, `RevertToSelf` handling, and client disconnection during partial frame transmission.
- Priority: High.

**SCM service lifecycle and stop-pending transitions:**
- Untested: Service registration quoting, service start/stop transitions under SCM dispatcher, and shutdown hook fan mode restoration.
- Priority: High.

**Firmware command payload assembly and verification:**
- Untested: Command byte construction for `0x1A`, `0x27`, and `0x20009` against real firmware responses; alias resolution for `"quiet"` and `"cool"` power profiles.
- Priority: High.

**Windows desktop integration:**
- Untested: Real notification balloon dispatch via `Shell_NotifyIconW`, tray icon message loop cleanup, and autostart registry creation.
- Priority: Medium.

---

*Concerns audit refreshed: 2026-11-09*
