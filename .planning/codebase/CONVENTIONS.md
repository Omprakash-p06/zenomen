---
last_mapped_commit: ecfd66bdc4fe2a588e06acb885b8c17802804ae4
last_mapped_at: 2026-10-30
---
# Coding Conventions

**Analysis Date:** 2026-10-30

## Naming Patterns

**Files:**
- The application file is `openomen` — extensionless, lowercase, matches the installed command and the PyInstaller `name=` in `openomen.spec`. Keep it extensionless: `Analysis(['openomen'])` depends on it.
- Docs: `README.md`, `LICENSE`, prose runbook `how-to-release.txt`

**Functions:**
- `snake_case`, verb-first, prefixed by subsystem rather than namespaced by module (there are no modules). The accepted prefixes:
  - `read_*` — non-mutating readers returning either a dict of values or `bool`: `read_fan_rpm()` `openomen:58`, `read_fan_boost_status()` `openomen:121`, `read_temperature()` `openomen:220`, `read_cpu_stats()` `openomen:333`, `read_gpu_stats()` `openomen:422`, `read_all_stats()` `openomen:539`, `read_system_info()` `openomen:578`, `read_power_profile()` `openomen:758`, `read_rgb_state()` `openomen:1369`
  - `set_*` — single-value sysfs writers returning `bool`: `set_fan_boost_mode()` `openomen:136`, `set_power_profile()` `openomen:774`
  - `apply_*` — multi-step mutators: `apply_combined_profile()` `openomen:801`, `apply_rgb_hardware()` `openomen:1423`
  - `get_*` — lookups: `get_friendly_label()` `openomen:200`, `get_amd_driver_info()` `openomen:401`, `get_install_path()` `openomen:829`
  - `rgb_*` — RGB command handlers: `rgb_status()` `openomen:1492`, `rgb_list()`, `rgb_set()`, `rgb_power()`
  - `ensure_*` — idempotent setup: `ensure_root()` `openomen:113`, `ensure_rgb_config()` `openomen:1355`
  - `run_*` — long tasks: `run_doctor()` `openomen:991`
  - `check_*` / `find_*` / `detect_*` — predicates and probes: `check_root()` `openomen:110`, `find_hwmon_path()` `openomen:40`, `detect_rgb_hardware()` `openomen:1413`
- Private/underscore-prefixed functions: **not used at all** — everything is public. Follow that (no new `_helper()` names unless you introduce a package).

**Variables:**
- `snake_case`. Hardware values are named for the file they came from: `hwmon_path`, `pwm_enable_path`, `pwm_enable_path`, `raw_label`, `temp_celsius`, `fan_rpm`, `card_dir`, `vendor_file`, `busy_file`.
- CLI-parsed names mirror the flag: `args.command`, `args.fan_command`, `args.power_command`, `args.rgb_command`, `args.watch`, `args.force`, `args.verify`.
- Counters for checklist output: `passed`, `failed`, `gpus_found`, `gpu_detected`.

**Constants:**
- `UPPER_SNAKE` at module level: `VERSION` (`openomen:33`), `APP_NAME` (`openomen:34`, currently unused), `CLEAR_SCREEN`/`HIDE_CURSOR`/`SHOW_CURSOR` (`openomen:36`–`38`), `PROFILE_CHOICES_PATH`/`PROFILE_PATH` (`openomen:748`–`749`), `COMBINED_PROFILES` (`openomen:752`), `RGB_*` paths (`openomen:1303`–`1305`), `DEFAULT_RGB_PROFILES` (`openomen:1307`).
- Exception: `CPU_DRIVER_PRIORITY`, `CPU_LABEL_KEYWORDS`, `SKIP_LABEL_KEYWORDS` are declared **inside** functions (`openomen:234`, `openomen:1110`) and are therefore duplicated. Any new sensor-selection logic should promote them to module level instead of adding a third copy.

**Types:**
- None. No type hints, annotations, `typing` imports, dataclasses, classes, or enums anywhere in the codebase. Do not introduce a type-hint style for one function — if annotations are wanted, they need a project-wide decision (and Python version pinning, which the repo lacks).

## Code Style

**Formatting:**
- No formatter config (no `black`, `ruff`, `autopep8`, `.editorconfig`). Formatting is hand-maintained and roughly PEP 8 with permissive spacing.
- Observed deviations to preserve consistency with:
  - occasional double blank lines inside functions (e.g. before `return` in `read_fan_boost_status()` `openomen:133`)
  - occasional extra spaces for visual alignment: `idle_delta  = idle2  - idle1` (`openomen:362`), `filled  = int(...)` (`openomen:383`)
  - trailing whitespace exists in a few places
- Indentation: 4 spaces, LF newlines, UTF-8 with literal Unicode in output strings (`█`, `░`, `─`, `✓`, `✗`, `°`, `—`).
- Line length is not enforced; long lines exist (e.g. the `argparse` description at `openomen:1559`, `needs_sudo` at `openomen:860`).

**Linting:**
- None configured. No `.flake8`, `ruff.toml`, `pylintrc`, `mypy.ini`, `.pre-commit-config.yaml`.
- Known lint-level issues exist and are tolerated today: unused imports `io` (`openomen:28`) and `contextlib` (`openomen:29`), unused constant `APP_NAME` (`openomen:34`), redundant `import sys` inside `ensure_root()` (`openomen:114`), f-string with no placeholders at `openomen:166`.

## Import Organization

**Order (single block at top, `openomen:19`–`30`):**
1. Stdlib only, alphabetization not enforced — actual order is `argparse, os, sys, glob, subprocess, time, json, urllib.request, urllib.error, tempfile, re, io, contextlib, shutil, ssl`
2. Then module-level constants (`VERSION`, `APP_NAME`, ANSI codes)
3. One stray **function-local** import of `sys` inside `ensure_root()` (`openomen:114`) — remove it rather than emulate it

**Path Aliases:**
- None. No package, so no `__init__.py`, no relative imports.

**Rule for new dependencies:** the project is deliberately stdlib-only (that is what makes a 64 KB self-updating binary viable). Adding any third-party import requires updating `openomen.spec` hidden imports and `how-to-release.txt`, and should be justified in the PR.

## Error Handling

**Patterns:**
- `try/except` around the specific operation, catching narrow OS exceptions: `(IOError, ValueError)` for reads (`openomen:66`), `IOError` for single reads (`openomen:131`), `PermissionError` then `IOError` for writes (`openomen:173`–`179`), `OSError` for `readlink` (`openomen:410`).
- Mixed tuples are common: `except (IOError, json.JSONDecodeError)` for profile loading (`openomen:1397`, `openomen:1409`), `except (IOError, ValueError)` for fan RPM (`openomen:69`), `except (subprocess.TimeoutExpired, FileNotFoundError)` for external tools (`openomen:680`, `openomen:1001`).
- Message format: `print(f"Error: <what failed>: {e}", file=sys.stderr)` followed by an actionable follow-up line. Examples: `openomen:174`–`175` ("Make sure you're running with sudo."), `openomen:145`–`146` ("Make sure the hp-wmi kernel module is loaded."), `openomen:534`.
- Warnings from non-fatal issues use the same shape with the `Warning:` prefix (`openomen:1367`, `openomen:1398`).
- **Never raise** for expected conditions — return `False`/`None` and let the dispatcher map it to exit code 1. The only exceptions that escape are programming errors (e.g. `ValueError` from `int(col[0:2], 16)` on a malformed RGB profile, `openomen:1474`).
- Bare `except IOError: pass` is common for optional sensors (`openomen:70`, `openomen:219`, `openomen:479`). Prefer keeping the loop resilient, but when adding a new one consider capturing the first error and surfacing it if nothing was found.
- `KeyboardInterrupt` is caught and converted to a clean message in `monitor_stats_loop()` (`openomen:1288`), `read_cpu_stats()` (`openomen:392`), `read_gpu_stats()` (`openomen:452`), and `update_openomen()` (`openomen:984`).
- Only one broad catch exists: `except Exception as e` in `update_openomen()` (`openomen:987`). Do not spread this pattern.

## Logging

**Framework:** None — `print()` only.

**Patterns:**
- Success/normal output → stdout, aligned label/value with `:` padding: `f"Fan Status:       {desc}"` (`openomen:95`), `f"  Temperature:    {temp}°C"` (`openomen:440`), `f"CPU:              {cpu_name}"` (`openomen:341`).
- Errors → `file=sys.stderr`.
- Sections in multi-part output → `─── Name ─────` headers with U+2500 (e.g. `openomen:543`, `openomen:999`, `openomen:1051`).
- Checklists (doctor) → `[✓]` / `[✗]` prefixes with a trailing summary line (`openomen:1276`).
- Bars → 20-cell `█`/`░` with `{usage:5.1f}%` (`openomen:381`–`386`).
- No log levels, no timestamps, no file output. Do not add a logging dependency for a single message.

## Comments

**When to Comment:**
- Section banners separate subsystems: `# ====...` blocks (e.g. the RGB banner `openomen:1293`–`1301`), and `# ACPI PROFILES!!` (`openomen:747`).
- Inline comments explain *why* a hardware workaround exists, e.g. `# Small delay for sanity check!!` (`openomen:162`), `# Verify the write actually stuck by reading back the value` (`openomen:160`), `# PyInstaller sets sys.executable to the actual binary path` (`openomen:115`), `# Intel per-core temps` in the driver list (`openomen:235`).
- Casual tone with `!!` and lowercase fragments is established; exact tone matters less than the "why" content.

**Docstrings:**
- Every top-level function has a one-line docstring; a third use multi-line. Style: descriptive sentence fragment, often with a leading space, e.g. `""" Dynamically resolve the hwmon path for hp-wmi """` (`openomen:41`), `""" this just display current fan status and RPM information """` (`openomen:90`), `""" reads it from pwm1_enable file earlier """` (`openomen:122`).
- No parameter/return sections, no `Args:`/`Returns:` blocks, no type info. New functions should carry a one-line docstring in the same style.
- Module docstring (`openomen:2`–`17`) holds the copyright, trademark disclaimer, warranty disclaimer, and GPL notice — preserve it verbatim.

## Function Design

**Size:** Highly variable and uncapped — `run_doctor()` is ~285 lines (`openomen:991`–`1276`), `read_system_info()` ~168 (`openomen:578`–`746`), while `check_root()` is 2 lines. Small focused helpers (`get_friendly_label()`, `find_hwmon_path()`) sit next to monoliths. When extending a long function, consider extracting rather than appending (see `CONCERNS.md`).

**Parameters:**
- 0–2 positional params, no keyword-only args, no defaults except `update_openomen(force=False, verify=True)` (`openomen:855`) and `monitor_stats_loop(interval=1.5)` (`openomen:1278`).
- Nested `def`s are used as closures for hardware scanning (`get_hwmon_driver()`, `read_hwmon_temps()` inside `read_temperature()` at `openomen:266`–`297`; `get_hwmon_driver_doctor()`/`read_hwmon_temps_doctor()` inside `run_doctor()`).

**Return Values:**
- Readers return `dict` when multiple values belong together (`read_fan_rpm()`), otherwise `bool` for "did I print something".
- Writers return `bool`; `None` is used for "not found" (`find_hwmon_path()`, `read_fan_boost_status()`, `read_power_profile()`).
- Dispatch maps to exit codes: `return 0 if fn(...) else 1` (`openomen:1646`, `openomen:1672`, `openomen:1689`, `openomen:1697`).

**Side effects:**
- Printing is the contract. Handlers also `time.sleep()` (`openomen:162`, `openomen:356`), re-exec the process (`openomen:119`), and spawn subprocesses — assume any `read_*`/`set_*` call may do all three.

## Module Design

**Exports:** No `__all__`, no package. The "public API" is the CLI surface plus whatever `main()` dispatches to.

**Barrel Files:** Not applicable.

**Mutable module-level state:** Only path constants and the profile dicts; nothing is mutated at runtime except through `write_rgb_state()` (file, not memory). Avoid adding mutable globals — each invocation must remain independent.

**Ordering dependency (important):** constants are declared *after* functions that reference them (`PROFILE_CHOICES_PATH` at `openomen:748` is used by `read_system_info()` at `openomen:578`). This works because lookups are lazy. If you restructure the file into modules, move these to the top of their module — and if you add a new constant used by a function defined above it, put it near the existing path-constant cluster so the pattern stays readable.

## CLI Conventions

- Subcommand tree built with nested `add_subparsers(dest=...)`: `command` → `fan_command` / `power_command` / `rgb_command` (`openomen:1567`–`1616`).
- Every subcommand gets a `help=` string (short, imperative) and long commands get an `epilog` (`openomen:1562`).
- A parent command with no subcommand prints its own help and `return 1` (`openomen:1642`–`1644`, `openomen:1670`–`1672`).
- `-w`/`--watch` is the established flag name for live repetition (`openomen:1574`, `openomen:1576`).
- `-v`/`--version` is registered on the root parser only (`openomen:1618`); the version string comes from `VERSION`, but the description hardcodes `V0.4.2` separately at `openomen:1559` — when bumping, update both.
- Exit codes: `0` success, `1` failure or missing subcommand. No other codes are used.

---

*Convention analysis: 2026-10-30*
