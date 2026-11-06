---
last_mapped_commit: 30d62de2cdf6f507822944e5602bdd907539ce38
last_mapped_at: 2026-11-06
---
# Testing Patterns

**Analysis Date:** 2026-11-06

## Scope

This document describes the tests that ship with the **Rust + Slint implementation**
(`src/`, `tests/`). The retired Python CLI had no tests at all; ignore any testing
guidance inherited from it.

## Test Framework

**Runner:**
- Rust built-in test harness (`cargo test`) — no external runner, no `#[tokio::test]`,
  no `rstest`, no `proptest`, no `serial_test`
- Config: `Cargo.toml` has **no `[dev-dependencies]` section at all**. Every test uses
  `std`, the crate's public API, and the standard `assert!` family.
- Integration suites are declared implicitly: any file at `tests/*.rs` is its own crate.

**Assertion Library:**
- Standard macros only: `assert!`, `assert_eq!`, `assert_ne!`
- `assert_eq!` is used with a third message argument whenever the value is not
  self-describing, e.g. `assert!(agg2 - 75.0).abs() < 0.1, "Expected 75%, got {agg2}")`
  (`tests/linux_procfs_tests.rs:29`)
- Error variants are matched, not string-compared:
  ```rust
  assert!(matches!(
      manual_err,
      HardwareControlError::UnsupportedMode(_)
  ));
  ```
  (`tests/hardware_service_tests.rs:76-79`)

**Run Commands:**

```bash
xvfb-run -a cargo test --all          # Canonical run (CI + README)
cargo test --all                      # Works only if a display server is available
cargo test                            # Library unit tests + inline modules
cargo test --lib                      # Inline #[cfg(test)] modules only
cargo test --test degradation_tests   # One integration suite
cargo test <substring>                # Filter by test name
```

`xvfb-run` is mandatory on headless machines because the `zenomen` library links Slint
and the crate's probe binaries must initialise the GUI stack, even though **no test
instantiates a window** (`.github/workflows/ci.yml:50-52`).

CI gate that matters here (`.github/workflows/ci.yml:47-48`):

```bash
xvfb-run --auto-servernum cargo test --all
```

All four gates must pass before a PR: `cargo fmt --all -- --check`,
`cargo clippy --all-targets --all-features -- -D warnings`, the test command above,
and `cargo audit`.

## Test File Organization

**Location:**
- **Integration tests:** `tests/<area>_tests.rs` — one file per subsystem area,
  always suffixed `_tests.rs`
- **Inline unit tests:** `#[cfg(test)] mod tests { use super::*; ... }` at the bottom of
  the module under test. Used for pure logic that benefits from private access.

**Current inventory — 110 `#[test]` functions across 15 integration suites + 9 inline
modules:**

| Suite | Tests | Focus |
|---|---|---|
| `tests/hardware_service_tests.rs` | 9 | `write_and_verify`, `FanMode` conversion, polkit contract |
| `tests/degradation_tests.rs` | 8 | PERF-05 degradation matrix (`degradation_01`…`degradation_08`) |
| `tests/desktop_integration_tests.rs` | 8 | autostart, single-instance D-Bus, tray, icons |
| `tests/rgb_tests.rs` | 8 | hex validation, presets, migration, three sysfs variants |
| `tests/config_tests.rs` | 6 | TOML roundtrip, atomic save, corruption recovery, clamping |
| `tests/sensor_registry_tests.rs` | 6 | sensor priority resolution, live-host smoke test |
| `tests/benchmark_tests.rs` | 5 | RSS instrumentation, worker latency, tick budget |
| `tests/packaging_tests.rs` | 5 | embedded assets vs `data/`, install/uninstall, root guard |
| `tests/power_preset_tests.rs` | 5 | ACPI profile writes and combined presets |
| `tests/telemetry_history_tests.rs` | 4 | bounded `VecDeque`, eviction, SVG path generation |
| `tests/linux_procfs_tests.rs` | 4 | `/proc/stat` deltas, `/proc/meminfo` parsing |
| `tests/fan_control_tests.rs` | 3 | `AppState` event transitions, offline client |
| `tests/linux_gpu_tests.rs` | 3 | `nvidia-smi` parsing, DRM vendor detection |
| `tests/mock_hardware_tests.rs` | 3 | the three `MockScenario` variants |
| `tests/diagnostics_tests.rs` | 2 | diagnostics collection and export |

| Inline module | Tests |
|---|---|
| `src/core/state.rs` | 10 |
| `src/core/telemetry.rs` | 6 |
| `src/hardware/linux/cpu.rs` | 5 |
| `src/hardware/linux/gpu.rs` | 3 |
| `src/hardware/linux/hwmon.rs` | 3 |
| `src/hardware/linux/memory.rs` | 2 |
| `src/service/polkit.rs` | 1 |
| `src/desktop/system_integration.rs` | 1 |
| `src/service/contract.rs` | 0 (`#[cfg(test)]` builder only) |

**Note:** `README.md:226` claims "108 automated unit and integration tests". The actual
count is **110**. Treat `cargo test` output as authoritative.

**Naming:**
- Integration files: `<area>_tests.rs`
- Test functions: **descriptive snake_case sentences**, no `test_` prefix in newer suites:
  `history_evicts_oldest_beyond_max_samples`, `cpu_sampler_calculates_jiffies_deltas_correctly`,
  `strict_hex_color_validation_success_and_failure`
- Older suites still carry `test_`: `test_fan_mode_state_transitions`,
  `test_embedded_assets_match_disk_files`. **Prefer the descriptive form for new tests.**
- The degradation matrix uses a zero-padded ordinal tied to the matrix row:
  `degradation_01_fan_hwmon_absent_leaves_cpu_temp_working`
  (`tests/degradation_tests.rs:23`)
- Inline modules: `fn <behavior>_<condition>()`, e.g.
  `history_evicts_oldest_beyond_max_samples`, `svg_path_breaks_on_none_gaps`

**Structure:**

```text
tests/
├── benchmark_tests.rs
├── config_tests.rs
├── degradation_tests.rs
├── desktop_integration_tests.rs
├── diagnostics_tests.rs
├── fan_control_tests.rs
├── hardware_service_tests.rs
├── linux_gpu_tests.rs
├── linux_procfs_tests.rs
├── mock_hardware_tests.rs
├── packaging_tests.rs
├── power_preset_tests.rs
├── rgb_tests.rs
├── sensor_registry_tests.rs
└── telemetry_history_tests.rs
```

## Test Structure

**Suite Organization:** every integration file opens with a `//!` module doc comment
listing the requirement IDs it enforces:

```rust
//! Integration tests for zenomen-hw service, verified sysfs writes, and polkit contract.
```

(`tests/hardware_service_tests.rs:1`)

Larger suites enumerate their coverage as a bullet list with requirement IDs:

```rust
//! Verifies:
//! - Multi-variant interface discovery (`FourZone`, `Legacy`, `KernelPatch`) (RGB-01)
//! - Preset definitions (`White`, `Gaming`, `Rainbow`, `Off`) (RGB-03)
```

(`tests/rgb_tests.rs:1-14`)

**Setup pattern:** build the fixture, exercise the public API, assert on returned data.
No `#[before]`/`#[after]` macros, no `ctor`. Setup is plain Rust at the top of the test.

**Teardown pattern:** explicit `let _ = fs::remove_dir_all(&root);` at the end, **or** a
`Drop` impl for helper structs that own a temp directory (`tests/diagnostics_tests.rs:28-32`,
`tests/hardware_service_tests.rs:53-57`). Prefer the `Drop` form so a panicking assertion
still cleans up.

**Assertion pattern:** one behaviour per `#[test]`; multiple `assert!` calls are fine when
they check the same scenario. Related state transitions are numbered inline:

```rust
// 1. Success event updates fan mode and clears errors
state.apply(&AppEvent::FanModeChanged(Ok("max".to_string())));
// 2. Failure event preserves active mode and sets error
```

(`tests/fan_control_tests.rs:13-30`)

**Inline module shape:**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn fully_capable() -> HardwareCapabilities {
        HardwareCapabilities::all_supported()
    }

    #[test]
    fn subsystem_count_matches_capability_flags() { ... }
}
```

(`src/core/state.rs:307-321`)

## Mocking

**Framework:** none — the project mocks by **injecting filesystem roots** and by using a
purpose-built mock provider.

**1. Path injection (the dominant pattern).** Production code accepts a root prefix and
tests build a fake sysfs/procfs tree under `std::env::temp_dir()`:

| Injectable entry point | File |
|---|---|
| `SensorRegistry::scan_roots(hwmon_root, acpi_root)` | `src/hardware/registry.rs:42` |
| `LinuxHardwareProvider::with_all_roots(...)` | `src/hardware/linux/provider.rs:55` |
| `HardwareBackend::new(root)` | `src/service/backend.rs:64` |
| `collect_diagnostics_from_roots(...)` | `src/core/diagnostics.rs:74` |
| `open_releases_page_with(launcher)` | `src/desktop/releases.rs:26` |
| `install_to_prefix(prefix)` / `uninstall_from_prefix(prefix)` | `src/desktop/system_integration.rs` |
| `is_autostart_enabled_at(path)` / `set_autostart_enabled_at(path, bool)` | `src/desktop/autostart.rs` |

When adding a new hardware reader, **accept a root/path parameter** rather than reading an
absolute path. That is the only mocking seam the project has.

**2. In-memory provider.** `MockHardware` implements `HardwareProvider` for three
scenarios (`src/hardware/mock.rs:41-58`):
- `MockScenario::Nominal` — every capability true, plausible values
- `MockScenario::ZeroCapabilities` — all capability flags false
- `MockScenario::SensorDataGap` — capabilities advertised but readings `None` for a subset

Selected in production code via `ZENOMEN_MOCK_SCENARIO`:

```bash
ZENOMEN_MOCK_SCENARIO=zero cargo run
ZENOMEN_MOCK_SCENARIO=gap  cargo run
```

(`src/hardware/mock.rs:12-15`, `src/main.rs:167-178`). `MockScenario::from_name` and
`from_env` are parseable, and an unknown value logs a warning and falls back to the real
Linux provider. Tests construct the mock directly with `MockHardware::new(MockScenario::X)`
rather than mutating env vars — `std::env::set_var` is process-global and races under the
parallel test runner.

**3. Test-only builder methods.** Methods gated behind `#[cfg(test)]` may widen
construction options without shipping them:

```rust
#[cfg(test)]
pub fn with_polkit_required(mut self, require_polkit: bool) -> Self { ... }
```

(`src/service/contract.rs:44-47`). Follow this pattern for test-only seams; never add a
`pub` production API that only tests use.

**4. Trait substitution for desktop side effects.** `open_releases_page_with` and the
autostart `*_at` functions let tests pass a closure/path so no browser launches and no file
is written to `$XDG_CONFIG_HOME` (`tests/desktop_integration_tests.rs`).

**What to mock:**
- Every filesystem read under `/sys` or `/proc` — always via a temp root
- External tools (`nvidia-smi`) — test the parser directly
  (`parse_nvidia_smi_output` in `tests/degradation_tests.rs:9`) and never spawn the binary
- All hardware telemetry — `MockHardware`, not the real provider
- Desktop side effects (browser launch, package install, D-Bus daemon)

**What NOT to mock:**
- Pure parsers and math — feed literal `&str` fixtures instead
  (`tests/linux_procfs_tests.rs:11-19`)
- `AppState` / `AppEvent` transitions — drive the real reducer
  (`tests/fan_control_tests.rs`)
- The D-Bus interface structs themselves — call the methods directly and read the
  `mpsc` channel (`tests/desktop_integration_tests.rs:77-88`)

## Fixtures and Factories

**Temp directory helpers.** Four hand-rolled variants exist; none are shared across
crates because each integration test file is its own crate:

```rust
// Ownership + Drop cleanup — preferred
struct TestDir { path: PathBuf }

impl TestDir {
    fn new(prefix: &str) -> Self {
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let path = std::env::temp_dir()
            .join(format!("zenomen_test_{prefix}_{timestamp}"));
        fs::create_dir_all(&path).unwrap();
        Self { path }
    }
}
impl Drop for TestDir {
    fn drop(&mut self) { let _ = fs::remove_dir_all(&self.path); }
}
```

(`tests/diagnostics_tests.rs:12-32`)

```rust
// Domain-specific builder — sensor trees
struct TestHwmonTree(PathBuf);
impl TestHwmonTree {
    fn new(prefix: &str) -> Self { /* nanos-suffixed dir */ }
    fn create_hwmon(&self, id: &str, name: &str,
                    temp_inputs: &[(&str, Option<&str>, &str)],
                    fan_inputs: &[(&str, &str)],
                    has_pwm: bool) { ... }
}
```

(`tests/sensor_registry_tests.rs:12-70`)

```rust
// Full fake machine — hwmon + ACPI in one rooted tree
struct MockSysfs { root: PathBuf }
impl MockSysfs {
    fn new(test_name: &str) -> Self { /* writes name, pwm1_enable, fan*_input,
                                        platform_profile_choices, platform_profile */ }
    fn backend(&self) -> HardwareBackend { HardwareBackend::new(&self.root) }
}
```

(`tests/hardware_service_tests.rs:10-57`)

**Location:** helpers live at the top of the test file that uses them. There is no shared
`tests/common/` module.

**Naming:** `temp_test_dir(name)`, `create_temp_dir(name)`, `TestDir`, `TestHwmonTree`,
`MockSysfs`. Prefix strings identify the subsystem: `zenomen_rgb_test_`,
`zenomen_config_test_`, `zenomen_deg_`, `zenomen_test_hw_`.

**Collision warning:** helpers either append `std::time::SystemTime::now()` nanos
(`tests/rgb_tests.rs:17-27`, `tests/config_tests.rs:17-27`,
`tests/sensor_registry_tests.rs:14-22`, `tests/diagnostics_tests.rs:15-22`,
`tests/linux_gpu_tests.rs`) **or** only `std::process::id()`
(`tests/degradation_tests.rs:13-18`, `tests/hardware_service_tests.rs:16-20`,
`tests/desktop_integration_tests.rs:19`, `tests/power_preset_tests.rs`,
`tests/packaging_tests.rs`). The pid-only form is collision-safe across processes but not
within one process if two tests share a prefix. **When adding a helper, include both
`std::process::id()` and a nanosecond timestamp**, and remove any pre-existing directory
before creating it (`let _ = fs::remove_dir_all(&root);`).

**Test data:** inline string literals, not files on disk:

```rust
let sample1 = "\
cpu  1000 0 500 8500 0 0 0 0 0 0
cpu0 500 0 250 4250 0 0 0 0 0 0
";
```

(`tests/linux_procfs_tests.rs:11-15`)

Real repo files are read directly when the assertion is "embedded asset matches shipped
file":

```rust
let desktop_disk = fs::read_to_string("data/applications/zenomen.desktop")
    .expect("desktop entry exists");
assert_eq!(EMBEDDED_DESKTOP_ENTRY, desktop_disk);
```

(`tests/packaging_tests.rs:13-18`)

## Coverage

**Requirements:** None enforced. No `tarpaulin`, `llvm-cov`, `grcov`, Codecov, or coverage
badge exists anywhere in the repo or CI.

**View Coverage:**

```bash

# Not configured. If needed locally:

cargo install cargo-llvm-cov
cargo llvm-cov --all --html
```

Do not add a coverage gate without a project-level decision; the current quality bar is
**zero clippy warnings + all tests green + degradation matrix complete**, not a percentage.

## Test Types

**Unit tests (inline `#[cfg(test)] mod tests`):**
- Scope: pure functions, reducer logic, parsers, math
- Location: bottom of `src/core/state.rs`, `src/core/telemetry.rs`,
  `src/hardware/linux/{cpu,gpu,hwmon,memory}.rs`, `src/service/polkit.rs`,
  `src/desktop/system_integration.rs`
- Approach: `use super::*;` for private access; no filesystem unless the function owns it

**Integration tests (`tests/*_tests.rs`):**
- Scope: public API across module boundaries, real filesystem under a temp root,
  event channels, D-Bus interface structs
- Approach: import through `zenomen::` (the public crate name), never `crate::`
- Are the primary suite; 79 of the 110 tests live here

**End-to-end tests:** not used. No `assert_cmd`, no `trycmd`, no spawned GUI process.

**Benchmark / performance tests:**
- `tests/benchmark_tests.rs` asserts RSS bounds (`< 500 MB` sanity, `> 0`),
  per-tick latency, and headless worker behaviour
- `BenchmarkTimer::start("name")` marks named phases (`tests/benchmark_tests.rs:35`)
- These are ordinary `#[test]` functions — no `criterion`, no `#[bench]`

**Degradation tests:**
- `tests/degradation_tests.rs` is a numbered matrix (`degradation_01` … `degradation_08`)
  proving that removing one sensor, interface, tool, or daemon produces a clean
  "unavailable" state and no panic
- Each row is cross-referenced in `docs/RESOURCE-BUDGET.md`. **Adding a new optional
  hardware feature means adding a degradation row and a test.**

## Common Patterns

**Async testing:** there is effectively none. The D-Bus layer is `async` (`zbus`), but no
test uses `#[tokio::test]` or awaits a zbus call. Instead, tests exercise the synchronous
pieces around the async boundary:

- Call the interface method directly on the struct and assert the `mpsc` message:
  ```rust
  let (action_tx, action_rx) = mpsc::sync_channel(5);
  let service = ApplicationService::new(action_tx);
  assert!(service.present_window().is_ok());
  assert_eq!(action_rx.try_recv().unwrap(), DesktopAction::PresentWindow);
  ```
  (`tests/desktop_integration_tests.rs:77-84`)
- Spawn the real worker thread and use `recv_timeout`:
  ```rust
  let worker = spawn_monitoring_worker(provider, event_sender, command_receiver);
  let event = event_receiver
      .recv_timeout(Duration::from_millis(500))
      .expect("Worker must send initial capabilities immediately");
  ```
  (`tests/benchmark_tests.rs:44-52`)

Always use `recv_timeout`, never a bare `recv()` — a hung worker must fail the test rather
than stall CI.

**Error testing:**

```rust
let err = FanMode::parse_requested("turbo").unwrap_err();
assert!(matches!(err, HardwareControlError::InvalidParameter(_)));

assert!(matches!(RgbColor::from_hex(""), Err(RgbError::EmptyColor)));
```

(`tests/hardware_service_tests.rs:75-82`, `tests/rgb_tests.rs:45-49`)

Match on the enum variant; do not assert on the `Display` string unless the message itself
is the contract.

**Timing / latency testing:**

```rust
let start = Instant::now();
// ... exercise the function ...
assert!(start.elapsed() < Duration::from_millis(5));
```

(`tests/benchmark_tests.rs`, `tests/telemetry_history_tests.rs`)

Prefer generous bounds (milliseconds, not microseconds) because CI runners are noisy.

**Host-dependent guards.** Tests that touch real hardware or an installed daemon must
detect the situation and return early rather than fail:

```rust
let status = client.status();
if status.is_connected() {
    eprintln!("skipping ...: production service is active on host");
    return;
}
```

(`tests/fan_control_tests.rs:67-77`)

and privilege checks:

```rust
if !is_root() {
    assert!(matches!(install(), Err(SystemIntegrationError::NotRoot)));
}
```

(`tests/packaging_tests.rs:56-62`)

**Fragile counter-example — do not copy.** `live_host_initialization_resolves_real_sensors`
(`tests/sensor_registry_tests.rs:193-209`) asserts that a `k10temp` sensor is present on
the build host. That is an AMD-only assumption and will fail on Intel CI runners or
containers with no hwmon. New host-touching tests should skip when the expected device is
absent, not assert it exists.

**CWD dependency.** Tests that read `data/...` or `Cargo.toml`
(`tests/packaging_tests.rs`) are repo-relative and only pass when `cargo test` runs from the
repository root. Never `cd` inside a test; if a test needs a repo file, resolve it from
`env!("CARGO_MANIFEST_DIR")` in new code.

## Gaps

- **No Slint rendering tests.** No test constructs an `AppWindow`, calls `show()`, or
  inspects a rendered surface. `src/app.rs` (`apply_to_ui`, callback wiring) is untested.
  The `xvfb-run` requirement exists only because the linked library initialises a
  display connection.
- **No real D-Bus integration test.** The polkit contract, service registration,
  `zenomen-hw` daemon startup, and `HardwareServiceClient` handshake are validated by
  unit-level struct tests, not by running a bus. A regression in the XML interface or
  `.service` file would only be caught by packaging asset equality tests.
- **`src/service/contract.rs` has no `#[test]`.** Its `#[cfg(test)] with_polkit_required`
  builder is never exercised, and `authorize()` has no coverage.
- **Duplicated fixtures.** Four temp-dir helpers and two hwmon-tree builders differ only in
  naming. Extracting `tests/common/mod.rs` would reduce drift, but note that each
  `tests/*.rs` is a separate crate and would need `mod common;`.
- **Repo-relative file reads** make `tests/packaging_tests.rs` fail when run from any
  directory other than the repo root.
- **No coverage measurement**, so untested modules (`src/app.rs`, `src/service/contract.rs`,
  `src/desktop/tray.rs`) are invisible to any metric.
- **README test count is stale** (108 vs. the actual 110). Update `README.md:226` whenever
  suites are added.

---

*Testing analysis: 2026-11-06*
