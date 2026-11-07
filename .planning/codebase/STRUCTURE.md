---
last_mapped_commit: 761e5f975d7c22d1c48bf777a0bcaa1f2db582ba
last_mapped_at: 2026-11-07
---
# Codebase Structure

**Analysis Date:** 2026-11-07

## Directory Layout

```text
zenomen/
├── Cargo.toml              # Single crate; two [[bin]] targets; `ui` default feature
├── Cargo.lock              # Committed on purpose (application, cargo-audit input)
├── build.rs                # Compiles ui/app.slint only when CARGO_FEATURE_UI is set
├── AGENTS.md               # Non-negotiable engineering rules
├── README.md               # User-facing install/security/architecture overview
├── src/
│   ├── lib.rs              # Module tree; `ui`-gated `app` module + include_modules!()
│   ├── main.rs             # `zenomen` GUI entry point and thread orchestration
│   ├── app.rs              # Slint ↔ core bridge (only file that touches AppWindow)
│   ├── logging.rs          # tracing stderr + daily rotating file
│   ├── bin/
│   │   └── zenomen-hw.rs   # `zenomen-hw` root daemon entry point
│   ├── core/
│   │   ├── mod.rs          # Re-exports the core surface
│   │   ├── events.rs       # AppCommand, AppEvent, ScreenId
│   │   ├── state.rs        # AppState + apply(&AppEvent) + status derivation
│   │   ├── worker.rs       # 1 Hz monitoring worker + mutation offload
│   │   ├── telemetry.rs    # Bounded history + SVG path generation
│   │   ├── diagnostics.rs  # Report assembly + atomic JSON export
│   │   └── benchmark.rs    # RSS/PSS instrumentation and checkpoints
│   ├── hardware/
│   │   ├── mod.rs          # Re-exports; cfg-gated Linux provider
│   │   ├── provider.rs     # HardwareProvider trait + capabilities/snapshot types
│   │   ├── registry.rs     # Canonical sensor discovery (hwmon/ACPI/battery)
│   │   ├── mock.rs         # MockHardware + MockScenario
│   │   ├── unavailable.rs  # Zero-capability stub for unsupported platforms
│   │   ├── rgb.rs          # RgbInterfaceVariant detection (read side)
│   │   └── linux/
│   │       ├── mod.rs      # Linux backend module tree
│   │       ├── provider.rs # LinuxHardwareProvider
│   │       ├── cpu.rs      # /proc/stat, cpufreq, /proc/cpuinfo parsers
│   │       ├── gpu.rs      # DRM enumeration, AMD sysfs, NVIDIA watcher
│   │       ├── memory.rs   # /proc/meminfo parser
│   │       └── hwmon.rs    # /sys/class/hwmon scanner + sensor readers
│   ├── service/
│   │   ├── mod.rs          # D-Bus + control subsystem re-exports
│   │   ├── contract.rs     # #[interface] com.zenomen.Hardware1 + polkit gate
│   │   ├── backend.rs      # Verified sysfs writes, presets, RGB variants
│   │   ├── client.rs       # Blocking zbus client used by the GUI worker
│   │   ├── polkit.rs       # PolicyKit1 CheckAuthorization
│   │   └── types.rs        # FanMode, PresetResult, HardwareControlError
│   ├── desktop/
│   │   ├── mod.rs          # Desktop integration re-exports
│   │   ├── single_instance.rs  # com.zenomen.App session service
│   │   ├── tray.rs         # StatusNotifierItem + DBusMenu
│   │   ├── autostart.rs    # XDG ~/.config/autostart/zenomen.desktop
│   │   ├── releases.rs     # xdg-open releases page (no networking)
│   │   └── system_integration.rs  # Root installer; embeds data/** via include_str!
│   ├── config/
│   │   ├── mod.rs          # Load/atomic-save/corrupt recovery
│   │   └── schema.rs       # AppConfig, GeneralConfig, TelemetryConfig + sanitize
│   └── rgb/
│       ├── mod.rs          # RGB subsystem re-exports
│       ├── schema.rs       # RgbColor/RgbProfile strict validation
│       └── migration.rs    # OpenOMEN profile migration + built-in presets
├── ui/
│   ├── app.slint           # AppWindow shell, properties, callbacks, screen router
│   ├── screens/            # one file per ScreenId (overview, cooling, performance,
│   │                       #   lighting, settings, diagnostics)
│   ├── components/         # sidebar, status-strip, telemetry-chart, sparkline,
│   │                       #   metric-card, value-readout, action-button, etc.
│   └── theme/              # colors/typography/metrics tokens + bundled fonts
├── tests/                  # 16 integration test files (see Testing below)
├── data/
│   ├── applications/zenomen.desktop
│   ├── dbus/com.zenomen.Hardware1.{conf,service}
│   ├── polkit/com.zenomen.hardware.policy
│   ├── systemd/zenomen-hw.service
│   └── icons/              # hicolor PNG sizes + SVG + zenomen.ico
├── docs/
│   ├── PACKAGING.md
│   ├── PARITY.md
│   ├── RESOURCE-BUDGET.md
│   └── decisions/ADR-001-ui-framework.md
├── scripts/                # generate-icons.sh, validate-icons.py, measure-budget.sh
├── logo/                   # source SVGs for branding
├── dist/                   # generated packaging output + budget CSVs (gitignored)
├── .github/workflows/      # ci.yml, release.yml
└── .planning/              # GSD project state (PROJECT, ROADMAP, codebase maps, phases)
```

## Directory Purposes

**`src/core/`:**
- Purpose: the application's truth and cadence, independent of Slint and of any OS path.
- Contains: typed messages, `AppState`, the monitoring worker, bounded telemetry history, diagnostics, RSS instrumentation.
- Key files: `src/core/events.rs`, `src/core/state.rs`, `src/core/worker.rs`.
- Rule: nothing here touches hardware directly; `src/hardware` owns sensors, `src/app.rs` owns rendering.

**`src/hardware/`:**
- Purpose: the only place that reads sensors.
- Contains: the `HardwareProvider` seam, canonical registry, mock/unavailable implementations, platform backend directory.
- Key files: `src/hardware/provider.rs`, `src/hardware/registry.rs`, `src/hardware/linux/provider.rs`.
- Rule: Linux-specific paths stay under `src/hardware/linux/`; new platforms get a sibling directory, not a branch inside the Linux one.

**`src/service/`:**
- Purpose: everything that mutates hardware or speaks D-Bus about it.
- Contains: server interface, verified sysfs backend, Polkit verifier, blocking client, control domain types.
- Key files: `src/service/contract.rs`, `src/service/backend.rs`, `src/service/client.rs`.
- Rule: this is the only module allowed to write sysfs; it must not import Slint or `src/app.rs`.

**`src/desktop/`:**
- Purpose: session-bus and XDG integration that is not hardware-related.
- Contains: single-instance service, tray, autostart, release link, root installer embedding `data/**`.
- Key files: `src/desktop/single_instance.rs`, `src/desktop/tray.rs`, `src/desktop/system_integration.rs`.

**`src/config/`:**
- Purpose: persistent user preferences and validation.
- Contains: TOML schema with serde defaults, atomic save, backup-on-corrupt.
- Key files: `src/config/mod.rs`, `src/config/schema.rs`.

**`src/rgb/`:**
- Purpose: lighting domain model independent of sysfs.
- Contains: colour/profile validation, built-in presets, non-destructive OpenOMEN migration.
- Key files: `src/rgb/schema.rs`, `src/rgb/migration.rs`.

**`ui/`:**
- Purpose: declarative presentation only.
- Contains: one `AppWindow` shell, six screens, reusable components, theme tokens and fonts.
- Key files: `ui/app.slint`, `ui/screens/*.slint`, `ui/components/status-strip.slint`.
- Rule: no timers, no file access, no hardware calls — only properties, callbacks, and bindings.

**`tests/`:**
- Purpose: integration tests across module boundaries using temp filesystem trees and mocks.
- Contains: one file per subsystem area (see “Testing” below).
- Key files: `tests/hardware_service_tests.rs`, `tests/sensor_registry_tests.rs`, `tests/degradation_tests.rs`.

**`data/`:**
- Purpose: files installed to fixed system paths and embedded into the root installer.
- Contains: systemd unit, D-Bus config/service, Polkit policy, desktop entry, icons.
- Key files: `data/systemd/zenomen-hw.service`, `data/dbus/com.zenomen.Hardware1.conf`, `data/polkit/com.zenomen.hardware.policy`.
- Rule: `src/desktop/system_integration.rs` embeds these with `include_str!`; `tests/packaging_tests.rs` asserts embedded copies equal the on-disk files.

## Key File Locations

**Entry Points:**
- `src/main.rs`: GUI binary `zenomen` (requires `ui` feature, `Cargo.toml:40-43`).
- `src/bin/zenomen-hw.rs`: daemon binary `zenomen-hw`.
- `build.rs`: Slint compilation and Windows resource embedding.
- `src/lib.rs`: crate module tree; `slint::include_modules!()` under `cfg(feature = "ui")`.

**Configuration:**
- `Cargo.toml`: features (`ui`), zbus/serde/tracing deps, release memory profile, `[package.metadata.packager]`.
- `src/config/schema.rs`: runtime user configuration schema and bounds.
- `data/systemd/zenomen-hw.service`: sandbox and sysfs carve-outs.
- `data/dbus/com.zenomen.Hardware1.conf`: bus policy (root owns the name; anyone may call).
- `data/polkit/com.zenomen.hardware.policy`: `com.zenomen.hardware.control` action.
- `ui/theme/*.slint`: visual tokens.

**Core Logic:**
- `src/core/state.rs`: single-writer state machine.
- `src/core/worker.rs`: sampling loop and mutation offload.
- `src/core/telemetry.rs`: history bound and SVG geometry.
- `src/hardware/registry.rs`: canonical sensor selection.
- `src/service/backend.rs`: verified hardware writes.
- `src/service/contract.rs`: D-Bus API surface.

**Testing:**
- `tests/hardware_service_tests.rs`: backend writes/presets/RGB against a temp sysfs tree.
- `tests/sensor_registry_tests.rs`: hwmon/ACPI discovery priority.
- `tests/degradation_tests.rs`: zero/partial capability honesty.
- `tests/packaging_tests.rs`: embedded assets match `data/**`.
- `tests/linux_procfs_tests.rs`, `tests/linux_gpu_tests.rs`: parser and DRM fixtures.

## Naming Conventions

**Files:**
- Rust modules: `snake_case.rs`, one concern per file (`single_instance.rs`, `write_and_verify` lives in `backend.rs`).
- Binary targets: hyphenated names matching the installed command (`zenomen-hw`, file `src/bin/zenomen-hw.rs`).
- Slint components: `kebab-case.slint` (`status-strip.slint`, `telemetry-chart.slint`); screens named after the `ScreenId` in lowercase (`overview.slint` ↔ `ScreenId::Overview`).
- Integration tests: `<area>_tests.rs` (`fan_control_tests.rs`, `power_preset_tests.rs`).
- Docs: uppercase hyphenated (`RESOURCE-BUDGET.md`, `PACKAGING.md`); ADRs use `ADR-NNN-topic.md`.

**Functions:**
- `snake_case`, verb-first, domain-specific: `sample_telemetry`, `apply`, `write_and_verify`, `ensure_single_instance`, `find_fan_control_path`.
- Constructors: `new`, `with_capacity`, `with_all_roots`, `scan_roots`, `from_snapshot`, `from_sysfs`.

**Variables:**
- `snake_case`; short-lived channel ends named by role (`event_sender`, `command_receiver`, `action_sender`).
- Shared state named for the owned concept (`state`, `registry_slot`, `mutation_in_flight`, `close_to_tray_flag`).

**Types:**
- `PascalCase` nouns; enum variants describe outcomes, not implementation (`FanModeChanged`, `TelemetrySamplingFailed`, `InterfaceMissing`).
- Traits are capability nouns (`HardwareProvider`); option structs end in `Config`/`Result`/`Snapshot`/`Capabilities`/`Descriptor`.
- Slint: components `PascalCase`; properties and callbacks `kebab-case` (`in-out property <string> status-text`, `callback set-fan-mode(string)`), referenced from Rust as `set_status_text`, `on_set_fan_mode`.

**D-Bus:**
- Bus/interface name: reverse-DNS `com.zenomen.Hardware1`; object path `/com/zenomen/Hardware1`.
- Methods `PascalCase` (`GetCapabilities`, `SetFanMode`, `ApplyPreset`).
- Session app: `com.zenomen.App`, path `/com/zenomen/App`, methods `PresentWindow`/`Activate`/`Quit`.

**Constants:**
- `SCREAMING_SNAKE_CASE` (`EXPECTED_SENSOR_COUNT`, `POLKIT_ACTION_CONTROL`, `SAMPLING_INTERVAL`, `COMMAND_QUEUE_DEPTH`); filesystem targets live beside the installer in `src/desktop/system_integration.rs`.

## Where to Add New Code

**New telemetry sensor / reading:**
1. Read it in the platform backend: `src/hardware/linux/` (parser helpers in `cpu.rs`/`gpu.rs`/`memory.rs`/`hwmon.rs`; discovery in `src/hardware/registry.rs`).
2. Add the field to `TelemetrySnapshot` (`src/hardware/provider.rs:152`). Use `Option`/empty for “not read”; never a fabricated zero.
3. Update `TelemetrySnapshot::reported_sensor_count` and `EXPECTED_SENSOR_COUNT` if it is a counted sensor (`src/hardware/provider.rs:191-216`).
4. Add the capability flag to `HardwareCapabilities` and its `has_*` accessor (`src/hardware/provider.rs`), and set it in `SensorRegistry::detect_capabilities` / `MockHardware` / `UnavailableHardwareProvider`.
5. Format it in `apply_to_ui` (`src/app.rs`) and add a `.slint` property/binding (`ui/app.slint`, screen/component files).
6. Cover with tests in `tests/mock_hardware_tests.rs`, `tests/linux_procfs_tests.rs`, or `tests/sensor_registry_tests.rs`.

**New hardware backend (another OS or validation tier):**
1. Create `src/hardware/<platform>/` with a provider implementing `HardwareProvider` (`Send + Sync`).
2. Re-export it from `src/hardware/mod.rs` behind `#[cfg(target_os = "...")]`.
3. Extend the provider selection in `src/main.rs:186-211`; keep `UnavailableHardwareProvider` as the fallback.
4. Leave `src/hardware/unavailable.rs` as zero-capability; do not add speculative reads there.
5. Add capability/telemetry tests that fail honestly when paths are absent.

**New user-facing hardware control (fan/power/RGB/etc.):**
1. Add the typed operation to `HardwareBackend` in `src/service/backend.rs` using `write_and_verify` (never a raw `fs::write`).
2. Expose it in `src/service/contract.rs` with `#[interface]`; call `self.authorize(&hdr).await?` first if it mutates.
3. Add a client wrapper in `src/service/client.rs` (blocking zbus, actionable errors).
4. Add `AppCommand` + `AppEvent` variants in `src/core/events.rs`, handle the command in `src/core/worker.rs`, and fold the event in `src/core/state.rs::apply`.
5. Publish it in `src/app.rs::apply_to_ui` and wire the callback in `install_callbacks`.
6. Add the `.slint` callback/property and screen UI.
7. If a new Polkit-gated method appears, no policy file change is needed unless the action ID changes; keep `POLKIT_ACTION_CONTROL` (`src/service/polkit.rs:10`).

**New screen:**
1. Add a variant to `ScreenId` and update `ALL`, `index`, `title` (`src/core/events.rs:16-67`).
2. Create `ui/screens/<name>.slint` with an `export component`.
3. Import it in `ui/app.slint`, extend `screen-titles`, the `current-screen` bound, and add an `if root.current-screen == N :` branch.
4. Add the sidebar row (`ui/components/sidebar.slint`) and any callbacks.
5. Update the `AppState` default screen only if the new screen should be the landing page.

**New `AppEvent`/`AppCommand` variant:**
- `src/core/events.rs` (definition) → `src/core/state.rs::apply` (exhaustive match; add a test) → `src/app.rs::apply_to_ui` (publish) → `src/core/worker.rs` (produce/consume). Never mutate UI state from a worker thread directly.

**New setting:**
1. Add the field to `GeneralConfig`/`TelemetryConfig` with a serde default (`src/config/schema.rs`).
2. Add bounds to `AppConfig::sanitize`.
3. Add a command in `src/core/events.rs` and a persistence branch in `src/core/worker.rs`.
4. Add the `.slint` property/callback in `ui/app.slint` and the Settings screen, and set it in `initialize`/`apply_to_ui`.
5. Extend `tests/config_tests.rs`.

**New desktop integration:**
- Session-bus services: `src/desktop/` (new module) + re-export in `src/desktop/mod.rs`; wire the action into `DesktopAction` handling in `src/main.rs` if it affects the window.
- Root-installable assets: add the file under `data/<kind>/`, embed it with `include_str!` in `src/desktop/system_integration.rs`, add an install target constant, and extend `tests/packaging_tests.rs`.
- Packaging mapping: add to `[package.metadata.packager.deb.files]` in `Cargo.toml`.

**New test:**
- Integration: `tests/<area>_tests.rs`; use a `TempTree`/`MockSysfs` struct under `std::env::temp_dir()` with a `Drop` cleanup (see `tests/hardware_service_tests.rs:9-53`). Never touch live `/sys` or `/proc`.
- Unit: `#[cfg(test)] mod tests` at the bottom of the module (see `src/core/state.rs:313`, `src/hardware/linux/hwmon.rs:88`).
- GUI-free tests should use `MockHardware` rather than constructing `AppWindow`.

**Where NOT to add code:**
- No sysfs writes outside `src/service/backend.rs`.
- No `slint` imports outside `src/app.rs` and the gated `src/lib.rs` include.
- No hardware reads in `.slint` files or `src/app.rs`.
- No OS-specific path strings outside `src/hardware/linux/`, `src/service/backend.rs`, and `src/core/diagnostics.rs` (the latter only for report metadata).
- No new async runtime; the GUI stays on `std::thread` + `mpsc` and `zbus::blocking`.

## Testing Layout

`tests/` contains 16 integration files, each targeting one subsystem:

| File | Focus |
|------|-------|
| `hardware_service_tests.rs` | verified writes, fan parsing, presets, RGB against temp sysfs |
| `sensor_registry_tests.rs` | k10temp/coretemp/acpitz priority, fan/ACPI/battery discovery |
| `degradation_tests.rs` | zero/partial capability behaviour and honest UI state |
| `fan_control_tests.rs` | fan mode mapping and D-Bus surface |
| `power_preset_tests.rs` | preset mapping and partial-success semantics |
| `rgb_tests.rs` | hex validation, profiles, migration, variant writes |
| `linux_procfs_tests.rs` | `/proc/stat`, `/proc/meminfo`, cpufreq parsers |
| `linux_gpu_tests.rs` | DRM enumeration and nvidia-smi parsing |
| `diagnostics_tests.rs` | report assembly and JSON export |
| `telemetry_history_tests.rs` | bound, gap handling, SVG paths |
| `mock_hardware_tests.rs` | scenario behaviour |
| `config_tests.rs` | load/save/corruption/sanitize |
| `logging_tests.rs` | log directory and rotation contract |
| `desktop_integration_tests.rs` | autostart, single instance, tray assets |
| `packaging_tests.rs` | embedded assets match `data/**`, root gating |
| `benchmark_tests.rs` | RSS instrumentation and checkpoints |

CI runs `cargo fmt --all -- --check`, `cargo clippy --all-targets --all-features -- -D warnings`, `xvfb-run --auto-servernum cargo test --all`, and `cargo audit` (`.github/workflows/ci.yml`). Tests that construct the Slint window require Xvfb.

## Special Directories

**`data/`:**
- Purpose: installable system assets and their embedded copies.
- Generated: No.
- Committed: Yes.

**`dist/`:**
- Purpose: packager output and resource-budget CSVs (`budget-dashboard-open.csv`, `budget-hw-daemon.csv`, `budget-tray-minimized.csv`).
- Generated: Yes.
- Committed: No (`.gitignore` ignores `dist/`).

**`ui/theme/fonts/`:**
- Purpose: bundled Inter and JetBrains Mono fonts with licenses, so rendering is deterministic offline.
- Generated: No.
- Committed: Yes.

**`logo/` and `data/icons/`:**
- Purpose: brand sources (`logo/*.svg`) and packaged icon sizes (`data/icons/hicolor/<size>/apps/zenomen.png`, plus `.ico`).
- Generated: Partly (`scripts/generate-icons.sh`, `scripts/validate-icons.py`); outputs are committed.
- Committed: Yes.

**`scripts/`:**
- Purpose: icon generation/validation and RSS-budget measurement (`measure-budget.sh`).
- Generated: No.
- Committed: Yes.

**`docs/`:**
- Purpose: packaging, platform parity, resource budget, and ADRs. Update these when interfaces or build requirements change (AGENTS.md rule 10).
- Generated: No.
- Committed: Yes.

**`.github/workflows/`:**
- Purpose: `ci.yml` quality gates and `release.yml` tagged Linux builds/publishing.
- Generated: No.
- Committed: Yes.

**`.planning/`:**
- Purpose: GSD project state (`PROJECT.md`, `ROADMAP.md`, `REQUIREMENTS.md`, phases, this `codebase/` map).
- Generated: By GSD commands.
- Committed: Yes.

**`target/`:**
- Purpose: Cargo build artifacts (dev/test profiles disable debuginfo and incremental to limit size).
- Generated: Yes.
- Committed: No.

**`.claude/`, `.agents/`, `.gemini/`, `.gsd/`, `.freebuff/`, `.mcp.json`:**
- Purpose: local agent/tooling configuration; not part of the shipped application.
- Generated: Tooling.
- Committed: Only `.mcp.json` is tracked; the rest are local.

---

*Structure analysis: 2026-11-07*
