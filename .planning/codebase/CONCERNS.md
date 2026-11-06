---
last_mapped_commit: 30d62de2cdf6f507822944e5602bdd907539ce38
last_mapped_at: 2026-11-06
---
# Codebase Concerns

**Analysis Date:** 2026-11-06

## Tech Debt

**Permanent degraded state from a hardcoded capability flag:**
- Issue: `src/hardware/registry.rs:133` hardcodes `battery_metrics: false` in `detect_capabilities()`, but `HardwareCapabilities` has eight subsystem flags and `src/core/state.rs:23` defines `SUBSYSTEM_COUNT = 8`. `reported_sensor_count()` therefore can never exceed 7 on real Linux, so `AppState::is_degraded()` is permanently `true` (see Known Bugs).
- Files: `src/hardware/registry.rs`, `src/hardware/provider.rs`, `src/core/state.rs`
- Impact: The "degraded hardware" warning is always visible on supported machines; the Diagnostics screen's battery row is a hardcoded false claim rather than a measured state. This undermines the project's core value that absence is *measured*, not guessed.
- Fix approach: Implement real battery detection by probing `/sys/class/power_supply/BAT*` (and populate `battery_metrics` plus a battery field on `TelemetrySnapshot`), or remove `battery_metrics` from the struct and reduce `SUBSYSTEM_COUNT` to 7 until battery telemetry exists. Whichever is chosen, make the count test in `src/core/state.rs` derive from the fields instead of a literal.

**`EXPECTED_SENSOR_COUNT` includes a sensor the Linux provider never sets:**
- Issue: `TelemetrySnapshot::EXPECTED_SENSOR_COUNT = 12` (`src/hardware/provider.rs:188`) counts `rgb_profile`, but `src/hardware/linux/provider.rs` never populates `rgb_profile` (grep for `rgb_profile:` in the Linux provider returns nothing). A fully healthy Linux machine therefore reports "11 of 12 sensors reported".
- Files: `src/hardware/provider.rs:183-213`, `src/hardware/linux/provider.rs`, `src/core/state.rs:298-301`
- Impact: Status-strip text is permanently wrong; the second `is_degraded()` branch reinforces the always-degraded bug above.
- Fix approach: Populate `rgb_profile` from the active RGB state (the state already exists in `AppState.active_rgb_profile`, but the provider is stateless), or exclude it from the sensor count and publish only the telemetry fields the provider actually emits. Keep `EXPECTED_SENSOR_COUNT` adjacent to `reported_sensor_count()` so the two cannot drift.

**Dead UI state field `pending_fan_mode`:**
- Issue: `AppState.pending_fan_mode` is read in `src/app.rs:432` and cleared in `src/core/state.rs:163`, but nothing in the codebase ever assigns `Some(...)`. `tests/fan_control_tests.rs:11,17,26` assert it stays `None`.
- Files: `src/core/state.rs:52,90,163`, `src/app.rs:432`, `tests/fan_control_tests.rs`
- Impact: The "Applying … and verifying read-back…" UI copy can never appear, so the verified-write behaviour — a headline product promise — has no in-progress affordance. Users see no feedback between clicking and the result event.
- Fix approach: Set `pending_fan_mode = Some(mode)` when dispatching `AppCommand::SetFanMode`, clear it on `FanModeChanged`/`ControlFailed`, and add a test that exercises the transition. Alternatively delete the field and the UI branch to avoid dead code.

**Duplicated polkit authorization plumbing:**
- Issue: `src/service/polkit.rs` builds the `CheckAuthorization` call by hand, and `src/service/client.rs:252-300` (`check_polkit_status`) hand-builds the same polkit method call again with its own argument marshalling. Both construct `org.freedesktop.PolicyKit1.Authority` / `CheckAuthorization` strings independently.
- Files: `src/service/polkit.rs`, `src/service/client.rs`
- Impact: Any polkit API detail change must be made twice; the client path is not exercised by the service path's tests.
- Fix approach: Extract one `check_authorization(conn, sender, action, allow_interaction)` helper (the service already has this in `src/service/polkit.rs`) and have the client call it through a shared module rather than re-implementing the call.

**Stringly-typed D-Bus contract duplicated between client and service:**
- Issue: `src/service/client.rs` invokes `call_method(..., "ApplyRgbProfile", ...)`, `"SetRgbEnabled"`, `"ApplyPreset"`, etc. as string literals; `src/service/contract.rs` declares the same names via `#[interface(name = "com.zenomen.Hardware1")]` method names. There is no generated zbus proxy, so a rename in the contract compiles cleanly and fails only at runtime.
- Files: `src/service/client.rs:302-344`, `src/service/contract.rs`
- Impact: Method-name drift is a silent runtime failure mode; argument-order drift is worse (positional `call_method` tuples).
- Fix approach: Generate a `#[zbus::proxy]` trait from the service interface (or a shared `const` module with method names and signatures) and call through it. This also removes hand-rolled body deserialization.

**Hardcoded filesystem roots bypass the backend test seam:**
- Issue: `HardwareBackend` has a configurable `sysfs_root` (`src/service/backend.rs:53-73`) used by `with_all_roots`-style tests, but `src/hardware/registry.rs:123` calls `detect_rgb_interface(std::path::Path::new("/"))` with a literal `/`, and `src/hardware/linux/provider.rs` reads `/proc/stat`, `/proc/meminfo`, and `/usr/bin/nvidia-smi` directly.
- Files: `src/service/backend.rs`, `src/hardware/registry.rs`, `src/hardware/linux/provider.rs`, `src/hardware/linux/gpu.rs`
- Impact: Capability detection and telemetry cannot be tested against a fixture tree, so tests depend on the host machine's hardware (see Fragile Areas) and CI coverage is weaker than it looks.
- Fix approach: Thread a `root: &Path` through `detect_capabilities` and the Linux provider constructor; resolve `/proc` and `nvidia-smi` through it. Add fixture-based tests under `tests/`.

**Configuration loaded twice with divergent fallbacks:**
- Issue: `src/main.rs:62-64` resolves the config path with a `config.toml` fallback and calls `load_or_default`; `src/core/worker.rs:84` independently calls `default_config_path()` and loads again, with its own fallback behaviour.
- Files: `src/main.rs:62-64`, `src/core/worker.rs:84-95`, `src/config/mod.rs`
- Impact: Two sources of truth for the same file can disagree (main uses one config, worker another) and a corrupt-config notice emitted by one path may not reach the other. Startup is also doing redundant disk I/O.
- Fix approach: Load once in `main`, pass `AppConfig` (and the corrupt notice) into the worker via the command channel or constructor. The worker's later reloads for settings changes should remain, but the initial load should not be duplicated.

**Documentation drift against code:**
- Issue: `README.md:70` says the D-Bus config installs to `/etc/dbus-1/system.d/…` while `Cargo.toml:71` and `src/desktop/system_integration.rs:14` install to `/usr/share/dbus-1/system.d/…`; `README.md:226` says 108 tests, `docs/PARITY.md` cites other counts, and the tree contains 110 `#[test]` attributes. `docs/PARITY.md` marks MON-02 (per-core CPU), MON-06 (VRAM/power draw), and RGB-02 (persistence) as `PARITY (Automated)` though the values are sampled and discarded or never persisted.
- Files: `README.md`, `docs/PARITY.md`, `docs/PACKAGING.md`, `Cargo.toml`, `src/desktop/system_integration.rs`
- Impact: Reviewers and the planner trust parity claims that the code does not support; test counts in the README mislead contributors.
- Fix approach: Derive counts in CI (a job that greps `#[test]` and asserts the README number), and downgrade PARITY rows to "PARTIAL" until the underlying behaviour is implemented. Fix the `/etc` vs `/usr/share` path in the README.

**Planning-state drift:**
- Issue: `.planning/state.json` marks Phase 5 `pending` while `docs/PACKAGING.md` and `docs/RESOURCE-BUDGET.md` describe Phase 5 deliverables as complete. Several prior `.planning/codebase/*.md` files show as deleted in `git status`.
- Files: `.planning/state.json`, `docs/PACKAGING.md`, `docs/RESOURCE-BUDGET.md`
- Impact: `/gsd-next` and progress reporting will route work back to a phase whose artifacts already exist or vice versa.
- Fix approach: Reconcile `state.json` with the actual phase artifacts before the next planning run.

## Known Bugs

**`AppState::is_degraded()` is always true on real Linux:**
- Symptoms: The UI's gold "degraded" warning is permanently shown; Diagnostics claims the battery device is absent even on machines with a battery.
- Files: `src/core/state.rs:289-303`, `src/hardware/registry.rs:133`, `src/hardware/provider.rs:188`, `src/app.rs:416`
- Trigger: Start the app on any machine where `detect_capabilities()` is used — `battery_metrics` is always `false`, so `reported_sensor_count()` is 7 < `SUBSYSTEM_COUNT` (8), and the first branch returns `true` before the snapshot count is even considered.
- Workaround: None; the warning cannot be dismissed.
- Fix: See Tech Debt's battery item. Add a regression test that builds a fully capable state and asserts `!is_degraded()`; the existing `src/core/state.rs:317-320` test only checks the capability count, not `is_degraded()` itself.

**`apply_preset` reports success when both power and fan writes fail:**
- Symptoms: `HardwareService::apply_preset` (`src/service/contract.rs:161-178`) returns `Ok(PresetResult)` whenever the backend returns `Ok`, and `HardwareBackend::apply_preset` (`src/service/backend.rs:269-350`) returns `Ok` even when `power_applied == false && fan_applied == false`. The message says "failed", but the D-Bus call is a success and `partial_success` is `false`.
- Files: `src/service/contract.rs:161-178`, `src/service/backend.rs:315-350`
- Trigger: Run `apply_preset("balanced")` on a machine with no `platform_profile` and no `pwm1_enable`; both legs error, the result is `Ok`.
- Impact: The client's error path is never taken for a total failure; the UI can show a benign state for a no-op. Contradicts "every write is verified".
- Workaround: Inspect `PresetResult.power_applied`/`fan_applied` on the client.
- Fix: Return `Err(HardwareControlError::VerificationFailed/InterfaceMissing)` when neither leg applied, and keep `Ok` only for at least one confirmed application. Add a test for the double-failure case.

**`set_rgb_enabled(true)` destroys the selected profile:**
- Symptoms: Enabling RGB writes hardcoded white to all four zones (`src/service/contract.rs:199-206`) instead of restoring the user's selected profile.
- Files: `src/service/contract.rs:195-212`
- Trigger: Select "Gaming" in the UI, toggle lighting off then on — the keyboard comes back white.
- Impact: RGB selection is not durable across power toggles; the UI's `active_rgb_profile` and hardware state diverge.
- Fix: Have the service accept a profile payload for enable, or have the client re-apply `active_rgb_profile` after enabling. At minimum, document that enable means "white".

**RGB selection and power state are never persisted:**
- Symptoms: `AppState` defaults to the first built-in preset (`src/core/state.rs:95`) and `rgb_power_enabled: true`; no state file is written. Only the legacy-import path touches `~/.config/zenomen/rgb/` (`src/rgb/migration.rs`).
- Files: `src/core/state.rs:95-96`, `src/app.rs`, `src/rgb/migration.rs`, `src/config/mod.rs`
- Trigger: Restart the app after choosing a profile — the default returns.
- Impact: RGB-02 ("persist active profile") is claimed in `docs/PARITY.md` but not implemented; the configuration system already exists and is unused for this.
- Fix: Add `rgb.active_profile` and `rgb.power_enabled` to `AppConfig` (`src/config/schema.rs`) and load them into `AppState` at startup; save on selection.

**Uninstall leaves the privileged daemon binary behind:**
- Symptoms: `uninstall_from_prefix` (`src/desktop/system_integration.rs:176-192`) removes the desktop entry, systemd unit, D-Bus conf/service, and polkit policy, but never removes `/usr/bin/zenomen-hw`, which the install path writes (`install_daemon_binary_from` with `target_bin`).
- Files: `src/desktop/system_integration.rs:176-192`, `src/desktop/system_integration.rs:87-119`
- Trigger: `zenomen --uninstall-system-integration` followed by `ls /usr/bin/zenomen-hw`.
- Impact: A root-owned binary remains executable after uninstall; re-installs can silently run a stale daemon (see next item).
- Fix: Add `prefix.join("usr/bin/zenomen-hw")` (or the resolved target) to `targets`, and add a packaging test.

**Installer silently accepts a stale daemon binary:**
- Symptoms: `install_daemon_binary_from` returns `Ok(())` when `candidate` is `None` but `target_bin.exists()` (`src/desktop/system_integration.rs:111-112`), with no version check.
- Files: `src/desktop/system_integration.rs:87-119`
- Trigger: Upgrade the GUI without shipping/placing a new `zenomen-hw`; the old daemon keeps running against the new client contract.
- Impact: Version skew between `zenomen` and `zenomen-hw`; the client already ignores `service_version` (see below), so mismatch is invisible.
- Fix: Compare `CARGO_PKG_VERSION` against a version reported by the daemon and refuse (or warn loudly) on mismatch; make the "target exists" branch an explicit opt-in.

**Client ignores the service version:**
- Symptoms: `src/service/contract.rs:98` publishes `"service_version": env!("CARGO_PKG_VERSION")` in capabilities, but `src/service/client.rs` never reads it.
- Files: `src/service/client.rs`, `src/service/contract.rs:98`
- Impact: A stale `zenomen-hw` (see above) is undetectable; new methods fail with opaque D-Bus "unknown method" errors.
- Fix: Parse `service_version` from `get_capabilities` in `status()` and surface a mismatch in `ServiceConnectionStatus`/Diagnostics.

**RGB client methods bypass the shared connection path:**
- Symptoms: `apply_rgb_profile` (`src/service/client.rs:302`) and `set_rgb_enabled` (`src/service/client.rs:325`) call `self.connect()` instead of `self.connect_service()`. The latter (line 232) presumably carries retry/service-activation handling used by the other methods.
- Files: `src/service/client.rs:302,325,232`
- Impact: RGB writes fail differently from fan/power writes when the daemon is not yet activated or the bus is briefly unavailable; no retry.
- Fix: Route both through `connect_service()`.

**Logging initialized after D-Bus setup, losing early diagnostics:**
- Symptoms: `src/main.rs:72-85` creates the single-instance connection and registers the tray, then `src/main.rs:89` calls `tracing_subscriber::fmt::init()`. Any `tracing::warn!` before line 89 (e.g. the `Disabled(e)` branch at line 83) is dropped.
- Files: `src/main.rs:63-91`
- Trigger: Run without a session bus; the "running in standalone mode" warning never appears.
- Impact: Hard-to-diagnose startup failures in exactly the environment where logging matters most.
- Fix: Initialize the subscriber before single-instance/tray setup.

**Teardown can hang joining the desktop-action thread:**
- Symptoms: `src/main.rs:245-253` drops the session connection and then `let _ = desktop_action_thread.join();` with no timeout.
- Files: `src/main.rs:138,245-253`
- Trigger: Close the window while a `DesktopAction` handler is blocked on a D-Bus call to a wedged session bus.
- Impact: The process does not exit; tray/desktop integration appears hung.
- Fix: Give the action thread a shutdown signal and a bounded join, or use a channel `recv_timeout` in the loop so it exits promptly when the sender is dropped.

**`nvidia-smi` query thread/process leak on timeout:**
- Symptoms: `sample_nvidia_smi` (`src/hardware/linux/gpu.rs:187-219`) spawns a thread per tick, waits 200 ms, and on timeout deliberately does not join ("let thread detach"). The detached thread's `Command::output()` may still be running `nvidia-smi`.
- Files: `src/hardware/linux/gpu.rs:187-219`, `src/hardware/linux/provider.rs`
- Trigger: A dGPU in D3cold or a slow driver where `nvidia-smi` takes > 200 ms (common on first call and on hybrid laptops).
- Impact: At 1 Hz sampling, up to one thread + one `nvidia-smi` process per second can accumulate during a slow period; this directly threatens the ≤40 MB RSS and ≤0.1% idle CPU budgets and is invisible to the current measurement script (which samples only the parent process).
- Fix: Run `nvidia-smi` on a single long-lived watcher thread with a bounded queue (drop stale requests), or use `tokio::process` with kill-on-timeout. Record child-process RSS/CPU in `scripts/measure-budget.sh`.

**`nvidia-smi` timeout is too aggressive for first call:**
- Symptoms: 200 ms is below typical cold `nvidia-smi` latency (often 300 ms–1 s).
- Files: `src/hardware/linux/gpu.rs:187`
- Impact: First samples after boot/resume report "Unavailable" even though the GPU is healthy, which the UI then presents as a sensor failure.
- Fix: Use a longer timeout (e.g. 1–2 s) on a dedicated thread with stale-result dropping, rather than a short timeout on a per-tick thread.

**Sampled values discarded before reaching the UI:**
- Symptoms: `src/hardware/linux/provider.rs:127` destructures `(aggregate_pct, _per_core)` and drops per-core utilization; `sample_amd_gpu` reads `mem_info_vram_used`/`mem_info_vram_total` (`src/hardware/linux/gpu.rs:166-177`) and `parse_nvidia_smi_output` reads VRAM (`gpu.rs:237-245`) but `TelemetrySnapshot` (`src/hardware/provider.rs:152-181`) has no VRAM or power-draw fields.
- Files: `src/hardware/linux/provider.rs:127`, `src/hardware/linux/cpu.rs:114-152`, `src/hardware/linux/gpu.rs`, `src/hardware/provider.rs:152-181`
- Impact: MON-02 and MON-06 are claimed as PARITY in `docs/PARITY.md` but the values never reach the snapshot; the work is paid for and thrown away.
- Fix: Add `per_core_utilization: Vec<f32>`, `gpu_vram_used_mb`/`gpu_vram_total_mb`, and `gpu_power_w` to `TelemetrySnapshot`, populate them, and render them (or stop sampling them until the UI exists).

## Security Considerations

**Polkit policy demands admin authentication for every control change:**
- Risk: `data/polkit/com.zenomen.hardware.policy` sets `allow_active` to `auth_admin_keep` for `com.zenomen.hardware.control`; every fan/power/RGB change prompts for an administrator password (cached briefly). This is not a bypass, but it is a UX/design mismatch for a "quiet, premium system utility" and encourages users to keep a root shell open instead.
- Files: `data/polkit/com.zenomen.hardware.policy`, `src/service/contract.rs:56-77`
- Current mitigation: Polkit is enforced on every mutating method via `authorize()`, which fails closed if the caller sender is missing.
- Recommendations: Decide the intended trust model. If the local active user should be able to change fans without a password, use `allow_active=yes` for the control action (still `auth_admin` for inactive/remote). If admin auth is intended, surface the prompt reason clearly and avoid re-prompting within the `auth_admin_keep` window. Add an integration test that asserts an unauthenticated caller is denied (none exists — see Test Coverage Gaps).

**Read methods are unauthenticated and expose hardware telemetry to every local user:**
- Risk: `get_capabilities`, `get_fan_status`, `get_power_profile`, `get_rgb_status`, etc. in `src/service/contract.rs` have no `authorize()` call, and `data/dbus/com.zenomen.Hardware1.conf` allows `send_destination` for the default policy. Any local user can read fan speeds, temperatures, and hardware model strings.
- Files: `src/service/contract.rs`, `data/dbus/com.zenomen.Hardware1.conf`
- Current mitigation: The D-Bus name is root-owned (`<policy user="root">`), so only root can impersonate the service.
- Recommendations: This is probably acceptable (telemetry is not secret), but document it explicitly. If a multi-user host is a target, add a `get_capabilities`-level restriction or a separate read action with `allow_any=yes`.

**Systemd sandbox does not actually bound sysfs writes:**
- Risk: `data/systemd/zenomen-hw.service` uses `ProtectSystem=strict` plus `ReadWritePaths=-/sys/devices/platform/hp-wmi …`. `ProtectSystem=strict` remounts most of the filesystem read-only but explicitly exempts `/sys`, so the `ReadWritePaths` carve-outs are not the effective boundary — the daemon can write anywhere in sysfs the kernel permits, including unrelated devices. The comment at lines 20-22 acknowledges `ProtectKernelTunables` was omitted for this reason.
- Files: `data/systemd/zenomen-hw.service:14-22`
- Current mitigation: The Rust backend only ever writes to hardcoded paths resolved under `/` (`src/service/backend.rs:64-73`), so application-level path control is the real boundary.
- Recommendations: Add `ProtectKernelTunables=yes` and rely on the service's own path allow-list for writes (test that fan/power/RGB still work), or drop the misleading `ReadWritePaths` entries and document that `/sys` is writable. Validate the unit under a real systemd boot, not just a unit-file lint.

**Privileged binary replacement without signature or hash verification:**
- Risk: `install_daemon_binary_from` (`src/desktop/system_integration.rs:87-119`) copies a candidate `zenomen-hw` to `/usr/bin` and chmods it `0755`. The install path runs as root (`--install-system-integration` requires it). There is no signature, hash, or package-manager ownership check.
- Files: `src/desktop/system_integration.rs:87-119`, `src/main.rs:35-56`
- Current mitigation: The candidate is expected to sit next to the GUI binary or come from the `.deb`; the release workflow builds both.
- Recommendations: Prefer installing the daemon only from the package (remove the manual copy path), or verify a SHA-256 manifest shipped alongside the binary. At minimum, log the source path and hash to the journal.

**Self-install writes to system paths from the GUI process:**
- Risk: The GUI, normally an unprivileged process, can invoke `install_system_integration`/`uninstall` (via CLI flags) that write to `/usr/bin`, `/usr/lib/systemd`, `/usr/share/dbus-1`, and `/usr/share/polkit-1`. The privilege boundary is the user's `sudo`, not the D-Bus service the project designed for privileged writes.
- Files: `src/main.rs:35-56`, `src/desktop/system_integration.rs:121-175`
- Current mitigation: These are explicit CLI flags, not reachable from the Slint UI.
- Recommendations: Keep installer actions in the package's `postinst`/`prerm` scripts instead of the GUI binary, or gate them behind a dedicated polkit action so the privilege path is auditable.

**Input validation is solid where it exists, but incomplete:**
- Risk: `set_power_profile` rejects newline/slash/NUL (`src/service/backend.rs:234-250`) and RGB colors are parsed through `RgbColor::from_hex` (`src/rgb/schema.rs:102-108`), so the previous Python `int(col[0:2], 16)` crash class is gone. However, `apply_rgb_profile` deserializes arbitrary JSON and trusts `profile.zones` length (fixed array `[RgbColor; 4]`), and `set_rgb_enabled` hardcodes white — no validation issue today, but no fuzz/property tests either.
- Files: `src/service/backend.rs:234-250`, `src/service/contract.rs:180-193`, `src/rgb/schema.rs`
- Current mitigation: Serde type-checking plus explicit character checks.
- Recommendations: Add a test that feeds malformed/hostile JSON to `apply_rgb_profile` and asserts `InvalidArgs` rather than a panic; consider `#[serde(deny_unknown_fields)]` if forward-compat is not required.

## Performance Bottlenecks

**`nvidia-smi` subprocess at 1 Hz:**
- Problem: Every telemetry tick spawns `nvidia-smi` (`src/hardware/linux/gpu.rs:187-219`) plus a thread, even when the GPU is idle or absent. Process spawn is the dominant per-tick cost and also counts toward RSS via the child.
- Files: `src/hardware/linux/gpu.rs`, `src/hardware/linux/provider.rs`
- Cause: No caching, no watcher thread, no capability gate (it runs whenever a dGPU is detected, and the 200 ms timeout means a slow call is retried next tick).
- Improvement path: A single `nvidia-smi` watcher thread (or `nvidia-smi --loop=1 --query-gpu=... --format=csv` streaming) publishing the latest sample over a channel; sample becomes a lock-free read. Disable entirely when no NVIDIA device is present.

**Blocking `std::thread::sleep` inside async zbus methods:**
- Problem: `write_and_verify` (`src/service/backend.rs:12-48`) sleeps between write and read-back (`50 ms` for fan, `10 ms` for RGB) and is called from `async fn` handlers in `src/service/contract.rs`. `std::thread::sleep` blocks the executor thread, and `HardwareBackend` is called directly (not `spawn_blocking`), so concurrent D-Bus requests serialize behind each sleep.
- Files: `src/service/backend.rs:12-48`, `src/service/contract.rs:120-212`
- Cause: Synchronous backend API used from async handlers.
- Improvement path: Make the backend methods `async` and use `tokio::time::sleep`, or wrap calls in `tokio::task::spawn_blocking`. With retries (3 × backoff), a single failing fan write can block the executor for ~350 ms.

**D-Bus status and polkit checks block the sampling thread every five ticks:**
- Problem: `src/core/worker.rs:299-308` calls `client.status()` and `client.check_polkit_status()` on the same thread that samples telemetry. Both are synchronous `zbus::blocking` calls (`src/service/client.rs:78,252`) that create a fresh connection each time (`self.connect()`), and `check_polkit_status` performs a full polkit `CheckAuthorization` round-trip.
- Files: `src/core/worker.rs:95,299-308`, `src/service/client.rs:59,78,232,252`
- Cause: Service health is polled inline with sampling instead of on its own cadence/thread.
- Improvement path: Move status/polkit probes to a separate low-frequency (e.g. 30 s) thread or an async task, and reuse a persistent connection. This also removes a per-call connection setup cost from the idle path.

**Resource budgets are not met and the measurement method masks it:**
- Problem: `docs/RESOURCE-BUDGET.md` claims PASS on a ≤40 MB UI budget, but its own table and `dist/budget-*.csv` show VmRSS of 132.71 MB (warm dashboard) and 95.57 MB (tray), with the doc substituting `Private_Dirty` (16.56/16.59 MB) for the requirement's RSS metric. The CPU row cites a 5-sample 0.0914% while the 60-sample artifact in the same CSV records 6.0233%. `scripts/measure-budget.sh` samples only the parent process (missing `nvidia-smi` children) and always exits 0, so it cannot gate anything.
- Files: `docs/RESOURCE-BUDGET.md`, `dist/budget-dashboard-open.csv`, `dist/budget-hw-daemon.csv`, `dist/budget-tray-minimized.csv`, `scripts/measure-budget.sh`
- Cause: Methodology redefines the metric after failing it; no CI gate; short sampling window; no child-process accounting.
- Improvement path: Decide whether the requirement is VmRSS or Private_Dirty (the PROJECT constraint says "peak RSS"), measure with the same definition, run a ≥300 s window, sum child processes, and make the script exit non-zero when a threshold is exceeded so CI can enforce it. Re-measure on real GPU hardware (llvmpipe inflates VmRSS, but may not explain 132 MB of file-backed pages alone).

**Two threads per mutating action, leaked on timeout:**
- Problem: `execute_mutating_action` (`src/core/worker.rs:350-380`) spawns a control thread and an RPC thread per action, waits 30 s, and on timeout drops the receiver — the RPC thread and any in-flight D-Bus/hardware write continue. There is no guard against a second action starting while the first is still in flight.
- Files: `src/core/worker.rs:350-380`
- Cause: Timeout implemented by abandoning the worker rather than cancelling it.
- Improvement path: Use a single control executor with a request queue and a cancellation token; if the hardware write must complete, report "still in progress" rather than timing out and letting a late success mutate state behind the UI's back. At minimum, keep the RPC `JoinHandle` and refuse new actions until it resolves.

**History and telemetry allocations per tick:**
- Problem: `TelemetrySnapshot` is cloned into `TelemetryHistory` (bounded `VecDeque`, `HISTORY_WINDOW = 120`) and into `AppState.latest_snapshot`; vectors (`fan_rpms`, `power_profiles`) are reallocated each sample, and SVG path strings are regenerated in `src/core/state.rs`/`src/app.rs` on every update.
- Files: `src/core/telemetry.rs`, `src/core/state.rs`, `src/app.rs`
- Cause: Snapshot is a plain cloneable struct; no `Arc` or reuse.
- Improvement path: Wrap the snapshot in `Arc<TelemetrySnapshot>` and clone the pointer; keep chart paths only for the visible screen. This is a secondary optimization behind the `nvidia-smi` and blocking-call fixes.

## Fragile Areas

**Worker control concurrency and timeout semantics:**
- Files: `src/core/worker.rs:350-380`
- Why fragile: Detached RPC threads, no in-flight guard, and a 30 s timeout that can fire before polkit interaction completes. A late success emits an event the UI no longer correlates with the original request, and two rapid toggles can race.
- Safe modification: Add a state machine (Idle → Pending → Applied/Failed) keyed by action, reject concurrent actions with a visible message, and only clear pending on a terminal event. Add `pending_fan_mode`/`pending_power_profile` state to make the lifecycle explicit.
- Test coverage: `tests/fan_control_tests.rs` asserts `pending_fan_mode` stays `None` (the opposite of the desired behaviour); no concurrency or timeout test exists.

**Service contract ↔ backend coupling and error mapping:**
- Files: `src/service/contract.rs`, `src/service/backend.rs`, `src/service/types.rs`
- Why fragile: `HardwareControlError` is converted to `zbus::fdo::Error` via `From`, and several handlers return `Ok` for partial failure (see Known Bugs). The `apply_preset` message is built by string concatenation, so the UI parses prose rather than structured fields.
- Safe modification: Change `apply_preset` to return `Err` on total failure, and add structured `PresetResult` fields to the event path instead of parsing `message`. Add tests per error variant.
- Test coverage: `tests/hardware_service_tests.rs` and `tests/power_preset_tests.rs` cover success and some errors, but not the total-failure contract.

**Capability detection heuristics and hardware assumptions:**
- Files: `src/hardware/registry.rs:117-160`, `src/hardware/linux/hwmon.rs`, `src/hardware/linux/gpu.rs`
- Why fragile: `detect_capabilities` hardcodes `battery_metrics: false` and probes RGB at `/` regardless of test roots; CPU temperature resolution is a priority list of driver names (`k10temp`, `zenpower`, …) that omits `hp-wmi`, so some OMEN/Victus models fall through to `acpitz` or `None`.
- Safe modification: Thread a root through detection, add `hp-wmi` to the temperature priority (after `k10temp`, before `acpitz`), and read battery presence from `/sys/class/power_supply`. Add fixture-based tests for each driver name.
- Test coverage: `tests/sensor_registry_tests.rs` covers priority logic with synthetic devices, but not the `hp-wmi` path or battery detection.

**Host-dependent integration tests:**
- Files: `tests/hardware_service_tests.rs`, `tests/degradation_tests.rs`, `tests/desktop_integration_tests.rs`
- Why fragile: Some tests attempt real system-bus connections or depend on `nvidia-smi`/sysfs presence; they pass or fail based on the machine and whether a session bus is available in CI.
- Safe modification: Gate hardware-dependent tests behind `#[ignore]` or an env var (`ZENOMEN_HW_TESTS=1`), and make the default suite use fixtures/`with_all_roots`. Add a CI job that runs the ignored set on a self-hosted runner if real-hardware coverage is wanted.
- Test coverage: See Test Coverage Gaps.

**RGB interface variant branching:**
- Files: `src/hardware/rgb.rs`, `src/service/backend.rs:439-500`, `src/rgb/migration.rs`
- Why fragile: Three incompatible interfaces (`FourZone`, `Legacy`, `KernelPatch`) are selected by filesystem probing; the `KernelPatch` branch writes 12 raw bytes with a single read-back and no retry/backoff, unlike the other branches which use `write_and_verify`.
- Safe modification: Route the `KernelPatch` branch through an equivalent retry helper (`write_and_verify_bytes`) and surface `VerificationFailed` consistently. Add a fixture tree per variant.
- Test coverage: `tests/rgb_tests.rs` covers schema/migration; variant detection and the raw-write path are under-tested.

**Single-instance and tray lifecycle:**
- Files: `src/main.rs:72-91,138-160`, `src/desktop/single_instance.rs`, `src/desktop/tray.rs`
- Why fragile: Logging init happens after registration; the tray/session connection is held in `_session_conn` and dropped at an unspecified point; teardown joins threads without timeouts. A second instance exits after triggering the primary over D-Bus, so any failure there is silent.
- Safe modification: Initialize logging first, hold the session connection in an explicit struct, and add bounded shutdown with a cancellation flag. Test "second instance exits and presents the first" under a private session bus (`dbus-run-session`).
- Test coverage: `tests/desktop_integration_tests.rs` covers some helpers; the full lifecycle is not exercised.

## Scaling Limits

**Telemetry history:** Bounded at 120 samples (`HISTORY_WINDOW` in `src/core/state.rs`), which is correct for the 2-minute requirement, but chart path regeneration walks the whole window on every tick (O(120) string building at 1 Hz). Fine today; if the window becomes configurable or the sample rate rises, move rendering off the sampling thread.

**Sensor/fan counts:** `fan_rpms` is a `Vec<u32>` sized by discovery, and `power_profiles` is a `Vec<String>`. The UI iterates both without a cap; a pathological hwmon tree with dozens of fans would grow the status strip. Cap at the number of UI slots or paginate.

**D-Bus connections:** The client opens a new connection for every `status()`, `check_polkit_status()`, `get_fan_status()`, and control call (`src/service/client.rs:59,232`). Each connection costs a socket and a round-trip; at 1 Hz status polling plus control actions this is measurable. A single persistent connection (or a zbus proxy) removes the limit.

**Process/thread growth under slow `nvidia-smi`:** See Known Bugs; at 1 Hz with a 200 ms timeout and detached threads, growth is unbounded for the duration of the slow period. This is the most likely path to breaching the RSS budget in the field.

**Log/event retention:** `TelemetryHistory` is bounded, but `AppState.power_profile_choices` and `last_control_error` are not; a service that returns a growing choices list would grow memory. Low risk.

## Dependencies at Risk

**`nvidia-smi` (external binary):**
- Risk: Not a Cargo dependency, but a hard runtime dependency for dGPU telemetry. Missing, old, or slow versions degrade or leak (see above); the 200 ms timeout assumes a fast binary.
- Impact: NVIDIA telemetry unavailable or the sampling loop stalls/leaks.
- Migration plan: Treat as optional (already returns `None`), add a capability probe at startup, and prefer reading `/sys/class/drm/card*/device/` where possible (utilization/power are not always exposed, so `nvidia-smi` stays for those fields).

**`zbus` 5 with the `blocking` feature:**
- Risk: The synchronous client (`src/service/client.rs`) and async service (`src/service/contract.rs`) share the same crate but different execution models; the `blocking` feature pulls in a nested runtime, and blocking calls from async contexts are a known footgun (see the perf section).
- Impact: Executor stalls, hard-to-debug deadlocks during teardown.
- Migration plan: Standardize on the async API with `tokio` (or zbus's own executor) for the service and a dedicated blocking thread for the UI, or move the client behind a channel-based actor. Pin the minor version until the migration.

**`slint` 1.17 with `renderer-femtovg` + `backend-winit`:**
- Risk: The RSS budget depends on this feature set; enabling `renderer-software` or additional backends would change memory characteristics. Upstream minor bumps can change defaults.
- Impact: Budget regression after an upgrade.
- Migration plan: Keep the feature set explicit (already done in `Cargo.toml`) and add a CI job that runs the budget script against the release profile on a fixed runner.

**`directories` 6.0 and `Cargo.toml` path assumptions:**
- Risk: Config paths (`~/.config/zenomen`) are stable, but the legacy migration reads `~/.config/openomen/rgb`; a future `directories` major could change `config_dir()` behaviour on some platforms.
- Impact: Profiles silently not found.
- Migration plan: Pin the major version and assert the resolved path in `tests/config_tests.rs`.

**`cargo-packager` / packaging metadata:**
- Risk: The `.deb` file mappings in `Cargo.toml:67-77` are the source of truth for installed paths, but `src/desktop/system_integration.rs` embeds its own copies of those paths. They agree today (`/usr/share/dbus-1/system.d/…`) but the README disagrees (`/etc/...`).
- Impact: A path change in one place installs files where the daemon does not look.
- Migration plan: Generate both from a single `const` module or assert equality in `tests/packaging_tests.rs` (which already compares embedded vs disk contents).

## Missing Critical Features

**Battery telemetry:**
- Problem: `HardwareCapabilities.battery_metrics` exists and `src/app.rs:416` binds it to the UI (`set_has_battery_metrics`), but no provider sets it to `true` and `TelemetrySnapshot` has no battery fields.
- Blocks: The "unavailable vs measured" contract for battery; the always-degraded bug; a truthful Diagnostics battery row.
- Fix: Probe `/sys/class/power_supply/BAT*` (`capacity`, `status`, `power_now`), add fields to the snapshot, and render them.

**Per-core CPU utilization in the UI:**
- Problem: `sample_proc_stat` returns per-core values (`src/hardware/linux/cpu.rs:114-152`) but the provider discards them (`_per_core`).
- Blocks: MON-02 parity and any per-core chart.
- Fix: Add `per_core_utilization` to `TelemetrySnapshot` and a UI view; or remove the computation and downgrade PARITY.

**GPU VRAM and power draw:**
- Problem: AMD sysfs and `nvidia-smi` both read VRAM (`src/hardware/linux/gpu.rs:166-177,237-245`), but the snapshot has no fields and the UI cannot show them.
- Blocks: MON-06 parity.
- Fix: Add `gpu_vram_used_mb`, `gpu_vram_total_mb`, `gpu_power_w` to `TelemetrySnapshot` (and parse `power.draw` from `nvidia-smi`), then render.

**RGB state persistence:**
- Problem: See Known Bugs. No `rgb.*` keys in `AppConfig`; active profile resets on restart.
- Blocks: RGB-02 parity.
- Fix: Extend `src/config/schema.rs`, load into `AppState`, save on change.

**Automated polkit enforcement:**
- Problem: `with_polkit_required` (`src/service/contract.rs:45-48`) exists only in `#[cfg(test)]` and is never called by a test; no test asserts that an unauthorized caller is denied.
- Blocks: Confidence in the SVC-03 security claim.
- Fix: Add a test that constructs `HardwareService` with `require_polkit = true` and a fake sender, asserting `AccessDenied`; run it under a private bus if possible.

**Two-hour soak and real-hardware budget validation:**
- Problem: `docs/RESOURCE-BUDGET.md` §3 marks the soak NOT MEASURED and the budget numbers are from Xvfb/llvmpipe.
- Blocks: Any credible claim that the ≤40 MB / ≤0.1% targets are met.
- Fix: Run the soak on a real laptop, record the CSV in `dist/`, and make the result a release checklist item.

**CI gates for budgets and end-to-end D-Bus:**
- Problem: `.github/workflows/ci.yml` runs fmt, clippy, tests under Xvfb, and `cargo audit` — no release-profile test, no coverage, no budget gate, no D-Bus service activation test. Unlike `release.yml`, the CI actions are not SHA-pinned (`actions/checkout@v4`, `taiki-e/install-action@…` without a digest).
- Blocks: Regressions in the exact areas the project cares about (memory, privilege path) pass CI silently.
- Fix: Add a `--release` test job, a budget script gate on a fixed runner, a `dbus-run-session` integration job, and pin actions by SHA.

## Test Coverage Gaps

**Polkit authorization enforcement:**
- What's not tested: That mutating D-Bus methods reject unauthorized callers; `with_polkit_required` is defined but unused. `tests/hardware_service_tests.rs:265-268` only greps the policy XML for `auth_admin_keep`.
- Files: `src/service/contract.rs:45-48,56-77`, `tests/hardware_service_tests.rs`, `tests/power_preset_tests.rs:223`
- Risk: A regression that skips `authorize()` (or a future method added without it) ships undetected — a privilege-escalation bug.
- Priority: High

**Fan/power control concurrency and timeout:**
- What's not tested: Two rapid actions, the 30 s timeout path, and late-success-after-timeout in `execute_mutating_action`. `tests/fan_control_tests.rs` asserts `pending_fan_mode` stays `None`, i.e. it tests the absence of the feature.
- Files: `src/core/worker.rs:350-380`, `tests/fan_control_tests.rs`
- Risk: Races and orphaned hardware writes reach users.
- Priority: High

**Total-failure contract for presets and RGB enable:**
- What's not tested: `apply_preset` returning `Ok` when both legs fail; `set_rgb_enabled(true)` overwriting the profile.
- Files: `src/service/backend.rs:315-350`, `src/service/contract.rs:161-212`
- Risk: The UI reports success for no-ops, violating the core verification promise.
- Priority: High

**Battery and capability-count invariants:**
- What's not tested: That a fully capable real-Linux state is not degraded; battery detection; `EXPECTED_SENSOR_COUNT` matching what providers actually set.
- Files: `src/core/state.rs:289-320`, `src/hardware/registry.rs`, `tests/sensor_registry_tests.rs`
- Risk: The always-degraded bug persists unnoticed.
- Priority: High

**`nvidia-smi` timeout/leak behaviour:**
- What's not tested: Slow/missing `nvidia-smi` under repeated sampling; thread/process accumulation. `tests/linux_gpu_tests.rs` covers parsing, not the spawn/timeout path.
- Files: `src/hardware/linux/gpu.rs:187-219`, `tests/linux_gpu_tests.rs`
- Risk: The RSS/CPU budget silently degrades on hybrid laptops.
- Priority: Medium

**Real-hardware / system-bus integration:**
- What's not tested in CI: Service activation via D-Bus, polkit in a real session, systemd unit sandboxing, install/uninstall round-trip. Some tests connect to the host bus and are environment-dependent.
- Files: `tests/hardware_service_tests.rs`, `tests/desktop_integration_tests.rs`, `tests/packaging_tests.rs`, `.github/workflows/ci.yml`
- Risk: Packaging and privilege-path regressions are found only by users.
- Priority: Medium

**Release-profile behaviour and resource budgets:**
- What's not tested: `cargo test --release`, the `panic = "abort"` profile interacting with tests, and any budget assertion. CI runs debug tests only.
- Files: `.github/workflows/ci.yml`, `Cargo.toml:36-42`, `scripts/measure-budget.sh`
- Risk: Release-only failures (optimizations, abort semantics) and budget regressions.
- Priority: Medium

**RGB interface variants and raw `KernelPatch` write:**
- What's not tested: Detection of all three variants against fixture trees; the retry/read-back path for the 12-byte `KernelPatch` write.
- Files: `src/hardware/rgb.rs`, `src/service/backend.rs:459-500`, `tests/rgb_tests.rs`
- Risk: A variant-specific write silently fails on some models.
- Priority: Medium

**Config corruption and dual-load divergence:**
- What's not tested: That `main` and the worker resolve the same file and agree on defaults when the config is corrupt; `tests/config_tests.rs` covers load/save but not the two call sites.
- Files: `src/main.rs:62-64`, `src/core/worker.rs:84`, `tests/config_tests.rs`
- Risk: Users see settings that do not take effect.
- Priority: Low

---

*Concerns audit: 2026-11-06*
