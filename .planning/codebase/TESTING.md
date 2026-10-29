---
last_mapped_commit: ecfd66bdc4fe2a588e06acb885b8c17802804ae4
last_mapped_at: 2026-10-30
---
# Testing Patterns

**Analysis Date:** 2026-10-30

## Test Framework

**Runner:**
- **None.** There is no test framework, no test runner, no test configuration, and no test files anywhere in the repository.

**Assertion Library:**
- Not applicable.

**Run Commands:**

```bash

# There is no test command. Nothing to run.

# The only executable verification that exists today is manual:

openomen doctor        # hardware compatibility checklist (requires sudo)
openomen info          # diagnostic dump intended for bug reports
openomen --version     # smoke test that the binary runs
```

## Test File Organization

**Location:**
- No `tests/` directory, no co-located test files. The tracked tree is: `openomen`, `openomen.spec`, `how-to-release.txt`, `README.md`, `LICENSE` (plus `.planning/`).

**Naming:**
- Not established. If tests are added, use `tests/test_<subsystem>.py` (pytest discovery default) since the flat root already contains the app file with no extension, which would otherwise confuse collectors.

**Structure:**

```text
(no test structure exists)
```

## Test Structure

**Suite Organization:**

```python

# None present. The closest thing to a test is the manual checklist in run_doctor():

#   openomen:999   ─── Kernel & Drivers ───

#   openomen:1051  ─── Fan Control ───

#   openomen:1100  ─── Thermal Sensors ───

#   openomen:1175  ─── Power Profiles ───

#   openomen:1205  ─── GPU Stats ───

#   openomen:1240  ─── Battery ───

#   openomen:1267  ─── RGB Keyboard ───

#   openomen:1276  Doctor complete: {passed} passed, {failed} failed.

```

**Patterns:**
- `run_doctor()` (`openomen:991`–`1276`) is a manual, environment-dependent acceptance test: it probes hp-wmi, fan write permission, fan RPM readability, CPU thermal sensors, ACPI profiles, GPU tooling, battery charge threshold, and RGB interfaces, incrementing `passed`/`failed` counters.
- Every check increments a counter; a missing fan hwmon device deliberately adds `failed += 4` (`openomen:1097`) to weight the fan block.
- There is no assertion library and no non-zero exit code on doctor failures — `run_doctor()` always lets `main()` return `0` (`openomen:1695`), so **CI-style pass/fail is not expressible today**.

## Mocking

**Framework:** None.

**Patterns:**

```python

# The codebase's testability seams, and what a future test would need to patch:

# 1. All hardware paths are string literals inside functions  -> patch `open` / use tmp_path + monkeypatch of module constants

# 2. Path constants ARE module-level for power/RGB             -> monkeypatch PROFILE_PATH (openomen:749), RGB_CONFIG_DIR (openomen:1303)

# 3. External tools go through subprocess.run                  -> patch subprocess.run (nvidia-smi openomen:433, lsmod openomen:1016)

# 4. Environment is read via os.geteuid / os.access            -> patch to simulate root and writability (openomen:111, openomen:860)

# 5. Process replacement via os.execvp                         -> MUST be patched or monkeypatched, else the test process is replaced by sudo

# 6. Network update path uses urllib.request.urlopen           -> patch to avoid hitting api.github.com (openomen:881)

```

**What to Mock:**
- `os.execvp` (in `ensure_root()`, `openomen:119`) — non-negotiable; unpatched tests will exec sudo
- `subprocess.run` (all external commands: `nvidia-smi`, `lsmod`, `sudo`, downloaded-binary verification at `openomen:938`)
- `urllib.request.urlopen` and `urllib.request.Request` (update flows)
- `os.geteuid`, `os.access`, `shutil.which` (privilege and install-path decisions)
- `time.sleep` (`openomen:162`, `openomen:356`, monitor loop) to keep tests fast
- Module constants for sysfs paths: `PROFILE_PATH`, `PROFILE_CHOICES_PATH`, `RGB_CONFIG_DIR`/`RGB_PROFILES_DIR`/`RGB_STATE_PATH`

**What NOT to Mock:**
- `find_hwmon_path()` / `read_fan_rpm()` / `read_temperature()` internals — prefer pointing them at a fake `tmp_path` tree via monkeypatched `glob`/`open`, so the driver-priority logic itself is exercised
- `json` round-tripping for RGB profiles — use real files in `tmp_path` (cheap, no I/O hazards)
- The argparse tree — build it through `main()` with patched `sys.argv` so subcommand wiring is covered

**Loading the module under test:** the file has no `.py` extension, so `import openomen` will not work directly. Use `importlib.util.spec_from_loader` / `SourceFileLoader`, or have the test suite install the file as `openomen.py` via a fixture or `conftest.py` symlink. This is the single biggest practical obstacle to adding tests and is worth solving first.

## Fixtures and Factories

**Test Data:**
- None exist. A test suite would need synthetic sysfs trees. Example of the shape required by the readers:

```text
tmp/sys/class/hwmon/hwmon0/name          -> "k10temp\n"
tmp/sys/class/hwmon/hwmon0/temp1_input   -> "45000\n"
tmp/sys/class/hwmon/hwmon0/temp1_label   -> "Tctl\n"
tmp/sys/devices/platform/hp-wmi/hwmon/hwmon3/pwm1_enable -> "2\n"
tmp/sys/devices/platform/hp-wmi/hwmon/hwmon3/fan1_input  -> "2400\n"
tmp/sys/firmware/acpi/platform_profile          -> "balanced\n"
tmp/sys/firmware/acpi/platform_profile_choices  -> "low-power balanced performance\n"
```

- Note: readers hardcode absolute `/sys` paths, so fixtures require monkeypatching `glob.glob` and/or `builtins.open` rather than just setting a root prefix. Introducing a module-level `SYSFS_ROOT = "/sys"` (and composing every path from it) would make the whole surface testable with one monkeypatch — the highest-leverage refactor for testability.

**Location:**
- N/A

## Coverage

**Requirements:** None enforced. No coverage tooling, no thresholds, no CI gate.

**View Coverage:**

```bash

# No coverage configuration exists. If added:

# python -m pytest --cov=. tests/

```

## Test Types

**Unit Tests:**
- None. The natural unit-test targets, if introduced:
  - `get_friendly_label()` (`openomen:200`) — pure dict lookup, zero I/O
  - `read_cpu_stats()` usage math (`openomen:365`–`369`) — pure arithmetic on parsed `/proc/stat` values
  - `get_install_path()` (`openomen:829`) — pure path-selection logic given `shutil.which`/`os.access`
  - `get_amd_driver_info()` (`openomen:401`) — `readlink` basename vs `/proc/modules` fallback
  - `apply_rgb_hardware()` color normalization (`openomen:1443`–`1452`) — hex parsing and `zones` vs `animation.full` fallback
  - `detect_rgb_hardware()` (`openomen:1413`) — which of the three interfaces is reported

**Integration Tests:**
- None. Best candidates are the readers that touch several files at once: `read_temperature()` driver prioritization (`openomen:220`, multi-hwmon fake tree), `read_gpu_stats()` vendor dispatch (`openomen:460`–`530`, fake DRM tree + patched `nvidia-smi`), and `update_openomen()` (`openomen:851`, patched `urlopen` + `tmp_path` install dir + patched `subprocess.run`).

**E2E Tests:**
- Not used, and not realistically available: writes go to real `/sys` hardware. `run_doctor()` is the substitute and is manual by design.

## Common Patterns

**Manual verification workflow (current de-facto practice):**

```bash

# what a contributor is expected to do per how-to-release.txt + README

python3 -m venv .venv && source .venv/bin/activate
pip install pyinstaller
pyinstaller --onefile --name openomen openomen
sudo dist/openomen doctor     # hardware compatibility
sudo dist/openomen stats      # sanity-check readings
dist/openomen --version       # verify frozen binary reports VERSION
```

**Error Testing:**

```python

# No pattern exists. Handlers swallow IOError and return False, so a future

# test should assert on (a) the boolean return and (b) captured stderr text,

# e.g. capsys.readouterr().err containing "Error: Permission denied"

# (message produced at openomen:174).

```

**Non-determinism to control in tests:**
- `/proc/stat` deltas + `time.sleep(0.5)` in `read_cpu_stats()` (`openomen:356`)
- 0.1 s settle after the fan write before read-back (`openomen:162`)
- 1.5 s monitor interval (`openomen:1278`), yet `--watch` help text says "every 2 seconds" (`openomen:1574`) — a test asserting the interval would document the discrepancy
- Live network in `update_openomen()` and live `nvidia-smi` presence

## Missing Test Infrastructure (what to add first)

- `tests/` directory with a `conftest.py` that loads the extensionless `openomen` as a module
- `requirements-dev.txt` (or `pyproject.toml`) pinning `pytest` (and optionally `pytest-cov`)
- A `.gitignore` covering `.venv/`, `build/`, `dist/`, `__pycache__/`, `.pytest_cache/` — none exists, so test artifacts would otherwise be committed
- A CI workflow (`.github/workflows/`) running `pytest` on push — currently there is no CI at all
- Module-level `SYSFS_ROOT`-style indirection so fake hardware trees are injectable without patching `builtins.open`

---

*Testing analysis: 2026-10-30*
