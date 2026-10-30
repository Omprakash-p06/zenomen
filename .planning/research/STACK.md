# Stack Research

**Domain:** Native Linux desktop system utility (hardware control + telemetry) with a cross-platform GUI  
**Researched:** 2026-10-30  
**Updated:** 2026-10-30 (incorporated review corrections: memory precision, RAM from scratch, dynamic capability detection, mandatory polkit)  
**Confidence:** HIGH for Rust/Slint stack and D-Bus boundaries; MEDIUM-HIGH for memory measurement and packaging toolchains

## Recommended Stack

### Core Technologies

| Technology | Version | Purpose | Why Recommended |
|------------|---------|---------|-----------------|
| Rust | edition 2024, stable toolchain | All application logic, hardware access, scheduling | Memory safety without a GC (essential for a ≤40 MB UI budget), single static binary, and the language the hardware-adjacent ecosystem (`zbus`, `sysinfo`, `nvml`) is written in |
| Slint | 1.17.x | Declarative GUI compiled to native code | Compiled `.slint` markup with no webview and no JS runtime. 1.17 (2026-06-24) added `SystemTrayIcon`, `Tooltip`, `RadioGroup`. Slint's "<300 KiB runtime" is a toolkit baseline; whole-application RSS (fonts, renderer, buffers, models) is budgeted at ≤40 MB peak and must be measured empirically |
| Cargo | stable | Build, dependency, release profiles | `opt-level = "s"`, `lto = true`, `codegen-units = 1`, `strip = true`, `panic = "abort"` all directly serve the memory/size budget |

### Supporting Libraries

| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| `slint` + `slint-build` | 1.17 | GUI + build-time `.slint` compilation | From M1 onward. Feature-gate: `std`, `compat-1-2`, `backend-winit`, `renderer-femtovg` |
| `sysinfo` | 0.38.x | Cross-platform CPU/RAM/process stats | Used for CPU utilization and RAM. Note: RAM telemetry was completely absent in legacy Python OpenOMEN; ZenOmen implements it from scratch using `MemAvailable` math |
| `serde` + `toml` | serde 1, toml 0.9 | Configuration (`config.toml`) | M1 for settings, M3+ for hardware profiles |
| `serde_json` | 1.x | RGB profile migration from legacy format | Used to parse existing `~/.config/openomen/rgb/*.json` safely without deleting source files |
| `zbus` | latest 5.x line | D-Bus system service and client | M3 onward — connects unprivileged GUI (`zenomen-gui`) to root service (`zenomen-hw`), and provides single-instance activation on session bus (`com.zenomen.App`) |
| `polkit` / `zbus::Proxy` to `org.freedesktop.PolicyKit1` | via zbus | Mandatory authorization for hardware writes | M3: mandatory check on every mutating operation (`SetFanMode`, `SetPowerProfile`, `ApplyRgbProfile`, `SetRgbEnabled`) |
| `tracing` + `tracing-subscriber` | 0.1 / 0.3 | Structured logs, `env-filter` | From M1 — lightweight, and diagnostics export depends on it |
| `thiserror` | 2.x | Typed `HardwareError` enum | From M1; maps 1:1 to D-Bus error contract |
| `nvml-wrapper` | latest | NVIDIA telemetry without spawning a process | Prefer over `nvidia-smi` subprocess once GPU polling is tuned (M2/M5). Keep `nvidia-smi` as the initial implementation for simplicity |
| `directories` | latest | XDG config/log paths | M1 |

### Development Tools

| Tool | Purpose | Notes |
|------|---------|-------|
| `cargo fmt` | Formatting gate | `cargo fmt --all -- --check` in CI |
| `cargo clippy` | Lint gate | `--all-targets -- -D warnings`; zero-warning tolerance |
| `cargo test` | Unit + integration tests | Pure calculation tests need no hardware; `MockHardware` implements `HardwareProvider` |
| `cargo audit` | Dependency vulnerability gate | **Mandatory** given `cargo-packager`'s advisory history. Fails CI on new advisories |
| `cargo packager` | DEB + AppImage production | Pin version; fallback to `cargo-deb` + `appimagetool` if blocked |

## Installation

```bash
# Core
cargo add slint --no-default-features --features std,compat-1-2,backend-winit,renderer-femtovg
cargo add sysinfo --no-default-features --features system
cargo add serde --features derive
cargo add toml thiserror anyhow
cargo add tracing
cargo add tracing-subscriber --features fmt,env-filter
cargo add directories

# Later phases
cargo add zbus          # M3: hardware service & single-instance
cargo add serde_json    # M4: RGB profile migration
cargo add nvml-wrapper  # M2/M5: NVIDIA telemetry

# Dev
cargo add --build slint-build
cargo install cargo-audit --locked
cargo install cargo-packager --locked
```

## Alternatives Considered

| Recommended | Alternative | When to Use Alternative |
|-------------|-------------|-------------------------|
| Slint | `egui` / `eframe` | Prototyping; immediate-mode redraws continuously, violating the ≤0.1% idle CPU budget |
| Slint | `iced` | Heavier baseline RSS than Slint's femtovg renderer |
| Slint | Tauri (webview) | Webview RSS alone typically exceeds the entire 40 MB budget |
| Slint | GTK4 (`gtk4-rs`) | Ties the app to GTK/libadwaita; contradicts cross-platform architecture readiness |
| `zbus` / D-Bus service | polkit-less setuid helper | Never. setuid binaries are a major security vulnerability |
| `cargo packager` | `cargo-deb` + `appimagetool` | Documented fallback if `cargo audit` reports unpatched advisories in packager |

## What NOT to Use

| Avoid | Why | Use Instead |
|-------|-----|-------------|
| `cargo-packager` pinned blindly to `latest` | Open maintenance issue (Jul 2026) regarding unaddressed RUSTSEC advisories | Pin exact version + `cargo audit`; fallback to `cargo-deb` + `appimagetool` |
| `tray-icon` crate | Redundant as of Slint 1.17 `SystemTrayIcon` | Slint native `SystemTrayIcon` |
| Any `tokio` async runtime in v1 | Unnecessary binary size and RSS overhead for 1–5 s polling | `std::thread` worker with deadline scheduling; zbus for D-Bus |
| Webview-based UI (Tauri, Wails) | Exceeds the 40 MB RSS budget | Slint native compiled GUI |
| Blocking operations on UI thread | Freezes Slint event loop (the exact flaw in Python's 500 ms sampling sleep) | Monitoring worker publishes snapshots; UI consumes them |
| Arbitrary sysfs path arguments via D-Bus | Root privilege escalation risk | Predefined whitelisted methods with compiled path constants |
| Kernel version as sole compatibility check | ACPI `platform_profile` works from 5.15+; fan control varies by EC firmware and model | Dynamic capability detection (`HardwareCapabilities`) |

## Memory Benchmark Definitions

The ≤40 MB target applies strictly to the unprivileged desktop UI process (`zenomen-gui`):
- **Cold startup RSS:** < 30 MB (immediately after initial window render)
- **Warm dashboard RSS:** < 38 MB (after 10 minutes of active 1 Hz telemetry sampling)
- **Minimized tray RSS:** < 25 MB (telemetry throttled while hidden)
- **Peak RSS:** ≤ 40 MB (under interaction and page switching over 2 hours)
- **Hardware service RSS (`zenomen-hw`):** ≤ 15 MB (measured separately, never summed into UI figure)

Tooling: `/proc/[pid]/smaps_rollup`, `ps -o rss,command`, and heap profiling via `heaptrack` or `valgrind --tool=massif`.

## Stack Patterns by Variant

**If the target is an OMEN/Victus whose `hp-wmi` exposes writable `pwm1_enable`:**
- Expose Automatic (`2`) and Maximum (`0`) modes with verified read-back.
- Do not expose manual mode (`1`) in v1 without validated hardware curve support.

**If the target lacks writable `pwm1_enable`:**
- Disable fan control in UI and display an honest plain-language explanation; read-only RPM telemetry remains active.

**If the desktop environment is stock GNOME:**
- Tray requires AppIndicator extension. ZenOmen provides single-instance activation via `com.zenomen.App` on the session bus, ensuring relaunching the app focuses the existing window even when no tray icon is visible.

## Version Compatibility

| Package A | Compatible With | Notes |
|-----------|-----------------|-------|
| `slint` 1.17 | `slint-build` 1.17 | Must match minor versions exactly |
| `slint` 1.17 | Rust stable ≥ 1.85 | Edition 2024 |
| `sysinfo` 0.38 | Rust stable | Pinned API |
| `zbus` 5.x | Rust stable | Standalone executor for D-Bus service and client |
| `nvml-wrapper` | NVIDIA driver ≥ 450 | Optional runtime path, never compile-time hard dependency |
| Linux kernel | ≥ 5.15 for ACPI `platform_profile` | Capability detection determines actual feature presence, not kernel version |

---
*Stack research for: ZenOmen*  
*Updated: 2026-10-30*
