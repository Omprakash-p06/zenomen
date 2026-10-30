# Feature Research

**Domain:** Laptop hardware control center / system monitor for HP OMEN & Victus on Linux
**Researched:** 2026-10-30
**Confidence:** HIGH for table stakes (established by the incumbent tools and by OpenOMEN's own proven feature set); MEDIUM for differentiators

## Feature Landscape

### Table Stakes (Users Expect These)

Features users assume exist. Missing these = the product feels incomplete or broken.

| Feature | Why Expected | Complexity | Notes |
|---------|--------------|------------|-------|
| Live CPU utilization | Every monitor has it; it is the reason to open the app | LOW | Per-core + aggregate. Read two `/proc/stat` samples on a worker thread, never on the UI thread |
| CPU temperature | The single most-watched number on a gaming laptop | MEDIUM | Identify by hwmon driver + label, never by `tempN` index. Report "Unavailable" when absent |
| GPU name, utilization, temperature | Users game; the GPU is why they bought the laptop | MEDIUM-HIGH | Three vendors, three interfaces. NVIDIA via `nvidia-smi`/NVML, AMD via `gpu_busy_percent`, Intel via DRM with per-card paths |
| RAM usage | Baseline monitoring | LOW | Use `MemAvailable`, not `MemFree`; label distinctly from "free" |
| Fan RPM | Proof the cooling is working; the #1 reason to install an OMEN tool | LOW-MEDIUM | `fan1_input`/`fan2_input`; show "Unavailable" rather than 0 |
| Fan mode switch (auto vs maximum) | The flagship action of the legacy CLI | MEDIUM | Write `pwm1_enable`, read back, report the actual state. Gate entirely on capability detection |
| Current fan mode display | Users need to know what the fans are doing *now* | LOW | Must distinguish auto / maximum / manual / unknown — the legacy tool collapsed `1` into "Unknown" |
| Power profile display + switch | ACPI `platform_profile` is the standard knob; users expect Quiet/Balanced/Performance | MEDIUM | Read choices dynamically from `platform_profile_choices`; never hardcode; verify after write |
| Power profile choices only when advertised | Applying an unsupported profile is a common user complaint | LOW | If the file lists two choices, show two |
| Explicit unsupported state | Firmware varies wildly across models; a missing feature must be explained, not hidden | LOW | This is the product's trust posture, implemented as UI text |
| Minimize to tray + restore | Standard desktop app behavior; a monitoring app that must stay open is useless | MEDIUM | Slint 1.17 `SystemTrayIcon`. GNOME needs the AppIndicator extension — provide a non-tray fallback |
| Autostart at login | Users want the tool ready after boot | LOW | XDG autostart `.desktop` entry, written into `~/.config/autostart` |
| Settings persistence | Changing a setting twice is a bug to users | LOW | `config.toml`, validated, safe defaults on corrupt file |
| No crash on missing sensors | A monitor that crashes when a sensor is absent is unusable | MEDIUM | Every provider returns `Result`; nothing unwraps hardware I/O |
| Reasonable resource usage | A monitoring tool that eats 300 MB is a joke to this audience | HIGH | The ≤40 MB / ≤0.1% budget is the differentiator *and* a table stake for this audience |

### Differentiators (Competitive Advantage)

Features that set the product apart. Not required, but valuable.

| Feature | Value Proposition | Complexity | Notes |
|---------|-------------------|------------|-------|
| Verified writes (read-back, always) | The legacy tool already proved this matters; most GUI tools show success optimistically | MEDIUM | Every mutation re-reads state and reports the *actual* result; partial application is reported as partial |
| Capability-first UI | Shows exactly what *this* machine supports instead of greying out a generic layout | MEDIUM | One `HardwareCapabilities` snapshot drives page rendering |
| Diagnostics export built for bug reports | The legacy `doctor`/`info` output is how 8 device models got validated | MEDIUM | Version, OS/kernel, hardware IDs, detected sensors, recent telemetry, errors — no unrelated user data |
| Rolling telemetry charts with bounded memory | History without a memory leak; a real differentiator vs. CLI tools | MEDIUM | Fixed-capacity `VecDeque`, 120 samples @1 Hz ≈ 2 min |
| Genuinely low footprint | 40 MB / 0.1% is a headline claim most Electron-era peers cannot make | HIGH | Must be measured and enforced, not asserted |
| Restrained, non-gamer aesthetic | Japanese-minimalism dark theme instead of RGB-everything | MEDIUM | Crimson/gold used sparingly; status never conveyed by color alone |
| Tray quick menu with live profile | "Current profile: Balanced" without opening a window | LOW | Slint tray `Menu` |
| Combined presets (Quiet/Balanced/Gaming) | One click sets ACPI profile + fan mode together | LOW-MEDIUM | Mapping already proven in the CLI; must report partial application honestly |
| Legacy RGB profile migration | Existing OpenOMEN users keep their lighting setup | MEDIUM | Read `~/.config/openomen/rgb/*.json`, convert to TOML, never silently discard |

### Anti-Features (Commonly Requested, Often Problematic)

Features that seem good but create problems. Documented so they are not re-added casually.

| Feature | Why Requested | Why Problematic | Alternative |
|---------|---------------|-----------------|-------------|
| Manual fan curves in v1 | "The whole point of a fan tool" | Requires arbitrary PWM writes; on hardware whose EC may ignore or mis-handle them, this can overheat a laptop. Even upstream only just landed model-gated manual support | Ship auto/max (verified, firmware-bounded); add curves only after the control path is proven and model-gated |
| Arbitrary PWM percentage slider | Looks like fine control | A percentage of an unknown maximum is meaningless — RPM-to-percent requires a reliable max, which the hardware does not expose | Show RPM; offer only the modes hardware advertises |
| Fan control as the default in a fresh install | Users want quiet fans | Writing `pwm1_enable` at startup changes thermal behavior without consent | Default to firmware/BIOS control; require an explicit user action |
| A background daemon applying fan settings persistently | Desirable eventually | An unsupervised root process holding PWM state with no safe recovery policy is a real thermal risk | Defer; if added, strictly bounded and restoring firmware control on exit |
| In-app binary self-update in v1 | Convenience, and the old CLI had it | Download-vs-verify-vs-install-vs-rollback is where the legacy tool's worst security bugs live (disabled TLS, unverified binary executed as root) | Link to GitHub Releases; let DEB/AppImage/installer own upgrades |
| Full RGB effects editor in v1 | Users like effects | Huge surface area (per-zone editors, animation timing) for a feature the profile schema cannot even express in hardware today | Presets + on/off; migrate profiles; keep effects for later |
| Cloud account, sync or anonymized telemetry | "Improve the product" | Contradicts local-first; for a root-privileged hardware tool it is a trust hazard | Local diagnostics export the user chooses to share manually |
| Overclocking / undervolting | Performance tuning | Silent instability and hardware damage; no safe verification loop | Out of scope, documented |
| "Automatically optimize thermals" | One-click magic | Predictive thermal control needs validated models; acting on bad models is worse than doing nothing | Defer until fan-curve foundations and thermal data exist |
| Windows fan control in v1 | Cross-platform ambition | There is no universal Windows laptop fan API; promising it without validated OEM interfaces produces a broken feature | Linux-first; Windows telemetry/controls in later releases only when validated |

## Feature Dependencies

```
Verified write path (read-back)
    └──requires──> Capability detection
    └──requires──> Privileged service (D-Bus + polkit)

Combined presets (Quiet/Balanced/Gaming)
    └──requires──> ACPI power profile control
    └──requires──> Fan mode control
                       └──requires──> hp-wmi writable pwm1_enable

System tray
    └──requires──> GUI shell (Slint)
Autostart ──requires──> Config persistence
Close-to-tray ──requires──> Tray ──conflicts──> "exit on window close"

Telemetry charts
    └──requires──> Bounded history buffer
    └──requires──> Monitoring worker (non-blocking sampling)

Diagnostics export ──requires──> Config persistence + tracing logs
RBG profile migration ──requires──> serde_json parsing + strict validation
DEB/AppImage packaging ──requires──> Working release build + stable version

Single-instance enforcement ──enhances──> Tray (a second launch must focus the first window)
```

### Dependency Notes

- **Verified writes require capability detection:** you cannot verify what the hardware never claimed to support; detection must precede every write path.
- **Fan mode control requires writable `pwm1_enable`:** several OMEN 16 models expose RPM but not a writable PWM interface, so fan control is genuinely conditional.
- **Combined presets require both power and fan control:** if only one is available, the preset must apply the available half and report the rest as unsupported — not fail wholesale.
- **Charts require a non-blocking worker:** sampling on the UI thread is what produced the legacy dashboard's fixed 500 ms stall.
- **Tray conflicts with "exit on window close":** closing to tray and quitting must be two distinct actions, or users cannot quit.
- **Packaging requires a frozen version:** DEB/AppImage artifacts with a moving version undermine the release pipeline and the update-check link.

## MVP Definition

### Launch With (v1)

The Linux GUI MVP — matches the confirmed v1 scope.

- [ ] GUI shell: Overview, Cooling, Performance, Lighting, Settings, Diagnostics — with capability-aware rendering
- [ ] CPU utilization (per-core + aggregate), frequency, temperature
- [ ] GPU name/utilization/temperature for supported NVIDIA/AMD/Intel hardware
- [ ] RAM (total/available/used)
- [ ] Fan RPM + current mode; supported modes switchable and verified
- [ ] ACPI power profiles: discovered, displayed, applied, verified
- [ ] Combined presets applied transactionally with honest partial-failure reporting
- [ ] Basic RGB: detect, apply presets, enable/disable, migrate legacy profiles
- [ ] Restricted D-Bus hardware service with polkit; no root GUI
- [ ] Tray, autostart, close-to-tray, single instance
- [ ] Bounded telemetry history + rolling charts
- [ ] Diagnostics export
- [ ] TOML config with validation and safe defaults
- [ ] Resource budget measured and reported (≤40 MB UI RSS, ≤0.1% idle CPU)
- [ ] DEB + AppImage; CI/CD with automated releases and checksums
- [ ] "Check for updates" link to Releases

### Add After Validation (v1.x)

- [ ] Update-availability check with release notes, signature verification, and user-confirmed install — the four-step path the user deferred
- [ ] Per-core CPU detail and GPU VRAM/power-draw surfaced more prominently (if v1 hides them)
- [ ] Theme selection and configurable polling intervals
- [ ] Additional confirmed device models from community reports
- [ ] Windows telemetry (CPU/RAM via Win32) with tray + startup, no hardware control yet

### Future Consideration (v2+)

- [ ] Manual fan curves with model-gated PWM — the headline remaining gap vs OMEN Gaming Hub, and the prerequisite is a safe, validated write path
- [ ] Full RGB editor (per-zone colors, brightness/speed, effects honored from profile)
- [ ] Fan-mode/RGB restoration after reboot or suspend (requires a bounded, recoverable daemon)
- [ ] Windows hardware control, only for validated OEM interfaces
- [ ] Automated thermal optimization on validated models

## Feature Prioritization Matrix

| Feature | User Value | Implementation Cost | Priority |
|---------|------------|---------------------|----------|
| CPU/GPU/RAM/temp monitoring | HIGH | MEDIUM | P1 |
| Fan RPM + auto/max toggle (verified) | HIGH | MEDIUM | P1 |
| ACPI power profiles (verified) | HIGH | MEDIUM | P1 |
| Explicit unsupported states | HIGH | LOW | P1 |
| Capability detection | HIGH | MEDIUM | P1 |
| Tray + autostart + single instance | HIGH | MEDIUM | P1 |
| Config persistence | HIGH | LOW | P1 |
| DEB + AppImage + CI/CD releases | HIGH | MEDIUM | P1 |
| Diagnostics export | MEDIUM | MEDIUM | P1 |
| Bounded history + charts | MEDIUM | MEDIUM | P1 |
| Combined presets | MEDIUM | LOW | P1 |
| Basic RGB (presets, on/off, migration) | MEDIUM | MEDIUM | P1 |
| Resource budget enforcement | HIGH | HIGH | P1 |
| Update-availability check | MEDIUM | MEDIUM | P2 |
| Theme selection, polling intervals | LOW | LOW | P2 |
| GPU VRAM/power draw prominence | MEDIUM | LOW | P2 |
| Windows telemetry | MEDIUM | HIGH | P3 |
| Manual fan curves | HIGH | HIGH | P3 |
| Full RGB editor | MEDIUM | HIGH | P3 |
| Windows hardware control | MEDIUM | HIGH | P3 |

**Priority key:**
- P1: Must have for v1 launch
- P2: Should have, add when possible (v1.x)
- P3: Future consideration (v2+)

## Competitor Feature Analysis

| Feature | OpenOMEN (legacy CLI, Python) | Windows OMEN Gaming Hub | `nbfc-linux` / patched-driver tools | Our Approach |
|---------|-------------------------------|-------------------------|-------------------------------------|--------------|
| Fan control | auto/max via `pwm1_enable`, verified read-back | Full control incl. curves, tied to HP's EC driver | Manual PWM after patching `hp-wmi`; requires kernel work | auto/max in v1, verified, capability-gated; curves later |
| Fan curves | Advertised as planned, not implemented | Yes | Partially (config files) | Explicitly deferred to v2+, model-gated |
| Monitoring | Rich CLI dashboard (CPU/GPU/temp/fans) | Yes, with graphs, in a heavyweight app | Minimal (fan-focused) | GUI with bounded-history charts, low footprint |
| RGB | Presets via JSON; most profile fields silently ignored | Full control incl. per-key | None | Presets + on/off, with honest documentation of what hardware honors |
| Power profiles | ACPI profiles + combined presets | Yes, plus Windows-specific modes | None | ACPI only, discovered dynamically, verified |
| Diagnostics | Useful `doctor` + `info` for bug reports | Logs bundled in-app | Community forum troubleshooting | One-click export the maintainer can act on |
| Footprint | CLI, negligible | Heavy (Electron-era app, hundreds of MB) | Small | ≤40 MB / ≤0.1% as a measured, published budget |
| Distribution | Self-updating binary from Releases | Windows installer | Distro packages / manual | DEB + AppImage via CI, checksums, package-manager upgrades |

**Where the gap is:** no Linux tool currently offers *both* honest, verified hardware control *and* a genuinely low-footprint GUI. The Windows OMEN Gaming Hub has the features but not the footprint or the platform; the Linux alternatives have neither the GUI nor the verification discipline.

## Sources

- OpenOMEN `.planning/codebase/` maps (ARCHITECTURE.md, CONCERNS.md, INTEGRATIONS.md) — the proven feature set, its known defects, and the `doctor`/`info` diagnostics pattern; HIGH
- OpenOMEN `README.md` — advertised features and the "planned: manual fan curves, GUI, HP-WMI EC support" list; HIGH
- User-provided ZenOmen specification (30 Oct 2026) — FR-001…FR-066 priorities, NFR budget, exclusions; HIGH (authoritative for this project)
- Arch Wiki HP Omen 16-c0140AX — fan boost via `pwm1_enable = 0`; HIGH
- Phoronix / LKML (Jan–May 2026) — `hp-wmi` manual fan support arriving for Victus S-Series with dual-channel PWM; MEDIUM
- HP support forum reports of model firmware where fan control is inaccessible under Linux; MEDIUM
- Competitor scan: `arfelious/omen-fan-control`, OMEN Gaming Hub, `nbfc-linux` positioning; MEDIUM

---
*Feature research for: ZenOmen*
*Researched: 2026-10-30*
