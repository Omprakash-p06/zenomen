---
last_mapped_commit: ecfd66bdc4fe2a588e06acb885b8c17802804ae4
last_mapped_at: 2026-10-30
---
# Codebase Structure

**Analysis Date:** 2026-10-30

## Directory Layout

```text
zenomen/                     # repo root = project root
├── openomen                 # ENTIRE application: 1703-line extensionless Python 3 script
├── openomen.spec            # PyInstaller build spec (onefile, console, upx)
├── how-to-release.txt       # Manual release runbook (venv → pip install pyinstaller → build → upload)
├── README.md                # User-facing docs: supported devices, install, update, disclaimer
├── LICENSE                  # GNU GPL v3
└── .planning/               # GSD planning artifacts (this codebase map)
    └── codebase/
```

There are no source subdirectories, no package, no `src/`, no `tests/`, no `docs/`. Six tracked files total.

## Directory Purposes

**Repository root:**
- Purpose: Everything. The application is a single file at the top level because PyInstaller is pointed at it directly (`openomen.spec` → `Analysis(['openomen'], ...)`).
- Contains: the script, the build spec, the release runbook, README, license
- Key files: `openomen`, `openomen.spec`, `how-to-release.txt`, `README.md`

**`.planning/codebase/`:**
- Purpose: GSD codebase map consumed by `/gsd:plan-phase` and `/gsd:execute-phase`
- Contains: these seven analysis documents
- Key files: `STACK.md`, `ARCHITECTURE.md`, `STRUCTURE.md`, `CONVENTIONS.md`, `TESTING.md`, `INTEGRATIONS.md`, `CONCERNS.md`

**Generated at runtime (git-ignored by convention only — there is no `.gitignore`):**
- `~/.config/openomen/rgb/profiles/*.json` — user RGB profiles, seeded from `DEFAULT_RGB_PROFILES` (`openomen:1355`)
- `~/.config/openomen/rgb/state.json` — `{"enabled": bool, "active_profile": str}` (`openomen:1305`)
- `build/`, `dist/` — PyInstaller output, created in-tree by `how-to-release.txt` (not ignored)

## Key File Locations

**Entry Points:**
- `openomen:1702`: `if __name__ == '__main__': sys.exit(main())` — the only entry point
- `openomen:1557`: `main()` — argparse tree + dispatch chain
- `openomen:113`: `ensure_root()` — secondary entry via `os.execvp('sudo', ...)`

**Configuration:**
- `openomen.spec`: PyInstaller `Analysis`/`PYZ`/`EXE` (`name='openomen'`, `console=True`, `upx=True`)
- `how-to-release.txt`: build/install instructions (only build documentation that exists)
- `openomen:33`: `VERSION = "0.4.2"` — the single version constant (also hardcoded in the argparse description at `openomen:1559`)
- `openomen:748`–`749`: `PROFILE_CHOICES_PATH`, `PROFILE_PATH` (ACPI sysfs)
- `openomen:752`: `COMBINED_PROFILES`
- `openomen:1303`–`1305`: `RGB_CONFIG_DIR`, `RGB_PROFILES_DIR`, `RGB_STATE_PATH`
- `openomen:1307`: `DEFAULT_RGB_PROFILES`

**Core Logic (by line range in `openomen`):**
- `openomen:40`–`56`: `find_hwmon_path()` — hp-wmi hwmon discovery
- `openomen:58`–`107`: fan RPM + fan status readers
- `openomen:110`–`119`: root checks / sudo re-exec
- `openomen:121`–`198`: fan mode read/write/toggle with read-back verification
- `openomen:200`–`218`: `get_friendly_label()` — hwmon label → human name map
- `openomen:220`–`331`: `read_temperature()` — two-pass driver/label prioritization
- `openomen:333`–`399`: `read_cpu_stats()` — `/proc/stat` delta + per-core bars
- `openomen:401`–`420`: `get_amd_driver_info()`
- `openomen:422`–`537`: `read_gpu_stats()` — NVIDIA → AMD → Intel
- `openomen:539`–`576`: `read_all_stats()` — dashboard composition
- `openomen:578`–`746`: `read_system_info()` — bug-report diagnostics dump
- `openomen:758`–`827`: ACPI power profile read/write + combined profiles
- `openomen:829`–`849`: `get_install_path()`
- `openomen:851`–`989`: `update_openomen()` — GitHub API, download, verify, replace
- `openomen:991`–`1276`: `run_doctor()` — pass/fail compatibility checklist
- `openomen:1278`–`1301`: `monitor_stats_loop()` — ANSI watch loop
- `openomen:1355`–`1411`: RGB config/state helpers
- `openomen:1413`–`1555`: RGB hardware detection, application, and command handlers

**Testing:**
- None. No `tests/`, no `test_*.py`, no `*_test.py`, no conftest, no fixtures. See `TESTING.md`.

## Naming Conventions

**Files:**
- Application file is **extensionless and lowercase**: `openomen` — deliberately, so the shipped executable and the source share one name and `openomen.spec` can reference it directly
- Docs are `README.md` + `LICENSE` (conventional), build spec is `<app>.spec`, release doc is kebab-case prose `how-to-release.txt`
- Planning docs are `UPPER_SNAKE.md`

**Directories:**
- Flat root; no nesting. If directories are ever added, follow the lowercase convention already implied by `~/.config/openomen/rgb/profiles/`

**Functions (in `openomen`):**
- `snake_case`, verb-first, with a subsystem prefix that acts as a poor-man's namespace:
  - `read_*` — read-only hardware/stat readers (`read_fan_rpm`, `read_cpu_stats`, `read_gpu_stats`, `read_power_profile`)
  - `set_*` — mutating writers returning `bool` (`set_fan_boost_mode`, `set_power_profile`)
  - `apply_*` — multi-step mutators (`apply_combined_profile`, `apply_rgb_hardware`)
  - `get_*` — pure-ish lookups (`get_friendly_label`, `get_amd_driver_info`, `get_install_path`)
  - `rgb_*` — RGB command handlers (`rgb_status`, `rgb_list`, `rgb_set`, `rgb_power`)
  - `ensure_*` — idempotent setup (`ensure_root`, `ensure_rgb_config`)
  - `run_*` — long task entry points (`run_doctor`)
  - `check_*` / `find_*` / `detect_*` — predicates and probes

**Variables:**
- `snake_case`; local hardware values are short and descriptive (`hwmon_path`, `pwm_enable_path`, `raw_label`, `temp_celsius`, `gpus_found`)
- Constants are `UPPER_SNAKE` at module level (`VERSION`, `CLEAR_SCREEN`, `CPU_DRIVER_PRIORITY` — the latter is *local* to both `read_temperature()` and `run_doctor()`, duplicated rather than module-level)
- Args/flag names mirror CLI: `args.command`, `args.fan_command`, `args.power_command`, `args.rgb_command`, `args.watch`, `args.force`, `args.verify`

**Types:**
- No type hints anywhere. No classes, no dataclasses, no TypedDicts, no enums. Plain `dict`/`list`/`str`/`bool`.

## Where to Add New Code

**New subcommand (the common case):**
- 1. Register it inside `main()` in the argparse block, `openomen:1557`–`1621` (`subparsers.add_parser(...)` plus a nested `add_subparsers` if it has verbs). Keep the order aligned with the `epilog` command list at `openomen:1562`.
- 2. Add the dispatch branch in the `if/elif args.command` chain, `openomen:1626`–`1698`. Always end with an explicit `return 0 if handler(...) else 1`; returning `None` silently yields exit code 0.
- 3. Call `ensure_root()` first inside the handler if the command writes to sysfs — never after other work, because `ensure_root()` re-execs the process (`openomen:113`).
- 4. Mention the command in the `README.md` feature list and the `--help` epilog.

**New hardware reading function:**
- Add a module-level `read_<thing>()` next to the closest existing reader (fan ~`openomen:58`, thermal ~`openomen:220`, GPU ~`openomen:422`, power ~`openomen:758`).
- Return data structures where possible (follow `read_fan_rpm()` returning a dict) and do the printing in a separate formatter; do not repeat the "print and return bool" pattern of `read_gpu_stats()`.
- Guard every `open()` with `(IOError, PermissionError, ValueError)` and route messages to `stderr` with an actionable hint.
- If it touches hwmon, route discovery through `find_hwmon_path()` for hp-wmi and through one shared driver-priority helper for thermal (do not add a third copy of `CPU_DRIVER_PRIORITY`).

**New mutating operation:**
- Define the sysfs path constant next to `PROFILE_PATH` (`openomen:748`) rather than inline, and use named mode constants instead of string literals like `"0"`/`"2"`.
- Validate the requested value against what the hardware advertises (mirror `set_power_profile()` reading `platform_profile_choices`, `openomen:776`–`789`).
- Add a read-back verification step after the write, as `set_fan_boost_mode()` does (`openomen:157`–`171`).

**New RGB profile behavior:**
- Profile fields are defined in `DEFAULT_RGB_PROFILES` (`openomen:1307`) and consumed in `apply_rgb_hardware()` (`openomen:1423`). Fields currently stored but unused: `length_ms`, `change`, `brightness`, `speed`, `loop`. Implementing animations means honoring these here, and `monitor_stats_loop()` is the existing example of a timed loop to model on.
- New hardware variant: extend `detect_rgb_hardware()` (`openomen:1413`) **and** the parallel detection branch at the top of `apply_rgb_hardware()` (`openomen:1427`–`1433`) — they must stay in sync.

**Utilities:**
- There is no `utils` module. Put shared helpers near their first caller and above their use; note the file already relies on runtime ordering (path constants defined after the functions that read them, e.g. `PROFILE_PATH` at `openomen:748` used by `read_system_info()` at `openomen:578`).
- If the file keeps growing, the natural first split is by subsystem: `hwmon.py`, `gpu.py`, `power.py`, `rgb.py`, `update.py`, with `openomen` becoming thin `main()`. Do not split casually — `openomen.spec` points at a single script and `ensure_root()` assumes one executable.

**Tests (when introduced):**
- Put them in a new `tests/` directory at the repo root (they do not exist yet) and add a `requirements-dev.txt` or `pyproject.toml`, plus a `.gitignore` for `.venv/`, `build/`, `dist/`, `__pycache__/`.

## Special Directories

**`build/` and `dist/` (PyInstaller output):**
- Purpose: Frozen binary artifacts; `dist/openomen` is the release upload
- Generated: Yes
- Committed: No — but there is **no `.gitignore`**, so they will show up in `git status` if produced in-tree

**`.venv/` (local build env per `how-to-release.txt`):**
- Purpose: Isolated pyinstaller install
- Generated: Yes
- Committed: No — same missing-`.gitignore` caveat

**`~/.config/openomen/rgb/` (outside the repo):**
- Purpose: Per-user RGB profiles and active state
- Generated: Yes, on first `rgb` command (`ensure_rgb_config()`, `openomen:1355`)
- Committed: N/A (user home)

---

*Structure analysis: 2026-10-30*
