---
last_mapped_commit: ecfd66bdc4fe2a588e06acb885b8c17802804ae4
last_mapped_at: 2026-10-30
---
# Technology Stack

**Analysis Date:** 2026-10-30

## Scope

- This document maps the **legacy Python implementation** currently on disk — accurate today, and the behavioural reference during the Rust rewrite
- The **target v1 stack is Rust (edition 2024) + Slint 1.17 with a D-Bus hardware service** — see `.planning/PROJECT.md` (Constraints, Key Decisions) and `.planning/research/STACK.md`
- The Python CLI is removed in Phase 5 once Rust parity is verified (`REL-08`) — do not add new Python features

## Languages

**Primary:**
- Python 3 - Entire application, in a single file `openomen` (1703 lines, no `.py` extension so it can be shipped as a named executable)

**Secondary:**
- Bash - Not present in the repo; operational instructions only, in `how-to-release.txt`

## Runtime

**Environment:**
- CPython 3.x (any 3.6+; no version pinned anywhere — no `pyproject.toml`, `setup.py`, `.python-version`, or `requirements.txt`)
- Linux kernel as the effective runtime dependency: reads `/proc/*` and `/sys/*` directly
- Requires Linux 6.18+ per `README.md` (ACPI `platform_profile` needs 5.15+, enforced by message text only)

**Package Manager:**
- None. Zero third-party dependencies — standard library only.
- Lockfile: missing (no lockfile exists because there are no dependencies)

## Frameworks

**Core:**
- None. No web framework, no CLI framework beyond the standard library `argparse`.
- `argparse` with nested `add_subparsers` is the entire command framework — `openomen:1557` (`main()`)

**Testing:**
- None. No test framework, no test files, no test runner config.

**Build/Dev:**
- PyInstaller - Freezes the script into a standalone Linux binary
  - Spec file: `openomen.spec` (`EXE(..., name='openomen', console=True, upx=True, ...)`)
  - Documented flow: `how-to-release.txt` (`pyinstaller --onefile --name openomen openomen`)
  - Output: `dist/openomen`, uploaded manually to GitHub Releases

## Key Dependencies

**Critical (all standard library):**
- `argparse` - CLI parsing and subcommand tree (`openomen:1557`)
- `subprocess` - Shelling out to `nvidia-smi`, `lsmod`, `sudo`, and to run the downloaded binary during self-update (`openomen:433`, `openomen:938`, `openomen:1016`)
- `urllib.request` / `urllib.error` - GitHub Releases API and binary download in `update_openomen()` (`openomen:851`)
- `ssl` - Used only to *disable* certificate verification: `ssl._create_unverified_context()` (`openomen:861`)
- `json` - RGB profile/state persistence (`openomen:1355`–`openomen:1411`)
- `glob` - Enumerating `/sys/class/hwmon/hwmon*`, `/sys/class/drm/card*/device`, `/sys/class/power_supply/BAT*` (`openomen:40`, `openomen:231`, `openomen:460`)
- `tempfile` / `shutil` - Download staging and atomic-ish binary replacement (`openomen:851`–`openomen:960`, `openomen:829`)

**Infrastructure (external commands invoked, not packages):**
- `nvidia-smi` - NVIDIA GPU name/temp/util/VRAM/power (`openomen:429`, `openomen:628`)
- `sudo` - Privilege escalation via `os.execvp('sudo', ...)` (`openomen:113`) and file replacement during update (`openomen:955`)
- `lsmod` - Fallback for hp-wmi module detection when `/proc/modules` is unreadable (`openomen:1035`)

**Imported but unused** (dead imports to remove when touching the header):
- `io` (`openomen:28`), `contextlib` (`openomen:29`)

## Configuration

**Environment:**
- No environment variables read anywhere. No `.env` file, no config file for the app itself.
- All configuration is implicit in the host system: sysfs paths, `/proc` files, and CLI flags.
- User-writable state lives at `~/.config/openomen/rgb/` (`openomen:1303`–`1305`):
  - `profiles/*.json` - one file per RGB profile, seeded from `DEFAULT_RGB_PROFILES` (`openomen:1307`)
  - `state.json` - `{"enabled": bool, "active_profile": str}`

**Build:**
- `openomen.spec` - PyInstaller Analysis/EXE definition, `upx=True`, `console=True`, `strip=False`
- `how-to-release.txt` - manual release runbook (venv, pip install pyinstaller, build, upload `dist/openomen`)
- No CI config of any kind (no `.github/`, no `Makefile`, no `tox.ini`, no `.pre-commit-config.yaml`)

## Platform Requirements

**Development:**
- Linux with `python3` and the `venv` module
- `pyinstaller` installed into a local `.venv` (per `how-to-release.txt`)
- No `.gitignore`, so `.venv/`, `build/`, and `dist/` are not excluded from git if created in-tree

**Production:**
- Distribution target: single-file Linux x86-64 executable attached to GitHub Releases, installed to `/usr/local/bin/openomen` or `~/.local/bin/openomen` (`openomen:829` `get_install_path()`)
- `root` (via sudo) required for all write operations: fan mode, power profile, RGB, and `doctor`
- Hardware: HP OMEN / Victus laptops exposing the `hp-wmi` hwmon device; model list in `README.md`

---

*Stack analysis: 2026-10-30*
