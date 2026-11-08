<!-- refreshed: 2026-11-08 -->
# Codebase Concerns

**Analysis Date:** 2026-11-08

Scope: whole repository, verified against source on 2026-11-08. Severity reflects user-visible risk, not effort.

## Priority Summary

| # | Concern | Severity | Primary file |
|---|---------|----------|--------------|
| 1 | Windows hardware control is simulated, not implemented, and reports success | Critical | `src/service/windows/hp_wmi.rs` |
| 2 | Windows GUI hangs on exit when `close_to_tray = false` (leaked channel sender) | High | `src/desktop/windows/tray.rs`, `src/main.rs` |
| 3 | Hardware mutations can leak a thread + pipe handle forever after timeout | High | `src/core/worker.rs`, `src/service/windows/client.rs` |
| 4 | Autostart toggle on Windows writes an XDG file and never touches HKCU Run | High | `src/core/worker.rs` |
| 5 | Root install copies `zenomen-hw` from a user-writable location without verification | High | `src/desktop/system_integration.rs` |
| 6 | Named-pipe DACL admits every interactive local user; session ID is the only gate | High | `src/service/windows/security.rs` |
| 7 | Blocking `ConnectNamedPipe` + 30 s timeout makes service shutdown hang-prone | Medium | `src/service/windows/server.rs`, `lifecycle.rs` |
| 8 | Two incompatible wire protocols; the versioned one is unused | Medium | `src/service/protocol.rs`, `src/service/windows/protocol.rs` |
| 9 | Diagnostics fabricate OS, authorization, and BIOS values | Medium | `src/core/diagnostics.rs`, `src/hardware/windows/device.rs` |
| 10 | Windows tests assert the simulated success path as correct behavior | Medium | `tests/windows_service_tests.rs` |

---

## Tech Debt

**Windows hardware control is a simulation layer:**
- Issue: Every Windows hardware operation is faked. `connect_hp_wmi()` returns `Ok(HpWmiConnection { is_available: true })` without opening `ROOT\WMI` or checking `hpqBIntM` (`src/service/windows/hp_wmi.rs:33-45`). Capabilities are unconditionally reported as fully present (`src/service/windows/hp_wmi.rs:48-68`). Fan mode and power profile are stored in process-local statics (`src/service/windows/hp_wmi.rs:70-71`) and `query_fan_status` never reads firmware (`src/service/windows/hp_wmi.rs:78`). The RGB writer builds a 24-byte payload and discards it (`let _ = payload;` at `src/service/windows/hp_wmi.rs:195`) before returning `Ok(profile.name.clone())`.
- Files: `src/service/windows/hp_wmi.rs`, `src/service/windows/profiles.rs`, `src/service/windows/server.rs`, `src/hardware/windows/provider.rs`
- Impact: The Windows build claims fan control, power profiles, RGB, battery, and full telemetry that it cannot deliver. `HardwareResponse::RgbEnabledSet(enabled)` is returned directly from the request with no hardware call (`src/service/windows/server.rs:102`). `resolve_board_validation` marks boards `88F8`, `8A24`, `88D0`, `8A3D`, `8A3E` as write-verified purely from the board ID (`src/service/windows/profiles.rs:138-164`), so the "verified" gate passes while writes do nothing. This contradicts `src/hardware/unavailable.rs:1-11` ("it must not lie") and AGENTS.md rules 5 and 9.
- Fix approach: Make `HpWmiConnection` hold a real WMI session (or a declared "not implemented" state) and return `HardwareControlError::InterfaceMissing` when the class cannot be opened. Delete the process-local statics; read state back from firmware. Propagate write failures instead of returning the requested value. Until then, route Windows through `UnavailableHardwareProvider` so the UI renders `Unavailable`.

**Two wire protocols, neither authoritative:**
- Issue: `src/service/protocol.rs` defines a little-endian, versioned, request-id envelope (`PROTOCOL_VERSION = 1`, `MAX_FRAME_BYTES = 64 * 1024` at lines 25 and 209-232) with a typestate `HardwareService` contract; `src/service/windows/protocol.rs` defines a separate big-endian, unversioned 4-byte length prefix (`MAX_PAYLOAD_SIZE = 65_536`, lines 40-90) with a different request/response enum. `src/service/contract.rs` uses neither: it exposes string-based D-Bus methods and JSON for capabilities.
- Files: `src/service/protocol.rs`, `src/service/windows/protocol.rs`, `src/service/contract.rs`, `src/service/backend.rs`
- Impact: The versioned protocol cannot detect a peer speaking the Windows framing, and request IDs (`RequestEnvelope.id`) are never correlated with responses on Windows. The typed `HardwareService` abstraction is dead code relative to the shipped transport, so protocol changes must be made three times.
- Fix approach: Pick one framing (the versioned one), delete the duplicate enum, and make `src/service/contract.rs` and the Windows client encode/decode through it. Add a version-mismatch test.

**Windows hardening logic duplicated and asymmetric:**
- Issue: Fan-control detection in the GUI is `dev.name == "hp" || dev.name == "hp-wmi" || dev.name.contains("platform")` (`src/hardware/registry.rs:281`), while the daemon resolves a stricter `hp`/`hp-wmi` match. Temperature input selection is lexicographic over `temp1_input`, `temp10_input`, `temp2_input` (`src/hardware/registry.rs:260`), so `temp10_input` wins over `temp2_input`.
- Files: `src/hardware/registry.rs`, `src/service/backend.rs`
- Impact: GUI and daemon can disagree about which sysfs node is writable; a machine exposing both `platform` and `hp-wmi` can show fan control in the UI and have the daemon reject the write. Lexicographic sensor selection can bind the wrong temperature channel on kernels with >9 inputs.
- Fix approach: Move path discovery into one shared function used by both binaries (or make the daemon authoritative and have the GUI consume daemon capabilities only). Sort by parsed numeric index, not string.

**Silent fallbacks hide failures from the user:**
- Issue: `load_or_default` returns defaults when the config file exists but is unreadable, without surfacing a notice (`src/config/mod.rs`); `show_notification` logs a warning but returns `Ok(())` on `Shell_NotifyIconW` failure (`src/desktop/windows/notifications.rs:40-50`); diagnostics export spawn errors are discarded (`let _ =` in `src/app.rs`); service start/stop errors are discarded in the worker (`src/core/worker.rs:181-182,204`).
- Files: `src/config/mod.rs`, `src/desktop/windows/notifications.rs`, `src/app.rs`, `src/core/worker.rs`
- Impact: Failures present as "the setting did nothing" with no UI message. The autostart case is worse: the Windows toggle silently writes a Linux `.desktop` file and reports success (see Known Bugs).
- Fix approach: Extend the existing `corrupt_notice` mechanism to unreadable-file errors; make `show_notification` return `Err`; surface `AppEvent::ConfigSaveFailed`-style events for autostart/service operations.

**`unsafe impl Send/Sync` on raw Win32 handles:**
- Issue: `unsafe impl Send/Sync for TrayHandle` (`src/desktop/windows/tray.rs:64-65`, carrying a raw `HWND` and `NOTIFYICONDATAW`) and `unsafe impl Send/Sync for MutexHandle` (`src/desktop/windows/single_instance.rs:30-31`). `TrayHandle::drop` calls into Win32 from the main thread while the tray pump thread is still alive.
- Files: `src/desktop/windows/tray.rs`, `src/desktop/windows/single_instance.rs`
- Impact: Correctness depends on `Shell_NotifyIconW` and `PostMessageW` being safe cross-thread (true) and on the message pump exiting before the process (not guaranteed). No compile-time protection against future misuse.
- Fix approach: Keep the raw handles inside a single owning thread and expose only a command channel; or document the invariants with a `SAFETY:` comment per block and add a teardown handshake (join the pump thread).

**Blocking API used inside D-Bus method handlers:**
- Issue: `StatusNotifierItem`/`TrayDbusMenu`/`ApplicationService` call `SyncSender::send` (blocking, capacity 16 from `src/main.rs:108`) from zbus handler threads (`src/desktop/tray.rs:87,93,99,168,171`, `src/desktop/single_instance.rs:39,46,53`).
- Files: `src/desktop/tray.rs`, `src/desktop/single_instance.rs`
- Impact: If the desktop-action thread is stalled (e.g., inside `slint::invoke_from_event_loop` during teardown), zbus dispatch blocks for the full channel capacity. Teardown at `src/main.rs:353-364` relies on `drop(_session_conn)` to release those sender clones implicitly.
- Fix approach: Use `try_send` and drop/log on full, as the UI path already does. Explicitly remove the D-Bus objects before joining the action thread.

**Log sink selection by executable name:**
- Issue: `src/logging.rs` decides "am I the service" by checking whether the executable name contains `zenomen-hw`.
- Files: `src/logging.rs`
- Impact: A renamed or symlinked binary writes logs to the wrong sink; tests that rename the binary can silently change behavior.
- Fix approach: Pass an explicit `LogRole` from each `main`.

---

## Known Bugs

**Windows GUI never exits when `close_to_tray = false`:**
- Symptoms: Closing the window destroys the UI, but the process stays alive with no visible window until killed.
- Files: `src/desktop/windows/tray.rs:150-155,238-242,297-308`, `src/main.rs:186-197,361-364`
- Trigger: Set "close to tray" off, then close the window. `on_close_requested` calls `slint::quit_event_loop()` (`src/main.rs:194`) without sending `DesktopAction::Quit`.
- Mechanism: The tray thread stores the `SyncSender<DesktopAction>` with `Box::into_raw` into `GWLP_USERDATA` (`src/desktop/windows/tray.rs:152`) and never frees it on `WM_DESTROY` (`:303-308`). The leaked sender keeps the channel open after the pump exits, so `for action in action_receiver` (`src/main.rs:205`) never ends and `desktop_action_thread.join()` (`src/main.rs:364`) blocks forever. The `Quit` path happens to work because it `break`s the loop (`src/main.rs:232`).
- Fix approach: Free the boxed sender in `WM_DESTROY` (`Box::from_raw` after clearing `GWLP_USERDATA`), and assert on teardown that the tray thread has exited. Add a regression test with a fake pump.

**Windows autostart toggle is a no-op:**
- Symptoms: Toggling "start on login" persists `autostart = true` in `config.toml` and creates `%APPDATA%\autostart\zenomen.desktop`, but the app does not start at login.
- Files: `src/core/worker.rs:181-182,204`, `src/desktop/autostart.rs`, `src/desktop/mod.rs:11,28`, `src/desktop/windows/autostart.rs`
- Trigger: Any Windows autostart toggle from Settings.
- Mechanism: The worker always calls `crate::desktop::autostart::set_autostart_enabled` (the XDG implementation) and discards the result. The HKCU implementation is only re-exported as `set_windows_autostart_enabled` (`src/desktop/mod.rs:28`) and has no caller. `AppState` then reflects the requested config value, so the UI shows "on".
- Fix approach: Route through a `#[cfg(windows)]` dispatcher, return the error to the UI, and re-read the effective state (`is_windows_autostart_enabled`) instead of echoing config.

**Temperature history "300 s" option exceeds documented retention:**
- Symptoms: Selecting the 300 s history window works, but the `TelemetryHistory` docs claim retention "never more than `MAX_SAMPLES`" (120).
- Files: `src/core/telemetry.rs:27,172-180,195-203`, `src/config/schema.rs:98`
- Trigger: Choose 300 s in Settings.
- Mechanism: `set_capacity(300)` correctly sets `capacity = 300`, so the doc comment is wrong, not the behavior. `set_capacity` also never shrinks the `VecDeque` allocation, so switching 300 → 30 keeps the old buffer.
- Fix approach: Fix the doc comment; call `shrink_to_fit()` after bulk eviction if the allocation matters.

**Windows diagnostics are hardcoded:**
- Symptoms: The diagnostics report always says `Microsoft Windows 10/11` / `NT 10.0` and `Session Authenticated (Local Named Pipe)` regardless of the actual OS build or whether the service is reachable.
- Files: `src/core/diagnostics.rs:86-87,124`
- Trigger: Export diagnostics on any Windows machine with the service stopped.
- Fix approach: Query `RtlGetVersion`/`GetVersionEx` and the pipe/service state; emit `Unavailable` when unknown.

**Fabricated identity fallbacks:**
- Symptoms: A machine whose BIOS version cannot be read reports `F.00`; a machine whose SMBIOS strings are missing reports `HP` / `OMEN Laptop`.
- Files: `src/hardware/windows/device.rs:58,119,163,167`
- Trigger: WMI/registry read failure (including non-HP hardware).
- Fix approach: Return `Option<String>` and let `UnavailableHardwareProvider`-style panels render the gap.

**Uninstall leaves the daemon behind:**
- Symptoms: `uninstall_system_integration` removes the unit and polkit file but not `/usr/bin/zenomen-hw`.
- Files: `src/desktop/system_integration.rs` (`DAEMON_BIN_TARGET` at line 18, install at `:78-118`, uninstall later in file)
- Trigger: Uninstall, then reinstall an older package; the stale root-owned daemon binary remains executable.
- Fix approach: Remove `DAEMON_BIN_TARGET` during uninstall (best-effort, ignore `NotFound`).

---

## Security Considerations

**Named-pipe DACL trusts the whole interactive session:**
- Risk: `PIPE_SDDL = "D:(A;;GA;;;SY)(A;;GA;;;BA)(A;;GRGW;;;IU)"` (`src/service/windows/security.rs:114-118`) grants read/write to `NT AUTHORITY\Interactive Users`, i.e. every locally logged-on user. Authorization then relies on comparing the client's session ID and optional impersonation (`src/service/windows/security.rs`).
- Files: `src/service/windows/security.rs`, `src/service/windows/server.rs`, `data/polkit/com.zenomen.hardware.policy`
- Current mitigation: Session-ID comparison and impersonation attempts; the polkit policy applies only to the D-Bus daemon (`allow_active=auth_admin_keep`, `allow_any=auth_admin`).
- Recommendations: Add an explicit ACL for the installing user SID (as endorsed by the Windows service model) instead of `IU`; fail closed when `create_pipe_security_descriptor` fails instead of passing `sa_ptr = NULL` (`src/service/windows/pipe.rs:33-58`), which silently inherits the default DACL; log authorization decisions (currently only authorization results are logged, not denials with SIDs).

**Root install copies a binary from a user-writable directory:**
- Risk: `install_daemon_binary` resolves `current_exe().with_file_name("zenomen-hw")` and `fs::copy`s it to `/usr/bin` as root (`src/desktop/system_integration.rs:78-118`). If an administrator runs the install from a user-writable location (AppImage extraction dir, Downloads, a user-owned checkout), a malicious `zenomen-hw` beside the GUI becomes a root-owned setuid-adjacent binary started by systemd.
- Files: `src/desktop/system_integration.rs`
- Current mitigation: Root check (`is_root`), atomic rename via a temp file, mode `0755`.
- Recommendations: Verify the candidate against a trusted source (package path, signature, or a hash shipped with the GUI) and refuse to copy from paths writable by non-root (`stat` mode/ownership); prefer shipping the daemon in the package and never copying at runtime.

**`systemctl` PATH fallback:**
- Risk: `run_systemctl` uses `/usr/bin/systemctl` when present, otherwise falls back to the bare name `"systemctl"`, resolved through `PATH` (`src/desktop/system_integration.rs:194-203`). Called as root during install/uninstall, a poisoned `PATH` executes attacker code.
- Files: `src/desktop/system_integration.rs`
- Recommendations: Use an absolute path only; if missing, return an error naming the expected path.

**Verification gate is metadata-only:**
- Risk: `authorize_mutation` trusts `BoardValidation` flags, which are granted from a board-ID table (`src/service/windows/profiles.rs:138-164`); `bios_version` is recorded but never compared despite the docstring at `:27`.
- Files: `src/service/windows/profiles.rs`, `src/service/windows/server.rs`
- Current mitigation: Unknown boards are read-only; a mismatch between `validation.board_id` and the detected board is rejected (covered by `tests/windows_service_tests.rs:211-216`).
- Recommendations: Compare `bios_version` against the ratified record (or a supported range), and record who/what ratified it. Do not grant RGB write for boards whose RGB variant was not detected.

**Service hardening gaps (Windows):**
- Risk: No failure actions, no service description, no restricted SID type, and no least-privilege token beyond the SCM defaults (`src/service/windows/lifecycle.rs`). `OpenSCManagerW(SC_MANAGER_ALL_ACCESS)` is broader than needed for status queries (`:207,246`).
- Files: `src/service/windows/lifecycle.rs`, `data/systemd/zenomen-hw.service`
- Current mitigation: The Linux unit is well sandboxed (`ProtectSystem=strict`, `ReadOnlyPaths=/`, `PrivateDevices=true`, `SystemCallFilter=@system-service`, `CapabilityBoundingSet=` empty).
- Recommendations: Add `SERVICE_CONFIG_FAILURE_ACTIONS`, `SERVICE_CONFIG_DESCRIPTION`, and `SERVICE_SID_TYPE_UNRESTRICTED` where appropriate; mirror the Linux sandbox intent in the Windows service token.

**Environment-controlled transport switch:**
- Risk: `ZENOMEN_HW_SESSION` selects the session bus instead of the system bus in the daemon (`src/bin/zenomen-hw.rs:59`, used by `src/service/contract.rs`).
- Files: `src/bin/zenomen-hw.rs`, `src/service/contract.rs`
- Current mitigation: Documented as a test hook; the systemd unit does not set it.
- Recommendations: Gate the override behind `#[cfg(test)]` or a `--session-bus` flag that the packaged unit never passes, so an inherited environment cannot downgrade the daemon.

---

## Performance Bottlenecks

**Mutation timeout leaks threads and handles:**
- Problem: `execute_mutating_action_with_timeout` spawns a wrapper thread and an inner thread per action (`src/core/worker.rs:390-435`); on timeout the caller gets an error but the inner thread stays blocked in the transport forever. On Windows the transport is `ReadFile` on a message-mode pipe with no overlapped I/O or read timeout (`src/service/windows/client.rs:149`).
- Files: `src/core/worker.rs`, `src/service/windows/client.rs`, `src/service/client.rs`
- Cause: No cancellation mechanism; the pipe handle is owned by the abandoned thread.
- Impact: Each hang leaks one thread + one handle; a restarted service or a wedged driver turns a single UI click into permanent resource growth. The "in-flight" guard (`IN_FLIGHT` style tracking) stays set, so subsequent mutations are serialized behind a phantom operation.
- Improvement path: Use overlapped I/O with `CancelIoEx` + `WaitForSingleObject`, or a move-only `Arc<Mutex<Connection>>` whose owner performs a bounded `SetCommTimeouts`/`PIPE_NOWAIT` read. At minimum, track and log leaks by thread ID and make the in-flight guard clear on timeout.

**Blocking accept with a 30 s hang budget:**
- Problem: `PIPE_TIMEOUT_MS = 30000` (`src/service/windows/pipe.rs:13`) plus a blocking `ConnectNamedPipe` in a single-threaded loop (`src/service/windows/server.rs:122-292`).
- Files: `src/service/windows/pipe.rs`, `src/service/windows/server.rs`, `src/service/windows/lifecycle.rs:207,246`
- Cause: One client at a time; SCM stop is reported with a 3 s wait hint while the loop can take 30 s.
- Impact: `zenomen-hw` can appear hung to the SCM and be killed, and every GUI request queues behind the current client. No per-client rate limiting exists (`server.rs` handles requests as fast as they arrive).
- Improvement path: Lower the timeout, check a stop flag between requests, and accept/close with overlapped I/O so `SERVICE_STOP` is honored in <1 s. Add a simple per-connection request budget.

**Unbounded UI event channel:**
- Problem: `mpsc::channel()` for `AppEvent` (`src/main.rs:325`) with no backpressure. Telemetry events arrive at the polling rate; UI callbacks run on the Slint event loop.
- Files: `src/main.rs`
- Impact: If the UI thread stalls (modal dialog, slow chart path generation), events accumulate without limit. Bounded by the 40 MB RSS budget claim and by chart caching, but no hard cap exists.
- Improvement path: Use `sync_channel` with a small bound and coalesce telemetry events (latest-wins) instead of queueing every sample.

**Per-tick process spawn if `nvidia-smi` is missing:**
- Problem: The NVIDIA watcher thread is spawned with `.expect(...)` (`src/hardware/linux/gpu.rs:226`); when the tool is absent, the sampling path attempts a process per tick.
- Files: `src/hardware/linux/gpu.rs`, `src/hardware/linux/provider.rs`
- Impact: Wasted forks/execs every poll (500 ms–5 s) on non-NVIDIA laptops, contrary to the lean-dependency goal.
- Improvement path: Resolve the tool once, cache a tri-state (present/absent/error) and back off on absence; drop the `expect` for a logged fallback.

---

## Fragile Areas

**Windows service dispatch and state:**
- Files: `src/service/windows/server.rs:11-104`, `src/service/windows/hp_wmi.rs:70-71`
- Why fragile: A 32-line match over `HardwareRequest` (`server.rs`, 32 `=>` arms) mixes authorization, hardware calls, and response construction; the global `Mutex<Option<String>>` statics make results order-dependent and shared across connections (including tests running in one process).
- Safe modification: Extract one handler per request variant, return `Result<HardwareResponse, HardwareControlError>`, and remove the statics in favor of an explicit `FirmwareState` passed through `handle_request`.
- Test coverage: Mock-only (`tests/windows_service_tests.rs`); no test asserts failure paths for `set_fan_mode`/`set_rgb_profile`.

**Win32 tray pump and teardown:**
- Files: `src/desktop/windows/tray.rs:92-226,228-311`
- Why fragile: Raw message loop, `GWLP_USERDATA` pointer aliasing, `unsafe impl Send/Sync`, and a `PostMessageW(WM_CLOSE)` shutdown with no join. The leak documented above lives here. `RegisterClassW` return value is ignored (`:120-122`), so a second registration in the same process silently uses the existing class.
- Safe modification: Free user data in `WM_DESTROY`, store the pump `JoinHandle` in `TrayHandle`, check `RegisterClassW`, and add `SAFETY:` comments.
- Test coverage: None on Windows; `tests/desktop_integration_tests.rs` covers XDG paths only.

**Migration engine reads third-party files:**
- Files: `src/rgb/migration.rs:185-270`
- Why fragile: `fs::read_to_string` on arbitrary `.json` entries in `~/.config/openomen/rgb` (`:214`). A symlink to a FIFO blocks startup; a symlink to a huge file allocates unbounded memory. `target_path.exists()` then `fs::write` is a TOCTOU window, and `create_dir_all` errors are discarded (`:200`).
- Safe modification: `symlink_metadata` and refuse non-regular files, cap file size (e.g. 1 MiB), use `OpenOptions::create_new` for the target.
- Test coverage: `tests/rgb_tests.rs` covers parsing/preservation, not symlinks or size limits.

**Two hardware provider trees with divergent capability logic:**
- Files: `src/hardware/provider.rs:210-240`, `src/hardware/registry.rs`, `src/hardware/windows/provider.rs`
- Why fragile: `EXPECTED_SENSOR_COUNT = 11` and `is_degraded = reported < expected` mean a healthy Linux laptop without a dGPU is permanently "degraded"; Windows reports 11/11 from a simulated provider, so "degraded" is inverted between platforms.
- Safe modification: Derive expectations from detected devices (per-sensor availability) instead of a fixed count, or make `is_degraded` platform-specific and document it.
- Test coverage: `tests/sensor_registry_tests.rs`, `tests/degradation_tests.rs` cover Linux paths only.

**Frame parsing panics in tests, not production — but tests are the only protocol proof:**
- Files: `src/service/protocol.rs:264-322`, `src/service/windows/protocol.rs`
- Why fragile: The protocol tests `expect` on every decode, so a schema change fails as a panic rather than a typed assertion. Neither protocol has a round-trip test across both implementations.
- Safe modification: Add property tests for length-prefix boundaries (0, 4, `MAX + 1`, truncated) and a cross-codec test.
- Test coverage: Boundary cases exist (`protocol.rs:308-322`); no fuzzing.

---

## Scaling Limits

**Single-client service:**
- Current capacity: One pipe instance, one connection at a time (`src/service/windows/server.rs:122-292`).
- Limit: A second GUI client or a stuck client blocks all others; the SCM stop budget (3 s hint) is shorter than the accept timeout (30 s).
- Scaling path: Pre-create N pipe instances and dispatch per-connection threads (or overlapped I/O in one thread).

**Fixed 4-zone RGB model:**
- Current capacity: `[RgbColor; 4]` (`src/rgb/schema.rs:126`) and 4-zone payload (`src/service/windows/hp_wmi.rs:187-195`).
- Limit: 3-zone or per-key keyboards cannot be represented without a schema migration; `parse_openomen_profile` rejects anything that does not resolve all 4 zones (`src/rgb/migration.rs:160-171`).
- Scaling path: Version the profile schema (`zones: Vec<RgbColor>` + `zone_count`), keep a 4-zone deserializer for v1 files.

**History and chart buffers:**
- Current capacity: 120 samples default, 300 selectable; chart paths cached per size (`src/core/telemetry.rs:27,172-210`).
- Limit: `generate_chart_paths_cached` invalidates both chart and sparkline caches on every push (`src/core/telemetry.rs:190-192`), so a 300-point window regrows path strings every second; RSS headroom for the ≤40 MB budget shrinks with the largest window.
- Scaling path: Cache per-window paths lazily, or precompute incremental path segments.

**Command queue:**
- Current capacity: 8 commands, `try_send` with drop-and-log (`src/app.rs:30,50,392`).
- Limit: A burst of UI toggles (history window + polling + theme + autostart) silently drops commands; the UI has already applied the optimistic state, so it can desync from the worker/config.
- Scaling path: Merge idempotent config commands (coalesce last-writer-wins) or raise the bound and surface a "busy" state.

---

## Dependencies at Risk

**Slint 1.17 with hand-picked features:**
- Risk: `slint` is pinned to `1.17` with `default-features = false` and `renderer-femtovg`/`backend-winit` (`Cargo.toml`). Minor upgrades regularly change renderer/backend feature names and Slint's generated Rust API.
- Impact: A dependency bump can break `ui/*.slint` codegen or the femtovg renderer on either platform; `slint-build` must move in lockstep.
- Migration plan: Bump `slint` and `slint-build` together, run `xvfb-run cargo test --all`, and verify the Linux/Windows screenshot paths manually.

**`zbus 5` (blocking API) for D-Bus:**
- Risk: Blocking `Connection` + synchronous handlers is a thin layer over the async core; teardown semantics (drop = deregister) are implicit and version-sensitive.
- Impact: The `desktop_action_thread.join()` hang class of bugs depends on zbus drop behavior; upgrades can change it.
- Migration plan: Track zbus releases, add the teardown regression test described above, and consider explicit object-server removal.

**`windows-sys 0.59` with a large feature list:**
- Risk: `windows-sys` is a raw, semver-unstable surface; 0.59 → 0.60+ renames modules and types. The crate is pinned by version, not by a lock-file-only policy (the lock file is committed, so builds are reproducible until `cargo update`).
- Impact: A caret bump silently changes FFI signatures; the unsafe call sites in `src/hardware/windows/*`, `src/service/windows/*`, and `src/desktop/windows/*` all need review.
- Migration plan: Bump deliberately with `cargo update -p windows-sys --precise`, compile both targets, and re-audit every `unsafe` block.

**`winres 0.1` (build dependency):**
- Risk: `winres` is effectively unmaintained; it runs at build time for the Windows resource file.
- Impact: Build breakage on newer Rust/Windows SDKs; no security exposure at runtime.
- Migration plan: Move to `embed-resource` or a pre-generated `.res` if it breaks.

**No automated dependency updates:**
- Risk: No `.github/dependabot.yml` or Renovate config exists; `cargo audit` runs only in the Linux CI job (`.github/workflows/ci.yml:45-58`).
- Impact: Vulnerable transitive crates can sit unnoticed between manual audits; the Windows job never audits.
- Migration plan: Add Dependabot for `cargo` and run `cargo audit` in both jobs (or a dedicated job).

---

## Missing Critical Features

**Real Windows hardware backend:**
- Problem: No WMI session, no `hpqBIntM` invocation, no firmware read-back (see Tech Debt #1).
- Blocks: Any truthful Windows fan/power/RGB/battery feature; Windows release readiness; the `UnavailableHardwareProvider` promise.

**End-to-end Windows validation:**
- Problem: No hardware-in-the-loop test or documented validation matrix; `.planning/WINDOWS.md` records that CI has never executed (entry 2) and the original UI stub (entry 1).
- Blocks: Any claim that Windows support is "validated"; `resolve_board_validation` has no evidence behind its allow-list.

**Service lifecycle hardening:**
- Problem: No failure actions, no graceful stop within the SCM budget, no restart backoff, no structured audit log for privileged mutations.
- Blocks: Reliable unattended operation; forensic review after a malicious local client.

**Bounded/cancellable transport:**
- Problem: No overlapped I/O, no per-request deadlines on the pipe, no cancellation token shared with the worker timeout.
- Blocks: Reclaiming resources after a wedged firmware call; predictable UI latencies.

**Update mechanism:**
- Problem: Only a browser link to GitHub Releases (`src/desktop/releases.rs:13`), explicitly no HTTP/self-update.
- Blocks: Security patch delivery to installed users; version-skew detection between GUI and daemon.

---

## Test Coverage Gaps

**Windows service behavior is tested against the simulation:**
- What's not tested: That a mutation reaches firmware or fails when it cannot. `tests/windows_service_tests.rs:219-257` builds `HpWmiConnection { is_available: true }`, calls `handle_request(ApplyPreset, ...)`, and asserts `is_full_success()` plus `confirmed_fan_mode == Some("max")` — values produced by the in-process `Mutex` statics, not hardware.
- Files: `tests/windows_service_tests.rs:219-257`, `src/service/windows/hp_wmi.rs`
- Risk: The suite actively certifies the fabrication; a real implementation will initially "fail" these tests.
- Priority: High

**No Windows runtime tests for tray, pipe, autostart, notifications:**
- What's not tested: Tray teardown (the exit hang), DACL/session enforcement, `ReadFile` timeout behavior, HKCU Run round-trip, notification failure propagation.
- Files: `src/desktop/windows/*`, `src/service/windows/*`, `tests/windows_service_tests.rs` (non-Windows branch at `:259-349` only exercises guards)
- Risk: The highest-severity Windows bugs (exit hang, autostart no-op) have no regression test.
- Priority: High

**No cross-platform protocol compatibility test:**
- What's not tested: Encoding with `src/service/protocol.rs` and decoding with `src/service/windows/protocol.rs` (and vice versa), version mismatch handling, request-ID correlation.
- Files: `src/service/protocol.rs`, `src/service/windows/protocol.rs`
- Risk: Silent desynchronization if the transports are ever unified.
- Priority: Medium

**Migration robustness:**
- What's not tested: Symlinked/FIFO/oversized source files, concurrent target writes, permission-denied directories.
- Files: `src/rgb/migration.rs`, `tests/rgb_tests.rs`
- Risk: Startup hang or unbounded allocation from a hostile `~/.config/openomen/rgb`.
- Priority: Medium

**Mutation-timeout resource reclamation:**
- What's not tested: That the abandoned thread/handle is reclaimed, that the in-flight guard clears, and that a second mutation can proceed after a timeout.
- Files: `src/core/worker.rs:542-600` (tests assert the timeout event and guard behavior, not resource reclamation)
- Risk: Leaks accumulate in production with no signal.
- Priority: Medium

**Live-hardware validation:**
- What's not tested: Any real sysfs write/read-back on OMEN/Victus hardware, hwmon layouts with >9 temp inputs, multi-fan machines.
- Files: `tests/*` (all mock/temp-dir based), `src/hardware/registry.rs`
- Risk: Path-discovery regressions (e.g. `temp10` ordering, `platform` fan detection) ship undetected.
- Priority: Medium

---

## Prioritized Remediation Order

1. Stop the Windows lie: route Windows through `UnavailableHardwareProvider` until WMI is real, or implement `connect_hp_wmi`/read-back and remove the statics and ack-only `SetRgbEnabled`. Update `tests/windows_service_tests.rs` in the same change.
2. Fix the exit hang: free the `GWLP_USERDATA` sender on `WM_DESTROY`, join the pump thread, and add a teardown regression test.
3. Make autostart platform-correct and surface failures to the UI.
4. Add read cancellation (`CancelIoEx`/overlapped) and clear the in-flight guard on timeout; add a leak regression test.
5. Harden the root install path (trusted source + ownership check) and the `systemctl` invocation.
6. Tighten the pipe DACL/authorization and fail closed on security-descriptor creation.
7. Unify the protocol and add cross-codec/version tests.
8. Fix diagnostics/identity fabrication (`F.00`, hardcoded OS, static authorization string).
9. Add dependency automation and audit both CI jobs.

---

*Concerns audit: 2026-11-08*
