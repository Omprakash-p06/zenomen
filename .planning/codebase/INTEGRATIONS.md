---
last_mapped_commit: 761e5f975d7c22d1c48bf777a0bcaa1f2db582ba
last_mapped_at: 2026-11-07
---
# External Integrations

**Analysis Date:** 2026-11-07

## Overview

ZenOmen has no HTTP API clients, cloud services, databases, or webhooks. Its external
surface is entirely local:

- D-Bus on the system bus (privileged hardware control) and session bus
  (single-instance, tray).
- Polkit authorization through `org.freedesktop.PolicyKit1`.
- Linux kernel pseudo-filesystems: procfs, sysfs, DMI, ACPI, DRM, hwmon.
- Three external process invocations: optional `nvidia-smi`, root-only `systemctl`,
  and user-delegated `xdg-open`.

The only "update check" opens the GitHub Releases page in the user's browser; no
version check, download, or self-update code exists.

## APIs & External Services

### System D-Bus — Hardware Control Service

- **Bus name:** `com.zenomen.Hardware1`
- **Object path:** `/com/zenomen/Hardware1`
- **Interface:** `com.zenomen.Hardware1`
- **Server:** `zenomen-hw` (`src/bin/zenomen-hw.rs`, `src/service/contract.rs`)
- **Client:** `HardwareServiceClient` (`src/service/client.rs`)
- **SDK/client library:** `zbus` 5.19.0 with the `blocking` feature
- **Activation file:** `data/dbus/com.zenomen.Hardware1.service` → `/usr/bin/zenomen-hw`
- **Bus policy:** `data/dbus/com.zenomen.Hardware1.conf` — only `root` may own the name;
  any local user may send/receive messages.
- **Service manager:** `data/systemd/zenomen-hw.service` (`Type=dbus`,
  `BusName=com.zenomen.Hardware1`, `ExecStart=/usr/bin/zenomen-hw`).

**Methods:**

| Method | Mutating | Polkit required | Return |
|---|---|---|---|
| `GetCapabilities` | No | No | JSON string: fan/power/RGB availability and variant |
| `GetFanStatus` | No | No | `(String mode, Vec<u32> rpms)` |
| `SetFanMode(mode)` | Yes | `com.zenomen.hardware.control` | Confirmed mode string |
| `GetPowerProfiles` | No | No | Advertised ACPI profile names |
| `GetPowerProfile` | No | No | Active ACPI profile |
| `SetPowerProfile(profile)` | Yes | `com.zenomen.hardware.control` | Confirmed profile |
| `ApplyPreset(preset)` | Yes | `com.zenomen.hardware.control` | `PresetResult` serialized as JSON |
| `ApplyRgbProfile(profile_json)` | Yes | `com.zenomen.hardware.control` | Confirmation string |
| `SetRgbEnabled(enabled)` | Yes | `com.zenomen.hardware.control` | Confirmation string |

Mutating methods call `PolkitVerifier::check_authorization` with the caller's D-Bus
sender name (`src/service/contract.rs:55-80`). The service is root-owned and sandboxed
by systemd; the GUI never performs raw sysfs writes.

**Client behavior:** caches the system-bus connection; checks liveness with
`GetCapabilities`; maps `ServiceUnknown`/missing `.service` errors to a user-readable
"daemon not running" message (`src/service/client.rs:58-98`, `232-249`).

### Session D-Bus — Single Instance

- **Bus name / interface:** `com.zenomen.App`
- **Object path:** `/com/zenomen/App`
- **Methods:** `PresentWindow`, `Activate`, `Quit`
- **Purpose:** second launch asks the primary instance to show its window, then exits
  (`src/desktop/single_instance.rs`).
- **Name acquisition:** `RequestName` with flag `4` (`DO_NOT_QUEUE`); primary owner
  registers the object server. If the session bus is unavailable, the app runs
  standalone rather than failing (`SingleInstanceResult::Disabled`).

### Session D-Bus — System Tray

- **`org.kde.StatusNotifierItem`** at `/StatusNotifierItem` — properties include
  `Category`, `Id`, `Title`, `Status`, `IconName`, `Menu`; methods `Activate`,
  `SecondaryActivate`, `ContextMenu`, `Scroll` (`src/desktop/tray.rs`).
- **`com.canonical.dbusmenu`** at `/MenuBar` — `GetLayout`, `Event`, `AboutToShow`,
  with "Open ZenOmen" and "Quit ZenOmen" items.
- **Registration:** calls `RegisterStatusNotifierItem` on
  `org.kde.StatusNotifierWatcher` at `/StatusNotifierWatcher`. A missing watcher is
  logged at info level and treated as optional (e.g., stock GNOME without an
  AppIndicator extension).

### Polkit — Authorization

- **Service:** `org.freedesktop.PolicyKit1`
- **Object path:** `/org/freedesktop/PolicyKit1/Authority`
- **Interface:** `org.freedesktop.PolicyKit1.Authority`
- **Method:** `CheckAuthorization(subject, action_id, details, flags, cancellation_id)`
- **Subject:** `("system-bus-name", { "name": <caller unique bus name> })`
- **Action ID:** `com.zenomen.hardware.control`
- **Policy file:** `data/polkit/com.zenomen.hardware.policy`
- **Defaults:** `allow_any=auth_admin`, `allow_inactive=auth_admin`,
  `allow_active=auth_admin_keep`.
- **Call sites:**
  - Daemon enforcement with interactive prompt allowed (`flags = 1`):
    `src/service/polkit.rs:28-71`.
  - GUI diagnostics with no prompt (`flags = 0`):
    `src/service/client.rs:251-299`.

### GitHub Releases — Browser-Delegated Update Link

- **URL constant:** `https://github.com/Omprakash-p06/zenomen/releases/latest`
  (`src/desktop/releases.rs:13`).
- **Mechanism:** spawns `xdg-open <URL>` with stdin/stdout/stderr set to null and reaps
  the child on a background thread (`src/desktop/releases.rs:26-44`).
- **No network client, no download, no signature check, no self-update.**
- The URL is opened only from the UI callback `on_open_releases_page`
  (`src/app.rs:265-272`).

## Kernel Interfaces (sysfs / procfs / DMI / ACPI)

These are the primary "external APIs" for telemetry and hardware control. Reads happen
in the unprivileged GUI process; writes happen only in `zenomen-hw`
(`src/service/backend.rs`).

### sysfs — sensors and controls

| Path / pattern | Access | Used for | Source |
|---|---|---|---|
| `/sys/class/hwmon/hwmon*/name` | Read | Driver identity (`k10temp`, `coretemp`, `acpitz`, `hp`, `hp-wmi`, GPU drivers). | `src/hardware/linux/hwmon.rs:32-62`, `src/hardware/registry.rs:38-82` |
| `/sys/class/hwmon/hwmon*/temp*_input` | Read | Temperature in millidegrees Celsius. | `src/hardware/linux/hwmon.rs:64-71` |
| `/sys/class/hwmon/hwmon*/temp*_label` | Read | Sensor label selection (`Tctl`, `Tdie`, `Package id 0`). | `src/hardware/linux/hwmon.rs:79-86` |
| `/sys/class/hwmon/hwmon*/fan*_input` | Read | Fan tachometer RPM. | `src/hardware/linux/hwmon.rs:73-77`, `src/hardware/registry.rs:90-96` |
| `/sys/class/hwmon/hwmon*/pwm1_enable` | Read | Fan mode (`0=max`, `1=manual`, `2=auto`). | `src/hardware/registry.rs:98-114` |
| `/sys/devices/platform/hp-wmi/hwmon/hwmon*/pwm1_enable` | Read/write | Primary HP fan-mode control interface; fallback scan of `/sys/class/hwmon`. | `src/service/backend.rs:76-124` |
| `/sys/firmware/acpi/platform_profile` | Read/write | Active ACPI platform profile; write is validated and read back. | `src/service/backend.rs:217-265` |
| `/sys/firmware/acpi/platform_profile_choices` | Read | Advertised profile names; writes are rejected unless listed. | `src/service/backend.rs:195-258`, `src/hardware/registry.rs:123-130` |
| `/sys/devices/platform/hp-wmi/rgb_zones/zone0{0..3}` | Read/write | Four-zone RGB interface (hex per zone). | `src/hardware/rgb.rs:33-54`, `src/service/backend.rs:390-461` |
| `/sys/devices/platform/hp-wmi/keyboardleds` | Read/write | Legacy monolithic 24-hex-character RGB interface. | `src/hardware/rgb.rs:41-45`, `src/service/backend.rs:409-473` |
| `/sys/class/leds/hp_omen::kbd_backlight/zone_colors` | Read/write | Kernel-patch RGB interface (12 raw bytes). | `src/hardware/rgb.rs:47-51`, `src/service/backend.rs:428-508` |
| `/sys/class/drm/card[0-9]+/device/vendor` | Read | PCI vendor ID (AMD `0x1002`, Intel `0x8086`, NVIDIA `0x10de`). | `src/hardware/linux/gpu.rs:63-105` |
| `/sys/class/drm/card[0-9]+/device/gpu_busy_percent` | Read | AMD GPU utilization. | `src/hardware/linux/gpu.rs:144-156` |
| `/sys/class/drm/card[0-9]+/device/mem_info_vram_used` / `mem_info_vram_total` | Read | AMD VRAM usage/total (bytes converted to MB). | `src/hardware/linux/gpu.rs:167-178` |
| `/sys/class/drm/card[0-9]+/device/hwmon/hwmon*/temp1_input` | Read | GPU temperature in millidegrees. | `src/hardware/linux/gpu.rs:117-165` |
| `/sys/devices/system/cpu/cpu0/cpufreq/scaling_cur_freq` | Read | CPU frequency in kHz; falls back to `/proc/cpuinfo` `cpu MHz`. | `src/hardware/linux/provider.rs:133-140` |
| `/sys/class/power_supply/BAT*/capacity` | Presence check only | Battery capability flag; battery telemetry is not sampled. | `src/hardware/registry.rs:68-71`, `153-169` |
| `/sys/class/dmi/id/product_name`, `/sys/class/dmi/id/sys_vendor` | Read | DMI/SMBIOS model and vendor for diagnostics. | `src/core/diagnostics.rs:58-86` |

The daemon's systemd unit grants write access only to
`-/sys/devices/platform/hp-wmi`, `-/sys/firmware/acpi`, `-/sys/class/hwmon`, and
`-/sys/class/leds`; the rest of the filesystem is read-only
(`data/systemd/zenomen-hw.service:14-32`).

### procfs and system metadata

| Path / pattern | Access | Used for | Source |
|---|---|---|---|
| `/proc/stat` | Read | CPU utilization deltas and per-core jiffies. | `src/hardware/linux/provider.rs:124-131`, `src/hardware/linux/cpu.rs` |
| `/proc/cpuinfo` | Read | CPU model name and frequency fallback. | `src/hardware/linux/provider.rs:62-63`, `133-140` |
| `/proc/meminfo` | Read | RAM total/available → used MB. | `src/hardware/linux/memory.rs:26-72` |
| `/proc/self/smaps_rollup` | Read | RSS/PSS/Private_Dirty benchmark metrics. | `src/core/benchmark.rs:42-79` |
| `/proc/self/status` | Read | `VmRSS` fallback when `smaps_rollup` is unavailable. | `src/core/benchmark.rs:87-110` |
| `/proc/self/stat` | Read | CPU tick sampling for benchmark percentage. | `src/core/benchmark.rs:145-180` |
| `/proc/sys/kernel/osrelease` | Read | Kernel version for diagnostics. | `src/core/diagnostics.rs:67-70`, `184-188` |
| `/etc/os-release` | Read | Distribution name (`PRETTY_NAME`/`NAME`) for diagnostics. | `src/core/diagnostics.rs:66-70`, `162-182` |
| `/proc/<pid>/status`, `/proc/<pid>/stat` | Read | External budget harness samples `VmRSS` and utime+stime. | `scripts/measure-budget.sh:54-68` |

No direct block-device, `/dev`, or kernel-module I/O is used. No sensor data is
fabricated: every reader returns `Option`/empty and the UI renders `Unavailable`.

## Data Storage

**Databases:**
- None. No SQL/NoSQL client or embedded database crate is present.

**File Storage (local filesystem only):**

| Path | Format | Writer | Purpose |
|---|---|---|---|
| `~/.config/zenomen/config.toml` | TOML | GUI | General and telemetry settings; atomic save; corruption backup. |
| `~/.config/zenomen/diagnostics/zenomen-diagnostics-<timestamp>.json` | JSON | GUI | Diagnostics export (`src/app.rs:233-245`). |
| `~/.config/zenomen/rgb/` | JSON profiles | GUI | ZenOmen RGB profile storage. |
| `~/.config/openomen/rgb/` | JSON | Read-only | Legacy OpenOMEN migration source; never modified or deleted. |
| `~/.config/autostart/zenomen.desktop` | Desktop Entry | GUI | XDG autostart toggle. |
| `~/.local/state/zenomen/logs/` | Text logs | GUI + daemon | Daily rolling logs, 7 files maximum. |

- Config saves are atomic: temporary file in the same directory, `sync_all`, rename,
  then parent-directory sync (`src/config/mod.rs:85-116`).
- Diagnostics exports use a temporary file plus rename
  (`src/core/diagnostics.rs:144-160`).
- No cache directory, no key-value store, and no remote storage are used.

**Caching:**
- In-memory only: cached system-bus connection (`src/service/client.rs:38-75`) and the
  most recent NVIDIA sample with a 3-second freshness window
  (`src/hardware/linux/gpu.rs:191-245`).
- Telemetry history is in-memory (120 samples by default; configurable 30/60/120/300).

## Authentication & Identity

**Auth provider:** Polkit (system authorization), not an application account system.

- No usernames, passwords, tokens, OAuth, OIDC, or API keys are stored or transmitted.
- Identity is the caller's D-Bus unique name, resolved by the system bus.
- The D-Bus policy permits any local user to call the daemon, but every mutating method
  performs its own Polkit check.
- The systemd unit runs as root with: `ProtectSystem=strict`, `ProtectHome=true`,
  `NoNewPrivileges=true`, empty `CapabilityBoundingSet`/`AmbientCapabilities`,
  `RestrictAddressFamilies=AF_UNIX`, and `SystemCallFilter=@system-service`
  (`data/systemd/zenomen-hw.service:14-32`).
- `ZENOMEN_HW_SESSION` is a development/test override that binds the daemon to the
  session bus instead of the system bus (`src/bin/zenomen-hw.rs:23-27`). It grants no
  additional privilege; do not use it in production.

## Monitoring & Observability

**Error tracking:**
- None. No Sentry, telemetry endpoint, OpenTelemetry, or remote error reporting.

**Logs:**
- `tracing` 0.1.44 with `tracing-subscriber` 0.3.23 and `EnvFilter`.
- Stderr layer for terminal/journald; optional file layer with daily rotation,
  `zenomen` prefix, `.log` suffix, max 7 files.
- Default filter `info`; override with `RUST_LOG`.
- The daemon logs startup and service registration; systemd/journald captures stderr.
- Benchmark instrumentation reads `/proc/self/smaps_rollup`, `/proc/self/status`, and
  `/proc/self/stat`; `ZENOMEN_BENCHMARK=1` enables stdout benchmark lines
  (`src/core/benchmark.rs`).

## CI/CD & Deployment

**Hosting:**
- GitHub Releases (`https://github.com/Omprakash-p06/zenomen/releases`).
- No container registry, cloud runtime, or package repository is configured.

**CI pipeline — `.github/workflows/ci.yml`:**
- Trigger: push to `main`, pull request to `main`.
- Runner: `ubuntu-latest`; permissions: `contents: read`.
- Steps: checkout (`actions/checkout@v4`), install stable Rust + clippy + rustfmt,
  install native Linux build dependencies, `cargo fmt --all -- --check`,
  `cargo clippy --all-targets --all-features -- -D warnings`,
  `xvfb-run --auto-servernum cargo test --all`, `cargo audit`.
- `cargo-audit` is installed through `taiki-e/install-action@cargo-audit`; its version
  is not pinned in the repository.

**Release pipeline — `.github/workflows/release.yml`:**
- Trigger: tags matching `v*` and manual `workflow_dispatch`.
- Permissions: `contents: write`.
- Verifies the tag equals the `Cargo.toml` package version.
- Re-runs format, clippy, tests, and `cargo audit` with `--locked`.
- Builds `deb` and `appimage` via `cargo packager --release --formats deb,appimage`.
- Verifies artifact filenames contain the package version, writes
  `sha256sum *.deb *.AppImage > SHA256SUMS`, and publishes a GitHub Release with
  `softprops/action-gh-release` (pinned commit SHA).
- Release-action and checkout versions are pinned to commit SHAs; CI checkout is not.

**Packaging targets:**

| Target | Output | Configuration |
|---|---|---|
| Debian `.deb` | `dist/zenomen_<version>_amd64.deb` | `[package.metadata.packager.deb]` in `Cargo.toml:88-99` |
| AppImage | `dist/ZenOmen_<version>_amd64.AppImage` | `[package.metadata.packager]` in `Cargo.toml:70-86` |

**Installed system files (both packaging paths / installer):**

| Source | Target |
|---|---|
| `zenomen` | `/usr/bin/zenomen` |
| `zenomen-hw` | `/usr/bin/zenomen-hw` |
| `data/applications/zenomen.desktop` | `/usr/share/applications/zenomen.desktop` |
| `data/systemd/zenomen-hw.service` | `/usr/lib/systemd/system/zenomen-hw.service` |
| `data/dbus/com.zenomen.Hardware1.conf` | `/usr/share/dbus-1/system.d/com.zenomen.Hardware1.conf` |
| `data/dbus/com.zenomen.Hardware1.service` | `/usr/share/dbus-1/system-services/com.zenomen.Hardware1.service` |
| `data/polkit/com.zenomen.hardware.policy` | `/usr/share/polkit-1/actions/com.zenomen.hardware.policy` |

- The AppImage path embeds the desktop entry, systemd unit, D-Bus files, and Polkit
  policy via `include_str!`, copies an adjacent `zenomen-hw` binary to `/usr/bin`, and
  installs the files with `--install-system-integration` / removes them with
  `--uninstall-system-integration` (`src/desktop/system_integration.rs`). Both commands
  require effective UID 0 and quit with an explicit error otherwise.
- The installer then runs `/usr/bin/systemctl` (falling back to `/bin/systemctl` or
  `systemctl` in `PATH`) with `--no-ask-password`: `daemon-reload`, `enable`,
  `restart`; uninstall uses `stop`, `disable`, `daemon-reload`.
- **Note:** `README.md:70` says the D-Bus policy installs to
  `/etc/dbus-1/system.d/...`, but the authoritative packaging and installer code uses
  `/usr/share/dbus-1/system.d/...` (`Cargo.toml:94`,
  `src/desktop/system_integration.rs:14`).
- No Flatpak, Snap, RPM, Arch package, or Docker image configuration exists in the
  repository.

## Environment Configuration

**Required environment variables:** none. Both binaries have safe defaults; all
variables below are optional.

| Variable | Consumer | Effect |
|---|---|---|
| `RUST_LOG` | GUI + daemon | Log filter; default `info`. |
| `ZENOMEN_MOCK_SCENARIO` | GUI | Selects mock provider scenario; unknown values fall back to real Linux provider. |
| `ZENOMEN_BENCHMARK=1` | GUI | Emits benchmark measurements to stdout. |
| `ZENOMEN_HW_SESSION` | Daemon | Binds to session bus instead of system bus (dev/test only). |
| `LANG`, `LC_CTYPE`, `LC_ALL` | GUI | UTF-8 locale fallback before winit starts. |

**Secrets location:**
- None in the repository. No `.env`, credential file, token, or key material is
  present or read by the application.
- Privilege is delegated to Polkit and the system D-Bus policy; no application secrets
  are involved.

**Runtime prerequisites (integration-relevant):**
- System D-Bus for hardware control; a session D-Bus is required for single-instance
  enforcement and tray registration, but the app degrades gracefully without one.
- systemd for packaged `zenomen-hw.service` management.
- Polkit authority and an authentication agent for interactive authorization.
- `hp-wmi` kernel module for HP fan/power/RGB interfaces.
- `nvidia-smi` only when NVIDIA telemetry is desired; its absence is non-fatal.
- `xdg-open` only when the user clicks "Check for updates"; failure is logged.

## Webhooks & Callbacks

**Incoming:**
- No HTTP endpoints or webhooks.
- D-Bus is the inbound interface: system-bus method calls to `com.zenomen.Hardware1`,
  session-bus calls to `com.zenomen.App`, StatusNotifierItem/DBusMenu events, and
  D-Bus service activation of `zenomen-hw`.
- CLI entry points actually parsed by `src/main.rs`: `--install-system-integration`,
  `--uninstall-system-integration`, and `--minimized`/`-m` (`src/main.rs:40-74`).
  `README.md:128-140` also advertises `--export-diagnostics`, `--help`, and
  `--version`, but no matching argument handling was found in the source; diagnostics
  export is currently a UI callback (`src/app.rs:220-262`).

**Outgoing:**
- System-bus method calls to `com.zenomen.Hardware1`.
- System-bus `CheckAuthorization` calls to `org.freedesktop.PolicyKit1`.
- Session-bus `RegisterStatusNotifierItem` to `org.kde.StatusNotifierWatcher`.
- Subprocesses: `nvidia-smi`, `systemctl`, `xdg-open`.
- No outgoing HTTP, SMTP, WebSocket, or webhook traffic.

## Failure / Degradation Behavior

| Missing integration | Observed behavior | Source |
|---|---|---|
| `zenomen-hw` not running / no `.service` activation | GUI reports service unavailable, remains read-only, shows actionable message. | `src/service/client.rs:232-249` |
| Polkit denies or no agent | Mutating call fails before sysfs write; hardware state unchanged. | `src/service/contract.rs:55-80` |
| StatusNotifierWatcher absent | Tray registration skipped with info log; app continues. | `src/desktop/tray.rs:201-217` |
| Session D-Bus absent | Single-instance/tray disabled; app runs standalone. | `src/desktop/single_instance.rs:66-74` |
| `nvidia-smi` absent or fails | dGPU telemetry returns `None`; never blocks the sampling tick and never fabricates values. | `src/hardware/linux/provider.rs:66-73`, `src/hardware/linux/gpu.rs:248-292` |
| hwmon/ACPI/RGB sysfs absent | Capability flags false; UI shows `Unavailable`; no panic. | `src/hardware/registry.rs`, `src/hardware/rgb.rs` |
| `xdg-open` missing | Logged warning only; no crash. | `src/app.rs:267-271` |

---

*Integration audit: 2026-11-07*
