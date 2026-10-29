---
last_mapped_commit: ecfd66bdc4fe2a588e06acb885b8c17802804ae4
last_mapped_at: 2026-10-30
---
# Architecture

**Analysis Date:** 2026-10-30

## System Overview

```text
┌─────────────────────────────────────────────────────────────┐
│                     CLI / Dispatch Layer                     │
│          `main()` — argparse subparser tree                  │
│          `openomen:1557`                                     │
└──────────────────────────┬──────────────────────────────────┘
                           │  args.command / args.*_command
                           ▼
┌─────────────────────────────────────────────────────────────┐
│              Privilege & Capability Gate                     │
│   `ensure_root()` (re-exec via sudo)  `check_root()`         │
│   `find_hwmon_path()` (capability probe)                     │
│   `openomen:110`–`130`, `openomen:40`                        │
└──────────────────────────┬──────────────────────────────────┘
                           ▼
┌───────────────┬───────────────┬───────────────┬─────────────┐
│  Fan control  │ Thermal/CPU   │  GPU telemetry│ Power/RGB   │
│ `set_fan_*`   │ `read_temp*`  │ `read_gpu_*`  │ `set_power_*`│
│ `read_fan_*`  │ `read_cpu_*`  │ `get_amd_*`   │ `apply_rgb_*`│
└───────┬───────┴───────┬───────┴───────┬───────┴──────┬──────┘
        │               │               │              │
        ▼               ▼               ▼              ▼
┌─────────────────────────────────────────────────────────────┐
│     OS Interfaces: /sys (hwmon, drm, acpi, leds) + /proc     │
│     subprocess: nvidia-smi, lsmod, sudo                      │
└──────────────────────────┬──────────────────────────────────┘
                           ▼
┌─────────────────────────────────────────────────────────────┐
│   Presentation: print() to stdout / stderr                   │
│   Persistence: ~/.config/openomen/rgb/{state.json,profiles}  │
└─────────────────────────────────────────────────────────────┘
```

## Component Responsibilities

| Component | Responsibility | File |
|-----------|----------------|------|
| `main()` | Build argparse tree, dispatch on `command`/`fan_command`/`power_command`/`rgb_command`, translate handler booleans into exit codes | `openomen:1557`–`1700` |
| `ensure_root()` / `check_root()` | Acquire root by re-exec'ing under sudo; detect current euid | `openomen:110`–`119` |
| `find_hwmon_path()` | Resolve the `hp-wmi` hwmon directory that actually exposes `pwm1_enable` (models vary) | `openomen:40`–`56` |
| Fan subsystem | Read mode/RPM, write `pwm1_enable`, toggle, verify write stuck | `openomen:58`–`198` |
| Thermal subsystem | Rank hwmon drivers by CPU relevance, pick one temperature, humanize labels | `openomen:200`–`331` |
| CPU subsystem | Read model, per-core usage deltas, frequency, render bars | `openomen:333`–`399` |
| GPU subsystem | NVIDIA via `nvidia-smi`; AMD/Intel via DRM sysfs; vendor-ID dispatch | `openomen:401`–`537` |
| Dashboard | Compose Fan + Power + CPU + GPU sections | `openomen:539`–`576` |
| Diagnostics | `read_system_info()` dump; `run_doctor()` pass/fail checklist with counters | `openomen:578`–`746`, `openomen:991`–`1276` |
| Power profiles | ACPI `platform_profile` read/write + combined profile table | `openomen:748`–`827` |
| Self-update | Locate install path, query GitHub, download, verify, replace | `openomen:829`–`989` |
| Monitor mode | ANSI clear-screen loop around the dashboard | `openomen:1278`–`1301` |
| RGB subsystem | JSON profile CRUD, hardware interface detection, sysfs writes | `openomen:1303`–`1555` |

## Pattern Overview

**Overall:** Monolithic single-file CLI with direct hardware coupling ("scripts-as-CLI"). There is no package, no modules, no classes, and no service abstraction.

**Key Characteristics:**
- One 1703-line executable file `openomen` (extensionless, shebang `#!/usr/bin/env python3`)
- Fully flat namespace: 46 module-level functions/constants, zero classes, zero dataclasses
- Stdlib only; every hardware access is a direct `open()`/`glob()` on a hardcoded `/sys` or `/proc` path
- Functions **print** rather than return data; the boolean return is only a success flag for exit-code mapping
- Handlers are stateless: each invocation reads current hardware state fresh, no daemon, no cache
- Root is acquired by re-exec (process replacement), not by a privilege-separated helper
- Distribution is a frozen binary that can overwrite itself from GitHub

## Layers

**CLI / Dispatch Layer:**
- Purpose: Own all user input parsing and command routing
- Location: `openomen:1557`–`1700`
- Contains: The argparse construction (subparsers for `fan`, `cpu`, `gpu`, `stats`, `info`, `power`, `rgb`, `doctor`, `update`), then one `if/elif` chain on `args.command`
- Depends on: every handler function below it
- Used by: `if __name__ == '__main__'` entry (`openomen:1702`)

**Privilege & Capability Layer:**
- Purpose: Guarantee the permissions and kernel interfaces a command needs before touching hardware
- Location: `openomen:110`–`119` (`ensure_root`), `openomen:40`–`56` (`find_hwmon_path`)
- Contains: euid check, sudo re-exec with frozen-binary awareness, hwmon discovery
- Depends on: `os`, `sys`
- Used by: `set_fan_boost_mode()`, `rgb_power()`, `apply_rgb_hardware()`, `doctor`, `power set/apply`

**Hardware Access Layer:**
- Purpose: Translate sysfs/proc files into Python values and back
- Location: fan `openomen:58`–`198`; thermal `openomen:220`–`331`; power `openomen:758`–`799`; RGB writes `openomen:1423`–`1490`
- Contains: `open()` reads, guarded writes, read-back verification, driver-priority heuristics
- Depends on: the filesystem only
- Used by: presentation layer and dispatch

**Presentation / Composition Layer:**
- Purpose: Format readings for humans
- Location: `openomen:89`–`107`, `openomen:333`–`399`, `openomen:422`–`537`, `openomen:539`–`576`, `openomen:578`–`746`, `openomen:991`–`1276`
- Contains: aligned `print()` calls, Unicode bar charts (`█`/`░`), `─── Section ───` headers, `[✓]`/`[✗]` checklist output
- Depends on: hardware access layer
- Used by: dispatch and `monitor_stats_loop()`

**State / Config Layer:**
- Purpose: Persist user RGB choices across invocations
- Location: `openomen:1303`–`1411`
- Contains: `RGB_CONFIG_DIR`/`RGB_STATE_PATH` constants, `DEFAULT_RGB_PROFILES`, `ensure_rgb_config()`, `read_rgb_state()`, `write_rgb_state()`, `load_rgb_profile()`, `list_rgb_profiles()`
- Depends on: `json`, `os`, `glob`
- Used by: `rgb_status()`, `rgb_set()`, `rgb_power()`

**Self-Management Layer:**
- Purpose: Install/upgrade the tool and diagnose the host
- Location: `openomen:829`–`989` (update), `openomen:991`–`1276` (doctor)
- Contains: install-path resolution (`shutil.which` → `~/.local/bin` → `/usr/local/bin`), GitHub API + asset download, temp-file staging, PyInstaller-safe self-replacement
- Depends on: `urllib`, `ssl`, `tempfile`, `shutil`, `subprocess`, root
- Used by: `update` and `doctor` commands

## Data Flow

### Primary Flow — a mutating command (`openomen fan max`)

1. `sys.exit(main())` at `openomen:1702`
2. `parser.parse_args()` → `args.command == 'fan'`, `args.fan_command == 'max'` (`openomen:1626`)
3. Dispatch branch matches (`openomen:1642`): `ensure_root()`
4. `ensure_root()` sees `geteuid() != 0` → `os.execvp('sudo', ['sudo', <self>, 'fan', 'max'])` (`openomen:113`–`118`). Process is **replaced**; the new root process starts at step 1 again and this time passes the euid check.
5. `set_fan_boost_mode('enable')` (`openomen:136`): `find_hwmon_path()` → opens `pwm1_enable` for write → writes `'0'`
6. `time.sleep(0.1)` then read-back via `read_fan_boost_status()`; mismatch prints a diagnostic and returns `False` (`openomen:157`–`171`)
7. Return `True` → dispatch returns `0`; failure → `1` (`openomen:1646`)

### Read-Only Flow — dashboard (`openomen stats` / monitor mode)

1. `read_all_stats()` (`openomen:539`) prints the Fan section from `read_fan_boost_status()` + `read_fan_rpm()`
2. Power section from `read_power_profile()` (ACPI file)
3. CPU section from `read_cpu_stats()` → two `/proc/stat` snapshots 500 ms apart → per-core delta math → bar rendering, plus `read_temperature()`
4. GPU section from `read_gpu_stats()` → `nvidia-smi` first, then DRM `vendor` ID scan (`0x1002` AMD, `0x8086` Intel)
5. In `-w` mode, `monitor_stats_loop()` wraps the same call in `while True:` with ANSI clear + `time.sleep(1.5)` (`openomen:1278`)

### Self-Update Flow

1. `update_openomen(force, verify)` (`openomen:851`)
2. `get_install_path()` resolves the target, then writability of its directory decides `needs_sudo` (`openomen:829`, `openomen:855`)
3. Unless `--force`, GET the GitHub releases API, strip `CLI_`/`v` from `tag_name`, then run the *installed* binary and regex its output for `V(\d+\.\d+\.\d+)` to compare versions (`openomen:881`–`902`)
4. Download to a `NamedTemporaryFile` (`suffix='_openomen_new'`), reject a 0-byte body, `chmod 0o755` (`openomen:911`–`921`)
5. Unless `--no-verify`, execute the downloaded binary and re-parse its version (`openomen:923`–`947`)
6. Replace: `sudo rm -f` + `sudo mv`, or `os.remove` + `shutil.move` (`openomen:951`–`960`)

**State Management:**
- Hardware state lives in the kernel (sysfs); the app is stateless between runs for fan/power
- The only app-owned persistent state is the RGB JSON pair under `~/.config/openomen/rgb/`
- Monitor mode keeps nothing in memory beyond loop counters; Ctrl+C exits cleanly via `finally` restoring the cursor (`openomen:1297`–`1301`)

## Key Abstractions

**Handler-with-side-effects:**
- Purpose: One function per user-visible action; performs I/O and prints, returns `bool`
- Examples: `read_cpu_stats()`, `read_gpu_stats()`, `set_power_profile()`, `rgb_set()`
- Pattern: No return value carrying data. Reuse requires capturing stdout.
- Convention: callers convert to exit codes with `return 0 if fn(...) else 1` (`openomen:1646`, `openomen:1660`, `openomen:1680`)

**Driver-priority heuristic:**
- Purpose: Choose the *right* temperature/GPU sensor among many hwmon devices
- Examples: `CPU_DRIVER_PRIORITY = ["coretemp","k10temp","zenpower","hp-wmi","acpitz"]` and `CPU_LABEL_KEYWORDS` / `SKIP_LABEL_KEYWORDS` sets (`openomen:234`–`263`); duplicated in `run_doctor()` (`openomen:1110`–`1112`)
- Pattern: Ordered allow-list + keyword deny-list, two-pass (strict then relaxed)

**Combined profile table:**
- Purpose: Map one friendly name to both an ACPI profile and a fan mode
- Example: `COMBINED_PROFILES = {"gaming": ("performance","max"), "balanced": ("balanced","auto"), "quiet": ("cool","auto")}` (`openomen:752`)
- Pattern: Declarative dict driving `apply_combined_profile()` (`openomen:801`)

**RGB profile schema:**
- Purpose: Describe a lighting look
- Example: `DEFAULT_RGB_PROFILES` entries with `name`, `description`, `length_ms`, `change`, `brightness`, `speed`, `loop`, `zones{0..3: "#RRGGBB"}` (`openomen:1307`)
- Pattern: Seeded JSON files on disk; `zones` preferred, with a legacy `animation.full` fallback in `apply_rgb_hardware()` (`openomen:1447`–`1452`)
- Note: `length_ms`, `change`, `brightness`, `speed`, and `loop` are **stored but never applied** — only `zones` colors reach hardware

**Hardware-variant dispatch:**
- Purpose: Support three incompatible RGB interfaces with one command
- Example: `detect_rgb_hardware()` returns one of `four_zone` / `legacy` / `kernel_patch` (`openomen:1413`), consumed by `apply_rgb_hardware()` (`openomen:1427`–`1433`)
- Pattern: Capability probe, then branch on variant

## Entry Points

**Process entry:**
- Location: `openomen:1702` — `if __name__ == '__main__': sys.exit(main())`
- Triggers: `openomen` on `$PATH`; also `python3 openomen`; also the frozen binary
- Responsibilities: Hand off to `main()`, propagate its int exit code

**Command entry — `main()`:**
- Location: `openomen:1557`
- Triggers: Every invocation
- Responsibilities: Parse, print help + `return 1` when no subcommand, dispatch, map success booleans to exit codes

**Re-exec entry — sudo:**
- Location: `openomen:113`–`118`
- Triggers: Any privileged command run as a non-root user
- Responsibilities: `os.execvp` with `sys.executable` when frozen, else `argv[0]`; forwards `sys.argv[1:]`

## Architectural Constraints

- **Threading:** None. Single-threaded, fully synchronous. Blocking `time.sleep(0.5)` inside `read_cpu_stats()` (`openomen:356`) and `time.sleep(0.1)` after the fan write (`openomen:162`).
- **Global state:** All module-level constants and mutable defaults — `VERSION` (`openomen:33`), `PROFILE_PATH`/`PROFILE_CHOICES_PATH` (`openomen:748`–`749`), `COMBINED_PROFILES` (`openomen:752`), `RGB_CONFIG_DIR`/`RGB_STATE_PATH` (`openomen:1303`–`1305`), `DEFAULT_RGB_PROFILES` (`openomen:1307`). Note path constants are defined *after* functions that reference them (legal only because calls happen at runtime, e.g. `read_system_info()` uses `PROFILE_CHOICES_PATH` defined 170 lines later).
- **Process replacement:** `ensure_root()` destroys the current process. Any work done before it in the same handler is discarded — this is why `ensure_root()` must be the first statement (see `openomen:138` and `openomen:1425`).
- **Privilege acquisition is inconsistent across handlers:** `set_fan_boost_mode()` guards itself with `ensure_root()` (`openomen:138`) and `apply_rgb_hardware()` does too (`openomen:1425`), while dispatch also calls `ensure_root()` for `fan max/auto/toggle` and `power set/apply` (`openomen:1619`–`1625`, `openomen:1661`/`1664`) and for `doctor` (`openomen:1693`). The guard is therefore often redundant — but `rgb set` has no dispatch-level guard, so `rgb_set()` writes `state.json` as the invoking user, then `apply_rgb_hardware()` re-execs the whole command under sudo and runs it a **second time** as root, leaving the config file root-owned. Any new handler that can reach `ensure_root()` indirectly must expect the command body to execute twice.
- **Frozen-binary awareness:** `getattr(sys, 'frozen', False)` and `sys.executable` appear in `ensure_root()` (`openomen:116`); self-update must therefore resolve paths via `shutil.which`, not `__file__`.
- **No import graph:** everything is inlined, so there are no circular-import problems — but also no testable seams.

## Anti-Patterns

### Printing instead of returning

**What happens:** Readers such as `read_gpu_stats()` and `read_temperature()` print and return only `bool` (`openomen:422`, `openomen:220`).
**Why it's wrong:** Nothing can consume a temperature programmatically; the doctor, dashboard, and any future GUI must duplicate the logic (and the duplication is already visible between `read_temperature()` and `run_doctor()`).
**Do this instead:** Return structured data (e.g. a dict/list of readings) from an internal function, and have a thin `print_*` formatter. Mirror the existing separation that `read_fan_rpm()` already gets right — it returns a dict (`openomen:58`).

### Duplicated hardware-discovery logic

**What happens:** The hwmon driver-priority scan, label keyword filters, and temp parsing exist twice: once in `read_temperature()` (`openomen:220`–`331`) and again, near-verbatim, as `get_hwmon_driver_doctor()` + `read_hwmon_temps_doctor()` + the loop in `run_doctor()` (`openomen:1114`–`1180`).
**Why it's wrong:** The two copies already diverge (the doctor version drops the label-priority filter, so it can report a different sensor than the dashboard). Fixing sensor selection requires editing two places.
**Do this instead:** Extract one `iter_cpu_temp_sensors()` helper and call it from both, as the fan code already centralizes on `find_hwmon_path()`.

### Duplicated section rendering

**What happens:** The fan status/RPM block is written twice — `read_fan_status()` (`openomen:89`) and inline in `read_all_stats()` (`openomen:544`–`557`).
**Why it's wrong:** Output formats will drift; changing the label width requires two edits.
**Do this instead:** Have `read_all_stats()` call `read_fan_status()` under a section header.

### Unverified remote code executed as root

**What happens:** `update_openomen()` downloads a binary and runs it (`openomen:938`) with no signature or checksum, over a TLS context with verification disabled (`openomen:861`), then installs it via sudo.
**Why it's wrong:** A compromised network path or GitHub account yields root code execution.
**Do this instead:** Keep default TLS verification, publish and check a SHA-256 (or minisign signature) for the asset, and never execute the downloaded binary as a verification step — read the version from the binary or trust a signed manifest.

### Silent failure via bare `except ... pass`

**What happens:** Dozens of `except IOError: pass` / `except (IOError, ValueError): pass` blocks swallow failures, e.g. `openomen:70`, `openomen:479`, `openomen:1373`.
**Why it's wrong:** A permission or driver problem looks identical to "sensor absent"; users get "No GPU found" with no cause.
**Do this instead:** Collect the errors and surface them once (or honor a `--debug` flag), instead of discarding them.

### Magic mode constants

**What happens:** `pwm1_enable` values `"0"` (max/manual) and `"2"` (auto) are compared as string literals in five places (`openomen:94`–`98`, `openomen:150`–`153`, `openomen:190`–`193`, `openomen:546`–`551`, `openomen:726`).
**Why it's wrong:** The kernel also defines `1` = manual PWM; the code prints "Unknown" for it and cannot express it. Typos are unguarded.
**Do this instead:** Define `FAN_MODE_MAX = "0"`, `FAN_MODE_AUTO = "2"`, `FAN_MODE_MANUAL = "1"` and a `FAN_MODE_DESCRIPTIONS` dict next to `PROFILE_PATH`.

## Error Handling

**Strategy:** Fail soft and local. Handlers catch expected OS exceptions, print a human-readable message to stderr, and return `False`; dispatch converts `False` to exit code `1`. There is no exception hierarchy, no logging, and no top-level `try/except` around `main()`.

**Patterns:**
- Per-call `try/except (IOError, PermissionError)` with a specific stderr message and actionable hint (`openomen:172`–`180`, `openomen:788`–`798`, `openomen:1479`–`1490`)
- Return-value convention: `bool` out of every handler; `return 0 if fn(...) else 1` at the dispatch site (`openomen:1646`, `openomen:1652`, `openomen:1672`, `openomen:1689`, `openomen:1697`)
- Hard verification after writes: fan mode read-back (`openomen:157`–`171`) and 0-byte download rejection (`openomen:918`)
- `KeyboardInterrupt` handled in monitor loops and in `update_openomen()` (`openomen:1296`, `openomen:977`) to restore the cursor / print a clean message
- Broad `except Exception as e` only in `update_openomen()` (`openomen:983`) — appropriate there as a last resort, but it also hides programming errors
- Silent-success fallback: the dispatch chain ends with an unconditional `return 0` (`openomen:1700`). A subcommand that is registered in argparse but has no matching `elif args.command == ...` branch therefore exits **0** with no output — a failure that looks like success. Every new subcommand needs an explicit branch that returns `1` on failure.

## Cross-Cutting Concerns

**Logging:**
- No logging framework. `print()` for stdout, `print(..., file=sys.stderr)` for errors. No verbosity/debug flag.

**Validation:**
- Input validation is delegated to argparse (required positionals like `power set <profile>`, `rgb set <profile>`).
- Domain validation happens at the point of use: `set_power_profile()` checks the name against `platform_profile_choices` before writing (`openomen:779`–`789`); `apply_combined_profile()` checks against `COMBINED_PROFILES` (`openomen:804`); `rgb_set()` checks the profile file exists (`openomen:1522`).
- Hex color strings are **not** validated before slicing into `int(col[0:2], 16)`, so a malformed `zones` value raises an uncaught `ValueError` (`openomen:1472`–`1477`).

**Authentication:**
- None. Authorization is Unix euid only, acquired by sudo re-exec; `doctor` and all writers require root.

---

*Architecture analysis: 2026-10-30*
