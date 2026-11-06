---
last_mapped_commit: 30d62de2cdf6f507822944e5602bdd907539ce38
last_mapped_at: 2026-11-06
---
# External Integrations

**Analysis Date:** 2026-11-06

## Network Posture

**ZenOmen has no network dependency and ships no HTTP client.**

- `Cargo.lock` contains zero packages named `reqwest`, `ureq`, `hyper`, `rustls`, `native-tls`, `openssl`, `curl`, `isahc`, or `sentry` (verified by name search in the lockfile).
- No source file opens a network socket, starts a server, or performs an HTTP request; all IPC goes through local D-Bus (via `zbus`) and the desktop display connection.
- There is **no self-updater, no version-check API, no download, no signature fetch, no telemetry upload, and no crash reporting**.
- "Check for updates" is a browser hand-off only:
  - `RELEASES_URL = "https://github.com/Omprakash-p06/zenomen/releases/latest"` (`src/desktop/releases.rs:13`)
  - `open_releases_page()` spawns `xdg-open <url>` with stdin/stdout/stderr set to `Stdio::null()` and reaps the child on a detached thread (`src/desktop/releases.rs:26`–`44`)
- Keep this property: do not add an HTTP client or background update checker without an explicit project decision.

## APIs & External Services

### D-Bus System Bus — `com.zenomen.Hardware1` (privileged hardware control)

The primary integration contract between the unprivileged GUI and root-only hardware access.

| Property | Value | Source |
|---|---|---|
| Bus | system bus (session bus only when `ZENOMEN_HW_SESSION` is set, for development) | `src/bin/zenomen-hw.rs:26`–`30` |
| Bus name | `com.zenomen.Hardware1` | `src/bin/zenomen-hw.rs:34` |
| Object path | `/com/zenomen/Hardware1` | `src/bin/zenomen-hw.rs:35` |
| Interface | `com.zenomen.Hardware1` | `src/service/contract.rs:82` |
| Owner | `root`, enforced by D-Bus policy | `data/dbus/com.zenomen.Hardware1.conf:6`–`9` |
| Server | `HardwareService` | `src/service/contract.rs:14`–`213` |
| Client | `HardwareServiceClient` (blocking zbus) | `src/service/client.rs:34`–`344` |
| Backend | `HardwareBackend` (sysfs writes + read-back) | `src/service/backend.rs:52`–`502` |

**Methods** (`src/service/contract.rs`; zbus exposes them PascalCase on the wire):

| Method | Signature | Mutating / polkit-gated | Purpose |
|---|---|---|---|
| `GetCapabilities` | `() -> String` (JSON) | No | `has_fan_control`, `has_power_profiles`, `has_rgb_lighting`, `rgb_variant`, `service_version`. Used as the liveness probe by `status()`. |
| `GetFanStatus` | `() -> (String, Vec<u32>)` | No | Active fan mode + RPMs. |
| `SetFanMode` | `(String) -> String` | **Yes** | Accepts only `auto` / `max`; verified read-back. |
| `GetPowerProfiles` | `() -> Vec<String>` | No | ACPI `platform_profile_choices`. |
| `GetPowerProfile` | `() -> String` | No | Active ACPI profile. |
| `SetPowerProfile` | `(String) -> String` | **Yes** | Validated against advertised choices; verified read-back. |
| `ApplyPreset` | `(String) -> String` (JSON `PresetResult`) | **Yes** | Combined quiet/balanced/gaming preset; reports partial success. |
| `ApplyRgbProfile` | `(String JSON) -> String` | **Yes** | Deserializes `RgbProfile` and applies `zones`; rejects invalid JSON with `InvalidArgs`. |
| `SetRgbEnabled` | `(bool) -> String` | **Yes** | `true` = white on all four zones, `false` = black. |

**Authorization:** every mutating method calls `HardwareService::authorize()` (`src/service/contract.rs:55`–`79`) before touching hardware. It extracts the caller's unique bus name from the message header, then calls `org.freedesktop.PolicyKit1.Authority.CheckAuthorization` at `/org/freedesktop/PolicyKit1/Authority` with:
- subject `("system-bus-name", {"name": <unique sender>})`
- action `com.zenomen.hardware.control`
- interactive flag `1` (the desktop polkit agent may prompt)
- empty cancellation id

Response is deserialized as `(bba{ss})` — `(is_authorized, is_challenge, details)` (`src/service/polkit.rs:44`–`70`; regression-tested at `src/service/polkit.rs:76`–`100`).

`with_polkit_required(false)` is `#[cfg(test)]` only (`src/service/contract.rs:43`–`47`); never enable it in a production path.

**Error mapping** (`src/service/types.rs:133`–`145`):

| Internal | D-Bus error |
|---|---|
| `PolkitDenied` | `AccessDenied` |
| `InvalidParameter` | `InvalidArgs` |
| `UnsupportedMode` | `NotSupported` |
| `InterfaceMissing` | `FileNotFound` |
| `VerificationFailed` / `IoError` | `Failed` |

**Service-unavailable handling:** the client distinguishes `Connected` / `Disconnected` / `Unavailable` (`src/service/client.rs:10`–`29`), caches the system connection, resets it on failure, and rewrites `ServiceUnknown` into actionable "start `zenomen-hw`" guidance (`src/service/client.rs:232`–`252`).

**D-Bus policy** (`data/dbus/com.zenomen.Hardware1.conf`):
- Only `root` may own the name.
- Any local user may send/receive on the name; mutation is authorized inside the service by polkit.

**D-Bus activation** (`data/dbus/com.zenomen.Hardware1.service`): `Name=com.zenomen.Hardware1`, `Exec=/usr/bin/zenomen-hw`, `User=root`.

**Installed locations** (authoritative: `Cargo.toml:67`–`75` and `src/desktop/system_integration.rs:12`–`18`):
- `/usr/lib/systemd/system/zenomen-hw.service`
- `/usr/share/dbus-1/system-services/com.zenomen.Hardware1.service`
- `/usr/share/dbus-1/system.d/com.zenomen.Hardware1.conf`
- `/usr/share/polkit-1/actions/com.zenomen.hardware.policy`

`README.md:70` says the D-Bus config lands in `/etc/dbus-1/system.d/`; the Cargo packager mapping and installer target `/usr/share/dbus-1/system.d/`. Treat the code as correct and fix the README when touching it.

### Polkit

- Action ID: `com.zenomen.hardware.control` — constant `POLKIT_ACTION_CONTROL` (`src/service/polkit.rs:10`).
- Policy file: `data/polkit/com.zenomen.hardware.policy`.
- Defaults: `allow_any = auth_admin`, `allow_inactive = auth_admin`, `allow_active = auth_admin_keep` — an admin password prompt for fan/power/RGB changes, cached for the active session.
- No custom polkit agent is implemented; the desktop environment's agent supplies the dialog.
- The GUI performs a **non-interactive** check (`flags = 0`) for Diagnostics via `HardwareServiceClient::check_polkit_status()` (`src/service/client.rs:252`–`299`), reporting "Authorized", "Authentication challenge required", or "Authorization not granted".
- Mutating RPCs run on `zenomen-ctrl-*` / `zenomen-rpc-*` threads with a **30-second** timeout so a polkit prompt cannot stall the 1 Hz telemetry loop (`src/core/worker.rs:350`–`379`).

### systemd

- Unit: `data/systemd/zenomen-hw.service` → `/usr/lib/systemd/system/zenomen-hw.service`.
- `Type=dbus`, `BusName=com.zenomen.Hardware1`, `ExecStart=/usr/bin/zenomen-hw`, `Restart=on-failure`, `RestartSec=3s`.
- Hardening: `ProtectSystem=strict`, `ProtectHome=true`, `ReadOnlyPaths=/`, `PrivateTmp=true`, `ProtectControlGroups=true`, `RestrictRealtime=true`, `RestrictNamespaces=true`, `NoNewPrivileges=true`, empty `CapabilityBoundingSet=`/`AmbientCapabilities=`, `RestrictAddressFamilies=AF_UNIX`, `SystemCallFilter=@system-service`, `LockPersonality=true`, `MemoryDenyWriteExecute=true`.
- Writable carve-outs are `-`-prefixed so absent hardware paths do not fail startup: `/sys/devices/platform/hp-wmi`, `/sys/firmware/acpi`, `/sys/class/hwmon`, `/sys/class/leds`.
- `ProtectKernelTunables` is **intentionally omitted** because it would make `/sys` read-only (`data/systemd/zenomen-hw.service:21`–`22`). Do not add it.
- Installer drives `systemctl --no-ask-password daemon-reload/enable/restart` (and `stop`/`disable` on uninstall), preferring `/usr/bin/systemctl`, then `/bin/systemctl`, then `PATH` (`src/desktop/system_integration.rs:195`–`217`).

### D-Bus Session Bus — GUI desktop integration

| Integration | Bus name / path / interface | File |
|---|---|---|
| Single instance | owns `com.zenomen.App`, object `/com/zenomen/App`, interface `com.zenomen.App` | `src/desktop/single_instance.rs:10`–`12` |
| Single-instance RPCs | `PresentWindow`, `Activate`, `Quit` | `src/desktop/single_instance.rs:29`–`54` |
| Tray (SNI) | `org.kde.StatusNotifierItem` at `/StatusNotifierItem` | `src/desktop/tray.rs:14`, `:22`–`:107` |
| Tray menu | `com.canonical.dbusmenu` at `/MenuBar` | `src/desktop/tray.rs:15`, `:123`–`:184` |
| Tray registration | `RegisterStatusNotifierItem` on `org.kde.StatusNotifierWatcher` at `/StatusNotifierWatcher` | `src/desktop/tray.rs:186`–`:220` |

- Name acquisition uses `RequestName("com.zenomen.App", 4)` (`DBUS_NAME_FLAG_DO_NOT_QUEUE`). Replies `1`/`4` = primary; `2`/`3` = already running, so the secondary calls `PresentWindow` and exits (`src/desktop/single_instance.rs:77`–`113`).
- Missing session bus is non-fatal: the GUI logs a warning and runs standalone (`src/main.rs:68`–`72`, `src/desktop/single_instance.rs:73`–`79`).
- SNI properties exposed: `Category`, `Id`, `Title`, `Status`, `WindowId`, `IconName`, `OverlayIconName`, `AttentionIconName`, `AttentionMovieName`, `Menu`, `ItemIsMenu`; methods `Activate`, `SecondaryActivate`, `ContextMenu`, `Scroll`.
- DBusMenu exposes `Version=3`, `Status="normal"`, `GetLayout`, `Event`, `AboutToShow`, with two items: **Open ZenOmen** (id 1) and **Quit ZenOmen** (id 2).
- No `StatusNotifierWatcher` (e.g. stock GNOME without AppIndicator) is logged at info level and startup continues; D-Bus single-instance remains the fallback activation path.

### XDG desktop integration

- Autostart entry managed at `~/.config/autostart/zenomen.desktop`, content in `src/desktop/autostart.rs:17`–`30`, `Exec=zenomen --minimized`.
- Enabled/disabled by creating or removing the file; `Hidden=true` and `X-GNOME-Autostart-enabled=false` are honoured when reading state (`src/desktop/autostart.rs:41`–`73`).
- Packaged desktop entry: `data/applications/zenomen.desktop`, `Exec=zenomen %U`, `Categories=Utility;System;`, `StartupWMClass=zenomen`.
- AppImage / standalone installs embed the desktop entry, systemd unit, D-Bus conf/service, and polkit policy with `include_str!` and write them atomically as root (`src/desktop/system_integration.rs:20`–`26`, `:117`–`138`, `:178`–`193`).

## Kernel Integration Surface (sysfs / procfs)

All hardware access is direct file I/O. There are no ioctls, no libsensors, no udev rules, and no ZenOmen kernel module.

**Telemetry (read-only, GUI process):**

| Interface | Data | Code |
|---|---|---|
| `/proc/stat` | Aggregate CPU utilization from jiffies deltas | `src/hardware/linux/cpu.rs:25`–`153`, `src/hardware/linux/provider.rs:122`–`129` |
| `/proc/cpuinfo` | CPU model string and frequency fallback | `src/hardware/linux/cpu.rs:155`–`186`, `src/hardware/linux/provider.rs:61`, `:131`–`137` |
| `/sys/devices/system/cpu/cpu0/cpufreq/scaling_cur_freq` | Current frequency (kHz → MHz) | `src/hardware/linux/provider.rs:38`, `src/hardware/linux/cpu.rs:169`–`174` |
| `/proc/meminfo` | RAM total/available; used = total − available | `src/hardware/linux/memory.rs:26`–`73` |
| `/sys/class/hwmon/hwmon*/name` | hwmon driver identification | `src/hardware/linux/hwmon.rs:32`–`62` |
| `/sys/class/hwmon/hwmon*/temp*_input` + `temp*_label` | Temperatures (millidegrees) and labels | `src/hardware/linux/hwmon.rs:65`–`86` |
| `/sys/class/hwmon/hwmon*/fan*_input` | Fan tachometer RPM | `src/hardware/linux/hwmon.rs:74`–`77`, `src/hardware/registry.rs:74`–`80` |
| `pwm1_enable` read path | Active fan mode (`0` max / `1` manual / `2` auto) | `src/hardware/registry.rs:82`–`98` |
| `/sys/class/drm/card[0-9]+/device/vendor` | PCI vendor ID (`0x1002` AMD, `0x8086` Intel, `0x10de` NVIDIA) | `src/hardware/linux/gpu.rs:86`–`97`, `:23`–`29` |
| `/sys/class/drm/card*/device/gpu_busy_percent` | AMD utilization | `src/hardware/linux/gpu.rs:150`–`155` |
| `/sys/class/drm/card*/device/mem_info_vram_{used,total}` | AMD VRAM | `src/hardware/linux/gpu.rs:167`–`177` |
| `/sys/class/drm/card*/device/hwmon/hwmon*/temp1_input` | GPU temperature | `src/hardware/linux/gpu.rs:117`–`127`, `:157`–`164` |
| `/sys/firmware/acpi/platform_profile` | Active power profile | `src/hardware/registry.rs:100`–`105` |
| `/sys/firmware/acpi/platform_profile_choices` | Advertised profiles | `src/hardware/registry.rs:107`–`114` |
| `/etc/os-release`, `/proc/sys/kernel/osrelease`, `/sys/class/dmi/id/{product_name,sys_vendor}`, `/sys/class/drm` | Diagnostics metadata | `src/core/diagnostics.rs:61`–`89` |
| `/proc/self/smaps_rollup` (fallback `/proc/self/status`), `/proc/self/stat` | RSS/PSS/private-dirty and CPU instrumentation | `src/core/benchmark.rs:42`–`145` |
| `/proc/<pid>/status`, `/proc/<pid>/stat` | External budget harness only (not product code) | `scripts/measure-budget.sh:50`–`65` |

**Hardware control (privileged `zenomen-hw` only):**

| Interface | Operation | Code |
|---|---|---|
| `/sys/devices/platform/hp-wmi/hwmon/hwmon*/pwm1_enable` (primary) or `/sys/class/hwmon/hwmon*/pwm1_enable` where `name` is `hp`/`hp-wmi` (fallback) | Fan mode: `0` max, `2` auto; write then read back, 3 attempts, exponential backoff | `src/service/backend.rs:12`–`48`, `:77`–`115`, `:162`–`176` |
| `/sys/firmware/acpi/platform_profile` | Power profile, validated against `platform_profile_choices`; read-back verified | `src/service/backend.rs:196`–`248` |
| `/sys/devices/platform/hp-wmi/rgb_zones/zone0{0..3}` | RGB variant `FourZone`; hex per zone, write + read-back per zone | `src/service/backend.rs:352`–`357`, `:442`–`451` |
| `/sys/devices/platform/hp-wmi/keyboardleds` | RGB variant `Legacy`; one 24-character hex payload, write + read-back | `src/service/backend.rs:361`–`364`, `:452`–`463` |
| `/sys/class/leds/hp_omen::kbd_backlight/zone_colors` | RGB variant `KernelPatch`; 12 raw bytes written with `fs::write`, then one read-back comparison | `src/service/backend.rs:367`–`370`, `:464`–`500` |
| Any detected RGB variant | Read current colors (`get_rgb_colors`) | `src/service/backend.rs:381`–`437` |

**Semantics and safeguards:**
- Fan mode: `0` = Max, `1` = Manual, `2` = Auto (`src/service/types.rs:31`–`44`); v1 writes only `auto`/`max` and rejects `manual` with `UnsupportedMode`.
- Every write is verified by reading the value back; mismatch returns `VerificationFailed` rather than claiming success.
- Missing interfaces return `InterfaceMissing` → D-Bus `FileNotFound`; the UI renders "Unavailable", never a fabricated zero.
- `HardwareCapabilities::battery_metrics` is declared but `detect_capabilities()` hardcodes it to `false`; there is no `/sys/class/power_supply` read path (`src/hardware/registry.rs:117`–`135`). Do not claim battery support without implementing it.
- `HardwareBackend` takes a configurable `sysfs_root` (`HardwareBackend::new`), and `SensorRegistry::scan_roots` / `LinuxHardwareProvider::with_all_roots` accept alternate roots for tests. Production uses `/`.

## External Processes

| Process | Required? | Invocation | Purpose |
|---|---|---|---|
| `nvidia-smi` | Optional | `nvidia-smi --query-gpu=name,utilization.gpu,temperature.gpu,memory.used,memory.total --format=csv,noheader,nounits`; 200 ms receive timeout on a helper thread; on timeout the thread is detached rather than joined (`src/hardware/linux/gpu.rs:187`–`219`, `src/hardware/linux/provider.rs:164`–`166`) | NVIDIA telemetry. Absent/suspended GPU degrades to DRM sampling or `None`. |
| `xdg-open` | Optional | `xdg-open https://github.com/Omprakash-p06/zenomen/releases/latest`, stdio null, child reaped asynchronously (`src/desktop/releases.rs:26`–`44`) | Open the Releases page. |
| `systemctl` | Install/uninstall only | `/usr/bin/systemctl` or `/bin/systemctl` or `systemctl`; `--no-ask-password` plus `daemon-reload`, `enable`, `restart`, `stop`, `disable` (`src/desktop/system_integration.rs:195`–`217`) | Register/unregister `zenomen-hw`. |

All commands are spawned with explicit argument vectors. No `sh -c`, no shell interpolation, no user-supplied command or path. The GUI never invokes `sudo`; privilege escalation is polkit + the root daemon.

## Data Storage

**Databases:** None. No SQL, no embedded KV store, no daemon-side persistence.

**File storage (local only):**
- `~/.config/zenomen/config.toml` — user settings; atomic replace with fsync of file and parent directory (`src/config/mod.rs:85`–`116`).
- `~/.config/zenomen/config.toml.bak.<unix-seconds>` — backup of an unparseable config (`src/config/mod.rs:53`–`78`).
- `~/.config/zenomen/rgb/*.json` — RGB profiles, serde JSON with strict `#RRGGBB` validation (`src/rgb/schema.rs`).
- `~/.config/zenomen/diagnostics/zenomen-diagnostics-<timestamp>.json` — user-triggered export, written atomically (`src/app.rs:233`–`242`, `src/core/diagnostics.rs:145`–`160`). Never uploaded.
- `~/.config/autostart/zenomen.desktop` — XDG autostart entry (`src/desktop/autostart.rs`).
- `~/.config/openomen/rgb/*.json` — **read-only legacy import source**, never modified or deleted (`src/rgb/migration.rs:42`–`48`, `:185`–`270`).
- System files owned by packaging/installer: `/usr/bin/zenomen`, `/usr/bin/zenomen-hw`, `/usr/share/applications/zenomen.desktop`, `/usr/lib/systemd/system/zenomen-hw.service`, `/usr/share/dbus-1/system-services/com.zenomen.Hardware1.service`, `/usr/share/dbus-1/system.d/com.zenomen.Hardware1.conf`, `/usr/share/polkit-1/actions/com.zenomen.hardware.policy`, `/usr/share/icons/hicolor/*/apps/zenomen.*`.

**Caching:** None on disk. Hardware state is read fresh each tick. In-memory telemetry is a bounded `VecDeque` of 120 samples (`src/core/telemetry.rs:22`–`25`).

**Concurrency:** Config writes are atomic (temp + `sync_all` + rename). RGB writes go through the daemon's verified-write helper. No cross-process file locks; the session-bus name `com.zenomen.App` is the single-instance guard.

## Authentication & Identity

- **No accounts, login, tokens, OAuth, or cloud identity.**
- **Authorization is polkit only**, scoped to `com.zenomen.hardware.control`, with `zenomen-hw` running as root under systemd.
- D-Bus name ownership is restricted to root by policy; any local user may call read-only methods, while mutating methods are gated inside the service.
- The system-integration installer checks effective UID == 0 via `geteuid()` and refuses to write anything as non-root (`src/desktop/system_integration.rs:41`–`53`, `:117`–`120`).
- No secrets exist in the repository: no `.env`, no credential files, no signing keys. Release integrity is SHA-256 checksums only (`SHA256SUMS`), not GPG signing.

## Monitoring & Observability

**Error tracking:** None. No Sentry, crash reporter, or remote logging.

**Logs:**
- `tracing` to **stderr** in both binaries. GUI: `tracing_subscriber::fmt::init()` (`src/main.rs:75`). Daemon: `EnvFilter` defaulting to `info` (`src/bin/zenomen-hw.rs:12`–`16`).
- `RUST_LOG` controls verbosity; structured fields are used (`version`, `scenario`, `path`, `error`, `rss_mb`, `scanned`, `imported`).
- No log files, rotation, or log levels beyond `tracing` defaults.
- Optional benchmark lines on stdout when `ZENOMEN_BENCHMARK=1` (`src/core/benchmark.rs:245`–`256`).

**User-facing diagnostics:** JSON report with app version, OS name, kernel release, DMI product/vendor, capabilities, CPU/fan/GPU sensor bindings, hardware-service status, polkit status, latest telemetry, last sampling error, and UTC timestamp (`src/core/diagnostics.rs:29`–`56`). Exported locally, never uploaded.

**Budget measurement:** `scripts/measure-budget.sh` samples `/proc/<pid>/status` and `/proc/<pid>/stat`; results are recorded in `docs/RESOURCE-BUDGET.md`. Targets: GUI ≤40 MB peak RSS and ≤0.1% idle CPU; daemon budget reported separately (≤15 MB, measured ~3.4 MB).

## CI/CD & Deployment

**Hosting:** GitHub (`https://github.com/Omprakash-p06/zenomen`); binary distribution via GitHub Releases.

**CI (`.github/workflows/ci.yml`):** push/PR to `main`; stable Rust with clippy + rustfmt; native deps (`libxkbcommon-dev`, `libfontconfig1-dev`, `libgl1-mesa-dev`, `xvfb`); `cargo fmt --all -- --check`; `cargo clippy --all-targets --all-features -- -D warnings`; `xvfb-run --auto-servernum cargo test --all`; `cargo audit`. `permissions: contents: read`, concurrency cancellation enabled.

**Release (`.github/workflows/release.yml`):** triggered by `v*` tags or manual dispatch.
- Verifies the tag equals the `zenomen` version from `cargo metadata`.
- Runs the same gates with `--locked`, then `cargo packager --release --formats deb,appimage`.
- Installs `dpkg-dev`, `file`, `libfuse2` in addition to build deps.
- Generates `dist/SHA256SUMS` and publishes `.deb`, `.AppImage`, and checksums via `softprops/action-gh-release`.
- Actions are pinned by commit SHA; `permissions: contents: write` is limited to this workflow.
- The latest commit (`30d62de`) restricts release builds to Linux packages only; no Windows artifact is produced.

**Distribution:** Debian/Ubuntu `.deb` and portable `.AppImage`. AppImage users run `sudo ./ZenOmen_*.AppImage --install-system-integration` once to install the daemon and system files from embedded assets.

## Environment Configuration

**Required env vars:** none. The app starts with an empty environment given a session bus and the expected kernel interfaces.

**Optional env vars:**

| Variable | Values | Effect |
|---|---|---|
| `RUST_LOG` | tracing filter | Log verbosity |
| `ZENOMEN_MOCK_SCENARIO` | `nominal`, `zero` / `zerocapabilities`, `gap` / `sensordata` | Use `MockHardware` instead of the Linux provider (`src/hardware/mock.rs:60`–`69`) |
| `ZENOMEN_BENCHMARK` | `1` | Emit benchmark lines |
| `ZENOMEN_HW_SESSION` | any value | Bind `zenomen-hw` to the session bus for local testing |
| `LC_CTYPE` / `LC_ALL` / `LANG` | locale strings | GUI injects `LC_CTYPE=C.UTF-8` when the locale lacks a UTF-8 suffix (`src/main.rs:21`–`31`) |

**Secrets location:** none. No `.env` file and no credential material is present or read.

## Webhooks & Callbacks

**Incoming:** None over HTTP. The only inbound channels are D-Bus method calls on:
- system bus `com.zenomen.Hardware1` (hardware control)
- session bus `com.zenomen.App` (`PresentWindow`, `Activate`, `Quit`)
- session bus SNI/DBusMenu objects (tray clicks)
- session bus `org.kde.StatusNotifierWatcher.RegisterStatusNotifierItem`

**Outgoing:** None. No webhook POSTs, telemetry callbacks, update pings, or email.

## Integration Constraints for Future Work

- The GUI process must remain unprivileged: no direct sysfs writes, no `sudo`, no shell commands. All mutations go through `com.zenomen.Hardware1`.
- The daemon accepts only predefined operations and validated values (mode allow-list, membership in `platform_profile_choices`, 4-zone RGB schema). Never accept arbitrary paths, command strings, or shell fragments.
- New D-Bus methods must be polkit-gated if they mutate hardware, mirrored in `src/service/client.rs`, and surfaced in diagnostics when user-visible.
- Keep the no-network guarantee: no HTTP client, no downloader. Update discovery stays a browser hand-off in `src/desktop/releases.rs`.
- Every new sysfs read must degrade to `None`/`Unavailable`; every new write must use `write_and_verify` semantics (`src/service/backend.rs:12`–`48`).
- New daemon sysfs paths require a matching `ReadWritePaths` carve-out in `data/systemd/zenomen-hw.service`, or `ProtectSystem=strict` will deny the write.
- Do not add `ProtectKernelTunables` to the unit; it would make `/sys` read-only and break fan/power/RGB control.
- Keep `README.md:70`'s D-Bus path claim aligned with the real installer target (`/usr/share/dbus-1/system.d/`).

---

*Integration audit: 2026-11-06*
