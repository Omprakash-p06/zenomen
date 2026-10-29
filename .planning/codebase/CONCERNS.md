---
last_mapped_commit: ecfd66bdc4fe2a588e06acb885b8c17802804ae4
last_mapped_at: 2026-10-30
---
# Codebase Concerns

**Analysis Date:** 2026-10-30

## Tech Debt

**Single 1703-line file (`openomen`):**
- Issue: The entire application — CLI parsing, five hardware subsystems, self-update, diagnostics, and RGB — lives in one extensionless file with no modules, no classes, and no internal boundaries.
- Files: `openomen` (whole file)
- Impact: No testable seams, no importable units, and any two contributors touching different subsystems conflict on every PR. Function-level coupling is by name only.
- Fix approach: Introduce a package (`hwmon.py`, `fan.py`, `thermal.py`, `gpu.py`, `power.py`, `rgb.py`, `update.py`) with `openomen` kept as a thin `main()`. Note `openomen.spec` points at a single script (`Analysis(['openomen'])`) and `ensure_root()` assumes one executable, so migrate incrementally and update the spec + `how-to-release.txt` together.

**Duplicated hwmon discovery / thermal selection:**
- Issue: `CPU_DRIVER_PRIORITY`, the skip-keyword filter, and temp parsing exist twice — inside `read_temperature()` (`openomen:234`–`331`) and again in `run_doctor()` via `get_hwmon_driver_doctor()` / `read_hwmon_temps_doctor()` (`openomen:1110`–`1180`).
- Files: `openomen:220`–`331`, `openomen:1110`–`1180`
- Impact: The copies have already diverged — the doctor version omits the CPU-label priority filter, so `openomen doctor` can report a different sensor than `openomen cpu` on the same machine. Bug reports get contradictory thermals.
- Fix approach: One module-level `CPU_DRIVER_PRIORITY` plus a shared `iter_cpu_temp_sensors()` generator consumed by both call sites.

**Duplicated fan/report rendering:**
- Issue: The fan status + RPM block is written twice: `read_fan_status()` (`openomen:89`–`107`) and inline in `read_all_stats()` (`openomen:544`–`557`).
- Files: `openomen:89`, `openomen:544`
- Impact: Formatting drift between `openomen fan status` and `openomen stats`; two edits per label change.
- Fix approach: `read_all_stats()` should call `read_fan_status()` under its `─── Fan ───` header.

**Magic `pwm1_enable` mode literals, incomplete mode model:**
- Issue: `"0"` (max/manual) and `"2"` (auto) are compared as raw strings in five places (`openomen:94`–`98`, `openomen:150`–`153`, `openomen:190`–`193`, `openomen:546`–`551`, `openomen:726`), and value `"1"` (manual PWM) is unhandled, falling to "Unknown".
- Files: `openomen:121`–`198`, `openomen:539`–`557`, `openomen:722`–`734`
- Impact: Manual fan-curve work (a stated roadmap item in `README.md`) requires touching all five sites; a typo'd literal silently reports "Unknown" instead of failing.
- Fix approach: Module-level `FAN_MODE_MAX = "0"`, `FAN_MODE_MANUAL = "1"`, `FAN_MODE_AUTO = "2"` with a `FAN_MODE_DESCRIPTIONS` map.

**Version string duplicated:**
- Issue: `VERSION = "0.4.2"` (`openomen:33`) and the argparse description hardcoding `"... - V0.4.2 - audixdev"` (`openomen:1559`).
- Files: `openomen:33`, `openomen:1559`
- Impact: `openomen --help` and `openomen --version` can disagree after a partial bump; the self-updater parses the binary's printed version (`openomen:894`, `openomen:930`), so a mismatch corrupts update detection.
- Fix approach: Compose the description with `f"... - V{VERSION} - audixdev"`.

**Dead code and imports:**
- Issue: `io` (`openomen:28`) and `contextlib` (`openomen:29`) are imported and never used; `APP_NAME` (`openomen:34`) is never referenced; `import sys` is redundantly re-imported inside `ensure_root()` (`openomen:114`).
- Files: `openomen:19`–`34`, `openomen:114`
- Impact: Minor noise, but it hides the real dependency surface of a file that prides itself on stdlib-only clarity.
- Fix approach: Delete them.

**Stored-but-unimplemented RGB schema fields:**
- Issue: `DEFAULT_RGB_PROFILES` (`openomen:1307`) defines `length_ms`, `change`, `brightness`, `speed`, and `loop`, but `apply_rgb_hardware()` (`openomen:1423`) only ever writes `zones` colors. The `animation.full` fallback path (`openomen:1447`–`1452`) supports an animation-shaped profile the CLI cannot create.
- Files: `openomen:1307`–`1353`, `openomen:1443`–`1452`
- Impact: Users editing profiles expect animation control that silently does nothing — the profile looks accepted and the keyboard does not animate.
- Fix approach: Either implement these fields or remove them from default profiles and document that only `zones` is honored.

## Known Bugs

**`stats -w` accepts `--watch` and ignores it:**
- Symptoms: `openomen stats -w` prints one snapshot and exits, even though `--help` advertises "Update every 2 seconds (exit with Ctrl+C)".
- Files: `openomen:1580` (flag registered), `openomen:1648`–`1650` (handler ignores `args.watch`)
- Trigger: `openomen stats -w`
- Workaround: Use `openomen cpu -w`, `gpu -w`, or `fan status -w`, which do enter the monitor loop.

**`-w` always shows the full dashboard, not the requested subsystem:**
- Symptoms: `openomen cpu -w`, `openomen gpu -w`, and `openomen fan status -w` all render Fan + Power + CPU + GPU every cycle instead of just the named one.
- Files: `openomen:1629`, `openomen:1636`, `openomen:1643` (all route to `monitor_stats_loop()`), `openomen:1278`–`1294`
- Trigger: any `-w` invocation
- Workaround: None; users watch extra sections.

**Monitor interval contradicts documented behavior:**
- Symptoms: help text says "Update every 2 seconds" (`openomen:1574`), but `monitor_stats_loop(interval=1.5)` runs at 1.5 s (`openomen:1278`).
- Files: `openomen:1278`, `openomen:1574`, `openomen:1576`, `openomen:1578`, `openomen:1580`
- Trigger: any watch mode
- Workaround: None needed functionally.

**Intel iGPU frequency lookup is hardcoded to `card0`:**
- Symptoms: For a second Intel GPU (or a system where the Intel card is not `card0`), the primary path `os.path.join(card_dir, "drm/card0/gt_cur_freq_mhz")` (`openomen:508`) is wrong — it composes a path using the *current* `card_dir` plus a *fixed* `card0`. Frequency then silently prints nothing (the fallback glob may or may not hit).
- Files: `openomen:506`–`519`
- Trigger: multi-GPU system, or Intel GPU not enumerated first
- Workaround: None; frequency is simply absent for those configs.

**Malformed RGB hex crashes with a traceback:**
- Symptoms: A hand-edited profile with `"zones": {"0": "red"}` (or `"#FFF"`) raises an uncaught `ValueError` at `int(col[0:2], 16)` (`openomen:1474`), printing a Python traceback instead of an `Error:` line.
- Files: `openomen:1443`–`1477` (no validation), `openomen:1474`–`1476` (parse)
- Trigger: `openomen rgb set <profile>` with an invalid profile file, when lights are enabled
- Workaround: Delete the offending file in `~/.config/openomen/rgb/profiles/`; `list_rgb_profiles()` will warn about invalid *JSON* but not invalid colors.

**`rgb set` runs the command body twice and can leave root-owned config:**
- Symptoms: As a non-root user, `openomen rgb set gaming` writes `state.json` as the user, then `apply_rgb_hardware()` calls `ensure_root()` (`openomen:1425`) which re-execs under sudo with the original argv — so `rgb_set()` runs again as root and rewrites `state.json` root-owned. Later non-root `rgb set` calls fail to write state (permission denied → `Error: Could not save state file`).
- Files: `openomen:1520`–`1536` (no `ensure_root()` at entry), `openomen:1425`, `openomen:1379`–`1387`
- Trigger: `openomen rgb set <profile>` as a normal user
- Workaround: `sudo chown -R $USER ~/.config/openomen`

**Intel/AMD GPU enumeration may double-count a card:**
- Symptoms: `read_gpu_stats()` globs `/sys/class/drm/card[0-9]*/device` and increments `gpus_found` per entry; DRM can expose multiple card nodes or a card plus its render node, so a single GPU can be listed more than once.
- Files: `openomen:460`–`530`
- Trigger: systems with multiple DRM cards/render nodes
- Workaround: None.

## Security Considerations

**TLS verification disabled in the updater:**
- Risk: Every update request uses `ssl._create_unverified_context()` (`openomen:861`), applied to both the API call (`openomen:881`) and the binary download (`openomen:924`). An on-path attacker can supply any binary.
- Files: `openomen:861`, `openomen:881`, `openomen:924`
- Current mitigation: None.
- Recommendations: Use the default context, or `ssl.create_default_context()`. There is no reason to disable verification for github.com.

**Unverified remote binary is executed and then installed with root privileges:**
- Risk: `update_openomen()` runs the freshly downloaded file (`subprocess.run([tmp_path], ...)`, `openomen:938`) to read its version, then installs it via `sudo rm` + `sudo mv` (`openomen:962`–`963`). No checksum or signature is checked at any point. Combined with the disabled TLS above, this is remote root code execution.
- Files: `openomen:911`–`965`
- Current mitigation: Only a 0-byte-size check (`openomen:918`) and, when `--force` is *not* passed, a version-string comparison. `--no-verify` removes even that (`openomen:1605`).
- Recommendations: Publish a SHA-256 (and ideally a minisign/sigstore signature) per release; verify before executing; stop executing the downloaded file as a verification step (read the version by other means or trust a signed manifest).

**Spoofed User-Agent in the download request:**
- Risk: `headers={'User-Agent': 'Mozilla/5.0'}` (`openomen:922`) is an anti-pattern that can mask the tool from GitHub's own abuse detection and looks evasive.
- Files: `openomen:921`–`922`
- Current mitigation: None.
- Recommendations: Send an honest UA, e.g. `f"openomen/{VERSION}"`.

**Unprivileged users can influence a root-executed write:**
- Risk: `ensure_root()` re-execs under sudo and forwards `sys.argv[1:]` (`openomen:117`–`119`). RGB profile contents (user-writable JSON in `~/.config/openomen/rgb/profiles/`) are then written to `/sys` as root (`openomen:1455`–`1490`). Currently only fixed-length hex/binaries derived from those strings reach sysfs, so impact is bounded, but any future parsing of profile JSON into paths or commands would become a privilege boundary violation.
- Files: `openomen:113`–`119`, `openomen:1423`–`1490`
- Current mitigation: No `shell=True` anywhere; paths are composed from constants, not user input.
- Recommendations: Keep `subprocess` list-form only; validate profile schema strictly before root writes.

**Broad `Exception` catch hides real failures in the update path:**
- Risk: `except Exception as e` (`openomen:987`) converts programming errors (e.g. an unexpected `KeyError` in release JSON parsing) into a friendly message, making silent breakage likely across GitHub API changes.
- Files: `openomen:987`–`989`
- Current mitigation: Prints the exception text.
- Recommendations: Narrow to expected network/parse errors and let genuine bugs surface.

## Performance Bottlenecks

**Fixed 500 ms sleep on every CPU read:**
- Problem: `read_cpu_stats()` blocks 0.5 s to compute deltas (`openomen:344`–`356`); `read_all_stats()` calls it on every dashboard render, so `stats`, `cpu`, and every monitor tick pay the full cost.
- Files: `openomen:344`–`356`, `openomen:539`–`576`
- Cause: Sleep-based sampling with no reuse of prior counters.
- Improvement path: Cache the previous `/proc/stat` sample in memory (monitor mode only) and compute deltas against the last tick instead of sleeping.

**`nvidia-smi` spawned on every GPU/dashboard/docotor call with a 5 s timeout:**
- Problem: `subprocess.run(..., timeout=5)` at `openomen:433` and `openomen:628`; on systems without NVIDIA, every invocation pays a failed-exec cost, and on slow drivers a 5 s stall.
- Files: `openomen:429`–`450`, `openomen:626`–`640`
- Cause: No capability caching.
- Improvement path: Resolve `shutil.which('nvidia-smi')` once (the doctor already does this at `openomen:1208`) and skip entirely, or cache the negative result in memory for monitor mode.

**Full sysfs re-scan per monitor tick:**
- Problem: `monitor_stats_loop()` re-globs `/sys/class/hwmon`, `/sys/class/drm`, and re-reads every sensor at 1.5 s intervals (`openomen:1278`–`1294`).
- Files: `openomen:1278`, `openomen:40`, `openomen:231`, `openomen:460`
- Cause: No cached device topology.
- Improvement path: Enumerate hwmon/drm devices once at loop start and reuse the paths; only the `*_input` values need re-reading.

## Fragile Areas

**Hardware path assumptions:**
- Files: `openomen:44` (`/sys/devices/platform/hp-wmi/hwmon`), `openomen:231` (`/sys/class/hwmon`), `openomen:460`, `openomen:748`–`749`, `openomen:1303`, `openomen:1435`–`1440`, `openomen:1468`–`1489`
- Why fragile: Every path is an absolute string literal composed at call time. Model-to-model firmware differences (noted in `README.md`: "firmware varies by model") change which files exist. Nothing is centralized, so there is no single place to add a model quirk.
- Safe modification: Never hardcode a new absolute path inline; add a module-level constant alongside `PROFILE_PATH`/`RGB_CONFIG_DIR`, and always probe with `os.path.exists` before opening (the existing pattern).
- Test coverage: None.

**RGB hardware variant detection duplicated:**
- Files: `detect_rgb_hardware()` `openomen:1413`–`1421` vs the inline re-detection in `apply_rgb_hardware()` `openomen:1427`–`1433`
- Why fragile: The same three-path probe is written twice with different return conventions (`"Four-Zone Control (rgb_zones)"` vs `"four_zone"`). Adding a fourth interface requires editing both, and a mismatch means `doctor` reports support that `rgb set` cannot use.
- Safe modification: Have `apply_rgb_hardware()` consume `detect_rgb_hardware()`'s result via one mapping.
- Test coverage: None.

**Write-then-verify depends on a fixed 0.1 s delay:**
- Files: `openomen:162`–`171`
- Why fragile: `pwm1_enable` read-back after `time.sleep(0.1)` can fail spuriously on a slow EC/firmware, reporting "write failed to stick" when the write would have landed. It is also the only real correctness guard on hardware mutation.
- Safe modification: Retry the read-back a few times with backoff before declaring failure; keep the verification, don't remove it.
- Test coverage: None.

**Re-exec privilege model:**
- Files: `openomen:113`–`119`
- Why fragile: `os.execvp('sudo', ...)` replaces the process. Any state computed before it (including already-written RGB state in `rgb set`) is either lost or duplicated. If `sudo` is absent (minimal containers, some immutable distros) this raises an uncaught `FileNotFoundError` traceback. It also interacts with PyInstaller via `sys.executable`, so a packaging change can break escalation.
- Safe modification: Call `ensure_root()` as the first statement of every privileged handler; wrap the `execvp` in a `FileNotFoundError`/`OSError` handler with an actionable message ("sudo not found — run as root").
- Test coverage: None, and tests *must* patch `os.execvp`.

**Pipeless parsing of external command output:**
- Files: `openomen:433`–`450` (comma-split of `nvidia-smi` CSV), `openomen:1016`/`openomen:1040` (substring match of `lsmod`), `openomen:893`–`901` / `openomen:929`–`937` (regex `V(\d+\.\d+\.\d+)` on binary output)
- Why fragile: `nvidia-smi` CSV fields are assumed to have no commas and at least 5 parts (`openomen:437`); the version regex assumes output contains `Vx.y.z` and silently yields "Unable to detect version" otherwise, which then disables update comparison.
- Safe modification: Split with a limit (`line.split(",", 5)`), and treat a failed version parse as an explicit error rather than a soft skip.
- Test coverage: None.

## Scaling Limits

**Single-file distribution:**
- Current capacity: One 64 KB script / frozen binary; ~46 module-level functions.
- Limit: Beyond a few hundred more lines, review and conflict cost dominates; there is no way to unit-test a subsystem in isolation.
- Scaling path: Package split (see Tech Debt) with `openomen` retained as the console-script entry so the release artifact and `openomen.spec` stay valid.

**Monitor mode and terminal assumptions:**
- Current capacity: One dashboard render per 1.5 s; ANSI escapes written unconditionally (`openomen:36`–`38`, `openomen:1280`–`1282`).
- Limit: No TTY detection — redirecting `openomen stats -w` to a file emits raw `\033[2J\033[H` sequences, and the loop never terminates on its own (Ctrl+C only).
- Scaling path: Gate escapes on `sys.stdout.isatty()` and add a bounded `--iterations` option if non-interactive use is needed.

## Dependencies at Risk

**`nvidia-smi` (external, optional):**
- Risk: Not installed on AMD/Intel-only systems; the code already treats absence as normal (`openomen:450`).
- Impact: NVIDIA stats section is skipped; no crash.
- Migration plan: None needed — but `doctor` correctly flags the missing tool and suggests `nvidia-utils` (`openomen:1212`).

**`sudo` (external, assumed present):**
- Risk: Absent on some systems; `ensure_root()` does not handle the failure.
- Impact: Uncaught `FileNotFoundError` traceback on every privileged command.
- Migration plan: Catch the error and print "re-run as root" guidance; also support running from a root shell without sudo.

**GitHub Releases API (unauthenticated):**
- Risk: 60 requests/hour per IP; unauthenticated rate limiting can make `openomen update` fail with HTTP 403 on shared/NAT'd networks. Response shape changes also break parsing (`openomen:881`–`902`).
- Impact: Update command fails; no fallback path except `--force`, which skips version detection entirely.
- Migration plan: Surface the HTTP status/message, and consider `--force`-style direct download as an automatic fallback.

**Python version unpinned:**
- Risk: No `pyproject.toml`, no `requires-python`, no `.python-version`. Stdlib usage is conservative, but PyInstaller output depends on the building interpreter.
- Impact: A binary built on a different Python can behave differently; contributors cannot know the baseline.
- Migration plan: Document the build interpreter in `how-to-release.txt` and add a `pyproject.toml` with `requires-python`.

## Missing Critical Features

**No `.gitignore`:**
- Problem: `.venv/` (created by the documented release flow), `build/`, `dist/`, `__pycache__/`, and `.pytest_cache/` are all untracked-but-not-ignored, so `git add -A` can commit a multi-MB frozen binary or a virtualenv.
- Blocks: Safe routine commits; the release binary could be checked in accidentally.
- Files: repository root (no `.gitignore`)

**No CI / no automated verification:**
- Problem: No `.github/` workflows, no lint, no tests, no release automation. `VERSION` bumps, the PyInstaller build, and the release upload are manual (`how-to-release.txt`).
- Blocks: Catching regressions in sysfs parsing before release; reproducible releases.
- Files: repository root

**No dependency manifest:**
- Problem: No `requirements.txt`, `pyproject.toml`, or `setup.py`; the only build dependency (`pyinstaller`) is mentioned in prose.
- Blocks: Reproducible builds; automated environment setup.
- Files: `how-to-release.txt`

**Fan curves (roadmap item):**
- Problem: Advertised as planned in `README.md`, but there is no PWM write path (`pwm1`), no curve evaluation, and no daemon to drive it. Only the binary max/auto toggle exists.
- Blocks: The headline feature gap versus OMEN Gaming Hub.

**Daemon / persistence:**
- Problem: `README.md` describes a "driver, and daemon", but the codebase is CLI-only; fan mode set by `openomen fan max` is not restored at boot or after suspend, since state lives in the EC/kernel and nothing re-applies it.
- Blocks: Durable configuration across reboots; `length_ms`/`loop` RGB animation requires a running process that does not exist.

## Test Coverage Gaps

**Everything is untested (0 test files, 0 test frameworks, 0 CI):**
- What's not tested: All 46 functions in `openomen`.
- Files: `openomen` (entire file)
- Risk: Any edit to sensor parsing, profile application, or the updater ships unverified. The duplication noted above has already caused a real behavioral divergence (`doctor` vs `cpu` sensor choice) that a single shared test would have caught.
- Priority: High

**Highest-risk untested paths, in priority order:**
- `update_openomen()` (`openomen:851`–`989`): network + code execution + root file replacement; untestable today because `urlopen`, `subprocess.run`, and install-path resolution are not injectable. Priority: **High**
- `apply_rgb_hardware()` (`openomen:1423`–`1490`): binary profiling of hex strings reaching sysfs, three hardware variants, no validation. Priority: **High**
- `read_temperature()` (`openomen:220`–`331`): driver/label prioritization decides which number users see on their dashboard; no fixture tree exists. Priority: **High**
- `read_gpu_stats()` (`openomen:422`–`537`): vendor-ID dispatch, `nvidia-smi` CSV parsing, the `card0` frequency bug above. Priority: **Medium**
- `ensure_root()` (`openomen:113`–`119`): process replacement; must be patched in any test harness, so its behavior is entirely unexercised. Priority: **Medium**
- `read_cpu_stats()` delta math (`openomen:358`–`369`): pure arithmetic with a divide-by-zero guard, trivially unit-testable and currently not. Priority: **Medium**
- `main()` dispatch (`openomen:1557`–`1700`): exit-code mapping and the `stats -w` bug above; testable by patching `sys.argv`. Priority: **Medium**

---

*Concerns audit: 2026-10-30*
