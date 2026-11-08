# Codebase Structure

**Analysis Date:** 2026-11-08

## Directory Layout

```text
zenomen/
├── .github/                 # CI (Linux+Windows quality gates) and release workflows
├── .planning/               # GSD planning docs (PROJECT, ROADMAP, phases, codebase)
├── data/                    # Runtime assets shipped by packaging and the root installer
│   ├── applications/        # zenomen.desktop freedesktop entry
│   ├── dbus/                # com.zenomen.Hardware1 system-bus service + policy
│   ├── icons/               # Application icons (png/svg/ico, hicolor theme)
│   ├── polkit/              # com.zenomen.hardware.policy authorization action
│   └── systemd/             # zenomen-hw.service unit
├── docs/                    # Architecture notes, ADRs, testing/handoff docs
│   ├── architecture/        # CROSS_PLATFORM_ARCHITECTURE, HP_WMI_PROTOCOL, IPC_SECURITY
│   ├── decisions/           # ADR-001..ADR-004
│   └── testing/             # Windows test matrix, board validation, audits
├── logo/                    # Source logo SVGs (not shipped directly)
├── packaging/windows/       # WiX MSI definitions (ZenOmen.wxs)
├── scripts/                 # generate-icons.sh, measure-budget.sh, validate-icons.py
├── src/                     # Rust crate (library + two binaries)
│   ├── lib.rs               # Module tree, `slint::include_modules!()`
│   ├── main.rs              # GUI binary entry point (requires `ui` feature)
│   ├── logging.rs           # tracing + rotating file initialization
│   ├── app.rs               # Slint bridge: AppCommand <-> AppEvent
│   ├── bin/
│   │   └── zenomen-hw.rs    # Privileged daemon entry point (no `ui` feature)
│   ├── config/              # AppConfig schema + atomic TOML persistence
│   ├── core/                # Events, state, worker, telemetry, diagnostics, benchmark
│   ├── desktop/             # Single-instance, tray, autostart, releases, installer
│   │   └── windows/         # Win32 implementations of the same surface
│   ├── hardware/            # Hardware abstraction boundary
│   │   ├── linux/           # procfs/sysfs/DRM/nvidia-smi backends
│   │   └── windows/         # PDH/DXGI/WMI backends
│   ├── rgb/                 # RGB schema, presets, OpenOMEN migration
│   ├── service/             # Privileged control: D-Bus, backend, client, polkit
│   │   └── windows/         # SCM service, named pipe, HP WMI, security, board policy
│   └── ui/                  # GPUI-CE view-model scaffold (compiled, not on render path)
├── tests/                   # Integration test files (18)
├── ui/                      # Slint UI definitions (the live render path)
│   ├── app.slint            # AppWindow shell, properties, callbacks, screen router
│   ├── components/          # Reusable .slint components
│   ├── screens/             # One .slint per screen
│   └── theme/               # Color/typography/metrics tokens + bundled fonts
├── build.rs                 # Slint compilation + Windows resource/manifest
├── Cargo.toml               # Features, dependencies, two [[bin]] targets, profiles
├── Cargo.lock               # Committed for reproducible builds and cargo-audit
├── AGENTS.md                # 10 non-negotiable engineering rules
├── README.md
├── LICENSE                  # GPL-3.0-only
└── NOTICE
```

## Directory Purposes

**`src/core/`:**
- Purpose: The session's truth and the core↔UI contract. No hardware access.
- Contains: `events.rs` (`AppCommand`, `AppEvent`, `ScreenId`), `state.rs` (`AppState::apply`), `worker.rs` (1 Hz loop + mutation offload), `telemetry.rs` (bounded history + SVG paths), `diagnostics.rs` (JSON report), `benchmark.rs` (RSS/PSS instrumentation).
- Key files: `src/core/events.rs`, `src/core/state.rs:109`, `src/core/worker.rs:34`.

**`src/hardware/`:**
- Purpose: One trait, per-platform implementations, and shared data types.
- Contains: `provider.rs` (trait + `HardwareCapabilities` + `TelemetrySnapshot` + `BatteryTelemetry`), `registry.rs` (canonical sysfs bindings), `rgb.rs` (interface variant detection), `mock.rs`, `unavailable.rs`, `linux/`, `windows/`.
- Key files: `src/hardware/provider.rs`, `src/hardware/mod.rs`, `src/hardware/registry.rs`.

**`src/hardware/linux/`:**
- Purpose: All Linux-specific sensor code. This is the only place allowed to name `/sys` or `/proc` sensor paths for telemetry.
- Contains: `provider.rs` (`LinuxHardwareProvider`), `cpu.rs` (proc/stat sampler, cpuinfo fallback), `gpu.rs` (DRM enumeration, AMD sampling, `NvidiaSampler` thread), `memory.rs` (`/proc/meminfo`), `hwmon.rs` (hwmon scan/read primitives).
- Key files: `src/hardware/linux/provider.rs`, `src/hardware/linux/gpu.rs:191`, `src/hardware/linux/cpu.rs:104`.

**`src/hardware/windows/`:**
- Purpose: All Windows-specific read-only telemetry.
- Contains: `provider.rs`, `cpu.rs` (PDH/system times), `gpu.rs` (DXGI adapters + GPU performance collector), `memory.rs` (`GlobalMemoryStatusEx`), `battery.rs` (`GetSystemPowerStatus`), `sensors.rs` (WMI temperature/fan probes), `device.rs` (board ID, BIOS, PnP).
- Key files: `src/hardware/windows/provider.rs:9`, `src/hardware/windows/device.rs:69`.

**`src/service/`:**
- Purpose: Privileged mutation seam and its client.
- Contains: `contract.rs` (zbus `com.zenomen.Hardware1` interface with Polkit gate), `backend.rs` (verified sysfs writes, presets, RGB), `client.rs` (`HardwareServiceClient`, platform switch), `polkit.rs`, `types.rs` (`FanMode`, `PresetResult`, `HardwareControlError`), `protocol.rs` (reserved platform-neutral wire contract, not on the live path).
- Key files: `src/service/backend.rs:12`, `src/service/contract.rs:14`, `src/service/client.rs:44`.

**`src/service/windows/`:**
- Purpose: Windows privileged service — SCM lifecycle, named-pipe transport, HP WMI driver, board policy.
- Contains: `lifecycle.rs` (register/uninstall/run SCM service), `server.rs` (`handle_request`, `run_pipe_server`), `pipe.rs` (pipe creation + framing), `protocol.rs` (`HardwareRequest`/`HardwareResponse`, 4-byte BE length frames), `security.rs` (DACL SDDL + console-session check), `authorization.rs` (board validation gate), `profiles.rs` (`BoardProfile`, `BoardValidation`), `hp_wmi.rs` (HP `hpqBIntM` command set), `client.rs` (`WindowsServiceClient`).
- Key files: `src/service/windows/lifecycle.rs:9`, `src/service/windows/server.rs:11`, `src/service/windows/hp_wmi.rs:13`.

**`src/desktop/`:**
- Purpose: Desktop residency and OS integration (XDG/D-Bus on Linux, Win32 on Windows).
- Contains: `single_instance.rs` (`com.zenomen.App` session bus), `tray.rs` (StatusNotifierItem + dbusmenu), `autostart.rs` (XDG `.desktop`), `releases.rs` (browser-delegated update link), `system_integration.rs` (root installer with `include_str!` embedded assets), `windows/` (mutex, `Shell_NotifyIconW` tray, HKCU Run autostart, known folders, notifications, MSI/service installer).
- Key files: `src/desktop/single_instance.rs:10`, `src/desktop/system_integration.rs:12`, `src/desktop/windows/mod.rs`.

**`src/config/`:**
- Purpose: Persistent preferences.
- Contains: `schema.rs` (`AppConfig`, `GeneralConfig`, `TelemetryConfig`, polling bounds), `mod.rs` (`project_dirs`, `default_config_path`, `load_or_default`, `save_atomic`, `sanitize`).
- Key files: `src/config/schema.rs:17`, `src/config/mod.rs:28`.

**`src/rgb/`:**
- Purpose: Keyboard lighting domain.
- Contains: `schema.rs` (`RgbColor`, `RgbProfile`, `RgbError`), `migration.rs` (`builtin_presets`, `migrate_openomen_profiles`, `parse_openomen_profile`).
- Key files: `src/rgb/schema.rs:29`, `src/rgb/migration.rs`.

**`src/ui/` (Rust):**
- Purpose: Parallel GPUI-CE view-model scaffold for a planned toolkit migration.
- Contains: `app.rs` (`GpuiApp`), `components/` (`MetricCard`, `SidebarView`, `StatusStrip`, `TelemetryChart`, `ValueReadout`, `ActionButton`), `screens/` (one model per Slint screen), `theme/` (`Palette`, `RgbaColor`, `Spacing`, `Typography`).
- Key files: `src/ui/mod.rs`, `src/ui/app.rs:19`.
- **Important:** `GpuiApp` is never constructed by `src/main.rs`; the live UI is Slint. Treat this tree as design scaffolding, not the application.

**`ui/` (Slint):**
- Purpose: The live presentation layer.
- Contains: `app.slint` (root `AppWindow` with all `in-out property` values, `callback` declarations, screen router), `components/` (14 reusable components), `screens/` (overview, cooling, performance, lighting, settings, diagnostics), `theme/` (`colors.slint`, `typography.slint`, `metrics.slint`, bundled Inter + JetBrains Mono fonts).
- Key files: `ui/app.slint:29`, `ui/theme/metrics.slint`, `build.rs` (compiles with the `fluent` style).

**`data/`:**
- Purpose: Files installed into system locations by packaging or `--install-system-integration`.
- Contains: desktop entry, systemd unit, D-Bus service + policy, Polkit policy, icon set.
- Key files: `data/applications/zenomen.desktop`, `data/systemd/zenomen-hw.service`, `data/dbus/com.zenomen.Hardware1.service`, `data/polkit/com.zenomen.hardware.policy`.

**`tests/`:**
- Purpose: Integration tests exercising cross-module behavior without live hardware.
- Contains: 18 test files, ~3,800 lines, covering mock hardware, degradation, config, telemetry history, sensor registry, fan/power control, RGB, desktop integration, diagnostics, packaging, logging, benchmark, Linux procfs/GPU, Windows telemetry/service.
- Key files: `tests/hardware_service_tests.rs`, `tests/fan_control_tests.rs`, `tests/degradation_tests.rs`, `tests/sensor_registry_tests.rs`.

## Key File Locations

**Entry Points:**
- `src/main.rs`: GUI binary (`zenomen`), requires `ui` feature; bootstrap, provider selection, lifecycle.
- `src/bin/zenomen-hw.rs`: privileged daemon (`zenomen-hw`), builds without Slint.
- `build.rs`: compiles `ui/app.slint` when `CARGO_FEATURE_UI` is set; Windows icon/manifest.

**Configuration:**
- `Cargo.toml`: features (`ui`, default), two `[[bin]]` targets, release/dev/test profiles, platform dependencies.
- `src/config/mod.rs` / `src/config/schema.rs`: runtime configuration.
- `.github/workflows/ci.yml`: Linux (fmt, clippy `-D warnings`, xvfb tests, `cargo audit`) and Windows quality gates.
- `.github/workflows/release.yml`: packaging/release pipeline.

**Core Logic:**
- `src/core/events.rs`: `AppCommand`, `AppEvent`, `ScreenId` — start here to understand any feature.
- `src/core/state.rs`: `AppState::apply` — every visible value originates here.
- `src/core/worker.rs`: cadence, sampling, mutation threading.
- `src/app.rs`: property mapping (`apply_to_ui`) and callback wiring (`install_callbacks`).
- `src/hardware/provider.rs`: the hardware contract and absence semantics.
- `src/service/backend.rs` / `src/service/contract.rs`: verified writes and authorization.

**Testing:**
- `tests/*.rs`: integration tests (build with default features; several are platform-gated).
- Inline `#[cfg(test)]` modules in: `src/core/state.rs`, `src/core/telemetry.rs`, `src/core/worker.rs`, `src/hardware/unavailable.rs`, `src/hardware/linux/{cpu,gpu,hwmon,memory}.rs`, `src/service/{contract,polkit,protocol}.rs`, `src/desktop/system_integration.rs`.
- `scripts/measure-budget.sh`: RSS budget measurement.

## Naming Conventions

**Rust files:**
- `snake_case`, one concern per file: `state.rs`, `worker.rs`, `provider.rs`, `registry.rs`.
- Platform directories are literal `linux` / `windows` (never `unix`/`win`).
- `mod.rs` re-exports the public surface of each directory (see `src/hardware/mod.rs`, `src/service/mod.rs`, `src/desktop/mod.rs`).

**Rust types:**
- `PascalCase` structs/enums/traits: `AppState`, `AppEvent`, `AppCommand`, `HardwareProvider`, `TelemetrySnapshot`, `FanMode`, `PresetResult`, `BoardProfile`.
- Trait names are capability nouns (`HardwareProvider`); error enums end in `Error` (`HardwareControlError`, `RgbError`, `SystemIntegrationError`, `ReleasesError`).
- Constructors: `new`, `with_*` for alternate roots/config, `from_*` for conversions (`from_snapshot`, `from_state`, `from_index`, `from_hex`, `from_sysfs`).

**Rust functions/variables:**
- `snake_case`, verb-first: `sample_telemetry`, `read_cpu_temp`, `apply_rgb_hardware`, `write_and_verify`, `spawn_monitoring_worker`.
- Event/command variants are imperative noun phrases (`SetFanMode`, `ApplyPreset`, `HistoryWindowChanged`).

**Tests:**
- Test names are full sentences describing the invariant, e.g. `a_second_mutation_is_rejected_while_one_is_in_flight`, `starts_waiting_for_a_reading_and_reports_nothing_supported`, `one_missing_interface_is_degraded`.
- Fixture helpers are local `fn`s inside the test module (`fully_capable()`, `full_snapshot()`).

**Slint files:**
- File names `kebab-case`: `ui/components/action-button.slint`, `ui/components/status-strip.slint`, `ui/screens/overview.slint`.
- Component names `PascalCase`: `AppWindow`, `MetricCard`, `StatusStrip`, `UnavailablePanel`.
- Properties and callbacks `kebab-case`: `active-screen`, `has-cpu-metrics`, `refresh-telemetry`, `history-window-changed`.
- Cross-file imports use forward-slash relative paths (`"theme/colors.slint"`, `"../components/value-readout.slint"`).

**Requirement traceability:**
- Comments cite requirement IDs in parentheses: `FOUND-04`, `FOUND-05`, `MON-04`, `MON-11`, `SYS-06`, `WIN-03`, `RGB-02`, `REL-06`. Preserve this convention when touching traced behavior.

## Where to Add New Code

**New telemetry sensor:**
1. Add the field to `TelemetrySnapshot` in `src/hardware/provider.rs` and decide its absence type (`Option<T>` or empty collection — never `0`).
2. Implement the read in the matching subsystem file: `src/hardware/linux/<subsystem>.rs` or `src/hardware/windows/<subsystem>.rs`.
3. Wire it into `src/hardware/linux/provider.rs` (or `windows/provider.rs`) inside `sample_telemetry`.
4. If it is a capability, add a flag to `HardwareCapabilities`, `HardwareCapabilities::*` constructors, and `SensorRegistry::detect_capabilities`.
5. Populate a value in `src/hardware/mock.rs`; leave `None`/zero-capability in `src/hardware/unavailable.rs`.
6. Update `TelemetrySnapshot::EXPECTED_SENSOR_COUNT` and `reported_sensor_count` in `src/hardware/provider.rs`.
7. Add a Slint property in `ui/app.slint` and format it in `apply_to_ui` (`src/app.rs`).
8. Add tests in `tests/linux_*`/`tests/sensor_registry_tests.rs` and a unit test beside the parser.

**New screen:**
- Slint: create `ui/screens/<name>.slint`, import it in `ui/app.slint`, add it to `screen-titles`, the router block, and the sidebar. Bump the `current-screen` range check in `ui/app.slint`.
- Core: add a variant to `ScreenId`, then update `ALL`, `index()`, `from_index()`, and `title()` in `src/core/events.rs`.
- Rust view model (optional, scaffold only): `src/ui/screens/<name>.rs` + re-export in `src/ui/screens/mod.rs`.

**New UI command:**
- Add a variant to `AppCommand` (`src/core/events.rs`).
- Handle it in the worker `match` (`src/core/worker.rs`) — never do work in the callback.
- Emit the corresponding `AppEvent`; add the `AppState::apply` arm and the `apply_to_ui` formatting.
- Wire the Slint callback in `ZenOmenApp::install_callbacks` (`src/app.rs:94`).

**New privileged operation:**
- Backend: add the verified operation to `src/service/backend.rs` (use `write_and_verify`).
- D-Bus: add an `async fn` to the `#[interface(name = "com.zenomen.Hardware1")]` impl in `src/service/contract.rs` and call `self.authorize(&hdr).await?` for any mutation.
- Client: add the corresponding method to `src/service/client.rs` (and the Windows variant in `src/service/windows/client.rs`).
- Windows: add `HardwareRequest`/`HardwareResponse` variants (`src/service/windows/protocol.rs`), dispatch in `server::handle_request`, and an authorization rule in `src/service/windows/authorization.rs`.
- Worker: route the command through `execute_mutating_action` so single-flight and the 30 s timeout apply.

**New configuration field:**
- Add it to `src/config/schema.rs` with `#[serde(default)]` and include it in `sanitize()`.
- Handle `AppCommand::UpdateConfig` persistence in `src/core/worker.rs`.
- Add a Slint property + callback and bridge it in `src/app.rs`.
- Add a round-trip test in `tests/config_tests.rs`.

**New desktop behavior:**
- Implement Linux semantics in `src/desktop/<area>.rs` and Windows semantics in `src/desktop/windows/<area>.rs`.
- Re-export from `src/desktop/mod.rs` under the existing `#[cfg(target_os = "windows")]` block.
- Wire into `src/main.rs` bootstrap/teardown.

**Utilities:**
- Shared helpers belong in the module that owns the domain. There is no generic `src/utils/`; keep helpers close to use and prefer existing domain types (`FanMode`, `RgbColor`, `AppEvent`) over new ad-hoc structs.

## Special Directories

**`.planning/`:**
- Purpose: GSD project management (`PROJECT.md`, `REQUIREMENTS.md`, `ROADMAP.md`, `STATE.md`, `phases/`, `research/`, `codebase/`).
- Generated: Mixed — authored plans and generated maps.
- Committed: Yes.

**`target/` and `dist/`:**
- Purpose: Cargo build output and packager staging.
- Generated: Yes.
- Committed: No (`.gitignore`).

**`data/`:**
- Purpose: Install-source assets; embedded into the binary via `include_str!` in `src/desktop/system_integration.rs`.
- Generated: No (icons are generated by `scripts/generate-icons.sh` but committed).
- Committed: Yes.

**`ui/theme/fonts/`:**
- Purpose: Bundled Inter and JetBrains Mono TTFs with their OFL license files.
- Generated: No.
- Committed: Yes.

**`packaging/windows/` and `packaging/windows/wix/`:**
- Purpose: WiX MSI authoring for the Windows installer.
- Generated: No.
- Committed: Yes.

**`logo/`:**
- Purpose: Source logo SVGs, not packaged directly (shipped icons live in `data/icons/`).
- Generated: No.
- Committed: Yes.

## Build Wiring Notes

- `Cargo.toml` declares `[[bin]] zenomen` with `required-features = ["ui"]`; `zenomen-hw` has no such requirement so the daemon builds with `--no-default-features`.
- `build.rs` compiles `ui/app.slint` only when `CARGO_FEATURE_UI` is set, and pins the `fluent` style so developer and CI rendering match.
- `src/lib.rs` gates `pub mod app` and `pub mod ui` behind the `ui` feature and calls `slint::include_modules!()` to generate `AppWindow` from `ui/app.slint`.
- On Windows, `build.rs` attaches `data/icons/zenomen.ico`, version metadata, and an `asInvoker` DPI-aware manifest.

---

*Structure analysis: 2026-11-08*
