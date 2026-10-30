# Stack Research

**Domain:** Native Linux desktop system utility (hardware control + telemetry) with a cross-platform GUI
**Researched:** 2026-10-30
**Confidence:** MEDIUM-HIGH (versions verified against official release notes; memory numbers are unverified claims until measured)

## Recommended Stack

### Core Technologies

| Technology | Version | Purpose | Why Recommended |
|------------|---------|---------|-----------------|
| Rust | edition 2024, stable toolchain | All application logic, hardware access, scheduling | Memory safety without a GC (essential for a ≤40 MB budget), single static binary, and the language the hardware-adjacent ecosystem (zbus, sysinfo, nvml) is written in |
| Slint | 1.17.x | Declarative GUI compiled to native code | Compiled `.slint` markup with no webview and no JS runtime — the only mainstream Rust GUI toolkit whose runtime claim is measured in hundreds of KiB, not tens of MB. 1.17 (2026-06-24) added the pieces this product needs: `SystemTrayIcon`, `Tooltip`, `RadioGroup` |
| Cargo | stable | Build, dependency, release profiles | `opt-level = "s"`, `lto = true`, `codegen-units = 1`, `strip = true`, `panic = "abort"` all directly serve the memory/size budget |

### Supporting Libraries

| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| `slint` + `slint-build` | 1.17 | GUI + build-time `.slint` compilation | From M1 onward. Feature-gate: `std`, `compat-1-2`, `backend-winit`, `renderer-femtovg` |
| `sysinfo` | 0.38.x | Cross-platform CPU/RAM/process stats | Use for CPU utilization and RAM only. Keep one long-lived `System` instance — utilization is computed from previous readings, so a fresh instance always reports 0 |
| `serde` + `toml` | serde 1, toml 0.9 | Configuration (`config.toml`) | M1 for settings, M3+ for hardware profiles |
| `serde_json` | 1.x | RGB profile migration from the legacy JSON format | Only when migrating `~/.config/openomen/rgb/*.json` |
| `zbus` | latest 5.x line | D-Bus system service and client | M3 onward — privileged hardware writes. Preferred over `dbus-rs` (blocking, C-linked) |
| `polkit` / `zbus::Proxy` to `org.freedesktop.PolicyKit1` | via zbus | Authorize callers of the hardware service | M3, when the service is introduced |
| `tracing` + `tracing-subscriber` | 0.1 / 0.3 | Structured logs, `env-filter` | From M1 — cheap, and the diagnostics export depends on it |
| `thiserror` | 2.x | Typed `HardwareError` enum | From M1; the error-code contract in the spec maps 1:1 to variants |
| `nvml-wrapper` | latest | NVIDIA telemetry without spawning a process | Prefer over `nvidia-smi` subprocess once GPU polling is tuned (M2/M5). Keep `nvidia-smi` as the first implementation for simplicity |
| `directories` | latest | XDG config/log paths | M1 |

### Development Tools

| Tool | Purpose | Notes |
|------|---------|-------|
| `cargo fmt` | Formatting gate | `cargo fmt --all -- --check` in CI |
| `cargo clippy` | Lint gate | `--all-targets -- -D warnings`; do not let warnings accumulate from M1 |
| `cargo test` | Unit + integration tests | Pure calculation tests need no hardware; `MockHardware` implements `HardwareProvider` |
| `cargo audit` | Dependency vulnerability gate | **Mandatory** given `cargo-packager`'s open RUSTSEC reports (see below). Fail CI on new advisories |
| `cargo packager` | DEB + AppImage production | Pin the version; see the maintenance caveat |

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
cargo add zbus          # M3: hardware service
cargo add serde_json    # M5: RGB profile migration
cargo add nvml-wrapper  # M2/M5: NVIDIA telemetry

# Dev
cargo add --build slint-build
cargo install cargo-audit --locked
cargo install cargo-packager --locked
```

## Alternatives Considered

| Recommended | Alternative | When to Use Alternative |
|-------------|-------------|-------------------------|
| Slint | `egui` / `eframe` | Prototyping or tool-style UIs where immediate-mode is fine. Immediate-mode redraws constantly, which fights the ≤0.1% idle CPU budget |
| Slint | `iced` | If you want a pure-Rust retained-mode MTV architecture with more mature text input. Heavier baseline RSS than Slint's femtovg renderer |
| Slint | Tauri (webview) | Only if the team is web-first and webview RSS/startup cost is acceptable — it is not, under a 40 MB budget |
| Slint | GTK4 (`gtk4-rs`) | Best native Linux integration and libadwaita look, but ties the app to GTK and its dependency chain; contradicts "cross-platform from the beginning" |
| `zbus` / D-Bus service | polkit-less setuid helper | Never. setuid binaries are a far larger attack surface than a reviewable D-Bus service |
| `zbus` / D-Bus service | systemd socket-activated root daemon | D-Bus already gives you activation, authorization and introspection |
| `cargo packager` | `cargo-deb` + `appimagetool` + `linuxdeploy` | Fall back here if `cargo audit` reports unpatched RUSTSEC advisories in packager's dependency tree at implementation time |
| `cargo packager` | `cargo-dist` | Better for CLI binaries; it does not produce DEB/AppImage desktop app bundles |
| `nvml-wrapper` | `nvidia-smi` subprocess | Keep the subprocess for M2 (simplest correct thing); move to NVML when optimizing |
| `sysinfo` | Hand-rolled `/proc` parsing | Use hand-rolled parsing only where the spec requires Linux-specific semantics (`MemAvailable`, per-core `/proc/stat` deltas with iowait). `sysinfo` must not silently change the definition of "used" |

## What NOT to Use

| Avoid | Why | Use Instead |
|-------|-----|-------------|
| `cargo-packager` pinned blindly to `latest` | An open maintenance issue (Jul 2026) states it is "not really maintained anymore and has a lot of RUSTSECs not addressed". It did receive a post-handoff release (0.12.0, Feb 2026) and is still used in the wild, so this is a **risk to manage, not a blocker** — pin an exact version and gate CI on `cargo audit` | Pin exact version + `cargo audit`; switch to `cargo-deb` + `appimagetool` if advisories are unresolvable |
| `tray-icon` crate for the tray | Redundant as of Slint 1.17, which ships `SystemTrayIcon`. Two tray implementations will fight over the same StatusNotifierItem name | Slint `SystemTrayIcon` |
| Any `tokio` async runtime in v1 | A single monitoring thread + zbus's own executor is enough; async adds binary size, RSS and reasoning cost for zero benefit at 1–5 s polling | A `std::thread` worker with deadline scheduling; zbus only where the service needs it |
| Webview-based UI (Tauri, Wails) | Webview RSS alone typically exceeds the entire 40 MB budget | Slint native rendering |
| Blocking operations on the UI thread | Slint runs the UI loop on the main thread; any `nvidia-smi` / `sleep` / D-Bus wait there freezes the window (the exact defect class that made OpenOMEN's Python dashboard sleep 500 ms per render) | Monitoring worker thread publishes snapshots; UI only consumes them |
| `unsafe` sysfs writes with `std::fs` on arbitrary user strings | Path traversal / privilege escalation into a root service | Validate every value against a whitelist, never interpolate user input into paths |
| Unpinned `slint` (e.g. `"1"`) | Slint's minor releases have added elements this app depends on (tray in 1.17); floating versions make builds non-reproducible | Pin `1.17` and bump deliberately |

## Stack Patterns by Variant

**If the target is an OMEN/Victus whose `hp-wmi` exposes writable `pwm1_enable`:**
- Use the sysfs path directly (0 = maximum/boost, 2 = automatic/BIOS per validated practice)
- Because this is the only officially exposed interface, and the mapping is hardware-specific

**If the target is a Victus S-Series on kernel ≥ ~6.20/7.0:**
- Manual fan control and PWM reporting arrive via new upstream `hp-wmi` support (single PWM value driving both fans). Treat "manual" as a *new capability* discovered at runtime, not an assumption
- Because the driver support only recently landed and is model-gated

**If the target lacks writable `pwm1_enable` entirely (known on several OMEN 16 models):**
- Disable the Cooling page's write controls and state the reason; read-only RPM may still work
- Because users already run patched `hp-wmi` kernels to obtain these interfaces; the app must never pretend a control exists

**If `renderer-femtovg` memory is still too high:**
- Benchmark `renderer-software` / Skia variants against the same page; measure, don't assume
- Because Slint's "<300 KiB runtime" claim describes the runtime, not the complete process

**If the desktop is GNOME:**
- Tray requires the AppIndicator/KStatusNotifierItem extension. Do not make the tray the only way to reach the app — closing to tray must leave an alternative path (relaunch focuses the existing window)
- Because Slint's Linux tray is D-Bus StatusNotifierItem, not XEmbed, and stock GNOME does not display those by default

## Version Compatibility

| Package A | Compatible With | Notes |
|-----------|-----------------|-------|
| `slint` 1.17 | `slint-build` 1.17 (same minor) | `slint-build` must match the runtime crate exactly |
| `slint` 1.17 | Rust stable ≥ 1.80 | Edition 2024 needs Rust ≥ 1.85 |
| `sysinfo` 0.38 | Rust stable | API changed across 0.3x minors — pin and consult docs for the exact line |
| `zbus` 5.x | Rust stable, `async` via its own executor | Do not mix the `async-io` and `tokio` features gratuitously |
| `nvml-wrapper` | NVIDIA driver ≥ 450 | Absent on AMD/Intel-only systems — must be an optional runtime path, not a compile-time assumption |
| Linux kernel ≥ 6.18 | `platform_profile` (ACPI) present; `hp-wmi` behavior varies | The ACPI `platform_profile` interface needs 5.15+; the project README states 6.18+ as the validated baseline |

## Sources

- https://slint.dev/blog/slint-1.17-released — verified 1.17 (2026-06-24) adds `SystemTrayIcon` (Linux/Windows/macOS), `Tooltip`, `RadioGroup`; HIGH confidence
- https://slint.dev/blog/making-slint-desktop-ready — tray on Linux via D-Bus, not XEmbed; roadmap toward desktop readiness; HIGH
- https://slint.dev — "<300KiB RAM" runtime claim, GPL/commercial/royalty-free licensing; HIGH for licensing, LOW for whole-app RSS
- https://github.com/uAtomicBoolean/slint-template/issues/6 (2026-07-28) — "cargo packager ... not really maintained anymore and a lot of RUSTSECs not addressed"; MEDIUM
- Search results showing cargo-packager 0.12.0 (2026-02-02, first post-handoff release) and active use in late-2026 projects — MEDIUM
- https://docs.kernel.org/hwmon/hp-wmi-sensors.html — HP-WMI sensor sysfs attributes; HIGH
- https://wiki.archlinux.org/title/HP_Omen_16-c0140AX — `pwm1_enable = 0` enables fan boost; HIGH for this model
- LKML / Phoronix (Jan–May 2026) — `hp-wmi` manual fan control for Victus S-Series, dual-channel PWM; MEDIUM (recent, still landing)
- https://docs.crabnebula.dev/packager/ — DEB/AppImage/NSIS target support; MEDIUM

---
*Stack research for: ZenOmen — Rust+Slint Linux-first hardware control center*
*Researched: 2026-10-30*
