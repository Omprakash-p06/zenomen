# Roadmap: ZenOmen

## Overview

ZenOmen replaces the Python OpenOMEN CLI with a Rust + Slint desktop application, built in five phases that each end in something you can actually run. It implements a **two-process application architecture with three logical layers**: an unprivileged desktop GUI (`zenomen-gui`) that owns a strict ≤40 MB peak RSS budget, and a privileged hardware service (`zenomen-hw`) owning privileged sysfs writes behind mandatory polkit authorization with an independent ≤15 MB RSS budget.

Phase 1 puts a working GUI on mock hardware so the UI, the worker and the resource instrumentation exist before any hardware code does. Phase 2 makes it real for reading — telemetry (including RAM from `/proc/meminfo` implemented for the first time), a single shared `SensorRegistry`, capability detection and diagnostics — which requires no privilege and is where the app first becomes useful on your laptop. Phase 3 is the trust-critical one: the privileged D-Bus service (`com.zenomen.Hardware1`), mandatory polkit authorization, verified fan and power writes, and combined presets reporting granular `PresetResult` rather than claiming atomic transactions. Phase 4 turns it into a desktop application — tray, autostart, single instance with D-Bus window activation (`com.zenomen.App`) as a fallback for stock GNOME, persisted settings, and basic RGB with safe migration of existing JSON profiles. Phase 5 makes it shippable: DEB and AppImage, an audit-gated release pipeline, a measured resource budget distinguishing cold start, warm dashboard, minimized tray, and peak RSS, the ZenOmen rebrand, and removal of the Python CLI once parity is proven.

## Phases

**Phase Numbering:**
- Integer phases (1, 2, 3): Planned milestone work
- Decimal phases (2.1, 2.2): Urgent insertions (marked with INSERTED)

Decimal phases appear between their surrounding integers in numeric order.

- [x] **Phase 1: Foundation — GUI Shell on Mock Hardware** - Rust + Slint app with all six screens, theme, worker and instrumentation, running end-to-end on a mock provider
- [x] **Phase 2: Linux Telemetry & Capability Detection** - Real CPU/GPU/RAM/thermal readings, shared SensorRegistry, capability-aware UI, charts and diagnostics export
- [ ] **Phase 3: Verified Hardware Control** - Privileged D-Bus service, mandatory polkit, verified fan and ACPI writes, PresetResult reporting
- [ ] **Phase 4: Desktop Integration, Config & Basic RGB** - Tray, autostart, single instance with GNOME fallback, persisted settings, RGB presets with safe legacy JSON migration
- [ ] **Phase 5: Distribution, Resource Budget & Rebrand** - DEB/AppImage, audit-gated releases, empirical memory/CPU verification, ZenOmen rebrand, Python CLI removed

## Phase Details

### Phase 1: Foundation — GUI Shell on Mock Hardware

**Goal**: A runnable ZenOmen application whose entire UI is driven by mock hardware data, with the resource instrumentation needed to hold the 40 MB budget later.
**Mode:** mvp
**Depends on**: Nothing (first phase)
**UI hint**: yes
**Requirements**: FOUND-01, FOUND-02, FOUND-03, FOUND-04, FOUND-05, MON-09, PERF-03
**Success Criteria** (what must be TRUE):
  1. Launching the app opens the Overview page in the ZenOmen dark theme, and all six screens are reachable by sidebar click and by keyboard alone.
  2. The Overview shows plausible CPU, GPU, RAM, fan and power readings sourced entirely from the mock provider, including a rolling chart fed by a fixed-capacity history buffer (120 samples).
  3. Running the app with a mock provider configured to report zero capabilities still launches and explains what is unsupported, with no panic and no blank panels.
  4. Window frame time stays stable while the monitoring worker publishes updates — no hardware read or external command is reachable from a UI callback.
  5. Memory and CPU instrumentation logs metrics distinguishing cold start, warm dashboard, minimized tray, and peak RSS; CI fails on a formatting violation, a clippy warning, a failing test, or a dependency advisory (`cargo audit`).

**Plans**: 3/3 plans executed

Plans:
- [x] 01-01: Rust + Slint scaffold, build pipeline, theme tokens and CI gates (`fmt`, `clippy -D warnings`, `cargo audit`)
- [x] 01-02: Application shell — navigation, six screens, Overview bound to core events (`AppCommand`/`AppEvent`)
- [x] 01-03: Mock hardware provider, monitoring worker, bounded history (120 samples) and benchmark instrumentation

### Phase 2: Linux Telemetry & Capability Detection

**Goal**: ZenOmen shows real readings from this machine, knows exactly which capabilities it has, and can produce a diagnostics export.
**Mode:** mvp
**Depends on**: Phase 1
**Requirements**: MON-01, MON-02, MON-03, MON-04, MON-05, MON-06, MON-07, MON-08, MON-10, MON-11, MON-12, DIAG-01, DIAG-02
**Success Criteria** (what must be TRUE):
  1. On a supported OMEN/Victus machine the Overview shows real CPU utilization (aggregate and per-core deltas without sleeps), frequency, temperature, GPU readings (dynamically discovered DRM paths, no `card0` hardcoding), RAM (computed via `MemAvailable` from `/proc/meminfo`, implemented for the first time), and fan RPM matching kernel state.
  2. CPU temperature and hardware sensors are resolved through a single shared `SensorRegistry` driven by driver and label priority, ensuring dashboard and diagnostics identify the exact same sensor.
  3. On this machine, a component that is genuinely absent (for example no NVIDIA GPU, or a fan sensor that is not exposed) shows "Unavailable" with a reason — never `0`.
  4. Charts accumulate from the live stream and the history window can be changed in the UI without a restart; manual refresh updates the dashboard on demand.
  5. A diagnostics export written to a file contains version, OS/kernel, hardware identifiers, detected sensors from the shared registry, capability status and recent errors.

**Plans**: 3/3 plans executed

Plans:
- [x] 02-01: Linux hardware provider — procfs (stat delta, meminfo), dynamic DRM sysfs, NVIDIA fallback, and hwmon telemetry (Wave 1)
- [x] 02-02: Capability detection and single shared SensorRegistry across dashboard and diagnostics (Wave 2, depends on 02-01)
- [x] 02-03: Live charts, history window controls, on-demand refresh, diagnostics detection report and export (Wave 3, depends on 02-02)

### Phase 3: Verified Hardware Control

**Goal**: Fan modes and power profiles can be changed safely — through a privileged service, with every write verified by read-back and honest reporting of what actually changed.
**Mode:** mvp
**Depends on**: Phase 2
**Requirements**: COOL-01, COOL-02, COOL-03, COOL-04, COOL-05, COOL-06, PWR-01, PWR-02, PWR-03, PWR-04, PWR-05, PWR-06, SVC-01, SVC-02, SVC-03, SVC-04, SVC-05, SVC-06, DIAG-04
**Success Criteria** (what must be TRUE):
  1. Changing the fan mode changes the mode on the machine, and the UI shows the value read back from hardware with retry/backoff rather than the requested one. Validated modes (Auto and Max) are supported; manual mode is not offered merely because numeric 1 exists in sysfs.
  2. A write that does not stick after retries is reported as a `VERIFICATION_FAILED` error with an explanation — never as success.
  3. The GUI process runs strictly unprivileged; all privileged work lives in the `zenomen-hw` system D-Bus service, which enforces mandatory polkit authorization (`com.zenomen.hardware.control`) on every mutating method.
  4. The service implements a formalized API contract (`GetCapabilities`, `GetFanStatus`, `SetFanMode`, `GetPowerProfiles`, `GetPowerProfile`, `SetPowerProfile`, `ApplyRgbProfile`, `SetRgbEnabled`), rejects arbitrary paths or commands, and diagnostics show service connection and permission status.
  5. Only profiles and fan modes the machine advertises are offered; unavailable controls are explained rather than hidden silently.
  6. Applying a combined preset (Gaming, Balanced, Quiet) reports granular results via `PresetResult` (`power_applied`, `fan_applied`, `partial_success`), reporting partial success honestly without claiming false atomic rollback.

**Plans**: 3/3 plans executed

Plans:
- [x] 03-01: `zenomen-hw` service — formalized D-Bus contract (`com.zenomen.Hardware1`), mandatory polkit authorization, systemd sandboxing, whitelisted validation (Wave 1)
- [x] 03-02: Verified fan control — mode read/write (Auto/Max), read-back with retry/backoff, capability gating, Cooling page (Wave 2, depends on 03-01)
- [x] 03-03: ACPI power profiles, combined presets with `PresetResult` partial-apply reporting, Performance page (Wave 3, depends on 03-02)

### Phase 4: Desktop Integration, Config & Basic RGB

**Goal**: ZenOmen behaves like a real desktop application — it lives in the tray, starts with the session, remembers settings, and can drive keyboard lighting with existing profiles safely carried over.
**Mode:** mvp
**Depends on**: Phase 3
**Requirements**: RGB-01, RGB-02, RGB-03, RGB-04, RGB-05, RGB-06, RGB-07, RGB-08, SYS-01, SYS-02, SYS-03, SYS-04, SYS-05, SYS-06, SYS-07, SYS-08, SYS-09, SYS-10, DIAG-03
**Success Criteria** (what must be TRUE):
  1. The tray menu opens the dashboard, shows current profile, opens diagnostics, toggles launch at login and quits; closing window honours close-to-tray preference, and quitting is a separate action.
  2. Launching ZenOmen a second time focuses the existing window via D-Bus session activation (`com.zenomen.App` `PresentWindow`), providing a reliable fallback on desktop environments like stock GNOME where tray icons are not displayed.
  3. Settings — theme selection (Dark), polling intervals clamped to safe bounds, start-minimized, autostart — survive a restart; a corrupt `config.toml` loads safe defaults, preserves original file for troubleshooting and informs user.
  4. Existing `~/.config/openomen/rgb/*.json` profiles are safely migrated without deleting or overwriting source files; valid `#RRGGBB` zone colors are imported; unsupported animation/timing fields are identified without failing.
  5. An invalid color value produces a clear error message, not a traceback; RGB writes are verified where hardware supports read-back.
  6. The app is fully reachable across GNOME, KDE, and X11 environments.

**Plans**: 3 plans

Plans:
- [ ] 04-01: Tray, autostart, close-to-tray and single-instance handling with D-Bus activation GNOME fallback
- [ ] 04-02: Configuration persistence — TOML schema, validation, atomic writes, theme and interval settings
- [ ] 04-03: Basic RGB — capability detection, presets, on/off, strict `#RRGGBB` validation, safe legacy JSON profile migration

### Phase 5: Distribution, Resource Budget & Rebrand

**Goal**: ZenOmen is installable, measurable and honest — packaged for Linux, released automatically with an audit gate, proven inside its resource budget, and presented as its own project.
**Mode:** mvp
**Depends on**: Phase 4
**Requirements**: REL-01, REL-02, REL-03, REL-04, REL-05, REL-06, REL-07, REL-08, PERF-01, PERF-02, PERF-04, PERF-05, PERF-06
**Success Criteria** (what must be TRUE):
  1. The DEB installs on a clean Ubuntu-based system and launches from the application menu; the AppImage runs on a machine with no build toolchain, and both install the desktop entry, systemd unit, D-Bus policy, and polkit rules.
  2. Tagging a release publishes the DEB, AppImage and checksum file automatically, and CI blocks on a clippy warning, failing test or dependency advisory (`cargo audit`).
  3. A two-hour tray session stays at or below 40 MB peak RSS for the UI process with flat memory, idle CPU stays at or below 0.1% on dashboard, and hardware service stays ≤15 MB RSS (reported separately).
  4. Removing a sensor, an external tool or the hardware service one at a time never crashes the app; each absence degrades to an explained state.
  5. The README presents ZenOmen with GPL-3.0, upstream attribution and the HP disclaimer intact, the Python `openomen` CLI is removed after parity verification, and "Check for updates" opens the Releases page without downloading anything.

**Plans**: 3 plans

Plans:
- [ ] 05-01: Packaging — DEB, AppImage, desktop entry, systemd unit, D-Bus/polkit policy, CI/release workflows with checksums and audit gate
- [ ] 05-02: Resource budget measurement — empirical cold start, warm dashboard, minimized tray, and peak RSS profiling, degradation matrix
- [ ] 05-03: Rebrand and cleanup — README/docs, update link, remove the Python CLI after full parity verification

## Progress

**Execution Order:**
Phases execute in numeric order: 1 → 2 → 3 → 4 → 5

| Phase | Plans Complete | Status | Completed |
|-------|----------------|--------|-----------|
| 1. Foundation | 3/3 | Complete | 2026-11-01 |
| 2. Linux Telemetry | 3/3 | Complete | 2026-11-01 |
| 3. Verified Control | 3/3 | Complete | 2026-11-02 |
| 4. Desktop Integration | 0/3 | Planned | - |
| 5. Distribution | 0/3 | Not started | - |
