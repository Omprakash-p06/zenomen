---
last_mapped_commit: 4baf4bdbfc56a9755e48c37893d7515f1ffa172d
last_mapped_at: 2026-11-07
---
# Technology Stack

**Analysis Date:** 2026-11-07

## Summary

ZenOmen is a native Linux hardware control center built as two Rust binaries in one
Cargo package:

- `zenomen` — unprivileged Slint desktop GUI (`src/main.rs`, `src/app.rs`, `ui/`),
  gated by `required-features = ["ui"]`.
- `zenomen-hw` — privileged, headless system D-Bus daemon (`src/bin/zenomen-hw.rs`,
  `src/service/`), compiled without linking Slint via `--no-default-features`.

The stack deliberately contains no webview, no JavaScript runtime, no async runtime
(Tokio is not a direct dependency), no database, and no HTTP client. Hardware state is
read from procfs/sysfs and mutated only by `zenomen-hw` through verified sysfs writes.
Logging uses `tracing-appender` 0.2 for non-blocking daily rotating files under the
standard state directory. Telemetry tracks 11 core sensors (excluding the UI-only
synthetic RGB profile), with NVIDIA sampling handled by a long-lived `NvidiaSampler`
thread. Windows target compilation is supported with `--target x86_64-pc-windows-msvc
--no-default-features`, backed by an architecture-ready `UnavailableHardwareProvider`
and `winres`.

## Languages

**Primary:**
- Rust, edition 2024 (`Cargo.toml:4`) — all application, IPC, hardware, and test code
  under `src/` and `tests/`.
- Slint DSL 1.x — 23 `.slint` files under `ui/` (`ui/app.slint`, `ui/screens/`,
  `ui/components/`, `ui/theme/`), compiled into Rust at build time by
  `slint-build` in `build.rs` when the `ui` feature is enabled.

**Secondary:**
- Bash — `scripts/generate-icons.sh` (icon generation) and
  `scripts/measure-budget.sh` (RSS/CPU measurement from `/proc`).
- Python 3, standard library only (`os`, `struct`, `sys`, `pathlib`) —
  `scripts/validate-icons.py` for icon dimension/ICO validation.
- TOML — user configuration (`~/.config/zenomen/config.toml`) and Cargo manifests.
- JSON — RGB profiles, diagnostics exports, and D-Bus payloads.
- XML — D-Bus bus policy and Polkit policy (`data/dbus/`, `data/polkit/`).

No C, C++, Go, Node.js, or TypeScript source is present.

## Runtime

**Environment:**
- Native Linux userspace processes; no VM, interpreter, or managed runtime at runtime.
- Windows target support is architecture-ready for headless daemon and library builds
  via `--target x86_64-pc-windows-msvc --no-default-features`, backed by
  `UnavailableHardwareProvider` (`src/hardware/unavailable.rs`).
- GUI process runs the Slint event loop on the main thread, with long-lived background
  threads:
  - Telemetry sampling worker (`src/core/worker.rs`).
  - Event pump and D-Bus action dispatch (`src/main.rs`).
  - `NvidiaSampler` watcher thread (`zenomen-nv-watch` in `src/hardware/linux/gpu.rs`).
  - Non-blocking log writer thread managed by `tracing-appender` (`src/logging.rs`).
  - One-shot diagnostics-export worker (`src/app.rs`).
- Daemon uses `zbus::block_on` with `std::future::pending::<()>()` to stay alive
  (`src/bin/zenomen-hw.rs`); zbus uses `async-io`/`async-executor`, not Tokio.
- `panic = "abort"` in release builds (`Cargo.toml:60`).

**Package Manager:**
- Cargo, with a committed `Cargo.lock` (lockfile format version 4).
- Resolved lockfile contains 564 package entries, including platform-specific and
  build-time dependencies.
- No `rust-toolchain.toml` or `.cargo/config.toml` exists. CI installs the latest
  stable Rust with `clippy` and `rustfmt` (`.github/workflows/ci.yml:36-40`).

## Frameworks

**Core:**
- Slint — manifest requirement `slint = "1.17"` (`Cargo.toml:17`), resolved to
  **1.18.1** in `Cargo.lock`. Optional dependency behind the default `ui` feature
  (`ui = ["dep:slint"]`). Features enabled: `std`, `compat-1-2`, `backend-winit`,
  `renderer-femtovg`. Default features are disabled, explicitly excluding the Qt and
  Skia backends to protect the memory budget.
- `slint-build` — manifest requirement `"1.17"` (`Cargo.toml:35`), resolved to
  **1.18.1**. Compiles `ui/app.slint` with the `fluent` style in `build.rs` only when
  the `ui` feature is enabled (`CARGO_FEATURE_UI`).
- zbus **5.19.0** — D-Bus client/server with the `blocking` feature
  (`Cargo.toml:31`). Used for system-bus hardware control, session-bus
  single-instance/tray integration, and Polkit checks.

**Testing:**
- Built-in Rust test harness (`#[test]`/`#[cfg(test)]`), no external test framework.
- 16 integration test files in `tests/`: `benchmark_tests.rs`, `config_tests.rs`,
  `degradation_tests.rs`, `desktop_integration_tests.rs`, `diagnostics_tests.rs`,
  `fan_control_tests.rs`, `hardware_service_tests.rs`, `linux_gpu_tests.rs`,
  `linux_procfs_tests.rs`, `logging_tests.rs`, `mock_hardware_tests.rs`,
  `packaging_tests.rs`, `power_preset_tests.rs`, `rgb_tests.rs`,
  `sensor_registry_tests.rs`, `telemetry_history_tests.rs`.
- Tests run headless under Xvfb because Slint test paths initialize a window:
  `xvfb-run --auto-servernum cargo test --all` (`.github/workflows/ci.yml:64-65`).
- Hardware tests use `MockHardware` (`src/hardware/mock.rs`) and temporary fixture
  directories; Linux parsers accept injectable roots.

**Build/Dev:**
- Cargo + `build.rs` (conditional Slint compilation; Windows icon embedding via `winres`).
- `cargo fmt` and `cargo clippy --all-targets --all-features -- -D warnings`.
- `cargo audit` against the committed `Cargo.lock` in CI.
- `cargo-packager` for `.deb` and `.AppImage` output; installed in CI, version not
  pinned in this repository.
- `Xvfb` for headless GUI tests.

## Key Dependencies

**Critical:**
| Crate | Manifest requirement | Resolved (`Cargo.lock`) | Why it matters |
|---|---|---|---|
| `slint` | `1.17` (caret) | `1.18.1` | All GUI rendering; optional behind `ui` feature so `zenomen-hw` does not link a toolkit. |
| `slint-build` | `1.17` (caret) | `1.18.1` | Build-time `.slint` compiler in `build.rs` (skipped when `ui` feature is disabled). |
| `zbus` | `5` | `5.19.0` | Every D-Bus integration: hardware service, client, Polkit, single-instance, tray. |
| `serde` | `1.0` + `derive` | `1.0.229` | Config, D-Bus payloads, diagnostics, RGB profile serde. |
| `serde_json` | `1.0` | `1.0.154` | Diagnostics JSON, RGB profile JSON, D-Bus JSON strings. |
| `toml` | `0.8` | `0.8.23` | `~/.config/zenomen/config.toml` parsing/serialization. |
| `thiserror` | `2.0` | `2.0.21` | Typed error enums in hardware/service/desktop layers. |
| `directories` | `6.0` | `6.0.0` | XDG config/state/autostart paths via `BaseDirs`/`ProjectDirs`. |
| `tracing` | `0.1` | `0.1.44` | Structured logging across both binaries. |
| `tracing-subscriber` | `0.3` + `env-filter` | `0.3.23` | `RUST_LOG` filtering and stderr/file layers. |
| `tracing-appender` | `0.2` | `0.2.5` | Non-blocking daily rolling log files under standard state directory. |

**Infrastructure / transitive (selected):**
| Crate | Resolved | Role |
|---|---|---|
| `winit` | `0.30.13` | Windowing backend used by Slint's `backend-winit`. |
| `femtovg` | `0.27.0` | GPU vector renderer used by `renderer-femtovg`. |
| `i-slint-backend-winit` | `1.18.1` | Slint winit backend implementation. |
| `i-slint-renderer-femtovg` | `1.18.1` | Slint femtovg renderer implementation. |
| `zvariant` | `5.15.1` | D-Bus value serialization used directly by Polkit/tray code. |
| `async-io` | `2.6.0` | zbus I/O driver; no Tokio runtime. |
| `async-executor` | `1.14.0` | Executor backing zbus async operations. |
| `libc` | `0.2.191` | Transitive platform glue. |

**Windows-only build dependency:**
- `winres` **0.1.12** (`Cargo.toml:44-45`) in `[target.'cfg(windows)'.build-dependencies]`
  embeds `data/icons/zenomen.ico` into Windows executables (`build.rs:18-25`).

## Explicitly Absent / Not Used

Verified against `Cargo.lock` and source imports:

- No `anyhow` in direct dependencies (removed in favor of `thiserror`).
- No HTTP/TLS stack: no `reqwest`, `hyper`, `ureq`, `curl`, `rustls`, `native-tls`, or
  `openssl`.
- No database/ORM: no `sqlx`, `rusqlite`, `diesel`.
- No `sysinfo` and no `nvml-wrapper`; telemetry is hand-parsed from procfs/sysfs and
  `nvidia-smi` via a dedicated `NvidiaSampler`.
- No Tokio direct dependency or runtime. `tokio 1.54.1` appears in the lockfile only as
  a `wasm-bindgen-futures` dependency for WebAssembly targets, not as an application
  runtime.
- No GTK/Qt bindings and no Slint Qt/Skia features.
- No `.env` file is present in the repository.

## Configuration

**Feature flags (`Cargo.toml:10-14`):**
- `default = ["ui"]` — builds the desktop GUI and links Slint by default.
- `ui = ["dep:slint"]` — gates `src/app.rs`, `slint::include_modules!()` in
  `src/lib.rs:8-19`, and the `zenomen` binary (`required-features = ["ui"]`).
- Building with `--no-default-features` builds the library and `zenomen-hw` daemon only,
  without compiling or linking Slint. This allows headless daemon compilation.

**Lean dev/test profiles (`Cargo.toml:47-55`):**
- `[profile.dev]`: `debug = 0`, `incremental = false` — prevents the `target/`
  directory from ballooning with multi-gigabyte DWARF debuginfo and incremental caches.
- `[profile.test]`: `debug = 0`, `incremental = false` — keeps test compile artifacts
  lean and bounded.
- `[profile.release]`: `opt-level = "s"`, `lto = true`, `codegen-units = 1`,
  `panic = "abort"`, `strip = true` (`Cargo.toml:57-64`).

**User/application configuration:**
| Path | Purpose | Source |
|---|---|---|
| `~/.config/zenomen/config.toml` | General and telemetry settings; atomic save with `.bak.<timestamp>` fallback on parse failure. | `src/config/mod.rs:20-116` |
| `~/.config/zenomen/diagnostics/zenomen-diagnostics-<timestamp>.json` | Exported diagnostics report. | `src/app.rs:233-245` |
| `~/.config/zenomen/rgb/` | ZenOmen RGB profile storage. | `src/rgb/migration.rs:46-49` |
| `~/.config/openomen/rgb/` | Legacy OpenOMEN profiles; read-only migration source. | `src/rgb/migration.rs:41-44` |
| `~/.config/autostart/zenomen.desktop` | XDG login autostart entry. | `src/desktop/autostart.rs:11-28` |
| `~/.local/state/zenomen/logs/` | Daily rolling logs (`zenomen` prefix, `.log` suffix, max 7 files) via `tracing-appender`. | `src/logging.rs:27-56` |

**Environment variables (all optional):**
| Variable | Effect | Source |
|---|---|---|
| `RUST_LOG` | `tracing` filter; defaults to `info`. | `src/logging.rs:77` |
| `ZENOMEN_MOCK_SCENARIO` | Selects mock telemetry scenario (`nominal`, `zero`, `gap`, etc.). | `src/main.rs:170-184`, `src/hardware/mock.rs:71-118` |
| `ZENOMEN_BENCHMARK=1` | Emits benchmark lines from `/proc/self` measurements. | `src/core/benchmark.rs:242-257` |
| `ZENOMEN_HW_SESSION` | Dev/test override: daemon connects to the session bus instead of the system bus. | `src/bin/zenomen-hw.rs:23-27` |
| `LANG`, `LC_CTYPE`, `LC_ALL` | Linux UTF-8 locale fallback before Slint/winit initialization. | `src/main.rs:27-38` |

There are no required secrets. `SLINT_STYLE` is not honored because `build.rs` pins
the `fluent` style explicitly (`build.rs:13`).

**Build configuration files:**
- `Cargo.toml` — package metadata, features, dependency pins, dev/test/release profiles,
  and `cargo-packager` metadata.
- `Cargo.lock` — committed and authoritative for shipped builds (`.gitignore` comment
  explicitly keeps it).
- `build.rs` — conditional Slint compilation (when `ui` is active) and Windows resources (`winres`).
- `.github/workflows/ci.yml` — format, clippy, headless tests, audit.
- `.github/workflows/release.yml` — tagged builds, packaging, checksums, GitHub Release.

**Common build commands:**

```bash
cargo build --release                                             # GUI + daemon
cargo build --release --no-default-features                       # daemon only; no Slint
cargo check --target x86_64-pc-windows-msvc --no-default-features # Windows compilation check
xvfb-run -a cargo test --all                                      # tests
cargo packager --release --formats deb,appimage
```

## Version Pinning Notes

- The `Cargo.toml` comment says Slint is "pinned to 1.17", but `version = "1.17"` is a
  caret requirement and `Cargo.lock` resolves **1.18.1**. Builds use the lockfile;
  `docs/decisions/ADR-001-ui-framework.md` explicitly flags this policy mismatch.
- `Cargo.toml` declares no `rust-version`. `README.md:148` states "Rust 2024 edition
  (1.85+)", while `docs/decisions/ADR-001-ui-framework.md:31` records Slint 1.18.1's
  MSRV as 1.88. CI does not pin a toolchain version beyond `stable`.
- All exact versions above come from `Cargo.lock`; no versions were inferred from
  crates.io.

## Platform Requirements

**Development:**
- Linux with a Rust stable toolchain supporting edition 2024.
- Build tools and native libraries used by CI: `libxkbcommon-dev`,
  `libfontconfig1-dev`, `libgl1-mesa-dev`, `xvfb`
  (`.github/workflows/ci.yml:44-51`); `build-essential`/`pkg-config` are documented in
  `README.md:152-162`.
- Runtime test/integration services: D-Bus daemon, systemd, Polkit.
- `cargo-audit` and `cargo-packager` for CI-equivalent checks and packaging.

**Production:**
- Linux with systemd and a system D-Bus, plus Polkit for privileged actions.
- HP OMEN/Victus laptop with the `hp-wmi` kernel module for fan/power/RGB control
  (`README.md:261-266`).
- OpenGL 2.1+ (or Mesa llvmpipe software fallback) for Slint's femtovg renderer.
- Debian package runtime dependencies: `libxkbcommon0`, `libfontconfig1`, `libgl1`,
  `libdbus-1-3` (`Cargo.toml:88-90`).
- Non-HP or unsupported hardware should run in read-only telemetry mode; missing
  interfaces degrade to `Unavailable` rather than fabricated readings.

**Windows / non-Linux:**
- Compilation supported with `--target x86_64-pc-windows-msvc --no-default-features`.
- The crate is architecture-ready for Windows: Slint `backend-winit` + `winres` icon
  resource embedding (`build.rs`).
- No Windows hardware backend is implemented yet: `UnavailableHardwareProvider`
  (`src/hardware/unavailable.rs`) implements `HardwareProvider`, reports zero
  capabilities (`reported_sensor_count() == 0`), and returns an explicit `Err` on
  `sample_telemetry()`, which the UI renders as `Unavailable` panels.
- `build.rs` gates Slint compilation behind `CARGO_FEATURE_UI`, preventing build
  failures on headless daemon builds.
- All real hardware integrations are under `src/hardware/linux/` and compiled only
  for Linux (`src/hardware/mod.rs:16-23`, `src/main.rs`).

**Resource targets (not enforced by code):**
- GUI peak private dirty RSS ≤ 40 MB; daemon idle RSS ≤ 15 MB
  (`docs/RESOURCE-BUDGET.md`). These targets drive the Slint feature selection and
  release profile.

---

*Stack analysis: 2026-11-07*
