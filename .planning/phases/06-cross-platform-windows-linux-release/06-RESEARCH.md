# Phase 6 Research: Unified Cross-Platform Release (Windows 10/11 + Linux Hardening)

**Phase:** 06 — Unified Cross-Platform Release (Windows 10/11 + Linux Hardening)  
**Researched:** 2026-11-07  
**Method:** Deep technical analysis of `.planning/codebase/*` (all 7 documents: STACK, INTEGRATIONS, ARCHITECTURE, STRUCTURE, CONVENTIONS, TESTING, CONCERNS), Linux kernel `hp-wmi.c` driver, Microsoft Win32/WMI/DXGI/PDH documentation, and the comprehensive cross-platform roadmap specification (`pasted-text-4.txt`).
**Status:** Complete & Fully Specified

---

## 1. Executive Summary & Codebase Context

ZenOmen is currently implemented as a two-process Rust (edition 2024) application for Linux:
1. `zenomen`: Unprivileged desktop GUI built with Slint (`backend-winit`, `renderer-femtovg`), owning an RSS budget of ≤40 MB and idle CPU ≤0.1%.
2. `zenomen-hw`: Privileged root daemon communicating over system D-Bus (`com.zenomen.Hardware1`), with verified sysfs writes behind Polkit authorization (`com.zenomen.hardware.control`).

The working tree at commit `745ba8d` has 123 automated tests passing, clean formatting, zero clippy warnings, and established type-level compilation readiness for Windows (`UnavailableHardwareProvider`, `winres` icon embedding, `--target x86_64-pc-windows-msvc --no-default-features`).

Per the user's directive, the entire cross-platform expansion and technical debt remediation is unified into **one single phase (Phase 6)**. This phase achieves two non-negotiable milestones:
1. **Complete Linux Hardening**: Resolving every remaining item in `.planning/codebase/CONCERNS.md` (D-Bus systemd activation delegation, restricted sysfs write paths, non-destructive autostart, RGB toggle profile restoration, session D-Bus absence shutdown safety, dynamic `CLK_TCK` tick calculation, and battery telemetry fields).
2. **Native Windows 10/11 Implementation**: Delivering a real, uncompromised Windows application (CPU/RAM/GPU/Battery native telemetry, secure local named pipe IPC, HP BIOS WMI mailbox driver for verified fan/power/RGB control, native notification area tray, single-instance mutex, and dual-OS packaging).

---

## 2. Windows Hardware Interface & HP WMI Specification

### 2.1 The HP BIOS WMI Mailbox
Unlike standard desktop hardware, HP OMEN and Victus laptops manage fan curves, ACPI platform performance profiles, thermal modes, and 4-zone keyboard lighting through an ACPI/WMI BIOS interface.

- **WMI Namespace:** `ROOT\WMI`
- **WMI Class:** `hpqBIntM`
- **HP BIOS WMI GUID:** `5FB7F034-2C63-45E9-BE91-3D44E2C707E4` (`HPWMI_BIOS_GUID`)
- **HP Firmware Event GUID:** `95F24279-4D7B-4334-9387-ACCDC67EF61C`
- **Event Class:** `ROOT\WMI: hpqBEvnt`
- **Execution Method:** WMI method `hpqBIntM.Execute` taking structured byte buffers:
  - Command Header: Category, Command ID, Input Buffer Size, Output Buffer Size.
  - Return Format: Status code in byte 0 (`0x00` = Success, `0x04` = Invalid Parameter, `0x05` = Unsupported).

### 2.2 Validated HP Command Catalogue
| Command ID | Command Category | Purpose | Operation | Expected Payload / Response |
| :--- | :--- | :--- | :--- | :--- |
| `0x10` | `0x20008` (Gaming) | Fan Count Query | Read | Returns number of physical fans (typically 2 for OMEN/Victus). |
| `0x11` | `0x20008` (Gaming) | Fan Speed Query | Read | Returns current RPM for CPU and GPU fans. |
| `0x1A` | `0x20008` (Gaming) | Performance Mode | Read/Write | Mode selector: Default (`0x00`), Performance (`0x01`), Cool (`0x02`), Quiet (`0x03`). |
| `0x21` | `0x20008` (Gaming) | GPU Thermal Mode | Read | Returns active dGPU thermal policy. |
| `0x22` | `0x20008` (Gaming) | GPU Thermal Setting | Write | Configures dGPU thermal target. |
| `0x26` | `0x20008` (Gaming) | Max Fan Status Query | Read | Returns whether Max fan mode (`1`) or Auto mode (`0`) is active. |
| `0x27` | `0x20008` (Gaming) | Max Fan Setting | Write | `0` = Auto fan curve, `1` = Maximum fan speed. |
| `0x28` | `0x20008` (Gaming) | System Design Data | Read | Reports thermal thresholds, board capabilities, and fan curve points. |
| `0x29` | `0x20008` (Gaming) | Power Limits | Write | Configures PL1 / PL2 power budgets on supported firmware. |
| `0x2D` | `0x20008` (Gaming) | Victus S Fan Speed | Read | Victus-specific fan speed read-back. |
| `0x2E` | `0x20008` (Gaming) | Victus S Fan Level | Write | Victus-specific multi-step fan level setting. |
| `0x20009` | `0x20009` (Lighting) | 4-Zone RGB Keyboard | Read/Write | 24-byte RGB zone payload (4 zones × 3 bytes RGB + animation mode flags). |

### 2.3 Device & Board Identity Verification
To protect hardware safety, the Windows hardware service must verify the machine's motherboard identity before attempting any WMI firmware write:
- **Motherboard Manufacturer:** `Win32_BaseBoard.Manufacturer` (must contain `"HP"` or `"Hewlett-Packard"`).
- **Motherboard Product ID:** `Win32_BaseBoard.Product` (e.g., `"88F8"`, `"8A24"`, `"8A3E"`).
- **System Model:** `Win32_ComputerSystem.Model` (must contain `"OMEN"` or `"Victus"`).
- **BIOS Version:** `Win32_BIOS.SMBIOSBIOSVersion`.

**Safety Rule:** If the motherboard product ID is not present in the verified board profile catalogue, the service must report `fan_control = false` and refuse mutating firmware writes, degrading to read-only telemetry.

---

## 3. Native Windows Telemetry APIs

The Windows telemetry engine under `src/hardware/windows/` must use native OS C/COM APIs directly without invoking shell commands (`powershell.exe`, `wmic.exe`, `cmd.exe`).

### 3.1 CPU Utilization, Frequency & Model
- **Utilization:** `GetSystemTimes(&idle_time, &kernel_time, &user_time)` (`FILETIME`). Calculate percentage via:
  $$\Delta \text{Total} = (\Delta \text{Kernel} + \Delta \text{User})$$
  $$\text{Utilization} \% = 100.0 \times \left(1.0 - \frac{\Delta \text{Idle}}{\Delta \text{Total}}\right)$$
- **Frequency:** `CallNtPowerInformation(ProcessorInformation, ...)` returning `PROCESSOR_POWER_INFORMATION` (`CurrentMhz`, `MaxMhz`).
- **Model Name:** `Win32_Processor.Name` via WMI, or registry `HARDWARE\DESCRIPTION\System\CentralProcessor\0\ProcessorNameString`.

### 3.2 Memory Telemetry
- **API:** `GlobalMemoryStatusEx(&mem_status)` (`MEMORYSTATUSEX`).
- **Calculation:**
  - Total Physical RAM: `mem_status.ullTotalPhys / (1024 * 1024)` MB.
  - Available Physical RAM: `mem_status.ullAvailPhys / (1024 * 1024)` MB.
  - Used Physical RAM: `(mem_status.ullTotalPhys - mem_status.ullAvailPhys) / (1024 * 1024)` MB.

### 3.3 GPU Telemetry & Video Memory
- **Adapter Enumeration:** DXGI 1.3 `CreateDXGIFactory1`, `IDXGIFactory1::EnumAdapters1`, `IDXGIAdapter1::GetDesc1`.
  - Filter out Microsoft Basic Render Driver (`adapter_desc.Flags & DXGI_ADAPTER_FLAG_SOFTWARE`).
  - Extract Dedicated Video Memory: `adapter_desc.DedicatedVideoMemory / (1024 * 1024)` MB.
  - Extract Vendor ID (NVIDIA: `0x10DE`, AMD: `0x1002`, Intel: `0x8086`).
- **Engine Utilization:** PDH (Performance Data Helper) query on `\GPU Engine(*)\Utilization Percentage`:
  - `PdhOpenQueryW`, `PdhAddCounterW`, `PdhCollectQueryData`, `PdhGetFormattedCounterValue`.
  - Aggregate engine utilization by physical adapter LUID.

### 3.4 Battery Telemetry
- **API:** `GetSystemPowerStatus(&power_status)` (`SYSTEM_POWER_STATUS`).
- **Fields:**
  - `present = power_status.BatteryFlag != 128` (128 = No system battery).
  - `charge_percent = if power_status.BatteryLifePercent <= 100 { Some(power_status.BatteryLifePercent) } else { None }`.
  - `charging = (power_status.BatteryFlag & 8) != 0` (8 = Charging).
  - `remaining_seconds = if power_status.BatteryLifeTime != 0xFFFFFFFF { Some(power_status.BatteryLifeTime) } else { None }`.

---

## 4. Windows Process Architecture & IPC Security

```text
┌────────────────────────────────────────────────────────┐
│ Process A: zenomen.exe (Unprivileged Desktop GUI)      │
│  - Slint UI (AppWindow, 6 screens, dark theme)         │
│  - src/core/worker.rs (1 Hz telemetry sampling)        │
│  - src/hardware/windows/provider.rs (Native Win32 APIs)│
│  - Unprivileged interactive user permissions           │
└───────────────────────────┬────────────────────────────┘
                            │
                            │ Local Named Pipe IPC
                            │ \\.\pipe\ZenOmen.Hardware.v1
                            │ Restricted Session DACL
                            │ Versioned JSON-RPC protocol
                            ▼
┌────────────────────────────────────────────────────────┐
│ Process B: zenomen-hw.exe (Windows Service / Elevated) │
│  - Service Control Manager (SCM) lifecycle             │
│  - Named pipe server with client session verification  │
│  - Verified board profile catalogue                    │
│  - HP BIOS WMI Mailbox driver (hpqBIntM / Execute)     │
│  - Read-back verification and honest failure reporting │
└────────────────────────────────────────────────────────┘
```

### 4.1 Named Pipe Security & Client Authentication
- **Pipe Name:** `\\.\pipe\ZenOmen.Hardware.v1`
- **Security Descriptor:** Custom security descriptor built with `InitializeSecurityDescriptor`, assigning a DACL with:
  - `GENERIC_ALL` for `LocalSystem` and `Administrators`.
  - `GENERIC_READ | GENERIC_WRITE` for the interactive desktop user (`Interactive User SID` `S-1-5-4`).
  - Network logon (`Network SID` `S-1-5-2`) explicitly denied.
- **Session Verification:** On connection, call `ImpersonateNamedPipeClient`, query thread token via `OpenThreadToken` and `GetTokenInformation(TokenSessionId)`, call `RevertToSelf`, and verify client `SessionId == WTSGetActiveConsoleSessionId()`. Reject cross-session or remote callers immediately.

---

## 5. UI Architecture: Slint Retained for Cross-Platform Parity

While the external roadmap suggested GPUI-CE, ADR-001 ([`docs/decisions/ADR-001-ui-framework.md`](file:///home/omprakash/Documents/zenomen/docs/decisions/ADR-001-ui-framework.md)) demonstrated that:
1. ZenOmen's complete presentation layer (20 `.slint` files, 6 screens, status strips, telemetry charts, dark theme) is already written, tested, and adheres strictly to the ≤40 MB RSS budget.
2. Slint 1.18 compiles out-of-the-box on Windows (`backend-winit`, `renderer-femtovg`) with Direct3D/OpenGL hardware acceleration.
3. GPUI-CE is currently pre-1.0 (v0.3.x, unstable, yanked point releases, requires Rust 1.95+). Rewriting all 20 screens as imperative Rust element trees introduces immense regression risk without functional benefit.
4. **Decision:** Retain Slint for both Linux and Windows. The domain and hardware provider seams are abstracted so the UI layer consumes identical typed `AppEvent` messages regardless of operating system.

---

## 6. Linux Concerns Reconciliation Inventory

Every item from `.planning/codebase/CONCERNS.md` is addressed in Wave 1:
1. **D-Bus Systemd Service Activation:** Bind `SystemdService=zenomen-hw.service` in `data/dbus/com.zenomen.Hardware1.service`.
2. **Systemd Unit Sandboxing:** Add `ReadWritePaths=-/sys/devices/platform/hp-wmi -/sys/class/hwmon` in `data/systemd/zenomen-hw.service`.
3. **RGB State Preservation:** Store last valid `RgbProfile` in `AppState` / `HardwareService`; restore previous colors on `SetRgbEnabled(true)`.
4. **Session D-Bus Disconnect Hang:** Explicit `drop(action_sender)` in `src/main.rs` when `ensure_single_instance` reports `Disabled`.
5. **Autostart Preservation:** Toggle `Hidden=true` or `X-GNOME-Autostart-enabled=false` in `src/desktop/autostart.rs` instead of deleting file.
6. **Dynamic Tick Rate in Benchmark:** Query `getconf CLK_TCK` via `sysconf` instead of hardcoding `USER_HZ = 100` in `src/core/benchmark.rs`.
7. **Offline Guidance Path:** Replace developer build paths (`target/debug/...`) with `sudo systemctl start zenomen-hw` in `src/service/client.rs`.
8. **SensorRegistry Root Isolation:** Pass configured root into `detect_rgb_interface` in `src/hardware/registry.rs`.
9. **Fan Acceptance Predicate Alignment:** Align `SensorRegistry` fan discovery to `hp-wmi` / `hp` hwmon drivers matching `backend.rs`.
10. **History Window Units:** Convert configured seconds to sample counts based on `polling_interval_ms` in `src/core/worker.rs`.
