---
phase: 05-distribution-resource-budget-rebrand
reviewed: 2026-11-02T16:05:00Z
depth: standard
files_reviewed: 42
files_reviewed_list:
  - .github/workflows/release.yml
  - .gitignore
  - Cargo.toml
  - NOTICE
  - README.md
  - data/applications/zenomen.desktop
  - data/systemd/zenomen-hw.service
  - docs/PACKAGING.md
  - docs/PARITY.md
  - docs/RESOURCE-BUDGET.md
  - scripts/measure-budget.sh
  - src/app.rs
  - src/bin/zenomen-hw.rs
  - src/config/mod.rs
  - src/core/benchmark.rs
  - src/core/events.rs
  - src/core/state.rs
  - src/core/worker.rs
  - src/desktop/mod.rs
  - src/desktop/releases.rs
  - src/desktop/system_integration.rs
  - src/hardware/rgb.rs
  - src/main.rs
  - src/rgb/migration.rs
  - src/rgb/mod.rs
  - src/service/backend.rs
  - src/service/client.rs
  - src/service/contract.rs
  - src/service/mod.rs
  - src/service/polkit.rs
  - src/service/types.rs
  - tests/benchmark_tests.rs
  - tests/config_tests.rs
  - tests/degradation_tests.rs
  - tests/desktop_integration_tests.rs
  - tests/fan_control_tests.rs
  - tests/hardware_service_tests.rs
  - tests/packaging_tests.rs
  - tests/power_preset_tests.rs
  - tests/rgb_tests.rs
  - ui/app.slint
  - ui/screens/settings.slint
findings:
  critical: 2
  warning: 16
  info: 11
  total: 29
status: resolved
remediation:
  commit: 46b871e
  status: all_critical_and_warning_fixed
---

# Phase 05: Code Review Report — Distribution, Resource Budget & Rebrand

**Reviewed:** 2026-11-02T16:05:00Z
**Depth:** standard
**Files Reviewed:** 42
**Status:** issues_found

## Summary

This review covers the 42 non-planning files changed by Phase 05 (`1cc4ee3`, `544c09f`, `b433344`, `aa893f7`) against diff base `77a5d22889974d3787a3f6b021cb6cd9ac54d655`: packaging metadata and release automation, the root-run system-integration installer, the systemd/D-Bus/polkit assets, the desktop release action, the monitoring-worker visibility gating, the resource-budget harness and docs, the rebrand, and the accompanying tests. The privilege boundary itself is well drawn: the GUI performs no hardware writes, the service exposes a fixed operation set, power-profile names are validated against `platform_profile_choices`, RGB input is limited to four `#RRGGBB` values, and polkit gating is applied to every mutating method. No path-traversal, command-injection, hardcoded-secret, or authentication-bypass issue was found in that boundary.

The phase is not ready to ship as-is. There are two release-blocking defects: application teardown is a guaranteed deadlock that leaves the process (and its single-instance D-Bus name) alive after the window closes, and the documented AppImage `--install-system-integration` flow writes a daemon unit that points at a binary the AppImage never installs, so hardware control remains unavailable after a "successful" install. Beyond those, the systemd sandbox can prevent the daemon from starting or writing sysfs, the cargo-packager configuration appears to resolve binaries in the wrong directory, the Debian package never enables or D-Bus-activates the daemon despite the docs claiming it is running, the release workflow trusts mutable action tags and does not verify tag/version agreement, and the resource-budget and parity documents report conclusions their own numbers and file references contradict.

**Scope provenance:** the evaluation-scope resolver reported `status: degraded` with reason `no-task-commit-rows`. The reviewed list is therefore the explicit 42-file set above (all changed, non-deleted, non-planning files). **Tooling limitation:** `cargo` is not installed in the review environment (`cargo: command not found`), so no build, `cargo fmt`, clippy, or test run was possible. All findings are from static reading; packaging-config findings that depend on cargo-packager runtime behavior are marked for verification.

## Narrative Findings (AI reviewer)

All findings below were produced by reading the current source at `depth: standard` and tracing the relevant call chains across `main.rs`, `worker.rs`, `app.rs`, `single_instance.rs`, `tray.rs`, `system_integration.rs`, and the service modules. No `<structural_findings>` block and no `<external_reviewer_evidence>` block were supplied, so there are no structural or external claims to reconcile. Every item cites a concrete file and line; no source file was modified. Severity uses the required tiers: **Critical (BLOCKER)** for incorrect behavior/security/data-loss risk that must be fixed before shipping, **Warning** for defects that degrade correctness/robustness, and **Info** for non-blocking quality and documentation issues.

## Critical Issues

### CR-01: Application teardown deadlocks; the process never exits

**File:** `src/main.rs:213-219` (see also `src/main.rs:75-88`, `src/main.rs:100`, `src/main.rs:188-197`; `src/core/worker.rs:99-118`; `src/desktop/single_instance.rs:22-30`; `src/desktop/tray.rs:190-199`; `src/app.rs:37-43`)
**Severity:** Critical (BLOCKER)
**Issue:** The three `join()` calls after the event loop can never complete, for three mutually reinforcing reasons:

1. `let command_sender = app.command_sender();` (`main.rs:100`) keeps an `AppCommand` sender alive for the rest of `main`, so the worker's `recv_timeout`/`recv` never observes `RecvTimeoutError::Disconnected` (`worker.rs:102-117`).
2. The event pump owns `app_bridge = app.clone()` (`main.rs:189-196`), and `ZenOmenApp` itself contains a `commands: SyncSender<AppCommand>` (`app.rs:37-43`). That clone is another live command sender. The pump only exits when the worker drops `event_sender`, and the worker only exits when the pump drops its `app` clone — a cycle independent of the local `command_sender`.
3. `_session_conn` (`main.rs:75-88`) owns the session-bus object server, which owns `ApplicationService` and both tray objects, each holding `SyncSender<DesktopAction>` (`single_instance.rs:22-24`, `tray.rs:18-25,109-117`). `_session_conn` is dropped only at the end of `main`, after `desktop_action_thread.join()`, so the `for action in action_receiver` loop (`main.rs:126`) never ends.

Consequences: closing the window (with `close_to_tray` disabled) hides it and then hangs the process forever; the process keeps `com.zenomen.App` and the tray objects, so the next launch takes the `AlreadyRunning` path and exits, and `PresentWindow` is delivered to an event loop that no longer exists. The user must `SIGKILL` the app. The same hang occurs on the tray "Quit" path because reason 2 still applies.
**Fix:** Introduce an explicit shutdown command so the worker exits independent of sender lifetime, then drop the remaining senders before joining, in dependency order:

```rust
// src/core/events.rs
pub enum AppCommand {
    // ...
    /// Terminate the monitoring worker without depending on channel disconnect.
    Shutdown,
}

// src/core/worker.rs (inside the command match)
AppCommand::Shutdown => {
    tracing::debug!("shutdown requested");
    break;
}

// src/main.rs (after run_event_loop_until_quit)
let _ = command_sender.try_send(zenomen::core::AppCommand::Shutdown);
drop(ui);                       // drops the close-to-tray sender held by the callback
drop(app);
let _ = worker.join();           // breaks on Shutdown and drops event_sender
let _ = event_pump.join();       // now sees the channel close
drop(_session_conn);             // drops ApplicationService/tray senders
let _ = desktop_action_thread.join();
drop(command_sender);            // or drop before the joins; the Shutdown is the guarantee
```

Also add a regression test that starts the real worker/event-pump wiring, sends `Shutdown`, and asserts both joins return within a bounded time. Do not rely on channel-disconnect as the only shutdown signal while the pump holds a `ZenOmenApp` clone.

### CR-02: AppImage `--install-system-integration` installs a daemon unit for a binary that does not exist

**File:** `src/desktop/system_integration.rs:64-93` (see also `data/systemd/zenomen-hw.service:10`, `data/applications/zenomen.desktop:6`, `README.md:55-59`, `docs/PACKAGING.md:85-97`)
**Severity:** Critical (BLOCKER)
**Issue:** `install()` writes exactly four text files via `install_to_prefix()` (`system_integration.rs:81-93`) and then runs `systemctl daemon-reload`, `enable zenomen-hw`, and `restart zenomen-hw` (`:72-74`). The embedded unit's `ExecStart=/usr/bin/zenomen-hw` (`data/systemd/zenomen-hw.service:10`). In the documented AppImage flow, no binary is ever installed to `/usr/bin`: the AppImage runs the GUI out of a temporary squashfs mount, and the integration command does not copy either binary to a system path. `systemctl restart` therefore fails after four files have already been written, `install()` returns `Err`, and the user is told the install failed — but with a partially installed unit. Even if systemd were coerced into starting it, the service cannot exec. The same applies to the installed desktop entry, whose `Exec=zenomen` (`data/applications/zenomen.desktop:6`) is not on `PATH` for an AppImage-only installation, so the menu entry is dead too.

**Fix:** In `install()`, resolve the daemon next to the running executable (`std::env::current_exe()?.parent().join("zenomen-hw")`), verify it exists, and install it to `/usr/bin/zenomen-hw` with mode `0o755` before writing/starting the unit. A copied raw AppImage binary may need the AppImage's bundled library path; if so, install a tiny wrapper that exports the AppImage `usr/lib` path, or bundle a static daemon. Do not run `systemctl restart` unless every prerequisite (daemon binary, unit, policy) is in place. Make `uninstall()` remove the installed daemon/launcher. Also either install a `zenomen` launcher for the GUI or point the desktop entry at the AppImage path. Add a packaging test that asserts the unit's `ExecStart` target exists after a mock AppImage install.

```rust
fn install_daemon_from_self() -> Result<(), SystemIntegrationError> {
    let src = std::env::current_exe()?.with_file_name("zenomen-hw");
    if !src.is_file() {
        return Err(SystemIntegrationError::Command(format!(
            "{} is missing; install the .deb or place zenomen-hw in /usr/bin",
            src.display()
        )));
    }
    // atomic copy to /usr/bin/zenomen-hw, chmod 0755, then continue
    Ok(())
}
```

## Warnings

### WR-01: A polkit prompt can block the only sampling worker indefinitely

**File:** `src/core/worker.rs:222-273` (see also `src/service/client.rs:59-68,123-141`; `src/service/polkit.rs:28-70`)
**Severity:** Warning
**Issue:** All mutating calls (`SetFanMode`, `SetPowerProfile`, `ApplyPreset`, `ApplyRgbProfile`, `SetRgbPower`) are synchronous `zbus::blocking` calls made on the single monitoring worker. `client.rs:123-133` has no timeout, and the service may ask polkit with interactive authorization (`polkit.rs:39-49`). If the desktop has no polkit agent, or the user ignores the dialog, the worker is blocked indefinitely: telemetry stops updating, the pending control never resolves, and the UI has no way to cancel. This is also the thread that services `SetUiVisible`, so tray restore cannot take effect until the prompt is answered.
**Fix:** Bound mutating calls with a timeout (e.g. run the call on a short-lived thread and `join` with a deadline, or use a zbus proxy with a timeout configured), surface a `Timeout` error in `last_control_error`, and make the pending state clear. If the timeout wins, cancel the authorization request via polkit's `CancelCheckAuthorization` using a non-empty cancellation id instead of the current `""` (`polkit.rs:40`). Keep the worker loop free of unbounded waits.

### WR-02: The status strip always says “Mock provider”, even with the Linux provider

**File:** `src/core/state.rs:249-264` (see also `ui/app.slint:64`, `src/main.rs:167-173`)
**Severity:** Warning
**Issue:** `status_text()` hardcodes `"Mock provider · ..."` in all three branches (`state.rs:251,256,262`), and the Slint default is also `"Mock provider · waiting for first reading"` (`ui/app.slint:64`). `main.rs` correctly sets `provider_mode` to `"linux"` for the real provider (`:170-172`), so the About panel and the status strip disagree, and every real-machine user is told the app is running against mock data. This directly undermines the “honest status” value the phase is rebranding around.
**Fix:** Carry the provider label in `AppState` (set from the same config/env decision as `provider_mode`) and format `"{provider} · ..."`; use a neutral default such as `"Waiting for first reading"` in the Slint file. Update the state tests at `state.rs:304-391` to construct the label explicitly.

### WR-03: The systemd sandbox may fail at start or block the sysfs writes it is meant to permit

**File:** `data/systemd/zenomen-hw.service:15-20` (see also `data/systemd/zenomen-hw.service:10`, `src/service/backend.rs:77-115,321-351`)
**Severity:** Warning
**Issue:** `ReadWritePaths=/sys/devices/platform/hp-wmi /sys/firmware/acpi` has no `-` prefix. Per `systemd.exec(5)`, a `ReadWritePaths=`/`ReadOnlyPaths=` path that does not exist makes the unit fail to start; `/sys/devices/platform/hp-wmi` is only present when the `hp-wmi` platform device exists, and the unit does not order itself after the module/device. The unit also sets `ProtectKernelTunables=true`, which makes `/sys` read-only; whether the `ReadWritePaths` carve-outs are applied after that mount (and therefore win) is mount-order-dependent and must be verified, because if they do not, every fan/power/RGB write returns `EROFS` while discovery still succeeds. Finally, the backend can discover control paths through `/sys/class/hwmon/*` and `/sys/class/leds/hp_omen::kbd_backlight` (`backend.rs:92-110,340-345`), which are not explicitly allow-listed (they usually resolve to the same device tree, but that is an assumption, not a guarantee).
**Fix:** Prefix each entry with `-` so a missing device is not fatal, add `ConditionPathExists=/sys/firmware/acpi/platform_profile` (or `After=` the module), and verify on real hardware under the installed unit that `SetFanMode`/`SetPowerProfile` and all three RGB variants can write. Run `systemd-analyze security --offline=yes zenomen-hw` in CI. If `ProtectKernelTunables=yes` shadows the writable mounts, remove it with a comment explaining that the service's entire purpose is the narrow `/sys` carve-outs, or move to `ReadOnlyPaths=/` plus explicit `ReadWritePaths=-...` and test both.

### WR-04: `NoNewPrivileges=false` and an unrestricted root capability set contradict the claimed sandbox

**File:** `data/systemd/zenomen-hw.service:24`
**Severity:** Warning
**Issue:** `NoNewPrivileges=false` is the insecure default, explicitly set here, and the unit sets no `CapabilityBoundingSet=` or `AmbientCapabilities=`. The daemon runs as root with the full capability set. A memory-safety or logic compromise in the D-Bus service therefore yields full root capabilities rather than the minimum needed to write a few sysfs files (which requires no capability beyond root DAC). `docs/PARITY.md:47` claims SVC-05 is “SUPERIOR” because the unit is sandboxed, but the capability and no-new-privs posture is weaker than that claim.
**Fix:** Set `NoNewPrivileges=true`, `CapabilityBoundingSet=` (empty), `AmbientCapabilities=`, and add `RestrictAddressFamilies=AF_UNIX`, `SystemCallFilter=@system-service`, `LockPersonality=true`, and `MemoryDenyWriteExecute=true`. Re-test fan/power/RGB writes; root DAC is sufficient for the sysfs writes and no capability should be needed.

### WR-05: Release workflow trusts mutable action tags and does not enforce the committed lockfile

**File:** `.github/workflows/release.yml:25,55,61,74` (also `:29-31,49,52,58`)
**Severity:** Warning
**Issue:** The publish job runs with `contents: write` (`:9-10`) and uses mutable refs: `actions/checkout@v4`, `taiki-e/install-action@cargo-audit`, `taiki-e/install-action@cargo-packager`, and `softprops/action-gh-release@v2`. A compromised or re-pointed tag executes arbitrary code with a token that can rewrite releases and repository contents. The build/test commands also omit `--locked`, so a tag build can silently resolve a different dependency graph from the committed `Cargo.lock` (the repo deliberately commits the lockfile — `.gitignore:5-8`).
**Fix:** Pin every action to a full commit SHA with the version in a comment, e.g. `uses: actions/checkout@<40-char-sha> # v4.2.2`; pin the install-action release rather than the `@cargo-audit` shorthand; use `permissions: contents: read` for the build/test job and move `contents: write` to a separate publish job; run `cargo test --locked --all` and change `before-packaging-command` to `cargo build --release --bins --locked`. Add Dependabot/Renovate or a scheduled pin update.

### WR-06: A release tag is never checked against `Cargo.toml`’s version

**File:** `.github/workflows/release.yml:3-6,63-82`
**Severity:** Warning
**Issue:** The workflow triggers on any `v*` tag and publishes whatever `cargo packager` produces, whose version comes from `Cargo.toml:3`. Tagging `v0.2.0` while `Cargo.toml` still says `0.1.0` publishes `zenomen_0.1.0_amd64.deb` under a `v0.2.0` release, and `SHA256SUMS`/README instructions then refer to the wrong artifact. There is no guard and no release notes/version consistency check.
**Fix:** Before packaging, add:

```yaml
- name: Verify tag matches Cargo.toml
  if: startsWith(github.ref, 'refs/tags/v')
  run: |
    tag="${GITHUB_REF_NAME#v}"
    ver=$(cargo metadata --no-deps --format-version 1 \
      | jq -r '.packages[] | select(.name == "zenomen") | .version')
    if [ "$tag" != "$ver" ]; then
      echo "Tag v$tag does not match Cargo.toml version $ver" >&2
      exit 1
    fi
```

Also assert the generated `.deb`/`.AppImage` filenames contain `${ver}` before publishing.

### WR-07: The Debian package never enables or starts the daemon, and there is no D-Bus activation file

**File:** `Cargo.toml:66-72` (see also `docs/PACKAGING.md:55-62`, `README.md:37-44`, `data/dbus/com.zenomen.Hardware1.conf`)
**Severity:** Warning
**Issue:** `[package.metadata.packager.deb.files]` ships the systemd unit, D-Bus policy, and polkit policy, but cargo-packager’s Debian config supports no maintainer scripts, and the repository ships no `/usr/share/dbus-1/system-services/com.zenomen.Hardware1.service` activation file. After `sudo apt install ./zenomen_*.deb`, the unit is installed but not enabled, so `systemctl is-active zenomen-hw` reports `inactive` and every mutating call fails with `ServiceUnavailable` until the user manually runs `systemctl enable --now zenomen-hw`. `docs/PACKAGING.md:62` claims “The daemon will be active and running under systemd”, which is false on a fresh install.
**Fix:** Ship a D-Bus system service activation file and map it in `deb.files`; the bus will then start the daemon on the first method call without a postinst:

```ini
[D-BUS Service]
Name=com.zenomen.Hardware1
Exec=/usr/bin/zenomen-hw
User=root
```

Alternatively add a maintainer script via the packager’s supported mechanism (if available in the pinned version) that runs `systemctl enable --now zenomen-hw` and `daemon-reload`. Either way, update the docs and add a test that parses the deb file map and asserts the daemon is reachable through one of the two paths.

### WR-08: cargo-packager resolves binaries under `dist/`, and `appimage.bins` is used for the wrong purpose

**File:** `Cargo.toml:56-61,74-75`
**Severity:** Warning
**Issue:** Per cargo-packager’s configuration reference, `binaries[].path` is resolved from `out-dir` when relative, and `out-dir = "dist"` (`:56`). `{ path = "zenomen" }` therefore resolves to `dist/zenomen`, while `before-packaging-command` (`:61`) builds into `target/release/`. The packaging step should fail to find the binaries (or package stale files if a previous run left them in `dist/`). Separately, `[package.metadata.packager.appimage] bins` is documented for **host** tools to wrap into the AppImage (for example `/usr/bin/xdg-open`), not for the application’s own binaries, which come from `binaries`; listing `dist/zenomen` and `dist/zenomen-hw` there is either an error or a duplicate bundle.
**Fix:** Point the binary entries at the Cargo output (or set `binaries-dir`), and remove the `appimage.bins` block:

```toml
# either:
binaries = [
  { path = "../target/release/zenomen", main = true },
  { path = "../target/release/zenomen-hw", main = false },
]
# or keep paths bare and add:
binaries-dir = "target/release"
```

Verify with a real `cargo packager --release --formats deb,appimage` run in CI (this environment has no cargo). Also add a packaging test that asserts the expected paths exist before packaging.

### WR-09: The documented two-hour soak command cannot work because of `shift 2`

**File:** `scripts/measure-budget.sh:10-23` (documented command at `docs/RESOURCE-BUDGET.md:52-54`)
**Severity:** Warning
**Issue:** In soak mode, line 13 consumes `--soak` and the duration. The remaining positionals are `<stage> <command...>`, but line 16 reads `STAGE="$1"` and `DURATION="$2"` (taking the command as the duration, later overwritten) and line 18 then `shift 2`, deleting the stage **and** the command. For the documented invocation

```bash
./scripts/measure-budget.sh --soak 7200 tray-minimized ./target/release/zenomen --minimized
```

the remaining `$@` becomes just `--minimized`; line 32 tries to execute `--minimized`, the background process dies immediately, and the warm-up check exits 1. The one command the documentation gives for the unmeasured soak cannot run. (The non-soak path also dies from `set -e` if fewer than two positionals are supplied.)
**Fix:** Parse the two modes explicitly and require a command:

```bash
if [ "${1:-}" = "--soak" ]; then
    SOAK_MODE=true
    SOAK_DURATION="${2:-7200}"
    shift 2
    STAGE="${1:?missing stage}"
    shift
    DURATION="$SOAK_DURATION"
else
    STAGE="${1:-dashboard-open}"
    DURATION="${2:-60}"
    [ "$#" -ge 3 ] || { echo "Usage: $0 [--soak <seconds>] <stage> <duration-seconds> <command...>" >&2; exit 1; }
    shift 2
fi
[ "$#" -ge 1 ] || { echo "missing command to measure" >&2; exit 1; }
```

Validate `DURATION` numerically and use `getopts` or a `while` loop for further flags.

### WR-10: Migration reports profiles as “imported” when the target already existed and nothing was written

**File:** `src/rgb/migration.rs:224-249` (consumed at `src/main.rs:59-64`)
**Severity:** Warning
**Issue:** In the `Ok(profile)` branch, the write is skipped when `target_path.exists()`, but `report.imported.push(profile.id)` runs unconditionally at line 244. The startup log then reports `imported = report.imported.len()` even though those files were preserved, not created. This makes a re-run look like it re-imported user profiles and hides the fact that the existing (possibly modified) file was kept, which is exactly the distinction the non-destructive migration promise depends on.
**Fix:** Track the outcome explicitly, e.g. add `preserved: Vec<(String, String)>` to `MigrationReport`, push `(file_stem, "target exists; kept existing profile")` when the write is skipped, and only push to `imported` after a successful `fs::write`. Update the log line and the `tests/rgb_tests.rs` assertions to cover an existing-target rerun.

### WR-11: A unit test performs a live D-Bus call and asserts the production daemon is offline

**File:** `tests/fan_control_tests.rs:66-77`
**Severity:** Warning
**Issue:** `test_client_handles_offline_daemon_gracefully` assumes no `com.zenomen.Hardware1` service is running. On a developer machine with the daemon installed and active, `status()` returns `Connected`, the first assertion fails, and the test is host-state dependent. Worse, if the test passed the status check only because of a transient failure, the following `client.set_fan_mode("max")` is a real hardware write through polkit; a test must not mutate the host’s fan mode.
**Fix:** Never call a mutating method against a live bus in a unit test. Guard the offline assertion (`if status.is_connected() { eprintln!("skipping: hardware service is active"); return; }`) and replace the mutation with a mock client behind a trait, or move this to an explicitly-marked hardware integration test that is skipped by default.

### WR-12: The releases test mutates the process-global `PATH` with `unsafe` while other tests run concurrently

**File:** `tests/desktop_integration_tests.rs:170-196`
**Severity:** Warning
**Issue:** The test writes `std::env::set_var("PATH", …)` in a multi-threaded test binary. In edition 2024 this is `unsafe` precisely because another thread may read the environment (for example another test spawning a process), so the test contains a data race/UB hazard. The restoration is manual, so a panic between the set and restore leaves the whole test binary with an empty `PATH`. It also only tests “xdg-open missing” by relying on the process environment rather than on the production code’s boundary.
**Fix:** Do not mutate the environment. Make the command name/path injectable (`open_releases_page_with("xdg-open")` or a small `CommandRunner` trait) and test the missing-command error against an explicitly empty `Command` lookup, or run this case in a child process with a controlled environment (`Command::new(std::env::current_exe())` + an env marker). If environment mutation is truly unavoidable, serialize it behind a `static Mutex` and use a drop guard to restore, but prefer injection.

### WR-13: Benchmark tests assert host-dependent memory and wall-clock timings

**File:** `tests/benchmark_tests.rs:17-38,73-84,151-180`
**Severity:** Warning
**Issue:** `memory_instrumentation_reads_valid_process_rss` asserts the whole test harness process stays under 30 MB RSS (`:33-37`), which depends on the runner, allocator, and test concurrency; a 40 MB budget for the release GUI is not the same as a 30 MB assertion on the test binary. At `:73-84` the “refresh” timing measures `try_send().elapsed()`, not the worker’s response, so it proves nothing about responsiveness and can pass while the worker is stalled. At `:174-180` the visibility test waits 1500 ms for no events on a 1000 ms cadence; scheduler delays, an in-flight service-status event, or a loaded CI host can make it flaky.
**Fix:** Remove the process-RSS assertion or gate it behind an opt-in environment variable and a dedicated process; measure event latency by recording `Instant` when `TelemetryUpdated` is received (not the send duration); and for the visibility test drain to quiescence, wait at least 2× the interval, and assert on the absence of `TelemetryUpdated` specifically rather than any event. Use generous timeouts (e.g. 2 s) consistently.

### WR-14: PARITY cites files that do not exist and reports a stale test count

**File:** `docs/PARITY.md:13,25,31,33,37,42,83`
**Severity:** Warning
**Issue:** The parity matrix points at `src/service/service.rs` (lines 13, 31, 33, 37, 42) and `src/core/history.rs` (line 25); neither file exists — the actual modules are `src/service/contract.rs` and `src/core/telemetry.rs`. This makes the “Verifying Test / Evidence” column unverifiable for the rows it covers. Line 83 claims “48 requirements verified by 72 automated integration tests in `tests/`”; the repository currently has **77** `#[test]` functions across `tests/` (105 including `src/` unit tests). A parity certification whose file paths and counts are wrong is not evidence.
**Fix:** Replace both paths, re-run and recount the tests (or generate the count in CI and link the run), and re-check each “Automated” row against a test that actually exists and exercises the claimed behavior. Consider making the matrix a generated artifact so it cannot drift.

### WR-15: RESOURCE-BUDGET marks stages PASS while the same table records VmRSS 3.5× over target

**File:** `docs/RESOURCE-BUDGET.md:17-27` (also `:50-55`, `scripts/measure-budget.sh`)
**Severity:** Warning
**Issue:** The table marks Warm Dashboard **PASS** with `Target RSS < 38 MB` but `Measured VmRSS (Xvfb) = 132.71 MB`; Peak Load is **PASS** with `Target ≤ 40 MB` and `141.20 MB`. The footnote explains Mesa/llvmpipe file-backed inflation, but the verdict column still says PASS next to a number that is 3.5× the stated budget, and the “Measured Internal RSS” column (16.56 MB) cannot simultaneously describe the same process at the same moment as 132.71 MB VmRSS. There is no machine-checkable gate in `scripts/measure-budget.sh` either — it prints a CSV and exits 0 regardless of the numbers. The result is a document that asserts a budget it does not demonstrate.
**Fix:** Report one measurement method per row, state which metric the target applies to, and mark the stage `PASS (Private_Dirty)` or `FAIL (VmRSS)` rather than an unqualified PASS. If the ≤40 MB constraint is meant to be total RSS, treat 132.71 MB as a failure and document the llvmpipe caveat as an environment deviation. Add a threshold check to the harness (`awk` over the peak and growth values, exit non-zero on breach) and record the CSV in the phase evidence. Note that the soak command itself is broken (WR-09), so “NOT MEASURED” is the only honest status there.

### WR-16: Synchronous sysfs I/O and `thread::sleep` run on the zbus async executor

**File:** `src/service/contract.rs:116-212` (see also `src/service/backend.rs:12-48,162-176,412-474`)
**Severity:** Warning
**Issue:** Every `#[interface]` method is `async`, but each mutating method directly calls synchronous `HardwareBackend` code (`fs::read_to_string`, `fs::write`, directory scans) and `write_and_verify` sleeps with `std::thread::sleep` for 50/100/200 ms per retry (`backend.rs:21-37`). The RGB path repeats that per zone (up to ~280 ms for FourZone). Because these futures never await, they block the zbus executor thread, delaying `GetCapabilities`, `GetFanStatus`, and every other client call on the same connection for the duration of the write — including the UI’s status polling. This is especially bad while a polkit prompt is outstanding (WR-01) because the executor is also the path for the authorization call.
**Fix:** Move the blocking backend work off the async executor: wrap each backend call in `async_io::blocking::unblock`/`smol::unblock` (zbus already depends on async-io) or `tokio::task::spawn_blocking`, or dispatch operations to a dedicated hardware thread and await a oneshot reply. Keep `write_and_verify` synchronous for unit tests and call it from the blocking pool. Add a test that issues a slow write and a concurrent `GetCapabilities`, asserting the read is not delayed by the write’s full retry budget.

## Info

### IN-01: Atomic writes are not durable across power loss

**File:** `src/desktop/system_integration.rs:50-61` (also `src/config/mod.rs:80-99`)
**Severity:** Info
**Issue:** `atomic_write_file` writes the temp file and renames it without `sync_all()` on the file or the parent directory. `save_atomic` syncs the temp file but not the parent directory before/after `rename`. A crash or power loss can leave a zero-length or missing config/unit even though the rename is atomic with respect to concurrent readers.
**Fix:** Write, `file.sync_all()`, `rename`, then open and `sync_all()` the parent directory. Do the same for the installer’s copied daemon binary in CR-02.

### IN-02: Two different `ProjectDirs` constructors are used for user paths

**File:** `src/app.rs:233-236` vs `src/config/mod.rs:21`
**Severity:** Info
**Issue:** Diagnostics use `ProjectDirs::from("com", "zenomen", "zenomen")` while the config uses `ProjectDirs::from("", "", "zenomen")`. On Linux both currently resolve to `~/.config/zenomen`, but on Windows the organization segment changes the path, so the diagnostic export directory and the config directory diverge on the “architecture-ready” platform.
**Fix:** Add one `project_dirs()` helper in `src/config/mod.rs` (or a paths module) and use it for config, RGB, autostart, and diagnostics so the identifier tuple is defined once.

### IN-03: History-window validation differs between the UI callback and the sanitizer

**File:** `src/app.rs:120-125` vs `src/config/schema.rs:96-99` (also `src/core/worker.rs:151-166`)
**Severity:** Info
**Issue:** `on_history_window_changed` accepts any `seconds >= 2` and sends the raw value; `sanitize()` only allows `{30,60,120,300}`. The worker emits `HistoryWindowChanged(samples)` with the unsanitized value before it emits the sanitized `ConfigLoaded`, so an out-of-set value briefly resizes the chart to an unsupported window and then snaps back. The current Slint UI only offers the four valid values, so this is latent, but the invariant is enforced in the wrong layer.
**Fix:** Validate against the allowed set in the callback (or expose the set from the config module) and have the worker emit the sanitized value in `HistoryWindowChanged`; keep `sanitize()` as the persistence guard.

### IN-04: NOTICE is not shipped in either package

**File:** `Cargo.toml:66-72`
**Severity:** Info
**Issue:** The Debian file map ships `README.md` and `LICENSE` under `/usr/share/doc/zenomen` but not `NOTICE`, which contains the HP trademark disclaimer and the GPL derivation statement. The project constraint says the trademark disclaimer must be retained and the license shipped with distributed binaries; the AppImage likewise does not include `NOTICE`.
**Fix:** Add `"NOTICE" = "/usr/share/doc/zenomen/NOTICE"` to `deb.files`, include it in AppImage resources, and reference it from the README. Add a packaging test that asserts the file map contains the notice.

### IN-05: The root-run installer resolves `systemctl` through `PATH`

**File:** `src/desktop/system_integration.rs:130-133`
**Severity:** Info
**Issue:** `Command::new("systemctl")` is executed as root. `sudo` normally resets `PATH` via `secure_path`, but if the installer is invoked from a root shell, with `sudo -E`, or from an environment with a writable early `PATH` entry, a malicious `systemctl` would execute as root. The rest of the installer correctly uses fixed absolute targets, so this is the one path lookup in the privileged path.
**Fix:** Use an absolute path (`/usr/bin/systemctl`, falling back to `/bin/systemctl` if absent) and pass `--no-ask-password`; log the resolved path. This also makes the privileged behavior deterministic.

### IN-06: hp-wmi fan discovery is duplicated between the service and the hardware layer

**File:** `src/service/backend.rs:77-115` (compare `src/hardware/linux/hwmon.rs` / `src/hardware/registry.rs`)
**Severity:** Info
**Issue:** The service re-implements the direct `hp-wmi/hwmon` scan and the `/sys/class/hwmon` name-matching fallback separately from the unprivileged hardware layer. The two can drift (for example, one may accept a `name` of `"hp_wmi"` while the other accepts only `"hp"`/`"hp-wmi"`), which would make the UI’s capability report disagree with what the service can actually control.
**Fix:** Extract a shared discovery helper (path-only, no writes) into `src/hardware` and have both the service and the registry use it; add a fixture-based test covering both layouts.

### IN-07: FourZone RGB writes can leave the keyboard in a mixed state

**File:** `src/service/backend.rs:418-424`
**Severity:** Info
**Issue:** Each of the four zones is written and verified independently. If zone 2 or 3 fails after zone 0/1 succeeded, the method returns an error but zones 0/1 remain changed; the UI shows a failure while the hardware is neither the old nor the requested profile. There is no snapshot or rollback.
**Fix:** Read the current colors via `get_rgb_colors()` before writing and restore them if a later zone fails; or write all zones through a single interface where the kernel supports it. At minimum, include the succeeded zone indices in the error so the user can re-apply.

### IN-08: Config is loaded twice, and migration runs before the single-instance check

**File:** `src/main.rs:53-66` (also `src/core/worker.rs:84-87`)
**Severity:** Info
**Issue:** `main` loads `config` at `:50`, then the worker reloads it from disk at `:84-87` instead of receiving the already-loaded value, so the two can disagree if the file changes in between. The RGB migration (`:53-66`) also runs before `ensure_single_instance` (`:74`), so every secondary launch scans the legacy profile directory and can race the primary’s migration writes.
**Fix:** Pass the loaded `AppConfig` into `spawn_monitoring_worker` (or an `Arc<Mutex<AppConfig>>`) and move migration after the single-instance check so only the primary performs it.

### IN-09: `PresetApplied` infers the fan mode from the preset name instead of the confirmed hardware value

**File:** `src/core/state.rs:171-179`
**Severity:** Info
**Issue:** When a preset succeeds, the state sets `fan_mode` to `"max"` only for `"gaming"` and `"auto"` for everything else. This happens to match the current `apply_preset` mapping, but it is a duplicated policy: any future preset or fallback mapping in `backend.rs:256-276` will make the UI lie about the fan mode. The service already re-reads the confirmed mode in `set_fan_mode` (`backend.rs:174`).
**Fix:** Add the confirmed mode to `PresetResult` (or emit a separate `FanModeChanged` after applying the preset) and store that value; do not derive hardware state from the preset’s name.

### IN-10: Installation and artifact names in the docs do not match the packager output

**File:** `README.md:15,40,51` (also `docs/PACKAGING.md:74`)
**Severity:** Info
**Issue:** README says telemetry is rendered via “60-second” sparklines although the default history window is 120 samples (`src/core/state.rs:27`); the `.deb` is shown as `zenomen_0.1.0_amd64.deb` while PACKAGING documents `zenomen_<version>_amd64.deb` (the version placeholder is fine, the tag/version drift is covered by WR-06); and the AppImage is shown as `ZenOmen-0.1.0-x86_64.AppImage` while cargo-packager’s documented output pattern is `ZenOmen_<version>_amd64.AppImage` (`docs/PACKAGING.md:74`). Users following the README will `chmod` a file that does not exist.
**Fix:** Generate the artifact names from the actual `dist/` output and use them consistently in both documents; change “60-second” to “configurable (default 120 s)” or make the docs derive from `HISTORY_WINDOW`.

### IN-11: Every “Check for updates” click leaves an unreaped child process

**File:** `src/desktop/releases.rs:26-34`
**Severity:** Info
**Issue:** `Command::spawn()` returns a `Child` that is immediately dropped without `wait()`. On Unix the exited `xdg-open` process remains a zombie until the GUI exits, because dropping `std::process::Child` does not reap it. The leak is small per click but unbounded over a long session, and it is easy to avoid.
**Fix:** Reap on a detached thread (`std::thread::spawn(move || { let _ = child.wait(); })`) or call `status()` if a short block is acceptable. Do not leave the `Child` unreaped.

---

_Reviewed: 2026-11-02T16:05:00Z_
_Reviewer: the agent (gsd-code-reviewer)_
_Depth: standard_
