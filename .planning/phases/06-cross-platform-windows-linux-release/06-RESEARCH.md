# Phase 6 Research: Unified Cross-Platform Release (Windows 10/11 + Linux Hardening)

**Phase:** 06 — Unified Cross-Platform Release (Windows 10/11 + Linux Hardening)  
**Researched:** 2026-11-07  
**Method:** Direct codebase analysis (`.planning/codebase/*`, `src/`, `tests/`), Windows API documentation, Linux kernel driver sources (`hp-wmi`), and cross-platform proposal specification (`pasted-text-3.txt`).  
**Status:** Complete

---

## 1. Executive Summary

ZenOmen currently runs as a production-grade, two-process Rust hardware control center on Linux (123 verified tests, Slint GUI, root daemon, Polkit authorization). While Windows compilation readiness was established at the type level (`UnavailableHardwareProvider`, `--target x86_64-pc-windows-msvc --no-default-features`, embedded `.ico` resources), running on Windows 10/11 requires implementing real native hardware telemetry, a secure local IPC and Windows service architecture, HP WMI firmware control, native desktop integration, and closing the remaining Linux concerns recorded in `.planning/codebase/CONCERNS.md`.

Per the user's explicit directive, the entire cross-platform release programme is unified into **one single phase (Phase 6)**, decomposed into five sequential, dependency-aware execution waves:
1. **Wave 1 (`06-01-PLAN.md`)**: Baseline, Linux Hardening & Concerns Remediation
2. **Wave 2 (`06-02-PLAN.md`)**: Cross-Platform Domain Contracts & Provider Seams
3. **Wave 3 (`06-03-PLAN.md`)**: Native Windows Telemetry Engine (`src/hardware/windows/`)
4. **Wave 4 (`06-04-PLAN.md`)**: Windows Privileged Hardware Service (`zenomen-hw-win`) & Named-Pipe IPC
5. **Wave 5 (`06-05-PLAN.md`)**: Windows Desktop Integration, Dual-OS Packaging & CI Matrix

---

## 2. Windows Hardware Interface & HP WMI Architecture

### 2.1 HP BIOS WMI Mailbox
Unlike standard desktop PCs, HP OMEN and Victus laptops expose fan control, ACPI performance profiles, and 4-zone RGB keyboard control through an ACPI/WMI BIOS interface. On Linux, this is driven by `hp-wmi.c`. On Windows, the HP BIOS WMI object is registered in WMI under `root\wmi`:
- **WMI Class:** `hpqBIntM` (or HP BIOS interface equivalent)
- **WMI GUID:** `5FB7F034-2C63-45e9-BE91-3D44E2C707E4` (`HPWMI_BIOS_GUID`)
- **Execution Mechanism:** WMI method call (`hpqBIntM.Execute`) passing a structured binary buffer:
  - Header: Command Category (e.g., `0x03` for thermal/fan, `0x02` for power, `0x01` for system), Command ID, Input Buffer Size, Output Buffer Size.
  - Return: Status code (`0x00` = Success, `0x04` = Invalid Parameter, `0x05` = Unsupported).

### 2.2 Control Catalog
| Subsystem | Linux Mechanism | Windows WMI / API Mechanism | Fallback / Safety |
| :--- | :--- | :--- | :--- |
| **Fan Control** | `/sys/devices/platform/hp-wmi/fan_mode` | HP BIOS WMI Thermal query (`0x03` / `0x2E` fan mode set) | Return `None` / `InterfaceMissing` if board ID does not match catalog |
| **Power Profiles** | `/sys/firmware/acpi/platform_profile` | HP BIOS WMI Performance Mode (`0x03` / `0x1B` or Windows ACPI profile) | Read-back verification; dual-leg failure honesty |
| **4-Zone RGB** | Four `/sys/class/leds/*` brightness | HP WMI Lighting Mailbox / Raw HID report | Refuse writes if board does not report RGB feature |
| **CPU Telemetry** | `/proc/stat`, `/proc/cpuinfo` | `GetSystemTimes`, `CallNtPowerInformation` | Honest percentage calculation |
| **RAM Telemetry** | `/proc/meminfo` (`MemAvailable`) | `GlobalMemoryStatusEx` (`ullAvailPhys`, `ullTotalPhys`) | Exact physical memory math |
| **GPU Telemetry** | DRM sysfs + `NvidiaSampler` | DXGI 1.3 `QueryVideoMemoryInfo` + PDH `GPU Engine` | Distinguish per-process from adapter-wide |
| **Battery** | `/sys/class/power_supply/BAT*` | `GetSystemPowerStatus` + WMI `Win32_Battery` | Real presence, percentage, and AC state |

---

## 3. Windows Process & Security Architecture

### 3.1 Privilege Boundary
- **GUI Process (`zenomen.exe`)**: Runs as a standard, unprivileged desktop user. Never runs as Administrator. Never executes raw WMI firmware mutation methods directly.
- **Hardware Service (`zenomen-hw.exe`)**: Windows Service registered with Service Control Manager (SCM), running under `LocalSystem` (or dedicated service account with WMI access permissions).

### 3.2 Named Pipe IPC
- **Endpoint:** `\\.\pipe\ZenOmen.Hardware.v1`
- **Security:** Explicit DACL assigned at creation (`CreateNamedPipeW`) granting access only to the interactive console user session (`GetNamedPipeClientSessionId`). Rejects network and remote callers.
- **Protocol:** Strict versioned JSON RPC matching the existing Linux contract (`GetCapabilities`, `GetFanStatus`, `SetFanMode`, `GetPowerProfiles`, `SetPowerProfile`, `ApplyPreset`, `ApplyRgbProfile`, `SetRgbEnabled`).

---

## 4. UI Framework Strategy (Slint vs GPUI-CE)

### 4.1 Findings & Recommendation
- **Current State:** ZenOmen has 20 complete, verified `.slint` components, 6 screens, and an optimized Rust bridge (`src/app.rs`). Slint compiles with `backend-winit` on Windows and Linux, respecting the strict ≤40 MB peak RSS budget.
- **GPUI-CE Evaluation:** GPUI-CE is currently at v0.3.x (unstable, breaking changes across versions, Rust 1.95+ requirement). Completely rewriting 20 screens and all components into imperative Rust element trees poses severe regression risk to an existing working application.
- **Strategy:** Retain Slint's production-ready cross-platform engine using `backend-winit` + `renderer-femtovg` (with DirectX/OpenGL acceleration on Windows), while decoupling domain contracts so any future UI migration can occur without modifying core hardware logic.

---

## 5. Linux Remediation & Tech Debt Closure

Before expanding the cross-platform surface, the remaining technical debt items identified in `.planning/codebase/CONCERNS.md` must be addressed:
1. `data/dbus/com.zenomen.Hardware1.service`: Add `SystemdService=zenomen-hw.service` delegation.
2. `data/systemd/zenomen-hw.service`: Add `ReadWritePaths=-/sys/devices/platform/hp-wmi -/sys/class/hwmon`.
3. `src/service/contract.rs`: Restore previous RGB color on `SetRgbEnabled(true)` instead of destroying the active profile with hardcoded white.
4. `src/main.rs`: Prevent shutdown hang when session D-Bus is unavailable by dropping `action_sender` explicitly.
5. `src/desktop/autostart.rs`: Non-destructive autostart toggle via `Hidden=true` / `X-GNOME-Autostart-enabled=false`.
6. `src/core/benchmark.rs`: Dynamically query system clock tick rate (`getconf CLK_TCK`) instead of hardcoding `USER_HZ = 100`.
7. `src/hardware/provider.rs`: Expand `TelemetrySnapshot` with real battery percentage and charging state.

---

## 6. Implementation Wave Plan Overview

| Plan | Scope | Target Files | Primary Truth / Deliverable |
| :--- | :--- | :--- | :--- |
| **06-01** | Linux Hardening & Concerns Remediation | `src/service/`, `data/`, `src/desktop/`, `src/main.rs`, `src/core/` | All open `CONCERNS.md` issues resolved; 100% Linux security & stability verified |
| **06-02** | Cross-Platform Domain Contracts | `src/hardware/provider.rs`, `src/core/`, `src/hardware/registry.rs` | Abstracted multi-platform telemetry & control contracts with strict test isolation |
| **06-03** | Windows Native Telemetry Engine | `src/hardware/windows/`, `src/hardware/mod.rs` | Real CPU, RAM, GPU, battery & thermal telemetry on Windows without fake zeros |
| **06-04** | Windows Hardware Service & Named Pipe | `src/service/windows/`, `src/bin/zenomen-hw.rs` | Local named-pipe IPC and HP BIOS WMI firmware control service |
| **06-05** | Windows Desktop Integration & Packaging | `src/desktop/windows/`, `Cargo.toml`, `.github/workflows/` | Native Windows tray, single instance, user autostart, and automated release packaging |
