<!-- refreshed: 2026-11-11 -->
# External Integrations

**Analysis Date:** 2026-11-11

## APIs & External Services

ZenOmen has **no network integrations**: no HTTP client, no telemetry upload, no accounts, no cloud sync. Every integration is local to the machine.

### Linux Kernel Interfaces (sysfs / procfs)

Read-only telemetry (`src/hardware/linux/`):
- `/proc/stat` — aggregate CPU utilization (`sample_proc_stat`, `src/hardware/linux/cpu.rs`).
- `/proc/cpuinfo` — CPU model and frequency fallback.
- `/proc/meminfo` — RAM via `MemTotal - MemAvailable` (`src/hardware/linux/memory.rs`).
- `/sys/devices/system/cpu/cpu0/cpufreq/scaling_cur_freq` — CPU frequency (`LinuxHardwareProvider`, `src/hardware/linux/provider.rs`).
- `/sys/class/hwmon/hwmon*/` — temperature and fan tachometers via `SensorRegistry::scan_roots` (`src/hardware/registry.rs`, `src/hardware/linux/hwmon.rs`).
  - CPU temperature priority: `k10temp` (Tctl/Tdie) → `coretemp` (Package id 0) → `acpitz` temp1.
  - Fan tachometers: any `fan*_input`; fan *control* is restricted to `hp`/`hp-wmi`/`platform` devices.
- `/sys/class/drm/card*/` — dynamic GPU discovery and AMD utilization/temperature (`src/hardware/linux/gpu.rs`).
- `/sys/class/power_supply/BAT*/{capacity,status,time_to_empty_now,time_to_full_now}` — battery telemetry.
- `/sys/firmware/acpi/platform_profile` and `platform_profile_choices` — ACPI power profiles.
- `/etc/os-release`, `/proc/sys/kernel/osrelease`, `/sys/class/dmi/id/{product_name,sys_vendor}` — diagnostics (`src/core/diagnostics.rs`).

Privileged writes (`src/service/backend.rs`, root only, behind D-Bus):
- `sys/devices/platform/hp-wmi/hwmon/hwmon*/pwm1_enable` — fan mode (0=max, 1=manual, 2=auto), with `write_and_verify` (3 attempts, 50 ms doubling backoff).
- `sys/firmware/acpi/platform_profile` — power profile, validated against `platform_profile_choices` and rejecting names containing `\n`, `\r`, `/`, `\0`.
- RGB interfaces, all under `/sys/devices/platform/hp-wmi/` or `/sys/class/leds/`:
  - `rgb_zones/zone00..zone03` (FourZone, hex strings),
  - `keyboardleds` (Legacy, 24-char concatenated hex),
  - `/sys/class/leds/hp_omen::kbd_backlight/zone_colors` (KernelPatch, 12 raw bytes).
  - Detection order is encoded in `HardwareBackend::find_rgb_interface` and `src/hardware/rgb.rs`.

### External Executables (Linux)

- `nvidia-smi` — optional NVIDIA GPU telemetry. A single background watcher thread (`NvidiaSampler` in `src/hardware/linux/gpu.rs`) is created at provider construction; the 1 Hz tick never blocks on the process and reads the latest sample (or nothing).
- `systemctl` — invoked by `src/desktop/system_integration.rs` (`daemon-reload`, `enable`, `restart`, `stop`, `disable`) with `--no-ask-password`, resolved as `/usr/bin/systemctl` → `/bin/systemctl` → `PATH`.
- GUI providers are resolved by `zenomen::hardware::linux::provider`; no `sudo` is ever invoked by the GUI.

### D-Bus — Linux System Bus (Privilege Separation Boundary)

- Bus name: `com.zenomen.Hardware1`; object path `/com/zenomen/Hardware1`; interface `com.zenomen.Hardware1`.
- Server: `src/service/contract.rs` (`HardwareService`), bus registration in `src/bin/zenomen-hw.rs`.
- Client: `src/service/client.rs` (`HardwareServiceClient`), caching one `zbus::blocking::Connection` in a `Mutex`.
- Methods:
  - `GetCapabilities() -> String` (JSON: `has_fan_control`, `has_power_profiles`, `has_rgb_lighting`, `rgb_variant`, `service_version`)
  - `GetFanStatus() -> (String, Vec<u32>)`
  - `SetFanMode(String) -> String` *(mutating)*
  - `GetPowerProfiles() -> Vec<String>`
  - `GetPowerProfile() -> String`
  - `SetPowerProfile(String) -> String` *(mutating)*
  - `ApplyPreset(String) -> String` (JSON `PresetResult`) *(mutating)*
  - `ApplyRgbProfile(String) -> String` (JSON profile) *(mutating)*
  - `SetRgbEnabled(bool) -> String` *(mutating)*
- Every mutating method calls `HardwareService::authorize`, which extracts the message sender and runs a Polkit `CheckAuthorization` with `flags = 1` (interactive allowed).
- Bus policy: `data/dbus/com.zenomen.Hardware1.conf`; activation file `data/dbus/com.zenomen.Hardware1.service`; unit `data/systemd/zenomen-hw.service`.
- Client-side error text is rewritten into actionable guidance when the service is missing (`connect_service`/`format_service_error` in `src/service/client.rs`).

### D-Bus — Linux Session Bus (Desktop Integration)

- Single instance: `src/desktop/single_instance.rs` owns a session-bus name and exposes `PresentWindow`; a second launch signals the primary and exits 0 (`src/main.rs`).
- Tray: `src/desktop/tray.rs` publishes `org.kde.StatusNotifierItem` and `com.canonical.dbusmenu`-style menu over the session bus.
- Failure to reach the session bus degrades to standalone mode (`SingleInstanceResult::Disabled`) rather than aborting.
- Tests may set `ZENOMEN_HW_SESSION` to move the *service* onto the session bus.

### Polkit Authorization (Linux Privilege Gate)

- Action id: `com.zenomen.hardware.control` (`POLKIT_ACTION_CONTROL` in `src/service/polkit.rs`).
- Policy file: `data/polkit/com.zenomen.hardware.policy`.
- Call: `org.freedesktop.PolicyKit1.Authority.CheckAuthorization` with subject `("system-bus-name", {"name": <unique name>})`.
- Diagnostics performs a *non-interactive* check (`flags = 0`) to report "Authorized" / "challenge required" / "not granted" (`check_polkit_status`, `src/service/client.rs`).

### systemd Service Lifecycle (Linux)

- Unit: `data/systemd/zenomen-hw.service`, installed to `/usr/lib/systemd/system/`.
- Enabled/restarted by `zenomen --install-system-integration` (`src/desktop/system_integration.rs`), which also copies `zenomen-hw` to `/usr/bin/zenomen-hw` (mode 0755, atomic rename) and installs desktop entry, D-Bus policy/service, and Polkit policy.
- Package maintainer scripts: `packaging/linux/debian/{postinst,prerm,postrm}`.

### Windows HP WMI Firmware Interface

Driver module: `src/service/windows/hp_wmi.rs`; protocol constants in `src/service/windows/victus_s.rs`.

- Namespace `ROOT\WMI`, class `hpqBIntM`, methods `hpqBIOSInt0/4/128/1024/4096` chosen by output size (`method_for_output_size`).
- Input instance class `hpqBDataIn`; output `hpqBDataOut128`/`hpqBDataOut4`.
- GUID `5FB7F034-2C63-45E9-BE91-3D44E2C707E4`; signature `SECU`.
- Command family `HPWMI_GM = 0x20008` for all gaming/fan/thermal calls; `0x20009` is the keyboard-lighting family and is **not** used as a write flag.
- Command types: `0x10` fan count (also arms a 120 s manual window), `0x11` generic fan speed, `0x1A` thermal profile write (`[0xFF, profile]`), `0x26` Max Fan read, `0x27` Max Fan write, `0x28` system design data, `0x4C` thermal profile query (diagnostic only), `0x2D` Victus S fan level read, `0x2E` Victus S fan level write, `0x2F` Victus S fan table.
- Board routing: `VICTUS_S_BOARDS` (`8A3D`, `8B2F`, `8BBE`, `8BD4`, `8BD5`, `8C99`, `8C9C`, `8DE1`) uses the Victus S protocol; `8A3D` additionally uses EC thermal-profile restoration (`EC_THERMAL_PROFILE_OFFSET = 0x95`).
- Every mutation reads firmware before and after writing (`set_fan_mode_with_target`, `set_power_profile`), sleeping 150 ms before the verification read-back; disagreement returns `VerificationFailed`.
- Real COM access lives in `src/service/windows/wmi_ffi.rs` (locator, services, class objects, VARIANT/SAFEARRAY handling). Tests substitute `FakeWmiAdapter` (`src/service/windows/hp_wmi.rs`).
- Diagnostics: `src/service/windows/wmi_diag.rs` (`zenomen-hw --wmi-diag`), read-only probes of class types and envelope construction.

### Windows Motherboard Safety & Physical Validation (WIN-07)

- `resolve_board_profile` (`src/service/windows/profiles.rs`) recognizes `88F8`, `8A24`, `8A3D`, `8A3E`, `88D0`; unknown boards get a generic read-only profile in `run_pipe_server`.
- `resolve_board_validation` returns write flags **only** for exact board+BIOS pairs:
  - `88F8` on `F.20`/`F.21`, `8A24` on `F.10`, `8A3D` on `F.22`.
  - `8A3D`/F.22: fan write, max fan, auto restore, and power write authorization are true; manual PWM and RGB remain false; `power_write_verified` and `power_read_verified` are false (EC-managed).
  - Everything else returns `BoardValidation::new_unvalidated` (reads allowed, all writes false).
- Enforcement: `authorize_mutation` (`src/service/windows/authorization.rs`) runs before any firmware call in `handle_request` (`src/service/windows/server.rs`). Read-only requests bypass validation.
- Hardware-in-the-loop evidence procedure: `docs/testing/HP_BOARD_VALIDATION.md`; runbooks `--validate-8a3d-fan` (`validation_runbook.rs`) and `--validate-8a3d-power` (`power_validation_runbook.rs`).
- Shutdown restore: the pipe server clears Max Fan (and re-applies the EC profile on `8A3D` via `SafeFallback(Balanced)`) only when `fan_write_verified` is true; otherwise it logs and skips.

### Windows Named-Pipe IPC (Privilege Separation Boundary)

- Pipe: `\\.\pipe\ZenOmen.Hardware.v1` (`PIPE_NAME`, `src/service/windows/pipe.rs`), 64 KiB buffer.
- Framing: 4-byte big-endian length prefix + JSON; `MAX_PAYLOAD_SIZE = 65536` enforced on encode and decode (`src/service/windows/protocol.rs`).
- Requests/responses: `HardwareRequest` / `HardwareResponse` enums (capabilities, fan status/mode, power profiles/profile, presets, RGB, service memory).
- DACL: SDDL `D:(A;;GA;;;SY)(A;;GA;;;BA)(A;;GRGW;;;IU)` — SYSTEM, Administrators, Interactive Users (`src/service/windows/security.rs`).
- Session check: `GetNamedPipeClientSessionId` must equal `WTSGetActiveConsoleSessionId`; token impersonation is used as defense-in-depth when available, and skipped (logged at debug) in console/dev mode.
- Client (`src/service/windows/client.rs`): `CreateFileW` with `SECURITY_SQOS_PRESENT | SECURITY_IMPERSONATION`, up to 5 attempts with 50 ms `WaitNamedPipeW`, single request/response per connection.

### Windows Service Control Manager (SCM)

- Service name `ZenOmenHardwareService`, display name "ZenOmen Hardware Control Service", `SERVICE_WIN32_OWN_PROCESS`, `SERVICE_AUTO_START`, LocalSystem (`src/service/windows/lifecycle.rs`).
- `StartServiceCtrlDispatcherW` with `ERROR_FAILED_SERVICE_CONTROLLER_CONNECT` fallback to console mode.
- Handles `SERVICE_CONTROL_STOP`/`SHUTDOWN` by setting a shared `AtomicBool` stop signal consumed by the pipe loop.
- Install/uninstall exposed via `zenomen --install-service`, `zenomen --uninstall-service`, and `zenomen-hw --install-service` / `--uninstall-service`.

### Windows Telemetry & Desktop APIs

- CPU: `GetSystemTimes` deltas (`src/hardware/windows/cpu.rs`), `NtQuerySystemInformation`-free implementation.
- RAM: `GlobalMemoryStatusEx` (`src/hardware/windows/memory.rs`).
- GPU: DXGI adapter enumeration (`src/hardware/windows/gpu.rs`), vendor recognition from PCI IDs.
- Temperature: PDH counters `\Thermal Zone Information(\_TZ.THRM|TZ00)\High Precision Temperature` / `Temperature`, Kelvin→Celsius, `None` on absence (`src/hardware/windows/sensors.rs`).
- Battery: `GetSystemPowerStatus` parsing (`src/hardware/windows/battery.rs`).
- Board/BIOS identity: registry/DMI reads in `src/hardware/windows/device.rs` (`read_board_id`, `read_bios_version`, `read_system_identity`).
- Display refresh rate: `EnumDisplayDevicesW` / `EnumDisplaySettingsW` / `ChangeDisplaySettingsExW` with `CDS_TEST` then `CDS_UPDATEREGISTRY` and read-back (`src/hardware/windows/display.rs`).
- Tray: `Shell_NotifyIconW` (`src/desktop/windows/tray.rs`); notifications in `notifications.rs`.
- Autostart: `HKCU\Software\Microsoft\Windows\CurrentVersion\Run` (`src/desktop/windows/autostart.rs`).
- Single instance: named mutex + window present (`src/desktop/windows/single_instance.rs`).
- Process memory: `K32GetProcessMemoryInfo` for GUI and service (`src/hardware/process_memory.rs`).
- Paths: `%LOCALAPPDATA%\ZenOmen` (GUI), `%PROGRAMDATA%\ZenOmen` (service) in `src/desktop/windows/paths.rs`.

## Data Storage

**Files on disk:**
- Config: `config.toml` under the platform config dir, written atomically (`src/config/mod.rs`).
- Backups: `config.toml.bak.<unix-timestamp>` when parsing fails.
- Logs: daily-rotated `zenomen*.log`, max 7 files (`src/logging.rs`).
  - Linux: `~/.local/state/zenomen/logs`.
  - Windows GUI: `%LOCALAPPDATA%\ZenOmen\logs`; service: `%PROGRAMDATA%\ZenOmen\logs` (selected by exe name containing `zenomen-hw`).
- Diagnostics export: `zenomen-diagnostics-<timestamp>.json` under the config dir `diagnostics/` (Linux) or `%LOCALAPPDATA%\ZenOmen\diagnostics` (Windows), atomic rename (`src/core/diagnostics.rs`).
- RGB profiles: JSON files in the ZenOmen RGB directory; legacy OpenOMEN profiles are migrated non-destructively at startup (`src/rgb/migration.rs`, called from `src/main.rs`).

**Databases / caches:** none.

## Authentication & Authorization

- Linux: Polkit action `com.zenomen.hardware.control` on the *system* bus; the GUI never runs as root and never writes sysfs.
- Windows: OS-level separation — GUI runs `asInvoker` (manifest in `build.rs`), service runs as LocalSystem; the pipe DACL plus interactive-console-session check is the authorization boundary. There is no password prompt.
- Physical-write gate: `BoardValidation` flags decide whether any firmware mutation is permitted, independent of OS authorization (WIN-07).
- Firmware authorization errors are surfaced distinctly as `HardwareControlError::AuthorizationFailed` (e.g. `WBEM_E_ACCESS_DENIED`, elevation required).

## Webhooks, Callbacks & Network Communications

- None. No HTTP server, no outbound requests, no webhook receivers, no analytics.
- The only "external link" behavior is `open_releases_page`, which asks the OS to open `RELEASES_URL` in the default browser on explicit user action (`src/desktop/releases.rs`, `src/desktop/windows/releases.rs`).

## Platform Capability Matrix

| Capability | Linux (verified) | Windows (this codebase) |
|---|---|---|
| CPU / RAM telemetry | procfs + sysfs | Win32 + PDK counters |
| GPU telemetry | DRM sysfs, `nvidia-smi` | DXGI enumeration; utilization via GPU collector |
| CPU temperature | hwmon priority matrix | PDH thermal zone, else `Unavailable` |
| Fan RPM | hwmon tachometers | via service (`0x2D` on Victus S, `0x11` otherwise) |
| Fan control | `pwm1_enable` via `zenomen-hw` | `0x27` / Victus S `0x2E`, gated by `BoardValidation` |
| Power profiles | ACPI `platform_profile` | `0x1A` (verified on WMI boards, unverified on `8A3D` EC) |
| RGB | Three sysfs variants | `set_rgb_profile` always returns `UnsupportedMode` (no verified contract) |
| Display refresh | not implemented (`None`/no-op) | GDI mode switch with read-back |
| Battery | `/sys/class/power_supply` | `GetSystemPowerStatus` |
| Unsupported OS | — | third OS: `UnavailableHardwareProvider` (zero capabilities) |

---

*Integrations analysis: 2026-11-11*
