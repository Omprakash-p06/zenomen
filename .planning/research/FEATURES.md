# Feature Research

**Domain:** Laptop hardware control center / system monitor for HP OMEN & Victus on Linux  
**Researched:** 2026-10-30  
**Updated:** 2026-10-30 (incorporated review corrections: aligned v1 vs v1.x priorities, competitor landscape, PresetResult, RAM from scratch, GNOME fallback)  
**Confidence:** HIGH for table stakes, competitive differentiators, and authoritative requirement alignments

## Feature Landscape

### Table Stakes (Users Expect These)

Features users assume exist. Missing these = the product feels incomplete or broken.

| Feature | Why Expected | Complexity | Notes |
|---------|--------------|------------|-------|
| Live CPU utilization | Baseline expectation | LOW | Per-core + aggregate. Computed via previous-tick deltas on a worker thread, never blocking the UI |
| CPU temperature | Crucial on high-performance laptops | MEDIUM | Identified by hwmon driver + label priority, never by `tempN` index. Reports "Unavailable" when absent |
| GPU name, utilization, temperature, VRAM, power | Dedicated GPU monitoring is essential for gaming laptops | MEDIUM-HIGH | Dynamically discovered DRM paths (no `card0` hardcoding), NVIDIA via `nvidia-smi`/NVML, AMD via `gpu_busy_percent` |
| RAM usage | Baseline monitoring | LOW | First-time implementation via `/proc/meminfo` using `MemAvailable` math (was completely missing in legacy CLI) |
| Fan RPM | Proof that cooling is operational | LOW-MEDIUM | `fan1_input`/`fan2_input`; shows "Unavailable" rather than 0 when absent |
| Fan mode switch (Auto vs Maximum) | Primary hardware control | MEDIUM | Validated Automatic (`2`) and Maximum (`0`) modes written via D-Bus, verified via read-back; manual mode (`1`) not exposed without curve support |
| Current fan mode display | Live status | LOW | Distinguishes Automatic, Maximum, and Manual (external/kernel control) rather than collapsing unknown values |
| Power profile display + switch | Standard ACPI knob | MEDIUM | Read dynamically from `platform_profile_choices`; verified after write |
| Explicit unsupported states | Hardware varies across models | LOW | Missing features display plain-language explanations instead of blank or disabled boxes |
| Minimize to tray + restore | Standard desktop utility behavior | MEDIUM | Slint 1.17 `SystemTrayIcon`; single-instance D-Bus activation fallback for stock GNOME |
| Autostart at login | Ready after boot | LOW | Standard XDG autostart `.desktop` entry in `~/.config/autostart` |
| Settings persistence | User preferences survive restarts | LOW | `config.toml`, validated loading, safe defaults on corruption preserving original file |
| No crash on missing sensors | Robust degradation | MEDIUM | Every hardware provider returns `Result`; absence degrades gracefully |
| Strict resource budget | Lightweight utility promise | HIGH | ≤40 MB peak UI RSS, ≤15 MB service RSS, ≤0.1% idle CPU; measured and enforced |

### Differentiators (Competitive Advantage)

How ZenOmen sets itself apart from existing tools (OpenOMEN, OMEN-HUB, victus-control, OmenCore). ZenOmen does not compete on having the most complex controls; it competes on trust, performance, and craft:

1. **Measured Low Memory Footprint:** ≤40 MB peak RSS for UI process, ≤15 MB for service; orders of magnitude lighter than Electron-based control centers.
2. **Instant Startup & Minimal Background CPU:** ≤0.1% idle CPU on dashboard, ≤0.02% in tray. Zero continuous UI polling or blocking sampling sleeps.
3. **Verified Hardware Operations:** Every fan mode, power profile, and RGB write is read back from hardware with retry/backoff.
4. **Honest Granular Presets (`PresetResult`):** Combined presets report exactly which subsystem succeeded (power vs fan) without false claims of atomic transactions or rollback.
5. **Reliable Capability Detection:** Hardware detection gates UI controls; missing features read "Unavailable" with plain-language reasons, never fabricated zeros.
6. **Unified Sensor Registry:** A single source of truth for both live dashboard and diagnostics, preventing sensor identification divergence.
7. **Restrained Visual Aesthetics:** Japanese minimalism (dark obsidian/charcoal surfaces, crimson/gold accents, Inter + JetBrains Mono) rather than noisy RGB gaming styling.
8. **Predictable Linux Integration:** Single-instance D-Bus activation (`com.zenomen.App`) provides a reliable fallback for stock GNOME users without tray extensions.
9. **Safe RGB Migration:** Preserves legacy `~/.config/openomen/rgb/*.json` files intact, migrates valid `#RRGGBB` colors, and clearly flags unsupported animation fields.

### Anti-Features (Excluded from Scope)

| Feature | Why Requested | Why Problematic | Resolution |
|---------|---------------|-----------------|------------|
| Manual fan curves in v1 | Fine-grained tuning | Writing unverified PWM to EC without kernel-level driver support can overheat or damage hardware | Excluded in v1; ship verified Auto/Max modes |
| Arbitrary PWM slider | Granular fan control | Percentage of unknown maximum is dangerous and meaningless without hardware curve calibration | Excluded in v1 |
| Unsupervised background fan daemon | Continuous fan tuning | Running daemon holding PWM state without fail-safe policies is a thermal risk | Excluded in v1 |
| In-app self-updater | Convenience | Downloading and executing binaries with root permissions creates severe security risks (the legacy CLI flaw) | Excluded; provide link to GitHub Releases; package-manager distribution |
| Full RGB effects editor | Custom animations | Large surface area for effects the hardware interface cannot actually express in Linux | Presets + on/off in v1; identify unsupported fields |
| Telemetry upload / Cloud accounts | Analytics | Contradicts local-first philosophy and creates privacy hazards | Local-first; diagnostics export is user-initiated |
| Overclocking / undervolting | Performance tuning | High hardware damage risk without safety margins | Out of scope |
| Windows hardware control in v1 | Cross-platform parity | No universal Windows laptop fan API; unvalidated OEM interfaces produce broken features | Linux-first; Windows telemetry deferred |

---

## MVP Definition (Authoritative v1 Scope)

This list matches the authoritative v1 requirements in `REQUIREMENTS.md`:

- [ ] **GUI Shell:** Overview, Cooling, Performance, Lighting, Settings, Diagnostics with capability-aware rendering
- [ ] **CPU Telemetry:** Aggregate and per-core utilization, live frequency, temperature selected by driver/label priority
- [ ] **GPU Telemetry:** Model, utilization, temperature, VRAM usage, and power draw for NVIDIA/AMD/Intel hardware (dynamically discovered DRM paths)
- [ ] **RAM Telemetry:** Total, available, and used RAM computed via `MemAvailable` from `/proc/meminfo` (first-time implementation)
- [ ] **Cooling:** Fan RPM, current mode display (Auto, Max, Manual/external); verified Auto/Max switching; manual mode not exposed without curve support
- [ ] **Performance:** ACPI power profiles dynamically read from `platform_profile_choices`, applied, and verified
- [ ] **Combined Presets:** Quiet, Balanced, Gaming presets with granular execution reporting (`PresetResult`: power_applied, fan_applied, partial_success)
- [ ] **Basic RGB:** Hardware capability detection, apply presets, enable/disable lighting, strict `#RRGGBB` validation; migrate legacy JSON profiles preserving source files and identifying unsupported animation fields
- [ ] **Privileged Hardware Service:** Standalone system D-Bus service (`com.zenomen.Hardware1`) with mandatory polkit authorization on all mutating actions
- [ ] **Desktop Integration:** System tray menu, XDG autostart, close-to-tray preference, theme selection (ZenOmen Dark), configurable monitoring intervals
- [ ] **Single Instance:** Enforced via D-Bus session activation (`com.zenomen.App` `PresentWindow`), providing window restoration on GNOME without tray icons
- [ ] **Telemetry Charts & History:** Rolling charts fed from a bounded 120-sample ring buffer with flat memory
- [ ] **Diagnostics:** Single shared `SensorRegistry` report, error history, and one-click file export
- [ ] **Config Persistence:** TOML configuration with validation, safe defaults on corruption preserving original file
- [ ] **Packaging & CI:** DEB and AppImage packages, automated GitHub Actions CI with clippy and `cargo audit` gates, release assets with checksums
- [ ] **Resource Budget:** Measured UI peak RSS ≤40 MB, service RSS ≤15 MB, idle CPU ≤0.1%

---

## Feature Prioritization Matrix

| Feature | User Value | Implementation Cost | Priority |
|---------|------------|---------------------|----------|
| CPU (aggregate & per-core) / GPU (util, temp, VRAM, power) / RAM / temp monitoring | HIGH | MEDIUM | P1 (v1) |
| Fan RPM + verified Auto/Max switching | HIGH | MEDIUM | P1 (v1) |
| ACPI power profiles (dynamically read + verified) | HIGH | MEDIUM | P1 (v1) |
| Granular combined presets (`PresetResult`) | MEDIUM | LOW | P1 (v1) |
| Explicit unsupported states & capability detection | HIGH | LOW | P1 (v1) |
| Shared `SensorRegistry` for dashboard and diagnostics | HIGH | MEDIUM | P1 (v1) |
| Privileged D-Bus service (`com.zenomen.Hardware1`) + mandatory polkit | HIGH | HIGH | P1 (v1) |
| Tray + autostart + single-instance GNOME fallback | HIGH | MEDIUM | P1 (v1) |
| Theme selection & configurable polling intervals | MEDIUM | LOW | P1 (v1) |
| Basic RGB (presets, on/off, safe legacy JSON migration) | MEDIUM | MEDIUM | P1 (v1) |
| Bounded history + rolling charts | MEDIUM | MEDIUM | P1 (v1) |
| Diagnostics export to file | MEDIUM | LOW | P1 (v1) |
| DEB + AppImage + CI/CD with `cargo audit` gate | HIGH | MEDIUM | P1 (v1) |
| Resource budget enforcement (≤40 MB UI RSS, ≤0.1% CPU) | HIGH | HIGH | P1 (v1) |
| Cryptographic in-app update checking with release notes | MEDIUM | MEDIUM | P2 (v1.x) |
| Additional confirmed device models from community reports | MEDIUM | LOW | P2 (v1.x) |
| Windows telemetry (CPU/RAM via Win32) | MEDIUM | HIGH | P2 (v1.x) |
| Manual fan curves with model-gated PWM | HIGH | HIGH | P3 (v2+) |
| Full RGB per-zone editor and hardware animations | MEDIUM | HIGH | P3 (v2+) |
| Fan/RGB state restoration after reboot/suspend | MEDIUM | MEDIUM | P3 (v2+) |
| Windows hardware control (validated OEM interfaces) | MEDIUM | HIGH | P3 (v2+) |

---

## Competitor Landscape & Positioning

| Feature | OpenOMEN (Legacy CLI) | OMEN-HUB (Rust GUI) | victus-control | OmenCore | ZenOmen (Target) |
|---------|----------------------|---------------------|----------------|----------|-------------------|
| Architecture | 1,702-line single script | Monolithic GUI | Mixed Python/GUI | Multi-platform CLI/GUI | Two-process architecture (UI + D-Bus service) |
| Memory Footprint | CLI, ephemeral | ~80–120 MB | ~100+ MB | Variable | **≤40 MB peak UI RSS, ≤15 MB service RSS** |
| Idle CPU | High (500 ms sleep polling) | Moderate | Moderate | Moderate | **≤0.1% idle CPU** (worker deadline scheduling) |
| Fan Control | Auto/Max, verified | Auto/Max | Fan curves via patched driver | Full fan control | Auto/Max verified; capability-gated |
| Presets | Boolean return | Basic modes | Profile switching | Profiles | **Granular `PresetResult` reporting** |
| Sensor Selection | Duplicated in `read_temp` & `doctor` | Generic hwmon | Specific to Victus | Generic | **Single unified `SensorRegistry`** |
| RAM Monitoring | **None** | Present | Present | Present | **`/proc/meminfo` (`MemAvailable`) from scratch** |
| Privilege Model | `sudo` re-exec (double execution) | Varies / root | Root helper | Driver-based | **Mandatory Polkit via system D-Bus daemon** |
| GNOME Support | Terminal only | Tray dependent | Tray dependent | Tray dependent | **Single-instance D-Bus activation fallback** |
| RGB Migration | Stored inert animation fields | Custom RGB | Custom | OpenRGB integration | **Safe JSON migration, static colors, strict `#RRGGBB`** |

---
*Feature research for: ZenOmen*  
*Updated: 2026-10-30*
