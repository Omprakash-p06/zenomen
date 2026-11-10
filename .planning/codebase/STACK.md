<!-- refreshed: 2026-11-11 -->
# Technology Stack

**Analysis Date:** 2026-11-11
**Scope:** Full repository (Rust workspace root, single package `zenomen`)

## Languages

**Primary:**
- Rust, edition 2024 (`Cargo.toml`). Toolchain requirement is Rust 1.85+ (edition 2024), stated in `README.md`.
- Rust is used for everything that executes: GUI binary, privileged service binary, hardware drivers, packaging tests, and the build script.

**Secondary (declarative UI):**
- Slint (`.slint`) for the live GUI tree: `ui/app.slint` plus `ui/screens/*.slint`, `ui/components/*.slint`, `ui/theme/*.slint`.

**Supporting (non-application, repo tooling):**
- Bash: `scripts/measure-budget.sh`, `scripts/generate-icons.sh`, `packaging/linux/build-appimage.sh`.
- Python: `scripts/validate-icons.py`, `scripts/branding/generate-branding.py` (asset validation/generation only, not shipped).
- PowerShell: `packaging/windows/build-installer.ps1` (Inno Setup driver).
- WiX XML / Inno Setup script: `packaging/windows/ZenOmen.wxs`, `packaging/windows/wix/ZenOmen.wxs`, `packaging/windows/inno/ZenOmen.iss`.

## Runtime

**Environment:**
- Two native binaries, no interpreter, no managed runtime:
  - `zenomen` — unprivileged desktop GUI (`src/main.rs`), `required-features = ["ui"]`.
  - `zenomen-hw` — privileged hardware control service (`src/bin/zenomen-hw.rs`).
- No async runtime is added by this project. `zbus` internally brings an async executor stack, but application code calls it through `zbus::block_on` in `src/bin/zenomen-hw.rs` and the blocking API in `src/service/client.rs`.

**Package Manager:**
- Cargo. `Cargo.lock` is committed and pins 564 packages (transitive included).
- Cargo Packager produces DEB/AppImage metadata via `[package.metadata.packager]` in `Cargo.toml`.

**Entry Points:**
- `src/main.rs` — GUI: logging, CLI flags (`--minimized`, `--install-system-integration`, `--uninstall-system-integration`, `--export-diagnostics` handled in `src/core/diagnostics.rs`/`src/app.rs`), config load, single-instance, tray, provider selection, worker spawn, Slint event loop.
- `src/bin/zenomen-hw.rs` — service: Linux D-Bus owner of `com.zenomen.Hardware1`; on Windows SCM dispatcher with console fallback plus `--install-service`, `--uninstall-service`, `--console`, `--wmi-diag`, `--validate-8a3d-fan`, `--validate-8a3d-power`.
- `build.rs` — compiles `ui/app.slint` only when the `ui` feature is enabled, and emits Windows resources.

## Frameworks

**UI:**
- Slint `1.17` with `default-features = false` and exactly `std`, `compat-1-2`, `backend-winit`, `renderer-femtovg`.
- Qt and Skia backends are deliberately excluded to protect the ≤40 MB RSS budget (comment in `Cargo.toml`).
- Style is pinned to `fluent` in `build.rs` so local and CI rendering match.
- `src/ui/` is a separate, non-rendering "GPUI-CE" view-model scaffold (`src/ui/app.rs`, `src/ui/screens/*.rs`); it is compiled but not wired into either binary.

**Privileged IPC:**
- Linux: `zbus 5` (features `["blocking"]`) over the system bus; interface defined by `#[interface(name = "com.zenomen.Hardware1")]` in `src/service/contract.rs`.
- Windows: hand-written Win32 named-pipe transport (`windows-sys 0.59`) in `src/service/windows/pipe.rs`, `server.rs`, `client.rs`.

**Hardware access:**
- Linux: direct sysfs/procfs reads (`src/hardware/linux/*`), `nvidia-smi` watcher for NVIDIA GPUs (`src/hardware/linux/gpu.rs`).
- Windows: `windows-sys` for Win32 APIs (CPU times, PDH thermal zones, DXGI GPU enumeration, GDI display modes, process memory), plus a hand-rolled COM/WMI FFI layer for HP BIOS calls in `src/service/windows/wmi_ffi.rs` and `src/service/windows/hp_wmi.rs`.

## Key Dependencies

**Runtime (all targets):**
- `slint 1.17` (optional; enables the `ui` feature).
- `thiserror 2.0` — typed error enums (`HardwareControlError`, `DisplayError`, `SystemIntegrationError`, `DesktopError`).
- `serde 1.0`/`serde_json 1.0` — IPC payloads, diagnostics JSON, RGB profiles, config.
- `toml 0.8` — `config.toml` parsing in `src/config/mod.rs`.
- `tracing 0.1`, `tracing-subscriber 0.3` (`env-filter`), `tracing-appender 0.2` — stderr + daily rotating file logging in `src/logging.rs`.
- `directories 6.0` — XDG/known-folder resolution via `directories::ProjectDirs::from("com", "zenomen", "zenomen")`.

**Linux-only:**
- `libc 0.2` — `sysconf(_SC_CLK_TCK)` in `src/core/benchmark.rs`, `geteuid()` in `src/desktop/system_integration.rs`.
- `zbus 5` — system D-Bus service and client, Polkit calls.

**Windows-only:**
- `windows-sys 0.59` with feature list in `Cargo.toml`: Foundation, SystemInformation, Power, Performance, Threading, Memory, Registry, LibraryLoader, Pipes, Services, RemoteDesktop, IO, WMI, Storage_FileSystem, Security, Security_Authorization, UI_WindowsAndMessaging, UI_Shell, System_Com, Graphics_Gdi, System_ProcessStatus.

**Build-only:**
- `slint-build 1.17` (all targets), `winres 0.1` (Windows resource/icon/manifest).

## Configuration

**Application config:**
- TOML at `ProjectDirs::config_dir()/config.toml` (`~/.config/zenomen/config.toml` on Linux; `%APPDATA%\zenomen\zenomen\config\config.toml`-style path on Windows via `directories`).
- Schema in `src/config/schema.rs`:
  - `[general]` — `theme`, `start_minimized`, `close_to_tray` (default true), `autostart`.
  - `[telemetry]` — `history_window` (seconds; validated to 30/60/120/300, default 120), `polling_interval_ms` (clamped 500–5000, default 1000).
  - `[display]` — `preferred_quiet_refresh_rate` (60), `preferred_active_refresh_rate` (144), `restore_refresh_rate_on_exit`, `apply_profile_on_startup`.
- Corrupt TOML is backed up to `config.toml.bak.<unix-ts>` and defaults are loaded (`load_or_default`, `src/config/mod.rs`).
- Writes are atomic (`save_atomic`).

**Environment variables:**
- `ZENOMEN_MOCK_SCENARIO` — `nominal` / `zero` / `gap` selects `MockHardware` instead of the real provider (`src/main.rs`, `src/hardware/mock.rs`).
- `ZENOMEN_HW_SESSION` — bind `zenomen-hw` to the session bus instead of the system bus (test/dev only, `src/bin/zenomen-hw.rs`).
- `ZENOMEN_BENCHMARK=1` — `log_benchmark` prints RSS lines (`src/core/benchmark.rs`).
- `RUST_LOG` — `tracing` `EnvFilter` (`src/logging.rs`).

**Build configuration:**
- `profile.dev` and `profile.test`: `debug = 0`, `incremental = false` (comment: prevent 20+ GB `target/`).
- `profile.release`: `opt-level = "s"`, `lto = true`, `codegen-units = 1`, `panic = "abort"`, `strip = true` — size/RSS-oriented, not throughput-oriented.

**Packaging metadata (`Cargo.toml`):**
- `product-name = "ZenOmen"`, `identifier = "com.zenomen.ZenOmen"`, category `Utility`, `license = GPL-3.0-only`, icons from `data/icons/hicolor/*`.
- `out-dir = "dist"`, binaries `zenomen` (main) and `zenomen-hw`.
- DEB dependencies: `libxkbcommon0`, `libfontconfig1`, `libgl1`, `libdbus-1-3`; maintainer scripts in `packaging/linux/debian/{postinst,prerm,postrm}`; explicit file mapping for desktop entry, systemd unit, D-Bus service/conf, Polkit policy, docs.

## Platform Requirements

**Development:**
- Linux dev boxes need `libxkbcommon-dev`, `libfontconfig1-dev`, `libx11-dev`, `libdbus-1-dev`, `pkg-config`, and `xvfb` to run GUI-rendering tests (`README.md`).
- Windows dev boxes need MSVC build tools; the WiX/Inno packaging steps are CI-only prerequisites.

**Production — Linux:**
- Kernel 5.15+ with `hp-wmi` loaded for fan control / RGB; `platform_profile` for ACPI power profiles.
- Wayland or X11 session; a StatusNotifierItem host for the tray.
- `zenomen-hw` installed as a systemd service with D-Bus policy and Polkit action.

**Production — Windows:**
- Windows 10/11 x86_64.
- Optional `ZenOmenHardwareService` (SCM, LocalSystem) for HP WMI fan/thermal control over the named pipe.

**CI (`.github/workflows/ci.yml`):**
- `linux-quality` (ubuntu-latest): `cargo fmt --all -- --check`, `cargo clippy --all-targets --all-features -- -D warnings`, `xvfb-run --auto-servernum cargo test --all`, `cargo audit`.
- `windows-quality` (windows-latest): fmt, clippy, `cargo test --all`.
- Release workflow (`.github/workflows/release.yml`) builds `.deb`, `.AppImage`, Inno EXE, WiX MSI and portable ZIP, verifies tag == `Cargo.toml` version, and publishes unified `SHA256SUMS`.

**Resource budgets (from `README.md`, `docs/RESOURCE-BUDGET.md`):**
- GUI `zenomen`: ≤40 MB peak RSS.
- Service `zenomen-hw`: ≤15 MB RSS.
- Measured with `scripts/measure-budget.sh` (`/proc/<pid>/status` `VmRSS` + `utime+stime`, `--soak` mode for growth).

---

*Stack analysis: 2026-11-11*
