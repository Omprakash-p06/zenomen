<!-- GSD:project-start source:PROJECT.md -->

## Project

**ZenOmen**

ZenOmen is a lightweight, cross-platform hardware control center for HP OMEN and Victus laptops. It is a Rust + Slint desktop application that replaces the Python `openomen` CLI (currently in this repository) with a graphical interface for the same hardware — monitoring CPU, GPU, RAM, temperatures and fan speeds, and managing power profiles, fan modes and keyboard RGB.

It is for Linux users who don't want to dual-boot just to control their own hardware, and it is built to feel like a quiet, premium system utility rather than a generic RGB control panel.

**Core Value:** Hardware control you can trust: every write to hardware is verified by reading the state back, and a sensor that isn't present reads "Unavailable" rather than a fabricated number. If everything else fails, this must work.

### Constraints

- **Tech stack**: Rust (edition 2024) + Slint for a compiled declarative GUI; TOML + Serde for configuration; `tracing` for structured logs; Cargo Packager for DEB/AppImage — dependencies stay deliberately small rather than pulling in a large framework
- **Performance**: ≤40 MB peak RSS for the desktop UI process and ≤0.1% idle CPU, measured over fixed windows rather than snapshots; if a separate hardware service exists it gets its own budget and is reported separately, never summed into the UI number
- **Performance method**: CPU sampling must not block the UI thread; one long-lived monitoring worker instead of a thread per sensor; bounded `VecDeque` history (120 samples ≈ 2 minutes at 1 Hz)
- **Security**: least privilege — the GUI runs as a normal user, read-only telemetry needs no root, all privileged writes go through the restricted D-Bus service, which accepts only predefined operations and never arbitrary paths or shell commands
- **Privacy**: local-first; no account, no database server, no cloud connection, no telemetry uploaded by default
- **Compatibility**: Linux first (Ubuntu-based and Arch-family validated); Windows kept architecture-ready but unadvertised until its interfaces are validated
- **Licensing**: GPL-3.0 inherited from OpenOMEN — original copyright notices preserved, modified sources marked, license shipped with distributed binaries, HP trademark disclaimer retained, no implied HP endorsement
- **Versioning**: semantic versioning; v0.1.0 is the foundation, 1.0.0 is the first stable release
- **No network dependency**: core functionality works fully offline

<!-- GSD:project-end -->

<!-- GSD:stack-start source:codebase/STACK.md -->

## Technology Stack

## Scope

- This document maps the **legacy Python implementation** currently on disk — accurate today, and the behavioural reference during the Rust rewrite
- The **target v1 stack is Rust (edition 2024) + Slint 1.17 with a D-Bus hardware service** — see `.planning/PROJECT.md` (Constraints, Key Decisions) and `.planning/research/STACK.md`
- The Python CLI is removed in Phase 5 once Rust parity is verified (`REL-08`) — do not add new Python features

## Languages

- Python 3 - Entire application, in a single file `openomen` (1703 lines, no `.py` extension so it can be shipped as a named executable)
- Bash - Not present in the repo; operational instructions only, in `how-to-release.txt`

## Runtime

- CPython 3.x (any 3.6+; no version pinned anywhere — no `pyproject.toml`, `setup.py`, `.python-version`, or `requirements.txt`)
- Linux kernel as the effective runtime dependency: reads `/proc/*` and `/sys/*` directly
- Requires Linux 6.18+ per `README.md` (ACPI `platform_profile` needs 5.15+, enforced by message text only)
- None. Zero third-party dependencies — standard library only.
- Lockfile: missing (no lockfile exists because there are no dependencies)

## Frameworks

- None. No web framework, no CLI framework beyond the standard library `argparse`.
- `argparse` with nested `add_subparsers` is the entire command framework — `openomen:1557` (`main()`)
- None. No test framework, no test files, no test runner config.
- PyInstaller - Freezes the script into a standalone Linux binary

## Key Dependencies

- `argparse` - CLI parsing and subcommand tree (`openomen:1557`)
- `subprocess` - Shelling out to `nvidia-smi`, `lsmod`, `sudo`, and to run the downloaded binary during self-update (`openomen:433`, `openomen:938`, `openomen:1016`)
- `urllib.request` / `urllib.error` - GitHub Releases API and binary download in `update_openomen()` (`openomen:851`)
- `ssl` - Used only to *disable* certificate verification: `ssl._create_unverified_context()` (`openomen:861`)
- `json` - RGB profile/state persistence (`openomen:1355`–`openomen:1411`)
- `glob` - Enumerating `/sys/class/hwmon/hwmon*`, `/sys/class/drm/card*/device`, `/sys/class/power_supply/BAT*` (`openomen:40`, `openomen:231`, `openomen:460`)
- `tempfile` / `shutil` - Download staging and atomic-ish binary replacement (`openomen:851`–`openomen:960`, `openomen:829`)
- `nvidia-smi` - NVIDIA GPU name/temp/util/VRAM/power (`openomen:429`, `openomen:628`)
- `sudo` - Privilege escalation via `os.execvp('sudo', ...)` (`openomen:113`) and file replacement during update (`openomen:955`)
- `lsmod` - Fallback for hp-wmi module detection when `/proc/modules` is unreadable (`openomen:1035`)
- `io` (`openomen:28`), `contextlib` (`openomen:29`)

## Configuration

- No environment variables read anywhere. No `.env` file, no config file for the app itself.
- All configuration is implicit in the host system: sysfs paths, `/proc` files, and CLI flags.
- User-writable state lives at `~/.config/openomen/rgb/` (`openomen:1303`–`1305`):
- `openomen.spec` - PyInstaller Analysis/EXE definition, `upx=True`, `console=True`, `strip=False`
- `how-to-release.txt` - manual release runbook (venv, pip install pyinstaller, build, upload `dist/openomen`)
- No CI config of any kind (no `.github/`, no `Makefile`, no `tox.ini`, no `.pre-commit-config.yaml`)

## Platform Requirements

- Linux with `python3` and the `venv` module
- `pyinstaller` installed into a local `.venv` (per `how-to-release.txt`)
- No `.gitignore`, so `.venv/`, `build/`, and `dist/` are not excluded from git if created in-tree
- Distribution target: single-file Linux x86-64 executable attached to GitHub Releases, installed to `/usr/local/bin/openomen` or `~/.local/bin/openomen` (`openomen:829` `get_install_path()`)
- `root` (via sudo) required for all write operations: fan mode, power profile, RGB, and `doctor`
- Hardware: HP OMEN / Victus laptops exposing the `hp-wmi` hwmon device; model list in `README.md`

<!-- GSD:stack-end -->

<!-- GSD:conventions-start source:CONVENTIONS.md -->

## Conventions

## Naming Patterns

- The application file is `openomen` — extensionless, lowercase, matches the installed command and the PyInstaller `name=` in `openomen.spec`. Keep it extensionless: `Analysis(['openomen'])` depends on it.
- Docs: `README.md`, `LICENSE`, prose runbook `how-to-release.txt`
- `snake_case`, verb-first, prefixed by subsystem rather than namespaced by module (there are no modules). The accepted prefixes:
- Private/underscore-prefixed functions: **not used at all** — everything is public. Follow that (no new `_helper()` names unless you introduce a package).
- `snake_case`. Hardware values are named for the file they came from: `hwmon_path`, `pwm_enable_path`, `pwm_enable_path`, `raw_label`, `temp_celsius`, `fan_rpm`, `card_dir`, `vendor_file`, `busy_file`.
- CLI-parsed names mirror the flag: `args.command`, `args.fan_command`, `args.power_command`, `args.rgb_command`, `args.watch`, `args.force`, `args.verify`.
- Counters for checklist output: `passed`, `failed`, `gpus_found`, `gpu_detected`.
- `UPPER_SNAKE` at module level: `VERSION` (`openomen:33`), `APP_NAME` (`openomen:34`, currently unused), `CLEAR_SCREEN`/`HIDE_CURSOR`/`SHOW_CURSOR` (`openomen:36`–`38`), `PROFILE_CHOICES_PATH`/`PROFILE_PATH` (`openomen:748`–`749`), `COMBINED_PROFILES` (`openomen:752`), `RGB_*` paths (`openomen:1303`–`1305`), `DEFAULT_RGB_PROFILES` (`openomen:1307`).
- Exception: `CPU_DRIVER_PRIORITY`, `CPU_LABEL_KEYWORDS`, `SKIP_LABEL_KEYWORDS` are declared **inside** functions (`openomen:234`, `openomen:1110`) and are therefore duplicated. Any new sensor-selection logic should promote them to module level instead of adding a third copy.
- None. No type hints, annotations, `typing` imports, dataclasses, classes, or enums anywhere in the codebase. Do not introduce a type-hint style for one function — if annotations are wanted, they need a project-wide decision (and Python version pinning, which the repo lacks).

## Code Style

- No formatter config (no `black`, `ruff`, `autopep8`, `.editorconfig`). Formatting is hand-maintained and roughly PEP 8 with permissive spacing.
- Observed deviations to preserve consistency with:
- Indentation: 4 spaces, LF newlines, UTF-8 with literal Unicode in output strings (`█`, `░`, `─`, `✓`, `✗`, `°`, `—`).
- Line length is not enforced; long lines exist (e.g. the `argparse` description at `openomen:1559`, `needs_sudo` at `openomen:860`).
- None configured. No `.flake8`, `ruff.toml`, `pylintrc`, `mypy.ini`, `.pre-commit-config.yaml`.
- Known lint-level issues exist and are tolerated today: unused imports `io` (`openomen:28`) and `contextlib` (`openomen:29`), unused constant `APP_NAME` (`openomen:34`), redundant `import sys` inside `ensure_root()` (`openomen:114`), f-string with no placeholders at `openomen:166`.

## Import Organization

- None. No package, so no `__init__.py`, no relative imports.

## Error Handling

- `try/except` around the specific operation, catching narrow OS exceptions: `(IOError, ValueError)` for reads (`openomen:66`), `IOError` for single reads (`openomen:131`), `PermissionError` then `IOError` for writes (`openomen:173`–`179`), `OSError` for `readlink` (`openomen:410`).
- Mixed tuples are common: `except (IOError, json.JSONDecodeError)` for profile loading (`openomen:1397`, `openomen:1409`), `except (IOError, ValueError)` for fan RPM (`openomen:69`), `except (subprocess.TimeoutExpired, FileNotFoundError)` for external tools (`openomen:680`, `openomen:1001`).
- Message format: `print(f"Error: <what failed>: {e}", file=sys.stderr)` followed by an actionable follow-up line. Examples: `openomen:174`–`175` ("Make sure you're running with sudo."), `openomen:145`–`146` ("Make sure the hp-wmi kernel module is loaded."), `openomen:534`.
- Warnings from non-fatal issues use the same shape with the `Warning:` prefix (`openomen:1367`, `openomen:1398`).
- **Never raise** for expected conditions — return `False`/`None` and let the dispatcher map it to exit code 1. The only exceptions that escape are programming errors (e.g. `ValueError` from `int(col[0:2], 16)` on a malformed RGB profile, `openomen:1474`).
- Bare `except IOError: pass` is common for optional sensors (`openomen:70`, `openomen:219`, `openomen:479`). Prefer keeping the loop resilient, but when adding a new one consider capturing the first error and surfacing it if nothing was found.
- `KeyboardInterrupt` is caught and converted to a clean message in `monitor_stats_loop()` (`openomen:1288`), `read_cpu_stats()` (`openomen:392`), `read_gpu_stats()` (`openomen:452`), and `update_openomen()` (`openomen:984`).
- Only one broad catch exists: `except Exception as e` in `update_openomen()` (`openomen:987`). Do not spread this pattern.

## Logging

- Success/normal output → stdout, aligned label/value with `:` padding: `f"Fan Status:       {desc}"` (`openomen:95`), `f"  Temperature:    {temp}°C"` (`openomen:440`), `f"CPU:              {cpu_name}"` (`openomen:341`).
- Errors → `file=sys.stderr`.
- Sections in multi-part output → `─── Name ─────` headers with U+2500 (e.g. `openomen:543`, `openomen:999`, `openomen:1051`).
- Checklists (doctor) → `[✓]` / `[✗]` prefixes with a trailing summary line (`openomen:1276`).
- Bars → 20-cell `█`/`░` with `{usage:5.1f}%` (`openomen:381`–`386`).
- No log levels, no timestamps, no file output. Do not add a logging dependency for a single message.

## Comments

- Section banners separate subsystems: `# ====...` blocks (e.g. the RGB banner `openomen:1293`–`1301`), and `# ACPI PROFILES!!` (`openomen:747`).
- Inline comments explain *why* a hardware workaround exists, e.g. `# Small delay for sanity check!!` (`openomen:162`), `# Verify the write actually stuck by reading back the value` (`openomen:160`), `# PyInstaller sets sys.executable to the actual binary path` (`openomen:115`), `# Intel per-core temps` in the driver list (`openomen:235`).
- Casual tone with `!!` and lowercase fragments is established; exact tone matters less than the "why" content.
- Every top-level function has a one-line docstring; a third use multi-line. Style: descriptive sentence fragment, often with a leading space, e.g. `""" Dynamically resolve the hwmon path for hp-wmi """` (`openomen:41`), `""" this just display current fan status and RPM information """` (`openomen:90`), `""" reads it from pwm1_enable file earlier """` (`openomen:122`).
- No parameter/return sections, no `Args:`/`Returns:` blocks, no type info. New functions should carry a one-line docstring in the same style.
- Module docstring (`openomen:2`–`17`) holds the copyright, trademark disclaimer, warranty disclaimer, and GPL notice — preserve it verbatim.

## Function Design

- 0–2 positional params, no keyword-only args, no defaults except `update_openomen(force=False, verify=True)` (`openomen:855`) and `monitor_stats_loop(interval=1.5)` (`openomen:1278`).
- Nested `def`s are used as closures for hardware scanning (`get_hwmon_driver()`, `read_hwmon_temps()` inside `read_temperature()` at `openomen:266`–`297`; `get_hwmon_driver_doctor()`/`read_hwmon_temps_doctor()` inside `run_doctor()`).
- Readers return `dict` when multiple values belong together (`read_fan_rpm()`), otherwise `bool` for "did I print something".
- Writers return `bool`; `None` is used for "not found" (`find_hwmon_path()`, `read_fan_boost_status()`, `read_power_profile()`).
- Dispatch maps to exit codes: `return 0 if fn(...) else 1` (`openomen:1646`, `openomen:1672`, `openomen:1689`, `openomen:1697`).
- Printing is the contract. Handlers also `time.sleep()` (`openomen:162`, `openomen:356`), re-exec the process (`openomen:119`), and spawn subprocesses — assume any `read_*`/`set_*` call may do all three.

## Module Design

## CLI Conventions

- Subcommand tree built with nested `add_subparsers(dest=...)`: `command` → `fan_command` / `power_command` / `rgb_command` (`openomen:1567`–`1616`).
- Every subcommand gets a `help=` string (short, imperative) and long commands get an `epilog` (`openomen:1562`).
- A parent command with no subcommand prints its own help and `return 1` (`openomen:1642`–`1644`, `openomen:1670`–`1672`).
- `-w`/`--watch` is the established flag name for live repetition (`openomen:1574`, `openomen:1576`).
- `-v`/`--version` is registered on the root parser only (`openomen:1618`); the version string comes from `VERSION`, but the description hardcodes `V0.4.2` separately at `openomen:1559` — when bumping, update both.
- Exit codes: `0` success, `1` failure or missing subcommand. No other codes are used.

<!-- GSD:conventions-end -->

<!-- GSD:architecture-start source:ARCHITECTURE.md -->

## Architecture

## System Overview

```text

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

- One 1703-line executable file `openomen` (extensionless, shebang `#!/usr/bin/env python3`)
- Fully flat namespace: 46 module-level functions/constants, zero classes, zero dataclasses
- Stdlib only; every hardware access is a direct `open()`/`glob()` on a hardcoded `/sys` or `/proc` path
- Functions **print** rather than return data; the boolean return is only a success flag for exit-code mapping
- Handlers are stateless: each invocation reads current hardware state fresh, no daemon, no cache
- Root is acquired by re-exec (process replacement), not by a privilege-separated helper
- Distribution is a frozen binary that can overwrite itself from GitHub

## Layers

- Purpose: Own all user input parsing and command routing
- Location: `openomen:1557`–`1700`
- Contains: The argparse construction (subparsers for `fan`, `cpu`, `gpu`, `stats`, `info`, `power`, `rgb`, `doctor`, `update`), then one `if/elif` chain on `args.command`
- Depends on: every handler function below it
- Used by: `if __name__ == '__main__'` entry (`openomen:1702`)
- Purpose: Guarantee the permissions and kernel interfaces a command needs before touching hardware
- Location: `openomen:110`–`119` (`ensure_root`), `openomen:40`–`56` (`find_hwmon_path`)
- Contains: euid check, sudo re-exec with frozen-binary awareness, hwmon discovery
- Depends on: `os`, `sys`
- Used by: `set_fan_boost_mode()`, `rgb_power()`, `apply_rgb_hardware()`, `doctor`, `power set/apply`
- Purpose: Translate sysfs/proc files into Python values and back
- Location: fan `openomen:58`–`198`; thermal `openomen:220`–`331`; power `openomen:758`–`799`; RGB writes `openomen:1423`–`1490`
- Contains: `open()` reads, guarded writes, read-back verification, driver-priority heuristics
- Depends on: the filesystem only
- Used by: presentation layer and dispatch
- Purpose: Format readings for humans
- Location: `openomen:89`–`107`, `openomen:333`–`399`, `openomen:422`–`537`, `openomen:539`–`576`, `openomen:578`–`746`, `openomen:991`–`1276`
- Contains: aligned `print()` calls, Unicode bar charts (`█`/`░`), `─── Section ───` headers, `[✓]`/`[✗]` checklist output
- Depends on: hardware access layer
- Used by: dispatch and `monitor_stats_loop()`
- Purpose: Persist user RGB choices across invocations
- Location: `openomen:1303`–`1411`
- Contains: `RGB_CONFIG_DIR`/`RGB_STATE_PATH` constants, `DEFAULT_RGB_PROFILES`, `ensure_rgb_config()`, `read_rgb_state()`, `write_rgb_state()`, `load_rgb_profile()`, `list_rgb_profiles()`
- Depends on: `json`, `os`, `glob`
- Used by: `rgb_status()`, `rgb_set()`, `rgb_power()`
- Purpose: Install/upgrade the tool and diagnose the host
- Location: `openomen:829`–`989` (update), `openomen:991`–`1276` (doctor)
- Contains: install-path resolution (`shutil.which` → `~/.local/bin` → `/usr/local/bin`), GitHub API + asset download, temp-file staging, PyInstaller-safe self-replacement
- Depends on: `urllib`, `ssl`, `tempfile`, `shutil`, `subprocess`, root
- Used by: `update` and `doctor` commands

## Data Flow

### Primary Flow — a mutating command (`openomen fan max`)

### Read-Only Flow — dashboard (`openomen stats` / monitor mode)

### Self-Update Flow

- Hardware state lives in the kernel (sysfs); the app is stateless between runs for fan/power
- The only app-owned persistent state is the RGB JSON pair under `~/.config/openomen/rgb/`
- Monitor mode keeps nothing in memory beyond loop counters; Ctrl+C exits cleanly via `finally` restoring the cursor (`openomen:1297`–`1301`)

## Key Abstractions

- Purpose: One function per user-visible action; performs I/O and prints, returns `bool`
- Examples: `read_cpu_stats()`, `read_gpu_stats()`, `set_power_profile()`, `rgb_set()`
- Pattern: No return value carrying data. Reuse requires capturing stdout.
- Convention: callers convert to exit codes with `return 0 if fn(...) else 1` (`openomen:1646`, `openomen:1660`, `openomen:1680`)
- Purpose: Choose the *right* temperature/GPU sensor among many hwmon devices
- Examples: `CPU_DRIVER_PRIORITY = ["coretemp","k10temp","zenpower","hp-wmi","acpitz"]` and `CPU_LABEL_KEYWORDS` / `SKIP_LABEL_KEYWORDS` sets (`openomen:234`–`263`); duplicated in `run_doctor()` (`openomen:1110`–`1112`)
- Pattern: Ordered allow-list + keyword deny-list, two-pass (strict then relaxed)
- Purpose: Map one friendly name to both an ACPI profile and a fan mode
- Example: `COMBINED_PROFILES = {"gaming": ("performance","max"), "balanced": ("balanced","auto"), "quiet": ("cool","auto")}` (`openomen:752`)
- Pattern: Declarative dict driving `apply_combined_profile()` (`openomen:801`)
- Purpose: Describe a lighting look
- Example: `DEFAULT_RGB_PROFILES` entries with `name`, `description`, `length_ms`, `change`, `brightness`, `speed`, `loop`, `zones{0..3: "#RRGGBB"}` (`openomen:1307`)
- Pattern: Seeded JSON files on disk; `zones` preferred, with a legacy `animation.full` fallback in `apply_rgb_hardware()` (`openomen:1447`–`1452`)
- Note: `length_ms`, `change`, `brightness`, `speed`, and `loop` are **stored but never applied** — only `zones` colors reach hardware
- Purpose: Support three incompatible RGB interfaces with one command
- Example: `detect_rgb_hardware()` returns one of `four_zone` / `legacy` / `kernel_patch` (`openomen:1413`), consumed by `apply_rgb_hardware()` (`openomen:1427`–`1433`)
- Pattern: Capability probe, then branch on variant

## Entry Points

- Location: `openomen:1702` — `if __name__ == '__main__': sys.exit(main())`
- Triggers: `openomen` on `$PATH`; also `python3 openomen`; also the frozen binary
- Responsibilities: Hand off to `main()`, propagate its int exit code
- Location: `openomen:1557`
- Triggers: Every invocation
- Responsibilities: Parse, print help + `return 1` when no subcommand, dispatch, map success booleans to exit codes
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

### Duplicated hardware-discovery logic

### Duplicated section rendering

### Unverified remote code executed as root

### Silent failure via bare `except ... pass`

### Magic mode constants

## Error Handling

- Per-call `try/except (IOError, PermissionError)` with a specific stderr message and actionable hint (`openomen:172`–`180`, `openomen:788`–`798`, `openomen:1479`–`1490`)
- Return-value convention: `bool` out of every handler; `return 0 if fn(...) else 1` at the dispatch site (`openomen:1646`, `openomen:1652`, `openomen:1672`, `openomen:1689`, `openomen:1697`)
- Hard verification after writes: fan mode read-back (`openomen:157`–`171`) and 0-byte download rejection (`openomen:918`)
- `KeyboardInterrupt` handled in monitor loops and in `update_openomen()` (`openomen:1296`, `openomen:977`) to restore the cursor / print a clean message
- Broad `except Exception as e` only in `update_openomen()` (`openomen:983`) — appropriate there as a last resort, but it also hides programming errors
- Silent-success fallback: the dispatch chain ends with an unconditional `return 0` (`openomen:1700`). A subcommand that is registered in argparse but has no matching `elif args.command == ...` branch therefore exits **0** with no output — a failure that looks like success. Every new subcommand needs an explicit branch that returns `1` on failure.

## Cross-Cutting Concerns

- No logging framework. `print()` for stdout, `print(..., file=sys.stderr)` for errors. No verbosity/debug flag.
- Input validation is delegated to argparse (required positionals like `power set <profile>`, `rgb set <profile>`).
- Domain validation happens at the point of use: `set_power_profile()` checks the name against `platform_profile_choices` before writing (`openomen:779`–`789`); `apply_combined_profile()` checks against `COMBINED_PROFILES` (`openomen:804`); `rgb_set()` checks the profile file exists (`openomen:1522`).
- Hex color strings are **not** validated before slicing into `int(col[0:2], 16)`, so a malformed `zones` value raises an uncaught `ValueError` (`openomen:1472`–`1477`).
- None. Authorization is Unix euid only, acquired by sudo re-exec; `doctor` and all writers require root.

<!-- GSD:architecture-end -->

<!-- GSD:skills-start source:skills/ -->

## Project Skills

No project skills found. Add skills to any of: `.claude/skills/`, `.agents/skills/`, `.cursor/skills/`, `.github/skills/`, or `.codex/skills/` with a `SKILL.md` index file.
<!-- GSD:skills-end -->

<!-- GSD:workflow-start source:GSD defaults -->

## GSD Workflow Enforcement

Before using Edit, Write, or other file-changing tools, start work through a GSD command so planning artifacts and execution context stay in sync.

Use these entry points:
- `/gsd-fast` for a trivial task inline, with no subagents and no PLAN.md
- `/gsd-quick` for small fixes, doc updates, and ad-hoc tasks
- `/gsd-debug` for investigation and bug fixing
- `/gsd-execute-phase` for planned phase work

Do not make direct repo edits outside a GSD workflow unless the user explicitly asks to bypass it.
<!-- GSD:workflow-end -->

<!-- GSD:profile-start -->

## Developer Profile

> Profile not yet configured. Run `/gsd-profile-user` to generate your developer profile.
> This section is managed by `generate-claude-profile` -- do not edit manually.
<!-- GSD:profile-end -->

<!-- graphicle:start (added by graphicle setup; graphicle disconnect removes it) -->
## Graphicle
Use the `graphicle` MCP server for where or how something works, for questions across files or history, and before changing unfamiliar code: `recall_project` or `query_graph` with the question, then `get_node` and `get_neighbors` for one symbol; answers cite file and lines. For an exact identifier you already know, grep is enough.
Without the server, run `graphicle context "<question>"` in a terminal.
<!-- graphicle:end -->
