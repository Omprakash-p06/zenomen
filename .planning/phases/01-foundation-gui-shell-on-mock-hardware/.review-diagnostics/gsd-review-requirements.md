- [ ] **FOUND-01**: User can launch ZenOmen and see the Overview page rendered in the ZenOmen dark theme
- [ ] **FOUND-02**: Every screen (Overview, Cooling, Performance, Lighting, Settings, Diagnostics) is reachable from the sidebar and by keyboard navigation
- [ ] **FOUND-03**: The application runs end-to-end against a mock hardware provider, so the full UI is usable and testable without the target laptop
- [ ] **FOUND-04**: On a machine where no hardware capability is detected, the app still launches and explains what is unsupported instead of erroring or showing blank panels
- [ ] **FOUND-05**: UI state changes only in response to typed core events (`AppEvent`); no hardware file or external command is ever read from the UI layer

### Monitoring

- [ ] **MON-01**: User can see the CPU model and aggregate utilization (calculated via previous-tick deltas on a worker thread, never blocking the UI)
- [ ] **MON-02**: User can see per-core CPU utilization
- [ ] **MON-03**: User can see current CPU frequency
- [ ] **MON-04**: User can see CPU temperature, chosen by hwmon driver and label priority rather than by channel index
- [ ] **MON-05**: User can see GPU name, utilization and temperature for supported NVIDIA, AMD and Intel hardware (with dynamically enumerated DRM paths, never hardcoding `card0`)
- [ ] **MON-06**: User can see GPU VRAM usage and power draw when the provider exposes them
- [ ] **MON-07**: User can see total, available and used system RAM, computed from `/proc/meminfo` via `MemAvailable` math (a first-time implementation, absent in the legacy CLI)
- [ ] **MON-08**: Any absent sensor or control displays an explicit "Unavailable" state with a reason — never a substituted `0`
- [ ] **MON-09**: User can see rolling telemetry charts fed from the bounded history buffer (120 samples)
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
--
| FOUND-01 | Phase 1 | Pending |
| FOUND-02 | Phase 1 | Pending |
| FOUND-03 | Phase 1 | Pending |
| FOUND-04 | Phase 1 | Pending |
| FOUND-05 | Phase 1 | Pending |
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
