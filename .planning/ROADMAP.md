# Roadmap: ZenOmen

## Overview

ZenOmen replaces the Python OpenOMEN CLI with a Rust + Slint desktop application, built in five phases that each end in something you can actually run. Phase 1 puts a working GUI on mock hardware so the UI, the worker and the resource instrumentation exist before any hardware code does. Phase 2 makes it real for reading — telemetry, capability detection and diagnostics — which requires no privilege and is where the app first becomes useful on your laptop. Phase 3 is the trust-critical one: the privileged D-Bus service, verified fan and power writes, and combined presets, deliberately isolated so the security boundary and its verification can be reviewed as a unit. Phase 4 turns it into a desktop application — tray, autostart, single instance, persisted settings, and basic RGB with migration of existing profiles. Phase 5 makes it shippable: DEB and AppImage, an audit-gated release pipeline, a measured resource budget, the ZenOmen rebrand, and removal of the Python CLI once parity is proven.

## Phases

**Phase Numbering:**
- Integer phases (1, 2, 3): Planned milestone work
- Decimal phases (2.1, 2.2): Urgent insertions (marked with INSERTED)

Decimal phases appear between their surrounding integers in numeric order.

- [ ] **Phase 1: Foundation — GUI Shell on Mock Hardware** - Rust + Slint app with all six screens, theme, worker and instrumentation, running end-to-end on a mock provider
- [ ] **Phase 2: Linux Telemetry & Capability Detection** - Real CPU/GPU/RAM/thermal readings, capability-aware UI, charts and diagnostics export
- [ ] **Phase 3: Verified Hardware Control** - Privileged D-Bus service, verified fan and ACPI writes, combined presets
- [ ] **Phase 4: Desktop Integration, Config & Basic RGB** - Tray, autostart, single instance, persisted settings, RGB presets with legacy migration
- [ ] **Phase 5: Distribution, Resource Budget & Rebrand** - DEB/AppImage, audit-gated releases, measured budget, ZenOmen rebrand, Python CLI removed

## Phase Details

### Phase 1: Foundation — GUI Shell on Mock Hardware
**Goal**: A runnable ZenOmen application whose entire UI is driven by mock hardware data, with the resource instrumentation needed to hold the 40 MB budget later.
**Mode:** mvp
**Depends on**: Nothing (first phase)
**Requirements**: FOUND-01, FOUND-02, FOUND-03, FOUND-04, FOUND-05, MON-09, PERF-03
**Success Criteria** (what must be TRUE):
  1. Launching the app opens the Overview page in the ZenOmen dark theme, and all six screens are reachable by sidebar click and by keyboard alone.
  2. The Overview shows plausible CPU, GPU, RAM, fan and power readings sourced entirely from the mock provider, including a rolling chart fed by a fixed-capacity history buffer.
  3. Running the app with a mock provider configured to report zero capabilities still launches and explains what is unsupported, with no panic and no blank panels.
  4. Window frame time stays stable while the monitoring worker publishes updates — no hardware read or external command is reachable from a UI callback.
  5. CI fails on a formatting violation, a clippy warning, a failing test, or a dependency advisory.
**Plans**: 3 plans

Plans:
- [ ] 01-01: Rust + Slint scaffold, build pipeline, theme tokens and CI gates
- [ ] 01-02: Application shell — navigation, six screens, Overview bound to core events
- [ ] 01-03: Mock hardware provider, monitoring worker, bounded history and benchmark instrumentation

### Phase 2: Linux Telemetry & Capability Detection
**Goal**: ZenOmen shows real readings from this machine, knows exactly which capabilities it has, and can produce a diagnostics export.
**Mode:** mvp
**Depends on**: Phase 1
**Requirements**: MON-01, MON-02, MON-03, MON-04, MON-05, MON-06, MON-07, MON-08, MON-10, MON-11, MON-12, DIAG-01, DIAG-02
**Success Criteria** (what must be TRUE):
  1. On a supported OMEN/Victus machine the Overview shows real CPU utilization (aggregate and per-core), frequency, temperature, GPU readings, RAM and fan RPM that match what the kernel reports for the same moment.
  2. CPU temperature is selected by hwmon driver and label, and the dashboard and diagnostics name the same sensor.
  3. On this machine, a component that is genuinely absent (for example no NVIDIA GPU, or a fan sensor that is not exposed) shows "Unavailable" with a reason — never `0`.
  4. Charts accumulate from the live stream and the history window can be changed in the UI without a restart; manual refresh updates the dashboard on demand.
  5. A diagnostics export written to a file contains version, OS/kernel, hardware identifiers, detected sensors, capability status and recent errors.
**Plans**: 3 plans

Plans:
- [ ] 02-01: Linux provider — procfs, hwmon, DRM and NVIDIA paths behind `HardwareProvider`
- [ ] 02-02: Capability detection plus the single shared sensor-selection helper used by every consumer
- [ ] 02-03: Live charts, refresh and history controls, diagnostics detection report and export

### Phase 3: Verified Hardware Control
**Goal**: Fan modes and power profiles can be changed safely — through a privileged service, with every write verified by read-back and honest reporting of what actually changed.
**Mode:** mvp
**Depends on**: Phase 2
**Requirements**: COOL-01, COOL-02, COOL-03, COOL-04, COOL-05, COOL-06, PWR-01, PWR-02, PWR-03, PWR-04, PWR-05, PWR-06, SVC-01, SVC-02, SVC-03, SVC-04, SVC-05, SVC-06, DIAG-04
**Success Criteria** (what must be TRUE):
  1. Changing the fan mode changes the mode on the machine, and the UI shows the value read back from hardware rather than the requested one.
  2. A write that does not stick is reported as a failure with an explanation — never as success.
  3. The GUI process runs unprivileged: no sudo, no setuid, and the privileged work appears only in the `zenomen-hw` service.
  4. The service rejects a parameter outside its whitelist with a coded error, and diagnostics show service connection and permission status.
  5. Only profiles and fan modes the machine advertises are offered; unavailable controls are explained rather than hidden silently.
  6. Applying a combined preset reports exactly which of its two parts succeeded.
**Plans**: 3 plans

Plans:
- [ ] 03-01: `zenomen-hw` service — D-Bus interface, polkit authorization, systemd sandboxing, whitelisted validation
- [ ] 03-02: Verified fan control — mode read/write, read-back with retry, capability gating, Cooling page
- [ ] 03-03: ACPI power profiles, combined presets with partial-apply reporting, Performance page

### Phase 4: Desktop Integration, Config & Basic RGB
**Goal**: ZenOmen behaves like a real desktop application — it lives in the tray, starts with the session, remembers settings, and can drive keyboard lighting with existing profiles carried over.
**Mode:** mvp
**Depends on**: Phase 3
**Requirements**: RGB-01, RGB-02, RGB-03, RGB-04, RGB-05, RGB-06, RGB-07, SYS-01, SYS-02, SYS-03, SYS-04, SYS-05, SYS-06, SYS-07, SYS-08, SYS-09, SYS-10, DIAG-03
**Success Criteria** (what must be TRUE):
  1. The tray menu opens the dashboard, shows the current profile, opens diagnostics, toggles launch at login and quits; closing the window honours close-to-tray, and quitting is a separate action.
  2. Launching ZenOmen a second time focuses the existing window instead of starting another process.
  3. Settings — theme, intervals, start-minimized, autostart — survive a restart; a deliberately corrupted `config.toml` loads safe defaults, preserves the original and tells the user.
  4. An existing `~/.config/openomen/rgb/` profile set appears in Lighting and can be applied; an unparsable profile file is preserved and reported.
  5. An invalid color value produces a clear error message, not a traceback.
  6. The app is fully reachable on a desktop with no tray support: relaunching brings the window back.
**Plans**: 3 plans

Plans:
- [ ] 04-01: Tray, autostart, close-to-tray and single-instance handling
- [ ] 04-02: Configuration persistence — schema, validation, atomic writes, Settings page
- [ ] 04-03: Lighting — capability detection, presets, on/off, strict validation, legacy profile migration

### Phase 5: Distribution, Resource Budget & Rebrand
**Goal**: ZenOmen is installable, measurable and honest — packaged for Linux, released automatically with an audit gate, proven inside its resource budget, and presented as its own project.
**Mode:** mvp
**Depends on**: Phase 4
**Requirements**: REL-01, REL-02, REL-03, REL-04, REL-05, REL-06, REL-07, REL-08, PERF-01, PERF-02, PERF-04, PERF-05, PERF-06
**Success Criteria** (what must be TRUE):
  1. The DEB installs on a clean Ubuntu-based system and launches from the application menu; the AppImage runs on a machine with no build toolchain, and both install the desktop entry, systemd unit and D-Bus/polkit policy.
  2. Tagging a release publishes the DEB, AppImage and checksum file automatically, and CI blocks a merge on a clippy warning, failing test or new dependency advisory.
  3. A two-hour tray session stays at or below 40 MB peak RSS with flat memory, and idle CPU at or below 0.1% with the dashboard open.
  4. Removing a sensor, an external tool or the hardware service one at a time never crashes the app; each absence degrades to an explained state.
  5. The README presents ZenOmen with GPL-3.0, upstream attribution and the HP disclaimer intact, the Python `openomen` CLI is gone, and "Check for updates" opens the Releases page without downloading anything.
**Plans**: 3 plans

Plans:
- [ ] 05-01: Packaging — DEB, AppImage, desktop entry, systemd unit, D-Bus/polkit policy, CI/release workflows with checksums and audit gate
- [ ] 05-02: Resource budget measurement — long-session RSS and CPU profile runs, fixes for anything over budget, degradation matrix
- [ ] 05-03: Rebrand and cleanup — README/docs, update link, remove the Python CLI after parity verification

## Progress

**Execution Order:**
Phases execute in numeric order: 1 → 2 → 3 → 4 → 5

| Phase | Plans Complete | Status | Completed |
|-------|----------------|--------|-----------|
| 1. Foundation | 0/3 | Not started | - |
| 2. Linux Telemetry | 0/3 | Not started | - |
| 3. Verified Control | 0/3 | Not started | - |
| 4. Desktop Integration | 0/3 | Not started | - |
| 5. Distribution | 0/3 | Not started | - |
