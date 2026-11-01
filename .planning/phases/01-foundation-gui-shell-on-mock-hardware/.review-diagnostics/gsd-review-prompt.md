# Cross-AI Plan Review Request

You are reviewing implementation plans for a software project phase.
Provide structured feedback on plan quality, completeness, and risks.

## Project Context
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

## Phase 1: Foundation GUI Shell on Mock Hardware
### Roadmap Section
### Phase 1: Foundation — GUI Shell on Mock Hardware
**Goal**: A runnable ZenOmen application whose entire UI is driven by mock hardware data, with the resource instrumentation needed to hold the 40 MB budget later.
**Mode:** mvp
**Depends on**: Nothing (first phase)
**UI hint**: yes
**Requirements**: FOUND-01, FOUND-02, FOUND-03, FOUND-04, FOUND-05, MON-09, PERF-03
**Success Criteria** (what must be TRUE):
  1. Launching the app opens the Overview page in the ZenOmen dark theme, and all six screens are reachable by sidebar click and by keyboard alone.
  2. The Overview shows plausible CPU, GPU, RAM, fan and power readings sourced entirely from the mock provider, including a rolling chart fed by a fixed-capacity history buffer (120 samples).
  3. Running the app with a mock provider configured to report zero capabilities still launches and explains what is unsupported, with no panic and no blank panels.
  4. Window frame time stays stable while the monitoring worker publishes updates — no hardware read or external command is reachable from a UI callback.
  5. Memory and CPU instrumentation logs metrics distinguishing cold start, warm dashboard, minimized tray, and peak RSS; CI fails on a formatting violation, a clippy warning, a failing test, or a dependency advisory (`cargo audit`).
**Plans**: 3 plans

Plans:
- [ ] 01-01: Rust + Slint scaffold, build pipeline, theme tokens and CI gates (`fmt`, `clippy -D warnings`, `cargo audit`)
- [ ] 01-02: Application shell — navigation, six screens, Overview bound to core events (`AppCommand`/`AppEvent`)
- [ ] 01-03: Mock hardware provider, monitoring worker, bounded history (120 samples) and benchmark instrumentation


### Requirements Addressed
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

### UI Specification & Design Contracts
---
phase: "1"
slug: "foundation-gui-shell-on-mock-hardware"
status: approved
shadcn_initialized: false
preset: none
created: "2026-10-30"
reviewed_at: "2026-10-30T17:39:59Z"
---

# Phase 1 — UI Design Contract

> Visual and interaction contract for the ZenOmen GUI shell. Generated by gsd-ui-researcher, verified by gsd-ui-checker.
> Phase 1 renders mock hardware data only. Nothing in this contract authorizes a hardware read or write from the UI layer.

---

## Design System

| Property | Value |
|----------|-------|
| Tool | none — repo-authored Slint tokens (`ui/theme/colors.slint`, `ui/theme/typography.slint`, `ui/theme/metrics.slint`); no component registry, no design-system package |
| Preset | not applicable |
| Component library | Slint 1.17 `std-widgets` (`Button`, `CheckBox`, `ScrollView`, `Slider`, `VerticalBox`, `HorizontalBox`, `GridLayout`) plus repo-authored components in `ui/components/` |
| Icon library | none — hand-authored `Path` glyphs in `ui/components/icons.slint` (20×20 box, 2px stroke, single colour bound to the surrounding text token) |
| Font | Inter (400, 600) and JetBrains Mono (400), embedded via `import "./fonts/Inter-Regular.ttf";`-style imports in `ui/theme/typography.slint` |

**Theme scope:** dark only in this phase. Every colour and type value lives in the three `ui/theme/*.slint` files and is referenced by name — no literal hex appears in a screen file. A light theme (SYS-08, Phase 4) will be a second token file, not a second set of screens.

**Slint facts this contract depends on** (verified 2026-10-30 against docs.slint.dev):
- A font is embedded by importing the `.ttf` in a `.slint` file; the family then resolves through `font-family` application-wide.
- `FocusScope` provides `focus-on-tab-navigation`, `key-pressed(event)`, `focus-changed-event(reason)` and `focus()`/`clear-focus()`; tab traversal follows declaration order.
- Non-printable keys map into the `Key` namespace (`Key.Return`, `Key.Space`, `Key.Escape`, `Key.Up`, `Key.Down`).
- `Path` accepts SVG path-data in `commands`, which is how charts are drawn (see Chart rendering below).

---

## Component Inventory

Could not enumerate: Slint 1.17 is not a dependency in this repository yet — there is no `Cargo.toml` and no `cargo` toolchain on the machine this contract was written on, so `std-widgets` cannot be listed from an installed package. Re-run at execution time and record the result: `ls ~/.cargo/registry/src/*/slint-1.17.*/widgets/*.slint | wc -l`.

The list below is therefore a **non-exhaustive** set of components this phase defines and may use. It is not a closed allowlist: any `std-widgets` element the installed Slint version exports may be used where it fits, and a new repo-authored component may be added when two screens need the same shape.

| Component | Location | Notes |
|-----------|----------|-------|
| `AppShell` | `ui/app.slint` | Window, sidebar + content split, global status strip, screen router |
| `Sidebar` / `NavRow` | `ui/components/sidebar.slint` | Six nav rows, active state, focus ring, arrow-key traversal |
| `ScreenHeader` | `ui/components/screen-header.slint` | Screen title (Heading) + right-aligned primary action slot |
| `StatusStrip` | `ui/components/status-strip.slint` | One-line global capability/mock/service summary; gold outline when it reports a degraded state |
| `MetricCard` | `ui/components/metric-card.slint` | Label, value + unit, optional sparkline, optional status pill |
| `ValueReadout` | `ui/components/value-readout.slint` | Mono numeric value + unit at Display or Heading size |
| `Sparkline` | `ui/components/sparkline.slint` | 28px `Path` trace inside a card; commands string supplied by the core |
| `TelemetryChart` | `ui/components/telemetry-chart.slint` | 160px multi-series chart, gold axis-free plot, 3 horizontal grid lines, legend |
| `StatusPill` | `ui/components/status-pill.slint` | Glyph + label + colour; the only component allowed to signal state |
| `UnavailablePanel` | `ui/components/unavailable-panel.slint` | Heading + reason + next step, replaces a control or a card body |
| `CapabilityNotice` | `ui/components/capability-notice.slint` | Inline raised-surface notice: what is unsupported here, and why |
| `SectionCard` | `ui/components/section-card.slint` | Title + body container with 1px outline, no shadow |
| `Icon` | `ui/components/icons.slint` | Named glyph set: `enso`, `cpu`, `gpu`, `memory`, `fan`, `power`, `keyboard`, `settings`, `stethoscope`, `warning`, `unavailable` |

---

## Spacing Scale

Declared values (all multiples of 4):

| Token | Value | Usage |
|-------|-------|-------|
| xs | 4px | Gap between a numeric value and its unit; icon-to-label gap |
| sm | 8px | Compact stack spacing inside a card; pill padding |
| md | 16px | Card padding; grid gap between metric cards; card-internal row spacing |
| lg | 24px | Page padding; gap between card grid and chart; header horizontal padding |
| xl | 32px | Gap between major blocks (hero row → chart → footer) |
| 2xl | 48px | Reserved for empty-state and notice vertical breathing room |
| 3xl | 64px | Page-level top offset when a screen has no header action |

Exceptions (structural dimensions, all multiples of 4 — none are ad-hoc spacing):

| Value | Purpose |
|-------|---------|
| 200px | Sidebar width, fixed at every window size |
| 64px | Header height |
| 40px | Status strip height; minimum height of a sidebar nav row and of any interactive target |
| 220px | Minimum metric-card width (drives the responsive column count below) |
| 28px | Sparkline height |
| 160px | Telemetry chart plot height |
| 1px / 2px | Stroke widths: 1px outlines and grid lines, 2px focus ring and chart strokes. Stroke widths are not spacing tokens |

Responsive column count for the Overview card grid (content width = window width − 200px sidebar − 48px page padding, gap 16px):

| Content width | Columns |
|---------------|---------|
| ≥ 928px | 4 |
| ≥ 692px | 3 |
| ≥ 456px | 2 |
| < 456px | 1 |

Default window 1180×760, minimum 940×620 — at the default the 4-column rule applies, at the minimum the 3-column rule applies.

---

## Typography

Exactly four sizes, exactly two weights.

| Role | Size | Weight | Line Height |
|------|------|--------|-------------|
| Body | 14px | 400 | 1.5 |
| Label | 12px | 400 | 1.4 |
| Heading | 20px | 600 | 1.25 |
| Display | 32px | 600 | 1.2 |

| Rule | Value |
|------|-------|
| Families | Inter for all prose; JetBrains Mono for every numeric readout (values, RPM, watts, °C, percentages, version strings) |
| Mono sizes | JetBrains Mono at Display (32px) and Heading (20px) only — never at Body or Label |
| Numeric alignment | Tabular figures; values left-aligned in cards so digits line up down a column |
| Label treatment | 12px, uppercase, letter-spacing 0.04em, `text-secondary` — used for card labels, units and table headers |
| Weight usage | 600 only for Display, Heading and the active sidebar row label; 400 everywhere else. No 300/500/700/800 in this phase |

---

### Research Findings
# Phase 1 Research: Foundation — GUI Shell on Mock Hardware

**Phase:** 1 of 5  
**Goal:** A runnable ZenOmen application whose entire UI is driven by mock hardware data, with the resource instrumentation needed to hold the 40 MB budget later.  
**Mode:** MVP / Tracer-first  
**Status:** Completed  
**Confidence:** HIGH  

---

## 1. Executive Overview & Scope

Phase 1 establishes the compiled Rust + Slint desktop application skeleton. Because actual hardware interaction is isolated behind the `HardwareProvider` trait, Phase 1 can be developed, tested, and benchmarked without needing HP OMEN/Victus hardware.

### Requirements Covered
- **FOUND-01:** User can launch ZenOmen and see the Overview page rendered in the ZenOmen dark theme.
- **FOUND-02:** Every screen (Overview, Cooling, Performance, Lighting, Settings, Diagnostics) is reachable from the sidebar and by keyboard navigation.
- **FOUND-03:** The application runs end-to-end against a mock hardware provider, so the full UI is usable and testable without the target laptop.
- **FOUND-04:** On a machine where no hardware capability is detected, the app still launches and explains what is unsupported instead of erroring or showing blank panels.
- **FOUND-05:** UI state changes only in response to typed core events (`AppEvent`); no hardware file or external command is ever read from the UI layer.
- **MON-09:** User can see rolling telemetry charts fed from the bounded history buffer (120 samples).
- **PERF-03:** Hardware sampling never blocks the UI thread; window frame time stays stable while telemetry refreshes.

---

## 2. Technology Stack & Build Pipeline

### 2.1 Dependencies (`Cargo.toml`)
```toml
[package]
name = "zenomen"
version = "0.1.0"
edition = "2024"
authors = ["Omprakash <p06>"]
license = "GPL-3.0-only"
description = "Lightweight hardware control center for HP OMEN and Victus laptops"

[dependencies]
# Slint GUI toolkit - pinned to 1.17
slint = { version = "1.17", default-features = false, features = [
    "std",
    "compat-1-2",
    "backend-winit",
    "renderer-femtovg"
] }

# Core utilities
thiserror = "2.0"
anyhow = "1.0"
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["fmt", "env-filter"] }
directories = "5.0"
serde = { version = "1.0", features = ["derive"] }
toml = "0.8"

[build-dependencies]
slint-build = "1.17"

[dev-dependencies]
# For testing and benchmarks
criterion = "0.5"

[profile.release]
opt-level = "s"      # Optimize for size/memory
lto = true           # Link-time optimization
codegen-units = 1    # Better dead-code stripping and inlining
panic = "abort"      # Strip unwinding tables
strip = true         # Strip symbols from final binary
```

### 2.2 Build Script (`build.rs`)
```rust
fn main() {
    let config = slint_build::CompilerConfiguration::new()
        .with_style("fluent".into()); // Base style, themed via our tokens
    slint_build::compile_with_config("ui/app.slint", config).expect("Slint build failed");
}
```

---

## 3. UI System & Design Tokens (Strict Alignment with `01-UI-SPEC.md`)

The UI contract defined in [`01-UI-SPEC.md`](file:///home/omprakash/Documents/zenomen/.planning/phases/01-foundation-gui-shell-on-mock-hardware/01-UI-SPEC.md) is authoritative:

### 3.1 Theme Tokens (`ui/theme/`)
- `colors.slint`:
  - Surfaces: Obsidian `#0A0B0E` (window background), Charcoal `#12141A` (cards), Slate `#1B1E25` (hover/interactive).
  - Outlines: Outline `#262A34` (1px borders, dividers, grid lines).
  - Accents: Crimson `#E53935` (primary accents, critical temps), Amber/Muted Gold `#D4A373` (focus ring, notices, secondary indicators).
  - Text: Primary `#EDEDED`, Secondary `#8A909E`, Disabled `#4E5360`.
- `typography.slint`:
  - Embedded fonts: Inter (Regular 400, SemiBold 600) and JetBrains Mono (400).
  - Type scale: Display (32px, 700), Heading (20px, 600), Body (14px, 400), Label (11px, 600 uppercase with letter-spacing), Readout Mono (24px, 400).
- `metrics.slint`:
  - Multiples of 4: `xs` (4px), `sm` (8px), `md` (16px), `lg` (24px), `xl` (32px), `2xl` (48px).
  - Structural constants: Sidebar width `200px`, Header height `64px`, StatusStrip height `40px`, Min Card Width `220px`, Sparkline height `28px`, Chart plot height `160px`.
  - Window defaults: `1180x760` (4 columns), minimum `940x620` (3 columns).

### 3.2 Component Inventory (`ui/components/`)
1. `icons.slint`: Hand-crafted SVG path glyphs (20x20 box, 2px stroke): `cpu`, `gpu`, `memory`, `fan`, `power`, `keyboard`, `settings`, `diagnostics`, `unavailable`, `warning`, `enso`.
2. `sidebar.slint`: 6 navigation items (`Overview`, `Cooling`, `Performance`, `Lighting`, `Settings`, `Diagnostics`), active left rail (2px gold), keyboard Up/Down focus.
3. `screen-header.slint`: Screen title + CTA action slot (`Refresh Telemetry`).
4. `status-strip.slint`: Global capability/mock summary at bottom (`40px`).
5. `metric-card.slint`: Label, numeric value + unit, optional status pill, optional sparkline.
6. `value-readout.slint`: Mono value + unit; wraps unit to new line if text overflows.
7. `sparkline.slint`: 28px height, 2px stroke `Path` accepting SVG path commands string (`M x y L x y ...`).
8. `telemetry-chart.slint`: 160px height multi-series chart with 3 horizontal grid lines, legend items, and "Waiting for first reading" state.
9. `status-pill.slint`: Glyph + label + semantic color.
10. `unavailable-panel.slint`: Heading + reason + next step (replaces cards/controls when unsupported).
11. `capability-notice.slint`: Inline notice explaining features arriving in future phases.
12. `section-card.slint`: Container with 1px outline.

### 3.3 Screen Specifications
- **Overview:** 4 hero metric cards (CPU, GPU, RAM, Fan) + wide `TelemetryChart` (160px) + Header with `Refresh Telemetry`.
- **Cooling:** Fan RPM readout rows + read-only fan mode status pill + `CapabilityNotice` ("Fan control arrives with the verified hardware service in Phase 3").
- **Performance:** Advertised ACPI profiles list + active profile pill + `CapabilityNotice` ("Applying a profile arrives with the verified hardware service in Phase 3").
- **Lighting:** RGB support state pill + active profile name + `CapabilityNotice` ("Applying a lighting preset arrives with the verified hardware service in Phase 4").
- **Settings:** Version, provider mode (`mock`), theme token display + `CapabilityNotice` ("Preferences persist in Phase 4").
- **Diagnostics:** Sensor capability summary table + `CapabilityNotice` ("Export arrives in Phase 2").

### Plans to Review
#### .planning/phases/01-foundation-gui-shell-on-mock-hardware/01-01-PLAN.md
---
phase: "01-foundation-gui-shell-on-mock-hardware"
plan: "01"
type: implementation
prefix: "01-01"
wave: 1
depends_on: []
requirements:
  - FOUND-01
files_modified:
  - Cargo.toml
  - build.rs
  - .gitignore
  - .github/workflows/ci.yml
  - ui/theme/colors.slint
  - ui/theme/typography.slint
  - ui/theme/metrics.slint
  - ui/app.slint
  - src/main.rs
autonomous: true
---

# Plan 01-01: Rust + Slint Scaffold, Build Pipeline, Theme Tokens & CI Gates

## Objective
Establish the compiled Rust (edition 2024) + Slint 1.17 project structure with theme tokens matching `01-UI-SPEC.md`, minimal application entry point, `.gitignore`, and GitHub Actions CI quality gates (`cargo fmt`, `cargo clippy -D warnings`, `cargo test`, `cargo audit`).

## Context & Design Rules
- Follow [`01-UI-SPEC.md`](file:///home/omprakash/Documents/zenomen/.planning/phases/01-foundation-gui-shell-on-mock-hardware/01-UI-SPEC.md) for exact theme hex values, font families (Inter, JetBrains Mono), spacing scale (multiples of 4), and structural dimensions.
- Use release profile optimizations in `Cargo.toml` (`opt-level = "s"`, `lto = true`, `strip = true`, `panic = "abort"`, `codegen-units = 1`) to uphold the ≤40 MB RSS budget.
- Pin Slint and `slint-build` to `1.17`.

## Tasks

### Task 1: Initialize Rust Project & Dependencies
1. Create `Cargo.toml` configured for Rust edition 2024, package name `zenomen`, with dependencies:
   - `slint = { version = "1.17", default-features = false, features = ["std", "compat-1-2", "backend-winit", "renderer-femtovg"] }`
   - `slint-build = "1.17"` (build dependency)
   - `thiserror`, `anyhow`, `tracing`, `tracing-subscriber`, `directories`, `serde`, `toml`
   - Release profile flags for size and memory optimization.
2. Create `.gitignore` ignoring `/target`, `Cargo.lock` (if library, or track if binary; binary should track or ignore target), `.venv/`, `build/`, `dist/`.
3. Create `build.rs` compiling `ui/app.slint` with `slint_build::compile_with_config`.

### Task 2: Implement Theme Tokens (`ui/theme/`)
1. Create `ui/theme/colors.slint` defining global palette exported as `ThemeColors`:
   - Surfaces: `obsidian: #0A0B0E`, `charcoal: #12141A`, `slate: #1B1E25`.
   - Borders/Grid: `outline: #262A34`.
   - Accents: `crimson: #E53935`, `gold: #D4A373`.
   - Text: `text-primary: #EDEDED`, `text-secondary: #8A909E`, `text-disabled: #4E5360`.
2. Create `ui/theme/typography.slint` defining `ThemeTypography`:
   - Font family references (`Inter`, `JetBrains Mono`).
   - Font sizes: `display: 32px`, `heading: 20px`, `body: 14px`, `label: 11px`, `readout-mono: 24px`.
3. Create `ui/theme/metrics.slint` defining `ThemeMetrics`:
   - Spacing: `xs: 4px`, `sm: 8px`, `md: 16px`, `lg: 24px`, `xl: 32px`, `2xl: 48px`.
   - Layout: `sidebar-width: 200px`, `header-height: 64px`, `status-strip-height: 40px`, `min-card-width: 220px`, `sparkline-height: 28px`, `chart-height: 160px`.
   - Window defaults: `default-width: 1180px`, `default-height: 760px`, `min-width: 940px`, `min-height: 620px`.

### Task 3: Minimal App Shell & Rust Entry Point
1. Create initial `ui/app.slint` importing theme tokens, creating `export component AppWindow inherits Window` with dark obsidian background, default/minimum dimensions, and a placeholder Overview title.
2. Create `src/main.rs`:
   - Initialize `tracing_subscriber`.
   - Call `AppWindow::new()?.run()?`.

### Task 4: Configure GitHub Actions CI (`.github/workflows/ci.yml`)
1. Setup CI workflow triggered on push/PR to `main`.
2. Job steps:
   - Check formatting: `cargo fmt --all -- --check`
   - Lint with zero warnings: `cargo clippy --all-targets --all-features -- -D warnings`
   - Run unit/integration tests: `cargo test --all`
   - Security audit: `cargo audit`

## Verification
- `cargo check` passes without warnings.
- `cargo fmt --check` succeeds.
- Theme tokens match `01-UI-SPEC.md` color definitions.


#### .planning/phases/01-foundation-gui-shell-on-mock-hardware/01-02-PLAN.md
---
phase: "01-foundation-gui-shell-on-mock-hardware"
plan: "02"
type: implementation
prefix: "01-02"
wave: 2
depends_on:
  - "01-01"
requirements:
  - FOUND-02
  - FOUND-04
  - FOUND-05
files_modified:
  - src/core/mod.rs
  - src/core/events.rs
  - src/core/state.rs
  - src/app.rs
  - ui/components/icons.slint
  - ui/components/sidebar.slint
  - ui/components/screen-header.slint
  - ui/components/status-strip.slint
  - ui/components/status-pill.slint
  - ui/components/unavailable-panel.slint
  - ui/components/capability-notice.slint
  - ui/components/section-card.slint
  - ui/components/metric-card.slint
  - ui/components/value-readout.slint
  - ui/screens/overview.slint
  - ui/screens/cooling.slint
  - ui/screens/performance.slint
  - ui/screens/lighting.slint
  - ui/screens/settings.slint
  - ui/screens/diagnostics.slint
  - ui/app.slint
  - src/main.rs
autonomous: true
---

# Plan 01-02: Application Shell, Navigation, Six Screens & Event Bridge

## Objective
Implement the full Slint UI shell with all six screens (`Overview`, `Cooling`, `Performance`, `Lighting`, `Settings`, `Diagnostics`), keyboard navigation, reusable components, zero-capability degradation handling, and the Rust core `AppCommand`/`AppEvent` message-passing bridge.

## Context & Design Rules
- Adhere strictly to [`01-UI-SPEC.md`](file:///home/omprakash/Documents/zenomen/.planning/phases/01-foundation-gui-shell-on-mock-hardware/01-UI-SPEC.md):
  - Sidebar: fixed `200px` width, 6 nav rows with 2px gold rail on active row, Up/Down arrow key traversal.
  - Keyboard navigation: declaration order `Tab` / `Shift+Tab`, `Return`/`Space` activation, `Escape` clears focus to active sidebar row.
  - Responsive layout: 4 columns at ≥928px content width (window 1180px), 3 columns at ≥692px content width (window 940px min size).
  - No hardware access from the UI: all state arrives via typed `AppEvent`s into Slint callbacks/properties (**FOUND-05**).
  - Zero-capability run (**FOUND-04**): if zero capabilities are reported, navigation and shell remain interactive, while content areas render `UnavailablePanel` stating the missing interfaces.

## Tasks

### Task 1: Reusable Slint Component Library (`ui/components/`)
1. Create `ui/components/icons.slint`: 20×20 hand-crafted SVG `Path` glyphs for `enso`, `cpu`, `gpu`, `memory`, `fan`, `power`, `keyboard`, `settings`, `diagnostics`, `warning`, `unavailable`.
2. Create `ui/components/status-pill.slint`: semantic status indicator (Normal, Degraded, Unavailable).
3. Create `ui/components/value-readout.slint`: monospace numeric readout that wraps the unit to a second line if space is constrained.
4. Create `ui/components/metric-card.slint`: card with title, readout, status pill, and sparkline slot.
5. Create `ui/components/section-card.slint`: container with 1px Outline border and Charcoal background.
6. Create `ui/components/screen-header.slint`: screen title + right-aligned action button slot.
7. Create `ui/components/status-strip.slint`: 40px bottom strip showing global status summary.
8. Create `ui/components/unavailable-panel.slint`: heading, reason, and next-step button for unsupported features.
9. Create `ui/components/capability-notice.slint`: inline banner explaining features arriving in later phases.

### Task 2: Implement Navigation & App Shell (`ui/components/sidebar.slint`, `ui/app.slint`)
1. Create `ui/components/sidebar.slint`:
   - 6 items: Overview, Cooling, Performance, Lighting, Settings, Diagnostics.
   - `FocusScope` with `key-pressed` handling Up/Down arrows and Return/Space activation.
   - Selected state: 2px gold left rail + font-weight 600.
2. In `ui/app.slint`:
   - Compose `Sidebar`, `ScreenHeader`, content viewport, and `StatusStrip`.
   - Maintain `active-screen` index property.
   - Support `Tab`/`Shift+Tab` traversing sidebar -> header action -> screen controls.

### Task 3: Implement All Six Screens (`ui/screens/`)
1. `overview.slint`: responsive grid of 4 metric cards (CPU, GPU, RAM, Fan) + wide chart slot.
2. `cooling.slint`: fan RPM rows, read-only mode pill, and `CapabilityNotice` explaining fan control arrives in Phase 3.
3. `performance.slint`: list of advertised ACPI profiles, active profile pill, and `CapabilityNotice`.
4. `lighting.slint`: RGB interface status pill, active profile display, and `CapabilityNotice`.
5. `settings.slint`: version readout, provider mode (`mock`), theme (`Dark`), and `CapabilityNotice`.
6. `diagnostics.slint`: capability summary table and `CapabilityNotice`.
7. Verify zero-capability support: when `capabilities.is_empty`, render `UnavailablePanel` on relevant screens.

### Task 4: Rust Event Bridge (`src/core/events.rs`, `src/app.rs`)
1. Define `AppCommand` in `src/core/events.rs`:
   - `Navigate(ScreenId)`
   - `RefreshTelemetry`
   - `SetHistoryWindow(usize)`
2. Define `AppEvent` in `src/core/events.rs`:
   - `ScreenChanged(ScreenId)`
   - `CapabilitiesLoaded(HardwareCapabilities)`
   - `TelemetryUpdated(TelemetrySnapshot)`
3. Create `src/app.rs` bridging Slint callbacks and UI property updates via `slint::invoke_from_event_loop`.

## Verification
- App compiles and runs without warnings.
- All six screens can be navigated via sidebar clicks and keyboard (`Tab`, `Up`/`Down`, `Enter`).
- Window resizes cleanly from 1180×760 to 940×620, reflowing Overview cards from 4 to 3 columns without clipping.
- Zero-capability test renders `UnavailablePanel` without panic or blank screens.


#### .planning/phases/01-foundation-gui-shell-on-mock-hardware/01-03-PLAN.md
---
phase: "01-foundation-gui-shell-on-mock-hardware"
plan: "03"
type: implementation
prefix: "01-03"
wave: 3
depends_on:
  - "01-02"
requirements:
  - FOUND-03
  - MON-09
  - PERF-03
files_modified:
  - src/hardware/mod.rs
  - src/hardware/provider.rs
  - src/hardware/mock.rs
  - src/core/telemetry.rs
  - src/core/worker.rs
  - src/core/benchmark.rs
  - ui/components/sparkline.slint
  - ui/components/telemetry-chart.slint
  - ui/screens/overview.slint
  - tests/mock_hardware_tests.rs
  - tests/telemetry_history_tests.rs
  - tests/benchmark_tests.rs
  - src/main.rs
autonomous: true
---

# Plan 01-03: Mock Provider, Monitoring Worker, Bounded History Charts & Benchmark Instrumentation

## Objective
Deliver a runnable, end-to-end ZenOmen application driven entirely by a configurable mock hardware provider (**FOUND-03**), with a non-blocking background monitoring worker (**PERF-03**), rolling telemetry charts backed by a bounded 120-sample history ring buffer (**MON-09**), and empirical memory/frame-time benchmarking instrumentation.

## Context & Design Rules
- Follow [`01-UI-SPEC.md`](file:///home/omprakash/Documents/zenomen/.planning/phases/01-foundation-gui-shell-on-mock-hardware/01-UI-SPEC.md):
  - `Sparkline`: 28px height, 2px stroke `Path` with no axes or labels.
  - `TelemetryChart`: 160px height plot, 3 horizontal grid lines at 1px outline, gold series strokes (2px), legend row.
  - Fewer than 2 samples renders "Waiting for first reading" text instead of a flat 0 line.
  - Sensor data gaps (missing samples) render as path breaks (`M` commands) rather than connecting across gaps.
  - Absent sensors render no sparkline.
- Strictly bounded memory (**PERF-04** preparation):
  - `VecDeque<TelemetryPoint>` capped at 120 entries; `pop_front()` before `push_back()`.
  - Update Slint model properties in place rather than rebuilding arrays on every tick.
- Non-blocking execution (**PERF-03**):
  - Monitoring worker runs in a separate `std::thread`, scheduled on deadlines.
  - Computes CPU deltas without inline sleeps; Slint main thread never waits on sampling.
- Benchmark instrumentation:
  - Parses `/proc/self/smaps_rollup` (RSS, PSS, Private Dirty) to measure cold start, warm dashboard, minimized tray, and peak RSS.

## Tasks

### Task 1: Hardware Provider Trait & Mock Provider (`src/hardware/`)
1. Create `src/hardware/provider.rs` defining:
   - `HardwareCapabilities` struct.
   - `TelemetrySnapshot` struct (CPU model, utilization, temp, freq, GPU name, util, temp, RAM total/used, Fan RPMs, Fan Mode, ACPI profile).
   - `HardwareProvider` trait: `fn capabilities(&self) -> HardwareCapabilities;` and `fn sample_telemetry(&self) -> Result<TelemetrySnapshot, String>;`.
2. Create `src/hardware/mock.rs` implementing `MockHardware`:
   - Configurable scenarios: `Nominal`, `ZeroCapabilities`, `SensorDataGap` (drops a sensor for 3 ticks to verify UI trace gap handling).
   - Simulates realistic oscillating metrics: CPU 15–45%, GPU 30–70%, RAM 8192/16384 MB (using `MemAvailable` math), Fans 2200–4200 RPM.

### Task 2: Bounded Telemetry Buffer & SVG Path Generator (`src/core/telemetry.rs`)
1. Create `src/core/telemetry.rs`:
   - `TelemetryHistory`: wraps `VecDeque<TelemetryPoint>` with `MAX_SAMPLES = 120`.
   - Pure function `generate_svg_path(samples: &[Option<f32>], width: f32, height: f32) -> String`:
     - Normalizes values to plot dimensions (inverting Y for SVG coordinates).
     - Returns empty string when `< 2` valid points exist.
     - Emits `M` (move) commands across `None` values to create visible breaks in plotted traces.
2. Create `ui/components/sparkline.slint`: 28px height `Path` binding `commands` property.
3. Create `ui/components/telemetry-chart.slint`: 160px height container with gridlines, series `Path`, legend items, and "Waiting for first reading" fallback.
4. Integrate Sparkline into `Overview` metric cards and `TelemetryChart` into the overview wide chart slot.

### Task 3: Deadline-Scheduled Monitoring Worker (`src/core/worker.rs`)
1. Create `src/core/worker.rs`:
   - Spawns a background `std::thread`.
   - Executes on a 1-second deadline schedule (`Instant::now() + Duration::from_secs(1)`).
   - Samples `MockHardware`, pushes to `TelemetryHistory`, generates SVG path strings, and schedules UI updates via `slint::invoke_from_event_loop`.
   - Handles `AppCommand::RefreshTelemetry` to trigger immediate on-demand sampling.

### Task 4: Benchmark & Memory Instrumentation (`src/core/benchmark.rs`)
1. Create `src/core/benchmark.rs`:
   - Helper parsing `/proc/self/smaps_rollup` (or `/proc/self/status` if rollup unavailable) for RSS, PSS, and Private_Dirty.
   - Logs memory benchmarks distinguishing:
     - Cold start RSS (initial window map).
     - Warm dashboard RSS (after 120 samples).
   - Measures frame times to ensure UI rendering remains steady at 60 FPS while the worker pushes updates.

### Task 5: Automated Test Suite (`tests/`)
1. `tests/mock_hardware_tests.rs`: verify all mock scenarios produce valid, non-zero telemetry.
2. `tests/telemetry_history_tests.rs`: verify `TelemetryHistory` maintains exactly 120 samples on overflow and SVG path generator handles breaks properly.
3. `tests/benchmark_tests.rs`: verify memory instrumentation reads process RSS accurately and stays well within the 40 MB threshold.

## Verification
- Running `cargo run` launches ZenOmen displaying dynamic, plausible readings and live rolling charts in the dark theme.
- Triggering `ZeroCapabilities` mock mode launches cleanly and shows `UnavailablePanel` across screens without crashing (**FOUND-04**).
- Triggering `SensorDataGap` produces a visible break in the chart trace as specified in `01-UI-SPEC.md`.
- Window frame time stays stable during telemetry updates (**PERF-03**).
- Process RSS starts <30 MB and stays <38 MB with 120 samples populated.
- `cargo test --all` passes 100%.


## Review Instructions

**Verify against source — do not review the plan text in isolation.** The plans reference real files, configurations, and scripts in this repo.
1. Open the referenced files and check each claim against the actual code.
2. For every strength or concern, cite concrete `path/to/file:line` evidence plus the mechanism.
3. When a plan asserts a mechanism works (a guard, a metric, a test that exercises a path), trace whether it actually does what is claimed — do not take the plan's word for it.
4. If you cannot read the repo (no file access), say so and downgrade that finding to an open question rather than asserting it.

Findings citing `file:line` evidence are weighted far more heavily than impressionistic ones; a review that only restates the plan's own claims has low value.

**Plan coverage is mandatory.** Give every listed id in the Plan Coverage Manifest its own `##`-level section headed with that id verbatim (e.g. `## 01-01`, `## 01-02`, `## 01-03`).

Analyze each plan and provide:
1. **Summary** — One-paragraph assessment
2. **Strengths** — What's well-designed (bullet points citing file:line evidence)
3. **Concerns** — Potential issues, gaps, risks (bullet points with severity: HIGH/MEDIUM/LOW citing file:line evidence)
4. **Suggestions** — Specific improvements (bullet points)
5. **Risk Assessment** — Overall risk level (LOW/MEDIUM/HIGH) with justification

Focus on:
- Missing edge cases or error handling
- Dependency ordering issues across waves
- Scope creep or over-engineering
- Security considerations & least privilege
- Performance implications (memory budget ≤40MB RSS, non-blocking UI thread)
- Whether the plans actually achieve the phase goals and success criteria

Output your review in markdown format.

## Plan Coverage Manifest

Total plans in this review: 3

Plan ids (give each one its own `##`-level section, headed verbatim):
- 01-01
- 01-02
- 01-03
