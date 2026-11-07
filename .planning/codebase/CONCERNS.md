---
last_mapped_commit: 4baf4bdbfc56a9755e48c37893d7515f1ffa172d
last_mapped_at: 2026-11-07
---
<!-- refreshed: 2026-11-07 -->

# Codebase Concerns

**Analysis Date:** 2026-11-07
**Branch:** `fix/zenomen-reliability`
**HEAD Commit:** `4baf4bdbfc56a9755e48c37893d7515f1ffa172d`

**Validation basis:** Static review of Rust sources, data files, build scripts, tests, and documentation at commit `4baf4bdbfc56a9755e48c37893d7515f1ffa172d`. Test coverage verified across dedicated unit test suites (`tests/sensor_registry_tests.rs`, `tests/linux_gpu_tests.rs`, `tests/logging_tests.rs`, `src/core/worker.rs:tests`, and `src/hardware/unavailable.rs:tests`).

---

## Recently Resolved Concerns

The reliability and platform hardening updates on branch `fix/zenomen-reliability` resolved several critical stability and architectural concerns:

| Resolved Concern | Location | Resolution Summary |
|---|---|---|
| **NVIDIA CLI Overhead & Cold-Start Latency** | `src/hardware/linux/gpu.rs:180-265`, `src/hardware/linux/provider.rs:27-172` | Replaced per-tick process execution and detached thread spawning with a single long-lived background `NvidiaSampler` thread (`zenomen-nv-watch`). Ticks request samples over a bounded sync channel (`capacity=1`) that coalesces rapid queries without blocking the 1 Hz monitoring loop. Telemetry is cached with a 3-second freshness window (`MAX_AGE`), returning `None` instead of stale or fabricated data if the dGPU is suspended in D3cold. |
| **Battery Capability Hardcoding** | `src/hardware/registry.rs:27-77, 146-168` | Eliminated hardcoded battery capability flags. `SensorRegistry::scan_roots` now performs dynamic sysfs battery presence scanning via `scan_battery_present()`, discovering `/sys/class/power_supply/BAT*` devices with readable `capacity` files. Correctly reports `battery_metrics = has_battery` without fabricating telemetry or assuming desktop machines have batteries. |
| **Worker Mutation Concurrency & Caller Timeout Vulnerability** | `src/core/worker.rs:26-30, 390-465` | Implemented `InFlightGuard(Arc<AtomicBool>)` in `execute_mutating_action_with_timeout`. Only one hardware mutation is permitted in flight; concurrent requests are rejected immediately with `"Another hardware change is still in progress"`. Even if the caller times out (e.g. 30s) waiting on authorization or sysfs I/O, the RAII `InFlightGuard` remains held in the background RPC thread until the physical operation finishes or unwinds, preventing race conditions and attribute corruption. |
| **Logging & Diagnostics in Headless/GUI Modes** | `src/logging.rs:1-103`, `src/main.rs:24`, `src/bin/zenomen-hw.rs:13` | Replaced ad-hoc stdout/stderr tracing init with `zenomen::logging::init()`. Configures an early non-blocking rotating file appender (`RollingFileAppender`, daily rotation, 7-day retention) targeting the platform state directory (`~/.local/state/zenomen/logs`). Initialized at process entry for both the GUI and `zenomen-hw` daemon; degrades gracefully to stderr if log directory creation fails, ensuring full diagnostics for terminal-less desktop launches. |
| **Unsupported Platform Compile Failures** | `src/hardware/unavailable.rs:1-52`, `src/hardware/mod.rs:15-20`, `src/main.rs:195-230` | Introduced `UnavailableHardwareProvider` for non-Linux targets (`#[cfg(not(target_os = "linux"))]`). Reports `HardwareCapabilities::none()` and returns honest non-implemented errors on sample attempts. Allows non-Linux builds (such as Windows architecture-readiness) to compile cleanly without referencing Linux-specific `/sys` or `/proc` files or fabricating fake hardware state. |

---

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

**Battery capability vs. telemetry gap:**
- Issue: Dynamic battery presence detection is implemented (`has_battery`), but `TelemetrySnapshot` does not yet contain charge percentage or power draw metrics (`src/hardware/provider.rs:152-181`). `has_battery` accurately reports presence, but full battery telemetry remains unimplemented.
- Files: `src/hardware/registry.rs:27-77`, `src/hardware/provider.rs:152-181`, `src/hardware/linux/provider.rs:117-205`
- Impact: UI cannot show real battery metrics (percentage, charge status) until sampling fields are added to `TelemetrySnapshot`.
- Fix approach: Add `battery_percent` and `battery_state` to `TelemetrySnapshot`, sample them from `/sys/class/power_supply/BAT*/capacity` and `status`, and expose them to the UI state.

**NVIDIA binary path resolution outside default PATH:**
- Issue: `NvidiaSampler::new("nvidia-smi")` relies on standard `PATH` resolution in `src/hardware/linux/provider.rs:73`, while detection in `provider.rs:67-70` checks explicit `/usr/bin/nvidia-smi` and `/bin/nvidia-smi`. Under stripped system environments where `PATH` does not include standard bin directories, execution could fail despite existence.
- Files: `src/hardware/linux/provider.rs:66-74`, `src/hardware/linux/gpu.rs:200-240`
- Impact: If `nvidia-smi` is located only in `/usr/bin` but not on current runtime `PATH`, `NvidiaSampler` could fail to spawn the query.
- Fix approach: Store the resolved absolute path during capability detection and pass that `PathBuf` directly into `NvidiaSampler::new(path)`.

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

**Autostart entry is PATH-dependent and deletion is destructive (Remaining Future Work):**
- Issue: The generated entry uses `Exec=zenomen --minimized` with no absolute path. Disabling autostart unconditionally deletes the file rather than setting `Hidden=true` or `X-GNOME-Autostart-enabled=false`, discarding any user customizations. Startup also never reconciles `config.general.autostart` with the actual file state.
- Files: `src/desktop/autostart.rs:17-28`, `src/desktop/autostart.rs:78-93`
- Impact: Autostart silently fails when the desktop session's `PATH` does not include the install location. A user who customizes the entry loses their edits on toggle. A config/file mismatch makes the settings toggle lie about the real state.
- Fix approach: Write an absolute `Exec` resolved from the installed binary path. Disable via `Hidden=true` or `X-GNOME-Autostart-enabled=false` instead of deleting. Reconcile state at startup.

**Privileged CLI flags are position-agnostic and not platform-gated:**
- Issue: `--install-system-integration` and `--uninstall-system-integration` are detected with `args().any(...)` anywhere on the command line, and the handlers compile on every platform even though they write Linux paths.
- Files: `src/main.rs:40-65`
- Impact: A stray argument triggers a root filesystem mutation instead of launching the GUI; on non-Linux builds the code still attempts Linux integration paths.
- Fix approach: Parse arguments with an explicit parser (only the first positional/subcommand form), and gate the handlers with `#[cfg(target_os = "linux")]`. Return a clear error on unsupported platforms.

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
- Impact: The broken-windows gate reports an open defect that is already resolved, so `/gsd-ship` can block on a false positive.
- Fix approach: Mark entry 1 fixed with a resolution note, or regenerate the ledger from current code.

---

## Known Bugs

**Shutdown hangs when session D-Bus is unavailable:**
- Symptoms: Closing the app never exits when `ensure_single_instance` returns `Disabled` (no session bus). The process stays alive after the event loop ends.
- Files: `src/main.rs:77-92`, `src/main.rs:138-168`, `src/main.rs:270-278`
- Trigger: Run ZenOmen in an environment without a session bus (e.g., bare TTY or container) and quit the window.
- Cause: `action_sender` is moved into `register_tray` only in the `Primary` branch (`src/main.rs:81`). In the `Disabled` branch the original sender remains alive in `main`, so the `for action in action_receiver` loop (`src/main.rs:144`) never observes disconnection. `desktop_action_thread.join()` at `src/main.rs:278` then blocks indefinitely.
- Workaround: Kill the process.
- Fix approach: `drop(action_sender)` explicitly after single-instance setup so the receiver disconnects when the window closes. Add a test asserting listener thread terminates in `Disabled` mode.

**`SetRgbEnabled(true)` destroys the active profile with hardcoded white:**
- Symptoms: Turning RGB back on paints all four zones white, regardless of the profile the user had selected before turning it off.
- Files: `src/service/contract.rs:195-213`, `src/core/state.rs:223-241`
- Trigger: Apply a non-white RGB profile, toggle RGB off, then on.
- Cause: The D-Bus method has no memory of the previous profile; `enabled == true` always writes `RgbColor::new(255, 255, 255)` to four zones. On the UI side, `RgbPowerChanged` only updates `rgb_power_enabled` and leaves `active_rgb_profile` and its colors stale, so the UI shows the old profile while hardware is white.
- Workaround: Re-apply the desired profile after re-enabling.
- Fix approach: Have the daemon or app state persist the last active profile and restore it upon re-enabling.

**Legacy RGB parsing can panic on non-ASCII content:**
- Symptoms: A legacy `keyboardleds` file with 24 bytes of non-ASCII text causes a panic at a byte-index slice, aborting the daemon (`panic = "abort"` in release).
- Files: `src/service/backend.rs:409-427`
- Trigger: A malformed or vendor-specific `keyboardleds` value whose byte length is 24 but whose character boundaries do not fall on multiples of 6.
- Cause: `trimmed.len() == 24` validates bytes, then `&trimmed[i * 6..(i + 1) * 6]` slices at byte offsets that may not be UTF-8 boundaries.
- Workaround: None from the UI; daemon aborts and systemd restarts it.
- Fix approach: Parse the legacy value as raw bytes, validate ASCII hexadecimal character sets, and reject invalid content with `HardwareControlError::InvalidParameter`.

**History capacity is not recomputed when the polling interval changes:**
- Symptoms: Changing the polling interval silently changes the wall-clock span of the chart, even though the selected "seconds" label is unchanged.
- Files: `src/core/worker.rs:190-201`, `src/core/worker.rs:156-172`, `src/core/state.rs:245-248`
- Trigger: Select `120s`, then change polling from 1 s to 2 s.
- Cause: Capacity is stored in samples (`AppEvent::ConfigLoaded` calls `history.set_capacity(config.telemetry.history_window)`), but the interval change only updates `sampling_interval`, not the capacity.
- Workaround: Re-select the history window after changing polling.
- Fix approach: Store the history window as a duration and recompute samples whenever the interval changes.

**Unexpected `RequestName` reply codes silently exit the app:**
- Symptoms: A second launch can exit with status 0 and no window, even though no primary instance is handling `PresentWindow`.
- Files: `src/desktop/single_instance.rs:87-113`
- Trigger: A `RequestName` reply other than 1/2/3/4 (e.g. protocol error or unexpected proxy response).
- Cause: The catch-all arm `_ => SingleInstanceResult::AlreadyRunning` treats unknown replies as already running. Additionally, failure to register `ApplicationService` is only logged, so the app can claim primary status without serving `PresentWindow`.
- Workaround: Kill stray processes and relaunch.
- Fix approach: Treat only `1|4` as primary, `2|3` as already running, and everything else as `Disabled` with a warning.

**`apply_rgb_profile` / `set_rgb_enabled` report raw errors when offline:**
- Symptoms: RGB actions fail with `ApplyRgbProfile failed: ...ServiceUnknown...` instead of the friendly offline message used by fan/power controls.
- Files: `src/service/client.rs:301-344`
- Trigger: Click an RGB control while `zenomen-hw` is not running.
- Cause: Direct `connect()` usage and `format!("... {e}")` instead of `connect_service()`/`format_service_error()`.
- Fix approach: Route through `connect_service()` and `format_service_error()`.

---

## Security Considerations

**D-Bus activation bypasses the systemd sandbox (Remaining Future Work):**
- Risk: The D-Bus activation file declares `Exec=/usr/bin/zenomen-hw` and `User=root` but omits `SystemdService=zenomen-hw.service`. When `dbus-daemon` activates the name on-demand, it spawns the binary directly as root outside systemd sandboxing (`ProtectSystem=strict`, `ProtectHome`, `CapabilityBoundingSet=`, `NoNewPrivileges`, etc.).
- Files: `data/dbus/com.zenomen.Hardware1.service:1-4`, `data/systemd/zenomen-hw.service:14-32`
- Current mitigation: Only root may own the bus name (`data/dbus/com.zenomen.Hardware1.conf:5-9`). When started via `systemctl start zenomen-hw`, systemd sandboxing applies fully.
- Recommendations: Add `SystemdService=zenomen-hw.service` to `data/dbus/com.zenomen.Hardware1.service` so `dbus-daemon` delegates execution to systemd.

**Systemd unit grants broader sysfs write access than needed (Remaining Future Work):**
- Risk: `ReadWritePaths` in `zenomen-hw.service` grants write access to entire directory trees (`/sys/devices/platform/hp-wmi`, `/sys/firmware/acpi`, `/sys/class/hwmon`, `/sys/class/leds`). A defect or vulnerability in the daemon could write to unintended attributes within those trees.
- Files: `data/systemd/zenomen-hw.service:19`
- Current mitigation: `ProtectSystem=strict`, empty `CapabilityBoundingSet`, `NoNewPrivileges`, `SystemCallFilter=@system-service`, `MemoryDenyWriteExecute`.
- Recommendations: Restrict `ReadWritePaths` specifically to the required control files (`pwm1_enable`, `platform_profile`, and specific LED brightness/color nodes). Add hardening directives such as `PrivateDevices=true`, `ProtectClock=true`, `ProtectKernelLogs=true`, and `ProcSubset=pid`.

**Root daemon logging under `ProtectHome=true`:**
- Risk: The file logger configured in `src/logging.rs` targets `~/.local/state/zenomen/logs`. Under `ProtectHome=true`, the root service cannot access `/home`, so the daemon's file appender fails creation and gracefully degrades to stderr (captured by journald).
- Files: `data/systemd/zenomen-hw.service:16`, `src/logging.rs:23-53`, `src/bin/zenomen-hw.rs:10-15`
- Current mitigation: Handled cleanly by graceful degradation in `open_file_appender()`; journald captures stderr output reliably.
- Recommendations: Add `LogsDirectory=zenomen` to `zenomen-hw.service` and configure the daemon to log to `/var/log/zenomen/` when run as root.

**Root installer copies any sibling file named `zenomen-hw`:**
- Risk: `install_daemon_binary_from` takes `current_exe().with_file_name("zenomen-hw")` and `fs::copy`s it to `/usr/bin/zenomen-hw` as root after only checking `Path::is_file()`. Running the installer from a shared or attacker-writable directory could allow unprivileged binary installation to `/usr/bin`.
- Files: `src/desktop/system_integration.rs:78-119`
- Current mitigation: Installer requires root privileges (`sudo`).
- Recommendations: Verify file ownership, reject symlinks, and enforce canonical package-derived source paths. Remove the binary on `uninstall_system_integration`.

**`systemctl` falls back to `PATH` resolution while running as root:**
- Risk: If `/usr/bin/systemctl` and `/bin/systemctl` are absent, `run_systemctl` invokes bare `"systemctl"`, resolving via root's `PATH`.
- Files: `src/desktop/system_integration.rs:194-220`
- Current mitigation: `/usr/bin/systemctl` is standard on systemd distributions.
- Recommendations: Fail closed with an explicit error rather than falling back to un-anchored `PATH` lookups.

**Polkit authorization path is effectively untested:**
- Risk: Unit tests verify serialization of polkit responses (`tests/`), but `HardwareService::authorize` end-to-end enforcement is not covered by a mock D-Bus client integration test.
- Files: `src/service/contract.rs:56-80`, `src/service/polkit.rs:28-105`
- Current mitigation: All mutating methods invoke `self.authorize(&hdr).await?` before backend execution.
- Recommendations: Add end-to-end integration tests asserting that requests without proper credentials return authorization errors and never execute sysfs writes.

**Read-only methods are unauthenticated by design:**
- Risk: `GetCapabilities`, `GetFanStatus`, `GetPowerProfiles`, and `GetPowerProfile` are callable by any local user over the system bus without authentication.
- Files: `src/service/contract.rs:86-142`, `data/dbus/com.zenomen.Hardware1.conf:11-16`
- Current mitigation: Mutating operations are strictly Polkit-gated; read-only operations expose hardware models and fan speeds for UI display.
- Recommendations: Document this intentional design decision in architecture security specs.

**Release `panic = "abort"` in a root daemon:**
- Risk: Any uncaught panic in `zenomen-hw` aborts the daemon immediately rather than returning an error to the caller.
- Files: `Cargo.toml:61-68`
- Current mitigation: `Restart=on-failure` in `zenomen-hw.service` restarts the service after 3 seconds.
- Recommendations: Audit all D-Bus handling code to prevent panics; consider configuring `panic = "unwind"` specifically for the daemon binary profile if feasible.

---

## Performance Bottlenecks

**Worker mutation concurrency & caller timeout vulnerability [RESOLVED]:**
- Resolution: Resolved by `InFlightGuard(Arc<AtomicBool>)` in `src/core/worker.rs:26-30, 390-465`. Hardware operations are serialized (one in-flight mutation at a time), and the guard remains held in the background RPC worker until physical completion even if the caller times out after 30s.

**NVIDIA CLI overhead & cold-start latency [RESOLVED]:**
- Resolution: Resolved by `NvidiaSampler` in `src/hardware/linux/gpu.rs:180-265`. Long-lived background thread (`zenomen-nv-watch`) with non-blocking coalescing requests and 3-second sample caching prevents repeated process spawning and stalls during D3cold suspension.

**Blocking filesystem I/O inside async D-Bus handlers:**
- Problem: Mutating methods in `HardwareService` (`src/service/contract.rs`) are declared `async`, but backend functions (`src/service/backend.rs`) execute synchronous `fs::write`, `fs::read_to_string`, and `thread::sleep` retry loops on the zbus executor thread.
- Files: `src/service/contract.rs:117-213`, `src/service/backend.rs:12-48`
- Impact: A slow sysfs write or unmaintained lock stalls all other D-Bus requests, including concurrent telemetry reads.
- Improvement path: Wrap backend calls in `tokio::task::spawn_blocking` or dedicated worker threads, using async sleep for retry delays.

**RGB migration runs synchronously before window creation:**
- Problem: `migrate_openomen_profiles` executes on the main thread prior to creating the Slint UI window (`src/main.rs:97-112`).
- Files: `src/main.rs:97-112`, `src/rgb/`
- Impact: Large legacy profile trees delay first window paint during cold-start.
- Improvement path: Defer migration to a background thread after the window is displayed, or gate it behind a single-run version stamp.

**History memory growth and per-frame chart regeneration:**
- Problem: Up to 300 samples are stored in telemetry history (`src/core/telemetry.rs:104-121`), and chart path generation executes repeatedly.
- Files: `src/core/telemetry.rs:104-137`, `src/core/state.rs:245-261`
- Impact: Path generation recalculates all points even when data points have not changed.
- Improvement path: Cache path geometries and invalidate only when new telemetry points arrive or dimensions change.

---

## Fragile Areas

**Unsupported platform compile failures [RESOLVED]:**
- Resolution: Resolved by `UnavailableHardwareProvider` in `src/hardware/unavailable.rs:1-52`. Non-Linux builds use this provider, reporting zero capabilities without attempting Linux sysfs/procfs I/O or failing compilation.

**Sysfs layout assumptions for HP hardware:**
- Files: `src/hardware/rgb.rs:33-53`, `src/service/backend.rs:77-115`, `src/hardware/registry.rs:256-315`
- Why fragile: Expects specific HP sysfs layouts (`rgb_zones/zone00`, `keyboardleds`, `leds/hp_omen::kbd_backlight/zone_colors`, `hp-wmi`). Kernel driver revisions or non-HP test environments can alter paths or attribute formats unexpectedly.
- Safe modification: Always route through discovery helpers with fallback checks; test against synthetic fixture directories.

**Legacy RGB exact-length contract:**
- Files: `src/service/backend.rs:409-427`
- Why fragile: Expects exact 24-byte strings and slices UTF-8 strings at byte boundaries `[i * 6..(i + 1) * 6]`. Malformed input causes slice panics.
- Safe modification: Parse as ASCII/hex bytes and return `InvalidParameter` on mismatch.

**Autostart reconciliation and user edits (Remaining Future Work):**
- Files: `src/desktop/autostart.rs:30-93`, `src/core/worker.rs:181-183`
- Why fragile: Disabling autostart deletes the `.desktop` file completely. Enabling writes a fixed template, wiping custom command-line arguments or environmental settings. Startup never synchronizes file state with `config.general.autostart`.
- Safe modification: Read and modify `Hidden=true` or `X-GNOME-Autostart-enabled=false` while preserving comments and custom attributes.

**Live-host tests in default suite:**
- Files: `tests/sensor_registry_tests.rs:246-262`, `tests/linux_procfs_tests.rs:55-76`, `tests/degradation_tests.rs:136-155`, `tests/fan_control_tests.rs:99-119`
- Why fragile: Some test cases read live host `/sys` and `/proc` entries directly. Results fluctuate depending on the host hardware (AMD Zen vs Intel vs VM).
- Safe modification: Move live-host tests behind a `--ignored` or `live-hardware` cargo feature flag; default all CI runs to fixture roots.

---

## Scaling Limits

**Single-threaded D-Bus backend executor:**
- Current capacity: One synchronous execution loop handles privileged mutations.
- Limit: Operations that prompt for polkit authentication block incoming requests until completed or timed out.
- Scaling path: Introduce async threadpool execution (`spawn_blocking`) for sysfs writes.

**Memory budget vs. measured RSS:**
- Current capacity: `docs/RESOURCE-BUDGET.md` documents `Private_Dirty` of 7.15–28.50 MB (PASS vs ≤40 MB target), while measured `VmRSS` under Xvfb is 132.71–141.20 MB due to mesa/llvmpipe allocations.
- Limit: CI lacks automated gating for memory benchmarks.
- Scaling path: Add a headless memory budget test in CI that verifies RSS against established thresholds.

---

## Dependencies at Risk

**`zbus` 5 with blocking client:**
- Risk: Both async service and blocking client are in use. Changes in default timeouts across minor versions can affect thread responsiveness.
- Mitigation: Pin minor version and enforce explicit per-call timeouts.

**Slint 1.17 pinned with `renderer-femtovg`:**
- Risk: Software rendering under Xvfb increases RSS footprint substantially.
- Mitigation: Retain decoupled daemon build (`--no-default-features`) and validate GUI performance on discrete hardware.

---

## Missing Critical Features

- **End-to-End Daemon Authorization Tests:** No integration tests verify polkit challenge/denial against `HardwareService` methods.
- **Battery Telemetry Metrics:** While presence detection (`has_battery`) is now dynamic, `TelemetrySnapshot` lacks charge percentage and power draw fields.
- **Cross-Platform Opener Abstraction:** `src/desktop/releases.rs:42` invokes `xdg-open` unconditionally on Linux; lacks macOS/Windows equivalents.

---

## Test Coverage Gaps

**Privileged service polkit paths:**
- What's not tested: `HardwareService::authorize` error responses and polkit denials.
- Priority: High

**Application lifecycle and D-Bus absent fallback:**
- What's not tested: `src/main.rs` clean shutdown when session bus is unavailable (`Disabled` single-instance result).
- Priority: High

**Legacy RGB malformed input:**
- What's not tested: Non-ASCII and uneven byte inputs to `keyboardleds` parser.
- Priority: High

**Autostart non-destructive round-trip:**
- What's not tested: Preserving user edits and custom fields in `zenomen.desktop`.
- Priority: Medium

---

## Remaining Future Work

1. **Hardening D-Bus Systemd Service Activation & Restricted Write Paths (`zenomen-hw`):**
   - **D-Bus Activation Delegation:** Update `data/dbus/com.zenomen.Hardware1.service` to include `SystemdService=zenomen-hw.service`. Currently, direct bus activation invokes `/usr/bin/zenomen-hw` without systemd sandboxing.
   - **Scoped Sysfs `ReadWritePaths`:** Narrow `data/systemd/zenomen-hw.service` from wide directory permissions (`/sys/devices/platform/hp-wmi`, `/sys/firmware/acpi`, `/sys/class/hwmon`, `/sys/class/leds`) to specific control attribute targets (`pwm1_enable`, `platform_profile`, and designated keyboard backlight sysfs files). Add `PrivateDevices=true`, `ProtectClock=true`, `ProtectKernelLogs=true`, and `ProcSubset=pid`.
   - **System Logging Target:** Provide dedicated service logging configuration (e.g. `LogsDirectory=zenomen` or explicit journald target) to avoid file write degradation when `ProtectHome=true` is enforced.

2. **Desktop Integration Autostart Non-Destructive Toggle:**
   - **Non-Destructive Disable:** Modify `src/desktop/autostart.rs:set_autostart_enabled_at` to write or update `Hidden=true` or `X-GNOME-Autostart-enabled=false` rather than calling `fs::remove_file(path)`.
   - **Preserve User Customizations:** When enabling, preserve existing keys, comments, and custom environment variables instead of overwriting the entire file with a static template.
   - **Absolute Exec Path:** Resolve `Exec` to the absolute binary installation path rather than relying on session `$PATH`.
   - **Startup Reconciliation:** Add startup logic in `src/main.rs` / `src/desktop/autostart.rs` to reconcile disk file state with `config.general.autostart`.

---

*Concerns audit: 2026-11-07 — Commit `4baf4bdbfc56a9755e48c37893d7515f1ffa172d`*
