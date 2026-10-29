---
last_mapped_commit: ecfd66bdc4fe2a588e06acb885b8c17802804ae4
last_mapped_at: 2026-10-30
---
# External Integrations

**Analysis Date:** 2026-10-30

## APIs & External Services

**Software Distribution (GitHub Releases):**
- GitHub REST API - Reads the latest release tag to decide whether an update is needed
  - Endpoint: `https://api.github.com/repos/openomen/openomen/releases/latest` (`openomen:881`)
  - SDK/Client: `urllib.request` (stdlib)
  - Auth: **None** — unauthenticated, so subject to the 60 req/hr IP limit
  - SSL: verification **disabled** via `ssl._create_unverified_context()` (`openomen:861`)
- GitHub Release asset download - Fetches the replacement binary
  - Versioned: `https://github.com/openomen/openomen/releases/download/{tag}/openomen` (`openomen:912`)
  - Forced/shortcut: `https://github.com/openomen/openomen/releases/latest/download/openomen` (`openomen:877`)
  - Client: `urllib.request` with a spoofed `User-Agent: Mozilla/5.0` header (`openomen:922`)
  - Sentinel header stripped from the upstream version string: tags may carry a `CLI_` prefix (`openomen:885`)

**GPU Telemetry:**
- NVIDIA Management Library CLI - `nvidia-smi`
  - Query: `--query-gpu=name,temperature.gpu,utilization.gpu,memory.used,memory.total,power.draw --format=csv,noheader,nounits` (`openomen:429`)
  - Query (system info): `--query-gpu=name,driver_version` (`openomen:628`)
  - Client: `subprocess.run(..., timeout=5)`
  - Failure mode handled: `FileNotFoundError`, `subprocess.TimeoutExpired`

**Privilege Escalation:**
- `sudo` - Re-executes the process as root rather than prompting for a password
  - `os.execvp('sudo', ['sudo', <self>, *argv[1:]])` (`openomen:119`)
  - Self path resolution handles PyInstaller: `sys.executable` when `sys.getattr(sys,'frozen',False)`, else `os.path.abspath(sys.argv[0])` (`openomen:116`)
  - Also invoked directly for file replacement: `sudo rm -f <path>` then `sudo mv <tmp> <path>` (`openomen:962`)

## Data Storage

**Databases:**
- None

**File Storage:**
- Local filesystem only.
  - Read-only system interfaces under `/sys` and `/proc` (detailed below)
  - Writable user state: `~/.config/openomen/rgb/state.json` and `~/.config/openomen/rgb/profiles/*.json` (`openomen:1303`–`1305`)
- Default profile seeding: if a profile JSON is absent, `ensure_rgb_config()` writes it (`openomen:1355`–`1367`)

**Hardware sysfs interfaces (read/write, the real "external" contract):**

| Purpose | Path | Access | Code |
|---------|------|--------|------|
| Fan mode | `/sys/devices/platform/hp-wmi/hwmon/hwmon*/pwm1_enable` (`"0"`=max/manual, `"2"`=auto/BIOS) | R/W | `openomen:40`, `openomen:121`, `openomen:136` |
| Fan RPM | `.../fan1_input`, `.../fan2_input` | R | `openomen:58` |
| Thermal | `/sys/class/hwmon/hwmon*/temp*_input` + `_label`, driver from `name` | R | `openomen:220`, `openomen:231` |
| CPU freq | `/sys/devices/system/cpu/cpu*/cpufreq/scaling_cur_freq` | R | `openomen:347` |
| CPU usage/model | `/proc/stat`, `/proc/cpuinfo`, `/proc/version`, `/proc/modules` | R | `openomen:333`, `openomen:578` |
| ACPI power profile | `/sys/firmware/acpi/platform_profile` (R/W), `platform_profile_choices` (R) | R/W | `openomen:748`–`749`, `openomen:774` |
| AMD/Intel GPU | `/sys/class/drm/card[0-9]*/device/{vendor,gpu_busy_percent,hwmon/hwmon*/temp1_input,driver}` | R | `openomen:461`–`530` |
| Battery | `/sys/class/power_supply/BAT*/charge_control_end_threshold` | R (checked only) | `openomen:1206`–`1230` |
| RGB four-zone | `/sys/devices/platform/hp-wmi/rgb_zones/zone00..03` (hex `RRGGBB`) | W | `openomen:1423`, `openomen:1467` |
| RGB legacy | `/sys/devices/platform/hp-wmi/keyboardleds` (concatenated 24-hex) | W | `openomen:1474` |
| RGB kernel-patch | `/sys/class/leds/hp_omen::kbd_backlight/zone_colors` (12 raw bytes RGB×4) | W (binary) | `openomen:1479` |

**Caching:**
- None. Every invocation re-scans sysfs. RGB state is persisted but is configuration, not a cache.

## Authentication & Identity

**Auth Provider:**
- None. No user accounts, no tokens, no API keys anywhere in the codebase.
- Authorization model is purely local Unix root: `ensure_root()` re-execs under sudo (`openomen:113`), and write paths are additionally guarded by `PermissionError` handling (`openomen:172`, `openomen:1486`).

## Monitoring & Observability

**Error Tracking:**
- None. No Sentry/Datadog/etc.

**Logs:**
- `print()` to stdout for user-facing output; `print(..., file=sys.stderr)` for errors.
- `run_doctor()` (`openomen:991`) and `read_system_info()` (`openomen:578`) are the "observability" story: users paste their output into bug reports (README links to GitHub Issues and Discord).
- Monitor mode uses raw ANSI escapes: `CLEAR_SCREEN`, `HIDE_CURSOR`, `SHOW_CURSOR` (`openomen:36`–`38`, used at `openomen:1278`).

## CI/CD & Deployment

**Hosting:**
- GitHub Releases (binary asset named `openomen`)
- Project site/wiki: `https://openomen.github.io/` and the GitHub wiki (linked from `README.md`)

**CI Pipeline:**
- None. No GitHub Actions workflow, no release automation. Building and uploading is manual per `how-to-release.txt`.
- Consequence: `VERSION` (`openomen:33`) and the PyInstaller artifact are bumped by hand each release.

## Environment Configuration

**Required env vars:**
- None.

**Secrets location:**
- None. There are no credentials in the project and none should be added.

## Webhooks & Callbacks

**Incoming:**
- None.

**Outgoing:**
- GitHub API GET (version check) and GitHub release asset GET (download) — both only when the user runs `openomen update`.

## Integration Risks (summary — details in `CONCERNS.md`)

- Update path executes the freshly downloaded, **unverified** binary (`subprocess.run([tmp_path])`, `openomen:938`) and then installs it with root privileges.
- TLS verification is disabled for every update request (`openomen:861`), so the download is open to on-path tampering.
- No checksum or signature is published or checked against the release asset.

---

*Integration audit: 2026-10-30*
