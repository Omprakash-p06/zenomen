# Architecture Research

**Domain:** Native desktop system utility with a privileged hardware backend  
**Researched:** 2026-10-30  
**Updated:** 2026-10-30 (incorporated review corrections: two-process model, D-Bus contract, sensor registry, preset results, memory benchmarks)  
**Confidence:** HIGH for the layered model, privilege split, and D-Bus contract; MEDIUM-HIGH for Slint concurrency primitives and memory budgets

## Standard Architecture

The established pattern for a hardware control GUI on Linux is a **two-process application architecture with three logical layers**:
1. **Desktop UI Process (`zenomen-gui`):** An unprivileged process running as the logged-in desktop user. It hosts both the compiled Slint presentation layer and the Rust application core (state management, deadline-scheduled monitoring worker, bounded telemetry ring buffer, and direct read-only sysfs/procfs sampling). It owns the strict **≤40 MB peak RSS** memory budget.
2. **Privileged Hardware Service (`zenomen-hw`):** A small, standalone root daemon running as a systemd service on the system D-Bus (`com.zenomen.Hardware1`). It owns all privileged hardware writes, enforces mandatory polkit authorization, validates all arguments against strict schemas, writes to sysfs, and executes read-back verification. It has its own independent memory budget (**≤15 MB RSS**), reported separately and never summed into the UI process figure.
3. **Logical Interface Layer (Linux Kernel):** The underlying kernel and ACPI interfaces (`/sys/class/hwmon`, `/sys/firmware/acpi`, `/sys/class/drm`, `/proc`). The kernel is an operating system interface layer, not a third application process.

### System Overview

```text
┌─────────────────────────────────────────────────────────────────────────┐
│  Desktop Process: zenomen-gui (Unprivileged, Normal Desktop User)       │
│  Budget: ≤40 MB Peak RSS, ≤0.1% Idle CPU                                │
│                                                                         │
│  ┌───────────────────────────────────────────────────────────────────┐  │
│  │  Presentation Layer — Slint (.slint markup, compiled)             │  │
│  │  Overview · Cooling · Performance · Lighting · Settings ·         │  │
│  │  Diagnostics      (system tray + single-instance window)          │  │
│  │  Never reads /sys, never writes files, never calls sudo           │  │
│  └─────────────────────────────────┬─────────────────────────────────┘  │
│                                    │  AppCommand ──▼ / ▲── AppEvent     │
│  ┌─────────────────────────────────┴─────────────────────────────────┐  │
│  │  Application Core — Rust                                          │  │
│  │  ┌────────────┐ ┌────────────┐ ┌─────────────┐ ┌──────────────┐   │  │
│  │  │State mgr   │ │Scheduler / │ │Telemetry    │ │Preset mgr    │   │  │
│  │  │(AppState)  │ │poll worker │ │buffer (120) │ │(PresetResult)│   │  │
│  │  └────────────┘ └────────────┘ └─────────────┘ └──────────────┘   │  │
│  │  ┌────────────┐ ┌────────────┐ ┌─────────────┐ ┌──────────────┐   │  │
│  │  │Config      │ │Diagnostics │ │SingleInstance│ │SensorRegistry│  │
│  │  │(TOML)      │ │export      │ │(D-Bus focus)│ │(shared)      │   │  │
│  │  └────────────┘ └────────────┘ └─────────────┘ └──────────────┘   │  │
│  └─────────────────┬───────────────────────────────┬─────────────────┘  │
│                    │                               │                    │
│                    │ HardwareProvider (in-process) │ D-Bus client       │
│                    │ Read-only telemetry: sysfs/   │ zbus               │
│                    │ /proc/meminfo/nvidia-smi      │ com.zenomen.       │
│                    │ (Zero root privilege needed)  │ Hardware1          │
└────────────────────┼───────────────────────────────┼────────────────────┘
                     │                               │ System D-Bus
                     │                               ▼
                     │       ┌────────────────────────────────────────────┐
                     │       │ Privileged Service: zenomen-hw             │
                     │       │ (Separate Process, Root Daemon, systemd)   │
                     │       │ Budget: ≤15 MB RSS (Reported Separately)   │
                     │       │ - Mandatory polkit authorization           │
                     │       │ - Whitelisted operations only              │
                     │       │ - Schema validation (no paths/commands)    │
                     │       │ - Write + read-back retry verification     │
                     │       └───────────────────────┬────────────────────┘
                     │                               │
                     ▼ Direct read-only              ▼ Privileged writes only
┌────────────────────────────────────────────────────┴────────────────────┐
│  Logical Layer: Linux Kernel Interfaces                                 │
│  /sys/class/hwmon · /sys/firmware/acpi · /sys/class/drm · /proc/meminfo │
└─────────────────────────────────────────────────────────────────────────┘
```

### Component Responsibilities

| Component | Responsibility | Process | Implementation Details |
|-----------|----------------|---------|------------------------|
| Slint presentation | Render state, collect user actions, display capability gaps and honest "Unavailable" states | `zenomen-gui` | Compiled `.slint` via `slint-build`; models bound to core structs; **zero direct hardware access** |
| Application core | State machine, scheduling, normalization, bounded history, config persistence, diagnostics | `zenomen-gui` | Pure Rust, standard threads; owns application lifecycle |
| Shared `SensorRegistry` | Single source of truth for detecting and resolving temperature, fan, and GPU sensors | `zenomen-gui` | Module-level priority discovery; dashboard and diagnostics share this exact registry |
| Measurement (read) path | Telemetry that needs no privilege (CPU, GPU, RAM, temps, fan RPM, active profiles) | `zenomen-gui` | **In-process**, direct sysfs/`/proc` reads — no D-Bus overhead for periodic polling |
| Monitoring worker | One long-lived thread sampling on deadlines (1s CPU, 2s GPU, 5s RAM/power) | `zenomen-gui` | Publishes `TelemetrySnapshot` to core; keeps prior counters for delta math (no sleeps) |
| Privileged service | The sole authorized writer to hardware sysfs files | `zenomen-hw` | Standalone root daemon on system bus; mandatory polkit check; verified read-back |
| Single-instance activator | Ensures only one UI process runs; refocuses existing window on duplicate launch | `zenomen-gui` | Registers `com.zenomen.App` on session bus; fallback for GNOME without tray icons |

---

## Formal D-Bus Interface Contract

The privileged daemon `zenomen-hw` claims the well-known bus name `com.zenomen.Hardware1` at object path `/com/zenomen/Hardware1` on the system bus.

### Methods

| Method | Signature | Description | Authorization |
|--------|-----------|-------------|---------------|
| `GetCapabilities` | `() -> (s: json)` | Returns structured JSON of verified hardware capabilities on this machine | Allowed (read-only query) |
| `GetFanStatus` | `() -> (s: mode, u: rpm1, u: rpm2)` | Reads current fan mode ("auto", "max", "manual") and live RPMs | Allowed (read-only query) |
| `SetFanMode` | `(s: mode) -> (b: success, s: actual_mode)` | Validates mode ("auto" or "max"), writes `pwm1_enable`, verifies via read-back | **Mandatory Polkit** (`com.zenomen.hardware.control`) |
| `GetPowerProfiles` | `() -> (as: choices, s: active)` | Returns advertised ACPI profile choices and currently active profile | Allowed (read-only query) |
| `GetPowerProfile` | `() -> (s: active)` | Reads active ACPI power profile from `platform_profile` | Allowed (read-only query) |
| `SetPowerProfile` | `(s: profile) -> (b: success, s: actual_profile)` | Validates profile against advertised choices, writes sysfs, verifies via read-back | **Mandatory Polkit** (`com.zenomen.hardware.control`) |
| `ApplyRgbProfile` | `(a{ss}: zones) -> (b: success, s: message)` | Validates zone color map (keys "0".."3", values `#RRGGBB`), writes to detected RGB sysfs, verifies if supported | **Mandatory Polkit** (`com.zenomen.hardware.control`) |
| `SetRgbEnabled` | `(b: enabled) -> (b: success)` | Enables or disables keyboard backlight sysfs, verifies via read-back | **Mandatory Polkit** (`com.zenomen.hardware.control`) |

### Polkit Authorization Policy

Polkit authorization is **mandatory** for every mutating operation (`SetFanMode`, `SetPowerProfile`, `ApplyRgbProfile`, `SetRgbEnabled`). It is not optional.
- Action identifier: `com.zenomen.hardware.control`
- Description: "Authorize ZenOmen to adjust laptop hardware cooling, power, or lighting settings"
- Default policy:
  - Any desktop user: `auth_admin_keep` (prompts once per session/timeout)
  - Inactive / remote sessions: `auth_admin`
- The service verifies the caller's credentials using `zbus::message::Header` sender credentials against `org.freedesktop.PolicyKit1.Authority`.

### Error Contract

The service returns standard D-Bus error replies. The UI maps these directly into typed `HardwareError` variants:
- `com.zenomen.Hardware1.Error.Unauthorized`: Polkit authorization was rejected or cancelled by the user.
- `com.zenomen.Hardware1.Error.InvalidParameter`: Argument rejected by schema (e.g. unknown fan mode, unadvertised power profile, malformed hex color).
- `com.zenomen.Hardware1.Error.UnsupportedOperation`: Control not supported on this specific machine/firmware.
- `com.zenomen.Hardware1.Error.HardwareNotFound`: The required sysfs path or kernel driver is absent.
- `com.zenomen.Hardware1.Error.WriteFailed`: Low-level I/O error occurred while writing to sysfs.
- `com.zenomen.Hardware1.Error.VerificationFailed`: Write completed without I/O error, but read-back verification did not match the requested state after retries.
- `com.zenomen.Hardware1.Error.Timeout`: Hardware operation exceeded timeout window.

### Timeouts, Retries, and Disconnection Policy

1. **Timeout:** All D-Bus calls enforce a client-side timeout of 5000 ms.
2. **Read-back Retry Policy:** When writing to sysfs, embedded controllers can take tens of milliseconds to reflect state. The service implements:
   - Initial read-back after 100 ms.
   - If state does not match, retry with exponential backoff (200 ms, then 400 ms) for up to 3 attempts total.
   - If still mismatched, returns `com.zenomen.Hardware1.Error.VerificationFailed`.
3. **Disconnection Handling:** If `zenomen-hw` is stopped, crashed, or uninstalled:
   - The UI catches D-Bus disconnection gracefully without panicking or stalling.
   - Mutating UI controls (fan toggle, power profile selector, lighting toggle) are set to disabled with an explanatory banner ("Hardware service unavailable — controls disabled").
   - Read-only telemetry continues functioning completely uninterrupted via in-process sysfs reads.

### Security Boundary Guarantees

- **No arbitrary paths:** The service builds all sysfs paths from compiled constants and validated capability identifiers. Never accepts a path string from D-Bus.
- **No shell commands:** The service never invokes a shell, `sh -c`, `system()`, or external processes. All writes are performed via direct `std::fs` operations.
- **Strict parameter parsing:** Enums are checked against strict string whitelists; RGB colors are validated as exact `#RRGGBB` hex strings before opening any file.

---

## Single Shared Sensor Registry

In the legacy Python CLI, `read_temperature()` and `run_doctor()` implemented duplicate, diverging sensor selection logic, occasionally identifying different sensors on the same machine. In ZenOmen, all sensor resolution is centralized in a single shared registry:

```rust
pub struct SensorInfo {
    pub driver: String,
    pub label: Option<String>,
    pub path: std::path::PathBuf,
    pub sensor_type: SensorType,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SensorType {
    CpuTemperature,
    GpuTemperature,
    FanSpeed,
    Other,
}

pub struct SensorRegistry {
    pub sensors: Vec<SensorInfo>,
}
```

### Sensor Resolution Rules
- **Never trust indices:** Never assume `hwmon0` is CPU or `temp1_input` is CPU temperature.
- **Priority heuristics:** Drivers are prioritized (`coretemp` > `k10temp` > `zenpower` > `hp-wmi` > `acpitz`); labels are matched against allowlists (`Package id 0`, `Tctl`, `CPU`) and denylists.
- **Dynamic DRM resolution:** Enumerate `/sys/class/drm/card*` dynamically; never hardcode `card0` for Intel or any other GPU vendor.
- **Shared consumption:** Both live dashboard telemetry and Diagnostics export read from this exact same `SensorRegistry`.

---

## Fan Control Model: Three States vs. Validated Modes

The Linux kernel `pwm1_enable` sysfs attribute defines three possible states:
- `0`: Maximum (boost)
- `1`: Manual (PWM duty cycle control)
- `2`: Automatic (firmware / BIOS default control)

### Policy for v1
1. **Reporting:** When reading `pwm1_enable`, ZenOmen accurately distinguishes `0` (Maximum), `1` (Manual), and `2` (Automatic). If external software or kernel patches put the fan into state `1`, ZenOmen displays "Manual (external/kernel control)" instead of collapsing it into "Unknown".
2. **Mutating Control:** For v1, ZenOmen only offers switching between validated modes: **Automatic** (`2`) and **Maximum** (`0`).
3. **No Unvalidated Manual Mode:** Writing `1` to `pwm1_enable` without hardware-supported PWM duty curve control can stall fans or trigger embedded controller faults. Manual mode is **never offered in the UI** merely because the numeric value `1` exists in the kernel documentation.

---

## Granular Preset Execution (`PresetResult`)

Combined presets (Gaming, Balanced, Quiet) adjust both the ACPI power profile and the fan mode. Because these are two independent kernel/EC operations, they are **not atomic** and cannot be rolled back safely if one fails.

ZenOmen replaces false claims of "transactionality" with explicit, granular reporting:

```rust
pub struct PresetResult {
    pub power_applied: bool,
    pub fan_applied: bool,
    pub partial_success: bool,
}
```

### UI Presentation
- **Full Success:** Both power profile and fan mode applied and verified.
- **Partial Application (Power Only):** Power profile applied, but fan mode write or verification failed. UI shows: "Power profile set to Performance, but fan mode could not be changed."
- **Partial Application (Fan Only):** Fan mode set to Maximum, but ACPI power profile failed. UI shows: "Fan mode set to Maximum, but power profile could not be changed."
- **Total Failure:** Neither setting could be applied.

---

## Memory Budget & Benchmarking Methodology

The product requirement is **≤40 MB peak RSS** for the UI process and **≤0.1% idle CPU** on the dashboard.

### Memory Categorization
ZenOmen's benchmarking distinguishes six distinct memory metrics:
1. **Cold Startup RSS:** Measured immediately after initial window map and initial mock/telemetry render (< 30 MB).
2. **Warm Dashboard RSS:** Measured after 10 minutes of active 1 Hz telemetry sampling with rolling charts populated (< 38 MB).
3. **Minimized Tray RSS:** Measured 5 minutes after closing the window to tray, with worker polling throttled (< 25 MB).
4. **Peak RSS:** Maximum RSS observed during interactive page switching, manual refreshes, and diagnostics generation over a 2-hour session (≤ 40 MB).
5. **Hardware Service RSS (`zenomen-hw`):** Measured independently (< 15 MB RSS). Never summed into the UI process figure.
6. **Proportional Set Size (PSS) & Private Dirty:** Telemetry benchmarks log `Private_Dirty` and PSS from `/proc/<pid>/smaps_rollup` alongside RSS to isolate shared library pages from actual application footprint.

### Measurement Procedure
- **Automated Memory Test:** A headless/scripted test run via `tests/benchmarks/memory_test.rs` logging `/proc/<pid>/smaps_rollup` at 10-second intervals for 2 hours.
- **Profiling Tooling:** `heaptrack` and `valgrind --tool=massif` used during Phase 5 optimization to ensure Slint model updates mutate in place without heap accumulation.

---

## Single-Instance & Desktop Integration (GNOME Fallback)

On Linux desktops, StatusNotifierItem (D-Bus tray) support is not universal. Specifically, stock GNOME does not display tray icons without third-party extensions (e.g. AppIndicator).

### Dual-Channel Accessibility
1. **Session Bus Registration:** On startup, `zenomen-gui` attempts to claim the well-known name `com.zenomen.App` on the D-Bus session bus.
2. **Duplicate Launch Detection:** If the name is already claimed:
   - The new process issues a D-Bus method call `PresentWindow()` to `com.zenomen.App`.
   - The running instance brings its main window to the foreground, restores it if minimized, and requests window focus.
   - The new process exits immediately with code 0.
3. **No Lost Windows:** A user on stock GNOME who closes to tray can simply re-launch ZenOmen from the application menu or terminal to regain immediate access to the window.
4. **Desktop Environments Tested:** GNOME (with and without extension), KDE Plasma, and X11 (XFCE / i3).

---

## Key Anti-Patterns to Avoid

1. **UI Reads Sysfs Directly:** Bypasses provider abstraction, blocks the Slint UI thread, and breaks mock testing.
2. **Blocking Operations on Telemetry Path:** A single 500 ms sleep on `/proc/stat` stalls the UI thread. Worker thread must compute deltas from prior counters.
3. **Hardcoding Hardware Paths:** Hardcoding `temp1_input`, `hwmon0`, or `card0` fails across laptops. Dynamic capability detection is required.
4. **Optimistic Hardware Writes:** Showing success before reading back the value is a safety hazard. Always verify with retry/backoff.
5. **Claiming Atomic Transactions:** Fan and power profiles are separate subsystems; always report partial application honestly.
6. **Polkit Optionality:** Describing root authorization as optional or running GUI via sudo creates severe privilege escalation vectors. Polkit is mandatory.

---
*Architecture research for: ZenOmen*  
*Updated: 2026-10-30*
