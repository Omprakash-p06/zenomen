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

