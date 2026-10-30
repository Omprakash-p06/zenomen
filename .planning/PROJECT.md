# ZenOmen

## What This Is

ZenOmen is a lightweight, cross-platform hardware control center for HP OMEN and Victus laptops. It is a Rust + Slint desktop application that replaces the Python `openomen` CLI (currently in this repository) with a graphical interface for the same hardware — monitoring CPU, GPU, RAM, temperatures and fan speeds, and managing power profiles, fan modes and keyboard RGB.

It is for Linux users who don't want to dual-boot just to control their own hardware, and it is built to feel like a quiet, premium system utility rather than a generic RGB control panel.

**Differentiating Focus:** Unlike Electron-based or heavy alternatives, ZenOmen competes on measured low memory consumption (≤40 MB peak RSS), instant startup, reliable capability-based hardware detection, verified hardware operations, clean visual design, minimal background activity, and predictable Linux desktop integration.

## Core Value

Hardware control you can trust: every write to hardware is verified by reading the state back, and a sensor that isn't present reads "Unavailable" rather than a fabricated number. If everything else fails, this must work.

## Requirements

### Validated

Proven valuable in the existing Python OpenOMEN CLI (`openomen`, v0.4.2) — these define the capability reference ZenOmen must reach, not an architectural baseline:

- ✓ Fan mode control (maximum / automatic) over the hp-wmi `pwm1_enable` interface, with read-back verification — existing
- ✓ Fan RPM reading (`fan1_input` / `fan2_input`) — existing
- ✓ CPU model, per-core utilization and live frequency — existing
- ✓ CPU temperature with driver- and label-priority sensor selection — existing
- ✓ GPU telemetry: NVIDIA via `nvidia-smi`, AMD and Intel via DRM sysfs, dispatch on vendor ID — existing
- ✓ ACPI power profile read, write and dynamic choice discovery — existing
- ✓ Combined presets mapping one name to an ACPI profile + fan mode (gaming / balanced / quiet) — existing
- ✓ Keyboard RGB over three hardware variants (four-zone `rgb_zones`, legacy `keyboardleds`, patched `zone_colors`), enable/disable, and JSON profile persistence under `~/.config/openomen/rgb/` — existing
- ✓ `doctor` hardware compatibility checklist and `info` diagnostic dump for bug reports — existing
- ✓ Distribution as a self-updating single-file binary attached to GitHub Releases; 8 confirmed HP OMEN / Victus devices — existing
- ✓ Combined population of CPU, GPU, temperature and fan readings into one dashboard (`stats`) — existing
- *(Note: The legacy CLI had NO RAM telemetry implementation; RAM is a first-time implementation in ZenOmen)*

### Active

v1 scope: the complete Linux GUI product.

- [ ] Rust + Slint desktop application with the full GUI: Overview, Cooling, Performance, Lighting, Settings, Diagnostics
- [ ] Capability-based hardware detection with an explicit "unsupported" state for every absent sensor or control
- [ ] CPU utilization (aggregate and per-core deltas without sleeps), frequency, temperature with historical telemetry charts
- [ ] GPU model, utilization, temperature, VRAM and power draw for supported NVIDIA / AMD / Intel hardware (dynamically discovered DRM paths, no `card0` hardcoding)
- [ ] System RAM (total, available, used) computed from `/proc/meminfo` via `MemAvailable` math — a first-time implementation
- [ ] Fan RPM and current fan mode read; supported fan modes (Auto/Max) applied and verified by read-back; unsupported manual mode not exposed merely because sysfs defines numeric 1
- [ ] ACPI power profiles discovered dynamically, applied only when advertised, and verified after the write
- [ ] Combined presets (Quiet / Balanced / Gaming) executed with granular per-subsystem reporting (`PresetResult`: power_applied, fan_applied, partial_success) instead of claiming atomic transactions or false success
- [ ] Basic keyboard RGB: capability detection, apply existing preset profiles, enable/disable lighting; migrate existing RGB profile JSON while preserving original files, validating `#RRGGBB` zone colors, and clearly identifying unsupported animation/timing fields without failing
- [ ] Restricted D-Bus system service (`com.zenomen.Hardware1`) owning all privileged hardware writes, with mandatory polkit authorization for every mutating operation
- [ ] System tray (open dashboard, current profile, diagnostics, launch-at-login, exit), autostart at login, and close-to-tray preference
- [ ] Single-instance enforcement with D-Bus window activation (`com.zenomen.App`), ensuring the window can always be focused even on desktop environments without a visible tray (e.g. stock GNOME)
- [ ] TOML configuration persistence with validated loading, safe bounds clamping, and safe defaults preserving any corrupt file
- [ ] Diagnostics export: version, OS/kernel, hardware identifiers, detected sensors from the shared registry, recent telemetry and errors
- [ ] Bounded telemetry history (fixed-capacity ring buffer of 120 samples) with strictly bounded memory
- [ ] Measured resource budget: ≤40 MB peak RSS for the desktop UI process (distinguishing cold start, warm dashboard, minimized tray, and peak RSS) and ≤0.1% idle CPU; hardware service has a separate budget (≤15 MB RSS) and is reported separately, never summed into the UI number
- [ ] Linux distribution: DEB and AppImage packages with desktop entry, systemd hardware service, D-Bus policy, and polkit rules
- [ ] Automated CI/CD: format, clippy (-D warnings), tests, dependency audit (`cargo audit`), and packaging on GitHub Actions, with releases on tags and published checksums
- [ ] A "Check for updates" link that opens the GitHub Releases page (no download, no self-modification)
- [ ] Repository rebranded to ZenOmen (README, docs) with GPL-3.0 notices and HP trademark disclaimer preserved
- [ ] Python `openomen` CLI removed from the repository once Rust parity is verified

### Out of Scope

- Manual fan curves (PWM curves, daemon driving them) — deliberately excluded from v1; unsafe to expose before the verified control path is proven, and requires a running process policy that doesn't exist yet
- Full RGB editor and advanced effects (zone-by-zone color picker, brightness/speed sliders, hardware animations) — v1 ships presets and on/off only
- Automated thermal optimization / predictive fan control — depends on safe fan curves and validated thermal models
- Overclocking and undervolting — hardware risk far outside a verified-write control model
- Cloud accounts, sync and telemetry upload — the product is local-first; nothing leaves the machine by default
- Windows hardware control — architecture stays platform-ready, but Windows fan/RGB/EC support needs its own interface research and validation; not advertised until validated
- In-app self-update (download, verify, install) — v1 defers to DEB/AppImage and installer flows; eliminates the root-execution and rollback complexity of an updater

## Context

**Current repository state:** a fork of OpenOMEN (`Omprakash-p06/zenomen`) containing only the Python implementation — a single extensionless 1703-line script `openomen`, `openomen.spec` (PyInstaller), `how-to-release.txt`, `README.md` and the GPL-3.0 `LICENSE`. There is no `Cargo.toml`, no Rust code, no tests, no CI, no `.gitignore` and no dependency manifest.

**Behavioral reference vs. architectural reference:**
The Python implementation is the hardware-behaviour reference, not the architectural reference. ZenOmen reproduces its validated capabilities, but emphatically NOT its bugs, synchronous design, or security mistakes:
- The legacy CLI blocks for 500 ms in `read_cpu_stats()` — Rust core uses previous-tick counter deltas on a worker thread.
- The legacy CLI loops synchronously in `monitor_stats_loop()` — Rust uses a deadline-scheduled sampling thread and compiled Slint event-driven rendering.
- The legacy CLI has NO RAM telemetry — ZenOmen implements proper `/proc/meminfo` parsing (`MemAvailable`) for the first time.
- The legacy CLI does not verify power profile writes — ZenOmen verifies every ACPI write with read-back.
- The legacy CLI returns boolean success on combined presets — ZenOmen returns structured `PresetResult` with granular per-subsystem reporting.
- The legacy CLI duplicates sensor selection in `read_temperature()` and `run_doctor()` — ZenOmen uses a single unified `SensorRegistry`.
- The legacy CLI hardcodes `card0` for Intel GPU frequency — ZenOmen dynamically discovers DRM device paths.
- The legacy CLI stores RGB `brightness`, `speed`, `length_ms`, `change`, `loop` without applying them, and returns success even if writes fail — ZenOmen validates `#RRGGBB`, migrates static colors, flags unsupported fields honestly, and verifies writes.
- The legacy CLI updater disables TLS verification and executes root binaries — ZenOmen uses package-manager distribution and outbound release links.
- The legacy CLI re-executes itself via `sudo` — ZenOmen uses a narrow D-Bus service with mandatory polkit authorization.

**Hardware environment:** HP OMEN / Victus laptops exposing hp-wmi; firmware varies by model, so capability detection must be dynamic (`HardwareCapabilities`), not tied to arbitrary kernel version checks. Eight devices are confirmed working in the existing project; no universal compatibility is claimed beyond that.

**Product direction:** Japanese minimalism plus dark gaming aesthetics (Enso-inspired identity, obsidian/charcoal surfaces, crimson and muted-gold accents used sparingly, Inter + JetBrains Mono, status conveyed by more than color alone).

## Constraints

- **Tech stack**: Rust (edition 2024) + Slint 1.17 for a compiled declarative GUI; TOML + Serde for configuration; `tracing` for structured logs; Cargo Packager for DEB/AppImage — dependencies stay deliberately small rather than pulling in a large framework.
- **Architecture**: Two-process application architecture with three logical layers:
  1. Desktop UI process (`zenomen-gui`): Slint UI + application core, running unprivileged as the desktop user.
  2. Privileged hardware service process (`zenomen-hw`): system D-Bus daemon running as root under systemd sandboxing.
  3. Logical interface layer: Linux kernel sysfs and procfs interfaces (`/sys/class/hwmon`, `/sys/firmware/acpi`, `/sys/class/drm`, `/proc`). The kernel is an interface layer, not an application process.
- **Performance**: ≤40 MB peak RSS for the desktop UI process and ≤0.1% idle CPU. The hardware service has its own separate budget (≤15 MB RSS) and is reported separately, never summed into the UI number. Memory distinguishes cold startup, warm dashboard, minimized tray, and peak load.
- **Performance method**: CPU sampling must not block the UI thread; one long-lived monitoring worker instead of a thread per sensor; bounded `VecDeque` history (120 samples ≈ 2 minutes at 1 Hz).
- **Security**: least privilege — the GUI runs as a normal user, read-only telemetry needs no root, all privileged writes go through the restricted D-Bus service (`com.zenomen.Hardware1`), which enforces mandatory polkit authorization and accepts only predefined operations, never arbitrary paths or shell commands.
- **Privacy**: local-first; no account, no database server, no cloud connection, no telemetry uploaded by default.
- **Compatibility**: Linux first (Ubuntu-based and Arch-family validated), capability-detected rather than kernel-version-gated; Windows kept architecture-ready but unadvertised until its interfaces are validated.
- **Licensing**: GPL-3.0 inherited from OpenOMEN — original copyright notices preserved, modified sources marked, license shipped with distributed binaries, HP trademark disclaimer retained, no implied HP endorsement.
- **Versioning**: semantic versioning; v0.1.0 is the foundation, 1.0.0 is the first stable release.
- **No network dependency**: core functionality works fully offline.

## Key Decisions

| Decision | Rationale | Outcome |
|----------|-----------|---------|
| Rewrite in Rust + Slint rather than extend the Python CLI | The 40 MB / 0.1% budget, a real GUI, tray integration and native packaging are not reachable from a single self-updating Python script | — Confirmed |
| GUI-only product; no Rust CLI surface | The CLI is being replaced rather than ported; scripting use is secondary to the desktop experience | — Confirmed |
| Python `openomen` stays in the repo until Rust parity, then is deleted | Keeps a working tool for existing users and a behavioral reference during the rewrite | — Confirmed |
| Two-process architecture with three logical layers | UI/core in unprivileged process (≤40 MB RSS); hardware service in separate root daemon (≤15 MB RSS); kernel interfaces underneath (not an application process) | — Confirmed |
| `HardwareProvider` trait with `RgbProvider` as a separate optional capability | Capability discovery over assumption: not every OMEN/Victus supports every feature, and RGB is genuinely optional | — Confirmed |
| UI never touches sysfs or invokes privileged commands | Keeps the hardware layer independent of the UI so the interface can evolve without risking hardware behavior | — Confirmed |
| Privileged writes behind a restricted D-Bus service (`com.zenomen.Hardware1`) with mandatory polkit | Avoids running the GUI as root and replaces OpenOMEN's re-exec-under-sudo model; mandatory polkit for all mutating methods | — Confirmed |
| Every hardware write verified by read-back; failures reported, never assumed | Trust is the core value; silent success on a failed write is the worst failure mode | — Confirmed |
| Single shared `SensorRegistry` for dashboard and diagnostics | Prevents sensor selection drift (which plagued legacy `read_temperature` vs `run_doctor`) | — Confirmed |
| Combined presets report granular `PresetResult` | ACPI power and fan mode are not atomic; granular reporting replaces false claims of transactional rollback | — Confirmed |
| Missing sensor displays "Unavailable", never zero | A fabricated reading is worse than an honest gap, and zero is indistinguishable from a real value | — Confirmed |
| No manual fan curves in v1 | Exposing arbitrary PWM before the verified control path is proven is a hardware-safety risk | — Confirmed |
| Basic RGB with strict migration of legacy JSON profiles | Preserves original JSON files; validates `#RRGGBB` zone colors; identifies unsupported animation fields without failing | — Confirmed |
| Single-instance activation via D-Bus (`com.zenomen.App`) | Guarantees window can always be focused when reopened, serving as an essential fallback on GNOME where tray icons are absent | — Confirmed |
| Bounded `VecDeque` telemetry history instead of unbounded growth | Predictable memory is a product requirement, not an optimization | — Confirmed |
| No in-app self-update in v1; a link to Releases instead | Package-manager updates are simpler and safer than re-implementing download-vs-install-vs-rollback in the app | — Confirmed |
| Fork preserves GPL-3.0 and upstream notices | Required by the license and by the community relationship with the original project | — Confirmed |
| Windows designed for but not shipped in v1 | Linux is the validated platform; Windows lacks a universal fan-control API, so its support needs separate research | — Confirmed |

## Evolution

This document evolves at phase transitions and milestone boundaries.

**After each phase transition**:
1. Requirements invalidated? → Move to Out of Scope with reason
2. Requirements validated? → Move to Validated with phase reference
3. New requirements emerged? → Add to Active
4. Decisions to log? → Add to Key Decisions
5. "What This Is" still accurate? → Update if drifted

**After each milestone**:
1. Full review of all sections
2. Core Value check — still the right priority?
3. Audit Out of Scope — reasons still valid?
4. Update Context with current state (confirmed devices, resource measurements, feedback)

---
*Last updated: 2026-10-30 after architectural and planning review corrections*
