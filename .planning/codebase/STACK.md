---
last_mapped_commit: e50af8f9ea61300a1c10d36dc6954e319da7b6e2
last_mapped_at: 2026-11-02
---
# Technology Stack

**Analysis Date:** 2026-11-02

## Scope

- ZenOmen today is a **Rust (edition 2024) + Slint** application, not the legacy Python tool. The Python `openomen` CLI, `openomen.spec`, and `how-to-release.txt` no longer exist on disk and must not be reintroduced.
- Two shipped binaries, both built from this one crate:
  - `zenomen` — unprivileged Slint desktop GUI, entry point `src/main.rs`
  - `zenomen-hw` — privileged system D-Bus daemon, entry point `src/bin/zenomen-hw.rs`
- Shared library crate `zenomen` (`src/lib.rs`) holds all hardware, service, config, RGB, and desktop logic. Binaries are thin wrappers.
- No network stack, no database, no cloud SDK. The dependency graph is deliberately small.

## Languages

**Primary:**
- Rust, edition `2024` — `Cargo.toml:4`. Edition 2024 requires rustc 1.85 or newer; CI and the release workflow install `stable` (`.github/workflows/ci.yml:36`–`40`, `.github/workflows/release.yml:38`–`42`). There is no `rust-toolchain.toml` and no declared MSRV.
- Slint DSL (`.slint`) — declarative UI under `ui/`, compiled to Rust at build time. Root component `AppWindow` in `ui/app.slint`; components in `ui/components/`, screens in `ui/screens/`, theme tokens in `ui/theme/`.

**Secondary:**
- Bash — `scripts/measure-budget.sh`, a resource-budget measurement harness only. Not part of the shipped product.
- TOML — configuration file format, `~/.config/zenomen/config.toml` (schema in `src/config/schema.rs`).
- JSON — RGB profiles (`~/.config/zenomen/rgb/*.json`, schema `src/rgb/schema.rs`), D-Bus payloads (`src/service/contract.rs`), diagnostics export (`src/core/diagnostics.rs`), and legacy OpenOMEN profile import (`src/rgb/migration.rs`).

## Runtime

**Environment:**
- Linux desktop session with system D-Bus, a D-Bus session bus, systemd, polkit, and an OpenGL 2.1+ driver. `README.md:65`–`71`.
- GUI process: single-threaded Slint/winit event loop plus dedicated named background threads (`zenomen-monitor-worker`, `zenomen-event-pump`, `zenomen-desktop-actions`, on-demand `zenomen-ctrl-*` / `zenomen-rpc-*`). See `src/core/worker.rs:33`–`380` and `src/main.rs:124`–`203`.
- Service process: `zbus::block_on` async executor hosting one D-Bus interface, blocking on `std::future::pending` (`src/bin/zenomen-hw.rs:23`–`45`).
- `zbus` is used in **blocking** mode in the GUI (`src/service/client.rs:5`, `src/desktop/single_instance.rs:8`, `src/desktop/tray.rs:9`) and in async mode inside the daemon.
- No Tokio runtime is started by application code. `tokio` appears in `Cargo.lock` only as a transitive dependency of `zbus`.

**Package Manager:**
- Cargo. Lockfile `Cargo.lock` is committed deliberately for reproducibility and `cargo audit` (`.gitignore` comment, lines 5–7).
- No `rust-toolchain.toml`, no `rustfmt.toml`, no `clippy.toml`, no `.cargo/config.toml`. Formatting uses rustfmt defaults; linting is default Clippy with `-D warnings` in CI.

## Frameworks

**Core:**
- `slint` 1.17 declared (`Cargo.toml:13`), **resolved to 1.18.1 in `Cargo.lock`** via caret semantics — with `default-features = false` and only `std`, `compat-1-2`, `backend-winit`, `renderer-femtovg`. The comment at `Cargo.toml:11`–`12` states Qt and Skia are excluded to protect the ≤40 MB RSS budget. Do not add `backend-qt`, `renderer-skia`, or accessibility features without re-measuring `docs/RESOURCE-BUDGET.md`.
- `slint-build` 1.17 declared / 1.18.1 locked — build-time compiler, invoked from `build.rs` with the `fluent` widget style pinned so developer machines and CI render identically (`build.rs:8`–`10`).
- `zbus` 5 declared / 5.19.0 locked — D-Bus system service, D-Bus client, session-bus single-instance service, and StatusNotifierItem/DBusMenu tray.
- `serde` 1.0 + `serde_json` 1.0 + `toml` 0.8 (locked 0.8.23) — config, RGB schema, D-Bus JSON payloads, diagnostics.
- `tracing` 0.1 + `tracing-subscriber` 0.3 with `env-filter` — structured logs in both binaries.

**Testing:**
- Built-in `cargo test`. Unit tests live inside modules (`#[cfg(test)]`, e.g. `src/hardware/linux/hwmon.rs:88`, `src/hardware/linux/cpu.rs:188`).
- Integration tests in `tests/` (17 files, e.g. `tests/hardware_service_tests.rs`, `tests/packaging_tests.rs`, `tests/config_tests.rs`).
- GUI tests require a display; CI runs `xvfb-run --auto-servernum cargo test --all` (`.github/workflows/ci.yml:64`–`65`).
- No external test framework, no snapshot/mock crate. Test doubles are hand-written (`src/hardware/mock.rs`) and temp directories are created manually.

**Build/Dev:**
- `cargo` + `build.rs`.
- `cargo-packager` — DEB and AppImage packaging, configured in `[package.metadata.packager]` (`Cargo.toml:46`–`75`).
- `cargo-audit` — dependency advisory gate in CI and release.
- `cargo fmt`, `cargo clippy` — quality gates.
- `scripts/measure-budget.sh` — RSS/CPU budget harness reading `/proc/<pid>/status` and `/proc/<pid>/stat`; supports `--soak`.
- `xvfb` — headless rendering for tests.
- `winres` 0.1 — Windows-only build dependency embedding `data/icons/zenomen.ico` (`build.rs:12`–`19`); Windows has no hardware provider yet and is unadvertised.

## Key Dependencies

Declared in `Cargo.toml`, with locked versions from `Cargo.lock`:

**Critical:**
- `slint` 1.18.1 (declared `1.17`) — entire GUI, including the `include_modules!` expansion in `src/lib.rs:14`. Renderer path is winit + FemtoVG, so native deps are xkbcommon, fontconfig, and OpenGL.
- `slint-build` 1.18.1 (declared `1.17`) — compiles `ui/app.slint`; every `.slint` change requires a rebuild.
- `zbus` 5.19.0 — all inter-process communication. Replacing it would touch `src/service/`, `src/desktop/single_instance.rs`, and `src/desktop/tray.rs`.
- `serde` 1.0.229 + `serde_json` 1.0.154 — derives on `AppConfig`, `RgbProfile`, `HardwareCapabilities`, `TelemetrySnapshot`, `PresetResult`, `ServiceConnectionStatus`, `RgbInterfaceVariant`.
- `toml` 0.8.23 — config parse/serialize in `src/config/mod.rs:47` and `:92`.

**Infrastructure:**
- `thiserror` 2.0.21 — error enums in `src/service/types.rs`, `src/rgb/schema.rs`, `src/desktop/releases.rs`, `src/desktop/system_integration.rs`.
- `anyhow` 1.0.104 — declared in `Cargo.toml:22` but **not referenced anywhere in `src/`**. Keep it out of new code or remove it; do not build an error-handling style around an unused crate.
- `tracing` 0.1.44 + `tracing-subscriber` 0.3.23 (`env-filter`) — `tracing_subscriber::fmt::init()` in the GUI (`src/main.rs:75`) and an explicit `EnvFilter` defaulting to `info` in `zenomen-hw` (`src/bin/zenomen-hw.rs:12`–`16`).
- `directories` 6.0.0 — `ProjectDirs::from("com", "zenomen", "zenomen")` for config/diagnostics (`src/config/mod.rs:20`–`22`) and `BaseDirs` for the XDG autostart file and legacy `~/.config/openomen` lookup (`src/desktop/autostart.rs:12`, `src/rgb/migration.rs:43`).

**Build-only:**
- `winres` 0.1.12 under `[target.'cfg(windows)'.build-dependencies]` (`Cargo.toml:34`–`35`).

**Not present (by design):**
- No HTTP client (`reqwest`, `ureq`, `hyper`, `curl`, `isahc`), no TLS stack (`rustls`, `native-tls`, `openssl`), no database driver, no async runtime owned by the app. Verified against the 557-package `Cargo.lock`: none of those names appear.

## Configuration

**Environment variables (all optional, none secret):**
- `RUST_LOG` — tracing filter for both binaries.
- `ZENOMEN_MOCK_SCENARIO` — switches the GUI to `MockHardware` with `nominal`, `zero`, or `gap` (`src/main.rs:158`–`166`, `src/hardware/mock.rs:9`–`16`).
- `ZENOMEN_BENCHMARK=1` — enables per-stage RSS lines on stdout (`src/core/benchmark.rs:245`–`250`).
- `ZENOMEN_HW_SESSION` — makes `zenomen-hw` bind the **session** bus instead of the system bus, for local development (`src/bin/zenomen-hw.rs:26`–`30`).
- No `.env` file, no secrets file, no credentials anywhere in the repository.

**User configuration:**
- `~/.config/zenomen/config.toml` — resolved from `ProjectDirs` (`src/config/mod.rs:25`–`27`). Sections `[general]` (`theme`, `start_minimized`, `close_to_tray`, `autostart`) and `[telemetry]` (`history_window`, `polling_interval_ms`); defaults and clamping in `src/config/schema.rs`.
- Polling interval is clamped to 500–5000 ms; `history_window` is restricted to 30/60/120/300 samples; malformed TOML is copied to `config.toml.bak.<unix-seconds>` and replaced by defaults (`src/config/mod.rs:52`–`77`).
- Writes are atomic: `.tmp` write → `sync_all` → `rename` (`src/config/mod.rs:84`–`105`).

**RGB state:**
- Profiles: `~/.config/zenomen/rgb/*.json` (`src/rgb/migration.rs:47`–`49`).
- Legacy source, read-only: `~/.config/openomen/rgb/*.json`, imported non-destructively at GUI startup (`src/main.rs:80`–`95`, `src/rgb/migration.rs:185`–`270`). Existing ZenOmen profiles are preserved, never overwritten; animation/timing keys are recorded in `unsupported_fields` instead of being applied.
- Built-in presets are in code, not on disk: `white`, `gaming`, `rainbow`, `off` (`src/rgb/migration.rs:52`–`99`).

**Other user state:**
- `~/.config/autostart/zenomen.desktop` — written/removed by `src/desktop/autostart.rs:67`–`92`.
- `~/.config/zenomen/diagnostics/zenomen-diagnostics-<timestamp>.json` — diagnostics export (`src/app.rs:233`–`241`).

**Build config files:**
- `Cargo.toml` — package, deps, release profile, cargo-packager metadata.
- `build.rs` — Slint compilation and Windows icon resource.
- `Cargo.lock` — committed.
- `.github/workflows/ci.yml`, `.github/workflows/release.yml`.
- `data/` — packaging inputs, embedded into the binary via `include_str!` in `src/desktop/system_integration.rs:20`–`26`.

## Build & Release Configuration

**Release profile (`Cargo.toml:37`–`44`):** `opt-level = "s"`, `lto = true`, `codegen-units = 1`, `panic = "abort"`, `strip = true`. These settings exist to protect the memory/size budget, not throughput. Do not relax them without measuring.

**Cargo Packager (`Cargo.toml:46`–`75`):**
- Product `ZenOmen`, identifier `com.zenomen.ZenOmen`, category `Utility`, GPL-3.0 license file, icons from `data/icons/hicolor/*/apps/zenomen.png` and the scalable SVG.
- Two binaries: `zenomen` (`main = true`) and `zenomen-hw` (`main = false`), out-dir `dist`, binaries-dir `target/release`, `before-packaging-command = "cargo build --release --bins --locked"`.
- DEB runtime dependencies: `libxkbcommon0`, `libfontconfig1`, `libgl1`, `libdbus-1-3`.
- DEB file mappings install: desktop entry → `/usr/share/applications/`, systemd unit → `/usr/lib/systemd/system/`, D-Bus activation → `/usr/share/dbus-1/system-services/`, D-Bus policy → `/usr/share/dbus-1/system.d/`, polkit policy → `/usr/share/polkit-1/actions/`, plus `README.md`, `LICENSE`, `NOTICE` under `/usr/share/doc/zenomen/`.
- Deliberately **no** `[package.metadata.packager.appimage]` section; `tests/packaging_tests.rs:154`–`171` asserts the critical mappings and the absence of invalid AppImage bin config.

**CI (`ci.yml`):** stable toolchain, native deps (`libxkbcommon-dev`, `libfontconfig1-dev`, `libgl1-mesa-dev`, `xvfb`), `cargo fmt --check`, Clippy with `-D warnings`, headless tests, `cargo audit`. Permissions are `contents: read`.

**Release (`release.yml`):** tag `v*` must match `Cargo.toml` version; runs the same gates plus `cargo packager --release --formats deb,appimage`, verifies artifact filenames contain the version, generates `SHA256SUMS`, and publishes via `softprops/action-gh-release`. Pinned action SHAs are used. `jq`, `dpkg-dev`, `file`, and `libfuse2` are used in this job.

## Platform Requirements

**Development:**
- Rust stable (edition 2024 ⇒ 1.85+), Cargo.
- Linux dev packages: `libxkbcommon-dev`, `libfontconfig1-dev`, `libgl1-mesa-dev`, `xvfb`.
- `cargo-packager` for packaging; `cargo-audit` for the security gate.
- A display or `xvfb-run` for tests that instantiate `AppWindow`.

**Production:**
- Linux only, validated on Ubuntu-based and Arch-family distributions. Kernel 6.1+ recommended (`README.md:67`).
- `hp-wmi` kernel module and standard `hwmon` sysfs; ACPI `platform_profile` for power modes.
- systemd + system D-Bus for `zenomen-hw`; polkit for authorization.
- OpenGL 2.1+ (Mesa llvmpipe acceptable as software fallback).
- Delivery: `.deb` and `.AppImage` attached to GitHub Releases; AppImage needs a one-time `sudo ./ZenOmen_*.AppImage --install-system-integration` to install the daemon and policy files (`docs/PACKAGING.md:85`–`97`).
- Windows: code is structured for it (`winres`, provider trait, mock provider) but no Windows hardware provider exists and it is not advertised.

---

*Stack analysis: 2026-11-02*
