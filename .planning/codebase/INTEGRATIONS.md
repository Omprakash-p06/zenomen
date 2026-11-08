# External Integrations

**Analysis Date:** 2026-11-08

## APIs & External Services

**Cloud/SaaS APIs:**
- None. There is no HTTP client dependency and no network code. The only outbound web action is delegating a GitHub Releases URL to the user's browser:
  - URL `https://github.com/Omprakash-p06/zenomen/releases/latest`
  - Linux: spawns `xdg-open` (`src/desktop/releases.rs`).
  - Windows: calls `ShellExecuteW` (`src/desktop/windows/releases.rs`).
  - No download, update, or telemetry upload occurs.

**External executables invoked:**
- `nvidia-smi` - one long-lived watcher thread queries `--query-gpu=name,utilization.gpu,temperature.gpu,memory.used,memory.total --format=csv,noheader,nounits`; a sample older than 3 seconds reads as unavailable (`src/hardware/linux/gpu.rs`). Search order: presence of `/usr/bin/nvidia-smi` or `/bin/nvidia-smi`.
- `xdg-open` - launches the Releases page (`src/desktop/releases.rs`).
- `systemctl` - root-only AppImage system integration install/uninstall calls `stop`, `disable`, `enable`, `daemon-reload` against `zenomen-hw` (`src/desktop/system_integration.rs`).
- No shell execution, no arbitrary command path, no raw WMI payload is exposed through the IPC protocols.

## Data Storage

**Databases:**
- None. No database crate, schema, or migration exists.

**File storage (local filesystem only):**
- Main config: `config.toml` under `directories::ProjectDirs::from("com", "zenomen", "zenomen")` config dir; atomic writes and `.bak.<timestamp>` recovery in `src/config/mod.rs`.
- RGB profiles: `config_dir()/rgb/*.json`; migration reads `~/.config/openomen/rgb` without modifying it (`src/rgb/migration.rs`, `src/rgb/schema.rs`).
- Diagnostics report: `zenomen-diagnostics-<timestamp>.json`, exported atomically (`src/core/diagnostics.rs`, `src/app.rs`). Linux path is `config_dir()/diagnostics`; Windows path is `%LOCALAPPDATA%\ZenOmen\diagnostics`.
- Logs: daily-rotated `zenomen*.log`, maximum 7 files (`src/logging.rs`).
- Windows service state: `%PROGRAMDATA%\ZenOmen` and user state `%LOCALAPPDATA%\ZenOmen`, resolved through `SHGetKnownFolderPath` (`src/desktop/windows/paths.rs`).

**Caching:**
- No external cache. Telemetry history is in-memory in `AppState` (`src/core/state.rs`); the NVIDIA sample is a small in-memory cache with a 3-second freshness bound (`src/hardware/linux/gpu.rs`).

## Authentication & Identity

**Linux:**
- Polkit action `com.zenomen.hardware.control`, declared in `data/polkit/com.zenomen.hardware.policy` with `auth_admin` / `auth_admin_keep`.
- The daemon calls `org.freedesktop.PolicyKit1.Authority.CheckAuthorization` on `/org/freedesktop/PolicyKit1/Authority` for every mutating D-Bus method (`src/service/polkit.rs`, `src/service/contract.rs`).
- D-Bus policy allows only root to own `com.zenomen.Hardware1` and any local user to call it; authorization remains inside the service (`data/dbus/com.zenomen.Hardware1.conf`).
- systemd hardening restricts the daemon: `ProtectSystem=strict`, `ProtectHome=true`, `ReadOnlyPaths=/`, writable carve-outs for `/sys/devices/platform/hp-wmi`, `/sys/firmware/acpi`, `/sys/class/hwmon`, `/sys/class/leds`, empty capability sets, `NoNewPrivileges=true`, `RestrictAddressFamilies=AF_UNIX`, `SystemCallFilter=@system-service` (`data/systemd/zenomen-hw.service`).

**Windows:**
- Service `ZenOmenHardwareService` runs as `NT AUTHORITY\LocalSystem`; installed by MSI (`packaging/windows/ZenOmen.wxs`) or `zenomen-hw --install-service` (`src/service/windows/lifecycle.rs`).
- Named pipe DACL: `D:(A;;GA;;;SY)(A;;GA;;;BA)(A;;GRGW;;;IU)` - full control for SYSTEM/Administrators, read/write for Interactive Users (`src/service/windows/security.rs`).
- Caller session validation uses `GetNamedPipeClientSessionId` against `WTSGetActiveConsoleSessionId`, with token impersonation as defense-in-depth (`src/service/windows/security.rs`).
- Board safety gate: mutations are rejected unless `BoardValidation` matches the detected `BaseBoardProduct` and the specific read/write flag is verified (`src/service/windows/authorization.rs`, `src/service/windows/profiles.rs`).

**GUI:**
- No login or user account. The GUI is always unprivileged and performs no raw hardware writes.

## Monitoring & Observability

**Error tracking:**
- None (no Sentry/remote reporting).

**Logs:**
- `tracing` with stderr and a non-blocking daily rolling file layer; failures degrade to stderr-only instead of blocking startup (`src/logging.rs`).
- Log destinations: Linux `~/.local/state/zenomen/logs`; Windows GUI `%LOCALAPPDATA%\ZenOmen\logs`; Windows service `%PROGRAMDATA%\ZenOmen\logs`.
- Diagnostics report includes OS/kernel strings, DMI vendor/product, capabilities, sensor bindings, GPU list, service connection/Polkit status, latest telemetry, recent error, and timestamp (`src/core/diagnostics.rs`).

## CI/CD & Deployment

**Hosting:**
- GitHub Releases for binaries/packages. Linux artifacts: `.deb`, `.AppImage`, `SHA256SUMS` via `cargo-packager`; Windows artifacts: `.msi` (WiX) and `.zip` of `zenomen.exe` + `zenomen-hw.exe` (`.github/workflows/release.yml`).

**CI pipeline:**
- `.github/workflows/ci.yml`: Ubuntu quality job (`cargo fmt --check`, clippy `-D warnings`, `xvfb-run cargo test --all`, `cargo audit`) and Windows quality job (`cargo fmt --check`, clippy, `cargo test --all`).
- `.github/workflows/release.yml`: triggered by `v*` tags or manual dispatch; verifies tag matches `Cargo.toml`, builds both OS artifacts, consolidates SHA-256 checksums, and publishes a GitHub Release.
- No container image, no cloud deployment target, no package registry publish.

## Environment Configuration

**Required env vars:**
- None are required for a normal run.
- Optional:
  - `ZENOMEN_MOCK_SCENARIO` - `nominal`, `zero-capabilities`, or `sensor-data-gap` (`src/hardware/mock.rs`, `src/main.rs`).
  - `ZENOMEN_HW_SESSION` - Linux daemon uses session D-Bus instead of system D-Bus (`src/bin/zenomen-hw.rs`).
  - `RUST_LOG` - log filtering (`src/logging.rs`).
  - `LC_CTYPE`, `LC_ALL`, `LANG` - Linux locale detection; `LC_CTYPE=C.UTF-8` is set internally when needed (`src/main.rs`).

**Secrets location:**
- No application secrets, API keys, tokens, or credentials are stored or read. No `.env` files are present. Polkit and Windows session/DACL checks supply authorization.

## Webhooks & Callbacks

**Incoming network webhooks:**
- None.

**Local IPC callbacks / signals:**
- Linux system D-Bus service `com.zenomen.Hardware1` at `/com/zenomen/Hardware1` (`src/service/contract.rs`). Methods: `GetCapabilities`, `GetFanStatus`, `SetFanMode`, `GetPowerProfiles`, `GetPowerProfile`, `SetPowerProfile`, `ApplyPreset`, `ApplyRgbProfile`, `SetRgbEnabled`.
- Linux session D-Bus single-instance service `com.zenomen.App` at `/com/zenomen/App`; it emits `PresentWindow`, fan-mode actions, and `Quit` (`src/desktop/single_instance.rs`).
- Freedesktop tray: `org.kde.StatusNotifierItem` at `/StatusNotifierItem`, `com.canonical.dbusmenu` at `/MenuBar`, registered with `org.kde.StatusNotifierWatcher` (`src/desktop/tray.rs`).
- Windows named pipe `\\.\pipe\ZenOmen.Hardware.v1`, message mode, 4-byte big-endian length prefix + JSON (`src/service/windows/pipe.rs`, `src/service/windows/protocol.rs`).
- Windows hidden tray message window receives `WM_TRAY_CALLBACK` (`0x0400 + 101`) for menu actions (`src/desktop/windows/tray.rs`).

---

## OS / Hardware Integration Surface

### Linux-only integrations

All paths are read by the unprivileged provider; writes are confined to the privileged daemon `zenomen-hw`.

**procfs/sysfs telemetry:**
- `/proc/stat` - aggregate and per-core CPU jiffies (`src/hardware/linux/cpu.rs`).
- `/proc/cpuinfo` - CPU model and frequency fallback (`src/hardware/linux/cpu.rs`).
- `/proc/meminfo` - `MemTotal` minus `MemAvailable` for used RAM (`src/hardware/linux/memory.rs`).
- `/sys/devices/system/cpu/cpu0/cpufreq/scaling_cur_freq` - CPU frequency in kHz (`src/hardware/linux/provider.rs`).
- `/sys/class/hwmon/hwmon*/name`, `temp*_input`, `temp*_label`, `fan*_input`, `pwm1_enable` - sensor discovery (`src/hardware/linux/hwmon.rs`, `src/hardware/registry.rs`).
- CPU temperature priority: `k10temp` (`Tctl`, `Tdie`), then `coretemp` (`Package id 0`), then `acpitz` `temp1_input` (`src/hardware/registry.rs`).
- Fan tachometers are enumerated from any hwmon `fan*_input`; fan control is restricted to devices named `hp`, `hp-wmi`, or containing `platform` and exposing `pwm1_enable`.
- `/sys/firmware/acpi/platform_profile` and `platform_profile_choices` - active and advertised ACPI power profiles (`src/hardware/registry.rs`, `src/service/backend.rs`).
- `/sys/class/drm/card[0-9]+/device/` - GPU vendor ID, `gpu_busy_percent`, `mem_info_vram_used`, `mem_info_vram_total`, and linked `hwmon/*/temp1_input` (`src/hardware/linux/gpu.rs`).
- `/sys/class/power_supply/BAT*/` - `capacity`, `status`, `time_to_empty_now`, `time_to_full_now` (`src/hardware/linux/provider.rs`).
- `/sys/class/dmi/id/product_name` and `sys_vendor`; `/etc/os-release`; `/proc/sys/kernel/osrelease` - diagnostics only (`src/core/diagnostics.rs`).

**Privileged write targets (daemon, `src/service/backend.rs`):**
- Fans: `/sys/devices/platform/hp-wmi/hwmon/hwmon*/pwm1_enable`, fallback to any `/sys/class/hwmon/hwmon*` whose `name` is `hp` or `hp-wmi`. Mapping: `0` = max, `1` = manual, `2` = auto. Writes are verified by read-back with 3 attempts and 50 ms exponential backoff.
- Power: `/sys/firmware/acpi/platform_profile`, validated against `platform_profile_choices`; 3 retries at 50 ms.
- RGB interface variants (`src/hardware/rgb.rs`, `src/service/backend.rs`):
  - Four-Zone: `/sys/devices/platform/hp-wmi/rgb_zones/zone00` ... `zone03` (6-hex-digit writes, 3 retries at 10 ms).
  - Legacy: `/sys/devices/platform/hp-wmi/keyboardleds` (single 24-character payload, 3 retries at 10 ms).
  - Kernel patch: `/sys/class/leds/hp_omen::kbd_backlight/zone_colors` (12 raw bytes, single read-back comparison).

**Linux IPC and service management:**
- D-Bus system bus: `com.zenomen.Hardware1`, object `/com/zenomen/Hardware1` (`src/bin/zenomen-hw.rs`, `src/service/contract.rs`).
- D-Bus activation file `data/dbus/com.zenomen.Hardware1.service` (`Exec=/usr/bin/zenomen-hw`, `User=root`, `SystemdService=zenomen-hw.service`).
- systemd unit `data/systemd/zenomen-hw.service` (`Type=dbus`, `BusName=com.zenomen.Hardware1`).
- Polkit policy `data/polkit/com.zenomen.hardware.policy`; action `com.zenomen.hardware.control`.
- XDG autostart entry `~/.config/autostart/zenomen.desktop` with `Exec=zenomen --minimized` (`src/desktop/autostart.rs`).
- StatusNotifierItem tray requires a Freedesktop SNI host; when `org.kde.StatusNotifierWatcher` is absent the tray is optional and startup continues (`src/desktop/tray.rs`).

### Windows-only integrations

**Win32 telemetry APIs (unprivileged `WindowsHardwareProvider`):**
- CPU utilization via `GetSystemTimes` deltas; model name read via registry/CPU brand path (`src/hardware/windows/cpu.rs`).
- RAM via `GlobalMemoryStatusEx` (`src/hardware/windows/memory.rs`).
- Battery via `GetSystemPowerStatus`; `BatteryFlag == 128` means no battery (`src/hardware/windows/battery.rs`).
- GPU enumeration via `dxgi.dll!CreateDXGIFactory1` / `IDXGIFactory1::EnumAdapters1`, and utilization via persistent PDH counters (`src/hardware/windows/gpu.rs`).
- Board identity via `HKLM\HARDWARE\DESCRIPTION\System\BIOS` values `SystemManufacturer`, `SystemProductName`, `BaseBoardProduct`, BIOS version (`src/hardware/windows/device.rs`).
- Temperature and fan telemetry are intentionally unavailable: `read_temperature_sensors` returns `(None, None)` and `read_fan_rpms` returns an empty vector (`src/hardware/windows/sensors.rs`).

**HP firmware/WMI interface (documented intent vs current code):**
- Declared namespace `ROOT\WMI`, class `hpqBIntM`, GUID `5FB7F034-2C63-45E9-BE91-3D44E2C707E4` (`src/service/windows/hp_wmi.rs`).
- Declared command IDs: `0x10` fan count, `0x11` fan speeds, `0x1A` performance mode, `0x26` max-fan status, `0x27` max-fan set, `0x28` system design data, `0x20009` four-zone RGB.
- The protocol specification (method `hpqBMtdx`, signature `SECU`, read/write request types) is documented in `docs/architecture/HP_WMI_PROTOCOL.md`.
- Current implementation status: `connect_hp_wmi()` on Windows returns `Ok(HpWmiConnection { is_available: true })` without opening `IWbemServices`, and fan/power state is stored in `static Mutex` values (`src/service/windows/hp_wmi.rs`). No COM/WMI method invocation is implemented. `Win32_System_Wmi` is enabled in `Cargo.toml` but unused. Treat physical WMI mutation as not yet wired despite docstrings and ADRs describing it.
- Board profiles and validation records (`88F8`, `8A24`, `8A3D`/`8A3E`, `88D0`) gate all writes (`src/service/windows/profiles.rs`); the pipe server falls back to `88F8` if the board ID cannot be read (`src/service/windows/server.rs`).

**Windows IPC and service management:**
- Named pipe `\\.\pipe\ZenOmen.Hardware.v1`, `PIPE_TYPE_MESSAGE | PIPE_READMODE_MESSAGE | PIPE_WAIT`, 64 KiB buffers, up to 4 instances, 30 s timeout (`src/service/windows/pipe.rs`).
- Protocol: JSON `HardwareRequest` / `HardwareResponse` with a 4-byte big-endian length prefix, 64 KiB cap (`src/service/windows/protocol.rs`).
- SCM service name `ZenOmenHardwareService`, display name `ZenOmen Hardware Control Service`, auto-start, LocalSystem (`src/service/windows/lifecycle.rs`, `packaging/windows/ZenOmen.wxs`).
- Service binary supports `--install-service`, `--uninstall-service`, `--console`/`-c`, and auto-falls back to console mode when not started by SCM (`src/bin/zenomen-hw.rs`, `src/service/windows/lifecycle.rs`).

**Windows desktop integration:**
- Single instance: session mutex `Local\ZenOmen.SingleInstance`; secondary instance activates the window via `FindWindowW` + `SetForegroundWindow` (`src/desktop/windows/single_instance.rs`).
- Tray: `Shell_NotifyIconW` plus a hidden Win32 message window and `TrackPopupMenu` (`src/desktop/windows/tray.rs`).
- Notifications: balloon/toast through `Shell_NotifyIconW` with `NIF_INFO` (`src/desktop/windows/notifications.rs`).
- Autostart: `HKCU\...\Run` registry value, command includes `--minimized` (`src/desktop/windows/autostart.rs`).
- Releases: `ShellExecuteW` opens the GitHub URL (`src/desktop/windows/releases.rs`).
- Known folders: `FOLDERID_LocalAppData` / `FOLDERID_ProgramData` via `SHGetKnownFolderPath` (`src/desktop/windows/paths.rs`).

### Platform separation notes

- Selection is compile-time in `src/main.rs` and `src/hardware/mod.rs`: Linux provider, Windows provider, or `UnavailableHardwareProvider` for other targets; zero capabilities are reported rather than fabricated readings (`src/hardware/provider.rs`, `src/hardware/unavailable.rs`).
- Cargo target dependencies keep `windows-sys` out of Linux builds and `libc` out of Windows builds (`Cargo.toml`).
- `src/service/windows/`, `src/hardware/windows/`, and `src/desktop/windows/` are declared unconditionally, but every Win32 call is inside `#[cfg(windows)]`/`#[cfg(target_os = "windows")]` blocks with non-Windows fallbacks.
- Linux daemon code uses `zbus`; Windows daemon code uses raw named pipes. Both share the typed contract in `src/service/protocol.rs` at the design level, but the Windows path serializes its own `HardwareRequest`/`HardwareResponse` in `src/service/windows/protocol.rs` (big-endian length prefix) while the shared contract uses a little-endian prefix (`src/service/protocol.rs`).

---

*Integration audit: 2026-11-08*
