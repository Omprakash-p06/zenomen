# Requirements: ZenOmen

**Defined:** 2026-10-30  
**Core Value:** Hardware control you can trust — every write verified by read-back, and a missing sensor reads "Unavailable" rather than a fabricated number.

## v1 Requirements

Requirements for the cross-platform ZenOmen application (Linux + Windows 10/11). Each maps to exactly one roadmap phase.

> **Amendment 2026-11-07 (Phase 6):** the product scope was widened from a Linux GUI MVP to a
> single cross-platform release. Windows telemetry is promoted from v2 into v1; Windows
> firmware control is in scope but ships **disabled** until a verified board profile exists.
> Previously undefined IDs cited by `ROADMAP.md` and the Phase 6 plans (`WIN-03..05`,
> `SEC-01..03`, `ARCH-01`, `TEST-01`) are now defined below. See
> `.planning/phases/06-cross-platform-windows-linux-release/06-CONTEXT.md` for the authority.

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
- [ ] **PERF-07**: On Windows, the GUI process working set stays within the same ≤40 MB budget measured with `GetProcessMemoryInfo` (never compared to Linux `Private_Dirty`), and the service reports its own working set separately.

### Cross-Platform UI (Phase 6)

- [ ] **UI-01**: The production UI is rebuilt in GPUI-CE 0.3.0; Slint source and dependencies are removed only after parity is verified on Linux and Windows.
- [ ] **UI-02**: All six screens (Overview, Cooling, Performance, Lighting, Settings, Diagnostics), the sidebar, status strip, telemetry charts and theme tokens are reproduced with the same information density and dark-theme styling.
- [ ] **UI-03**: UI state still changes only through typed `AppEvent`; the UI framework never owns hardware domain state.
- [ ] **UI-04**: GPUI-CE builds and runs on Linux (X11 and Wayland) and Windows 10/11 (Win32/DirectX), with keyboard navigation and accessible focus order.

### Windows Telemetry (Phase 6)

- [ ] **WIN-01**: Windows provider reports real CPU identity, aggregate and per-core utilization from `GetSystemTimes` deltas, and frequency from `CallNtPowerInformation`.
- [ ] **WIN-02**: Windows provider reports real physical RAM total and used memory from `GlobalMemoryStatusEx`.
- [ ] **WIN-03**: Windows provider enumerates GPU adapters via DXGI, reports dedicated VRAM from `DXGI_ADAPTER_DESC1` and per-adapter utilization from PDH `GPU Engine` counters, filtering software adapters.
- [ ] **WIN-04**: Windows provider reports battery presence, charge percentage, charging state and remaining time from `GetSystemPowerStatus`.
- [ ] **WIN-05**: Windows provider reports CPU/GPU temperature and fan RPM only from a verified source (HP WMI read, vendor sensor API, or an explicitly evaluated adapter); otherwise the metric is `Unavailable` with a reason.
- [ ] **WIN-06**: Windows system identity (manufacturer, model, baseboard product, BIOS version) is read via WMI/SMBIOS and drives board-profile resolution.

### Windows Hardware Service (Phase 6)

- [ ] **WIN-07**: Privileged hardware mutations run in a Windows service registered with the Service Control Manager; the GUI never requires administrator rights.
- [ ] **WIN-08**: GUI↔service IPC uses a versioned, typed, length-bounded protocol over `\\.\pipe\ZenOmen.Hardware.v1`, with an explicit DACL and caller session/user validation.
- [ ] **WIN-09**: HP firmware commands execute only when the board product ID, BIOS generation and firmware read-back match a verified board profile; unknown boards report `fan_control = false` and refuse mutating writes.
- [ ] **WIN-10**: Every Windows hardware write is read back and verified; partial failure is reported honestly and never as success.
- [ ] **WIN-11**: Fan, power, RGB and MUX controls ship disabled until physical-device validation evidence is recorded; the disabled state is explicit in capabilities and UI.

### Windows Desktop & Packaging (Phase 6)

- [ ] **WIN-12**: Native tray (`Shell_NotifyIconW`), single-instance mutex, user-level autostart (HKCU Run), notifications, releases-page opener and per-user/per-service data paths (`SHGetKnownFolderPath`) are implemented.
- [ ] **WIN-13**: A signed x64 installer and uninstaller install and remove the GUI, the hardware service, icons and version resources, with upgrade and rollback behavior.
- [ ] **WIN-14**: Windows 10 22H2 and Windows 11 (24H2/25H2) build and test in CI; hardware-mutation tests are opt-in and never required by default CI.

### Linux Hardening (Phase 6)

- [ ] **SEC-01**: D-Bus activation delegates to `zenomen-hw.service` via `SystemdService=`, so activation never starts an unconfined root process.
- [ ] **SEC-02**: The systemd unit restricts writable paths to exactly the sysfs interfaces the daemon needs, without breaking fan, power or RGB writes.
- [ ] **SEC-03**: The root daemon logs to an operator-readable location (journald or an ACL-protected path) instead of an inaccessible home directory.
- [ ] **SEC-04**: Installer actions validate canonical executable path, ownership and symlink status before any privileged use.
- [ ] **SEC-05**: Privileged helper execution resolves absolute binaries and fails closed instead of trusting `PATH`.
- [ ] **SEC-06**: Polkit allow/deny/challenge behavior is covered by integration tests against a controlled bus.

### Cross-Platform Architecture (Phase 6)

- [ ] **ARCH-01**: Platform selection is compile-time (`cfg(target_os)`); no Linux-only type, zbus reference, sysfs path or systemd code is reachable on Windows, and no Win32/WMI code is reachable on Linux.
- [ ] **ARCH-02**: Shared domain contracts (`TelemetrySnapshot`, `HardwareCapabilities`, `PresetResult`, `AppEvent`) stay UI- and platform-neutral, and every provider conforms to one trait.
- [ ] **ARCH-03**: Capability honesty — a missing sensor, service or firmware command is represented as unavailable with a reason, never zero or a fabricated capability.

### Testing & Quality (Phase 6)

- [ ] **TEST-01**: Default CI is deterministic and host-independent; tests use temp-directory fixtures and mock providers, never live `/sys`, root paths or an HP laptop.
- [ ] **TEST-02**: Hardware-dependent tests are explicitly marked opt-in and documented with the device they require.
- [ ] **TEST-03**: Windows protocol serialization, pipe ACL/session rejection, board-profile rejection, WMI timeout and service lifecycle have coverage runnable on CI.
- [ ] **TEST-04**: Test counts in `README.md`, `TESTING.md` and the codebase maps match actual `#[test]` enumeration.

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

- **COMP-WIN-01**: (Promoted) Windows telemetry, hardware control and desktop integration moved to v1 as `WIN-01`…`WIN-14` on 2026-11-07.
- **COMP-WIN-02**: Additional Windows board profiles with captured read-back evidence and per-model quirks.

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
| Unvalidated Windows firmware writes | No universal Windows laptop fan API; commands execute only behind a verified board profile, and unverified profiles ship with mutating controls disabled |
| Background daemon holding fan/PWM state | Unsupervised root process with no safe recovery policy is a thermal risk |
| A Rust CLI surface | The product is a GUI; scripting needs are secondary and unvalidated |

## Traceability

Which phase covers which requirement. Populated during roadmap creation.

| Requirement | Phase | Status |
|-------------|-------|--------|
| FOUND-01 | Phase 1 | Complete |
| FOUND-02 | Phase 1 | Complete |
| FOUND-03 | Phase 1 | Complete (STATE.md) |
| FOUND-04 | Phase 1 | Complete |
| FOUND-05 | Phase 1 | Complete |
| MON-09 | Phase 1 | Complete (STATE.md) |
| PERF-03 | Phase 1 | Complete (STATE.md) |
| MON-01 | Phase 2 | Complete (STATE.md) |
| MON-02 | Phase 2 | Complete (STATE.md) |
| MON-03 | Phase 2 | Complete (STATE.md) |
| MON-04 | Phase 2 | Complete (STATE.md) |
| MON-05 | Phase 2 | Complete (STATE.md) |
| MON-06 | Phase 2 | Complete (STATE.md) |
| MON-07 | Phase 2 | Complete (STATE.md) |
| MON-08 | Phase 2 | Complete (STATE.md) |
| MON-10 | Phase 2 | Complete (STATE.md) |
| MON-11 | Phase 2 | Complete (STATE.md) |
| MON-12 | Phase 2 | Complete (STATE.md) |
| DIAG-01 | Phase 2 | Complete (STATE.md) |
| DIAG-02 | Phase 2 | Complete (STATE.md) |
| COOL-01 | Phase 3 | Complete (STATE.md) |
| COOL-02 | Phase 3 | Complete (STATE.md) |
| COOL-03 | Phase 3 | Complete (STATE.md) |
| COOL-04 | Phase 3 | Complete (STATE.md) |
| COOL-05 | Phase 3 | Complete (STATE.md) |
| COOL-06 | Phase 3 | Complete (STATE.md) |
| PWR-01 | Phase 3 | Complete (STATE.md) |
| PWR-02 | Phase 3 | Complete (STATE.md) |
| PWR-03 | Phase 3 | Complete (STATE.md) |
| PWR-04 | Phase 3 | Complete (STATE.md) |
| PWR-05 | Phase 3 | Complete (STATE.md) |
| PWR-06 | Phase 3 | Complete (STATE.md) |
| SVC-01 | Phase 3 | Complete (STATE.md) |
| SVC-02 | Phase 3 | Complete (STATE.md) |
| SVC-03 | Phase 3 | Complete (STATE.md) |
| SVC-04 | Phase 3 | Complete (STATE.md) |
| SVC-05 | Phase 3 | Complete (STATE.md) |
| SVC-06 | Phase 3 | Complete (STATE.md) |
| DIAG-04 | Phase 3 | Complete (STATE.md) |
| RGB-01 | Phase 4 | Complete (STATE.md) |
| RGB-02 | Phase 4 | Complete (STATE.md) |
| RGB-03 | Phase 4 | Complete (STATE.md) |
| RGB-04 | Phase 4 | Complete (STATE.md) |
| RGB-05 | Phase 4 | Complete (STATE.md) |
| RGB-06 | Phase 4 | Complete (STATE.md) |
| RGB-07 | Phase 4 | Complete (STATE.md) |
| RGB-08 | Phase 4 | Complete (STATE.md) |
| SYS-01 | Phase 4 | Complete (STATE.md) |
| SYS-02 | Phase 4 | Complete (STATE.md) |
| SYS-03 | Phase 4 | Complete (STATE.md) |
| SYS-04 | Phase 4 | Complete (STATE.md) |
| SYS-05 | Phase 4 | Complete (STATE.md) |
| SYS-06 | Phase 4 | Complete (STATE.md) |
| SYS-07 | Phase 4 | Complete (STATE.md) |
| SYS-08 | Phase 4 | Complete (STATE.md) |
| SYS-09 | Phase 4 | Complete (STATE.md) |
| SYS-10 | Phase 4 | Complete (STATE.md) |
| DIAG-03 | Phase 4 | Complete (STATE.md) |
| REL-01 | Phase 5 | Complete (STATE.md) |
| REL-02 | Phase 5 | Complete (STATE.md) |
| REL-03 | Phase 5 | Complete (STATE.md) |
| REL-04 | Phase 5 | Complete (STATE.md) |
| REL-05 | Phase 5 | Complete (STATE.md) |
| REL-06 | Phase 5 | Complete (STATE.md) |
| REL-07 | Phase 5 | Complete (STATE.md) |
| REL-08 | Phase 5 | Complete (STATE.md) |
| PERF-01 | Phase 5 | Complete (STATE.md) |
| PERF-02 | Phase 5 | Complete (STATE.md) |
| PERF-04 | Phase 5 | Complete (STATE.md) |
| PERF-05 | Phase 5 | Complete (STATE.md) |
| PERF-06 | Phase 5 | Complete (STATE.md) |
| UI-01 | Phase 6 | Planned |
| UI-02 | Phase 6 | Planned |
| UI-03 | Phase 6 | Planned |
| UI-04 | Phase 6 | Planned |
| WIN-01 | Phase 6 | Planned |
| WIN-02 | Phase 6 | Planned |
| WIN-03 | Phase 6 | Planned |
| WIN-04 | Phase 6 | Planned |
| WIN-05 | Phase 6 | Planned |
| WIN-06 | Phase 6 | Planned |
| WIN-07 | Phase 6 | Planned |
| WIN-08 | Phase 6 | Planned |
| WIN-09 | Phase 6 | Planned |
| WIN-10 | Phase 6 | Planned |
| WIN-11 | Phase 6 | Planned |
| WIN-12 | Phase 6 | Planned |
| WIN-13 | Phase 6 | Planned |
| WIN-14 | Phase 6 | Planned |
| SEC-01 | Phase 6 | Planned |
| SEC-02 | Phase 6 | Planned |
| SEC-03 | Phase 6 | Planned |
| SEC-04 | Phase 6 | Planned |
| SEC-05 | Phase 6 | Planned |
| SEC-06 | Phase 6 | Planned |
| ARCH-01 | Phase 6 | Planned |
| ARCH-02 | Phase 6 | Planned |
| ARCH-03 | Phase 6 | Planned |
| TEST-01 | Phase 6 | Planned |
| TEST-02 | Phase 6 | Planned |
| TEST-03 | Phase 6 | Planned |
| TEST-04 | Phase 6 | Planned |
| PERF-07 | Phase 6 | Planned |

**Coverage:**
- v1 requirements: 103 total
- Mapped to phases: 103
- Unmapped: 0 ✓

> **Status-column note (2026-11-07):** the checkbox and status columns were not maintained
> while Phases 1–5 executed. The status column now reads `Complete (STATE.md)` because
> `.planning/STATE.md` records all five phases complete; the `- [ ]`/`- [x]` marks above are
> legacy and must not be used as the source of truth. Phase 6 rows are `Planned` until a
> `06-VERIFICATION.md` exists.

---
*Requirements defined: 2026-10-30*  
*Last updated: 2026-11-07 — Phase 6 amendment: Windows promoted to v1, missing IDs defined, traceability repaired*
