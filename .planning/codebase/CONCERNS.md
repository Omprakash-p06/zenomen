---
last_mapped_commit: 761e5f975d7c22d1c48bf777a0bcaa1f2db582ba
last_mapped_at: 2026-11-07
---
<!-- refreshed: 2026-11-07 -->

# Codebase Concerns

**Analysis Date:** 2026-11-07

**Validation basis:** Static review of the Rust sources, data files, tests, and documentation at commit `HEAD` on 2026-11-07. `cargo` is not installed in the analysis environment, so no `cargo test`, `cargo fmt --check`, or `cargo clippy` run was performed. Nothing below asserts that tests pass; findings were verified by reading source and cross-checking line references.

## Tech Debt

**History-window unit mismatch (UI says seconds, worker stores samples):**
- Issue: The settings UI labels the chart history options as `30s`, `60s`, `120s`, `300s` and the callback parameter is named `seconds`, but `SetHistoryWindow` is treated as a sample count. The displayed window duration therefore changes whenever `polling_interval_ms` changes, and the configured "seconds" value is never converted.
- Files: `ui/screens/settings.slint` (history-window options from line 348), `src/app.rs:120-124`, `src/core/worker.rs:156-172`, `src/core/worker.rs:190-201`
- Impact: At the default 1 s interval a "120s" selection holds 120 samples (≈120 s), but at a 2 s interval it holds 240 s of data. Users cannot reason about the window, and persisted config values are ambiguous.
- Fix approach: Convert seconds to samples at the command boundary using the active `sampling_interval` (`samples = seconds / interval`), store a duration in config, or rename the setting and labels to "samples". Recompute capacity when the polling interval changes; add a test asserting duration stays constant across interval changes.

**`SensorRegistry::detect_capabilities` hardcodes the RGB sysfs root:**
- Issue: Capability detection calls `detect_rgb_interface(Path::new("/"))` instead of deriving the root from the registry's scan roots or the provider's configured root.
- Files: `src/hardware/registry.rs:139`, `src/hardware/rgb.rs:33-53`
- Impact: Tests using fixture roots still probe the live host `/sys`, breaking test isolation and making custom-root providers report RGB capabilities they cannot actually control. It also contradicts the configurable-root design of `scan_roots` (`src/hardware/registry.rs:52-82`).
- Fix approach: Carry a `sysfs_root` (or an explicit RGB root) in `SensorRegistry`, or pass it into `detect_capabilities`; default to `/` only in `scan()`. Add a fixture-root test asserting RGB detection does not escape the root.

**Fan-control capability divergence between registry and daemon:**
- Issue: `SensorRegistry::resolve_fans` accepts `pwm1_enable` on any hwmon device, including a non-HP device, and reports `has_fan_control = true`. The privileged backend only accepts `hp-wmi`/`hp` devices and returns `InterfaceMissing` otherwise.
- Files: `src/hardware/registry.rs:256-315`, `src/service/backend.rs:77-115`
- Impact: The UI can advertise fan control, then every mutation fails with `InterfaceMissing`. The two discovery paths are independent implementations of the same rule and can drift again.
- Fix approach: Make the registry use the same acceptance predicate (hp/hp-wmi only) or have the daemon expose its resolved capability and let the UI consume it. Add a test with a non-HP hwmon `pwm1_enable` fixture asserting `fan_control == false`.

**Battery capability advertised but no battery telemetry exists:**
- Issue: `battery_metrics` is reported from `has_battery`, but `TelemetrySnapshot` has no battery fields and `LinuxHardwareProvider::sample_telemetry` never samples charge or power draw. The capability doc says "Battery charge/power can be read", while the registry doc says only presence is detected.
- Files: `src/hardware/registry.rs:30-35`, `src/hardware/registry.rs:149`, `src/hardware/provider.rs:45-46`, `src/hardware/provider.rs:152-181`, `src/hardware/linux/provider.rs:117-205`
- Impact: Any UI that trusts `has_battery_metrics()` will show a battery affordance with no data behind it. This violates the project rule that capabilities must not be fabricated.
- Fix approach: Rename the flag to `battery_present` and decouple it from `battery_metrics`, or add real charge/power fields to `TelemetrySnapshot` and sample them. Until sampling exists, return `battery_metrics: false` and document presence separately.

**NVIDIA detection path and invocation path disagree:**
- Issue: Capability detection checks absolute `/usr/bin/nvidia-smi` and `/bin/nvidia-smi`, but the sampler is constructed with the bare program name `"nvidia-smi"`, which resolves via `PATH`.
- Files: `src/hardware/linux/provider.rs:66-73`, `src/hardware/linux/gpu.rs:250-257`
- Impact: If the tool exists only at an absolute path outside the daemon's `PATH` (common under systemd, where `PATH` is minimal), the provider advertises a GPU it can never sample; conversely, a `PATH`-only tool is ignored during detection.
- Fix approach: Resolve the binary once to an absolute `PathBuf` during detection and pass that path to `NvidiaSampler::new`. Add a test for the detection/invocation path contract.

**Offline guidance hardcodes developer build paths:**
- Issue: When the daemon is absent, the client tells users to run `sudo target/debug/zenomen-hw` and `sudo target/debug/zenomen --install-system-integration`.
- Files: `src/service/client.rs:232-237`, `src/service/client.rs:239-248`
- Impact: Packaged users get instructions that cannot work; the message exposes a developer-only layout. It also contradicts `README.md` install guidance.
- Fix approach: Emit packaged guidance (`sudo systemctl start zenomen-hw` or reinstall the package). Centralize the message in one constant and cover it with a test.

**RGB client methods bypass the offline pre-check:**
- Issue: `apply_rgb_profile` and `set_rgb_enabled` call `connect()` directly, unlike fan/power/preset methods which call `connect_service()` and produce actionable offline errors.
- Files: `src/service/client.rs:301-344` vs. `src/service/client.rs:124`, `188`, `209`, `232-237`
- Impact: RGB failures surface raw zbus errors (`ServiceUnknown`/`UnknownMethod`) instead of the consistent "daemon not running" guidance, making support harder.
- Fix approach: Route both methods through `connect_service()` and `format_service_error()`; add a test that all mutating client methods share the same offline error prefix.

**Autostart entry is PATH-dependent and deletion is destructive:**
- Issue: The generated entry uses `Exec=zenomen --minimized` with no absolute path. Disabling autostart unconditionally deletes the file rather than setting `Hidden=true`, discarding any user edits. Startup also never reconciles `config.general.autostart` with the actual file state.
- Files: `src/desktop/autostart.rs:17-28`, `src/desktop/autostart.rs:78-93`
- Impact: Autostart silently fails when the desktop session's `PATH` does not include the install location. A user who customizes the entry loses their edits on toggle. A config/file mismatch makes the settings toggle lie about the real state.
- Fix approach: Write an absolute `Exec` resolved from the installed binary path, or use `Exec=zenomen` only when the desktop file install guarantees it. Disable via `Hidden=true` (or `X-GNOME-Autostart-enabled=false`) instead of deleting. Reconcile state at startup by writing or removing the entry to match config.

**Privileged CLI flags are position-agnostic and not platform-gated:**
- Issue: `--install-system-integration` and `--uninstall-system-integration` are detected with `args().any(...)` anywhere on the command line, and the handlers compile on every platform even though they write Linux paths.
- Files: `src/main.rs:40-65`
- Impact: A stray argument triggers a root filesystem mutation instead of launching the GUI; on non-Linux builds the code still attempts Linux integration paths. This is a surprising, destructive CLI surface.
- Fix approach: Parse arguments with a small explicit parser (only the first positional/subcommand form), and gate the handlers with `#[cfg(target_os = "linux")]`. Return a clear error on unsupported platforms.

**`USER_HZ` hardcoded to 100 in benchmark math:**
- Issue: `cpu_percent_between` divides jiffies by `USER_HZ = 100`, but `CLK_TCK` is not guaranteed to be 100. The measurement script correctly uses `getconf CLK_TCK`.
- Files: `src/core/benchmark.rs:142-143`, `src/core/benchmark.rs:232`
- Impact: CPU percentages reported by the benchmark are wrong by the ratio `CLK_TCK/100` on kernels configured differently, undermining budget verification.
- Fix approach: Read `getconf CLK_TCK` once (or parse `sysconf(_SC_CLK_TCK)` via libc) and pass it in; keep 100 only as a documented fallback. Add a test with an injected tick rate.

**Diagnostics export temp-name collision and no durability barrier:**
- Issue: The temp path is only `target.with_extension("tmp.<pid>")`, so two rapid exports in one process target the same file; the write is not `sync_all`ed before rename.
- Files: `src/core/diagnostics.rs:144-160`
- Impact: Concurrent/rapid exports can overwrite each other's temp file, and a crash after rename can leave a zero-length or truncated report on some filesystems.
- Fix approach: Include a monotonic counter or timestamp in the temp name, write via `File` + `write_all` + `sync_all`, then rename, matching `atomic_write_file` in `src/desktop/system_integration.rs:57-75`. Add a test for two sequential exports.

**Stale planning ledger entry:**
- Issue: `.planning/WINDOWS.md` entry 1 records a "Centred placeholder Text in `ui/app.slint`" that no longer exists; `ui/app.slint` is the full shell and contains no placeholder.
- Files: `.planning/WINDOWS.md:18`, `.planning/WINDOWS.md:24-34`, `ui/app.slint`
- Impact: The broken-windows gate reports an open defect that is already resolved, so `/gsd-ship` can block on a false positive and future agents may "fix" a non-issue.
- Fix approach: Mark entry 1 fixed with a resolution note, or regenerate the ledger from current code.

## Known Bugs

**Shutdown hangs when session D-Bus is unavailable:**
- Symptoms: Closing the app never exits when `ensure_single_instance` returns `Disabled` (no session bus). The process stays alive after the event loop ends.
- Files: `src/main.rs:77-92`, `src/main.rs:138-168`, `src/main.rs:270-278`
- Trigger: Run ZenOmen in an environment without a session bus (e.g., a bare TTY or a container) and quit the window/press quit.
- Cause: `action_sender` is moved into `register_tray` only in the `Primary` branch (`src/main.rs:81`). In the `Disabled` branch the original sender remains alive in `main`, so the `for action in action_receiver` loop (`src/main.rs:144`) never observes disconnection. `desktop_action_thread.join()` at `src/main.rs:278` then blocks forever.
- Workaround: Kill the process; there is no graceful path in this mode.
- Fix approach: `drop(action_sender)` explicitly after the match (or clone for tray and drop the original in all branches) so the receiver disconnects. Add a test that constructs the `Disabled` path with a fake sender and asserts the listener thread terminates.

**`SetRgbEnabled(true)` destroys the active profile with hardcoded white:**
- Symptoms: Turning RGB back on paints all four zones white, regardless of the profile the user had selected before turning it off.
- Files: `src/service/contract.rs:195-213`, `src/core/state.rs:223-241`
- Trigger: Apply a non-white RGB profile, toggle RGB off, then on.
- Cause: The D-Bus method has no memory of the previous profile; `enabled == true` always writes `RgbColor::new(255, 255, 255)` to four zones. On the UI side, `RgbPowerChanged` only updates `rgb_power_enabled` and leaves `active_rgb_profile` and its colors stale, so the UI shows the old profile while hardware is white.
- Workaround: Re-apply the desired profile after re-enabling.
- Fix approach: Have the daemon (or app) persist the last non-off profile and restore it on enable, or change the contract to `SetRgbEnabled(enabled, profile)` / `RestoreRgbProfile`. Update `RgbPowerChanged` handling to clear or refresh `active_rgb_profile` when the power state changes. Add a daemon test with a fixture root asserting the previous zone colors are written.

**Legacy RGB parsing can panic on non-ASCII content:**
- Symptoms: A legacy `keyboardleds` file with 24 bytes of non-ASCII text causes a panic at a byte-index slice, aborting the daemon (`panic = "abort"` in release).
- Files: `src/service/backend.rs:409-427`
- Trigger: A malformed or vendor-specific `keyboardleds` value whose byte length is 24 but whose character boundaries do not fall on multiples of 6.
- Cause: `trimmed.len() == 24` validates bytes, then `&trimmed[i * 6..(i + 1) * 6]` slices at byte offsets that may not be UTF-8 boundaries.
- Workaround: None from the UI; the daemon aborts and systemd restarts it.
- Fix approach: Parse the legacy value as bytes (`fs::read`, check `len == 24`, slice `&bytes[i*6..(i+1)*6]`) and reject non-ASCII/hex content with `InvalidParameter`. Add a test with 24-byte multibyte input.

**History capacity is not recomputed when the polling interval changes:**
- Symptoms: Changing the polling interval silently changes the wall-clock span of the chart, even though the selected "seconds" label is unchanged.
- Files: `src/core/worker.rs:190-201`, `src/core/worker.rs:156-172`, `src/core/state.rs:245-248`
- Trigger: Select `120s`, then change polling from 1 s to 2 s.
- Cause: Capacity is stored in samples (`AppEvent::ConfigLoaded` calls `history.set_capacity(config.telemetry.history_window)` at `src/core/state.rs:248`), but the interval change only updates `sampling_interval`, not the capacity.
- Workaround: Re-select the history window after changing polling.
- Fix approach: Store the history window as a duration and recompute samples whenever the interval changes; emit `ConfigLoaded`/`HistoryWindowChanged` with the recalculated value. Cover with a test.

**Unexpected `RequestName` reply codes silently exit the app:**
- Symptoms: A second launch can exit with status 0 and no window, even though no primary instance is handling `PresentWindow`.
- Files: `src/desktop/single_instance.rs:87-113`
- Trigger: A `RequestName` reply other than 1/2/3/4 (e.g., a protocol error or a proxy returning an unexpected code).
- Cause: The catch-all arm `_ => SingleInstanceResult::AlreadyRunning` (`src/desktop/single_instance.rs:112`) treats unknown replies as "already running". Additionally, failure to register `ApplicationService` is only logged (`src/desktop/single_instance.rs:91-93`), so the app can become primary without serving `PresentWindow`.
- Workaround: Launch again after killing stray processes.
- Fix approach: Treat only `1|4` as primary, `2|3` as already running, and everything else as `Disabled` with a warning. If `object_server().at` fails, degrade to standalone mode rather than claiming primary. Add tests for reply codes 0/5/unknown.

**`apply_rgb_profile` / `set_rgb_enabled` report raw errors when offline:**
- Symptoms: RGB actions fail with `ApplyRgbProfile failed: ...ServiceUnknown...` instead of the friendly offline message used by fan/power controls.
- Files: `src/service/client.rs:301-344`
- Trigger: Click an RGB control while `zenomen-hw` is not running.
- Cause: Direct `connect()` usage and `format!("... {e}")` instead of `connect_service()`/`format_service_error()`.
- Workaround: Read the fan/power error path for the correct guidance.
- Fix approach: Same as the tech-debt item above; route through the shared helpers.

## Security Considerations

**D-Bus activation bypasses the systemd sandbox:**
- Risk: The activation file declares `Exec=/usr/bin/zenomen-hw` and `User=root` but has no `SystemdService=zenomen-hw.service`. When dbus-daemon activates the name, it can exec the binary directly as root, outside `ProtectSystem=strict`, `ProtectHome`, `CapabilityBoundingSet=`, `NoNewPrivileges`, and the rest of the unit hardening.
- Files: `data/dbus/com.zenomen.Hardware1.service:1-4`, `data/systemd/zenomen-hw.service:14-32`, `Cargo.toml:90-99` (packaged files)
- Current mitigation: Only root may own the bus name (`data/dbus/com.zenomen.Hardware1.conf:5-9`), and the unit is installed. But the packaged DEB has no `postinst` to enable/start the unit (see `Cargo.toml` packaging metadata), so activation is a realistic path on a fresh install.
- Recommendations: Add `SystemdService=zenomen-hw.service` to the activation file so dbus-daemon delegates to systemd, and add a `postinst`/preset that enables the unit. Add a test asserting the activation file names the unit.

**Systemd unit grants broader sysfs write access than the daemon needs:**
- Risk: `ReadWritePaths` includes entire directories (`/sys/devices/platform/hp-wmi`, `/sys/firmware/acpi`, `/sys/class/hwmon`, `/sys/class/leds`), so any bug or compromised code path can write any file in those trees, not just `pwm1_enable`/`platform_profile`/RGB attributes. Hardening directives such as `PrivateDevices`, `ProtectClock`, `ProtectKernelLogs`, `ProcSubset=pid`, and `UMask` are absent.
- Files: `data/systemd/zenomen-hw.service:14-32`
- Current mitigation: `ProtectSystem=strict`, empty `CapabilityBoundingSet`, `NoNewPrivileges`, `SystemCallFilter=@system-service`, `MemoryDenyWriteExecute`.
- Recommendations: Narrow `ReadWritePaths` to the specific files/directories the backend uses, add the missing hardening directives, and set `UMask=0077`. Re-test fan/power/RGB writes under the tightened unit.

**Root daemon file logging silently fails under `ProtectHome=true`:**
- Risk: The daemon initializes the same file logger as the GUI, which resolves to `~/.local/state/zenomen/logs`. Under `ProtectHome=true` the root service cannot see `/home`, so file logging silently degrades to stderr-only. Support reports from the daemon then have no persisted evidence.
- Files: `data/systemd/zenomen-hw.service:16`, `src/logging.rs:23-33`, `src/logging.rs:76-103`, `src/bin/zenomen-hw.rs:10-13`
- Current mitigation: `logging::init()` degrades gracefully and journald still captures stderr.
- Recommendations: Give the daemon a system log path (e.g., `/var/log/zenomen` via `LogsDirectory=zenomen` and `StateDirectory=zenomen`) or explicitly document journald-only logging. Add a test or startup log line that states which sink is active.

**Root installer copies any sibling file named `zenomen-hw`:**
- Risk: `install_daemon_binary_from` takes `current_exe().with_file_name("zenomen-hw")` and `fs::copy`s it to `/usr/bin/zenomen-hw` as root after only `Path::is_file()`. If the user runs the installer from an attacker-writable directory containing a malicious `zenomen-hw`, or via a symlink, root installs attacker code. `uninstall_from_prefix` removes only the five text files and leaves `/usr/bin/zenomen-hw` behind, so a stale privileged binary persists after uninstall.
- Files: `src/desktop/system_integration.rs:78-119`, `src/desktop/system_integration.rs:176-192`
- Current mitigation: The installer requires root and is normally run from the packaged install location; `current_exe` is the running binary.
- Recommendations: Verify the source is a regular file (not a symlink), verify ownership/permissions, and prefer installing from the package payload or a path derived from a trusted prefix. Remove `DAEMON_BIN_TARGET` during uninstall. Add tests for symlink rejection and uninstall completeness.

**`systemctl` falls back to `PATH` resolution while running as root:**
- Risk: If neither `/usr/bin/systemctl` nor `/bin/systemctl` exists, `run_systemctl` executes bare `"systemctl"`, which a root `PATH` could resolve to an attacker-controlled binary.
- Files: `src/desktop/system_integration.rs:194-220`
- Current mitigation: Standard distributions provide `/usr/bin/systemctl`, so the fallback is rarely reached.
- Recommendations: Fail closed with `Command("systemctl not found in /usr/bin or /bin")` instead of falling back to `PATH`. Add a unit test for the resolver.

**Polkit authorization path is effectively untested:**
- Risk: The only polkit test checks response-signature serialization (`src/service/polkit.rs:74-105`). `HardwareService::authorize` (`src/service/contract.rs:56-80`) and the `with_polkit_required(false)` test helper (`src/service/contract.rs:44-48`) are never exercised, so a regression that skips authorization or mishandles a denial would not be caught.
- Files: `src/service/polkit.rs:74-105`, `src/service/contract.rs:44-48`, `src/service/contract.rs:56-80`, `src/service/contract.rs:117-213`
- Current mitigation: All mutating methods call `self.authorize(&hdr).await?` before touching the backend.
- Recommendations: Add tests for missing sender, missing connection, authorized, denied, and challenge responses, plus an integration test that invokes a mutating method without authorization and asserts the backend was not called.

**Read-only methods are unauthenticated by design:**
- Risk: `GetCapabilities`, `GetFanStatus`, `GetPowerProfiles`, `GetPowerProfile` are callable by any local user (`data/dbus/com.zenomen.Hardware1.conf:11-16`). They expose hardware model/state and service version.
- Files: `src/service/contract.rs:86-142`, `data/dbus/com.zenomen.Hardware1.conf:11-16`
- Current mitigation: Mutating calls are polkit-gated; read-only exposure is intentional for the UI.
- Recommendations: Confirm this is an accepted information-disclosure trade-off; if not, restrict read methods to the active session or document the exposure in `docs/`.

**Release `panic = "abort"` in a root daemon:**
- Risk: Any panic in a D-Bus handler kills the privileged daemon instead of unwinding to a per-request error. Combined with the legacy RGB slicing bug, malformed hardware content can take the service down until systemd restarts it.
- Files: `Cargo.toml:61-68`, `src/service/backend.rs:409-427`
- Current mitigation: `Restart=on-failure` in the unit (`data/systemd/zenomen-hw.service:11-12`).
- Recommendations: Keep `panic = "abort"` for the UI if size matters, but consider `panic = "unwind"` for the `zenomen-hw` profile, or audit handlers to eliminate panic paths (byte slicing, `unwrap`, indexing). At minimum, fix the known panic and add a fuzz/fixture test for malformed sysfs content.

## Performance Bottlenecks

**Blocking D-Bus calls on the monitoring worker have no timeout:**
- Problem: `HardwareServiceClient::status()` performs a synchronous `GetCapabilities`, and `check_polkit_status()` performs a synchronous polkit query. Both run on the worker thread every 5 samples. If the bus or daemon hangs, telemetry and command handling stop indefinitely, including `Shutdown`.
- Files: `src/service/client.rs:77-98`, `src/core/worker.rs:66-86`, `src/core/worker.rs:313-328`
- Cause: `zbus` blocking calls with default timeouts (or none) and no worker-side deadline.
- Improvement path: Use `zbus` async calls with an explicit timeout, or a dedicated status thread with a bounded channel and a watchdog. Add a test with a stub service that delays `GetCapabilities` and assert the worker still processes `Shutdown`.

**Blocking filesystem I/O and `thread::sleep` inside async D-Bus handlers:**
- Problem: Every mutating method is `async` but calls synchronous `fs::write`, `fs::read_to_string`, and `thread::sleep` in the retry loop. This blocks the zbus executor thread, serializing all requests (including reads) behind a polkit prompt or a slow sysfs write.
- Files: `src/service/contract.rs:117-213`, `src/service/backend.rs:12-48`
- Cause: The backend is synchronous and has no async boundary; `write_and_verify` sleeps between retries.
- Improvement path: Move backend work to `zbus::blocking`/`spawn_blocking` (or a dedicated worker) and await it, replace `thread::sleep` with an async delay, and add a concurrency test asserting a read completes while a slow write is in flight.

**`nvidia-smi` query has no timeout:**
- Problem: The watcher thread runs `Command::output()` with no timeout. A hung `nvidia-smi` (driver issue, GPU in a bad state) blocks the only watcher thread forever; `request()` then no-ops and `latest()` goes stale forever.
- Files: `src/hardware/linux/gpu.rs:235-264`
- Cause: `std::process::Command::output()` waits indefinitely.
- Improvement path: Wrap the child in a timeout (spawn + `try_wait` loop with a deadline, or `wait_timeout`), kill on expiry, and log a rate-limited warning. Add a test with a sleep script.

**RGB migration runs synchronously before window creation:**
- Problem: On every startup, `migrate_openomen_profiles` scans and copies legacy OpenOMEN profiles before the Slint window is created. Large profile directories delay first paint on the cold-start path that the RSS/startup budget measures.
- Files: `src/main.rs:97-112`, `src/rgb/` migration code
- Cause: Migration is unconditional and on the main thread.
- Improvement path: Run migration on a background thread after the window is shown, or gate it to run once via a version marker. Measure cold start before/after.

**History memory grows with configured capacity but the chart cost is unbounded by duration:**
- Problem: Capacity can be configured up to 300 samples (`src/core/telemetry.rs:104-121`), and every `ConfigLoaded` regenerates all chart/sparkline paths (`src/core/state.rs:245-252`). Combined with the unit mismatch, users can unintentionally hold a much longer window than the label implies, increasing per-frame path generation cost.
- Files: `src/core/telemetry.rs:104-137`, `src/core/state.rs:245-261`
- Cause: Path regeneration is O(samples) on every config/state change.
- Improvement path: Recompute paths only when the point set changes, and cap capacity by a duration budget rather than an arbitrary sample count.

## Fragile Areas

**Sysfs layout assumptions for HP hardware:**
- Files: `src/hardware/rgb.rs:33-53`, `src/service/backend.rs:77-115`, `src/hardware/registry.rs:256-315`, `src/hardware/linux/provider.rs:36-43`
- Why fragile: Three mutually exclusive RGB variants (`rgb_zones/zone00`, `keyboardleds`, `leds/hp_omen::kbd_backlight/zone_colors`), two fan-control discovery paths, and hardcoded `/sys/devices/platform/hp-wmi` / `/sys/class/hwmon` assumptions. A kernel or firmware change, or a non-HP machine with a `pwm1_enable`, changes behavior without a code change.
- Safe modification: Always go through the registry/backend discovery helpers; add fixture trees for each variant and a "non-HP pwm device" case. Never add a new hardcoded path outside `src/hardware/linux/` and `src/service/backend.rs`.
- Test coverage: Variants are covered by fixture tests (`tests/rgb_tests.rs`, `tests/fan_control_tests.rs`, `tests/degradation_tests.rs`), but real-host coverage is absent and the registry/daemon divergence is untested.

**Legacy RGB exact-length contract:**
- Files: `src/service/backend.rs:409-427`
- Why fragile: The code requires exactly 24 bytes and slices by byte offset; vendor kernels may emit different lengths, whitespace, or non-ASCII. This is both a correctness and a daemon-availability risk.
- Safe modification: Parse bytes, validate hex characters, and treat unexpected content as `InvalidParameter` rather than slicing. Add malformed-input tests.
- Test coverage: No test covers the legacy read path with malformed/non-ASCII content.

**Fan-mode sysfs encoding:**
- Files: `src/service/types.rs:21-39`, `src/hardware/registry.rs:99-114`
- Why fragile: `0/1/2` is mapped to `max/manual/auto`; if firmware uses a different encoding or adds a value, both the UI and daemon misreport state. `read_fan_mode` passes unknown non-empty strings through (`src/hardware/registry.rs:111`), while `FanMode::from_sysfs` may behave differently.
- Safe modification: Centralize the mapping in one type (`FanMode`) and have the registry reuse it. Add round-trip tests for every value.

**D-Bus service lifecycle and activation:**
- Files: `data/dbus/com.zenomen.Hardware1.service`, `data/systemd/zenomen-hw.service`, `src/bin/zenomen-hw.rs:20-44`, `src/desktop/single_instance.rs:87-113`
- Why fragile: The daemon relies on `std::future::pending()` to stay alive, systemd `Type=dbus`, and name ownership. The activation file and unit are two independent lifecycle definitions; the app-side single-instance logic has a silent fallback to "already running".
- Safe modification: Keep activation and unit metadata in sync; add a packaging test that asserts `SystemdService` points at the unit. Treat unknown bus replies as errors.

**Live-host tests:**
- Files: `tests/sensor_registry_tests.rs:246-262`, `tests/linux_procfs_tests.rs:55-76`, `tests/degradation_tests.rs:136-155`, `tests/fan_control_tests.rs:99-119`
- Why fragile: These tests instantiate `LinuxHardwareProvider::new()`, read `/proc`/`/sys`, or probe the system bus. They depend on the host having `k10temp`, a battery, or no production daemon; results differ between CI containers and developer laptops and violate the project rule to isolate tests from live hardware.
- Safe modification: Gate them behind `#[ignore]` or a `live-hardware` feature and run only in a dedicated job; keep fixture-based tests as the default suite.
- Test coverage: Live tests currently run unconditionally in `cargo test --all` (`ci.yml`), so CI can pass or fail based on the runner's hardware.

**Platform separation is incomplete:**
- Files: `src/hardware/linux/` (compiled unconditionally under `LinuxHardwareProvider`), `src/desktop/releases.rs:42-44`, `src/desktop/autostart.rs`, `src/desktop/system_integration.rs`, `src/core/diagnostics.rs:162-182`
- Why fragile: The project claims cross-platform behavior, but Linux-specific sysfs code lives in `src/hardware/linux/` and desktop helpers assume XDG/Linux. Diagnostics returns "Linux" when `/etc/os-release` is missing, which can mislabel other platforms.
- Safe modification: Gate Linux-only modules with `#[cfg(target_os = "linux")]`, use `xdg-open`/platform equivalents behind a small opener abstraction, and make OS detection return the actual platform fallback. Add a compile check for `--no-default-features` and non-Linux targets.

**Autostart reconciliation and user edits:**
- Files: `src/desktop/autostart.rs:30-93`, `src/core/worker.rs:181-183`, `src/core/worker.rs:202-210`
- Why fragile: Enabling always overwrites the file with the template; disabling deletes it. Any user customization is lost, and the config flag can disagree with disk state after manual edits or a failed write.
- Safe modification: Write `Hidden=true`/`X-GNOME-Autostart-enabled=false` on disable and preserve unknown keys on enable; reconcile at startup. Add tests for round-trip preservation.

## Scaling Limits

**Memory budget vs. measured RSS:**
- Current capacity: `docs/RESOURCE-BUDGET.md:10-22` records `Private_Dirty` of 7.15–28.50 MB and claims PASS for the ≤40 MB target, while the same table's measured `VmRSS` under Xvfb is 132.71–141.20 MB.
- Limit: The budget verdict is based on `Private_Dirty`, but `scripts/measure-budget.sh` samples `VmRSS` only and no CI gate enforces either metric. The 2-hour soak is explicitly `NOT MEASURED` (`docs/RESOURCE-BUDGET.md:48-55`).
- Scaling path: Decide which metric the budget means, make the script report both, add a CI budget check with a documented threshold, and run the soak on a real desktop. The integration test's `< 500 MB` assertion (`tests/benchmark_tests.rs:33-35`) is far too loose to catch regressions.

**Single-threaded daemon executor:**
- Current capacity: One async executor drives all D-Bus methods; each mutation performs synchronous I/O and sleeps.
- Limit: Concurrent callers queue behind the slowest write/polkit prompt; a hung filesystem operation stalls every request.
- Scaling path: Offload backend work to blocking tasks (see performance section) and bound the number of concurrent mutations.

**History buffer growth:**
- Current capacity: 2–300 samples (`src/core/telemetry.rs:104-121`), with per-update path generation.
- Limit: Memory is modest, but per-frame CPU grows with capacity and the unit mismatch can multiply the effective duration.
- Scaling path: Cap by duration and invalidate paths only when data changes.

## Dependencies at Risk

**`zbus` 5 with the `blocking` feature:**
- Risk: The blocking client and async service share one crate; the worker uses blocking calls while the daemon uses async methods with blocking internals. Version upgrades can change timeout defaults or executor behavior.
- Files: `Cargo.toml:34`, `src/service/client.rs`, `src/service/contract.rs`, `src/bin/zenomen-hw.rs`
- Impact: A silent behavior change in timeouts or name-request replies can reintroduce hangs or the single-instance false-positive.
- Migration plan: Pin the minor version, add contract tests for timeout/name-reply behavior, and prefer explicit timeouts over defaults.

**Slint 1.17 pinned with `renderer-femtovg`:**
- Risk: The UI stack is pinned and uses a software-rendering fallback under Xvfb; measured `VmRSS` is dominated by llvmpipe mappings.
- Files: `Cargo.toml:16-24`, `docs/RESOURCE-BUDGET.md:24`
- Impact: Upgrades may change RSS or rendering behavior; the budget documentation is already sensitive to the environment.
- Migration plan: Re-measure the budget on real GPU hardware before upgrading; keep the `--no-default-features` daemon build working independently of Slint.

**`tracing-appender` file logging in the daemon:**
- Risk: The daemon and GUI share one logging initializer and path, but the unit's `ProtectHome` makes the daemon's file sink fail.
- Files: `Cargo.toml:35`, `src/logging.rs`, `data/systemd/zenomen-hw.service:16`
- Impact: Silent loss of daemon logs.
- Migration plan: Add a daemon-specific sink or use journald; test the resolved path in both processes.

## Missing Critical Features

**No daemon-side authorization tests:**
- Problem: There is no test that exercises `HardwareService` methods end to end, including `authorize` allow/deny and backend invocation. `with_polkit_required` exists but is unused.
- Blocks: Confidence in the security boundary; any refactor of `contract.rs` can silently drop a polkit check.

**No shutdown/lifecycle tests for `main.rs`:**
- Problem: The `Disabled` hang, tray registration failure, and CLI flag handling have no tests because `main.rs` is a binary with inline logic.
- Blocks: Preventing the known shutdown hang from regressing and validating the single-instance fallbacks.

**No activation/unit consistency test:**
- Problem: Tests check that the D-Bus conf contains a root policy and that the unit has `Type=dbus`, but nothing asserts that the activation file delegates to the unit (`SystemdService=`) or that the packaged install enables it.
- Blocks: Catching the sandbox bypass on a fresh DEB install.

**No real battery telemetry:**
- Problem: `battery_metrics` is advertised but no charge/power fields exist in `TelemetrySnapshot` or the provider.
- Blocks: Any battery UI or diagnostics metric; the capability is currently misleading.

**No cross-platform opener abstraction:**
- Problem: `releases.rs` hardcodes `xdg-open` (`src/desktop/releases.rs:42-44`) despite cross-platform claims; autostart and system integration are Linux-only.
- Blocks: Honest non-Linux builds and a credible cross-platform story.

## Test Coverage Gaps

**Privileged service and polkit path:**
- What's not tested: `HardwareService` D-Bus methods, `authorize`, polkit denial/challenge, and the `with_polkit_required(false)` path.
- Files: `src/service/contract.rs:44-213`, `src/service/polkit.rs:28-105`
- Risk: A regression could allow unauthorized mutations or break all authorized ones without a failing test.
- Priority: High

**Application lifecycle and CLI:**
- What's not tested: `main.rs` shutdown wiring (especially the `Disabled` branch), tray registration failure, `--install-system-integration`/`--uninstall-system-integration` argument handling, and `--minimized`.
- Files: `src/main.rs:40-168`, `src/main.rs:259-281`
- Risk: The known shutdown hang and destructive flag behavior can regress unnoticed.
- Priority: High

**Systemd/D-Bus activation semantics:**
- What's not tested: The activation file's `SystemdService`, the unit's `ReadWritePaths`/hardening directives, `ProtectHome` vs. logging path, and packaged enablement.
- Files: `data/dbus/com.zenomen.Hardware1.service`, `data/systemd/zenomen-hw.service`, `Cargo.toml:90-99`, `tests/hardware_service_tests.rs:255-275`
- Risk: Security hardening can be bypassed or silently broken by a packaging change.
- Priority: High

**Legacy RGB malformed input:**
- What's not tested: Non-ASCII/24-byte content, wrong-length content, and partial zone data in the legacy read path.
- Files: `src/service/backend.rs:409-427`
- Risk: A malformed sysfs value panics the root daemon (abort profile).
- Priority: High

**Live-hardware tests are the default, not isolated:**
- What's not tested in isolation: Host-dependent behavior in `tests/sensor_registry_tests.rs:246-262`, `tests/linux_procfs_tests.rs:55-76`, `tests/degradation_tests.rs:136-155`, and `tests/fan_control_tests.rs:99-119`.
- Files: `tests/sensor_registry_tests.rs`, `tests/linux_procfs_tests.rs`, `tests/degradation_tests.rs`, `tests/fan_control_tests.rs`
- Risk: CI results depend on the runner's hardware and whether a production daemon is active; the suite can pass locally and fail in CI (or vice versa). This also violates the project rule to use isolated temp dirs and mock hardware.
- Priority: High

**Budget thresholds:**
- What's not tested: The ≤40 MB / ≤15 MB budget claims; the only assertion is `< 500 MB` in `tests/benchmark_tests.rs:33-35`.
- Files: `tests/benchmark_tests.rs`, `scripts/measure-budget.sh`, `docs/RESOURCE-BUDGET.md`
- Risk: Memory regressions up to 500 MB pass CI, and the documented PASS verdicts use a different metric than the script samples.
- Priority: Medium

**History-window semantics:**
- What's not tested: Duration stability across polling-interval changes, capacity recalculation, and the seconds-vs-samples contract.
- Files: `src/core/worker.rs:156-201`, `src/app.rs:120-124`, `src/core/state.rs:245-248`, `tests/telemetry_history_tests.rs`
- Risk: The UI label and actual data window diverge silently.
- Priority: Medium

**Autostart round-trip:**
- What's not tested: Preserving user edits, disable semantics (`Hidden=true` vs. delete), and startup reconciliation between config and file state.
- Files: `src/desktop/autostart.rs`, `tests/desktop_integration_tests.rs`
- Risk: User configuration loss and a toggle that misreports state.
- Priority: Medium

**Diagnostics export concurrency/durability:**
- What's not tested: Two rapid exports, crash between write and rename, and fsync behavior.
- Files: `src/core/diagnostics.rs:144-160`, `tests/diagnostics_tests.rs`
- Risk: Corrupt or lost support reports; low frequency but hard to diagnose.
- Priority: Low

---

*Concerns audit: 2026-11-07*
