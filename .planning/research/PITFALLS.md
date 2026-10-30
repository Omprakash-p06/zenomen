# Pitfalls Research

**Domain:** Linux hardware control + telemetry GUI (HP OMEN/Victus), Rust + Slint  
**Researched:** 2026-10-30  
**Updated:** 2026-10-30 (incorporated review corrections: shared sensor registry, 3-state fan gating, mandatory polkit, PresetResult, RAM from scratch, GNOME tray fallback, and memory benchmarks)  
**Confidence:** HIGH for pitfalls 1–6 (observed in the legacy implementation and/or documented in kernel/desktop sources); HIGH for 7–12 (validated architecture and ecosystem constraints)

## Critical Pitfalls

### Pitfall 1: Trusting sensor indices instead of names & hardcoding GPU paths

**What goes wrong:**
The app reports the wrong component's temperature, or fails to find the iGPU entirely. `temp1_input` is assumed to be the CPU, `hwmon0` is assumed to be the fan controller, and `card0` is hardcoded as the Intel GPU. On different laptop models or kernel updates, the numbers silently swap, frequency lookups fail, and users make thermal decisions from incorrect readings.

**Why it happens:**
The Linux hwmon ABI explicitly does not guarantee that `tempN` maps to a particular component, and `hwmonN` numbering depends on driver load order. Furthermore, DRM card indices vary based on iGPU/dGPU enumeration order (e.g. `card0` may be NVIDIA or AMD, while Intel is `card1`). The legacy Python script hardcoded `card0` for Intel GPU frequency lookups and duplicated temperature heuristics between `read_temperature()` and `run_doctor()`.

**How to avoid:**
Implement a single, shared `SensorRegistry` in the hardware layer that discovers sensors dynamically using driver priorities (`coretemp` > `k10temp` > `zenpower` > `hp-wmi` > `acpitz`) and label matches. Dynamically enumerate `/sys/class/drm/card*` by inspecting vendor IDs, never hardcoding `card0`. Both dashboard telemetry and diagnostics must consume the exact same `SensorRegistry`.

**Warning signs:**
Two code paths (dashboard vs. diagnostics) disagree about which sensor is the CPU; a hardcoded `card0` string exists anywhere; a sensor list is written inline in more than one function.

**Phase to address:**
Phase 2 (Linux Telemetry & Capability Detection) — the shared `SensorRegistry` must be established before any UI consumes real data.

---

### Pitfall 2: Assuming fan control exists or exposing manual mode prematurely

**What goes wrong:**
The Cooling page offers controls that silently do nothing, writes a PWM value the embedded controller ignores, or attempts to set "Manual" mode (`pwm1_enable = 1`) which stalls the laptop's fans or causes firmware lockups.

**Why it happens:**
`pwm1_enable` is exposed on some OMEN/Victus models and not others. The sysfs interface technically documents three states: `0` (Maximum), `1` (Manual PWM), and `2` (Automatic/BIOS). The legacy Python CLI only supported `0` and `2`. While `1` exists as a numeric value, HP-WMI firmware often lacks true arbitrary PWM curve support without custom kernel patches. Exposing manual mode just because the number `1` exists in sysfs docs is a hardware-safety hazard.

**How to avoid:**
1. Capability detection decides whether fan controls render at all; if `pwm1_enable` is not writable, display an honest "Unavailable" explanation.
2. For v1, expose only validated modes: **Automatic** (`2`) and **Maximum** (`0`).
3. If the kernel currently reports `1` (manual) on read, display it honestly as "Manual (external/kernel control)", but do not allow writing manual mode in v1.
4. Every write is followed by read-back verification with retry/backoff.

**Warning signs:**
A control is rendered before a capability probe; a write path has no read-back; manual mode slider is exposed without validated EC support; string literals `"0"`/`"2"` appear without named constants.

**Phase to address:**
Phase 3 (Verified Hardware Control) — capability gating and read-back verification must precede write exposure.

---

### Pitfall 3: Privilege escalation into the GUI process & optional authorization

**What goes wrong:**
The GUI runs as root or re-executes itself via `sudo`. The command body runs twice, user config becomes root-owned, and a user-editable configuration file becomes an input to a root write — an escalation vulnerability. Alternatively, treating polkit authorization as optional allows unprivileged processes to tamper with hardware.

**Why it happens:**
`sudo` re-exec is the easiest shortcut to write to `/sys`. In the legacy Python CLI, `rgb set` executed twice (once as user, once as root), changing config ownership.

**How to avoid:**
A two-process architecture: `zenomen-gui` is strictly unprivileged. Privileged writes live exclusively in the `zenomen-hw` system D-Bus daemon (`com.zenomen.Hardware1`). Polkit authorization (`com.zenomen.hardware.control`) is **mandatory** for every mutating method (`SetFanMode`, `SetPowerProfile`, `ApplyRgbProfile`, `SetRgbEnabled`). The daemon rejects arbitrary filesystem paths and shell commands.

**Warning signs:**
`ensure_root`-style code anywhere in the GUI; any `sudo` or `setuid` call; describing polkit authorization as optional; a D-Bus method that accepts a path parameter.

**Phase to address:**
Phase 3 (Hardware Service) — the service must be operational and audited before write capabilities are introduced in the UI.

---

### Pitfall 4: UI-thread blocking (the "lightweight app that freezes")

**What goes wrong:**
The window stutters or locks during telemetry refreshes, and idle CPU exceeds the 0.1% budget. A single 500 ms sampling sleep makes a 40 MB app feel sluggish and unresponsive.

**Why it happens:**
Direct hardware reads look synchronous and cheap. In the legacy Python tool, `read_cpu_stats()` literally executed `time.sleep(0.5)` to calculate utilization deltas, and `monitor_stats_loop()` polled synchronously. Furthermore, `nvidia-smi` can stall for seconds if the GPU driver is suspended.

**How to avoid:**
One long-lived monitoring worker with deadline scheduling running in a background thread. It preserves prior-tick counters to calculate CPU and disk deltas instantaneously without sleeps. Slint runs on the main thread and receives immutable `TelemetrySnapshot` events. External tools like `nvidia-smi` are resolved once and negative results are cached.

**Warning signs:**
Any `sleep()` in a sampling function; running `Command::new` or sysfs reads on the main thread; window frame time fluctuating when telemetry updates.

**Phase to address:**
Phase 1 (Foundation) — the worker thread and `AppCommand`/`AppEvent` bridge must be established in the initial scaffold.

---

### Pitfall 5: Unbounded memory in the "bounded memory" app

**What goes wrong:**
Process RSS starts at 25 MB but climbs past 150 MB after several hours in the tray. The core promise of lightweight system software is broken.

**Why it happens:**
Uncapped telemetry vectors, log buffers, and repeated Slint model allocations accumulate. Developers measure memory over a 30-second smoke test and miss long-term leaks.

**How to avoid:**
1. Use a fixed-capacity ring buffer (`VecDeque<TelemetryPoint>` capped at 120 samples) that drops the oldest sample before pushing.
2. Mutate Slint model rows in place rather than rebuilding arrays per tick.
3. Distinguish memory budgets: UI process peak RSS ≤40 MB (cold start <30 MB, warm dashboard <38 MB, minimized tray <25 MB); hardware service RSS ≤15 MB (measured separately).
4. Profile memory with `heaptrack` and `valgrind --tool=massif` during long-session tests (2 hours in tray).

**Warning signs:**
Unbounded `Vec::push` in the sampling loop; re-allocating Slint model arrays per tick; memory growing continuously over a 30-minute test.

**Phase to address:**
Phase 1 for the ring buffer; Phase 5 for automated 2-hour long-session verification.

---

### Pitfall 6: Optimistic or "transactional" claims on hardware writes

**What goes wrong:**
The UI claims a combined preset (e.g. "Gaming") applied successfully, but only the ACPI power profile stuck while the fan mode failed. Or the app claims "transactional rollback" for hardware operations that cannot be rolled back atomically.

**Why it happens:**
`write()` returning `Ok` only means the kernel accepted the buffer; the embedded controller might ignore the value. Furthermore, ACPI `platform_profile` and HP-WMI `pwm1_enable` are separate hardware endpoints without kernel transaction support.

**How to avoid:**
1. Verify every write by reading back state with retry/backoff (100 ms, 200 ms, 400 ms).
2. Never claim atomic transactions or rollback for combined presets. Use `PresetResult`:
   ```rust
   pub struct PresetResult {
       pub power_applied: bool,
       pub fan_applied: bool,
       pub partial_success: bool,
   }
   ```
3. Report partial applications honestly to the user (e.g. "Power profile set to Performance, but fan mode could not be changed").

**Warning signs:**
Success toast displayed immediately after `fs::write()`; absence of `PresetResult` or partial-apply UI states; claiming atomic rollback for ACPI/fan writes.

**Phase to address:**
Phase 3 (Verified Hardware Control).

---

### Pitfall 7: Packaging toolchain risk (`cargo-packager`)

**What goes wrong:**
Late in the project, packaging fails due to unmaintained dependencies or unaddressed RUSTSEC advisories in `cargo-packager`.

**Why it happens:**
`cargo-packager` underwent an ownership transfer and has known advisories in its dependency tree.

**How to avoid:**
Pin an exact version of `cargo-packager`, enforce `cargo audit` in CI, and establish `cargo-deb` + `appimagetool` as the documented fallback pipeline. Verify early on clean Ubuntu virtual environments.

**Warning signs:**
Floating `cargo-packager` version; `cargo audit` disabled or ignored in CI.

**Phase to address:**
Phase 1 for CI audit gates; Phase 5 for final package distribution.

---

### Pitfall 8: Tray invisibility on stock GNOME & window trap

**What goes wrong:**
On stock GNOME (which lacks StatusNotifierItem support out of the box), closing the app to the tray makes it disappear with no tray icon to restore it. The user believes the app crashed or became unreachable.

**Why it happens:**
Slint's Linux tray uses D-Bus StatusNotifierItem, which requires the AppIndicator GNOME Shell extension. Testing only on KDE or XFCE hides this issue.

**How to avoid:**
1. Implement a single-instance D-Bus activation mechanism: `zenomen-gui` registers `com.zenomen.App` on the session bus.
2. Launching ZenOmen again invokes `PresentWindow()` on `com.zenomen.App`, bringing the existing window to the front and focusing it, then the new process exits.
3. Provide an explicit "Close to tray" toggle in Settings, and test across GNOME, KDE, and X11.

**Warning signs:**
Relying solely on the tray icon to reopen the main window; absence of single-instance D-Bus focus handling.

**Phase to address:**
Phase 4 (Desktop Integration, Config & Basic RGB).

---

### Pitfall 9: Rendering generic layouts instead of hardware reality

**What goes wrong:**
The UI renders non-functional controls or fabricated zeros on laptops where specific sensors or controls are missing.

**Why it happens:**
Defaulting missing `Option<T>` values to `0` or hiding missing controls without explaining why.

**How to avoid:**
Capabilities drive UI rendering: every missing sensor displays an explicit "Unavailable" state with an explanation (e.g. "Fan control is unavailable on this device — firmware-controlled operation remains active"). Never substitute `0` for missing telemetry.

**Warning signs:**
Using `.unwrap_or_default()` on sensor readings; cards displaying `0 RPM` or `0°C` when sensors are absent.

**Phase to address:**
Phase 1 (UI Shell) for mock states; Phase 2 for real capability-driven rendering.

---

### Pitfall 10: Configuration corruption and destructive migration

**What goes wrong:**
A corrupt `config.toml` is silently overwritten with defaults, losing user preferences. Or legacy RGB migration deletes unparseable JSON files.

**Why it happens:**
Naïve error handling that writes default config whenever deserialization fails.

**How to avoid:**
1. If `config.toml` fails to parse, load safe defaults in memory, preserve the invalid file on disk (e.g. `config.toml.bak`), and notify the user.
2. Legacy RGB migration must never delete or overwrite existing `~/.config/openomen/rgb/*.json` files.
3. Perform atomic config writes using temporary files and atomic renames.

**Warning signs:**
Overwriting config files without preserving invalid ones; non-atomic file writes.

**Phase to address:**
Phase 4 (Configuration and RGB Migration).

---

### Pitfall 11: RGB profile fields that silently do nothing & unverified writes

**What goes wrong:**
Users import legacy RGB profiles containing `brightness`, `speed`, `length_ms`, `change`, and `loop`, expecting animations, but only static zone colors apply. Or writing RGB succeeds in the UI despite hardware failure.

**Why it happens:**
The legacy Python tool stored these animation fields in JSON without ever applying them to hardware, and returned success even if sysfs writes failed. Carrying over these fields without clarification creates false expectations.

**How to avoid:**
1. In v1 Basic RGB, clearly identify unsupported animation and timing fields during migration rather than promising them.
2. Validate `#RRGGBB` hex strings strictly (preventing Python's uncaught `ValueError` on strings like `"red"`).
3. Apply RGB through the privileged hardware service and verify writes where supported by hardware.

**Warning signs:**
Exposing animation sliders that the hardware does not support; accepting invalid hex without error handling.

**Phase to address:**
Phase 4 (Basic RGB & Migration).

---

### Pitfall 12: Porting legacy Python bugs as "behavioral baseline"

**What goes wrong:**
The Python CLI is treated as an architectural blueprint rather than a hardware-behaviour reference, leading to inherited bugs:
- No RAM telemetry (the legacy CLI lacked it entirely, so developers might overlook it; ZenOmen must implement it from scratch via `/proc/meminfo`).
- `card0` hardcoding for Intel GPU.
- 500 ms blocking sleep during CPU sampling.
- Ignoring `-w` flag on `stats`.
- Disabling TLS verification in updater.
- Diverging temperature sensor selection in `doctor`.

**How to avoid:**
Maintain an explicit "Do Not Port" defect list in project requirements and planning documentation. Verify that each new Rust subsystem solves these issues natively.

**Warning signs:**
Adopting Python script architecture (re-exec, synchronous loops, duplicate heuristics); missing RAM telemetry implementation.

**Phase to address:**
Continuous across all phases.

---
*Pitfalls research for: ZenOmen*  
*Updated: 2026-10-30*
