# Technology Stack

**Analysis Date:** 2026-11-08

## Languages

**Primary:**
- Rust edition 2024 - Entire application: GUI, hardware providers, privileged services, config, logging, RGB, desktop integration. Declared in `Cargo.toml`; no `rust-version` field.
- Slint DSL - Declarative UI: `ui/app.slint`, `ui/screens/*.slint`, `ui/components/*.slint`, `ui/theme/*.slint` (20 `.slint` files compiled at build time).

**Secondary:**
- Bash - Resource measurement and icon generation: `scripts/measure-budget.sh`, `scripts/generate-icons.sh`.
- Python 3 - Icon validation and raster fallback: `scripts/validate-icons.py`, embedded Python in `scripts/generate-icons.sh`.
- PowerShell inline - Windows release steps in `.github/workflows/release.yml`.

## Runtime

**Environment:**
- Native desktop process, no managed runtime and no async runtime. Long-lived threads use `std::thread` plus `std::sync::mpsc` channels (`src/core/worker.rs`, `src/main.rs`).
- `zbus` uses its async-io/async-executor stack, not Tokio. `tokio 1.54.1` appears in `Cargo.lock` only as a transitive dependency of `wasm-bindgen-futures`, not as a native runtime.
- Two binaries: unprivileged GUI `zenomen` and privileged daemon `zenomen-hw`, both declared in `Cargo.toml`.

**Toolchain:**
- Rust 2024 edition. `README.md` states Rust 1.85+; Slint 1.18.1 has MSRV 1.88 per `docs/decisions/ADR-001-ui-framework.md`.
- CI uses the stable toolchain with `clippy` and `rustfmt` (`.github/workflows/ci.yml`, `.github/workflows/release.yml`).

**Package Manager:**
- Cargo, lockfile committed (`Cargo.lock`, deliberately not ignored in `.gitignore`) so the shipped dependency graph is reproducible and auditable.
- GitHub Actions installs `cargo-audit` and `cargo-packager`; releases are uploaded as `.deb`, `.AppImage`, `.msi`, and `.zip`.

## Frameworks

**Core:**
- Slint 1.18.1 (locked; declared as `slint = "1.17"` caret in `Cargo.toml`) - desktop UI.
  - `default-features = false`; enabled features: `std`, `compat-1-2`, `backend-winit`, `renderer-femtovg`.
  - Deliberately excludes Qt/Skia backends and accessibility defaults to protect the <=40 MB RSS budget.
  - Backend stack resolved in `Cargo.lock`: `i-slint-backend-winit 1.18.1`, `winit 0.30.13`, `i-slint-renderer-femtovg 1.18.1`, `femtovg 0.27.0`, `glutin 0.32.3`, `fontdb 0.24.0`.
  - The Slint bridge is `src/app.rs` (`ZenOmenApp`); the compiled module is included by `slint::include_modules!()` in `src/lib.rs`.
- Note on UI direction: `docs/decisions/ADR-002-gpui-ce-migration.md` records an accepted plan to migrate to GPUI-CE, but no GPUI dependency exists in `Cargo.toml` and no GPUI calls exist in `src/ui/`. `src/ui/` is a plain-Rust view-model scaffold (`src/ui/app.rs` `GpuiApp`), while the production UI remains Slint (`src/main.rs` uses `zenomen::AppWindow`).

**Testing:**
- Rust built-in test harness (`cargo test`) with unit tests in-module and 18 integration suites under `tests/`.
- GUI tests are run headless under Xvfb: `xvfb-run --auto-servernum cargo test --all` (`.github/workflows/ci.yml`, `README.md`).
- No external assertion, mocking, or snapshot framework; fixtures are temporary directories and mock providers (`src/hardware/mock.rs`).

**Build/Dev:**
- `slint-build 1.18.1` - compiles `ui/app.slint` in `build.rs` with the pinned `fluent` style.
- `winres 0.1.12` - Windows-only build dependency; embeds `data/icons/zenomen.ico`, product metadata, and a per-monitor-V2 DPI manifest (`build.rs`).
- `cargo-packager` - DEB/AppImage packaging configured in `[package.metadata.packager]` in `Cargo.toml`.
- WiX Toolset (`wix build`) - Windows MSI from `packaging/windows/ZenOmen.wxs` (duplicate copy at `packaging/windows/wix/ZenOmen.wxs`).
- GitHub Actions - `.github/workflows/ci.yml`, `.github/workflows/release.yml`.

## Key Dependencies

**Critical:**
- `slint` 1.18.1 (lock; optional, feature `ui`) - UI toolkit. Build-time failure to compile `ui/app.slint` aborts GUI builds.
- `zbus` 5.19.0 with `blocking` - system D-Bus hardware daemon, session-bus single instance, StatusNotifierItem tray, and Polkit calls. Used only through `cfg(not(target_os = "windows"))` code paths.
- `windows-sys` 0.59.0 - raw Win32 bindings for telemetry, named pipes, SCM services, registry, shell, COM, and security. Enabled features are listed in `Cargo.toml` (`Win32_System_Wmi` is enabled but no WMI API calls are implemented).
- `serde` 1.0.229 (`derive`), `serde_json` 1.0.154, `toml` 0.8.23 - config, diagnostics JSON, IPC payloads, RGB profiles.
- `thiserror` 2.0.21 - typed error enums (`HardwareControlError`, `ProtocolError`, `RgbError`, `DesktopError`).
- `tracing` 0.1.44, `tracing-subscriber` 0.3.23 (`env-filter`), `tracing-appender` 0.2.5 - stderr + rolling file logs.
- `directories` 6.0.0 - `ProjectDirs::from("com", "zenomen", "zenomen")` for config, RGB, state/log, and autostart paths (`src/config/mod.rs`, `src/logging.rs`, `src/rgb/migration.rs`, `src/desktop/autostart.rs`).

**Platform-specific:**
- `libc` 0.2.191 - Unix-only dependency (declared under `[target.'cfg(unix)'.dependencies]`; used for effective-UID checks in `src/desktop/system_integration.rs`).
- `windows-sys` 0.59.0 - Windows-only direct dependency. `Cargo.lock` also contains `windows-sys` 0.48.0 (winres), 0.52.0, and 0.61.2 (zbus) as transitive versions.
- No `wmi` crate, no `windows` (windows-rs high-level) crate, no `winapi`.

**Not present (deliberate):**
- No HTTP/network client (`reqwest`, `hyper`, `ureq`), no database driver, no async runtime, no GTK/Qt/Tao toolkit, no self-updater.
- Release checking opens the GitHub Releases page in the system browser rather than performing network I/O (`src/desktop/releases.rs`, `src/desktop/windows/releases.rs`).

## Configuration

**Compile-time features:**
- `default = ["ui"]`; `ui = ["dep:slint"]` (`Cargo.toml`).
- `[[bin]] zenomen` requires feature `ui`; `[[bin]] zenomen-hw` has no UI requirement and is built with `--no-default-features` for the daemon so Slint is neither compiled nor linked.
- `build.rs` checks `CARGO_FEATURE_UI` and skips Slint compilation for UI-less builds.

**Runtime configuration:**
- `config.toml` resolved through `directories::ProjectDirs::from("com", "zenomen", "zenomen")` in `src/config/mod.rs` (`config_dir()/config.toml`).
- Schema in `src/config/schema.rs`: `[general]` (`theme`, `start_minimized`, `close_to_tray`, `autostart`) and `[telemetry]` (`history_window`, `polling_interval_ms`).
- Validation clamps polling to 500-5000 ms (default 1000) and restricts history to 30/60/120/300 (default 120).
- Corrupt TOML is backed up as `config.toml.bak.<unix_timestamp>` and safe defaults are used; saves are atomic (temp file + fsync + rename + parent-directory fsync).
- RGB profiles: `config_dir()/rgb` (`src/rgb/migration.rs`); read-only migration from `~/.config/openomen/rgb`.
- Diagnostics export: Linux `config_dir()/diagnostics/zenomen-diagnostics-<timestamp>.json`; Windows `%LOCALAPPDATA%\ZenOmen\diagnostics\...` (`src/app.rs`).
- Logs: Linux `~/.local/state/zenomen/logs` (fallback data dir); Windows `%LOCALAPPDATA%\ZenOmen\logs` for GUI and `%PROGRAMDATA%\ZenOmen\logs` when the executable name contains `zenomen-hw` (`src/logging.rs`, `src/desktop/windows/paths.rs`).

**Environment variables:**
- `ZENOMEN_MOCK_SCENARIO` - selects `MockHardware`: `nominal` (default), `zero-capabilities`, `sensor-data-gap` (`src/hardware/mock.rs`, `src/main.rs`).
- `ZENOMEN_HW_SESSION` - makes the Linux daemon bind the session bus instead of the system bus (`src/bin/zenomen-hw.rs`).
- `RUST_LOG` - tracing `EnvFilter`; defaults to `info` (`src/logging.rs`).
- `LC_CTYPE`/`LC_ALL`/`LANG` - read at startup on Linux; `LC_CTYPE=C.UTF-8` is set when no UTF-8 locale is present (`src/main.rs`).
- No `.env` files are present or read.

**Build config files:**
- `Cargo.toml` - package, features, dependencies, binaries, release/dev/test profiles, cargo-packager metadata.
- `build.rs` - Slint compilation, pinned `fluent` style, Windows icon/metadata/manifest.
- `.github/workflows/ci.yml`, `.github/workflows/release.yml`.
- `packaging/windows/ZenOmen.wxs` - per-machine MSI with GUI file, service file, service install/control, shortcuts.
- `data/systemd/zenomen-hw.service`, `data/dbus/*`, `data/polkit/*`, `data/applications/zenomen.desktop`.
- `scripts/measure-budget.sh` (RSS/CPU harness), `scripts/generate-icons.sh`, `scripts/validate-icons.py`.

**Build profiles (`Cargo.toml`):**
- `dev`/`test`: `debug = 0`, `incremental = false` to limit `target/` growth.
- `release`: `opt-level = "s"`, `lto = true`, `codegen-units = 1`, `panic = "abort"`, `strip = true` to protect the memory/size budget.

## Platform Requirements

**Development:**
- Linux: `build-essential`, `pkg-config`, `libfontconfig1-dev`, `libx11-dev`, `libxkbcommon-dev`, `libdbus-1-dev`, `xvfb` (Ubuntu/Debian); equivalent packages on Fedora/Arch per `README.md`.
- Windows: MSVC Rust toolchain, Windows SDK/`rc.exe` for `winres`, WiX (`dotnet tool install --global wix`) for MSI packaging.
- Fonts are vendored and embedded (`ui/theme/fonts/Inter-Regular.ttf`, `Inter-SemiBold.ttf`, `JetBrainsMono-Regular.ttf`), so no system font install is required.

**Production:**
- Linux kernel 5.15+, `hp-wmi` module, Wayland or X11, systemd, D-Bus system bus, Polkit, Freedesktop StatusNotifierItem host for the tray.
- DEB runtime dependencies: `libxkbcommon0`, `libfontconfig1`, `libgl1`, `libdbus-1-3` (`[package.metadata.packager.deb]`).
- Windows 10/11 x64. GUI runs as a standard user; optional `ZenOmenHardwareService` runs as `NT AUTHORITY\LocalSystem` and is installed by MSI (`packaging/windows/ZenOmen.wxs`).

---

*Stack analysis: 2026-11-08*
