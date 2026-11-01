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
