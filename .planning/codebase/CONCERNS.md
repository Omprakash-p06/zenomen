---
last_mapped_commit: e50af8f9ea61300a1c10d36dc6954e319da7b6e2
last_mapped_at: 2026-11-02
---
<!-- refreshed: 2026-11-02 -->

# Codebase Concerns

**Analysis Date:** 2026-11-02

## Tech Debt

**Parity matrix cites files that do not exist: [RESOLVED]**
- Resolution: `docs/PARITY.md` updated to cite active implementation files (`src/hardware/linux/hwmon.rs`, `src/hardware/linux/cpu.rs`, `src/hardware/linux/memory.rs`, `src/hardware/linux/gpu.rs`) and test counts synchronized with automated test list (79 integration tests, 29 unit tests, 108 total automated tests).

**Two independent implementations of hp-wmi fan discovery:**
- Issue: `src/service/backend.rs` and `src/hardware/registry.rs` scan sysfs trees to identify hp-wmi interfaces and tachometers.
- Files: `src/service/backend.rs:76`–`157`, `src/hardware/registry.rs:22`–`60`
- Impact: Two rules for the same hardware fact; model quirk updates require dual-site editing.

**Fan RPM enumeration stops at the first device and hardcodes four channels: [RESOLVED]**
- Resolution: `HardwareBackend::get_fan_rpms` now discovers all `fan*_input` channels dynamically across search directories, strips suffixes, parses integer indices, deduplicates, and collects all available tachometers without a hardcoded ceiling of four.

**Preset application infers the fan mode from the preset name, not the verified hardware value: [RESOLVED]**
- Resolution: `PresetResult` now includes `confirmed_power_profile: Option<String>` and `confirmed_fan_mode: Option<String>` populated directly from the verified hardware read-back in `apply_preset`. `AppState::apply` prioritizes confirmed hardware read-back over preset name inference.

**Mutation worker spawns two threads per action and leaks one on timeout:**
- Issue: `execute_mutating_action` (`src/core/worker.rs:350`–`380`) starts a `zenomen-ctrl-*` thread which starts a `zenomen-rpc-*` thread, then waits with `recv_timeout(30s)`.
- Files: `src/core/worker.rs:350`–`380`
- Impact: Slow calls cost two OS threads during polkit challenge or hardware timeout.

**Configuration writes run synchronously on the sampling thread:**
- Issue: Settings mutations call `crate::config::save_atomic` inline on the sampling worker.
- Files: `src/core/worker.rs`, `src/config/mod.rs`
- Impact: Sampling can stall during disk fsync.

**Atomic config write is not durable and uses a fixed temp name: [RESOLVED]**
- Resolution: `save_atomic` now generates process-unique staging filenames (`.{stem}.{pid}.tmp`) preventing write collisions across instances or threads, and explicitly invokes `fs::File::open(parent_dir)?.sync_all()` after atomic replacement to ensure directory metadata is durable across power loss.

**Documented resource budget rests on a substituted metric:**
- Issue: `docs/RESOURCE-BUDGET.md` records Warm Dashboard `VmRSS 132.71 MB` under a ≤ 40 MB target and marks the row PASS on `Private_Dirty` (16.56 MB), attributing the difference to Mesa's `llvmpipe` under Xvfb. The two-hour tray soak is recorded as **NOT MEASURED** (`docs/RESOURCE-BUDGET.md` §3).
- Files: `docs/RESOURCE-BUDGET.md:19`–`31`, `docs/RESOURCE-BUDGET.md:49`–`59`
- Impact: The headline "≤ 40 MB peak RSS" claim is currently supported by an argument about which number counts, not by a measurement taken on the hardware users run. Anyone auditing the constraint will stop at the 132 MB cell.
- Fix approach: One measurement on a machine with a real GPU driver, reported as `VmRSS`/`VmHWM` in the same table, plus the executed soak output. Keep `Private_Dirty` as a secondary column, not the verdict.

## Known Bugs

**A verifying write can make the UI report the service as disconnected:**
- Symptoms: During fan/power/RGB application, the status strip can flip to "Disconnected" and back. `HardwareServiceClient::status()` treats *any* `GetCapabilities` error as `Disconnected` and clears the cached connection (`src/service/client.rs:78`–`99`); the daemon's async executor is blocked for the whole read-back retry, so the probe can time out.
- Files: `src/service/client.rs:78`–`99`, `src/core/worker.rs:299`–`318`, `src/service/backend.rs:12`–`50`
- Trigger: Click a fan mode or RGB preset, then watch the status strip within 5 s (the status probe cadence in `src/core/worker.rs:299`–`302`).
- Workaround: None needed for control itself — the write usually succeeds; the indicator is what lies.

**Second instance can exit without showing a window:**
- Symptoms: Rapid double-launch leaves no visible window. `ensure_single_instance` requests `com.zenomen.App` (`src/desktop/single_instance.rs:76`–`90`), but the object is registered on the bus only afterwards (`src/desktop/single_instance.rs:91`); a secondary process that wins the `EXISTS` branch (`src/desktop/single_instance.rs:96`–`108`) in that window calls `PresentWindow` against a name with no object path yet, ignores the error, and exits as `AlreadyRunning`.
- Files: `src/desktop/single_instance.rs:68`–`116`, `src/main.rs:124`–`151`
- Trigger: Start two instances back-to-back on a cold session bus.
- Workaround: Launch once more; the primary is registered by then.

**Addressed writes report success for values the kernel rewrites:**
- Symptoms: None today for `auto`/`max`. `write_and_verify` compares read-back case-insensitively against the *requested* string (`src/service/backend.rs:32`), which is correct for `pwm1_enable` and `platform_profile`. Any future target whose kernel representation differs from the request (numeric formatting, aliases) will fail verification even though the hardware accepted it.
- Files: `src/service/backend.rs:12`–`50`
- Trigger: Adding a new verified target with a non-identity mapping.
- Workaround: Supply a per-target normaliser instead of comparing raw strings.

## Security Considerations

**Authorization can be disabled at construction time: [RESOLVED]**
- Resolution: `HardwareService::with_polkit_required` is gated strictly behind `#[cfg(test)]`. In production binaries (`zenomen-hw`), the builder is uncompilable and nonexistent, ensuring that the daemon service unconditionally initializes with `require_polkit = true` and enforces Polkit authorization for all mutating calls.

**Mock provider ships in the release binary behind an environment variable:**
- Risk: `ZENOMEN_MOCK_SCENARIO` switches the UI to `MockHardware` (`src/main.rs:158`–`166`) with no debug-build gate. Fabricated temperatures and fan speeds can be presented in a production install, which directly contradicts "hardware control you can trust".
- Files: `src/main.rs:153`–`166`, `src/hardware/mock.rs`
- Current mitigation: The variable is undocumented and the UI labels the provider (`ui.set_provider_mode("mock")`, status text "Mock provider").
- Recommendations: Compile the mock provider only under `debug_assertions` or an explicit `mock-provider` feature, and keep it out of release packaging. The distinction between mock and real must not depend on a user-visible label alone.

**Daemon sandbox carve-outs are broad by necessity:**
- Risk: `data/systemd/zenomen-hw.service` grants `ReadWritePaths` over all of `/sys/class/hwmon` and `/sys/firmware/acpi` because it cannot know which device nodes exist at install time. Any write reachable from those trees is available to the daemon, so the D-Bus contract — not the filesystem — is the only boundary (`src/service/contract.rs`).
- Files: `data/systemd/zenomen-hw.service:18`–`30`, `src/service/contract.rs`, `src/service/backend.rs`
- Current mitigation: `ProtectSystem=strict`, `NoNewPrivileges=true`, empty `CapabilityBoundingSet`, `SystemCallFilter=@system-service`, and a contract that accepts only enumerated modes, profile names, and `#RRGGBB` zone colours.
- Recommendations: Keep every new D-Bus method on the enumerated-value model; never let a caller pass a path. Add a unit test asserting `HardwareBackend` joins only fixed relative names, so a future "write this file" method fails review loudly.

**Verified-write retries are user-invisible by design:**
- Risk: A failed verification surfaces only as a generic control error string; there is no `tracing` line per attempt, and the daemon's log is the only durable record of a refused write.
- Files: `src/service/backend.rs:12`–`50`, `src/service/types.rs:100`–`120`
- Current mitigation: `HardwareControlError::VerificationFailed` carries target, actual value, and retry count, and reaches the UI's `last_control_error`.
- Recommendations: Emit a `tracing::warn!` per failed attempt with the sysfs path, so a field report can be diagnosed from `journalctl -u zenomen-hw`.

## Performance Bottlenecks

**Blocking sysfs I/O and `thread::sleep` on the zbus executor:**
- Problem: Every `#[interface]` method is `async`, but each mutating one calls sync `HardwareBackend` code directly (`src/service/contract.rs:116`–`128`, `145`–`158`, `160`–`173`, `175`–`192`), and `write_and_verify` sleeps 50/100/200 ms between read-back attempts (`src/service/backend.rs:26`). The RGB path repeats the cycle per zone.
- Files: `src/service/backend.rs:12`–`50`, `src/service/contract.rs:116`–`200`, `src/service/backend.rs:412`–`440`
- Cause: No `await` inside those futures, so the whole future occupies the executor thread. A FourZone RGB apply can hold it for roughly 280 ms — plus the duration of a polkit prompt, since `authorize()` runs on the same executor.
- Improvement path: Make the mutating methods call `zbus::Connection::executor()`-independent blocking work — either `tokio::task::spawn_blocking` against a moved `HardwareBackend` clone, or hand the closure to a dedicated control thread and `await` its oneshot. Reads (`GetCapabilities`, `GetFanStatus`) should never queue behind a write.

**Status polling competes with telemetry for the same connection:**
- Problem: The worker probes `GetCapabilities` every 5 s (`src/core/worker.rs:299`–`302`) on the cached blocking connection. While a write is verifying on the executor, that probe either waits or fails and clears the connection cache.
- Files: `src/core/worker.rs:299`–`318`, `src/service/client.rs:59`–`99`
- Cause: One connection and one executor serve reads and writes; there is no priority separation.
- Improvement path: Cache a service-status verdict with a TTL longer than the longest expected write, and require two consecutive failures before reporting `Disconnected`.

**Unbounded thread creation on the UI path:**
- Problem: Diagnostics export spawns a thread per click (`src/app.rs:227`–`260`), the releases page spawns a reaper thread per click (`src/desktop/releases.rs:29`–`38`), and each hardware action spawns two (`src/core/worker.rs:360`, `src/core/worker.rs:365`).
- Files: `src/app.rs:227`, `src/desktop/releases.rs:34`, `src/core/worker.rs:360`
- Cause: Thread-per-event instead of a bounded worker pool.
- Improvement path: One small command executor for desktop chores; keep the monitoring worker as the only sampling thread (the current design's strength).

**The UI's own `VmRSS` under software rendering is far above target:**
- Problem: 132.71 MB `VmRSS` in the warm-dashboard stage on Xvfb with Mesa `llvmpipe` (`docs/RESOURCE-BUDGET.md`), against a ≤ 40 MB constraint.
- Files: `docs/RESOURCE-BUDGET.md:19`–`31`, `Cargo.toml` (`opt-level = "s"`, `lto`, `strip`)
- Cause: Mesa's software rasteriser maps LLVM JIT segments into the process; file-backed and shared, but counted by `VmRSS`.
- Improvement path: Measure on a real GPU driver before treating this as an application regression; if the number must be defended on headless CI, publish `Private_Dirty` and `VmRSS` side by side in every run.

## Fragile Areas

**Monitoring worker state machine (`src/core/worker.rs`, 380 lines, no unit tests):**
- Files: `src/core/worker.rs`
- Why fragile: One loop owns sampling cadence, command dispatch, config persistence, visibility gating, service probing, and thread spawning. `ui_visible == false` switches the wait from `recv_timeout` (`src/core/worker.rs:102`) to a blocking `recv` (`src/core/worker.rs:111`), which means any new command must be safe to receive while no deadline is armed. Adding a command to `AppCommand` without a match arm silently does nothing.
- Safe modification: Add the arm, set `deadline` explicitly when the command changes the interval, and decide whether it must be answered while the window is hidden.
- Test coverage: None at the unit level; `tests/benchmark_tests.rs` exercises it indirectly.

**Shutdown ordering in `src/main.rs`:**
- Files: `src/main.rs:216`–`225`
- Why fragile: Teardown is a hand-ordered sequence (`src/main.rs:216`–`225`) — `Shutdown` command, drop senders, drop `ui`, drop `app`, join worker, join event pump, drop session connection, join desktop thread. A previous review (CR-01) found this deadlocked the process; the current order is the fix. Reordering or adding a joined thread can reintroduce a hang with no test to catch it.
- Safe modification: Preserve "signal first, drop senders, then join"; never join a thread before the channel feeding it is dropped.
- Test coverage: None — `tests/desktop_integration_tests.rs` does not drive process exit.

**D-Bus contract methods are synchronous by accident:**
- Files: `src/service/contract.rs:83`–`200`
- Why fragile: Signatures are `async` but behaviour is blocking. Any new method that copies the existing body inherits the executor stall described above; a method that instead introduces real `.await` could interleave with `authorize()` in ways no test covers (`require_polkit` is disabled in most tests).
- Safe modification: Follow one pattern for all mutating methods — `authorize().await` then offload the blocking call — and add a test that a slow backend does not delay `GetCapabilities`.
- Test coverage: `tests/hardware_service_tests.rs` covers authorization outcomes, not concurrency.

**RGB interface detection and multi-zone writes:**
- Files: `src/hardware/rgb.rs:34`–`60`, `src/service/backend.rs:412`–`440`
- Why fragile: Three incompatible interfaces (`four_zone`, `legacy`, `kernel_patch`) are detected by probing sysfs, and a four-zone apply writes zone by zone. An interruption mid-sequence leaves the keyboard in a mixed state, and detection on a partially-populated sysfs tree can pick the wrong variant.
- Safe modification: Treat `detect_rgb_interface` as the single source of truth (the daemon re-detects rather than trusting the client's variant), and keep per-zone writes in a fixed order.
- Test coverage: `tests/rgb_tests.rs` (8 tests) covers parsing and migration, not interrupted writes on real sysfs.

**Hardware provider selection via environment variable: [RESOLVED]**
- Resolution: `src/main.rs` strictly validates `ZENOMEN_MOCK_SCENARIO` using `MockScenario::from_name`. Whitespace or empty values are treated as unset, and invalid values emit a `tracing::warn!` and safely fall back to the real `LinuxHardwareProvider` rather than silently enabling mock data.

## Scaling Limits

**Telemetry history is fixed at 120 samples:**
- Current capacity: `MAX_SAMPLES = 120` (`src/core/telemetry.rs:27`), a `VecDeque` that drops the oldest point.
- Limit: The UI's window control offers 30/60/120/300 s (`src/config/mod.rs`, `tests/config_tests.rs`); anything beyond 120 samples cannot be retained, and the chart's `viewbox` scaling hides the truncation rather than reporting it.
- Scaling path: Size the deque from `config.telemetry.history_window × 1000 / polling_interval_ms` at worker start, and keep the 120 default for memory predictability. A ring buffer of `TelemetryPoint` (timestamp + a few `f32`s) is cheap at 300 samples.

**One D-Bus connection per process, all calls serialised:**
- Current capacity: A single cached blocking `Connection` (`src/service/client.rs:41`–`60`) and a single-executor daemon.
- Limit: Each in-flight verified write delays every read on that connection for its full retry window (up to ~280 ms for RGB, plus polkit prompt time).
- Scaling path: Separate read and control paths — either two connections or the control-thread handoff above. This becomes user-visible the moment the UI adds a second pending request.

**Fan control supports only `auto` and `max`:**
- Current capacity: `FanMode::parse_requested` rejects anything but the two documented modes (`src/service/backend.rs`, `src/service/types.rs`); `pwm1_enable = 1` (manual PWM) is displayed if the kernel reports it but cannot be requested.
- Limit: No fan curve, no per-fan control, no RPM target — the v1 boundary set in phase 3.
- Scaling path: Manual mode needs a validated 0–255 PWM range, a curve model in the daemon, and a UI whose writes are debounced; it is deferred to v2 by design.

## Dependencies at Risk

**`zbus` 5 with the `blocking` feature used from async contexts:**
- Risk: The client uses `zbus::blocking::Connection` (`src/service/client.rs:6`) while the daemon implements `async` interfaces. Both are supported, but mixing them is where the executor-stall class of bug lives, and `zbus` 5's executor semantics are not something the project can rely on across a major bump.
- Impact: A `zbus` 6 migration touches `src/service/{client,contract,polkit,single_instance,tray}.rs` at once.
- Migration plan: Pin 5.x in `Cargo.lock` (already audited in CI), and when upgrading, convert the client to the async API inside the control thread rather than porting the blocking calls one by one.

**`slint` 1.17 with `default-features = false`:**
- Risk: The RSS budget depends on excluding Qt and Skia (`Cargo.toml:11`–`19`). A feature-graph change in a minor Slint release — or a component reaching for `std-widgets` — can pull a backend back in.
- Impact: The memory constraint fails silently at build time, visible only by re-measuring RSS.
- Migration plan: Keep the constrained feature list and re-run `scripts/measure-budget.sh` in the release checklist; treat `Cargo.lock` diff on `slint*` as a review trigger.

**`panic = "abort"` in the release profile:**
- Risk: `expect`/`unwrap` in thread-spawn paths terminate the whole process (`src/main.rs:151`, `src/main.rs:200`, `src/core/worker.rs:353`), since unwinding is disabled.
- Impact: A resource-exhaustion condition (thread limit, fd limit) kills the GUI instead of degrading it.
- Migration plan: Convert spawn failures to a logged `Result` handled by the caller — the UI can run without an event pump only briefly, but a message beats an abort.

## Missing Critical Features

**Manual fan curves and per-fan control:**
- Problem: Only `auto` and `max` can be requested; `FanMode` exposes no manual target and `pwm1_enable = 1` is read-only in the UI.
- Blocks: Quiet-under-load tuning, which is the primary reason users install a tool like this; also blocks the "premium system utility" positioning.

**RGB animation fields are parsed but never applied:**
- Problem: Profiles carry unsupported animation/timing data that is tracked and surfaced as "deferred to v2" (`src/rgb/schema.rs:6`, `src/rgb/schema.rs:114`–`124`), and only `zones` colours ever reach hardware.
- Blocks: Any animated lighting effect; users migrating animation-bearing profiles from the Python CLI get static colour.

**Two-hour tray soak is unmeasured:**
- Problem: `docs/RESOURCE-BUDGET.md` §3 records the soak as **NOT MEASURED**, with flatness criteria (`peak RSS ≤ 40 MB`, `growth ≤ 1.0 MB` after a 5-minute warmup) left to a live session.
- Blocks: A defensible "no leak over time" claim for a always-running tray application.

**Windows is unbuilt:**
- Problem: No `#[cfg(windows)]` code exists in `src/`; the only Windows-specific artefact is the `winres` build dependency in `build.rs` and `Cargo.toml`. Portability rests entirely on the `HardwareProvider` trait (`src/hardware/provider.rs`) plus `cfg(windows)`-free Linux modules.
- Blocks: The "architecture-ready" claim in PROJECT.md — the abstraction is real, but nothing has been compiled for the target.

## Test Coverage Gaps

**Monitoring worker command handling:**
- What's not tested: `src/core/worker.rs` has no `#[test]` module. Command dispatch, visibility gating, deadline reset after an immediate refresh, and config persistence on each command arm are exercised only indirectly through benchmark and integration tests.
- Files: `src/core/worker.rs`, `tests/benchmark_tests.rs`
- Risk: A new `AppCommand` arm that forgets `deadline = Instant::now() + sampling_interval` produces a burst of samples that no test notices.
- Priority: High

**Async daemon legality under load:**
- What's not tested: No test asserts that a slow backend does not delay `GetCapabilities`/`GetFanStatus` on the zbus executor, nor that a timed-out write leaves the service responsive.
- Files: `src/service/contract.rs`, `src/service/backend.rs`, `tests/hardware_service_tests.rs`
- Risk: The blocking-in-async defect is invisible to the suite and will be reintroduced by any copy-pasted method.
- Priority: High

**Shutdown and process exit:**
- What's not tested: The teardown sequence in `src/main.rs:216`–`225` has no automated coverage; `tests/desktop_integration_tests.rs` does not run the event loop to completion.
- Files: `src/main.rs`, `tests/desktop_integration_tests.rs`
- Risk: CR-01 was a deadlock on exit; the fix is one reordering away from regressing with no failing test.
- Priority: High

**Tray menu over D-Bus:**
- What's not tested: `src/desktop/tray.rs` (StatusNotifierItem properties and `com.canonical.dbusmenu`) — no test drives the menu layout, activation IDs, or the `Activate` path.
- Files: `src/desktop/tray.rs`, `src/desktop/single_instance.rs`
- Risk: A broken menu ID silently does nothing on KDE/GNOME, the two environments users actually run.
- Priority: Medium

**Single-instance race:**
- What's not tested: The window between `RequestName` succeeding (`src/desktop/single_instance.rs:76`–`90`) and `object_server().at()` registering `ApplicationService` (`src/desktop/single_instance.rs:91`).
- Files: `src/desktop/single_instance.rs`
- Risk: Double-launch leaves no window; the failure is timing-dependent and reproducible only by hand.
- Priority: Medium

**Polkit refusal and challenge paths: [RESOLVED]**
- Resolution: Fixed D-Bus response deserialization in `src/service/polkit.rs` and `src/service/client.rs`. PolicyKit1 `CheckAuthorization` returns `(bba{ss})` (dict of string to string), whereas code previously expected `(bba{sv})` (`HashMap<String, OwnedValue>`), causing every hardware authorization check on live systems to fail with signature mismatch. Added automated unit test `test_polkit_response_signature_matches_spec` verifying `(bba{ss})` wire deserialization.

**Documented invariants:**
- What's not tested: Nothing verifies that file paths and test counts quoted in `docs/PARITY.md` and `docs/RESOURCE-BUDGET.md` match the tree.
- Files: `docs/PARITY.md`, `docs/RESOURCE-BUDGET.md`, `tests/packaging_tests.rs`
- Risk: The REL-08 evidence gate is already wrong (see Tech Debt) and can drift further with every refactor.
- Priority: Medium

**Benchmarks assert host-dependent numbers:**
- What's not tested: `tests/benchmark_tests.rs` (5 tests) compares memory and wall-clock timings against fixed thresholds on unknown hardware, so the suite's verdict depends on the runner.
- Files: `tests/benchmark_tests.rs`, `scripts/measure-budget.sh`
- Risk: A pass on a fast machine and a fail on a loaded one teach the team to ignore the suite — the opposite of a regression gate.
- Priority: Medium

---

*Concerns audit: 2026-11-02*
