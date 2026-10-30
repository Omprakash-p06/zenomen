# Project Research Summary

**Project:** ZenOmen  
**Domain:** Native Linux desktop hardware control center (HP OMEN / Victus), Rust + Slint  
**Researched:** 2026-10-30  
**Updated:** 2026-10-30 (incorporated review corrections: two-process architecture, D-Bus contract, sensor registry, PresetResult, RAM from scratch, GNOME fallback, memory benchmarks)  
**Confidence:** HIGH (architecture, features, pitfalls, D-Bus contract) / MEDIUM-HIGH (packaging toolchain, empirical memory validation)

## Executive Summary

ZenOmen is a rewrite, not a greenfield experiment: the Python OpenOMEN CLI in this repository has already proven which hardware interfaces work on which models, and it has already exposed precisely which mistakes cost it reliability. The research picture is therefore unusually clear — the value is in the *discipline* (capability detection, verified writes, honest "unavailable" states, a bounded footprint), not in discovering new hardware tricks.

The architecture is a **two-process application architecture with three logical layers**:
1. An unprivileged desktop application (`zenomen-gui`) hosting the Slint UI and Rust core, owning a strict ≤40 MB peak RSS budget.
2. A small, sandboxed root D-Bus service (`zenomen-hw`) owning privileged hardware writes behind mandatory polkit authorization, with an independent ≤15 MB RSS budget.
3. Logical interface layer: Linux kernel sysfs and procfs interfaces (`/sys/class/hwmon`, `/sys/firmware/acpi`, `/sys/class/drm`, `/proc`). The kernel is an OS interface layer, not an application process.

Rust + Slint 1.17 is the right stack: Slint 1.17 ships a native `SystemTrayIcon`, tooltips, and `RadioGroup`, so no separate tray crate is needed. To handle environments like stock GNOME where StatusNotifierItem is invisible without extensions, ZenOmen implements single-instance D-Bus activation (`com.zenomen.App` `PresentWindow`) so relaunching the app focuses the existing window. The packaging toolchain (`cargo-packager`) must be pinned and audit-gated with `cargo audit`, with `cargo-deb` + `appimagetool` as the documented fallback.

The biggest product risks are thermal and reputational: writing an unverified PWM value, or showing a control that silently does nothing, is worse than showing nothing at all. Every hardware write must be read back, every absent sensor must say "Unavailable", and the known defects of the Python implementation (blocking 500ms CPU sleeps, missing RAM telemetry, hardcoded `card0`, unapplied RGB fields, unverified power writes, and duplicate temperature heuristics) are treated as explicit anti-patterns not to reproduce.

## Key Findings

### Recommended Stack

Rust (edition 2024) + Slint 1.17 with `renderer-femtovg`, `sysinfo` 0.38 restricted to CPU/RAM semantics (`MemAvailable`), `serde`+`toml` for config, `zbus` for the system D-Bus service and session-bus single-instance handling, `tracing` for diagnostics, `thiserror` for the `HardwareError` contract, and `cargo-audit` + `cargo-packager` (pinned) in CI. Dependencies stay small on purpose; `tokio` and any webview are explicitly rejected.

**Core technologies:**
- **Slint 1.17** — compiled declarative GUI. Whole-app RSS is budgeted at ≤40 MB peak and ≤0.1% idle CPU.
- **Rust std threads** — one deadline-scheduled monitoring worker; avoids async runtime bloat at 1–5 s polling cadences.
- **`zbus` + polkit** — privileged-write boundary with mandatory polkit authorization (`com.zenomen.hardware.control`).
- **RAM from scratch** — `/proc/meminfo` parsing using `MemAvailable` math (implementing RAM telemetry for the first time).
- **`cargo-packager` (pinned) + `cargo audit`** — DEB/AppImage with an active advisory gate.

### Expected Features

The parity target is proven: fan mode + RPM, ACPI power profiles + combined presets, CPU/GPU/RAM/temperature, three RGB variants with JSON profiles, and a `doctor`/`info` diagnostic surface.

**Must have (table stakes & v1 scope):**
- Live CPU (aggregate & per-core) / GPU (model, temp, util, VRAM, power) / RAM / temperature monitoring with "Unavailable" rather than zero
- Fan RPM and current mode; auto/max switching verified by read-back; manual mode not exposed without hardware curve support
- ACPI power profile display and switching, choices read dynamically and verified after write
- Combined presets (Quiet/Balanced/Gaming) with granular execution reporting (`PresetResult`: power_applied, fan_applied, partial_success)
- Explicit unsupported states everywhere driven by dynamic capability detection
- Tray, autostart, single instance with GNOME window-focus fallback, persisted settings, theme selection, configurable polling intervals
- Basic RGB: capability detection, presets, on/off, and safe migration of legacy JSON profiles
- DEB + AppImage with automated CI/CD releases and checksums

**Differentiators (competitive advantage):**
- Verified writes with granular partial-apply reporting (`PresetResult`)
- Unified `SensorRegistry` preventing sensor identification drift
- Capability-driven UI instead of a generic greyed-out layout
- One-click diagnostics export built for bug reports
- Bounded-history telemetry charts (120 samples)
- Measured, published resource budget (≤40 MB UI RSS, ≤15 MB service RSS, ≤0.1% idle CPU)
- Restrained, non-gamer visual identity (Japanese minimalism dark theme)

**Defer (v2+):**
- Manual fan curves and persistent fan daemon
- Full RGB editor and hardware animation effects
- Restore-after-reboot/suspend behaviour
- Windows telemetry, then Windows hardware control

### Architecture Approach

Two application processes, one privilege boundary, three logical layers:
1. **Slint presentation (`zenomen-gui`)** — six pages, tray, theme; zero hardware access, zero blocking work.
2. **Application core (`zenomen-gui`)** — lifecycle, scheduler, telemetry normalization, 120-sample ring buffer, config, diagnostics, single-instance activator.
3. **`HardwareProvider` / `RgbProvider`** — capability-detected, vendor-neutral; reads in-process directly from sysfs/procfs without root privilege.
4. **Shared `SensorRegistry`** — single source of truth for temperature, fan, and GPU sensors shared by dashboard and diagnostics.
5. **`zenomen-hw` service** — standalone root daemon on system bus; mandatory polkit check; whitelisted methods (`GetCapabilities`, `GetFanStatus`, `SetFanMode`, `GetPowerProfiles`, `GetPowerProfile`, `SetPowerProfile`, `ApplyRgbProfile`, `SetRgbEnabled`); verified read-back with retry/backoff; systemd sandboxed.
6. **Packaging + CI** — `.desktop`, `zenomen-hw.service`, D-Bus/polkit policy, DEB/AppImage, checksums.

### Critical Pitfalls

1. **Trusting sensor indices & hardcoded paths** — resolve by hwmon driver + label priority; enumerate DRM card paths dynamically (never hardcode `card0`). Both dashboard and diagnostics consume the shared `SensorRegistry`.
2. **Assuming fan control exists or exposing manual mode prematurely** — `pwm1_enable` is model-gated. Expose only validated modes (Auto and Max) in v1; display manual mode if kernel reports it, but do not allow writing manual mode without hardware curve support. Verify every write with read-back.
3. **Privilege escalation into the GUI** — no sudo re-exec, no setuid; narrow D-Bus service with mandatory polkit authorization and compiled path constants.
4. **UI-thread blocking** — worker thread owns sampling; uses prior-tick counter deltas instead of Python's 500 ms sleep.
5. **Unbounded memory** — fixed-capacity `VecDeque` (120 samples); mutate Slint models in place; verify with a 2-hour tray session measuring cold start, warm dashboard, minimized tray, and peak RSS.
6. **Optimistic or "transactional" claims** — ACPI power and fan mode are not atomic; report granular `PresetResult` (power_applied, fan_applied, partial_success) instead of claiming atomic transactions or rollback.
7. **GNOME tray absence** — StatusNotifierItem is invisible on stock GNOME; implement single-instance D-Bus activation (`com.zenomen.App` `PresentWindow`) to focus existing window on duplicate launch.
8. **Inert RGB fields & destructive migration** — legacy Python stored animation fields without applying them; ZenOmen validates `#RRGGBB` colors strictly, migrates static colors, flags unsupported animation fields, and never deletes source JSON files.
9. **Inheriting legacy defects** — RAM telemetry absent in legacy CLI must be implemented from scratch; `card0` hardcoding, unverified writes, and TLS-disabled updaters are explicitly eliminated.

## Implications for Roadmap

The five-phase roadmap structure remains unchanged:
- **Phase 1: Foundation — GUI Shell on Mock Hardware** (theme, six screens, worker, ring buffer, resource instrumentation, CI gates)
- **Phase 2: Linux Telemetry & Capability Detection** (real procfs/hwmon/DRM reads, RAM via `MemAvailable`, shared `SensorRegistry`, charts, diagnostics export)
- **Phase 3: Verified Hardware Control** (`zenomen-hw` service, mandatory polkit, verified fan/power writes, `PresetResult`, Cooling & Performance pages)
- **Phase 4: Desktop Integration, Config & Basic RGB** (tray, autostart, single instance with GNOME focus, atomic TOML config, Settings, basic RGB with safe JSON migration)
- **Phase 5: Distribution, Resource Budget & Rebrand** (DEB/AppImage, CI release workflows with checksums and audit gate, 2-hour RSS/CPU verification, rebrand docs, remove Python CLI after parity verification)

---
*Project research summary for: ZenOmen*  
*Updated: 2026-10-30*
