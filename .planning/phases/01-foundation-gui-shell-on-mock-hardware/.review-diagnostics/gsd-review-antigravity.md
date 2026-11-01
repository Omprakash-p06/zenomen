# Cross-AI Plan Review: Phase 1 — Foundation (GUI Shell on Mock Hardware)

This review evaluates implementation plans `01-01`, `01-02`, and `01-03` for **Phase 1: Foundation — GUI Shell on Mock Hardware** of ZenOmen against the repository codebase, the design contract in `.planning/phases/01-foundation-gui-shell-on-mock-hardware/01-UI-SPEC.md`, the phase research in `.planning/phases/01-foundation-gui-shell-on-mock-hardware/01-RESEARCH.md`, and project constraints in `.planning/PROJECT.md` and `.planning/ROADMAP.md`.

---

## 01-01

### 1. Summary
Plan `01-01` establishes the core project structure, compiling Rust (edition 2024) and Slint 1.17 via `build.rs`, declaring design system tokens across `ui/theme/`, writing a minimal `src/main.rs` entrypoint, and introducing GitHub Actions CI gates (`cargo fmt`, `cargo clippy -D warnings`, `cargo test`, `cargo audit`). The plan successfully prioritizes low-memory release compilation flags to prepare for the ≤40 MB RSS budget, but it introduces critical token discrepancies against the approved UI specification, omits font asset provisioning, and overlooks Linux system build dependencies needed in CI.

### 2. Strengths
- **Memory-Oriented Profile Configuration**: Configures release profile flags in `Cargo.toml` (`opt-level = "s"`, `lto = true`, `strip = true`, `panic = "abort"`, `codegen-units = 1`) as documented in `.planning/phases/01-foundation-gui-shell-on-mock-hardware/01-01-PLAN.md:30-31`, directly reinforcing the ≤40 MB peak RSS constraint defined in `.planning/PROJECT.md:14-16`.
- **Targeted GUI Feature Selection**: Directly selects `default-features = false` with `backend-winit` and `renderer-femtovg` for Slint (`01-01-PLAN.md:37`), avoiding heavy Qt or Skia runtime dependencies that would violate memory constraints as identified in `.planning/research/STACK.md:34-45`.
- **Early Quality Gates**: Mandates automated formatting, zero-warning clippy lints, unit testing, and dependency vulnerability audits (`cargo audit`) in `.github/workflows/ci.yml` (`01-01-PLAN.md:65-71`), fulfilling requirement `REL-04` (`.planning/REQUIREMENTS.md:96`) and Phase 1 Success Criterion 5 (`.planning/ROADMAP.md:36`).

### 3. Concerns
- **[HIGH] Token and Typography Divergence from Approved UI Specification**:
  - In `01-01-PLAN.md:46-49`, Task 2 specifies theme colors that directly contradict the authoritative design contract in `.planning/phases/01-foundation-gui-shell-on-mock-hardware/01-UI-SPEC.md:126-136`:
    - Obsidian is defined as `#0A0B0E` in `01-01-PLAN.md:46`, but `01-UI-SPEC.md:128` explicitly mandates `#0B0C0F`.
    - Charcoal is defined as `#12141A` in `01-01-PLAN.md:46`, but `01-UI-SPEC.md:129` mandates `#15171C`.
    - Outline is defined as `#262A34` in `01-01-PLAN.md:47`, but `01-UI-SPEC.md:131` mandates Graphite `#262A33`.
    - Accent gold is defined as `#D4A373` in `01-01-PLAN.md:48`, but `01-UI-SPEC.md:134` mandates Muted Gold `#C6A15B` (which was contrast-verified in `01-UI-SPEC.md:137` to clear WCAG 2.1 4.5:1).
    - Crimson is defined as `#E53935` in `01-01-PLAN.md:48`, but `01-UI-SPEC.md:135` mandates `#E0606B`.
    - Text primary/secondary are `#EDEDED` and `#8A909E` in `01-01-PLAN.md:49`, but `01-UI-SPEC.md:132-133` mandates `#E8E6E3` and `#9BA1AC`.
  - In `01-01-PLAN.md:52`, Task 2 declares font sizes `display: 32px`, `heading: 20px`, `body: 14px`, `label: 11px`, and `readout-mono: 24px`. This violates `01-UI-SPEC.md:102-118`, which establishes: *"Exactly four sizes, exactly two weights: Body (14px, 400), Label (12px, 400), Heading (20px, 600), Display (32px, 600)... JetBrains Mono at Display (32px) and Heading (20px) only — never at Body or Label"*.
- **[HIGH] Missing Font Assets in Repository**:
  `01-UI-SPEC.md:26` and `:31` require Inter (400, 600) and JetBrains Mono (400) embedded via `import "./fonts/Inter-Regular.ttf";` in `ui/theme/typography.slint`. However, `01-01-PLAN.md:10-20` lists no font assets under `files_modified`, and a repository search reveals zero `.ttf` files on disk. If `typography.slint` imports non-existent font paths, Slint compilation in `build.rs` (`01-01-PLAN.md:42`) will fail immediately; if omitted, Slint falls back to unaligned system fonts that break tabular figure alignment (`01-UI-SPEC.md:290`).
- **[MEDIUM] Missing Linux System Dependencies and Tooling in CI**:
  In `01-01-PLAN.md:65-71`, the CI workflow executes `cargo test` and `cargo audit` on standard runners without installing prerequisite native Linux packages (`libxkbcommon-dev`, `libfontconfig1-dev`, `libgl1-mesa-dev`) required by `winit` and `femtovg`. Furthermore, `cargo-audit` is not a pre-installed binary on default GitHub Actions runner images; without an installation step (e.g. `taiki-e/install-action@cargo-audit`), Task 4 will fail the pipeline.
- **[LOW] Ambiguity Regarding `Cargo.lock` Tracking**:
  `01-01-PLAN.md:41` instructs to *"Create .gitignore ignoring /target, Cargo.lock (if library, or track if binary...)"*. Because ZenOmen is an application binary whose dependencies must be audited by `cargo audit` in CI, `Cargo.lock` must be explicitly committed, not optionally ignored.

### 4. Suggestions
- Synchronize all token definitions in `ui/theme/colors.slint` and `ui/theme/typography.slint` with `01-UI-SPEC.md:102-136` verbatim to preserve the WCAG 2.1 contrast calculations and the 4-size typography scale.
- Include tasks in `01-01-PLAN.md` to vendor the required font files (`Inter-Regular.ttf`, `Inter-SemiBold.ttf`, `JetBrainsMono-Regular.ttf`) under `ui/theme/fonts/` and add them to `files_modified`.
- In `.github/workflows/ci.yml`, add an `apt-get install -y libxkbcommon-dev libfontconfig1-dev` step and leverage `taiki-e/install-action@cargo-audit` to ensure headless CI builds succeed.
- Explicitly mandate committing `Cargo.lock` to source control.

### 5. Risk Assessment
**MEDIUM**. The core scaffolding strategy and release profile are sound, but color/type token divergence from the design contract, missing font files leading to build failures, and missing CI runner packages create immediate obstacles in Wave 1.

---

## 01-02

### 1. Summary
Plan `01-02` constructs the application shell, integrating the six screens (`Overview`, `Cooling`, `Performance`, `Lighting`, `Settings`, `Diagnostics`), sidebar navigation, custom Slint components, and the core message-passing bridge (`AppCommand` and `AppEvent`). The plan enforces strict decoupling of UI from hardware access (`FOUND-05`), but suffers from an inter-wave type dependency where Wave 2 references data structures not defined until Wave 3, threatening compilation at the end of Wave 2.

### 2. Strengths
- **Strict UI and Core Decoupling**: Enforces `FOUND-05` (`.planning/REQUIREMENTS.md:16` and `01-02-PLAN.md:49-50`): Slint UI properties update exclusively via `slint::invoke_from_event_loop` reacting to typed `AppEvent` variants, guaranteeing that no hardware sysfs path or external CLI is accessible from UI callbacks.
- **Navigation and Zero-Capability Graceful Degradation**: Addresses `FOUND-02` and `FOUND-04` (`01-02-PLAN.md:66-83`): All six screens are reachable by mouse and keyboard (`Tab`/`Shift+Tab`, `Up`/`Down`, `Return`), and missing capabilities render contextual `UnavailablePanel` and `CapabilityNotice` components rather than blank screens or panic conditions.
- **Comprehensive Component Hierarchy**: Deploys the reusable component set defined in `01-UI-SPEC.md:44-59` (`screen-header`, `status-strip`, `status-pill`, `unavailable-panel`, `capability-notice`, `section-card`, `metric-card`, `value-readout`).

### 3. Concerns
- **[HIGH] Inter-Wave Type Dependency Breaking Wave 2 Compilation**:
  In `01-02-PLAN.md:89-93` (Task 4), `AppEvent` is defined as:
  ```rust
  pub enum AppEvent {
      ScreenChanged(ScreenId),
      CapabilitiesLoaded(HardwareCapabilities),
      TelemetryUpdated(TelemetrySnapshot),
  }
  ```
  However, `HardwareCapabilities` and `TelemetrySnapshot` are defined in `src/hardware/provider.rs` in `01-03-PLAN.md:54-57` (Wave 3). The file `src/hardware/provider.rs` is neither created nor listed in `files_modified` in `01-02-PLAN.md:13-36`. Consequently, `01-02` cannot satisfy its stated verification criterion (`01-02-PLAN.md:96`: *"App compiles and runs without warnings"*) without stubbing or pre-declaring these types in Wave 2.
- **[MEDIUM] Slint Component Import Fracture on Wave 3 Elements (`Sparkline` & `TelemetryChart`)**:
  In `01-02-PLAN.md:58`, Task 1 defines `metric-card.slint` with a "sparkline slot", and Task 3 creates `overview.slint` containing metric cards and a "wide chart slot". However, `sparkline.slint` and `telemetry-chart.slint` are authored in Plan `01-03` (`01-03-PLAN.md:69-70`). If `metric-card.slint` or `overview.slint` imports these components during Wave 2, Slint compilation fails. If placeholder layouts are used, `metric-card.slint` is missing from `files_modified` in `01-03-PLAN.md:13-27`, leaving the wave boundary integration untracked.
- **[MEDIUM] Coarse vs Subsystem-Specific Capability Handling**:
  Task 3.7 (`01-02-PLAN.md:82`) asserts: *"Verify zero-capability support: when `capabilities.is_empty`, render `UnavailablePanel` on relevant screens."*
  `HardwareCapabilities` (`01-RESEARCH.md:165-172`) is a struct of discrete boolean flags (`cpu_temperature`, `fan_rpm`, `power_profiles`, etc.), not a collection with an `.is_empty` method. More importantly, `01-UI-SPEC.md:242` and `:285-286` require that each screen's unavailable panel describe the specific missing subsystem (e.g. `hp-wmi exposes no writable pwm1_enable here`). An aggregate `is_empty` flag fails to supply the required per-subsystem granularity.
- **[LOW] Component Glyph Naming Inconsistency**:
  In `01-02-PLAN.md:55`, `icons.slint` defines the glyph `diagnostics`, whereas `01-UI-SPEC.md:58` and `:233` explicitly name the glyph `stethoscope`. Aligning the naming avoids discrepancies across documentation and templates.

### 4. Suggestions
- Declare `HardwareCapabilities` and `TelemetrySnapshot` in Wave 2 (either inside `src/hardware/provider.rs` or in a shared `src/core/types.rs` module) so `src/core/events.rs` compiles at the completion of Plan `01-02`.
- Provide stub implementations for `sparkline.slint` and `telemetry-chart.slint` in Plan `01-02`, and add `metric-card.slint` to `files_modified` in `01-03-PLAN.md` to cleanly integrate sparklines in Wave 3.
- Replace the `.is_empty` check with per-subsystem capability queries (e.g. `has_fan_control()`, `has_power_profiles()`) to supply the specific messaging mandated by `01-UI-SPEC.md:285-286`.

### 5. Risk Assessment
**MEDIUM-HIGH**. The structural design of the shell and screens is well thought out, but the missing type declarations between Wave 2 and Wave 3 will break compilation if executed in strict wave isolation.

---

## 01-03

### 1. Summary
Plan `01-03` delivers the telemetry engine, implementing the `HardwareProvider` trait, a configurable `MockHardware` provider, a background sampling worker operating on deadline intervals, rolling telemetry charts fed by a bounded 120-sample `VecDeque` buffer, and memory/frame-time benchmarking instrumentation reading `/proc/self/smaps_rollup`. The telemetry buffer and thread isolation are well-architected for the ≤40 MB RSS goal, though headless CI execution and worker thread wake-up mechanics require refinement.

### 2. Strengths
- **Non-Blocking Telemetry Engine**: Satisfies `PERF-03` (`.planning/REQUIREMENTS.md:106`) and `MON-01` (`.planning/REQUIREMENTS.md:20`): Telemetry is sampled on a dedicated background `std::thread` on a 1-second deadline schedule (`01-03-PLAN.md:73-79`), calculating CPU and sensor deltas without blocking the Slint UI thread.
- **Strictly Bounded Telemetry Buffer**: Satisfies `MON-09` (`.planning/REQUIREMENTS.md:28`) and prepares for `PERF-04` (`.planning/REQUIREMENTS.md:107`): `TelemetryHistory` wraps `VecDeque<TelemetryPoint>` capped at `MAX_SAMPLES = 120` with explicit `pop_front()` eviction before `push_back()` (`01-03-PLAN.md:64`), ensuring flat memory usage over long sessions.
- **Resilient Path Generation**: Adheres to `01-UI-SPEC.md:248-254` and `:317`: `generate_svg_path` normalizes coordinates, returns an empty string when fewer than 2 valid points exist (displaying "Waiting for first reading"), and generates `M` break commands across `None` values to represent sensor data gaps.
- **Empirical Memory Benchmarking**: In `01-03-PLAN.md:80-87`, `src/core/benchmark.rs` parses `/proc/self/smaps_rollup` for `Rss:`, `Pss:`, and `Private_Dirty:`, enabling accurate verification of cold start (<30 MB) and warm dashboard (<38 MB) RSS targets.

### 3. Concerns
- **[HIGH] Headless CI Failures on GUI Benchmark Integration Tests**:
  In `01-03-PLAN.md:91`, Task 5 introduces `tests/benchmark_tests.rs`: *"verify memory instrumentation reads process RSS accurately and stays well within the 40 MB threshold"*, verified by `cargo test --all` (`01-03-PLAN.md:99`). In standard headless CI environments (such as GitHub Actions Ubuntu runners without a display), initializing Slint's `AppWindow` with `backend-winit` will panic or error without an active X11 or Wayland display server (`$DISPLAY`/`$WAYLAND_DISPLAY`). Plan `01-01`'s CI workflow lacks `xvfb-run` or headless testing flags, which will cause `cargo test` to fail in CI.
- **[MEDIUM] Worker Thread Responsiveness to On-Demand Refresh**:
  Task 3.1 (`01-03-PLAN.md:76-78`) specifies that the worker executes on a 1-second deadline and *"Handles `AppCommand::RefreshTelemetry` to trigger immediate on-demand sampling"*. If the worker loop uses `std::thread::sleep`, it cannot respond to `RefreshTelemetry` immediately. The worker must utilize a synchronized channel receiver with timeout (`receiver.recv_timeout(deadline - now)`), resetting the deadline when an immediate sample is triggered, in order to meet `MON-10` (`.planning/REQUIREMENTS.md:29`).
- **[MEDIUM] Multi-Series Telemetry Chart Rendering**:
  In `01-03-PLAN.md:65`, Task 2 defines `generate_svg_path(samples: &[Option<f32>], width: f32, height: f32) -> String`. Per `01-UI-SPEC.md:151-158`, `TelemetryChart` plots three concurrent series on the 160px plot: CPU utilization (Gold `#C6A15B`), GPU utilization (Text Secondary `#9BA1AC`), and RAM usage (Neutral Tint `#5D6470`). The plan must explicitly generate three distinct SVG command strings from `TelemetryHistory` and bind them to three separate `Path` elements in `telemetry-chart.slint`, rather than treating the chart as a single series.
- **[LOW] Undefined Frame-Time Measurement Mechanism in Slint**:
  In `01-03-PLAN.md:86`, Task 4 calls for measuring frame times to ensure steady 60 FPS rendering. Slint 1.17 does not provide a public user API to inspect per-frame render durations. The plan should specify whether this benchmark measures Slint event loop iteration timing or UI dispatch latency to avoid ambiguous test criteria.

### 4. Suggestions
- Configure CI in `.github/workflows/ci.yml` to wrap `cargo test` in `xvfb-run` (or ensure `benchmark_tests.rs` tests the sampling engine and memory parser without requiring an active graphical window) to guarantee clean headless execution.
- Implement the worker scheduling loop using `std::sync::mpsc::Receiver::recv_timeout` to process `AppCommand::RefreshTelemetry` without blocking on sleep timers.
- Explicitly define properties in `ui/components/telemetry-chart.slint` for `cpu_path`, `gpu_path`, and `ram_path`, with `TelemetryHistory` producing all three series paths during worker updates.

### 5. Risk Assessment
**MEDIUM**. The telemetry architecture, ring buffer sizing, and memory measurement methods are technically sound and directly address the core project requirements. Addressing headless CI compatibility and channel timeout synchronization will make execution seamless.
