---
last_mapped_commit: 30d62de2cdf6f507822944e5602bdd907539ce38
last_mapped_at: 2026-11-06
---
# Codebase Structure

**Analysis Date:** 2026-11-06

## Directory Layout

```text
zenomen/
├── Cargo.toml              # Package, deps, release profile, Cargo Packager metadata
├── Cargo.lock              # Committed on purpose (application, not library)
├── build.rs                # Compiles ui/app.slint; Windows icon resource
├── HEADERS/NOTICE/LICENSE  # GPL-3.0, HP trademark disclaimer
├── README.md               # Overview, security model, install/build instructions
├── AGENTS.md               # Agent guide (contains embedded legacy Python context — not source of truth)
│
├── src/
│   ├── main.rs             # `zenomen` GUI process entry point
│   ├── lib.rs              # Library module tree + `slint::include_modules!()`
│   ├── app.rs              # Slint bridge (callbacks → commands, events → properties)
│   ├── bin/
│   │   └── zenomen-hw.rs   # `zenomen-hw` privileged D-Bus daemon entry point
│   ├── core/               # State, events, worker, history, diagnostics, benchmark
│   ├── config/             # TOML schema + atomic persistence
│   ├── hardware/           # Provider boundary, sensor registry, mock, Linux adapters
│   │   └── linux/          # cpu.rs, gpu.rs, hwmon.rs, memory.rs, provider.rs
│   ├── service/            # D-Bus contract, daemon backend, blocking client, polkit
│   ├── desktop/            # Single instance, SNI tray, autostart, releases, installer
│   └── rgb/                # Color/profile schema, OpenOMEN migration
│
├── ui/
│   ├── app.slint           # Window shell, sidebar/header/status strip, screen router
│   ├── screens/            # overview, cooling, performance, lighting, settings, diagnostics
│   ├── components/         # Reusable Slint components (cards, charts, buttons, pills)
│   └── theme/              # colors.slint, typography.slint, metrics.slint, fonts/
│
├── tests/                  # Integration tests, one file per subsystem (15 files)
├── data/                   # Installed assets: desktop entry, D-Bus, systemd, polkit, icons
│   ├── applications/
│   ├── dbus/
│   ├── polkit/
│   ├── systemd/
│   └── icons/hicolor/
├── scripts/                # measure-budget.sh (RSS/CPU measurement harness)
├── docs/                   # PACKAGING.md, PARITY.md, RESOURCE-BUDGET.md
├── .github/workflows/      # ci.yml (quality gates), release.yml (tagged builds)
├── .planning/              # GSD project artifacts (PROJECT, ROADMAP, phases, codebase)
├── logo/                   # Source brand art
├── dist/                   # Generated packages + budget CSVs (gitignored)
└── target/                 # Cargo build output (gitignored)
```

## Directory Purposes

**`src/` (root files):**
- Purpose: The two process entry points and the single UI bridge.
- Contains: `main.rs` (GUI orchestration), `bin/zenomen-hw.rs` (daemon), `lib.rs` (module tree), `app.rs` (bridge).
- Key files: `src/main.rs:19`, `src/bin/zenomen-hw.rs:11`, `src/app.rs:36`.

**`src/core/`:**
- Purpose: The contract between UI and hardware, plus state and derived views.
- Contains: `events.rs` (`ScreenId`, `AppCommand`, `AppEvent`), `state.rs` (`AppState::apply`), `worker.rs` (1 Hz loop + mutation offload), `telemetry.rs` (ring buffer + SVG paths), `diagnostics.rs` (report/export), `benchmark.rs` (RSS/CPU metrics), `mod.rs` (re-exports).
- Key files: `src/core/worker.rs:33`, `src/core/state.rs:31`, `src/core/telemetry.rs:93`.

**`src/config/`:**
- Purpose: Durable user preferences.
- Contains: `schema.rs` (`AppConfig`, `GeneralConfig`, `TelemetryConfig`, bounds), `mod.rs` (ProjectDirs path, load with corruption backup, atomic save).
- Key files: `src/config/schema.rs:17`, `src/config/mod.rs:34`, `src/config/mod.rs:85`.

**`src/hardware/`:**
- Purpose: Everything that touches a sensor or kernel interface on the **read** path.
- Contains: `provider.rs` (trait + data types), `registry.rs` (canonical sensor scan), `mock.rs` (scenarios), `rgb.rs` (variant detection), `linux/` (CPU, GPU, hwmon, memory, provider), `mod.rs` (re-exports).
- Key files: `src/hardware/provider.rs:232`, `src/hardware/registry.rs:34`, `src/hardware/linux/provider.rs:35`.

**`src/hardware/linux/`:**
- Purpose: Concrete Linux telemetry parsers.
- Contains: `cpu.rs` (jiffies deltas, model, frequency), `gpu.rs` (DRM enumeration, `nvidia-smi` with timeout), `hwmon.rs` (device scan, temp/fan/label readers), `memory.rs` (`/proc/meminfo`), `provider.rs` (assembly + capabilities).
- Key files: `src/hardware/linux/cpu.rs:9`, `src/hardware/linux/gpu.rs:66`, `src/hardware/linux/hwmon.rs:33`, `src/hardware/linux/memory.rs:30`.

**`src/service/`:**
- Purpose: The **write** path and the privilege boundary.
- Contains: `contract.rs` (`HardwareService`, `#[interface(name = "com.zenomen.Hardware1")]`, `authorize`), `backend.rs` (`HardwareBackend`, `write_and_verify`, presets, RGB writes), `client.rs` (blocking GUI-side proxy, status/polkit probes), `polkit.rs` (`CheckAuthorization`), `types.rs` (`FanMode`, `PresetResult`, `HardwareControlError`).
- Key files: `src/service/contract.rs:83`, `src/service/backend.rs:12`, `src/service/client.rs:34`, `src/service/polkit.rs:10`.

**`src/desktop/`:**
- Purpose: Desktop-environment integration on the session bus and XDG paths.
- Contains: `single_instance.rs` (`com.zenomen.App`), `tray.rs` (StatusNotifierItem + DBusMenu), `autostart.rs` (`~/.config/autostart/zenomen.desktop`), `releases.rs` (`xdg-open` releases page), `system_integration.rs` (root installer with embedded assets).
- Key files: `src/desktop/single_instance.rs:67`, `src/desktop/tray.rs:186`, `src/desktop/system_integration.rs:122`.

**`src/rgb/`:**
- Purpose: RGB profile model and legacy-data import.
- Contains: `schema.rs` (`RgbColor`, `RgbProfile`, strict hex parsing), `migration.rs` (built-in presets, OpenOMEN profile parsing, non-destructive copy).
- Key files: `src/rgb/schema.rs:32`, `src/rgb/migration.rs:52`, `src/rgb/migration.rs:185`.

**`ui/`:**
- Purpose: Declarative presentation only; no hardware access, no formatting of raw values.
- Contains: `app.slint` (shell/router), `screens/` (six screens), `components/` (13 reusable components), `theme/` (color/type/metric tokens + vendored fonts).
- Key files: `ui/app.slint:29`, `ui/screens/overview.slint`, `ui/components/status-strip.slint`, `ui/theme/colors.slint`.

**`tests/`:**
- Purpose: Integration and contract tests against the public library API.
- Contains: 15 `*_tests.rs` files covering config, degradation, diagnostics, fans, service, GPU, procfs, mock, packaging, presets, RGB, registry, telemetry history, benchmark, desktop integration.
- Key files: `tests/sensor_registry_tests.rs`, `tests/hardware_service_tests.rs`, `tests/degradation_tests.rs`.

**`data/`:**
- Purpose: Assets installed by Cargo Packager and by `--install-system-integration`.
- Contains: `applications/zenomen.desktop`, `dbus/com.zenomen.Hardware1.{conf,service}`, `polkit/com.zenomen.hardware.policy`, `systemd/zenomen-hw.service`, `icons/hicolor/*/apps/zenomen.*`, `icons/zenomen.{png,svg,ico}`.
- Key files: `data/systemd/zenomen-hw.service`, `data/dbus/com.zenomen.Hardware1.conf`, `data/polkit/com.zenomen.hardware.policy`.

**`scripts/`:**
- Purpose: Reproducible measurement of the resource budget.
- Contains: `measure-budget.sh` (RSS from `/proc/<pid>/status`, CPU from `/proc/<pid>/stat`, `--soak` mode).

**`docs/`:**
- Purpose: Human documentation for packaging, feature parity, and resource budgets.

**`.github/workflows/`:**
- Purpose: CI and release automation.
- Contains: `ci.yml` (fmt, clippy `-D warnings`, tests, `cargo audit`), `release.yml` (tag/version check, DEB + AppImage, checksums).

**`.planning/`:**
- Purpose: GSD project memory (requirements, roadmap, phases, research, codebase maps).
- Generated: partly (by GSD commands); committed: yes.

## Key File Locations

**Entry Points:**
- `src/main.rs`: GUI startup, flags, threading, teardown.
- `src/bin/zenomen-hw.rs`: daemon startup and D-Bus name ownership.
- `build.rs`: Slint compilation (`ui/app.slint`, style `fluent`).
- `src/lib.rs`: module tree and `slint::include_modules!()`.
- `ui/app.slint`: window shell and screen router.

**Configuration:**
- `Cargo.toml`: dependencies, release profile, packager metadata, DEB files/depends.
- `build.rs`: build-time Slint configuration.
- `src/config/schema.rs`: user-facing config schema and bounds.
- `.github/workflows/ci.yml`, `.github/workflows/release.yml`: automation gates.
- `.gitignore`: ignores `/target` and `dist/`; keeps `Cargo.lock`.

**Core Logic:**
- `src/core/worker.rs`: sampling cadence, visibility gating, command handling.
- `src/core/state.rs`: authoritative view state and degradation logic.
- `src/core/events.rs`: command/event vocabulary and screen enum.
- `src/core/telemetry.rs`: bounded history and chart geometry.
- `src/hardware/registry.rs`: canonical sensor selection.
- `src/service/backend.rs`: all verified writes.

**UI:**
- `ui/screens/*.slint`: one file per screen.
- `ui/components/sidebar.slint`: nav model (`row-count: 6`).
- `ui/components/status-strip.slint`: global status/degradation line.
- `ui/theme/colors.slint`, `typography.slint`, `metrics.slint`: all design tokens.

**Testing:**
- `tests/*_tests.rs`: integration tests linked against the `zenomen` library.
- Inline `#[cfg(test)] mod tests` in `src/core/state.rs`, `src/core/telemetry.rs`, `src/hardware/linux/*`, `src/service/polkit.rs`, etc.

## Naming Conventions

**Files (Rust):**
- `snake_case.rs`; module name equals file name (`src/core/worker.rs` → `core::worker`).
- Binary targets in `src/bin/` use kebab-case matching the installed command: `zenomen-hw.rs` → binary `zenomen-hw`.
- Test files: `<subsystem>_tests.rs` at the `tests/` root (e.g. `sensor_registry_tests.rs`, `linux_gpu_tests.rs`).

**Files (Slint):**
- kebab-case for multiword components: `metric-card.slint`, `status-strip.slint`, `telemetry-chart.slint`, `screen-header.slint`.
- Single-word screens are lowercase: `overview.slint`, `cooling.slint`, `settings.slint`.
- Exported component names are PascalCase and match the file's role: `MetricCard`, `StatusStrip`, `TelemetryChart`, `Overview`.

**Rust Identifiers:**
- Types/traits/enums: PascalCase (`HardwareProvider`, `TelemetrySnapshot`, `AppCommand`).
- Functions/methods/modules: snake_case, verb-first for actions (`sample_telemetry`, `write_and_verify`, `collect_diagnostics`).
- Constants: SCREAMING_SNAKE_CASE (`MAX_SAMPLES`, `SAMPLING_INTERVAL`, `COMMAND_QUEUE_DEPTH`, `POLKIT_ACTION_CONTROL`).
- Struct fields mirror the domain concept, not the UI label (`cpu_temperature_c`, `ram_used_mb`, `fan_rpms`, `pending_fan_mode`).
- Test functions describe behavior: `starts_waiting_for_a_reading_and_reports_nothing_supported`, `history_strictly_bounds_samples_to_120_on_overflow`.

**Slint Identifiers:**
- Properties and callbacks: kebab-case (`has-cpu-metrics`, `status-degraded`, `set-fan-mode`, `export-diagnostics`).
- Globals: `ThemeColors`, `ThemeTypography`, `ThemeMetrics`.
- Component instances use lowercase names (`sidebar`, `root`, `shell-focus`).

**Files (data/packaging):**
- D-Bus/polkit names use reverse-DNS: `com.zenomen.Hardware1`, `com.zenomen.Hardware1.conf`, `com.zenomen.hardware.policy`.
- systemd unit: `zenomen-hw.service`.
- Desktop entry: `zenomen.desktop`.

**Environment variables:**
- `ZENOMEN_*` prefix: `ZENOMEN_MOCK_SCENARIO`, `ZENOMEN_HW_SESSION`, `ZENOMEN_BENCHMARK`; standard `RUST_LOG`.

## Where to Add New Code

**New telemetry sensor (read-only):**
1. Parser/reader: new or existing module under `src/hardware/linux/` returning `Option<T>` when absent.
2. Sampling: add the field to `TelemetrySnapshot` in `src/hardware/provider.rs:152` and populate it in `LinuxHardwareProvider::sample_telemetry` (`src/hardware/linux/provider.rs:115`).
3. Mock parity: populate it in `src/hardware/mock.rs` so scenarios remain representative.
4. Capability: add a flag to `HardwareCapabilities` (`src/hardware/provider.rs:28`) and set it in `SensorRegistry::detect_capabilities` (`src/hardware/registry.rs:117`).
5. Bridge: add a Slint property in `ui/app.slint` and write it in `apply_to_ui` (`src/app.rs:386`); empty string means unavailable.
6. Tests: `tests/<subsystem>_tests.rs` plus a slice in `tests/degradation_tests.rs` for the absent case.

**New hardware control operation (write path):**
1. Backend: method on `HardwareBackend` using `write_and_verify` (`src/service/backend.rs:12`), with validation before writing.
2. Contract: `#[interface]` method in `src/service/contract.rs:83` taking `#[zbus(header)]` and calling `self.authorize(&hdr).await?` first.
3. Client: blocking method in `src/service/client.rs:34`; wrap errors with `format_service_error` guidance.
4. Command: new `AppCommand` variant (`src/core/events.rs:71`) and a worker arm (`src/core/worker.rs:120`) using `execute_mutating_action`.
5. Event: new `AppEvent` variant (`src/core/events.rs:115`), handled in `AppState::apply` (`src/core/state.rs:109`).
6. UI: callback in `ui/app.slint`, screen wiring in `ui/screens/<screen>.slint`, property publication in `apply_to_ui` (`src/app.rs:386`).
7. Tests: `tests/hardware_service_tests.rs` for backend/contract, `tests/fan_control_tests.rs`-style state tests.

**New screen:**
1. `ui/screens/<name>.slint` exporting a PascalCase component.
2. `ScreenId` variant + `ALL` + `index()` + `from_index()` + `title()` (`src/core/events.rs:16-67`).
3. Router: import, property plumbing, and `if root.current-screen == N` block in `ui/app.slint`; update the `<= 5` guard (`ui/app.slint:191-192`).
4. Sidebar: row model and `row-count` in `ui/components/sidebar.slint:52`.
5. Bridge: any new callbacks in `ZenOmenApp::install_callbacks` (`src/app.rs:94`).

**New reusable component:**
- Implementation: `ui/components/<kebab-name>.slint`; export PascalCase; take all data as `in property`, emit `callback`.
- Reuse: import relatively from screens (`../components/...`) and from the shell (`components/...`).
- No literal colors or sizes: use `ui/theme/colors.slint`, `typography.slint`, `metrics.slint`.

**New configuration setting:**
1. Field in `src/config/schema.rs` with `#[serde(default = "...")]`.
2. Bounds/validation in `AppConfig::sanitize` (`src/config/schema.rs:90`).
3. `AppCommand` (or `UpdateConfig`) handling in `src/core/worker.rs:151-221`.
4. `AppState` field (`src/core/state.rs:31`) and `ConfigLoaded` publishing in `apply_to_ui` (`src/app.rs:397`).
5. Settings UI property + callback (`ui/screens/settings.slint`, `ui/app.slint`).
6. Round-trip test in `tests/config_tests.rs`.

**New diagnostics field:**
1. Add to `DiagnosticsReport` (`src/core/diagnostics.rs:29`).
2. Populate in `collect_diagnostics_from_roots` (`src/core/diagnostics.rs:74`) using an injectable path for tests.
3. Surface in `ui/screens/diagnostics.slint` only if it is user-facing.
4. Test in `tests/diagnostics_tests.rs`.

**New desktop integration or installed asset:**
1. Asset under `data/` in the correct subdirectory.
2. Embed with `include_str!` and add install/uninstall targets in `src/desktop/system_integration.rs:12-26`, `:122-176`.
3. Add the mapping to `[package.metadata.packager.deb.files]` in `Cargo.toml:67-75`.
4. Assert presence/content in `tests/packaging_tests.rs`.

**New test:**
- Integration: `tests/<area>_tests.rs`, importing through `zenomen::...`.
- Unit: inline `#[cfg(test)] mod tests` in the module under test.
- Prefer `scan_roots`/`with_all_roots`/`HardwareBackend::new(prefix)` injection over touching real `/sys`; see `tests/sensor_registry_tests.rs` and `tests/linux_procfs_tests.rs`.

## Special Directories

**`target/`:**
- Purpose: Cargo build artifacts including Slint-generated Rust.
- Generated: Yes.
- Committed: No (`.gitignore`).

**`dist/`:**
- Purpose: Cargo Packager output (`.deb`, `.AppImage`) and budget measurement CSVs.
- Generated: Yes.
- Committed: No (`.gitignore`); budget CSVs here are produced by `scripts/measure-budget.sh` during local measurement.

**`data/icons/hicolor/`:**
- Purpose: Multi-resolution installed icons (16–512 px + scalable).
- Generated: No (checked-in art).
- Committed: Yes.

**`ui/theme/fonts/`:**
- Purpose: Vendored Inter and JetBrains Mono faces with their OFL license files.
- Generated: No.
- Committed: Yes; keep the `*-OFL.txt` beside each face.

**`.agents/skills/`:**
- Purpose: Project-skill discovery location.
- Generated: No.
- Committed: Yes, but currently empty — no `SKILL.md` index exists, so there are no project-specific agent skills to apply.

**`.planning/`:**
- Purpose: GSD state (`PROJECT.md`, `REQUIREMENTS.md`, `ROADMAP.md`, `phases/`, `research/`, `codebase/`).
- Generated: Yes (by GSD commands).
- Committed: Yes.

**`logo/`:**
- Purpose: Source logo/brand artwork used to produce `data/icons/*`.

---

*Structure analysis: 2026-11-06*
