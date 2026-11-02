---
last_mapped_commit: e50af8f9ea61300a1c10d36dc6954e319da7b6e2
last_mapped_at: 2026-11-02
---
# External Integrations

**Analysis Date:** 2026-11-02

## Network Posture

**ZenOmen has no network dependency.** It works fully offline by design.

- No HTTP/TLS client is linked: `Cargo.lock` (557 packages) contains no `reqwest`, `ureq`, `hyper`, `curl`, `isahc`, `rustls`, `native-tls`, or `openssl`.
- The GUI makes zero network requests; all telemetry comes from local kernel interfaces.
- There is **no HTTP self-updater**, no release-check API call, no download, no signature fetch, no telemetry upload, and no crash reporting.
- The only outbound interaction is delegating a URL to the user's browser:
  - `RELEASES_URL = "https://github.com/Omprakash-p06/zenomen/releases/latest"` in `src/desktop/releases.rs:13`.
  - `open_releases_page()` spawns `xdg-open <url>` with stdio redirected to null and reaps the child on a background thread (`src/desktop/releases.rs:26`–`38`). It is wired to the Settings screen's "Check for updates" callback (`src/app.rs:265`–`270`).
  - "Check for updates" means "open the releases page", never "download and install".
- Do not add an HTTP client or background update checker without an explicit project decision; it would violate the offline/no-network constraint in `.planning/PROJECT.md`.

## APIs & External Services

### D-Bus System Bus — `com.zenomen.Hardware1` (privileged hardware control)

This is the primary integration contract between the unprivileged GUI and privileged hardware access.

| Property | Value | Source |
|---|---|---|
| Bus | system bus (session bus only when `ZENOMEN_HW_SESSION` is set, for development) | `src/bin/zenomen-hw.rs:26`–`30` |
| Bus name | `com.zenomen.Hardware1` | `src/bin/zenomen-hw.rs:34` |
| Object path | `/com/zenomen/Hardware1` | `src/bin/zenomen-hw.rs:35` |
| Interface | `com.zenomen.Hardware1` | `src/service/contract.rs:82` |
| Owner | `root` (enforced by D-Bus policy) | `data/dbus/com.zenomen.Hardware1.conf:6`–`9` |
| Client | `HardwareServiceClient` | `src/service/client.rs:34`–`324` |
| Server | `HardwareService` | `src/service/contract.rs:14`–`213` |
| Backend | `HardwareBackend` (sysfs writes + read-back) | `src/service/backend.rs:52`–`475` |

**Methods** (`src/service/contract.rs`; zbus exposes these as PascalCase):

| Method | Signature | Mutating / polkit-gated | Purpose |
|---|---|---|---|
| `GetCapabilities` | `() -> String` (JSON) | No | Reports `has_fan_control`, `has_power_profiles`, `has_rgb_lighting`, `rgb_variant`, `service_version`. Used as the liveness probe by `status()` (`src/service/client.rs:78`–`98`). |
| `GetFanStatus` | `() -> (String, Vec<u32>)` | No | Active fan mode and RPMs. |
| `SetFanMode` | `(String) -> String` | **Yes** | Accepts only `auto` / `max`; verifies read-back. |
| `GetPowerProfiles` | `() -> Vec<String>` | No | ACPI `platform_profile_choices`. |
| `GetPowerProfile` | `() -> String` | No | Active ACPI profile. |
| `SetPowerProfile` | `(String) -> String` | **Yes** | Validated against advertised choices; rejects newline/`/`/NUL; verifies read-back. |
| `ApplyPreset` | `(String) -> String` (JSON `PresetResult`) | **Yes** | Combined quiet/balanced/gaming preset; reports partial success honestly. |
| `ApplyRgbProfile` | `(String JSON) -> String` | **Yes** | Deserializes `RgbProfile`, applies `zones` to sysfs, verifies. |
| `SetRgbEnabled` | `(bool) -> String` | **Yes** | `true` applies white to all zones, `false` applies black. |

**Authentication:** every mutating method calls `HardwareService::authorize()` (`src/service/contract.rs:55`–`79`), which takes the D-Bus message sender, connects to `org.freedesktop.PolicyKit1` at `/org/freedesktop/PolicyKit1/Authority`, interface `org.freedesktop.PolicyKit1.Authority`, and calls `CheckAuthorization` with subject `("system-bus-name", {"name": <unique sender>})`, action `com.zenomen.hardware.control`, and interactive flag `1`. See `src/service/polkit.rs:28`–`71`.
- The service caches its own `zbus::Connection` so it can perform the polkit call (`src/service/contract.rs:31`–`35`, `src/bin/zenomen-hw.rs:40`).
- `with_polkit_required(false)` exists for tests/dev only (`src/service/contract.rs:44`–`47`); do not use it in production paths.
- The GUI performs a **non-interactive** polkit check (`flags = 0`) for the Diagnostics screen via `check_polkit_status()` (`src/service/client.rs:232`–`279`).

**Error mapping:** `HardwareControlError` → `zbus::fdo::Error` in `src/service/types.rs`:
- `PolkitDenied` → `AccessDenied`
- `InvalidParameter` → `InvalidArgs`
- `UnsupportedMode` → `NotSupported`
- `InterfaceMissing` → `FileNotFound`
- `VerificationFailed` / `IoError` → `Failed`

**D-Bus policy** (`data/dbus/com.zenomen.Hardware1.conf`):
- Only `root` may own the name and send to it.
- Every local user may send to and receive from `com.zenomen.Hardware1`; mutating authorization is delegated to polkit inside the service.

**D-Bus activation** (`data/dbus/com.zenomen.Hardware1.service`): `Name=com.zenomen.Hardware1`, `Exec=/usr/bin/zenomen-hw`, `User=root`. Installed to `/usr/share/dbus-1/system-services/`.

### Polkit

- Action ID: `com.zenomen.hardware.control` — constant `POLKIT_ACTION_CONTROL` (`src/service/polkit.rs:10`).
- Policy file: `data/polkit/com.zenomen.hardware.policy`, installed to `/usr/share/polkit-1/actions/`.
- Defaults: `allow_any = auth_admin`, `allow_inactive = auth_admin`, `allow_active = auth_admin_keep`. Expect an admin password prompt for fan/power/RGB changes, cached for the active session.
- No custom polkit agent is implemented; the desktop's agent handles the prompt. Mutating RPCs get a 30-second budget off the monitoring thread so a prompt cannot stall telemetry (`src/core/worker.rs:348`–`379`).

### systemd

- Unit: `data/systemd/zenomen-hw.service` → `/usr/lib/systemd/system/zenomen-hw.service`.
- `Type=dbus`, `BusName=com.zenomen.Hardware1`, `ExecStart=/usr/bin/zenomen-hw`, `Restart=on-failure`, `RestartSec=3s`.
- Hardened: `ProtectSystem=strict`, `ProtectHome=true`, `ReadOnlyPaths=/`, `PrivateTmp=true`, `RestrictAddressFamilies=AF_UNIX`, `SystemCallFilter=@system-service`, `CapabilityBoundingSet=` (empty), `NoNewPrivileges=true`, `MemoryDenyWriteExecute=true`, `LockPersonality=true`.
- Writable carve-outs are prefixed with `-` so missing paths do not fail startup: `/sys/devices/platform/hp-wmi`, `/sys/firmware/acpi`, `/sys/class/hwmon`, `/sys/class/leds`.
- `ProtectKernelTunables` is intentionally omitted because it would make `/sys` read-only (`data/systemd/zenomen-hw.service:21`–`22`). Do not add it.
- Lifecycle is driven by `systemctl daemon-reload/enable/restart` from the installer (`src/desktop/system_integration.rs:127`–`131`, `:189`–`215`).

### D-Bus Session Bus — GUI desktop integration

| Integration | Bus name / path / interface | File |
|---|---|---|
| Single instance | owns `com.zenomen.App`, object `/com/zenomen/App`, interface `com.zenomen.App` | `src/desktop/single_instance.rs:10`–`12` |
| Single-instance RPCs | `PresentWindow`, `Activate`, `Quit` | `src/desktop/single_instance.rs:32`–`54` |
| System tray (SNI) | `org.kde.StatusNotifierItem` at `/StatusNotifierItem` | `src/desktop/tray.rs:14`, `:28`–`106` |
| Tray menu | `com.canonical.dbusmenu` at `/MenuBar` | `src/desktop/tray.rs:15`, `:122`–`183` |
| Tray registration | calls `RegisterStatusNotifierItem` on `org.kde.StatusNotifierWatcher` at `/StatusNotifierWatcher` | `src/desktop/tray.rs:202`–`217` |

- Name acquisition uses `RequestName("com.zenomen.App", 4)` (`DBUS_NAME_FLAG_DO_NOT_QUEUE`); replies `1`/`4` mean primary, `2`/`3` mean an instance already exists and `PresentWindow` is invoked before exiting (`src/desktop/single_instance.rs:77`–`113`).
- If session D-Bus is unavailable, the GUI runs standalone with a warning rather than failing (`src/main.rs:68`–`72`).
- Tray absence is non-fatal: if no `StatusNotifierWatcher` exists (stock GNOME without AppIndicator), registration is logged and startup continues (`src/desktop/tray.rs:212`–`217`).

## Kernel Integration Surface (sysfs / procfs)

All hardware access is direct file I/O; there are no ioctls, no libsensors, and no kernel module of ZenOmen's own.

**Telemetry (read-only, GUI process):**

| Interface | Data | Code |
|---|---|---|
| `/proc/stat` | Aggregate CPU utilization from jiffies deltas | `src/hardware/linux/cpu.rs:28`–`153`, `src/hardware/linux/provider.rs:122`–`129` |
| `/proc/cpuinfo` | CPU model string and frequency fallback | `src/hardware/linux/cpu.rs:156`–`186`, `src/hardware/linux/provider.rs:61`, `:134`–`137` |
| `/sys/devices/system/cpu/cpu0/cpufreq/scaling_cur_freq` | Current CPU frequency (kHz → MHz) | `src/hardware/linux/provider.rs:38`, `src/hardware/linux/cpu.rs:170`–`174` |
| `/proc/meminfo` | RAM total/available; used = total − available | `src/hardware/linux/memory.rs:30`–`73` |
| `/sys/class/hwmon/hwmon*/name` | hwmon driver identification | `src/hardware/linux/hwmon.rs:33`–`62` |
| `/sys/class/hwmon/hwmon*/temp*_input` + `temp*_label` | Temperatures (millidegrees) and labels | `src/hardware/linux/hwmon.rs:67`–`86` |
| `/sys/class/hwmon/hwmon*/fan*_input` | Fan tachometer RPM | `src/hardware/linux/hwmon.rs:74`–`77`, `src/hardware/registry.rs:204`–`251` |
| `/sys/class/drm/card[0-9]+/device/vendor` | PCI vendor ID (0x1002 AMD, 0x8086 Intel, 0x10de NVIDIA) | `src/hardware/linux/gpu.rs:66`–`141` |
| `/sys/class/drm/card*/device/gpu_busy_percent` | AMD utilization | `src/hardware/linux/gpu.rs:151`–`155` |
| `/sys/class/drm/card*/device/mem_info_vram_{used,total}` | AMD VRAM | `src/hardware/linux/gpu.rs:167`–`177` |
| `/sys/class/drm/card*/device/hwmon/hwmon*/temp1_input` | GPU temperature | `src/hardware/linux/gpu.rs:117`–`127`, `:158`–`164` |
| `/sys/firmware/acpi/platform_profile` | Active power profile | `src/hardware/registry.rs:80`–`84` |
| `/sys/firmware/acpi/platform_profile_choices` | Advertised profiles | `src/hardware/registry.rs:87`–`93` |
| `/etc/os-release`, `/proc/sys/kernel/osrelease`, `/sys/class/dmi/id/{product_name,sys_vendor}`, `/sys/class/drm` | Diagnostics report metadata | `src/core/diagnostics.rs:61`–`89` |
| `/proc/self/smaps_rollup`, `/proc/self/status`, `/proc/self/stat` | RSS/PSS/private-dirty and CPU usage instrumentation | `src/core/benchmark.rs:44`–`200` |

**Hardware control (privileged `zenomen-hw` only):**

| Interface | Operation | Code |
|---|---|---|
| `/sys/devices/platform/hp-wmi/hwmon/hwmon*/pwm1_enable` (primary) or `/sys/class/hwmon/hwmon*/pwm1_enable` where `name` is `hp`/`hp-wmi` (fallback) | Fan mode: `0` = max, `2` = auto. Written then read back with up to 3 attempts and exponential backoff. | `src/service/backend.rs:77`–`115`, `:162`–`176`, `:12`–`48` |
| `/sys/firmware/acpi/platform_profile` | Power profile write, validated against `platform_profile_choices` | `src/service/backend.rs:217`–`248` |
| `/sys/devices/platform/hp-wmi/rgb_zones/zone0{0..3}` | RGB variant `FourZone`; hex string per zone | `src/service/backend.rs:324`–`331`, `:418`–`425` |
| `/sys/devices/platform/hp-wmi/keyboardleds` | RGB variant `Legacy`; one 24-character hex payload | `src/service/backend.rs:333`–`337`, `:426`–`436` |
| `/sys/class/leds/hp_omen::kbd_backlight/zone_colors` | RGB variant `KernelPatch`; 12 raw bytes | `src/service/backend.rs:339`–`346`, `:437`–`472` |
| Any of the RGB interfaces | Read back current colors | `src/service/backend.rs:354`–`409` |

**Notes:**
- Fan mode sysfs semantics: `0` = Max, `1` = Manual, `2` = Auto (`src/service/types.rs:31`–`44`). v1 only writes `auto` and `max`.
- Every write is verified by reading the file back; a mismatch returns `VerificationFailed` rather than claiming success (`src/service/backend.rs:39`–`47`).
- A missing interface yields `InterfaceMissing` → D-Bus `FileNotFound`, which the UI renders as "Unavailable"; never a fabricated zero.
- The GUI process must not gain write access to these paths. Keep all mutations behind `com.zenomen.Hardware1`.

## External Processes

| Process | Required? | Invocation | Purpose |
|---|---|---|---|
| `nvidia-smi` | Optional | `nvidia-smi --query-gpu=name,utilization.gpu,temperature.gpu,memory.used,memory.total --format=csv,noheader,nounits`, 200 ms timeout on a helper thread (`src/hardware/linux/gpu.rs:187`–`219`) | NVIDIA telemetry. Absent/suspended GPU degrades to `None`; never blocks the sampling loop. |
| `xdg-open` | Optional | `xdg-open <releases URL>`, stdio null, child reaped asynchronously (`src/desktop/releases.rs:26`–`44`) | Open the GitHub Releases page. |
| `systemctl` | Install/uninstall only | `/usr/bin/systemctl` or `/bin/systemctl`, `--no-ask-password`, `daemon-reload`/`enable`/`restart`/`stop`/`disable` (`src/desktop/system_integration.rs:189`–`215`) | Register/unregister `zenomen-hw`. |

All commands are spawned with explicit argument vectors; no `sh -c`, no shell interpolation, no arbitrary user-supplied paths.

## Data Storage

**Databases:** None. No SQL, no embedded KV store, no daemon-side persistence.

**File storage (local only):**
- `~/.config/zenomen/config.toml` — user settings, atomic replace (`src/config/mod.rs:84`–`105`). Corrupt files are backed up as `config.toml.bak.<unix-seconds>` (`src/config/mod.rs:52`–`77`).
- `~/.config/zenomen/rgb/*.json` — RGB profiles, serde JSON, validated `#RRGGBB` (`src/rgb/schema.rs:47`–`75`).
- `~/.config/zenomen/diagnostics/zenomen-diagnostics-<timestamp>.json` — user-triggered diagnostics export (`src/app.rs:233`–`241`, `src/core/diagnostics.rs`).
- `~/.config/autostart/zenomen.desktop` — XDG autostart entry, created/removed on toggle (`src/desktop/autostart.rs:17`–`92`).
- `~/.config/openomen/rgb/*.json` — **read-only legacy import source**; never modified or deleted (`src/rgb/migration.rs:4`–`6`, `:185`–`270`).
- System paths owned by packaging: `/usr/bin/zenomen`, `/usr/bin/zenomen-hw`, `/usr/share/applications/zenomen.desktop`, `/usr/lib/systemd/system/zenomen-hw.service`, `/usr/share/dbus-1/system-services/com.zenomen.Hardware1.service`, `/usr/share/dbus-1/system.d/com.zenomen.Hardware1.conf`, `/usr/share/polkit-1/actions/com.zenomen.hardware.policy`, `/usr/share/icons/hicolor/*/apps/zenomen.*` (`Cargo.toml:64`–`75`, `docs/PACKAGING.md:11`–`21`).

**Caching:** None. Hardware state is read fresh each tick; no snapshot database, no on-disk telemetry history. The in-memory history is a bounded ring in the app state (default 120 samples).

**Concurrency:** Config writes are atomic (tmp + `sync_all` + rename). RGB writes go through the daemon's verified-write helper. There is no file locking across processes; the single-instance D-Bus name is the guard against two GUIs.

## Authentication & Identity

- **No user accounts, no login, no tokens, no OAuth, no cloud identity.**
- **Authorization is polkit only**, scoped to `com.zenomen.hardware.control`, with the service running as root under systemd.
- D-Bus name ownership is restricted to root by policy; any local user may call read-only methods, while mutating methods are gated inside the service.
- The installer requires euid 0 (`src/desktop/system_integration.rs:41`–`53`, `:117`–`120`); it refuses to write anything as non-root.

## Monitoring & Observability

**Error tracking:** None. No Sentry, no crash reporter, no remote logging.

**Logs:**
- `tracing` to stderr in both binaries. GUI: `tracing_subscriber::fmt::init()` (`src/main.rs:75`). Daemon: `EnvFilter` defaulting to `info` (`src/bin/zenomen-hw.rs:12`–`16`).
- `RUST_LOG` controls verbosity. Structured fields are used (`version`, `scenario`, `path`, `error`, `rss_mb`).
- The daemon logs service start, name registration, polkit denials, and verification failures.
- Optional stdout benchmark lines when `ZENOMEN_BENCHMARK=1` (`src/core/benchmark.rs:245`–`256`).

**User-facing diagnostics:** JSON report with OS, kernel, DMI product/vendor, capabilities, sensor bindings, service status, polkit status, latest telemetry, last sampling error, and UTC timestamp (`src/core/diagnostics.rs:29`–`56`). Exported locally; never uploaded.

**Budget measurement:** `scripts/measure-budget.sh` reads `/proc/<pid>/status` and `/proc/<pid>/stat`; results recorded in `docs/RESOURCE-BUDGET.md`. Targets: GUI ≤40 MB peak RSS and ≤0.1% idle CPU; daemon budget reported separately (≤15 MB).

## CI/CD & Deployment

**Hosting:** GitHub repository `https://github.com/Omprakash-p06/zenomen`; releases on GitHub Releases.

**CI (`.github/workflows/ci.yml`):** push/PR to `main`; stable Rust with clippy + rustfmt; native deps; `cargo fmt --check`; Clippy `-D warnings`; `xvfb-run cargo test --all`; `cargo audit`. `permissions: contents: read`.

**Release (`.github/workflows/release.yml`):** triggered by `v*` tags or manual dispatch.
- Verifies the tag matches `Cargo.toml` version.
- Runs the same quality gates, then `cargo packager --release --formats deb,appimage`.
- Requires `libxkbcommon-dev`, `libfontconfig1-dev`, `libgl1-mesa-dev`, `xvfb`, `dpkg-dev`, `file`, `libfuse2`.
- Writes `dist/SHA256SUMS`, publishes `.deb`, `.AppImage`, and checksums via `softprops/action-gh-release`.
- Actions are pinned by commit SHA in the release workflow.

**Distribution targets:** Debian/Ubuntu `.deb`; portable `.AppImage`. AppImage users run `sudo ./ZenOmen_*.AppImage --install-system-integration` once to install the daemon, unit, D-Bus, polkit, and desktop entry from embedded assets (`src/desktop/system_integration.rs:20`–`26`, `docs/PACKAGING.md:85`–`97`).

## Environment Configuration

**Required env vars:** None. The application starts and functions with an empty environment (given a session bus and the expected kernel interfaces).

**Optional env vars:**
- `RUST_LOG` — log filtering.
- `ZENOMEN_MOCK_SCENARIO` (`nominal` / `zero` / `gap`) — replace the Linux provider with `MockHardware`.
- `ZENOMEN_BENCHMARK=1` — emit benchmark lines.
- `ZENOMEN_HW_SESSION` — bind `zenomen-hw` to the session bus for local testing.

**Secrets:** None in the repository. No `.env`, no credential files, no signing keys committed. Release signing/notarization is not configured beyond SHA-256 checksums.

## Webhooks & Callbacks

**Incoming:** None. No HTTP server, no socket listener, no webhook endpoint. The only inbound channels are D-Bus method calls on the system and session buses.

**Outgoing:** None. No webhook POSTs, no telemetry callbacks, no update pings.

## Integration Constraints for Future Work

- The GUI process must remain unprivileged: no direct sysfs writes, no `sudo`, no shell commands. All mutations go through `com.zenomen.Hardware1`.
- The daemon must keep accepting only predefined operations and validated values (mode allow-list, profile membership check, 4-zone RGB schema). Never accept arbitrary paths, command strings, or shell fragments.
- New D-Bus methods must be polkit-gated if they mutate hardware, and must be mirrored in `src/service/client.rs` and (if user-visible) the diagnostics report.
- Keep the no-network guarantee: no HTTP client, no update downloader. Update discovery stays a browser hand-off via `src/desktop/releases.rs`.
- Every new sysfs read must degrade to `None`/`Unavailable`, and every new write must use `write_and_verify` semantics (`src/service/backend.rs:12`–`48`).
- If adding a hardware path to the daemon, update the systemd `ReadWritePaths` carve-outs, otherwise `ProtectSystem=strict` will deny the write.

---

*Integration audit: 2026-11-02*
