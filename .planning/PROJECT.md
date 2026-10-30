# ZenOmen

## What This Is

ZenOmen is a lightweight, cross-platform hardware control center for HP OMEN and Victus laptops. It is a Rust + Slint desktop application that replaces the Python `openomen` CLI (currently in this repository) with a graphical interface for the same hardware — monitoring CPU, GPU, RAM, temperatures and fan speeds, and managing power profiles, fan modes and keyboard RGB.

It is for Linux users who don't want to dual-boot just to control their own hardware, and it is built to feel like a quiet, premium system utility rather than a generic RGB control panel.

## Core Value

Hardware control you can trust: every write to hardware is verified by reading the state back, and a sensor that isn't present reads "Unavailable" rather than a fabricated number. If everything else fails, this must work.

## Requirements

### Validated

Proven valuable in the existing Python OpenOMEN CLI (`openomen`, v0.4.2) — these define the parity target ZenOmen must reach, not features to re-invent.

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

### Active

v1 scope: the complete Linux GUI product.

- [ ] Rust + Slint desktop application with the full GUI: Overview, Cooling, Performance, Lighting, Settings, Diagnostics
- [ ] Capability-based hardware detection with an explicit "unsupported" state for every absent sensor or control
- [ ] CPU utilization, frequency, temperature and per-core readings displayed, with historical telemetry charts
- [ ] GPU model, utilization, temperature, VRAM and power draw for supported NVIDIA / AMD / Intel hardware
- [ ] System RAM (total, available, used) including MemAvailable-based math
- [ ] Fan RPM and current fan mode read; supported fan modes applied and verified by read-back
- [ ] ACPI power profiles discovered dynamically, applied only when advertised, and verified after the write
- [ ] Combined presets (Quiet / Balanced / Gaming) applied transactionally, reporting partial application instead of false success
- [ ] Basic keyboard RGB: capability detection, apply existing preset profiles, enable/disable lighting, and migrate existing RGB profile JSON
- [ ] Restricted D-Bus system service (`com.zenomen.Hardware1`) owning all privileged hardware writes, with polkit-based authorization
- [ ] System tray (open dashboard, current profile, diagnostics, launch-at-login, exit), autostart at login, and close-to-tray
- [ ] Single-instance enforcement
- [ ] TOML configuration persistence with validated loading and safe defaults on a corrupt file
- [ ] Diagnostics export: version, OS/kernel, hardware identifiers, detected sensors, recent telemetry and errors
- [ ] Bounded telemetry history (fixed-capacity ring buffer) with no unbounded growth
- [ ] Measured resource budget: ≤40 MB peak RSS for the UI process and ≤0.1% idle CPU, with the service measured separately
- [ ] Linux distribution: DEB and AppImage packages with desktop entry and systemd hardware service
- [ ] Automated CI/CD: format, clippy, tests and packaging on GitHub Actions, with releases on tags and published checksums
- [ ] A "Check for updates" link that opens the GitHub Releases page (no download, no self-modification)
- [ ] Repository rebranded to ZenOmen (README, docs) with GPL-3.0 notices and HP trademark disclaimer preserved
- [ ] Python `openomen` CLI removed from the repository once Rust parity is verified

### Out of Scope

- Manual fan curves (PWM curves, daemon driving them) — deliberately excluded from v1; unsafe to expose before the verified control path is proven, and requires a running process policy that doesn't exist yet
- Full RGB editor and advanced effects (zone-by-zone color picker, brightness/speed, hardware animations) — v1 ships presets and on/off only
- Automated thermal optimization / predictive fan control — depends on safe fan curves and validated thermal models
- Overclocking and undervolting — hardware risk far outside a verified-write control model
- Cloud accounts, sync and telemetry upload — the product is local-first; nothing leaves the machine by default
- Windows hardware control — architecture stays platform-ready, but Windows fan/RGB/EC support needs its own interface research and validation; not advertised until validated
- In-app self-update (download, verify, install) — v1 defers to DEB/AppImage and installer flows; eliminates the root-execution and rollback complexity of an updater

## Context

**Current repository state:** a fork of OpenOMEN (`Omprakash-p06/zenomen`) containing only the Python implementation — a single extensionless 1703-line script `openomen`, `openomen.spec` (PyInstaller), `how-to-release.txt`, `README.md` and the GPL-3.0 `LICENSE`. There is no `Cargo.toml`, no Rust code, no tests, no CI, no `.gitignore` and no dependency manifest.

**What the existing CLI establishes (from `.planning/codebase/`):**
- hp-wmi fan mapping: `pwm1_enable` `0` = maximum/manual, `2` = automatic/BIOS; this mapping is specific to the validated HP-WMI interface and must not be generalized to arbitrary PWM controllers
- Temperature sensors cannot be trusted by index — driver name and label must be read before assigning a human-readable component name
- Three incompatible RGB interfaces exist across models and are detected by capability probe
- The Python implementation prints rather than returns data, duplicates its hwmon/thermal discovery between `read_temperature()` and `run_doctor()` (already diverged), and has no testable seams

**Known defects in the Python implementation to not carry forward:**
- Discovered-and-ignored `-w` flag on `stats`; `-w` always renders the full dashboard instead of the requested subsystem
- Fixed `card0` hardcoding in Intel iGPU frequency lookup
- Uncaught `ValueError` traceback on a malformed RGB hex value
- `rgb set` executes its body twice (writes state as the user, then re-execs under sudo and rewrites it root-owned)
- Updater disables TLS verification, executes the downloaded binary, and installs it with root privileges with no checksum or signature
- `read_temperature()` and the doctor's sensor selection can report different sensors for the same machine

**Hardware environment:** HP OMEN / Victus laptops exposing hp-wmi; firmware varies by model, so capability detection must be dynamic. Eight devices are confirmed working in the existing project; no universal compatibility is claimed beyond that.

**Product direction:** Japanese minimalism plus dark gaming aesthetics (Enso-inspired identity, obsidian/charcoal surfaces, crimson and muted-gold accents used sparingly, Inter + JetBrains Mono, status conveyed by more than color alone). The visual system is part of the product, not a nice-to-have.

## Constraints

- **Tech stack**: Rust (edition 2024) + Slint for a compiled declarative GUI; TOML + Serde for configuration; `tracing` for structured logs; Cargo Packager for DEB/AppImage — dependencies stay deliberately small rather than pulling in a large framework
- **Performance**: ≤40 MB peak RSS for the desktop UI process and ≤0.1% idle CPU, measured over fixed windows rather than snapshots; if a separate hardware service exists it gets its own budget and is reported separately, never summed into the UI number
- **Performance method**: CPU sampling must not block the UI thread; one long-lived monitoring worker instead of a thread per sensor; bounded `VecDeque` history (120 samples ≈ 2 minutes at 1 Hz)
- **Security**: least privilege — the GUI runs as a normal user, read-only telemetry needs no root, all privileged writes go through the restricted D-Bus service, which accepts only predefined operations and never arbitrary paths or shell commands
- **Privacy**: local-first; no account, no database server, no cloud connection, no telemetry uploaded by default
- **Compatibility**: Linux first (Ubuntu-based and Arch-family validated); Windows kept architecture-ready but unadvertised until its interfaces are validated
- **Licensing**: GPL-3.0 inherited from OpenOMEN — original copyright notices preserved, modified sources marked, license shipped with distributed binaries, HP trademark disclaimer retained, no implied HP endorsement
- **Versioning**: semantic versioning; v0.1.0 is the foundation, 1.0.0 is the first stable release
- **No network dependency**: core functionality works fully offline

## Key Decisions

| Decision | Rationale | Outcome |
|----------|-----------|---------|
| Rewrite in Rust + Slint rather than extend the Python CLI | The 40 MB / 0.1% budget, a real GUI, tray integration and native packaging are not reachable from a single self-updating Python script | — Pending |
| GUI-only product; no Rust CLI surface | The CLI is being replaced rather than ported; scripting use is secondary to the desktop experience | — Pending |
| Python `openomen` stays in the repo until Rust parity, then is deleted | Keeps a working tool for existing users and a behavioral reference during the rewrite | — Pending |
| `HardwareProvider` trait with `RgbProvider` as a separate optional capability | Capability discovery over assumption: not every OMEN/Victus supports every feature, and RGB is genuinely optional | — Pending |
| UI never touches sysfs or invokes privileged commands | Keeps the hardware layer independent of the UI so the interface can evolve without risking hardware behavior | — Pending |
| Privileged writes behind a restricted D-Bus service (`com.zenomen.Hardware1`) with polkit | Avoids running the GUI as root and replaces OpenOMEN's re-exec-under-sudo model, which executes command bodies twice | — Pending |
| Every hardware write verified by read-back; failures reported, never assumed | Trust is the core value; silent success on a failed write is the worst failure mode | — Pending |
| Missing sensor displays "Unavailable", never zero | A fabricated reading is worse than an honest gap, and zero is indistinguishable from a real value | — Pending |
| No manual fan curves in v1 | Exposing arbitrary PWM before the verified control path is proven is a hardware-safety risk | — Pending |
| Bounded `VecDeque` telemetry history instead of unbounded growth | Predictable memory is a product requirement, not an optimization | — Pending |
| No in-app self-update in v1; a link to Releases instead | Package-manager updates are simpler and safer than re-implementing download-vs-install-vs-rollback in the app | — Pending |
| Fork preserves GPL-3.0 and upstream notices | Required by the license and by the community relationship with the original project | — Pending |
| Windows designed for but not shipped in v1 | Linux is the validated platform; Windows lacks a universal fan-control API, so its support needs separate research | — Pending |

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
*Last updated: 2026-10-30 after initialization*
