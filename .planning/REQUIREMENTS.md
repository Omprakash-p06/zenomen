# Requirements: ZenOmen

**Defined:** 2026-10-30  
**Core Value:** Hardware control you can trust — every write verified by read-back, and a missing sensor reads "Unavailable" rather than a fabricated number.

## v1 Requirements

Requirements for the Linux GUI MVP. Each maps to exactly one roadmap phase.

### Foundation

- [x] **FOUND-01**: User can launch ZenOmen and see the Overview page rendered in the ZenOmen dark theme
- [x] **FOUND-02**: Every screen (Overview, Cooling, Performance, Lighting, Settings, Diagnostics) is reachable from the sidebar and by keyboard navigation
- [x] **FOUND-03**: The application runs end-to-end against a mock hardware provider, so the full UI is usable and testable without the target laptop
- [x] **FOUND-04**: On a machine where no hardware capability is detected, the app still launches and explains what is unsupported instead of erroring or showing blank panels
- [x] **FOUND-05**: UI state changes only in response to typed core events (`AppEvent`); no hardware file or external command is ever read from the UI layer

### Monitoring

- [ ] **MON-01**: User can see the CPU model and aggregate utilization (calculated via previous-tick deltas on a worker thread, never blocking the UI)
- [ ] **MON-02**: User can see per-core CPU utilization
- [ ] **MON-03**: User can see current CPU frequency
- [ ] **MON-04**: User can see CPU temperature, chosen by hwmon driver and label priority rather than by channel index
- [ ] **MON-05**: User can see GPU name, utilization and temperature for supported NVIDIA, AMD and Intel hardware (with dynamically enumerated DRM paths, never hardcoding `card0`)
- [ ] **MON-06**: User can see GPU VRAM usage and power draw when the provider exposes them
- [ ] **MON-07**: User can see total, available and used system RAM, computed from `/proc/meminfo` via `MemAvailable` math (a first-time implementation, absent in the legacy CLI)
- [ ] **MON-08**: Any absent sensor or control displays an explicit "Unavailable" state with a reason — never a substituted `0`
- [x] **MON-09**: User can see rolling telemetry charts fed from the bounded history buffer (120 samples)
- [ ] **MON-10**: User can manually refresh telemetry on demand
- [ ] **MON-11**: User can configure the telemetry history window length
- [ ] **MON-12**: The dashboard and diagnostics agree on which sensor represents which component through a single shared `SensorRegistry` (no duplicated or diverging heuristics)

### Cooling

- [ ] **COOL-01**: User can see the RPM of each detected fan, with an unavailable state when a fan sensor is absent
- [ ] **COOL-02**: User can see the current fan mode, distinguishing automatic, maximum and manual (if reported by kernel) rather than collapsing unknown values
- [ ] **COOL-03**: User can switch between validated fan modes the hardware actually advertises (Auto and Max for v1; manual mode is not offered merely because sysfs defines numeric `1`)
- [ ] **COOL-04**: Every fan-mode write is verified by reading the state back with retry/backoff, and a mismatch is reported to the user instead of shown as success
- [ ] **COOL-05**: Fan controls are hidden or disabled with a plain-language explanation when the interface is missing or not writable on this model
- [ ] **COOL-06**: User is not required to grant privilege or change fan behaviour to view telemetry

### Performance

- [ ] **PWR-01**: User can see the ACPI platform profiles this machine advertises, read dynamically from `platform_profile_choices`
- [ ] **PWR-02**: User can see the currently active power profile
- [ ] **PWR-03**: User can apply an advertised power profile
- [ ] **PWR-04**: The applied power profile is verified by re-reading the state after the write with retry/backoff and the observed value is shown
- [ ] **PWR-05**: User can apply a combined preset (Quiet, Balanced, Gaming) that applies the ACPI profile and fan mode together
- [ ] **PWR-06**: Combined presets report granular execution status via `PresetResult` (`power_applied`, `fan_applied`, `partial_success`), informing the user exactly which part succeeded and which failed without claiming atomic rollback or false success

### Lighting

- [ ] **RGB-01**: User can see whether keyboard RGB is supported, based on detection of the hardware interface variant (`four_zone`, `legacy`, `kernel_patch`)
- [ ] **RGB-02**: User can see the currently active RGB profile
- [ ] **RGB-03**: User can apply an existing RGB preset profile to the keyboard
- [ ] **RGB-04**: User can enable and disable keyboard lighting
- [ ] **RGB-05**: Existing OpenOMEN RGB profiles in `~/.config/openomen/rgb/` are migrated without deleting, overwriting, or discarding any source file, preserving original files and validating zone colors strictly
- [ ] **RGB-06**: An invalid color value produces a clear error message instead of an uncaught traceback (strict `#RRGGBB` format)
- [ ] **RGB-07**: Profile fields the hardware cannot honour (e.g. animation settings: `brightness`, `speed`, `length_ms`, `change`, `loop`) are clearly identified as unsupported in v1 without failing the import or silently ignoring them
- [ ] **RGB-08**: RGB hardware writes are applied through the privileged service and verified by reading back state wherever the hardware interface supports it

### Hardware Service

- [ ] **SVC-01**: The GUI process runs as the normal user — no sudo re-exec, no setuid, no root GUI
- [ ] **SVC-02**: All privileged hardware writes go through the `com.zenomen.Hardware1` D-Bus system service
- [ ] **SVC-03**: Each privileged mutating action (`SetFanMode`, `SetPowerProfile`, `ApplyRgbProfile`, `SetRgbEnabled`) is authorized through polkit for the calling user (mandatory, not optional)
- [ ] **SVC-04**: The service implements a formalized API contract (`GetCapabilities`, `GetFanStatus`, `SetFanMode`, `GetPowerProfiles`, `GetPowerProfile`, `SetPowerProfile`, `ApplyRgbProfile`, `SetRgbEnabled`) accepting only predefined operations with schema-validated parameters, rejecting arbitrary paths or commands
- [ ] **SVC-05**: The service runs under a systemd unit with filesystem access sandboxed to only the sysfs paths it needs
- [ ] **SVC-06**: The GUI stays fully usable for telemetry when the hardware service is unavailable or disconnected, with controls disabled and the reason explained

### System Integration

- [ ] **SYS-01**: User can open the dashboard, see the current profile, open diagnostics, toggle launch at login and quit from a system tray menu
- [ ] **SYS-02**: User can enable or disable starting ZenOmen at login (via XDG autostart)
- [ ] **SYS-03**: User can choose to start minimized
- [ ] **SYS-04**: Closing the window respects the close-to-tray preference, and quitting is a separate explicit action
- [ ] **SYS-05**: Only one instance runs; launching again focuses the existing window via D-Bus activation (`com.zenomen.App`), providing a reliable fallback on desktop environments like stock GNOME where tray icons are not displayed by default
- [ ] **SYS-06**: User settings persist across restarts in `config.toml`
- [ ] **SYS-07**: A corrupt or invalid config loads safe defaults, preserves the original file for troubleshooting and informs the user
- [ ] **SYS-08**: User can select a theme (ZenOmen Dark)
- [ ] **SYS-09**: User can configure monitoring intervals per metric, clamped to safe bounds
- [ ] **SYS-10**: The app remains fully reachable when no system tray is available (tested on GNOME, KDE and X11)

### Diagnostics

- [ ] **DIAG-01**: User can see a detection report covering app version, OS, kernel, hardware identifiers, detected sensors from the shared registry and per-capability support status
- [ ] **DIAG-02**: The report includes recent errors and recent telemetry readings
- [ ] **DIAG-03**: User can export diagnostics to a file in one action, containing no unrelated personal data
- [ ] **DIAG-04**: User can see whether the hardware service is connected and whether permissions are sufficient

### Distribution

- [x] **REL-01**: A DEB package installs on a clean Ubuntu-based system and the app launches from the application menu
- [x] **REL-02**: An AppImage runs on a machine that has none of the build toolchain
- [x] **REL-03**: Packages install the desktop entry, the systemd unit, and the D-Bus and polkit policy files in their correct locations
- [x] **REL-04**: CI runs formatting, clippy (warnings as errors), tests and a dependency advisory audit (`cargo audit`), failing the build on any violation
- [x] **REL-05**: Tagging a release publishes the DEB, the AppImage and a checksum file automatically
- [x] **REL-06**: A "Check for updates" action opens the GitHub Releases page without downloading or executing anything
- [x] **REL-07**: The repository is presented as ZenOmen, with GPL-3.0 notices, upstream attribution and the HP trademark disclaimer preserved
- [x] **REL-08**: The Python `openomen` CLI is removed from the repository once Rust parity is verified

### Resource & Reliability

- [ ] **PERF-01**: Peak RSS of the desktop UI process stays at or below 40 MB over a two-hour tray session (distinguishing cold start <30 MB, warm dashboard <38 MB, minimized tray <25 MB, and peak load ≤40 MB)
- [x] **PERF-02**: Idle CPU stays at or below 0.1% of one core while the dashboard is open, and ≤0.02% when minimized to tray
- [x] **PERF-03**: Hardware sampling never blocks the UI thread; window frame time stays stable while telemetry refreshes
- [x] **PERF-04**: Telemetry memory is bounded and RSS stays flat across long sessions rather than growing
- [x] **PERF-05**: No combination of missing sensors, missing external tools or missing service causes a crash — every absence degrades gracefully
- [x] **PERF-06**: The hardware service process has its own measured budget (≤15 MB RSS), reported separately from the UI process and never summed into the 40 MB figure

## v2 Requirements

Deferred to a future release. Tracked but not in the current roadmap.

### Updates

- **UPD-01**: User can check for updates with release notes
- **UPD-02**: Update artifacts are verified cryptographically before install
- **UPD-03**: User confirms before an update installs, with download progress

### Cooling

- **COOL-V2-01**: User can define manual fan curves, gated to models with validated writable PWM
- **COOL-V2-02**: Fan mode and RGB state can be restored after reboot and resume

### Lighting

- **RGB-V2-01**: User can edit RGB per zone with a color picker
- **RGB-V2-02**: Brightness, speed, loop and animation fields are honoured by hardware that supports them

### Platform

- **WIN-01**: Windows builds show CPU and RAM telemetry with tray and startup
- **WIN-02**: Windows hardware control for validated OEM interfaces only

### Compatibility

- **COMP-01**: Additional confirmed device models with per-model quirk data
- **COMP-02**: A device capability matrix published from real reports

## Out of Scope

Explicitly excluded from v1, with reasons so they are not silently re-added.

| Feature | Reason |
|---------|--------|
| Manual fan curves and arbitrary PWM values | Writing unverified PWM on an embedded controller can overheat hardware; upstream support only recently landed and is model-gated. Requires the verified control path first |
| Full RGB editor, per-key effects, animations | Large surface area for a schema the hardware cannot express today; presets plus on/off cover real use |
| Automated thermal optimization / predictive fan control | Depends on safe fan curves and validated thermal models; acting on a bad model is worse than doing nothing |
| Overclocking and undervolting | Hardware-damage risk with no safe verification loop |
| In-app self-update (download/verify/install) | Package-manager upgrades are safer; a link to Releases avoids the legacy updater's root-execution and rollback complexity |
| Cloud accounts, sync, telemetry upload | Local-first product; uploading data from a root-privileged hardware tool is a trust hazard |
| Windows hardware control in v1 | No universal Windows laptop fan API; unvalidated OEM interfaces would ship as broken features |
| Background daemon holding fan/PWM state | Unsupervised root process with no safe recovery policy is a thermal risk |
| A Rust CLI surface | The product is a GUI; scripting needs are secondary and unvalidated |

## Traceability

Which phase covers which requirement. Populated during roadmap creation.

| Requirement | Phase | Status |
|-------------|-------|--------|
| FOUND-01 | Phase 1 | Complete |
| FOUND-02 | Phase 1 | Complete |
| FOUND-03 | Phase 1 | Pending |
| FOUND-04 | Phase 1 | Complete |
| FOUND-05 | Phase 1 | Complete |
| MON-09 | Phase 1 | Pending |
| PERF-03 | Phase 1 | Pending |
| MON-01 | Phase 2 | Pending |
| MON-02 | Phase 2 | Pending |
| MON-03 | Phase 2 | Pending |
| MON-04 | Phase 2 | Pending |
| MON-05 | Phase 2 | Pending |
| MON-06 | Phase 2 | Pending |
| MON-07 | Phase 2 | Pending |
| MON-08 | Phase 2 | Pending |
| MON-10 | Phase 2 | Pending |
| MON-11 | Phase 2 | Pending |
| MON-12 | Phase 2 | Pending |
| DIAG-01 | Phase 2 | Pending |
| DIAG-02 | Phase 2 | Pending |
| COOL-01 | Phase 3 | Pending |
| COOL-02 | Phase 3 | Pending |
| COOL-03 | Phase 3 | Pending |
| COOL-04 | Phase 3 | Pending |
| COOL-05 | Phase 3 | Pending |
| COOL-06 | Phase 3 | Pending |
| PWR-01 | Phase 3 | Pending |
| PWR-02 | Phase 3 | Pending |
| PWR-03 | Phase 3 | Pending |
| PWR-04 | Phase 3 | Pending |
| PWR-05 | Phase 3 | Pending |
| PWR-06 | Phase 3 | Pending |
| SVC-01 | Phase 3 | Pending |
| SVC-02 | Phase 3 | Pending |
| SVC-03 | Phase 3 | Pending |
| SVC-04 | Phase 3 | Pending |
| SVC-05 | Phase 3 | Pending |
| SVC-06 | Phase 3 | Pending |
| DIAG-04 | Phase 3 | Pending |
| RGB-01 | Phase 4 | Pending |
| RGB-02 | Phase 4 | Pending |
| RGB-03 | Phase 4 | Pending |
| RGB-04 | Phase 4 | Pending |
| RGB-05 | Phase 4 | Pending |
| RGB-06 | Phase 4 | Pending |
| RGB-07 | Phase 4 | Pending |
| RGB-08 | Phase 4 | Pending |
| SYS-01 | Phase 4 | Pending |
| SYS-02 | Phase 4 | Pending |
| SYS-03 | Phase 4 | Pending |
| SYS-04 | Phase 4 | Pending |
| SYS-05 | Phase 4 | Pending |
| SYS-06 | Phase 4 | Pending |
| SYS-07 | Phase 4 | Pending |
| SYS-08 | Phase 4 | Pending |
| SYS-09 | Phase 4 | Pending |
| SYS-10 | Phase 4 | Pending |
| DIAG-03 | Phase 4 | Pending |
| REL-01 | Phase 5 | Pending |
| REL-02 | Phase 5 | Pending |
| REL-03 | Phase 5 | Pending |
| REL-04 | Phase 5 | Pending |
| REL-05 | Phase 5 | Pending |
| REL-06 | Phase 5 | Pending |
| REL-07 | Phase 5 | Pending |
| REL-08 | Phase 5 | Pending |
| PERF-01 | Phase 5 | Pending |
| PERF-02 | Phase 5 | Pending |
| PERF-04 | Phase 5 | Pending |
| PERF-05 | Phase 5 | Pending |
| PERF-06 | Phase 5 | Pending |

**Coverage:**
- v1 requirements: 71 total
- Mapped to phases: 71
- Unmapped: 0 ✓

---
*Requirements defined: 2026-10-30*  
*Last updated: 2026-10-30 after architectural and planning review corrections*
