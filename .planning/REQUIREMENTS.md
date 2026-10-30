# Requirements: ZenOmen

**Defined:** 2026-10-30
**Core Value:** Hardware control you can trust — every write verified by read-back, and a missing sensor reads "Unavailable" rather than a fabricated number.

## v1 Requirements

Requirements for the Linux GUI MVP. Each maps to exactly one roadmap phase.

### Foundation

- [ ] **FOUND-01**: User can launch ZenOmen and see the Overview page rendered in the ZenOmen dark theme
- [ ] **FOUND-02**: Every screen (Overview, Cooling, Performance, Lighting, Settings, Diagnostics) is reachable from the sidebar and by keyboard navigation
- [ ] **FOUND-03**: The application runs end-to-end against a mock hardware provider, so the full UI is usable and testable without the target laptop
- [ ] **FOUND-04**: On a machine where no hardware capability is detected, the app still launches and explains what is unsupported instead of erroring or showing blank panels
- [ ] **FOUND-05**: UI state changes only in response to typed core events; no hardware file or external command is ever read from the UI layer

### Monitoring

- [ ] **MON-01**: User can see the CPU model and aggregate utilization
- [ ] **MON-02**: User can see per-core CPU utilization
- [ ] **MON-03**: User can see current CPU frequency
- [ ] **MON-04**: User can see CPU temperature, chosen by hwmon driver and label priority rather than by channel index
- [ ] **MON-05**: User can see GPU name, utilization and temperature for supported NVIDIA, AMD and Intel hardware
- [ ] **MON-06**: User can see GPU VRAM usage and power draw when the provider exposes them
- [ ] **MON-07**: User can see total, available and used system RAM, computed from `MemAvailable` rather than `MemFree`
- [ ] **MON-08**: Any absent sensor or control displays an explicit "Unavailable" state with a reason — never a substituted `0`
- [ ] **MON-09**: User can see rolling telemetry charts fed from the bounded history buffer
- [ ] **MON-10**: User can manually refresh telemetry on demand
- [ ] **MON-11**: User can configure the telemetry history window length
- [ ] **MON-12**: The dashboard and diagnostics agree on which sensor represents which component (one shared selection helper, no duplicated heuristics)

### Cooling

- [ ] **COOL-01**: User can see the RPM of each detected fan, with an unavailable state when a fan sensor is absent
- [ ] **COOL-02**: User can see the current fan mode, distinguishing automatic, maximum and manual rather than collapsing unknown values
- [ ] **COOL-03**: User can switch between the fan modes the hardware actually advertises
- [ ] **COOL-04**: Every fan-mode write is verified by reading the state back, and a mismatch is reported to the user instead of shown as success
- [ ] **COOL-05**: Fan controls are hidden or disabled with a plain-language explanation when the interface is missing or not writable on this model
- [ ] **COOL-06**: User is not required to grant privilege or change fan behaviour to view telemetry

### Performance

- [ ] **PWR-01**: User can see the ACPI platform profiles this machine advertises, read dynamically from `platform_profile_choices`
- [ ] **PWR-02**: User can see the currently active power profile
- [ ] **PWR-03**: User can apply an advertised power profile
- [ ] **PWR-04**: The applied power profile is re-read after the write and the observed value is shown
- [ ] **PWR-05**: User can apply a combined preset (Quiet, Balanced, Gaming) that sets the ACPI profile and fan mode together
- [ ] **PWR-06**: When only part of a combined preset can be applied, the user is told exactly which part succeeded and which did not

### Lighting

- [ ] **RGB-01**: User can see whether keyboard RGB is supported, based on detection of the hardware interface variant
- [ ] **RGB-02**: User can see the currently active RGB profile
- [ ] **RGB-03**: User can apply an existing RGB preset profile to the keyboard
- [ ] **RGB-04**: User can enable and disable keyboard lighting
- [ ] **RGB-05**: Existing OpenOMEN RGB profiles in `~/.config/openomen/rgb/` are migrated without discarding any file that cannot be parsed
- [ ] **RGB-06**: An invalid color value produces a clear error message instead of an uncaught traceback
- [ ] **RGB-07**: Profile fields the hardware cannot honour are either implemented or absent from the schema, never silently ignored

### Hardware Service

- [ ] **SVC-01**: The GUI process runs as the normal user — no sudo re-exec, no setuid, no root GUI
- [ ] **SVC-02**: All privileged hardware writes go through the `com.zenomen.Hardware1` D-Bus system service
- [ ] **SVC-03**: Each privileged action is authorized through polkit for the calling user
- [ ] **SVC-04**: The service accepts only predefined operations with whitelisted or schema-validated parameters, and rejects anything else with a coded error
- [ ] **SVC-05**: The service runs under a systemd unit with filesystem access limited to the sysfs paths it needs
- [ ] **SVC-06**: The GUI stays fully usable for telemetry when the hardware service is unavailable, with controls disabled and the reason explained

### System Integration

- [ ] **SYS-01**: User can open the dashboard, see the current profile, open diagnostics, toggle launch at login and quit from a system tray menu
- [ ] **SYS-02**: User can enable or disable starting ZenOmen at login
- [ ] **SYS-03**: User can choose to start minimized
- [ ] **SYS-04**: Closing the window respects the close-to-tray preference, and quitting is a separate explicit action
- [ ] **SYS-05**: Only one instance runs; launching again focuses the existing window rather than starting a second copy
- [ ] **SYS-06**: User settings persist across restarts in `config.toml`
- [ ] **SYS-07**: A corrupt or invalid config loads safe defaults, preserves the original file for troubleshooting and tells the user
- [ ] **SYS-08**: User can select a theme
- [ ] **SYS-09**: User can configure monitoring intervals per metric, clamped to safe bounds
- [ ] **SYS-10**: The app remains fully reachable when no system tray is available (for example stock GNOME without the AppIndicator extension)

### Diagnostics

- [ ] **DIAG-01**: User can see a detection report covering app version, OS, kernel, hardware identifiers, detected sensors and per-capability support status
- [ ] **DIAG-02**: The report includes recent errors and recent telemetry readings
- [ ] **DIAG-03**: User can export diagnostics to a file in one action, containing no unrelated personal data
- [ ] **DIAG-04**: User can see whether the hardware service is connected and whether permissions are sufficient

### Distribution

- [ ] **REL-01**: A DEB package installs on a clean Ubuntu-based system and the app launches from the application menu
- [ ] **REL-02**: An AppImage runs on a machine that has none of the build toolchain
- [ ] **REL-03**: Packages install the desktop entry, the systemd unit, and the D-Bus and polkit policy files in their correct locations
- [ ] **REL-04**: CI runs formatting, clippy (warnings as errors), tests and a dependency advisory audit, failing the build on any violation
- [ ] **REL-05**: Tagging a release publishes the DEB, the AppImage and a checksum file automatically
- [ ] **REL-06**: A "Check for updates" action opens the GitHub Releases page without downloading or executing anything
- [ ] **REL-07**: The repository is presented as ZenOmen, with GPL-3.0 notices, upstream attribution and the HP trademark disclaimer preserved
- [ ] **REL-08**: The Python `openomen` CLI is removed from the repository once Rust parity is verified

### Resource & Reliability

- [ ] **PERF-01**: Peak RSS of the desktop process stays at or below 40 MB over a two-hour tray session
- [ ] **PERF-02**: Idle CPU stays at or below 0.1% of one core while the dashboard is open
- [ ] **PERF-03**: Hardware sampling never blocks the UI thread; window frame time stays stable while telemetry refreshes
- [ ] **PERF-04**: Telemetry memory is bounded and RSS stays flat across long sessions rather than growing
- [ ] **PERF-05**: No combination of missing sensors, missing external tools or missing service causes a crash — every absence degrades gracefully
- [ ] **PERF-06**: The service process has its own measured budget, reported separately from the UI process and never summed into the 40 MB figure

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
| _(filled by roadmap)_ | — | — |

**Coverage:**
- v1 requirements: 70 total
- Mapped to phases: pending
- Unmapped: pending

---
*Requirements defined: 2026-10-30*
*Last updated: 2026-10-30 after initial definition*
