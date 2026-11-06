---
last_mapped_commit: 30d62de2cdf6f507822944e5602bdd907539ce38
last_mapped_at: 2026-11-06
---
# Technology Stack

**Analysis Date:** 2026-11-06

## Scope

ZenOmen is a **Rust (edition 2024) + Slint** application. The legacy Python `openomen` CLI has been retired (Phase 5) and no Python source, `openomen.spec`, or `requirements.txt` remains on disk. Do not reintroduce it.

One Cargo package (`zenomen` 0.1.0, `Cargo.toml:2`–`8`) produces two binaries plus a shared library:

| Artifact | Entry point | Role |
|---|---|---|
| `zenomen` | `src/main.rs` | Unprivileged Slint desktop GUI |
| `zenomen-hw` | `src/bin/zenomen-hw.rs` | Privileged system D-Bus hardware daemon |
| `libzenomen` | `src/lib.rs` | Hardware providers, service contract/client, config, RGB, desktop integration, Slint bridge |

`src/lib.rs:7`–`14` exposes `app`, `config`, `core`, `desktop`, `hardware`, `rgb`, `service`, then expands the compiled Slint UI with `slint::include_modules!()`.

Licensing is **GPL-3.0-only** (`Cargo.toml:6`); `NOTICE` preserves the OpenOMEN attribution, GPL notice, and the HP trademark disclaimer, and `LICENSE` ships in the `.deb` (`Cargo.toml:74`). Keep both files when packaging.

Supporting docs: `docs/PACKAGING.md` (DEB/AppImage contents), `docs/RESOURCE-BUDGET.md` (measured RSS/CPU), `docs/PARITY.md` (legacy→Rust requirement parity). `dist/` is gitignored packaging output, not source.

## Languages

**Primary:**
- **Rust, edition `2024`** — `Cargo.toml:5`. Edition 2024 requires rustc 1.85 or newer; there is no `rust-version` field and no `rust-toolchain.toml`, so the effective MSRV is the edition floor. CI and release install `stable` (`.github/workflows/ci.yml:36`–`40`, `.github/workflows/release.yml:38`–`42`).
- **Slint DSL (`.slint`)** — declarative UI, compiled into the binary at build time. Root component `AppWindow` in `ui/app.slint`; reusable widgets in `ui/components/`, screens in `ui/screens/`, design tokens in `ui/theme/`.

**Secondary:**
- **Bash** — `scripts/measure-budget.sh`, the RSS/CPU budget harness. Not shipped and not part of the product.
- **TOML** — user configuration at `~/.config/zenomen/config.toml`; schema in `src/config/schema.rs`.
- **JSON** — RGB profiles (`src/rgb/schema.rs`), D-Bus payloads (`src/service/contract.rs`), diagnostics export (`src/core/diagnostics.rs`), legacy profile import (`src/rgb/migration.rs`).

## Runtime

**Environment:**
- Linux desktop with **system D-Bus**, session D-Bus, systemd, polkit, and an OpenGL 2.1+ (or llvmpipe software) driver. `README.md:148`–`190` lists distro packages.
- **GUI process:** single Slint/winit event loop, plus dedicated named background threads:
  - `zenomen-monitor-worker` — 1 Hz sampling loop (`src/core/worker.rs:38`–`39`)
  - `zenomen-event-pump` — worker → UI event bridge (`src/main.rs:222`–`223`)
  - `zenomen-desktop-actions` — tray / single-instance actions (`src/main.rs:138`–`139`)
  - `zenomen-nv-query` — non-blocking `nvidia-smi` helper (`src/hardware/linux/gpu.rs:190`–`191`)
  - `zenomen-ctrl-*` / `zenomen-rpc-*` — per-operation mutating RPC threads with a 30 s budget (`src/core/worker.rs:350`–`379`)
  - `zenomen-diag-export` — asynchronous diagnostics file export (`src/app.rs:227`–`228`)
- **Service process:** `zbus::block_on` async executor hosting one D-Bus interface, kept alive with `std::future::pending::<()>().await` (`src/bin/zenomen-hw.rs:23`–`46`).
- `zbus` is used in **blocking** mode in the GUI (`src/service/client.rs:5`, `src/desktop/single_instance.rs:8`, `src/desktop/tray.rs:9`) and in async mode inside the daemon.
- **No Tokio runtime is created by application code.** `tokio` appears in `Cargo.lock` only through `wasm-bindgen-futures` (wasm-target plumbing), and no source references `tokio`, `tokio::runtime`, or `#[tokio::main]`.
- Threading is explicitly budgeted: the UI thread never reads sensors; all sampling runs on the monitor worker and all mutations on short-lived RPC threads (`src/core/worker.rs:1`–`18`).

**Package Manager:**
- **Cargo**, lockfile **present and committed** — `Cargo.lock` (lockfile format 4, **557 packages**). `.gitignore` deliberately does not ignore it so `cargo audit` audits the exact shipping graph.
- No `rust-toolchain.toml`, no `.cargo/config.toml`, no `rustfmt.toml`, no `clippy.toml`.

## Frameworks

**Core:**

| Framework | Declared | Locked | Purpose |
|---|---|---|---|
| `slint` | `1.17` | **1.18.1** | Entire GUI. `default-features = false` with only `std`, `compat-1-2`, `backend-winit`, `renderer-femtovg` (`Cargo.toml:11`–`19`). Qt and Skia backends are deliberately excluded to protect the ≤40 MB RSS budget. |
| `slint-build` | `1.17` | **1.18.1** | Build-time `.slint` compiler, invoked from `build.rs`. |
| `zbus` | `5` | **5.19.0** | System-bus daemon + client, session-bus single-instance service, and StatusNotifierItem/DBusMenu tray. Only the `blocking` feature is enabled. |
| `serde` + `serde_json` | `1.0` / `1.0` | 1.0.229 / 1.0.154 | Config, RGB schema, D-Bus JSON payloads, diagnostics. |
| `toml` | `0.8` | **0.8.23** | Configuration parse/serialize (`src/config/mod.rs:48`, `:92`). |
| `tracing` + `tracing-subscriber` | `0.1` / `0.3` (`env-filter`) | 0.1.44 / 0.3.23 | Structured logs in both binaries. |

**Slint configuration:**
- `build.rs:8`–`11` compiles `ui/app.slint` with the **`fluent`** widget style pinned in code (not `SLINT_STYLE`) so developer and CI rendering match.
- Fonts are vendored and embedded through `ui/theme/typography.slint:11`–`13`: `Inter-Regular.ttf`, `Inter-SemiBold.ttf`, `JetBrainsMono-Regular.ttf` in `ui/theme/fonts/` (SIL OFL 1.1, licence texts alongside). Do not replace with system fonts.
- `std-widgets` is used (e.g. `ScrollView` in `ui/app.slint:16`), which the `fluent` style supplies.

**Testing:**
- Built-in `cargo test`. No external test framework, no mock/snapshot crate, **no `[dev-dependencies]`** (`Cargo.toml` has none).
- 15 integration test files in `tests/` (79 `#[test]` functions) plus 31 inline `#[cfg(test)]` unit tests — 110 total at this commit. `README.md:226` still says "108 tests across 15 suites"; the README count lags the code.
- GUI tests instantiate the Slint window and therefore need a display; run `xvfb-run --auto-servernum cargo test --all` (`.github/workflows/ci.yml:62`–`65`).
- Test doubles are hand-written (`src/hardware/mock.rs`) and temporary directories are created manually.

**Build/Dev:**

| Tool | Version | Purpose |
|---|---|---|
| `cargo-packager` | CI-installed via `taiki-e/install-action` | DEB + AppImage from `[package.metadata.packager]` (`Cargo.toml:46`–`75`). |
| `cargo-audit` | CI-installed | Blocking advisory gate against `Cargo.lock`. |
| `cargo fmt` / `cargo clippy` | stable toolchain components | `fmt --check` and `clippy --all-targets --all-features -D warnings`. |
| `xvfb` | distro package | Headless display for tests. |
| `winres` | `0.1` | Windows-only build dependency embedding `data/icons/zenomen.ico` (`build.rs:12`–`19`). Windows has no hardware provider and is unadvertised. |
| `bash` + `/proc` | local | `scripts/measure-budget.sh` measures RSS and CPU against `docs/RESOURCE-BUDGET.md`. |

## Key Dependencies

**Critical:**

| Crate | Locked | Why it matters | Files that would change |
|---|---|---|---|
| `slint` | 1.18.1 | All UI, models, callbacks, event loop | `src/app.rs`, `src/main.rs`, `ui/**` |
| `slint-build` | 1.18.1 | Compiles every `.slint` change | `build.rs`, `ui/app.slint` |
| `zbus` | 5.19.0 | Every IPC path: daemon, client, single instance, tray | `src/service/`, `src/desktop/single_instance.rs`, `src/desktop/tray.rs` |
| `serde` / `serde_json` | 1.0.229 / 1.0.154 | Derives on `AppConfig`, `RgbProfile`, `HardwareCapabilities`, `TelemetrySnapshot`, `PresetResult`, `ServiceConnectionStatus` | `src/config/schema.rs`, `src/rgb/schema.rs`, `src/hardware/provider.rs`, `src/service/types.rs` |
| `toml` | 0.8.23 | Config round-trip | `src/config/mod.rs` |

**Infrastructure:**
- `thiserror` 2.0.21 — typed errors in `src/service/types.rs`, `src/rgb/schema.rs`, `src/desktop/releases.rs`, `src/desktop/system_integration.rs`.
- `directories` 6.0.0 — `ProjectDirs::from("com", "zenomen", "zenomen")` for `~/.config/zenomen/…` (`src/config/mod.rs:20`–`22`) and `BaseDirs` for autostart and legacy OpenOMEN paths.
- `tracing` / `tracing-subscriber` — stderr logging only; no file appender, no remote sink.
- `anyhow` 1.0.104 — **declared but unreferenced anywhere in `src/` or `tests/`**. Remove it or leave it unused; do not build an error-handling convention around it.

**Notable transitive crates (locked):**
- `winit` 0.30.13 — window/event-loop backend selected by Slint's `backend-winit`.
- `femtovg` 0.27.0 — OpenGL renderer selected by `renderer-femtovg`.
- `rustix` 1.1.6, `async-io` 2.6.0, `nix` 0.31.3 — Linux syscall/async plumbing beneath `zbus` and `winit`.
- `i-slint-backend-linuxkms` and `i-slint-renderer-software` appear in the lock as resolved optional packages but are **not enabled** by the feature set; only winit + FemtoVG is compiled.

**Version-pinning caveat:** `Cargo.toml:11`/`:13` say "pinned to 1.17", but `1.17` is a caret requirement and the lock resolves **1.18.1**. Either pin `=1.17` or update the comment when version policy is decided; do not describe 1.17 as an exact lock.

## Configuration

**Environment:**
- User config: **`~/.config/zenomen/config.toml`**, resolved via `directories::ProjectDirs` (`src/config/mod.rs:20`–`29`). Missing file → defaults; unparseable file → timestamped backup (`config.toml.bak.<unix-seconds>`) plus safe defaults and a UI notice (`src/config/mod.rs:34`–`78`).
- Schema (`src/config/schema.rs`):
  - `[general]`: `theme` (default `"dark"`), `start_minimized` (`false`), `close_to_tray` (`true`), `autostart` (`false`).
  - `[telemetry]`: `history_window` (default `120`; sanitized to one of `30/60/120/300`) and `polling_interval_ms` (default `1000`, clamped to **500–5000**).
- Writes are atomic: temp file → `sync_all` → rename → parent-directory `sync_all` (`src/config/mod.rs:85`–`116`).
- RGB profiles: `~/.config/zenomen/rgb/*.json`; legacy import source `~/.config/openomen/rgb/*.json` is read-only (`src/rgb/migration.rs:42`–`48`).
- Diagnostics exports: `~/.config/zenomen/diagnostics/zenomen-diagnostics-<timestamp>.json` (`src/app.rs:233`–`242`).
- Autostart entry: `~/.config/autostart/zenomen.desktop` (`src/desktop/autostart.rs:9`–`30`).

**Environment variables (all optional):**

| Variable | Effect | Source |
|---|---|---|
| `RUST_LOG` | Log filter; GUI calls `tracing_subscriber::fmt::init()`, daemon uses `EnvFilter` defaulting to `info` | `src/main.rs:75`, `src/bin/zenomen-hw.rs:12`–`16` |
| `ZENOMEN_MOCK_SCENARIO` | `nominal` / `zero` / `gap` switches to `MockHardware` | `src/main.rs:168`–`183`, `src/hardware/mock.rs:60`–`80` |
| `ZENOMEN_BENCHMARK=1` | Emits `zenomen-benchmark stage=… rss_mb=…` lines | `src/core/benchmark.rs:245`–`256` |
| `ZENOMEN_HW_SESSION` | Binds `zenomen-hw` to the **session** bus for local testing | `src/bin/zenomen-hw.rs:26`–`30` |
| `LC_CTYPE` / `LC_ALL` / `LANG` | Read at GUI start to inject `LC_CTYPE=C.UTF-8` when the locale lacks a UTF-8 suffix (xkbcommon Compose parsing) | `src/main.rs:21`–`31` |

**Build:**
- `Cargo.toml` — package metadata, Slint features, release profile, cargo-packager metadata.
- `build.rs` — Slint compile with `fluent` style; Windows icon resource under `cfg(windows)`.
- `[profile.release]`: `opt-level = "s"`, `lto = true`, `codegen-units = 1`, `panic = "abort"`, `strip = true` — size/RSS oriented, not throughput oriented (`Cargo.toml:37`–`44`). Keep `panic = "abort"` in mind: no unwinding-based recovery in release.
- `.github/workflows/ci.yml` — fmt, clippy, headless tests, `cargo audit`; `permissions: contents: read`.
- `.github/workflows/release.yml` — tag/version check, same gates, `cargo packager --release --formats deb,appimage`, `SHA256SUMS`, GitHub Release. Release actions are pinned to commit SHAs; CI's `actions/checkout@v4` and `taiki-e/install-action` are tag-referenced.
- Latest CI change (`30d62de`) restricts release builds to **Linux DEB + AppImage**; there is no Windows packaging job.

## Platform Requirements

**Development:**
- Rust 1.85+ (edition 2024) with `clippy` and `rustfmt`.
- Ubuntu/Debian: `build-essential`, `pkg-config`, `libfontconfig1-dev`, `libx11-dev`, `libxkbcommon-dev`, `libdbus-1-dev`, `xvfb` (`README.md:150`–`163`).
- Fedora: `gcc`, `pkg-config`, `fontconfig-devel`, `libX11-devel`, `libxkbcommon-devel`, `dbus-devel`, `xorg-x11-server-Xvfb`.
- Arch: `base-devel`, `pkgconf`, `fontconfig`, `libx11`, `libxkbcommon`, `dbus`, `xorg-server-xvfb`.
- `nvidia-smi` is optional; absent NVIDIA tooling degrades to no NVIDIA telemetry (`src/hardware/linux/provider.rs:65`–`67`).

**Production:**
- Debian/Ubuntu `.deb` depends on `libxkbcommon0`, `libfontconfig1`, `libgl1`, `libdbus-1-3` (`Cargo.toml:64`–`65`).
- Portable `.AppImage`; first run needs `sudo ./ZenOmen_*.AppImage --install-system-integration` to install `zenomen-hw`, the systemd unit, D-Bus policy/service, polkit policy, desktop entry, and icons (`src/desktop/system_integration.rs:12`–`26`, `docs/PACKAGING.md:85`–`97`).
- Required host services: systemd, polkit, system D-Bus. Hardware interfaces come from the `hp-wmi` kernel module (fan + RGB sysfs) and ACPI `platform_profile` (power presets).
- Wayland or X11; OpenGL 2.1+ with llvmpipe fallback (`README.md:262`–`267`).

## Deliberately Absent

- **No network stack** — no `reqwest`, `ureq`, `hyper`, `rustls`, `native-tls`, `openssl`, `curl`, or `isahc` in `Cargo.lock` (all zero matches). The only URL is handed to the user's browser.
- **No database**, no embedded KV store, no cloud SDK, no telemetry upload.
- **No async runtime constructed by ZenOmen** (no Tokio executor, no `async-std` main).
- **No Qt/Skia Slint backends**, no accessibility feature, no JS/Node toolchain.
- **No Python** — the legacy implementation is retired; `AGENTS.md` still embeds its documentation and is not authoritative.

---

*Stack analysis: 2026-11-06*
