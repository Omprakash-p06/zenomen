# Architecture Research

**Domain:** Native desktop system utility with a privileged hardware backend
**Researched:** 2026-10-30
**Confidence:** HIGH for the layered model and privilege split (well-established pattern, and the user's spec already fixes it); MEDIUM for the specific concurrency primitive choices

## Standard Architecture

The established pattern for a hardware control GUI on Linux is a **three-process split**: an unprivileged GUI, an unprivileged-or-system hardware service that owns privileged writes, and the kernel interfaces underneath. This is exactly how modern desktop tools (KDE's solid/powerdevil stack, system76's tools, fwupd) are arranged, and it is the model the project's specification fixes.

### System Overview

```text
┌──────────────────────────────────────────────────────────────┐
│  Presentation — Slint (.slint markup, compiled)              │
│  Overview · Cooling · Performance · Lighting · Settings ·    │
│  Diagnostics      (system tray + main window)                │
│  Never reads /sys, never writes files, never calls sudo      │
└───────────────────────────┬──────────────────────────────────┘
                            │  AppCommand ──▼ / ▲── AppEvent
┌───────────────────────────┴──────────────────────────────────┐
│  Application Core — Rust (unprivileged, main process)        │
│  ┌────────────┐ ┌────────────┐ ┌─────────────┐ ┌──────────┐ │
│  │State mgr   │ │Scheduler / │ │Telemetry    │ │Profile   │ │
│  │(AppState)  │ │poll worker │ │buffer (ring)│ │manager   │ │
│  └────────────┘ └────────────┘ └─────────────┘ └──────────┘ │
│  ┌────────────┐ ┌────────────┐ ┌─────────────┐              │
│  │Config      │ │Diagnostics │ │Services     │              │
│  │(TOML)      │ │export      │ │(tray,auto-  │              │
│  │            │ │            │ │ start,log)  │              │
│  └────────────┘ └────────────┘ └─────────────┘              │
└───────┬──────────────────────────────────┬───────────────────┘
        │ HardwareProvider (in-process,    │ D-Bus client
        │ read-only telemetry)             │ com.zenomen.Hardware1
        ▼                                  ▼
┌────────────────────────┐   ┌──────────────────────────────────┐
│ Platform backend        │   │ Privileged hardware service      │
│ (compiled per target)   │   │ (small root daemon, systemd)     │
│ linux/{procfs,hwmon,drm,│   │ - polkit authorization           │
│ acpi,hp_wmi}.rs         │   │ - whitelisted operations only    │
│ windows/{cpu,gpu,power}.│   │ - validates every parameter      │
└───────────┬─────────────┘   │ - read-back verification         │
            │                 └──────────────┬───────────────────┘
            ▼                                ▼
┌──────────────────────────────────────────────────────────────┐
│  Kernel / OS interfaces                                      │
│  /sys/class/hwmon · /sys/firmware/acpi · /sys/class/drm ·    │
│  /proc · hp-wmi sysfs · Win32 APIs                           │
└──────────────────────────────────────────────────────────────┘
```

### Component Responsibilities

| Component | Responsibility | Typical Implementation |
|-----------|----------------|------------------------|
| Slint presentation | Render state, collect input, show capability gaps and errors | `.slint` files compiled by `slint-build`; models bound to core structs; **no hardware access** |
| Application core | Lifecycle, scheduling, normalization, history, validation, persistence | Pure Rust, `std` threads; owns all state |
| `HardwareProvider` trait | One capability-discovered interface per platform | `detect`, `read_telemetry`, `get_capabilities`, fan/power read+write |
| `RgbProvider` trait | Optional lighting capability | Separate from `HardwareProvider` because RGB is genuinely optional |
| Measurement (read) path | Telemetry that needs no privilege | **In-process**, direct sysfs/`/proc` reads — no D-Bus round-trip per sensor |
| Privileged service | The only writer to hardware | Small root daemon on the system bus, polkit-gated, whitelisted ops |
| Monitoring worker | One long-lived thread sampling on deadlines | Publishes `TelemetrySnapshot` to the core; never touches the UI |
| Config + diagnostics | Local-first persistence and support output | TOML via serde, `tracing` logs, user-initiated export |

## Recommended Project Structure

The user's spec already lays out an incremental target. Research agrees with the shape, with one amendment: **do not create empty module trees ahead of need.**

```text
zenomen/
├── Cargo.toml              # workspace or single crate; pin slint = "1.17"
├── build.rs                # slint-build compiles ui/*.slint
├── Packager.toml           # DEB + AppImage (pin cargo-packager; audit it)
├── ui/
│   ├── app.slint           # shell, navigation
│   ├── overview.slint      # metrics + charts
│   ├── cooling.slint       # fans (capability-gated)
│   ├── performance.slint   # ACPI profiles
│   ├── lighting.slint      # RGB presets
│   ├── settings.slint
│   ├── diagnostics.slint
│   ├── components/         # metric-card, temperature-card, telemetry-chart,
│   │                       # profile-selector, status-indicator, sidebar
│   └── theme/              # colors.slint, typography.slint
├── src/
│   ├── main.rs             # wiring only
│   ├── app.rs              # UI <-> core bridge (AppCommand/AppEvent)
│   ├── core/               # state, events, scheduler, telemetry, memory, config, lifecycle
│   ├── hardware/           # provider trait + vendor-neutral models
│   ├── platform/
│   │   ├── linux/          # procfs, hwmon, drm, acpi, hp_wmi, dbus(client)
│   │   └── windows/        # created in the Windows milestone, not before
│   ├── services/           # tray, autostart, diagnostics, logging, updater-link
│   └── security/           # permissions, validation
├── daemon/zenomen-hw/      # the privileged service (separate binary)
├── packaging/linux/        # .desktop, .service, dbus policy, polkit policy
├── assets/
├── tests/                  # calculations, telemetry, profiles, mock_hardware, integration
└── .github/workflows/      # ci.yml, release.yml, security.yml
```

### Structure Rationale

- **`ui/` separate from `src/`:** `.slint` is markup, not Rust; keeping it out of `src/` makes the "UI never touches hardware" rule visually obvious and keeps `build.rs` trivial.
- **`daemon/` as a separate crate/binary:** the privileged service must be independently reviewable and independently packaged (it is the only artifact that installs into `/usr/lib/systemd/system`). A separate binary also keeps the GUI's RSS and blast radius small.
- **`platform/` split by OS, `hardware/` vendor-neutral:** matches the spec's `HardwareProvider` trait and makes the Windows milestone additive rather than a refactor.
- **`security/` as a real directory:** validation and permission logic is a first-class concern here, not a helper — it is the difference between a hardware tool and a root exploit.
- **Introduce incrementally:** the spec's instruction to avoid empty modules is right. M1 needs `main.rs`, `app.rs`, `core/{state,events,telemetry,memory,config,lifecycle}.rs`, `hardware/{mod,provider,mock}.rs`, `ui/app.slint` + `overview.slint` + `theme/` + a couple of components. Nothing under `daemon/` or `platform/` should exist until its milestone.

## Architectural Patterns

### Pattern 1: Message-passing bridge (no shared mutable UI state)

**What:** The UI sends `AppCommand` variants to the core; the core publishes `AppEvent` variants back. Slint models are updated only in response to events.
**When to use:** From M1, unconditionally.
**Trade-offs:** A little boilerplate per new field, in exchange for one place where state changes and zero cross-thread UI mutation. It also makes `MockHardware` trivially swappable for UI development and tests.

**Example:**
```rust
pub enum AppCommand {
    RefreshTelemetry,
    GetCapabilities,
    SetPowerProfile(PowerProfile),
    SetFanMode(FanMode),
    ApplyRgbProfile(RgbProfile),
    ExportDiagnostics,
}

pub enum AppEvent {
    TelemetryUpdated(TelemetrySnapshot),
    CapabilitiesUpdated(HardwareCapabilities),
    OperationSucceeded,
    OperationFailed(HardwareError),
}
```

### Pattern 2: Deadline-scheduled single monitoring worker

**What to do:** One thread; wait until the next deadline; sample only what is due (CPU 1 s, GPU 2 s, RAM 5 s, profile 5 s); normalize; publish one snapshot; recompute deadlines when the app state changes (dashboard vs tray).
**Why:** Polling is the app's steady-state CPU cost. One worker with per-metric periods and state-aware intervals is how the ≤0.1% idle target is actually met — a thread per sensor or a fixed 1 s full sweep is not.
**Trade-offs:** More bookkeeping than a fixed loop; worth it because the budget is a product requirement.

### Pattern 3: Snapshot-and-evict telemetry history

**What to do:** `VecDeque<TelemetryPoint>` capped at 120 entries; `pop_front` before `push_back`.
**Why:** Predictable memory. The alternative (`Vec` that grows) is the classic reason "lightweight" monitors end up at 500 MB after a day.
**Trade-offs:** Bound must be reasoned about per metric; 120 × 1 s ≈ 2 minutes of history is enough for a chart and small enough to ignore in the budget.

### Pattern 4: Capability detection as the single source of truth

**What to do:** `detect()` at startup produces `HardwareCapabilities`; every page, control and write path consults it. Capabilities are re-checked on service reconnect.
**Why:** Firmware varies per model, and `hp-wmi` support is actively changing upstream (manual fan control for Victus S landed only in the 6.20/7.0 cycle). Anything else hardcodes one machine's behavior.
**Trade-offs:** Slightly more up-front modeling; prevents an entire class of "button does nothing" bugs.

### Pattern 5: Privileged service with whitelisted, validated operations

**What to do:** A root daemon exposing only named methods (`SetFanMode(mode)`, `SetPowerProfile(profile)`, `ApplyRgbProfile(profile)`), where each parameter is validated against a whitelist or schema before any file is opened. Paths are constructed from constants, never from caller input.
**Why:** This is the trust boundary. The legacy Python tool executed its own command body twice under sudo and wrote user-influenced data to `/sys` as root; that must not carry over.
**Trade-offs:** More code than a setuid helper or an in-process sudo re-exec — and far less attack surface.

## Data Flow

### Telemetry flow (read-only, no privilege)

```text
Monitoring worker (deadline loop)
    ↓ samples due metrics (sysfs / /proc / nvidia-smi|NVML)
Normalize + validate (units, bounds, None when unavailable)
    ↓ push into bounded history
TelemetrySnapshot ──event──> App core ──> Slint models
    ↓ (on demand)
Diagnostics export (snapshot + recent history + errors)
```

### Control flow (privileged write, always verified)

```text
User clicks "Maximum" on Cooling page (or invokes a combined preset)
    ↓
Core validates against HardwareCapabilities  →  INVALID_PROFILE / UNSUPPORTED_OPERATION if not advertised
    ↓ AppCommand → D-Bus method call
zenomen-hw service
    ↓ polkit check (only if the reference implementation requires it)
    ↓ validates parameter against whitelist
    ↓ writes pwm1_enable
    ↓ reads back (with retry/backoff, not a single 0.1 s sleep)
    ↓ returns actual observed state or VERIFICATION_FAILED
Core  →  AppEvent::OperationSucceeded / OperationFailed(code)
    ↓
UI shows the real state; a partial apply reports exactly what did and did not change
```

### Startup / lifecycle flow

```text
Login
  ↓
zenomen starts (autostart arg: --minimized if configured)
  ↓
Single-instance check (D-Bus name or lock file) → existing instance? focus it and exit
  ↓
Load config.toml (invalid → safe defaults, keep the bad file for troubleshooting)
  ↓
Initialize provider (Linux) → detect capabilities → connect to zenomen-hw service
  ↓ service unavailable? → telemetry still works; controls disabled with an explanation
  ↓
Start monitoring worker
  ↓
Launch-to-tray? → tray only. Otherwise → dashboard window.
  ↓
Close window: to tray or exit, per config. Quit: stop worker, restore nothing it didn't change.
```

### Key Data Flows

1. **Metric read:** worker → normalizer → bounded history → snapshot event → UI model. Never UI-thread blocking.
2. **Hardware write:** UI intent → capability validation → D-Bus → service validation → sysfs write → read-back → observed state → UI. Every hop reports errors upward; none swallow them.
3. **Config:** load with validation → in-memory `Config` → user edit → validate → atomic write → applied to live worker (interval changes re-schedule without restart).
4. **Diagnostics:** version + OS/kernel + hardware IDs + detected sensors + recent telemetry + recent errors → user-initiated export to a file.

## Scaling Considerations

This product does not scale by users; it scales by **devices and metrics**.

| Scale | Architecture Adjustments |
|-------|--------------------------|
| 1 device (v1, developer's laptop) | `MockHardware` + one real Linux provider. All UI work is testable without hardware |
| 8–20 confirmed devices | Capability detection carries the load; every quirk is data in the detector, not a branch in the UI |
| 20+ devices, multiple vendors/kernels | Provider gains a per-model quirk table keyed by DMI/board name; add a `DeviceProfile` lookup rather than growing `if` chains |
| Many metrics/higher polling | Deadlines and per-metric periods already handle it; if charts grow, keep the ring buffer and decimate on render instead of storing more |

### Scaling Priorities

1. **First bottleneck: GPU polling cost.** Spawning `nvidia-smi` per tick is both slow and CPU-expensive. Fix by resolving the tool once, caching the negative result, then moving to NVML.
2. **Second bottleneck: sysfs re-scan per tick.** Enumerate hwmon/drm devices at loop start and re-read only `*_input` values. This is the legacy tool's known inefficiency.
3. **Third: render cost of live charts.** Update at the sampling rate but repaint only what changed; avoid continuously animating values.

## Anti-Patterns

### Anti-Pattern 1: UI reads sysfs directly

**What people do:** Bind a Slint callback to `std::fs::read_to_string("/sys/class/hwmon/...")` because it is one line.
**Why it's wrong:** It couples the UI to Linux paths, blocks the UI thread on file I/O (and on `nvidia-smi`), makes the Windows milestone a rewrite, and makes the whole app untestable without hardware.
**Do this instead:** All hardware access lives behind `HardwareProvider`; the UI only consumes `AppEvent`s.

### Anti-Pattern 2: Printing (or logging) instead of returning

**What people do:** Have the read function print and return `bool` — the legacy Python design.
**Why it's wrong:** Nothing can consume a temperature programmatically, so `doctor`, charts and any future export duplicate sensor logic and drift. That drift is already a real, reported bug in the legacy tool (the dashboard and `doctor` can name different sensors on the same machine).
**Do this instead:** Return structured `Result`/data; let the presentation layer format.

### Anti-Pattern 3: Sleep-based sampling on the hot path

**What people do:** `sleep(500ms)` between two `/proc/stat` reads, on the render path.
**Why it's wrong:** Every dashboard render — and every monitor tick — pays it; it also blocks whichever thread runs it.
**Do this instead:** Keep the previous counters in the worker and compute deltas against the last tick.

### Anti-Pattern 4: sudo re-exec / setuid escalation

**What people do:** Re-exec the whole app under `sudo` when a privileged action is needed.
**Why it's wrong:** The command body runs twice (state written as the user, then rewritten as root), user-writable data ends up in a root-executed path, and a missing `sudo` produces an uncaught traceback. All three are real defects in the legacy implementation.
**Do this instead:** A narrow D-Bus service; the GUI never escalates.

### Anti-Pattern 5: Optimistic success reporting

**What people do:** Write to sysfs, show a success toast.
**Why it's wrong:** Firmware may ignore the write. The user then believes the fans changed when they did not — a thermal-safety-adjacent lie.
**Do this instead:** Read back and report the observed state; distinguish "applied", "partially applied" and "failed".

### Anti-Pattern 6: Assuming `tempN`/`card0`/`hwmonN` numbering is stable

**What people do:** Read `temp1_input` and call it the CPU.
**Why it's wrong:** The kernel does not guarantee the mapping; the legacy tool hardcoded `card0` for Intel frequency and duplicated its sensor-selection heuristic until the copies disagreed.
**Do this instead:** Resolve by driver name + label, and compose DRM paths from the enumerated card, never from a literal `card0`.

## Integration Points

### External Services

| Service | Integration Pattern | Notes |
|---------|---------------------|-------|
| GitHub Releases | Outbound link only ("Check for updates") | No API call needed in v1; no download, no execution |
| D-Bus system bus | `zbus` client (GUI) / service (daemon) | Service name `com.zenomen.Hardware1`; ship a bus policy so only the expected owner can claim it |
| polkit | Service checks caller authorization per method | Ship a `.policy` file in the package |
| systemd | `zenomen-hw.service` unit, enabled when hardware control is installed | Only the service needs to be system-wide |
| XDG autostart | Write/remove `~/.config/autostart/zenomen.desktop` | User-level; toggled from Settings |
| Desktop tray (StatusNotifierItem) | Slint `SystemTrayIcon` | GNOME requires the AppIndicator extension — keep a non-tray path to the window |

### Internal Boundaries

| Boundary | Communication | Notes |
|----------|---------------|-------|
| UI ↔ core | `AppCommand` / `AppEvent` | The only way UI state changes; keeps the UI thread free |
| core ↔ provider | Trait methods returning `Result` | In-process, read-only, no privilege |
| core ↔ service | D-Bus methods + signals | The privilege boundary; every parameter validated on both sides |
| service ↔ kernel | Direct `std::fs` writes to whitelisted paths | Paths from constants; values from whitelists |
| core ↔ config | serde/TOML, atomic replace | Invalid file → safe defaults, original preserved |
| worker ↔ core | Snapshot publish (channel + shared ring buffer) | Never holds the UI lock while sampling |

## Sources

- User-provided ZenOmen specification §3–§9, §15 — the three-layer model, trait definitions, D-Bus interface, error codes, privilege model; HIGH (authoritative for this project)
- `.planning/codebase/ARCHITECTURE.md` (OpenOMEN) — the anti-patterns above are observed defects, not hypotheticals; HIGH
- https://security.opensuse.org/2024/04/02/kde6-dbus-polkit.html — standard D-Bus + polkit daemon arrangement on the system bus; HIGH
- https://docs.rs/zbus — service/client implementation pattern for Rust; HIGH
- https://slint.dev/blog/slint-1.17-released — `SystemTrayIcon` availability and the Linux D-Bus tray mechanism; HIGH
- https://docs.kernel.org/hwmon/sysfs-interface.html — hwmon attribute conventions and the warning that channel numbering is not guaranteed; HIGH

---
*Architecture research for: ZenOmen*
*Researched: 2026-10-30*
