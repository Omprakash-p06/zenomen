# Pitfalls Research

**Domain:** Linux hardware control + telemetry GUI (HP OMEN/Victus), Rust + Slint
**Researched:** 2026-10-30
**Confidence:** HIGH for pitfalls 1–6 (observed in the legacy implementation and/or documented in kernel/desktop sources); MEDIUM for 7–12 (community reports and ecosystem state)

## Critical Pitfalls

### Pitfall 1: Trusting sensor indices instead of names

**What goes wrong:**
The app reports the wrong component's temperature. `temp1_input` is assumed to be the CPU, or `hwmon0` is assumed to be the fan controller. On a different model the numbers silently swap, and users make thermal decisions from the wrong reading.

**Why it happens:**
The Linux hwmon ABI explicitly does not guarantee that `tempN` maps to a particular component, and `hwmonN` numbering depends on driver load order. On a laptop the driver/label set differs per model and kernel.

**How to avoid:**
Resolve sensors by reading `name` (driver) and `tempN_label` before assigning a friendly name, using one shared, module-level priority list. Compose every DRM path from the enumerated card directory rather than a literal `card0`. Export what was detected in Diagnostics so a mismatch is diagnosable.

**Warning signs:**
Two code paths (dashboard vs. diagnostics) can disagree about which sensor is the CPU; a hardcoded `card0` string exists anywhere; a sensor list is written inline in more than one function.

**Phase to address:**
The phase that builds Linux telemetry/sensor discovery — one shared sensor-selection helper must exist before any UI consumes it.

---

### Pitfall 2: Assuming fan control exists (or that it works the way this model's did)

**What goes wrong:**
The Cooling page offers controls that silently do nothing, or writes a PWM value the embedded controller ignores. Users conclude the app is broken — or worse, believe cooling changed when it did not.

**Why it happens:**
`pwm1_enable` is exposed on some OMEN/Victus models and not others; several OMEN 16 models expose RPM but no writable PWM interface without a patched `hp-wmi`, and upstream only began adding model-gated manual fan control in the 6.20/7.0 kernel cycle. Firmware differences also mean "writable" does not imply "honored".

**How to avoid:**
Capability detection decides whether the control renders at all; every write is followed by a read-back and reports the observed state, never an optimistic success. Treat `0`/`1`/`2` as named modes (`MAX`/`MANUAL`/`AUTO`) rather than magic strings, and report `MANUAL` honestly instead of "Unknown".

**Warning signs:**
A control is rendered before a capability probe; a write path has no read-back; the string literals `"0"`/`"2"` appear in more than one module; "success" is shown without re-reading the file.

**Phase to address:**
The cooling/control phase, immediately after the privilege boundary exists — never before verification works.

---

### Pitfall 3: Privilege escalation into the GUI process

**What goes wrong:**
The whole GUI runs as root, or the app re-executes itself under `sudo` mid-command. The command body runs twice, user-writable configuration gets written as root, and a missing `sudo` produces an uncaught traceback. A user-editable file becomes an input to a root write — an escalation path.

**Why it happens:**
Re-exec under sudo is the shortest visible path to "I need to write to `/sys`", and it works on the developer's machine. The second-execution and ownership side effects are invisible until a user reports "permission denied" on their own config.

**How to avoid:**
One narrow system service (`com.zenomen.Hardware1`) owns every privileged write; the GUI is always unprivileged and talks to it over D-Bus with polkit authorization. The service accepts only named operations with whitelisted/validated parameters, builds paths from constants, and never accepts a path or command from the caller.

**Warning signs:**
`ensure_root`-style code anywhere in the GUI; any `sudo`, `execvp` or setuid usage; a config file whose owner changes after a control operation; a D-Bus method that takes a path parameter.

**Phase to address:**
The privileged-control phase. It must exist *before* any write-capable UI, so no write path can be built the wrong way first.

---

### Pitfall 4: UI-thread blocking (the "lightweight app that freezes")

**What goes wrong:**
The window stutters or locks during every refresh, and idle CPU is far above the 0.1% target. A single 500 ms sampling sleep on the render path is enough to make a 40 MB app feel slower than a heavyweight one.

**Why it happens:**
Direct hardware reads and external commands look synchronous and cheap. `nvidia-smi` can take seconds on a slow driver; a two-sample `/proc/stat` delta requires waiting; sysfs reads on some devices block. Slint runs its event loop on the main thread, so any of these there is a visible freeze. The legacy Python dashboard does exactly this with a fixed 0.5 s sleep per render.

**How to avoid:**
One long-lived monitoring worker with deadline scheduling and per-metric periods (CPU 1 s, GPU 2 s, RAM 5 s, profile 5 s; slower in tray mode). It keeps its own previous counters, so no sleep is needed to compute deltas. The UI only consumes published snapshots. Resolve external tools once and cache the negative result.

**Warning signs:**
Any hardware read or `Command::new` reachable from a Slint callback; a `sleep` in the sampling path; a chart whose frame rate is tied to the sampling rate; the window's frame time changing when telemetry refreshes.

**Phase to address:**
The GUI foundation phase (the worker and the command/event split must be the first design, not a later optimization) — then re-measured in the optimization phase.

---

### Pitfall 5: Unbounded memory in the "bounded memory" app

**What goes wrong:**
RSS starts at 25 MB and reaches 200 MB after a few hours in tray mode. The product's headline claim fails on a long-running session, which is exactly the case a tray app must survive.

**Why it happens:**
History vectors, log buffers, path caches and per-refresh allocations accumulate. Slint model churn can grow without bound if models are rebuilt per tick. It is invisible in a 30-second smoke test.

**How to avoid:**
Fixed-capacity `VecDeque` (120 samples) with explicit eviction; mutate model rows in place instead of rebuilding them; cap log retention; run a long-session memory test (30 min dashboard, 2 h tray, repeated open/close) in the optimization phase with RSS recorded as a first-class verification item.

**Warning signs:**
Any `Vec::push` in the sampling loop with no corresponding cap; model arrays re-assigned per telemetry event; RSS growth that does not plateau over 30 minutes.

**Phase to address:**
Foundation phase for the buffer; optimization phase for the measured long-session test.

---

### Pitfall 6: Optimistic or partial-success reporting on hardware writes

**What goes wrong:**
The UI shows a green check, but the fans did not change, or a combined preset applied the power profile and silently failed on the fan mode. Users trust a lie, and the failure is undiagnosable.

**Why it happens:**
The write "succeeded" from the OS's perspective — `write()` returned without error and the EC ignored the value. Verification feels redundant when the write already returned `Ok`.

**How to avoid:**
Read back after every write, with retry/backoff rather than one fixed 0.1 s delay (a slow EC/firmware can fail a single-sample check spuriously). Distinguish `applied` / `partially applied` / `failed` in the event model and the UI. Never render a fabricated value — missing data is `Unavailable`, not `0`.

**Warning signs:**
A write path without a read-back; a success toast with no state re-read; a value of 0 displayed for an absent sensor; no `VERIFICATION_FAILED` error code in use.

**Phase to address:**
The privileged-control phase (verification is part of the write contract, not a follow-up).

---

### Pitfall 7: Recipe-packaging toolchain chosen for the wrong platform or state of maintenance

**What goes wrong:**
The release pipeline is blocked late in the project because the packaging tool produces wrong artifacts, has unpatched advisories, or cannot produce the required format. Discovered at release time, this costs days.

**Why it happens:**
`cargo-packager` is the documented default for Rust desktop packaging and appears in the spec, but it changed hands (a post-handoff 0.12.0 release in Feb 2026) and an open issue (Jul 2026) reports it not really maintained with unaddressed RUSTSEC advisories. It is still used successfully in the wild, so the risk is "unmanaged", not "unusable".

**How to avoid:**
Pin an exact `cargo-packager` version from day one, gate CI on `cargo audit`, and prove the DEB/AppImage pipeline early — in the packaging phase, not at first release. Keep `cargo-deb` + `appimagetool`/`linuxdeploy` as a documented fallback. Verify on a clean Ubuntu image that the DEB installs and the AppImage runs on a machine without the build toolchain.

**Warning signs:**
`cargo install cargo-packager --locked` without a version; `cargo audit` failing or not present in CI; packaging first attempted in the same phase as the release; AppImage that only runs on the build machine.

**Phase to address:**
The Linux packaging/distribution phase, plus a `cargo audit` gate established in the CI phase.

---

### Pitfall 8: Tray and autostart behaving differently per desktop environment

**What goes wrong:**
On stock GNOME there is no tray icon at all (GNOME does not display StatusNotifierItem without the AppIndicator extension). If the app's only path back to its window is the tray, "close to tray" becomes "close forever" for GNOME users.

**Why it happens:**
Slint's Linux tray support is D-Bus-based (not XEmbed), which modern desktops mostly support — except stock GNOME. Testing on one KDE/XFCE machine hides it entirely.

**How to avoid:**
Never make the tray the sole access path: relaunching the app should focus the existing window (single-instance handling covers this), and Settings must always let the user reach the window. Add a first-run note when tray support is unavailable. Test on GNOME, KDE and at least one X11 environment.

**Warning signs:**
"Hide window" with no other way back; no single-instance handling; autostart entry hardcoding an absolute path that breaks when installed to a different prefix.

**Phase to address:**
The desktop-integration phase (tray/autostart/single-instance are one coherent slice).

---

### Pitfall 9: Rendering the generic layout instead of this machine's reality

**What goes wrong:**
The app looks identical on every laptop: controls that do nothing, sensors showing zeros, and no explanation. Users conclude it does not support their device — when the real answer was "this model exposes two of the five sensors".

**Why it happens:**
Designing from the developer's machine and greying out what is missing is the path of least resistance, and `Option::unwrap_or_default()` pages over the gap with a plausible-looking zero.

**How to avoid:**
Capabilities drive rendering: show supported features, and for unsupported ones state why in plain language ("Fan control is unavailable on this device — firmware-controlled operation remains active"). Never substitute 0 for missing data.

**Warning signs:**
`unwrap_or_default()` on a sensor reading; a UI value with no "unavailable" representation; capability detection added after the pages were built.

**Phase to address:**
The GUI foundation phase for the model; every later phase must respect it.

---

### Pitfall 10: Configuration handling that destroys user data or silently misbehaves

**What goes wrong:**
A corrupt `config.toml` is overwritten with defaults, losing the user's setup. Or a bad value (e.g. a 0 ms polling interval) is accepted and pegs a core.

**Why it happens:**
"Parse; on error write defaults" is the simplest code and looks robust.

**How to avoid:**
On a parse error, load safe defaults, keep the invalid file for troubleshooting, and surface the problem. Validate ranges on load (clamping intervals to sane bounds) and validate again on write. Write atomically (temp file + rename) so a crash cannot truncate config. Migrating legacy RGB profiles must never discard an unparseable file.

**Warning signs:**
`serde` errors converted directly into a default config write; no range validation; non-atomic writes; migration code that deletes the source.

**Phase to address:**
The configuration/persistence phase (and again in the RGB migration work).

---

### Pitfall 11: RGB profile fields that silently do nothing

**What goes wrong:**
A user sets brightness, speed or `loop` in a profile and the keyboard ignores it — because the hardware interface only accepts zone colors. The setting appears accepted and vanishes on restart.

**Why it happens:**
The legacy schema defines `length_ms`, `change`, `brightness`, `speed`, `loop` but only ever writes `zones`, and there is no UI feedback that the fields are inert. Carrying the schema forward without fixing or documenting it repeats the disappointment.

**How to avoid:**
Decide explicitly per field: either implement it or remove it from the schema and document that only zone colors reach hardware. Validate color values strictly (`#RRGGBB` only) — the legacy tool raises an uncaught `ValueError` traceback on `"red"` instead of reporting an invalid profile.

**Warning signs:**
Profile fields with no assignment in the apply path; no color validation before `int(hex, 16)`; migration that produces fields the app will ignore.

**Phase to address:**
The RGB capability phase (validation and schema honesty are part of it, not a polish item).

---

### Pitfall 12: Leaving the old implementation's defects in place as "the baseline"

**What goes wrong:**
The Python tool is kept as the reference for behavior, so its known bugs get ported deterministically: `-w` ignored on `stats`, `-w` always showing the full dashboard, a 1.5 s interval documented as 2 s, hardcoded `card0`, double-executed `rgb set`, duplicate thermal selection. The rewrite inherits them and the "rewrite" buys nothing.

**Why it happens:**
"Match the behavior exactly" is the natural parity rule, and bugs look like behavior. Nobody re-reads the CLI help text against the code.

**How to avoid:**
Write down the known defects (they are already documented in `.planning/codebase/CONCERNS.md`) as explicit requirements *not* to reproduce, and verify each with a test or a manual check. Port the proven value, not the mistakes.

**Warning signs:**
A phase plan that says "match legacy behavior" with no delta list; help text copied without verification; no test covering the previously broken path.

**Phase to address:**
Requirements definition (now) and each phase's verification criteria (continuously).

## Technical Debt Patterns

| Shortcut | Immediate Benefit | Long-term Cost | When Acceptable |
|----------|-------------------|----------------|-----------------|
| `nvidia-smi` subprocess instead of NVML | No extra dependency, works today | ~5 s timeouts and per-tick process spawns; the top CPU/RSS offender | Acceptable for the first GPU implementation; replace during optimization |
| `MockHardware` shipped in the binary behind a flag | UI development without a laptop | Dead code risk if it leaks into release behavior | Acceptable and encouraged — it is how the app is tested at all |
| Single crate instead of a workspace (until the daemon lands) | Faster start, less config | The privileged daemon must not live in the GUI binary | Acceptable in M1; split before the service exists |
| Inline `.slint` files until components stabilize | Fewer files to manage | Copy-pasted UI diverges | Acceptable for one phase, never for shared metric cards |
| Skipping `cargo audit` locally | Faster commits | Advisory creep in a root-privileged tool | Never acceptable once the daemon exists |
| Deferring the Windows backend | Focus on the validated platform | The trait boundary can rot if nothing exercises it | Acceptable — provided the trait stays honest and tests use `MockHardware` rather than Linux paths |

## Integration Gotchas

| Integration | Common Mistake | Correct Approach |
|-------------|----------------|------------------|
| D-Bus system bus | Registering with no bus policy, letting any process claim the name | Ship a policy file restricting ownership to the service user |
| polkit | Granting `yes` to every caller | Authorize per action; `auth_admin_keep` for hardware writes |
| systemd unit | Running the daemon as root with full filesystem access | Sandbox it (`ProtectSystem`, `ProtectHome`, `PrivateTmp`, a limited `DeviceAllow`/read-write path set) since the only thing it needs is a few sysfs paths |
| XDG autostart | Absolute `Exec` path that breaks across prefixes | Resolve via `$PATH`/the installed location; `Exec=zenomen --minimized` |
| Desktop tray | Assuming the icon is visible on every DE | Fallback path to the window; detect and explain when unavailable |
| GitHub Releases | Calling the API on every launch (rate limits, offline failures) | v1 only links to the Releases page; no API dependency for core function |
| /sys writes | Trusting `write()` returning `Ok` as confirmation | Read back with retry/backoff; distinguish observed state |

## Performance Traps

| Trap | Symptoms | Prevention | When It Breaks |
|------|----------|------------|----------------|
| Full sensor sweep at a fixed 1 s interval | Idle CPU above target; fans of the app itself audibly spinning up | Per-metric deadlines (CPU 1 s, GPU 2 s, RAM 5 s, profile 5 s), slower in tray | Immediately on old/low-power CPUs |
| `nvidia-smi` per tick | 1–5 s stalls, process churn, idle CPU spikes | Resolve once, cache the negative, then NVML | As soon as a dashboard is left open |
| Re-globbing `/sys` every tick | Steady wasted CPU and syscalls | Enumerate hwmon/drm once at loop start; re-read only values | Within minutes of tray residency |
| Rebuilding Slint models per telemetry event | RSS climb + render spikes | Mutate rows in place; publish deltas | Hours of uptime (the tray case) |
| Unbounded history/log growth | "Lightweight" app at hundreds of MB | `VecDeque` cap 120; bounded log retention | Long sessions — exactly the tray scenario |
| Continuous value animation in the UI | Idle CPU never reaches 0.1% | Update values without animating them; animate only deliberate transitions | Immediately; it is a constant floor, not a spike |

## Security Mistakes

| Mistake | Risk | Prevention |
|---------|------|------------|
| GUI or daemon accepting a path/command from the caller | Arbitrary root file write / code execution | Whitelisted operations, constants for paths, schema validation before any write |
| TLS verification disabled in any network path | On-path binary substitution | v1 performs no downloads at all; if a check is added, use the default TLS context |
| Executing a downloaded artifact | Remote root code execution (the legacy updater's defect) | Never execute downloaded content; v1 links to Releases instead |
| Shipping root code without an audit gate | Privilege-boundary changes land unreviewed | `cargo audit` + clippy in CI from the first phase; security review before the service ships |
| Unvalidated values interpolated into sysfs paths or RGB hex parsing | Crashes and (future) injection into a root path | Strict `#RRGGBB` validation; reject rather than repair; error code, not traceback |
| Daemon with unrestricted filesystem access | A compromised daemon owns the machine | systemd sandboxing + the minimum writable path set |

## UX Pitfalls

| Pitfall | User Impact | Better Approach |
|---------|-------------|-----------------|
| Zero instead of "Unavailable" | Users chase a phantom problem | Explicit unavailable state with the reason |
| Silent success on an ignored write | User believes cooling changed | Show observed state; partial-apply messaging |
| Generic layout, unexplained gaps | "It doesn't support my laptop" | Capability-driven pages with plain-language explanations |
| A single aggregated "CPU" temperature when several exist | Wrong sensor becomes the headline number | Prefer the driver/label-prioritized CPU sensor; expose others on demand |
| Tray-only access on GNOME | App seems to vanish | Relaunch focuses the window; Settings reachable without the tray |
| RGB settings that silently don't apply | Distrust of every setting | Only offer what the hardware honors; document the rest |
| Animating numbers to look "alive" | Constant CPU floor, contradicts the product promise | Static values, restrained transitions |

## "Looks Done But Isn't" Checklist

- [ ] **Fan control:** renders, but was it read back and the *observed* mode shown? Verify with a forced failure (write then restore, confirm the UI reports the real state).
- [ ] **Power profiles:** applied, but is the profile re-read after the write, and are choices read from `platform_profile_choices` rather than hardcoded?
- [ ] **Unsupported hardware:** controls hidden/disabled with an explanation on a machine lacking `pwm1_enable` — not merely greyed out with a zero value.
- [ ] **Memory budget:** measured on a 2-hour tray session with RSS recorded, not inferred from a short smoke test.
- [ ] **Tray:** verified on GNOME (with and without AppIndicator) and KDE; quitting works from the tray; relaunch focuses the existing instance.
- [ ] **Packaging:** DEB installs on a clean Ubuntu image and the AppImage runs on a machine without the build toolchain; desktop entry and systemd unit both land where expected.
- [ ] **Privilege boundary:** no `sudo`/`execvp`/setuid anywhere in the GUI; the daemon rejects an out-of-whitelist parameter with an error code, not a traceback.
- [ ] **Config robustness:** corrupt `config.toml` → safe defaults, original preserved, user told; legacy RGB migration keeps the source.
- [ ] **RGB honesty:** profile fields either honored or absent from the schema; `"red"` produces an error message, not a traceback.
- [ ] **CI:** `fmt`, `clippy -D warnings`, `test`, `cargo audit` all fail the build on violation.
- [ ] **Legacy defects:** each item in `CONCERNS.md` that ZenOmen replaces is either absent by design or has a check proving so.

## Recovery Strategies

| Pitfall | Recovery Cost | Recovery Steps |
|---------|---------------|----------------|
| Wrong sensor reported to users | MEDIUM | Add the label/driver resolution helper, route both call sites through it, export detection in Diagnostics, release a patch with an explanation |
| Unsafe root write path discovered | HIGH | Disable the affected operation immediately, move it behind the service with validation, add a regression test, audit all other write paths |
| Memory grows in tray mode | MEDIUM | Cap the buffer, stop rebuilding models, add a long-session RSS test, release with measured numbers |
| Shipping toolchain blocked by advisories | MEDIUM | Freeze releases, migrate to `cargo-deb` + `appimagetool`, re-verify DEB/AppImage on a clean image |
| Config destroyed by a bad write | LOW-USERS / HIGH-TRUST | Restore from the preserved invalid file, move to atomic writes, add a validation test |
| Tray invisible on GNOME | LOW | Document the extension requirement, guarantee the relaunch-to-focus path |

## Pitfall-to-Phase Mapping

| Pitfall | Prevention Phase | Verification |
|---------|------------------|--------------|
| Sensor identity by name, not index | Foundation / Linux telemetry | Unit test with a synthetic hwmon tree; dashboard and diagnostics agree on the CPU sensor |
| Fan control capability + verified writes | Cooling control | Forced verification failure shows a failure message; unsupported device hides the control with a reason |
| Privilege escalation | Hardware service | No `sudo`/`execvp` in the GUI binary; daemon rejects an invalid parameter with a coded error |
| UI-thread blocking | GUI foundation | Window frame time unchanged while a GPU sample runs; full sweeps happen only on the worker |
| Unbounded memory | Foundation buffer + Optimization | 2-hour tray session RSS within budget and flat |
| Optimistic success reporting | Cooling/power control | Read-back mismatch produces `VERIFICATION_FAILED` on screen |
| Packaging toolchain risk | Packaging + CI | `cargo audit` green; DEB installs on a clean Ubuntu VM; AppImage runs off-machine |
| Tray/autostart DE variance | Desktop integration | Manual check on GNOME (with/without extension) and KDE; autostart works after a real login |
| Generic layout vs. machine reality | GUI foundation | Run on a machine or mock with a deliberately limited capability set |
| Config data loss | Configuration | Corrupt-file test preserves the original; interval out of range is clamped |
| Inert RGB fields | RGB capability | Setting a non-honored field is either impossible or clearly documented; invalid color gives a clean error |
| Inherited legacy defects | Requirements + every phase | The `CONCERNS.md` delta list is checked off per phase |

## Sources

- `.planning/codebase/CONCERNS.md` and `ARCHITECTURE.md` (OpenOMEN) — pitfalls 1, 3, 4, 5, 6, 11, 12 are documented observed defects with line references; HIGH
- https://docs.kernel.org/hwmon/sysfs-interface.html — no guarantee that `tempN`/`hwmonN` map to fixed components; HIGH
- https://docs.kernel.org/userspace-api/sysfs-platform_profile.html — profile choices must be read dynamically; HIGH
- https://github.com/uAtomicBoolean/slint-template/issues/6 (2026-07-28) — cargo-packager maintenance/RUSTSEC concern; MEDIUM
- Phoronix / LKML / Arch Wiki (Jan–Jul 2026) — model-gated manual fan support and `pwm1_enable` behavior; MEDIUM
- HP support forum and Reddit reports (2026) — models where fan control is inaccessible under Linux due to firmware/ACPI issues; MEDIUM
- https://slint.dev/blog/slint-1.17-released and the desktop-readiness post — Linux tray via D-Bus StatusNotifierItem, GNOME caveat; HIGH for the mechanism, MEDIUM for the GNOME-extension requirement

---
*Pitfalls research for: ZenOmen*
*Researched: 2026-10-30*
