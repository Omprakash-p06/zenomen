# Project Research Summary

**Project:** ZenOmen
**Domain:** Native Linux desktop hardware control center (HP OMEN / Victus), Rust + Slint
**Researched:** 2026-10-30
**Confidence:** HIGH (architecture, features, pitfalls) / MEDIUM (packaging toolchain, exact memory numbers)

## Executive Summary

ZenOmen is a rewrite, not a greenfield experiment: the Python OpenOMEN CLI in this repository has already proven which hardware interfaces work on which models, and it has already exposed precisely which mistakes cost it reliability. The research picture is therefore unusually clear — the value is in the *discipline* (capability detection, verified writes, honest "unavailable" states, a bounded footprint), not in discovering new hardware tricks.

The recommended approach is a three-layer split that the ecosystem already validates: a compiled Slint GUI that never touches `/sys`, a Rust core holding state/scheduling/history, and a small root D-Bus service that owns every privileged write behind polkit. Rust + Slint 1.17 is the right stack and is better than the spec assumed: 1.17 (2026-06-24) ships a native `SystemTrayIcon`, tooltips and `RadioGroup`, so no separate tray crate is needed. The main technical risk is not the GUI — it is the packaging toolchain: `cargo-packager` has an open July 2026 report of unaddressed RUSTSEC advisories, so it must be pinned and audit-gated, with `cargo-deb` + `appimagetool` as the documented fallback.

The biggest product risks are thermal and reputational, not technical: writing an unverified PWM value, or showing a control that silently does nothing, is worse than showing nothing at all. Every hardware write must be read back, every absent sensor must say "Unavailable", and the few known defects of the Python implementation must be listed as requirements *not* to reproduce — otherwise the rewrite will faithfully inherit them.

## Key Findings

### Recommended Stack

Rust (edition 2024) + Slint 1.17 with `renderer-femtovg`, `sysinfo` 0.38 restricted to CPU/RAM semantics, `serde`+`toml` for config, `zbus` for the system service, `tracing` for diagnostics, `thiserror` for the `HardwareError` contract, and `cargo-audit` + `cargo-packager` (pinned) in CI. Dependencies stay small on purpose; `tokio` and any webview are explicitly rejected.

**Core technologies:**
- **Slint 1.17** — compiled declarative GUI with a claimed sub-300 KiB runtime; the only realistic way to hold ≤40 MB RSS with a real UI. Tray/tooltips/radio groups landed in 1.17.
- **Rust std threads** — one deadline-scheduled monitoring worker; async complexity is not justified at 1–5 s polling.
- **`zbus` + polkit** — the privileged-write boundary, replacing the legacy sudo re-exec.
- **`sysinfo` 0.38** — cross-platform CPU/RAM, used narrowly so Linux-specific semantics (`MemAvailable`, iowait) are not silently redefined.
- **`cargo-packager` (pinned) + `cargo audit`** — DEB/AppImage with an active advisory gate.

### Expected Features

The parity target is proven: fan mode + RPM, ACPI power profiles + combined presets, CPU/GPU/RAM/temperature, three RGB variants with JSON profiles, and a `doctor`/`info` diagnostic surface that is how 8 devices got validated.

**Must have (table stakes):**
- Live CPU / GPU / RAM / temperature monitoring with "Unavailable" rather than zero
- Fan RPM and current mode; auto/max switching that is verified by read-back
- ACPI power profile display and switching, choices read dynamically
- Explicit unsupported states everywhere
- Tray, autostart, single instance, persisted settings
- DEB + AppImage with automated CI/CD releases

**Should have (competitive):**
- Verified writes with partial-apply reporting (most tools lie here)
- Capability-driven UI instead of a generic greyed-out layout
- One-click diagnostics export built for bug reports
- Bounded-history telemetry charts
- A measured, published 40 MB / 0.1% footprint
- Restrained, non-gamer visual identity
- Legacy RGB profile migration

**Defer (v2+):**
- Manual fan curves and any persistent fan daemon
- Full RGB editor and hardware effects
- Restore-after-reboot/suspend behaviour
- Windows telemetry, then Windows hardware control

### Architecture Approach

Three processes, one privilege boundary. The GUI (unprivileged) renders `AppEvent`s and emits `AppCommand`s; the core owns state, a deadline-scheduled worker, and a 120-sample ring buffer; the `zenomen-hw` system service validates whitelisted operations (polkit, constants-only paths) and writes to hwmon/ACPI, reading back every write. Telemetry reads stay in-process because they need no privilege — routing a temperature through D-Bus per tick would waste the CPU budget the product is built around.

**Major components:**
1. **Slint presentation** — six pages, tray, theme; zero hardware access, zero blocking work.
2. **Application core** — lifecycle, scheduler, telemetry normalization, bounded history, config, diagnostics.
3. **`HardwareProvider` / `RgbProvider`** — capability-detected, vendor-neutral; Linux implementation first, Windows later; `MockHardware` for tests and UI development.
4. **`zenomen-hw` service** — the only writer; validated, verified, sandboxed via systemd.
5. **Packaging + CI** — `.desktop`, `zenomen-hw.service`, D-Bus/polkit policy, DEB/AppImage, checksums.

### Critical Pitfalls

1. **Trusting sensor indices** — resolve by hwmon driver + label and compose DRM paths from the enumerated card; never assume `temp1` is the CPU or `card0` is the GPU. The legacy tool already has two diverging copies of this logic.
2. **Assuming fan control exists or is honored** — `pwm1_enable` is model-gated (Victus S manual support only landed in the 6.20/7.0 cycle), so capability-detect, then verify every write with retry/backoff and report the observed state.
3. **Privilege escalation into the GUI** — no sudo re-exec, no setuid; a narrow D-Bus service with validated parameters. The legacy `rgb set` already demonstrates the double-execution and root-owned-config failure mode.
4. **UI-thread blocking** — a single 500 ms sampling sleep on the render path is why the legacy dashboard stalls; the worker must own sampling and keep prior counters to avoid sleeps entirely.
5. **Unbounded memory in a "lightweight" app** — fixed-capacity `VecDeque`, mutate Slint models in place, and verify with a 2-hour tray session rather than a smoke test.
6. **Packaging toolchain risk** — pin `cargo-packager`, gate on `cargo audit`, prove the DEB/AppImage pipeline in its own phase, keep `cargo-deb` + `appimagetool` as fallback.
7. **Inheriting legacy defects** — `stats -w` being ignored, hardcoded `card0`, uncaught RGB hex tracebacks and duplicate sensor selection must be listed as things *not* to reproduce.

## Implications for Roadmap

Research suggests five phases at coarse granularity. The ordering is driven by one rule: **no write-capable UI may exist before the privilege boundary and read-back verification do.**

### Phase 1: Foundation — GUI Shell on Mock Hardware

**Rationale:** Every later phase needs a place to render and a way to measure. Starting with `MockHardware` means the entire UI is built and tested without a laptop, and the memory/CPU instrumentation exists before there is anything expensive to measure.
**Delivers:** Rust + Slint app; dark theme tokens; Overview page; `AppCommand`/`AppEvent` bridge; `HardwareProvider` + `MockHardware`; CPU/RAM calculation with tests; bounded telemetry history; benchmark instrumentation; `fmt`/`clippy`/`test`/`audit` CI.
**Addresses:** Skeleton of Overview, cap-driven layout, resource budget instrumentation.
**Avoids:** Pitfall 4 (blocking the UI thread) by establishing the worker/event split first; Pitfall 5 (unbounded memory) by shipping the ring buffer as part of the foundation; Pitfall 12 by starting with the delta list.

### Phase 2: Linux Telemetry and Capability Detection

**Rationale:** Read-only telemetry needs no privilege, so it can land before the risky control work — and it is what makes the app useful on day one. Capability detection must exist here because every later control gates on it.
**Delivers:** Real Linux provider (procfs/hwmon/DRM, NVIDIA via `nvidia-smi` for now); one shared sensor-selection helper driven by driver + label priority; `HardwareCapabilities` snapshot; telemetry charts from the bounded buffer; diagnostics export.
**Uses:** `sysinfo` for CPU/RAM, direct sysfs for spec-mandated semantics, `tracing` for the export.
**Implements:** `platform/linux/{procfs,hwmon,drm,acpi}.rs`, `core/{telemetry,memory}.rs`.
**Avoids:** Pitfall 1 (sensor identity by name), Pitfall 9 (generic layout), Pitfall 7's GPU cost (cache the `nvidia-smi` negative result).

### Phase 3: Verified Hardware Control Behind a Privileged Service

**Rationale:** This is the trust-critical phase and the whole reason for the service. It is deliberately isolated: fan/power writes, verification, and the privilege boundary are reviewed and tested together.
**Delivers:** `zenomen-hw` D-Bus service with polkit + systemd sandboxing; `SetFanMode` / `SetPowerProfile` / `ApplyRgbProfile` with whitelisted validation; read-back verification with retry/backoff; `VERIFICATION_FAILED` / `UNSUPPORTED_OPERATION` / `INVALID_PROFILE` surfaced in the UI; Cooling and Performance pages; combined presets with honest partial-apply reporting.
**Implements:** `daemon/zenomen-hw/`, `security/{permissions,validation}.rs`, `platform/linux/{acpi,hp_wmi,dbus}.rs`.
**Avoids:** Pitfall 2 (assumed fan control), Pitfall 3 (privilege escalation), Pitfall 6 (optimistic success).

### Phase 4: Desktop Integration, Config and Basic RGB

**Rationale:** A monitoring app that cannot persist settings or live in the tray is not shippable; these features share the "app lifecycle" concern and depend on the pages existing.
**Delivers:** Tray menu (open / current profile / diagnostics / launch at login / exit) via Slint 1.17 `SystemTrayIcon`; autostart; close-to-tray; single-instance with focus-existing; `config.toml` with validation + atomic writes + safe defaults; Settings page; Lighting page with presets, on/off, and migration of legacy `~/.config/openomen/rgb/*.json` with strict color validation.
**Avoids:** Pitfall 7 (DE/desktop variance), Pitfall 10 (config data loss), Pitfall 11 (inert RGB fields).

### Phase 5: Distribution, Resource Budget and Rebrand

**Rationale:** Packaging is where the project's riskiest dependency lives, so it gets its own phase and enough runway to swap tools if needed. The resource claim must be measured before it is published.
**Delivers:** DEB + AppImage with desktop entry, systemd unit, D-Bus/polkit policy; `ci.yml` / `release.yml` / `security.yml`; tag-driven releases with SHA256SUMS; `cargo audit` gate enforced; measured RSS/CPU report with the long-session test; README/docs rebrand with GPL-3.0 notices and HP disclaimer preserved; Python CLI removed after parity verification; "Check for updates" link.
**Avoids:** Pitfall 7 (packaging toolchain), Pitfall 5 (measured memory rather than claimed).

### Phase Ordering Rationale

- **Instrumentation before optimization:** the budget is measured from Phase 1 so Phase 5 can prove it rather than assert it.
- **Read before write:** telemetry (Phase 2) is privilege-free and unblocks value; control (Phase 3) is the risk; they never share a phase because a control bug in a telemetry phase would be misattributed.
- **The privilege boundary precedes all writes:** no sudo-re-exec path can be introduced "temporarily" because no write path exists before the service does.
- **Packaging last but not late:** it is a whole phase, not a last step, because the toolchain may need replacing.
- **The Python CLI is deleted only at the end:** it is the behavioural reference and the user's fallback during the rewrite.

### Research Flags

Phases likely needing deeper research during planning:
- **Phase 3:** hp-wmi's exact capability surface per model and the right verification retry/backoff profile; the D-Bus/polkit/systemd sandboxing specifics.
- **Phase 5:** current `cargo-packager` advisories and whether the fallback is needed; AppImage runtime behaviour on a clean machine.

Phases with standard patterns (skip research-phase):
- **Phase 1:** Slint + Rust project setup and the command/event bridge are well-documented.
- **Phase 2:** procfs/hwmon/DRM reading is thoroughly documented by the kernel and already implemented (twice) in the legacy code.
- **Phase 4:** XDG autostart and tray patterns are established; Slint 1.17's tray element is documented.

## Confidence Assessment

| Area | Confidence | Notes |
|------|------------|-------|
| Stack | HIGH | Versions and the 1.17 tray/tooltip additions verified against official release notes; memory figure below 300 KiB is a runtime claim, not app RSS |
| Features | HIGH | Table stakes are anchored in the proven Python feature set and the user's explicit v1 scope |
| Architecture | HIGH | The three-layer model matches both the spec and established Linux desktop practice; concurrency primitive choice is the least-verified part |
| Pitfalls | HIGH | Seven of twelve pitfalls are observed defects in the existing implementation, with line references |

**Overall confidence:** HIGH

### Gaps to Address

- **Whole-app RSS under Slint:** the "<300 KiB runtime" claim says nothing about a full app with charts, tray and fonts. Resolve by measuring in Phase 1 and gating in Phase 5; if femtovg misses, benchmark software/Skia renderers before changing the architecture.
- **`cargo-packager` health:** one maintained issue reports unaddressed RUSTSEC advisories; other projects still use it in late 2026. Resolve by pinning + `cargo audit` in Phase 1's CI and deciding definitively in Phase 5.
- **hp-wmi capability matrix:** which models expose writable `pwm1_enable` vs. RPM only, and what the new 6.20/7.0 manual fan support means per model. Resolve during Phase 3 planning; capability detection is the mitigation either way.
- **GNOME tray availability:** Slint's Linux tray is D-Bus-based and stock GNOME does not show it without an extension. Resolve by guaranteeing the relaunch-to-focus path in Phase 4 regardless.
- **Windows boundary honesty:** the trait must be validated by `MockHardware` and a second provider stub during Phase 1–2, or the Windows milestone becomes a refactor.

## Sources

### Primary (HIGH confidence)
- `.planning/codebase/{ARCHITECTURE,CONCERNS,STACK,INTEGRATIONS,STRUCTURE,CONVENTIONS,TESTING}.md` — the legacy implementation's proven features and observed defects, with line references
- User-provided ZenOmen specification (30 Oct 2026) — scope, stack, architecture, FR/NFR set, exclusions, milestone intent
- https://slint.dev/blog/slint-1.17-released — `SystemTrayIcon`, `Tooltip`, `RadioGroup` (2026-06-24)
- https://slint.dev/blog/making-slint-desktop-ready — Linux tray mechanism (D-Bus, not XEmbed)
- https://docs.kernel.org/hwmon/sysfs-interface.html — hwmon naming/unit conventions and non-guaranteed channel mapping
- https://docs.kernel.org/userspace-api/sysfs-platform_profile.html — dynamic choice discovery
- https://docs.kernel.org/hwmon/hp-wmi-sensors.html — HP-WMI sensor interface

### Secondary (MEDIUM confidence)
- https://github.com/uAtomicBoolean/slint-template/issues/6 (2026-07-28) — cargo-packager maintenance/RUSTSEC concern
- cargo-packager 0.12.0 (2026-02-02) post-handoff release and ongoing use in late-2026 projects
- https://security.opensuse.org/2024/04/02/kde6-dbus-polkit.html — D-Bus + polkit daemon arrangement
- Phoronix / LKML / Patchew (Jan–May 2026) — hp-wmi manual fan control for Victus S-Series, dual-channel PWM
- https://wiki.archlinux.org/title/HP_Omen_16-c0140AX — `pwm1_enable = 0` enables fan boost on that model
- OMEN/Victus community reports — models without writable fan interfaces under stock kernels

### Tertiary (LOW confidence)
- Slint's "<300 KiB RAM" marketing figure as an estimate for whole-app RSS — needs measurement, never a planning input
- Reddit/forum reports of specific OMEN 16 firmware behaviour — individual data points, not a capability matrix

---
*Research completed: 2026-10-30*
*Ready for roadmap: yes*
