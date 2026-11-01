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

---

## 4. Message-Passing Architecture (`AppCommand` / `AppEvent`)

To satisfy **FOUND-05** and **PERF-03**, the UI never accesses hardware or performs blocking work.

### 4.1 Event Bridge
```rust
// In src/core/events.rs
pub enum AppCommand {
    Navigate(ScreenId),
    RefreshTelemetry,
    SetHistoryWindow(usize),
}

pub enum AppEvent {
    CapabilitiesLoaded(HardwareCapabilities),
    TelemetryUpdated(TelemetrySnapshot),
    NavigationChanged(ScreenId),
    ErrorOccurred(String),
}
```

### 4.2 Slint Thread Handoff
Slint provides `slint::invoke_from_event_loop`. The background worker samples the mock provider on a deadline timer (1 Hz), publishes a `TelemetrySnapshot`, pushes it to the bounded `TelemetryHistory`, generates the SVG path strings for the charts, and schedules a UI model update:
```rust
let ui_handle = app_window.as_weak();
slint::invoke_from_event_loop(move || {
    if let Some(ui) = ui_handle.upgrade() {
        ui.set_cpu_utilization(snapshot.cpu_utilization);
        ui.set_sparkline_cpu(sparkline_svg.into());
        // Mutate model properties in place
    }
}).unwrap();
```

---

## 5. Mock Hardware Provider & Telemetry Engine

### 5.1 Provider Traits (`src/hardware/provider.rs`)
```rust
#[derive(Debug, Clone, Default)]
pub struct HardwareCapabilities {
    pub cpu_temperature: bool,
    pub gpu_temperature: bool,
    pub fan_rpm: bool,
    pub fan_control: bool,
    pub power_profiles: bool,
    pub keyboard_rgb: bool,
}

#[derive(Debug, Clone)]
pub struct TelemetrySnapshot {
    pub timestamp: std::time::Instant,
    pub cpu_name: String,
    pub cpu_utilization: f32,
    pub cpu_temperature: Option<f32>,
    pub cpu_frequency_mhz: u32,
    pub gpu_name: Option<String>,
    pub gpu_utilization: Option<f32>,
    pub gpu_temperature: Option<f32>,
    pub ram_total_mb: u64,
    pub ram_used_mb: u64,
    pub fan_rpms: Vec<u32>,
    pub fan_mode: String,
    pub power_profile: String,
}

pub trait HardwareProvider: Send + Sync {
    fn capabilities(&self) -> HardwareCapabilities;
    fn sample_telemetry(&self) -> Result<TelemetrySnapshot, String>;
}
```

### 5.2 `MockHardware` Implementation
Configurable with scenarios:
1. `Nominal`: Plausible oscillating readings (CPU 15-45%, GPU 30-70%, Fans 2200-4200 RPM, RAM 8192/16384 MB).
2. `ZeroCapabilities`: All capabilities set to `false` (testing **FOUND-04** zero-capability graceful degradation).
3. `SensorDataGap`: Temporarily drops a sensor for 3 ticks to verify trace gap rendering in charts (verifying `01-UI-SPEC.md` held-out test).

### 5.3 Bounded Telemetry History & SVG Path Generation
- Capacity: exactly 120 samples (`VecDeque<TelemetryPoint>`). When full, `pop_front()` before `push_back()`.
- SVG Path Generator: Pure function `generate_svg_path(data: &[f32], width: f32, height: f32) -> String`.
  - Format: `M x0 y0 L x1 y1 L x2 y2 ...`
  - Normalizes min/max to `[0, height]` with y inverted for SVG coordinate space.
  - If data has `< 2` samples, returns empty string so UI renders "Waiting for first reading".
  - If sensor is `None` (unavailable), produces breaks in the path (`M` command to resume).

---

## 6. Benchmarking & Memory Instrumentation (PERF-01, PERF-02, PERF-03)

### 6.1 Resident Set Size (RSS) Measurement
ZenOmen must strictly distinguish memory stages:
- **Cold start RSS:** < 30 MB (measured immediately after window map).
- **Warm dashboard RSS:** < 38 MB (measured after 10 minutes of active 1 Hz updates).
- **Minimized tray RSS:** < 25 MB (window hidden, sampling throttled).
- **Peak RSS:** ≤ 40 MB.

Instrumentation module `src/core/benchmark.rs` reads `/proc/self/smaps_rollup`:
- Parses `Rss:`, `Pss:`, `Private_Dirty:`.
- Measures frame times to ensure background mock updates never cause frame drops (`PERF-03`).

---

## 7. CI Pipeline (`.github/workflows/ci.yml`)

Automates strict gates from Day 1:
1. **Formatting:** `cargo fmt --all -- --check`
2. **Clippy:** `cargo clippy --all-targets --all-features -- -D warnings`
3. **Tests:** `cargo test --all`
4. **Dependency Audit:** `cargo audit` (enforces no unresolved vulnerabilities)

---

## 8. Plan Decomposition

Phase 1 breaks down into three executable plans:

1. **`01-01-PLAN.md` (Scaffolding, Build Pipeline, Theme Tokens, CI Gates)**
   - Setup `Cargo.toml` with edition 2024 and Slint 1.17.
   - Setup `build.rs` and Slint compilation.
   - Author theme tokens (`ui/theme/colors.slint`, `typography.slint`, `metrics.slint`).
   - Setup `.github/workflows/ci.yml` with `fmt`, `clippy -D warnings`, `test`, `audit`.

2. **`01-02-PLAN.md` (Application Shell, Navigation, Six Screens, Event Bridge)**
   - Author `ui/components/` (sidebar, screen-header, status-strip, status-pill, unavailable-panel, capability-notice, section-card, icons).
   - Implement `AppCommand` and `AppEvent` message-passing bridge.
   - Author all 6 screens with keyboard navigation (`Tab`, `Up`/`Down`, `Return`, `Escape`).
   - Wire Overview screen properties to Slint models.

3. **`01-03-PLAN.md` (Mock Provider, Worker Thread, Bounded Charts, Memory Instrumentation)**
   - Implement `HardwareProvider` trait and `MockHardware` with scenarios (Nominal, ZeroCapability, DataGap).
   - Implement deadline-scheduled background worker thread.
   - Implement 120-sample `VecDeque` ring buffer and SVG path generator for Sparkline and TelemetryChart.
   - Implement `/proc/self/smaps_rollup` memory logging and frame-time stability test.
   - Verify FOUND-01 through FOUND-05, MON-09, PERF-03 end-to-end.

---
*Phase 1 Research completed for: ZenOmen*

