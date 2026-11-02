# Phase 5 Research: Distribution, Resource Budget & Rebrand

**Phase:** 05 — Distribution, Resource Budget & Rebrand
**Researched:** 2026-11-02
**Method:** Direct repository inspection (`Cargo.toml`, `.github/workflows/ci.yml`, `data/`, `src/`, `tests/`, `README.md`) plus vendor documentation for Cargo Packager.
**Requirements in scope:** REL-01 … REL-08, PERF-01, PERF-02, PERF-04, PERF-05, PERF-06

> **Provenance note.** This phase was researched inline by the orchestrator because the
> runtime exposes no subagent tool, so no independent `gsd-phase-researcher` context
> produced it. Findings below are all traceable to files in this repository; anything
> that is a recommendation rather than an observation is labelled as such.

---

## Summary

The repository is further along on distribution than the roadmap implies: CI quality
gates (REL-04) already exist and are stricter than the requirement — `cargo fmt --check`,
`cargo clippy --all-targets --all-features -- -D warnings`, `xvfb-run cargo test --all`
and `cargo audit` all run on every push and PR. The D-Bus policy, systemd unit, polkit
policy and full hicolor icon set already exist under `data/`. Memory instrumentation
(`read_memory_metrics`, `BenchmarkTimer`, `log_benchmark`) already exists and is wired
into `main.rs` at cold start and warm dashboard.

What is genuinely missing is: a **packaging configuration** (no Cargo Packager/bundle
config exists anywhere), a **desktop entry** (the only `data/` artifact that is absent),
a **release workflow** (no tag trigger, no checksums), an **update action** (no code
anywhere opens the Releases page), **CPU measurement** (only RSS is instrumented), a
**tray-stage measurement point**, a **degradation test suite**, and the **rebrand**
(`README.md` is still entirely the upstream OpenOMEN page).

Three findings change the shape of the plan:

1. **`data/systemd/zenomen-hw.service` hardcodes `ExecStart=/usr/local/bin/zenomen-hw`.**
   A DEB installs to `/usr/bin/`. The unit must be corrected to `/usr/bin/zenomen-hw`,
   and the mismatch is silent — the service simply fails to start after `dpkg -i`.
2. **An AppImage cannot install the systemd unit, D-Bus policy or polkit action**, because
   those live in `/etc` / `/usr/share` and require root. REL-03 asks both packages to
   install them, so the AppImage needs an explicit, root-only installation path.
3. **The monitoring worker samples at 1 Hz unconditionally** (`src/core/worker.rs:88-91`,
   clamp 500–5000 ms) and has no notion of window visibility. PERF-02's ≤0.02 % minimized
   target is not reachable while a 1 Hz sample loop runs against hwmon/procfs, so pausing
   or stretching the cadence when hidden is a required change, not a measurement exercise.

---

## Current State Inventory

| Artifact | Exists? | Path | Notes |
|---|---|---|---|
| Cargo package metadata | ✅ | `Cargo.toml` | name `zenomen`, version `0.1.0`, `license = "GPL-3.0-only"`, repository set |
| Second binary | ✅ | `src/bin/zenomen-hw.rs` | auto-discovered as `zenomen-hw`; no explicit `[[bin]]` entries |
| CI (fmt/clippy/test/audit) | ✅ | `.github/workflows/ci.yml` | covers REL-04 already |
| Release workflow | ❌ | — | no tag trigger, no artifacts, no checksums |
| Packaging config | ❌ | — | no `Packager.toml`, no `[package.metadata.packager]`, no `openomen.spec` replacement |
| Desktop entry | ❌ | — | only `data/` file that does not exist |
| systemd unit | ⚠️ | `data/systemd/zenomen-hw.service` | exists, but `ExecStart` path is wrong for a DEB |
| D-Bus policy | ✅ | `data/dbus/com.zenomen.Hardware1.conf` | correct system bus policy shape |
| polkit policy | ✅ | `data/polkit/com.zenomen.hardware.policy` | action `com.zenomen.hardware.control`, `auth_admin_keep` |
| Icons | ✅ | `data/icons/hicolor/{16,32,48,64,128,256,512}x*/apps/zenomen.png` + `scalable/apps/zenomen.svg` | complete hicolor ladder |
| RSS instrumentation | ✅ | `src/core/benchmark.rs` | `smaps_rollup` with `/proc/self/status` fallback |
| CPU instrumentation | ❌ | — | no `/proc/self/stat` deltas anywhere |
| Stage benchmarks | ⚠️ | `src/main.rs:78,174` | `cold-start` and `warm-dashboard` only; no tray stage |
| Degradation tests | ⚠️ | `tests/mock_hardware_tests.rs` | mock scenarios exist; no per-absence matrix |
| Update action | ❌ | — | `ui/screens/settings.slint:470` About block shows version only |
| Rebrand | ❌ | `README.md` | still OpenOMEN title, badges, Discord invite, wiki links |
| Python CLI removal | ❌ | `openomen`, `openomen.spec`, `how-to-release.txt` | 1,703-line CLI still present |

---

## Findings

### F1 — CI already satisfies REL-04; the release job is what is missing

`.github/workflows/ci.yml` installs the stable toolchain with `clippy` + `rustfmt`,
installs `libxkbcommon-dev`, `libfontconfig1-dev`, `libgl1-mesa-dev`, `xvfb`, installs
`cargo-audit` via `taiki-e/install-action`, then runs format check, clippy with
`-D warnings`, `xvfb-run cargo test --all`, and `cargo audit`. Permissions are already
least-privilege (`contents: read`) and concurrency cancels superseded runs.

Consequence for planning: REL-04 needs **no new work beyond a package-build smoke
check**; the release workflow (REL-05) must add `contents: write` and must not duplicate
the quality gates — it should reuse them by depending on the same steps or by running
them before packaging.

### F2 — No packaging configuration exists; Cargo Packager is the intended tool

`PROJECT.md` names Cargo Packager for DEB/AppImage, and nothing in the tree implements it.
Cargo Packager configuration is `[package.metadata.packager]` in `Cargo.toml` (or a
standalone `Packager.toml`), with the keys this phase needs:

- `productName`, `identifier`, `description`, `longDescription`, `category = "Utility"`
- `licenseFile = "LICENSE"`, `icons = ["data/icons/..."]`, `resources`
- `binaries` (both `zenomen` and `zenomen-hw`), `outDir`
- `deb.depends`, `deb.files` (map of source → path inside the `.deb`), `deb.desktopTemplate`
- `beforePackagingCommand` (build both binaries in release mode)

**Recommended layout for the DEB** (`deb.files` mappings):

| Source | Installed path |
|---|---|
| `data/systemd/zenomen-hw.service` | `/usr/lib/systemd/system/zenomen-hw.service` |
| `data/dbus/com.zenomen.Hardware1.conf` | `/usr/share/dbus-1/system.d/com.zenomen.Hardware1.conf` |
| `data/polkit/com.zenomen.hardware.policy` | `/usr/share/polkit-1/actions/com.zenomen.hardware.policy` |
| `data/icons/hicolor/*/apps/zenomen.png` | `/usr/share/icons/hicolor/*/apps/zenomen.png` |
| `data/applications/zenomen.desktop` | `/usr/share/applications/zenomen.desktop` |
| `README.md`, `LICENSE` | `/usr/share/doc/zenomen/` |

Binaries land in `/usr/bin/zenomen` and `/usr/bin/zenomen-hw`. `deb.depends` should stay
minimal — the actual runtime dependencies are `libxkbcommon0`, `libfontconfig1`, `libgl1`
and `libdbus-1-3` (winit + femtovg + zbus), not the `-dev` packages CI installs.

### F3 — AppImage system integration needs an explicit root path

An AppImage is a single executable that mounts itself; `linuxdeploy` (driven by Cargo
Packager's `appimage` section) bundles the desktop entry and icons into the AppDir but
installs nothing into the host filesystem. Without extra work the AppImage therefore
fails REL-03's "both install the desktop entry, systemd unit, D-Bus policy and polkit
rules".

**Recommendation:** embed the three system files in the GUI binary with `include_str!`
and expose `zenomen --install-system-integration` / `--uninstall-system-integration`.
The command must refuse to run as non-root with a clear message, write only the three
fixed paths, and run `systemctl daemon-reload` + `systemctl enable --now zenomen-hw`.
This keeps the AppImage self-contained, avoids shipping a shell script that must locate
itself, and gives the DEB post-install behaviour parity without `postinst` scripts.
`zenomen.desktop` is still installed by `linuxdeploy` for AppImage desktop integration.

### F4 — `data/systemd/zenomen-hw.service` will not start after a DEB install

`ExecStart=/usr/local/bin/zenomen-hw` (line 9) is correct for a manual
`how-to-release.txt`-style install and wrong for every package manager. It must become
`/usr/bin/zenomen-hw`. The unit's hardening block (`ProtectSystem=strict`,
`ReadWritePaths=/sys/devices/platform/hp-wmi /sys/firmware/acpi`, `NoNewPrivileges=false`)
is otherwise coherent with the sysfs-only write surface Phase 3 delivered and should not
be loosened.

### F5 — There is no update action at all

`grep` across `src/` and `ui/` finds no reference to the GitHub Releases page, and
`ui/screens/settings.slint:470-493` has an About block containing only an app-version
row. REL-06 wants a "Check for updates" action that *opens* the Releases page and
downloads nothing — the deliberate opposite of the upstream CLI's self-updater.

**Recommendation:** add one callback `on_open_releases_page` in `src/app.rs` next to the
existing callbacks (`install_callbacks`, lines 84-215), a button in the Settings About
block, and a `RELEASES_URL` constant
(`https://github.com/Omprakash-p06/zenomen/releases/latest`). Launch with
`std::process::Command::new("xdg-open").arg(RELEASES_URL)` and do **not** wait on the
child — no new crate, no network code in the application, no blocking of the UI thread.
Failure (no `xdg-open`) must degrade to logging a warning, never a panic.

### F6 — Memory instrumentation is complete, CPU instrumentation is absent

`src/core/benchmark.rs` reads `Rss`/`Pss`/`Private_Dirty` from `/proc/self/smaps_rollup`
with a `/proc/self/status` fallback, and `tests/benchmark_tests.rs` already asserts the
headless process stays under 30 MB. `BenchmarkTimer` measures durations but nothing
computes a **CPU percentage**.

**Recommendation:** add a `read_cpu_usage()` helper that samples `/proc/self/stat`
fields 14/15 (`utime`, `stime`) in clock ticks, plus `CLK_TCK` (assume 100, the Linux
default, and let a caller override), and returns a percentage of one core for a given
elapsed window. This is sampling-time only and must never run on the UI thread.

### F7 — The measurement stages exist but the tray stage does not

`main.rs` calls `log_benchmark("cold-start")` at line 78 (before the window is created)
and `log_benchmark("warm-dashboard")` at line 174 (after the first capability load).
There is **no** measurement after the window is hidden. PERF-01 explicitly names four
distinct figures — cold start < 30 MB, warm dashboard < 38 MB, minimized tray < 25 MB,
peak load ≤ 40 MB — so the tray stage must be instrumented when the window hides
(`on_close_requested` in `main.rs:88-99` returns `CloseRequestResponse::HideWindow`).

### F8 — The 1 Hz worker keeps sampling while hidden, which blocks PERF-02

`src/core/worker.rs:88-91` clamps the polling interval to 500–5000 ms and the deadline
loop (lines 234-250) resamples unconditionally. Nothing tells the worker that the window
is hidden. A dashboard-freshness cadence of 1 Hz is reasonable for < 0.1 % idle CPU, but
0.02 % minimized leaves a budget of roughly 0.2 ms per second — telemetry sampling plus
Slint event dispatch will not fit.

**Recommendation:** add `AppCommand::SetUiVisible(bool)`. When hidden, the worker stops
periodic sampling and blocks on `recv()` (waking only for explicit commands); on show it
performs an immediate sample and resumes the deadline loop. Because the window is a
hidden Slint window rather than a destroyed one, no state is lost and the first frame
after restore is already populated. This is also the correct behaviour for PERF-04 —
a hidden window has no consumer for a 120-sample ring buffer being refreshed.

### F9 — Degradation is designed in but not tested as a matrix

The project already has `Unavailable` presentation (`ui/components/unavailable-panel.slint`),
`MockScenario` variations (`src/hardware/mock.rs`), and a service client that returns a
disconnected status rather than erroring (`src/service/client.rs`, consumed at
`src/core/worker.rs:58-83`). What is missing is a test that removes *each* input in turn
and asserts no panic:

| Absence | Expected degraded state |
|---|---|
| hwmon fan directory | fan RPM `Unavailable`, fan mode write returns a typed error |
| `pwm1_enable` | fan control unavailable, temperature still shown |
| CPU temperature sensor (all candidates) | temperature `Unavailable`, rest of telemetry unaffected |
| `nvidia-smi` | dGPU card showed `Unavailable`, iGPU/CPU telemetry unaffected |
| RGB zone files | Lighting screen shows the unavailable panel |
| `zenomen-hw` not running | service status disconnected, read-only dashboard fully functional |
| polkit denies the action | action returns a denial message, no partial write |
| config file corrupt | defaults loaded plus a `ConfigCorrupted` notice (already handled) |

PERF-05 is satisfied by an integration test per row, not by inspection.

### F10 — Rebrand boundary: not every `openomen` string may be deleted

The Python CLI must go (REL-08), but legacy-compatibility strings must stay:

- `src/rgb/mod.rs`, `src/rgb/migration.rs`, `tests/rgb_tests.rs`, `src/main.rs:22-39` and
  `ui/screens/lighting.slint` reference `~/.config/openomen/rgb` to migrate existing user
  profiles. Those are **user data paths**, not branding, and deleting them would break
  RGB-05 (shipped in Phase 4).
- `README.md` must be rewritten with the ZenOmen identity, badge URLs pointed at
  `Omprakash-p06/zenomen`, the upstream OpenOMEN project credited explicitly as the
  ancestor, the GPL-3.0 notice kept, and the HP trademark disclaimer preserved verbatim
  (currently `README.md`, `## Disclaimer`).
- `logo/zenomen-logo-v3-indigo-gold*.svg` is already the ZenOmen mark; `data/icons`
  already carry the ZenOmen name and need no renaming.

REL-08's precondition — "once Rust parity is verified" — must be recorded as evidence,
not asserted. The parity checklist is Phases 1–4 requirements (FAN-*, PWR-*, MON-*,
DIAG-*, SYS-*, RGB-*) and every one of them is marked complete in `REQUIREMENTS.md`.

---

## Recommended Approach

**Plan 05-01 — Packaging & Release (tracer first).** Produce one real, installable
artifact end to end before broadening: add the desktop entry, fix the systemd unit path,
add the `[package.metadata.packager]` block, build the DEB, install it in a clean
container/VM, and confirm the app launches from the menu with the service running and
polkit authorising. Only then add the AppImage, the `--install-system-integration` path,
and the tag-triggered release workflow with checksums and the audit gate.

**Plan 05-02 — Resource budget (tracer first).** Instrument a single honest number first
— CPU percentage for the UI process over a fixed window while the dashboard is open —
prove it against the 0.1 % target, then extend to the cold/warm/tray stages, the
visibility-gated worker, the two-hour soak, the service budget, and the degradation
matrix.

**Plan 05-03 — Rebrand & cleanup (tracer first).** Start with the outward-facing identity
in one commit: README rewritten, update action wired, About block showing ZenOmen, and
the Releases page opening on click. Then remove the Python CLI, verify the repository
still builds and every legacy-migration test still passes, and record the parity
evidence that justifies the removal.

Ordering: 05-01 and 05-03 share `README.md` and the packaging docs; 05-02 is independent
of both. Waves are 05-01 (wave 1) → 05-02 and 05-03 (wave 2), with 05-03 depending on
05-01 because its README install section must document the artifacts 05-01 produces.

---

## Pitfalls

1. **`panic = "abort"` in release** means a panic in the AppImage kills the process with
   no unwinding — packaging must not rely on catching panics, and degradation paths must
   return `Result`/`Option` as they already do.
2. **`strip = true` + `lto = true`** makes release builds slow; the release workflow
   should build once per job and reuse the artifacts rather than rebuilding per format.
   Cargo Packager's `beforePackagingCommand` runs per format, so guard against double
   builds by keeping the command idempotent (`cargo build --release --bins` is cheap the
   second time but should still be a single workspace-wide invocation).
3. **`cargo audit` needs the advisory database** — network access on the runner. It
   already works in `ci.yml`; the release workflow inherits the same dependency.
4. **`deb.files` paths are inside the package root, not the host root.** Mapping to
   `/usr/lib/systemd/system/...` is correct; mapping to `etc/...` without the leading
   slash is the common mistake that silently places files under `/usr/lib/...`.
5. **A hidden Slint window still holds its GL surface.** The minimized-tray figure must
   be measured with `CloseRequestResponse::HideWindow`, not by destroying the window,
   or the number will flatter the app.
6. **`/proc/self/smaps_rollup` may require permissions** on hardened kernels; the existing
   fallback to `/proc/self/status` already covers this and must be preserved.
7. **Two-hour soak cannot run in CI.** It must be a documented, reproducible local run
   whose output is committed as evidence. Do not fabricate the number: if the soak was
   not performed, PERF-01 stays unchecked.
8. **Removing `openomen` from the tree must not remove the migration tests** — they are
   the only guard that existing user profiles survive the rebrand.

---

## Open Questions

1. **Release trigger granularity.** Tag-only (`v*`) is simplest and matches
   `how-to-release.txt`'s intent; a `workflow_dispatch` dry-run mode is convenient but
   must never publish. Recommendation: tag-only for publishing, `workflow_dispatch` that
   builds and uploads artifacts but skips the release step.
2. **AppImage GUI-only, or both binaries?** The AppImage should carry both `zenomen` and
   `zenomen-hw` (via `appimage.bins`) so `--install-system-integration` has a service
   binary to point at; the alternative — GUI-only AppImage — cannot satisfy REL-03.
3. **Does `zenomen-hw` belong in `/usr/bin` or `/usr/libexec`?** `/usr/bin` keeps parity
   with the existing unit and is what the `data/` asset already implies; switching to
   `/usr/libexec` is more conventional for daemons but changes the unit, the AppImage
   layout, and the docs. Recommendation: stay with `/usr/bin`.

---

## References

- `.github/workflows/ci.yml` — existing quality gates (REL-04)
- `Cargo.toml` — package metadata, release profile, dependency set
- `data/systemd/zenomen-hw.service`, `data/dbus/com.zenomen.Hardware1.conf`,
  `data/polkit/com.zenomen.hardware.policy` — privileged integration assets
- `src/core/benchmark.rs` — RSS instrumentation and `log_benchmark`
- `src/core/worker.rs` — sampling cadence and the missing visibility signal
- `src/main.rs` — lifecycle hooks where stages are measured
- `ui/screens/settings.slint` — About block where the update action belongs
- `tests/benchmark_tests.rs` — existing headless memory assertion
- Cargo Packager configuration reference: <https://docs.crabnebula.dev/packager/configuration/>
