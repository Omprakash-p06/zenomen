<!-- refreshed: 2026-11-11 -->
# Codebase Structure

**Analysis Date:** 2026-11-11

## Directory Layout

```
zenomen/
├── Cargo.toml / Cargo.lock        # single package, two binaries, release profile for RSS
├── build.rs                       # Slint compile (ui feature) + Windows resources
├── AGENTS.md / README.md / LICENSE / NOTICE
├── .github/workflows/             # ci.yml (both OS), release.yml (DEB/AppImage/EXE/MSI/ZIP)
├── src/
│   ├── main.rs                    # GUI entry point
│   ├── lib.rs                     # module surface; slint::include_modules! under `ui` feature
│   ├── app.rs                     # Slint ⇄ core bridge
│   ├── logging.rs                 # stderr + rotating file subscriber
│   ├── bin/zenomen-hw.rs          # privileged service entry point
│   ├── core/                      # domain contract, state, worker, telemetry, diagnostics
│   ├── hardware/                  # read-only telemetry providers and sensor registry
│   │   ├── linux/                 # procfs/sysfs/DRM/nvidia-smi
│   │   └── windows/               # Win32/DXGI/PDH/GDI/registry
│   ├── service/                   # privileged control boundary
│   │   ├── contract.rs            # Linux D-Bus interface
│   │   ├── backend.rs             # Linux sysfs writes + verification
│   │   ├── polkit.rs              # Polkit CheckAuthorization
│   │   ├── client.rs              # platform-neutral client facade
│   │   └── windows/               # pipe protocol, security, server, WMI driver, boards
│   ├── desktop/                   # single instance, tray, autostart, integration installer
│   │   └── windows/               # Win32 equivalents
│   ├── config/                    # TOML schema + atomic persistence
│   ├── rgb/                       # profile schema, presets, legacy migration
│   └── ui/                        # dormant GPUI-CE view-model scaffold (not wired to a binary)
├── ui/                            # live Slint UI
│   ├── app.slint                  # AppWindow + screen routing
│   ├── screens/                   # overview, cooling, performance, lighting, settings, diagnostics
│   ├── components/                # sidebar, cards, charts, status strip, icons
│   └── theme/                     # colors, metrics, typography + bundled fonts
├── data/                          # installed system assets
│   ├── applications/zenomen.desktop
│   ├── dbus/                      # com.zenomen.Hardware1.conf / .service
│   ├── polkit/                    # com.zenomen.hardware.policy
│   ├── systemd/zenomen-hw.service
│   └── icons/                     # hicolor PNGs + SVG + zenomen.ico
├── packaging/
│   ├── linux/                     # AppRun, build-appimage.sh, debian/{postinst,prerm,postrm}
│   └── windows/                   # build-installer.ps1, inno/ZenOmen.iss, wix/ZenOmen.wxs
├── scripts/                       # measure-budget.sh, generate-icons.sh, validate-icons.py, branding/
├── tests/                         # 19 integration suites (238 tests)
├── docs/                          # architecture, decisions, release, testing
└── .planning/                     # GSD state, roadmap, references, this codebase map
```

## Directory Purposes & Module Boundaries

### `src/core/` — Domain Core (no hardware)
- `events.rs` — `AppCommand` (UI → core), `AppEvent` (core → UI), `ScreenId` (compile-time six-screen set).
- `state.rs` — `AppState` + `apply`; `status_text`, `is_degraded`; 10 co-located tests.
- `worker.rs` — `spawn_monitoring_worker`, `sample_and_dispatch`, `execute_mutating_action`; one-in-flight mutation guard.
- `telemetry.rs` — `TelemetryHistory` (bounded), `TelemetryPoint`, SVG path generation.
- `profile_orchestrator.rs` — `resolve_preset_targets`, `apply_system_profile`, display helpers.
- `diagnostics.rs` — `DiagnosticsReport`, `collect_diagnostics[_from_roots]`, atomic export.
- `benchmark.rs` — RSS/PSS/private-dirty and CPU sampling, `log_benchmark`.

### `src/hardware/` — Read-Only Telemetry
- `provider.rs` — data types + `HardwareProvider`; the only seam the core knows.
- `registry.rs` — `SensorRegistry`; canonical discovery/priority.
- `linux/` — `cpu.rs`, `gpu.rs`, `hwmon.rs`, `memory.rs`, `provider.rs`.
- `windows/` — `provider.rs`, `cpu.rs`, `gpu.rs`, `memory.rs`, `sensors.rs`, `display.rs`, `battery.rs`, `device.rs`.
- `mock.rs` — `MockHardware` + `MockScenario` (`nominal`, `zero`, `gap`), used by `ZENOMEN_MOCK_SCENARIO` and tests.
- `unavailable.rs` — zero-capability provider for unsupported OSes.
- `process_memory.rs` — GUI + service working set/private/peak, combined with a 10 s freshness rule.
- `rgb.rs` — `RgbInterfaceVariant` detection.

### `src/service/` — Privileged Control Seam
- `types.rs` — `FanMode`, `FanControlState`, `PresetResult`, `PowerProfileResult`, `PowerProfileStatus`, `HardwareControlError`.
- `protocol.rs` — RGB/player protocol helpers and 6 co-located tests.
- `client.rs` — `HardwareServiceClient`, `ServiceConnectionStatus`, Polkit readiness query.
- `backend.rs` — Linux `HardwareBackend` + `write_and_verify`.
- `contract.rs`, `polkit.rs` — Linux D-Bus interface and authorization.
- `windows/` — 15 modules; see below.

`src/service/windows/` detail:
| File | Role |
|---|---|
| `protocol.rs` | `HardwareRequest`/`HardwareResponse`, 64 KiB length-prefixed JSON frames |
| `pipe.rs` | Pipe creation (`\\.\pipe\ZenOmen.Hardware.v1`), frame read/write helpers |
| `security.rs` | Pipe DACL (SDDL), interactive-session verification |
| `server.rs` | `handle_request` dispatch + `run_pipe_server` loop + shutdown restore |
| `lifecycle.rs` | SCM register/uninstall, service main, console fallback |
| `client.rs` | Unprivileged pipe client with retry/backoff |
| `authorization.rs` | `authorize_mutation` board/validation policy |
| `profiles.rs` | `BoardProfile`, `BoardCapabilities`, `BoardValidation`, catalog and validation table |
| `hp_wmi.rs` | Real COM driver, `WmiAdapter`, `FakeWmiAdapter`, firmware operations |
| `wmi_ffi.rs` | Minimal COM/BSTR/VARIANT/SAFEARRAY FFI |
| `victus_s.rs` | Victus S fan-table parsing, thermal profiles, validation state machine |
| `validation_runbook.rs` | `--validate-8a3d-fan` hardware procedure |
| `power_validation_runbook.rs` | `--validate-8a3d-power` hardware procedure |
| `wmi_diag.rs` | `--wmi-diag` read-only probes |

### `ui/` — Live Declarative Slint UI
- `app.slint` — `AppWindow` with theme imports, sidebar, status strip, six screen components; all bindable properties (`cpu-path`, `fan-mode`, `power-profile-verified`, `zone-colors`, memory fields, etc.) are set from `src/app.rs`.
- `screens/*.slint` — one file per `ScreenId` in sidebar order.
- `components/*.slint` — reusable cards, charts, pills, `unavailable-panel`, icons.
- `theme/*.slint` — `ThemeColors` (Obsidian/Charcoal/Slate/Gold), `ThemeMetrics` (4 px grid), `ThemeTypography`; fonts in `ui/theme/fonts/`.
- Compile-time style is pinned to `fluent` by `build.rs`.

### `src/ui/` — Dormant GPUI-CE Scaffold
- `app.rs` (`GpuiApp`), `components/*.rs`, `screens/*.rs`, `theme/*.rs`.
- Compiled by `lib.rs` under the `ui` feature but never constructed by `src/main.rs`; documented in `docs/decisions/ADR-002-gpui-ce-migration.md`.

### `src/config/` & `src/desktop/` — Persistence and OS Integration
- `config/schema.rs` — `AppConfig`, bounds, `sanitize`; `config/mod.rs` — load/atomic save/backup.
- `desktop/mod.rs` re-exports the platform-specific surface; Linux paths in `single_instance.rs`, `tray.rs`, `autostart.rs`; Windows equivalents under `desktop/windows/`.
- `system_integration.rs` embeds `data/` assets via `include_str!` and installs them under a root prefix (testable).

## Key File Locations

| Concern | Location |
|---|---|
| GUI entry | `src/main.rs` |
| Service entry | `src/bin/zenomen-hw.rs` |
| Slint bridge | `src/app.rs` |
| State contract | `src/core/state.rs`, `src/core/events.rs` |
| Sampling loop | `src/core/worker.rs` |
| Provider contract | `src/hardware/provider.rs` |
| Sensor discovery | `src/hardware/registry.rs` |
| Linux driver | `src/hardware/linux/provider.rs` |
| Windows telemetry | `src/hardware/windows/provider.rs` |
| Linux privileged writes | `src/service/backend.rs` |
| Linux IPC/auth | `src/service/contract.rs`, `src/service/polkit.rs` |
| Windows IPC | `src/service/windows/{protocol,pipe,security,server,client}.rs` |
| Windows board safety | `src/service/windows/{profiles,authorization}.rs` |
| Windows HP driver | `src/service/windows/hp_wmi.rs`, `victus_s.rs` |
| Config | `src/config/{mod,schema}.rs` |
| Logging | `src/logging.rs` |
| Diagnostics | `src/core/diagnostics.rs` |
| Slint UI root | `ui/app.slint` |
| Installed assets | `data/**` |
| Packaging | `packaging/**`, `Cargo.toml` `[package.metadata.packager]` |
| Budget harness | `scripts/measure-budget.sh` |
| Integration tests | `tests/*.rs` |

## Naming Conventions & Code Style

- Modules are singular nouns (`core`, `service`, `hardware`, `desktop`, `rgb`); platform subtrees repeat the same file names (`provider.rs`, `client.rs`, `mod.rs`).
- Rust item names: `snake_case` functions, `UpperCamelCase` types, `SCREAMING_SNAKE_CASE` constants (e.g. `POLKIT_ACTION_CONTROL`, `PIPE_NAME`, `HPWMI_GM`, `EXPECTED_SENSOR_COUNT`).
- Domain identifiers are lowercase strings that cross IPC unchanged: `auto`, `max`, `quiet`, `balanced`, `performance`, `gaming`, `nominal`, `zero`, `gap`.
- Slint properties are `kebab-case` in `.slint` (`has-fan-control`, `pending-fan-mode`) and `set_*`/`on_*` accessors in Rust.
- Tests are descriptive sentences: `a_second_mutation_is_rejected_while_one_is_in_flight`, `test_protocol_rejects_payload_exceeding_64kb`.
- Files carry a `//!` module contract as the first item; non-obvious decisions are explained in comments rather than left implicit.
- Windows entry points are grouped in `src/desktop/windows/`; Linux-only code is `#[cfg(target_os = "linux")]` or `#[cfg(unix)]`.

## Where to Add New Code

### 1. Adding a New Hardware Sensor
1. Read it in the relevant provider (`src/hardware/linux/provider.rs` or `src/hardware/windows/provider.rs`).
2. Add an optional field to `TelemetrySnapshot` (`src/hardware/provider.rs`) and, if it is a *sensor*, bump `EXPECTED_SENSOR_COUNT` and `reported_sensor_count`.
3. Update `MockHardware` (`src/hardware/mock.rs`) and the mock scenarios.
4. Surface it in `src/app.rs::apply_to_ui` and the matching `.slint` screen; render empty as "Unavailable".
5. Add coverage in `tests/sensor_registry_tests.rs` / `tests/degradation_tests.rs`.

### 2. Adding a New UI Screen
1. Add a variant to `ScreenId` (`src/core/events.rs`) **and** insert it in `ScreenId::ALL` with an `index()` arm — the array order is the routing contract.
2. Create `ui/screens/<name>.slint` and wire it into `ui/app.slint`.
3. Add the Rust view-model only if `src/ui/` is being revived; the live UI needs nothing there.
4. Emit `AppEvent::ScreenChanged` from the worker; `src/app.rs` sets `active-screen`.

### 3. Adding a User Action / UI Command
1. Add the callback in `ui/screens/*.slint`.
2. Add an `AppCommand` variant (`src/core/events.rs`) and a matching `on_*` handler in `src/app.rs::install_callbacks` (queue, never block).
3. Handle it in `src/core/worker.rs`, producing an `AppEvent`.
4. Add an `AppState::apply` arm plus tests, and an `apply_to_ui` projection.

### 4. Adding a Privileged Mutation / IPC Operation
1. Add a variant to `HardwareRequest`/`HardwareResponse` (`src/service/windows/protocol.rs`) for Windows.
2. Add the authorization rule in `authorize_mutation` — read-only must remain unconditional; mutating must check `BoardValidation`.
3. Implement in the driver (`hp_wmi.rs`) with pre-write read, write, settle, post-write read.
4. Dispatch in `handle_request` (`server.rs`) and expose a client method.
5. On Linux, add the D-Bus method in `src/service/contract.rs` (call `self.authorize(&hdr)`), implement in `src/service/backend.rs` with `write_and_verify`, and add the client call in `src/service/client.rs`.
6. Add tests with `FakeWmiAdapter` / temp-dir sysfs roots.

### 5. Adding a Configuration Option
1. Add the field with `#[serde(default)]` in `src/config/schema.rs` and a default function.
2. Add bounds in `AppConfig::sanitize`.
3. Apply it in `src/core/worker.rs` (runtime) and/or `src/main.rs` (startup).
4. Reflect it in `ui/screens/settings.slint` and `src/app.rs` property writes.

---

*Structure analysis: 2026-11-11*
