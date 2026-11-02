---
last_mapped_commit: e50af8f9ea61300a1c10d36dc6954e319da7b6e2
last_mapped_at: 2026-11-02
---
# Codebase Structure

**Analysis Date:** 2026-11-02

## Directory Layout

```text
zenomen/
├── src/
│   ├── main.rs                 # GUI binary entry: flags, config, threads, event loop
│   ├── lib.rs                  # Library facade + slint::include_modules!()
│   ├── app.rs                  # The only UI <-> core bridge (ZenOmenApp)
│   ├── core/                   # Commands, events, state, worker, history, diagnostics
│   ├── hardware/               # Read boundary: provider trait, Linux + mock backends
│   │   └── linux/              # procfs/sysfs parsers (cpu, memory, hwmon, gpu)
│   ├── service/                # Write boundary: D-Bus client, contract, backend, polkit
│   ├── desktop/                # Tray, single instance, autostart, releases, installer
│   ├── config/                 # TOML schema + atomic persistence
│   ├── rgb/                    # RGB schema, presets, OpenOMEN migration
│   └── bin/
│       └── zenomen-hw.rs       # Privileged D-Bus daemon entry point
├── ui/
│   ├── app.slint               # Window shell, properties, screen router
│   ├── screens/                # One file per screen (6)
│   ├── components/             # Reusable presentation components
│   └── theme/                  # Colors, typography, metrics, bundled fonts
├── tests/                      # Integration tests (one file per subsystem)
├── data/                       # Packaged system assets (desktop, D-Bus, systemd, polkit, icons)
├── docs/                       # Packaging, parity and resource-budget documentation
├── scripts/                    # Operational tooling (resource budget harness)
├── dist/                       # Packaging output + budget CSVs (gitignored)
├── logo/                       # Source logo assets
├── build.rs                    # Compiles ui/app.slint at build time
├── Cargo.toml                  # Deps, release profile, Cargo Packager metadata
├── Cargo.lock                  # Committed: application, audited in CI
└── .github/workflows/          # ci.yml (fmt, clippy, xvfb tests, audit), release.yml
```

## Directory Purposes

**`src/` (library + GUI binary):**
- Purpose: All Rust application code. The crate is both a library (`zenomen`) and two binaries.
- Contains: Module roots with `mod.rs` re-exports, one concern per file.
- Key files: `src/lib.rs`, `src/main.rs`, `src/app.rs`, `src/bin/zenomen-hw.rs`.

**`src/core/`:**
- Purpose: Everything between the UI bridge and the hardware boundary.
- Contains: `events.rs` (typed contract), `state.rs` (single source of truth), `worker.rs` (1 Hz loop), `telemetry.rs` (bounded history + SVG paths), `diagnostics.rs` (JSON report), `benchmark.rs` (RSS instrumentation).
- Key files: `src/core/events.rs`, `src/core/worker.rs`, `src/core/state.rs`.

**`src/hardware/`:**
- Purpose: The read-only telemetry boundary.
- Contains: `provider.rs` (trait + data types), `registry.rs` (canonical sensor bindings), `mock.rs` (deterministic scenarios), `rgb.rs` (interface variant detection), `linux/` (kernel parsers).
- Key files: `src/hardware/provider.rs`, `src/hardware/registry.rs`, `src/hardware/linux/provider.rs`.

**`src/hardware/linux/`:**
- Purpose: Pure(ish) parsers for kernel interfaces, each testable against a temp root or fixture string.
- Contains: `cpu.rs`, `memory.rs`, `hwmon.rs`, `gpu.rs`, `provider.rs` (assembly), `mod.rs`.
- Key files: `src/hardware/linux/gpu.rs` (DRM enumeration + `nvidia-smi` timeout), `src/hardware/linux/hwmon.rs` (device scan + descriptors).

**`src/service/`:**
- Purpose: The privileged write boundary and its client.
- Contains: `client.rs` (blocking zbus client), `contract.rs` (`#[zbus::interface]` + polkit gate), `backend.rs` (verified sysfs writes), `polkit.rs` (authority calls), `types.rs` (`FanMode`, `PresetResult`, errors).
- Key files: `src/service/contract.rs`, `src/service/backend.rs`, `src/service/client.rs`.

**`src/desktop/`:**
- Purpose: Desktop-session integration without GTK/Qt.
- Contains: `single_instance.rs` (`com.zenomen.App`), `tray.rs` (StatusNotifierItem + dbusmenu), `autostart.rs` (XDG), `releases.rs` (`xdg-open`), `system_integration.rs` (root installer with `include_str!` assets).
- Key files: `src/desktop/tray.rs`, `src/desktop/system_integration.rs`.

**`src/config/`:**
- Purpose: Persistent preferences with corruption recovery.
- Contains: `schema.rs` (`AppConfig`, bounds, `sanitize()`), `mod.rs` (`project_dirs`, `load_or_default`, `save_atomic`).
- Key files: `src/config/schema.rs`, `src/config/mod.rs`.

**`src/rgb/`:**
- Purpose: Lighting domain model and legacy import.
- Contains: `schema.rs` (`RgbColor`, `RgbProfile`, strict hex), `migration.rs` (presets, OpenOMEN import, default dirs).
- Key files: `src/rgb/schema.rs`, `src/rgb/migration.rs`.

**`ui/`:**
- Purpose: The compiled Slint interface. No hardware access, no numeric interpretation.
- Contains: `app.slint` (shell/router), `screens/`, `components/`, `theme/`.
- Key files: `ui/app.slint`, `ui/components/sidebar.slint`, `ui/theme/metrics.slint`.

**`ui/screens/`:**
- Purpose: One exported component per screen, matching `ScreenId` order.
- Contains: `overview.slint`, `cooling.slint`, `performance.slint`, `lighting.slint`, `settings.slint`, `diagnostics.slint`.
- Key files: `ui/screens/overview.slint` (grid reflow), `ui/screens/settings.slint` (local `ToggleSwitch`).

**`ui/components/`:**
- Purpose: Shared visual primitives.
- Contains: `metric-card`, `value-readout`, `status-pill`, `status-strip`, `screen-header`, `section-card`, `unavailable-panel`, `capability-notice`, `action-button`, `telemetry-chart`, `sparkline`, `sidebar`, `icons`.
- Key files: `ui/components/unavailable-panel.slint` (the "Unavailable" contract), `ui/components/telemetry-chart.slint` (consumes SVG path strings).

**`ui/theme/`:**
- Purpose: Design tokens and bundled fonts (Inter, JetBrains Mono) so rendering is deterministic offline.
- Contains: `colors.slint`, `typography.slint`, `metrics.slint`, `fonts/`.

**`tests/`:**
- Purpose: Integration tests that exercise the public library API against temp roots and in-memory data.
- Contains: 15 `*_tests.rs` files grouped by subsystem.
- Key files: `tests/hardware_service_tests.rs`, `tests/sensor_registry_tests.rs`, `tests/degradation_tests.rs`.

**`data/`:**
- Purpose: System files shipped by the `.deb` and embedded for the AppImage installer.
- Contains: `applications/zenomen.desktop`, `dbus/` (service + policy conf), `polkit/com.zenomen.hardware.policy`, `systemd/zenomen-hw.service`, `icons/`.
- Key files: `data/systemd/zenomen-hw.service` (sandboxing), `data/polkit/com.zenomen.hardware.policy`.

**`docs/`:**
- Purpose: Human-facing operational documentation.
- Contains: `PACKAGING.md`, `PARITY.md`, `RESOURCE-BUDGET.md`.

**`scripts/`:**
- Purpose: Reproducible measurement and maintenance helpers.
- Contains: `measure-budget.sh` (RSS + CPU sampling harness for `PERF-01/02/06`).

**`dist/`:**
- Purpose: Local packaging output and budget measurement CSVs.
- Generated: Yes. Committed: No (gitignored).

**`target/`:**
- Purpose: Cargo build artifacts.
- Generated: Yes. Committed: No.

**`.planning/`:**
- Purpose: GSD planning artifacts (PROJECT, ROADMAP, REQUIREMENTS, phases, research, this codebase map).
- Generated: Yes (by GSD). Committed: Yes.

**`.github/workflows/`:**
- Purpose: CI gates and release automation.
- Contains: `ci.yml` (fmt check, clippy `-D warnings`, `xvfb-run cargo test --all`, `cargo audit`), `release.yml`.

## Key File Locations

**Entry Points:**
- `src/main.rs`: GUI process (`fn main`, flags, thread orchestration, teardown).
- `src/bin/zenomen-hw.rs`: privileged D-Bus daemon (`HardwareService` on the system bus).
- `ui/app.slint`: `AppWindow` component compiled by `build.rs`.
- `src/desktop/system_integration.rs`: `--install-system-integration` implementation.

**Configuration:**
- `Cargo.toml`: dependencies, Slint feature pinning, release profile, Cargo Packager metadata.
- `build.rs`: Slint compilation with `fluent` style pinned.
- `src/config/schema.rs`: config structure, bounds, defaults.
- `src/config/mod.rs`: config path (`~/.config/zenomen/config.toml`), atomic save, corruption backup.
- `data/polkit/com.zenomen.hardware.policy`: action `com.zenomen.hardware.control`.
- `data/systemd/zenomen-hw.service`: daemon sandboxing and lifecycle.
- `.github/workflows/ci.yml`: required checks.

**Core Logic:**
- `src/core/worker.rs`: sampling cadence, visibility gating, mutation offload.
- `src/core/state.rs`: every event folded into authoritative state.
- `src/core/events.rs`: command/event/screen contract.
- `src/hardware/provider.rs`: `HardwareProvider`, `TelemetrySnapshot`, `HardwareCapabilities`.
- `src/hardware/registry.rs`: sensor priority resolution.
- `src/service/backend.rs`: `write_and_verify` and sysfs operations.
- `src/service/contract.rs`: D-Bus method surface + polkit gate.

**UI:**
- `ui/app.slint`: property declarations, router, global callbacks.
- `ui/components/sidebar.slint`: screen order, labels and icons (must match `ScreenId`).
- `ui/screens/*.slint`: per-screen property inputs and callbacks.

**Testing:**
- `tests/`: 15 integration suites.
- Inline unit tests: `#[cfg(test)] mod tests` at the bottom of modules that need private access (e.g. `src/core/state.rs:293-448`).
- No dev-dependencies, no mock framework, no test config file.

**Assets:**
- `data/icons/`: PNG sizes + SVG + Windows `.ico`.
- `ui/theme/fonts/`: Inter Regular/SemiBold, JetBrains Mono Regular + OFL licenses.
- `logo/`: source logo SVGs.

## Naming Conventions

**Rust files:**
- `snake_case.rs`, one concern per file: `worker.rs`, `single_instance.rs`, `system_integration.rs`.
- Module folders use `mod.rs` with `pub mod` declarations and `pub use` re-exports (`src/core/mod.rs`, `src/service/mod.rs`, `src/hardware/mod.rs`).
- Binary entry is `src/bin/<binary-name>.rs` → `zenomen-hw` (hyphen in the binary name, underscore not used here).

**Rust symbols:**
- Types/traits/enums: `PascalCase` (`HardwareProvider`, `TelemetrySnapshot`, `HardwareControlError`, `RgbInterfaceVariant`).
- Functions/methods/variables: `snake_case`, verb-first (`sample_telemetry`, `write_and_verify`, `find_fan_control_path`, `detect_capabilities`).
- Constants: `UPPER_SNAKE` (`SAMPLING_INTERVAL`, `HISTORY_WINDOW`, `EXPECTED_SENSOR_COUNT`, `POLKIT_ACTION_CONTROL`, `SNI_OBJECT_PATH`).
- Threads: `kebab-case` names prefixed `zenomen-`: `zenomen-monitor-worker`, `zenomen-event-pump`, `zenomen-desktop-actions`, `zenomen-ctrl-<op>`, `zenomen-rpc-<op>`, `zenomen-diag-export`.
- Enum variants: `PascalCase`; `AppCommand` variants are imperative (`SetFanMode`, `RefreshTelemetry`, `UpdateConfig`); `AppEvent` variants are past-tense (`TelemetryUpdated`, `FanModeChanged`, `ConfigLoaded`).
- Error enums use `thiserror` with `#[derive(Debug, Error)]` and explicit `#[error("…")]` messages (`src/service/types.rs`, `src/rgb/schema.rs`, `src/desktop/system_integration.rs`).

**Slint files:**
- File names `kebab-case.slint`, matching the exported component concept: `metric-card.slint` → `MetricCard`, `unavailable-panel.slint` → `UnavailablePanel`, `screen-header.slint` → `ScreenHeader`.
- Exported components: `PascalCase` (`AppWindow`, `Overview`, `Cooling`, `StatusStrip`).
- Properties and callbacks: `kebab-case` (`has-fan-control`, `active-screen`, `set-fan-mode`, `open-diagnostics`).
- Private helpers: `private property`, `public function` for cross-component calls (`focus-active-row`).
- Imports are relative paths with `.slint` extension: `import { ThemeColors } from "theme/colors.slint";` (shell) and `"../theme/colors.slint"` (screens/components).

**Tests:**
- Files: `tests/<subsystem>_tests.rs` (`fan_control_tests.rs`, `linux_procfs_tests.rs`, `hardware_service_tests.rs`).
- Test functions: `test_*` or behaviour sentences (`history_strictly_bounds_samples_to_120_on_overflow`, `capabilities_loaded_clears_degradation`).
- Helpers: a local struct with `Drop` cleanup (`struct MockSysfs { root: PathBuf }` in `tests/hardware_service_tests.rs:8-52`) or a `temp_test_dir(name)` function (`tests/config_tests.rs:16-22`).

**Assets:**
- `data/` mirrors installed system paths: `data/dbus/`, `data/polkit/`, `data/systemd/`, `data/applications/`.
- Icons use freedesktop size folders: `data/icons/hicolor/<size>/apps/zenomen.png`.

## Where to Add New Code

### Adding a new sensor (read-only telemetry)

1. **Parse/read it:** add a function in the relevant `src/hardware/linux/*.rs` module (`cpu.rs` for `/proc`, `memory.rs`, `hwmon.rs`, `gpu.rs`), or a new `src/hardware/linux/<subsystem>.rs` declared in `src/hardware/linux/mod.rs`.
2. **Register canonical binding:** if it is a hwmon device, extend `SensorRegistry` (`src/hardware/registry.rs`): add the descriptor field, resolve it in `scan_roots`, add a `read_*` method.
3. **Extend the snapshot:** add the field to `TelemetrySnapshot` (`src/hardware/provider.rs`) using `Option`/empty semantics, and add it to the `reported_sensor_count()` array. Bump `EXPECTED_SENSOR_COUNT` if it is a new user-visible sensor.
4. **Populate it:** set the field in `LinuxHardwareProvider::sample_telemetry` (`src/hardware/linux/provider.rs`) and in `MockHardware` (`src/hardware/mock.rs`) for all three scenarios.
5. **Capability flag (if a new subsystem):** add to `HardwareCapabilities` + `has_*()` + `none()` + `all_supported()` in `src/hardware/provider.rs`, map it in `SensorRegistry::detect_capabilities`, and bump `SUBSYSTEM_COUNT` in `src/core/state.rs`.
6. **Chart it (optional):** add a series to `TelemetryPoint` and `TelemetryPaths` in `src/core/telemetry.rs`, then generate it in `AppState::apply` (`src/core/state.rs`).
7. **Expose it:** add an `in-out property` to `ui/app.slint` and set it in `apply_to_ui` (`src/app.rs:386-519`) — format to a string in Rust, never in Slint.
8. **Render it:** pass the property to the screen(s) in `ui/app.slint` and consume it in `ui/screens/*.slint`; use `UnavailablePanel` / `CapabilityNotice` when the value is empty.
9. **Test it:** fixture parser test in `tests/linux_procfs_tests.rs` / `tests/linux_gpu_tests.rs`; registry priority test in `tests/sensor_registry_tests.rs`; mock scenario test in `tests/mock_hardware_tests.rs`; degradation test in `tests/degradation_tests.rs`.

If the sensor also needs a **write**, follow "Adding a new D-Bus operation" instead and keep reads local.

### Adding a new D-Bus operation (hardware write)

1. **Backend:** implement the operation in `src/service/backend.rs` on `HardwareBackend`. Resolve paths via `self.resolve_path(...)`; validate every argument; write with `write_and_verify()`; re-read and return the confirmed state. Add any new error shape to `HardwareControlError` in `src/service/types.rs` and map it in `impl From<HardwareControlError> for zbus::fdo::Error`.
2. **Contract:** add an `async fn` inside the existing `#[interface(name = "com.zenomen.Hardware1")] impl HardwareService` block in `src/service/contract.rs`. Mutating methods MUST take `#[zbus(header)] hdr: Header<'_>` and call `self.authorize(&hdr).await?` first. Return `zbus::fdo::Result<T>`; never accept a filesystem path or a command string.
3. **Client:** add a blocking wrapper in `src/service/client.rs` using `conn.call_method(Some(destination), path, Some(interface), "MethodName", &(args,))` and `reply.body().deserialize()`.
4. **Command/event:** add `AppCommand::<Verb><Noun>` to `src/core/events.rs` and a matching `AppEvent::<Noun>Changed(Result<T, String>)`.
5. **Worker:** handle the command in `src/core/worker.rs`, using `execute_mutating_action("<short-name>", sender, || client.method(...), AppEvent::...)` so polkit cannot block sampling.
6. **State:** fold the event in `AppState::apply` (`src/core/state.rs`) — clear pending flags, store confirmed value, set/clear `last_control_error`.
7. **Bridge:** install the Slint callback in `src/app.rs::install_callbacks` (`send_command(...)`) and publish the result in `apply_to_ui`.
8. **UI:** declare the callback and properties in `ui/app.slint`, wire them in the relevant `ui/screens/*.slint`.
9. **Policy:** only if a genuinely different privilege class is needed, add an action ID to `data/polkit/com.zenomen.hardware.policy` and a constant in `src/service/polkit.rs`; otherwise reuse `com.zenomen.hardware.control`.
10. **Test:** extend `tests/hardware_service_tests.rs` with a `MockSysfs` tree asserting success, verification failure, and invalid-argument rejection; extend state transition tests (pattern: `tests/fan_control_tests.rs`) for the event → UI mapping.

### Adding a new UI screen

1. **Create the component:** `ui/screens/<kebab-name>.slint`, exporting `export component <PascalName> inherits Rectangle` with `in property` inputs and `callback` outputs only. Import theme + components from `../theme/…` and `../components/…`. Never read a sensor or call Rust.
2. **Router:** in `ui/app.slint` add the import, extend `screen-titles`, add `if root.current-screen == N : <PascalName> { ... }` inside the content `VerticalLayout`, and map properties/callbacks.
3. **Screen identity (all four places, same change):** in `src/core/events.rs` add a `ScreenId` variant, then update `ScreenId::ALL`, `index()`, `from_index()` (via `ALL`) and `title()`.
4. **Sidebar:** in `ui/components/sidebar.slint` add the label and icon at the same index and bump `row-count`.
5. **Bridge:** declare `in-out property`s on `AppWindow` (`ui/app.slint`), add callbacks there, install them in `src/app.rs::install_callbacks`, and publish any new state from `apply_to_ui`.
6. **State (optional):** only add `AppState` fields if the screen needs derived status (follow the `status_text()` / `is_degraded()` pattern in `src/core/state.rs`).
7. **Test:** there is no Slint test harness; assert the Rust-side invariant instead (screen index/`from_index` round-trip, event → state changes) in a `tests/*_tests.rs` suite, and keep every screen instantiated in `ui/app.slint` so the compiler type-checks it.

### Adding a new test

- **Location:** `tests/<subsystem>_tests.rs` for anything using the public `zenomen::…` API; inline `#[cfg(test)] mod tests` at the bottom of a module for private functions (pattern: `src/core/state.rs:293`).
- **Naming:** `<area>_tests.rs`; snake_case test names describing the guarantee.
- **Filesystem isolation:** build a temp root under `std::env::temp_dir().join(format!("zenomen_test_<name>_{pid}"))`, create the `sys/...` tree the backend expects, and clean up with a `Drop` impl (`tests/hardware_service_tests.rs:8-52`) or at the end of the test.
- **Hardware:** use `MockHardware` / `MockScenario` (`src/hardware/mock.rs`) or `HardwareBackend::new(temp_root)`; never touch real `/sys` in a test.
- **Worker:** use `spawn_monitoring_worker` with a channel and a `MockHardware` (pattern: `tests/benchmark_tests.rs`); always send `AppCommand::Shutdown` and join.
- **Run:** `cargo test --all`; CI runs it under `xvfb-run --auto-servernum` because Slint needs a display (` .github/workflows/ci.yml`).
- **No new dev-dependencies** unless unavoidable; the crate currently has none.

### Adding a new configuration option

1. Add the field with `#[serde(default = "...")]` to `src/config/schema.rs`, plus a default function and bounds.
2. Clamp/validate it in `AppConfig::sanitize()`.
3. Consume it where it belongs (worker for cadence, `src/main.rs` for startup behavior, `src/app.rs` for UI defaults).
4. Add persistence via `AppCommand::UpdateConfig` and an `AppEvent::ConfigLoaded` publish if it is user-editable.
5. Add a test in `tests/config_tests.rs` (roundtrip + clamping + corrupt fallback).

### Adding a new desktop action

1. Extend `DesktopAction` in `src/desktop/single_instance.rs`.
2. Emit it from the relevant D-Bus interface (`src/desktop/single_instance.rs` or `src/desktop/tray.rs`).
3. Handle it in the `zenomen-desktop-actions` loop in `src/main.rs:126-149`, wrapping any UI work in `slint::invoke_from_event_loop`.
4. Test it in `tests/desktop_integration_tests.rs`.

### Adding a new packaged asset

1. Add the file under `data/` mirroring its installed system path.
2. Register it in `[package.metadata.packager.deb.files]` in `Cargo.toml`.
3. If the AppImage/standalone installer needs it, embed it with `include_str!` in `src/desktop/system_integration.rs` and install it via `atomic_write_file`.
4. Extend `tests/packaging_tests.rs`, which asserts embedded assets match disk.

## Special Directories

**`target/`:**
- Purpose: Cargo build output including the generated Slint Rust code.
- Generated: Yes. Committed: No.

**`dist/`:**
- Purpose: `.deb`/AppImage output and resource-budget CSVs (`budget-dashboard-open.csv`, `budget-hw-daemon.csv`, `budget-tray-minimized.csv`).
- Generated: Yes. Committed: No.

**`ui/theme/fonts/`:**
- Purpose: Bundled Inter and JetBrains Mono faces so the UI looks identical offline; includes OFL license files.
- Generated: No. Committed: Yes.

**`data/`:**
- Purpose: Canonical system integration assets, consumed both by Cargo Packager and by `include_str!`.
- Generated: No. Committed: Yes — edits must be reflected in both packaging paths.

**`.planning/`:**
- Purpose: GSD project memory (PROJECT, ROADMAP, REQUIREMENTS, phase artifacts, research, this codebase map).
- Generated: Yes. Committed: Yes.

**`.github/workflows/`:**
- Purpose: CI (`ci.yml`) and release automation (`release.yml`).
- Generated: No. Committed: Yes.

---

*Structure analysis: 2026-11-02*
