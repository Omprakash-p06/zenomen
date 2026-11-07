# Phase 6 Research: Unified Cross-Platform Release (Windows 10/11 + Linux Hardening)

**Phase:** 06 — Unified Cross-Platform Release (Windows 10/11 + Linux Hardening)
**Researched:** 2026-11-07
**Method:** Deep technical analysis of `.planning/codebase/*` (STACK, INTEGRATIONS, ARCHITECTURE, STRUCTURE, CONVENTIONS, TESTING, CONCERNS), Linux kernel `hp-wmi.c` driver, Microsoft Win32/WMI/DXGI/PDH documentation, community firmware research, and the cross-platform specification (`pasted-text-5.txt`).
**Status:** Complete & Exhaustively Specified

---

## 1. Windows 10/11 Support Contract & Target Matrix

ZenOmen targets a native, production-grade cross-platform application across Linux, Windows 10, and Windows 11.

| Operating System | Target Specification | Servicing & Lifecycle Constraints |
| :--- | :--- | :--- |
| **Windows 10** | 22H2, x64, Build 19045 | End of general support: 14 October 2025. Consumer Extended Security Updates (ESU) through 12 October 2027. ZenOmen remains fully compatible with 22H2 without advertising general OS support. No Windows 11-only API is mandatory for core functionality. |
| **Windows 11** | 24H2, 25H2, and 26H2, x64 | Primary current test targets. Fully supported across modern hardware schedulers, Direct3D/DXGI versions, and power frameworks. |
| **Linux** | Ubuntu 22.04 LTS / 24.04 LTS, Debian 12, Fedora 39+, Arch Linux | Fully maintained primary platform with system D-Bus and systemd privilege isolation. |
| **Architecture** | x86_64 (`x86_64-pc-windows-msvc`, `x86_64-unknown-linux-gnu`) | Initial architecture target. |
| **UI Framework** | Slint (v1.18.1) with GPUI-CE (v0.3.0) parity migration track | Win32 + DirectX on Windows, Wayland + X11 on Linux. Strictly upholding ≤40 MB peak RSS and ≤0.1% idle CPU. |
| **Hardware Broker** | Elevated Windows Service (`zenomen-hw.exe`) / Linux systemd daemon (`zenomen-hw`) | Local named pipes on Windows (`\\.\pipe\ZenOmen.Hardware.v1`); System D-Bus on Linux (`com.zenomen.Hardware1`). |
| **Packaging** | WiX MSI Installer on Windows; DEB + AppImage on Linux | Clean install, service registration, upgrade, and removal without leaving orphaned system files. |

---

## 2. Windows Hardware Interfaces & HP WMI Firmware Mailbox

Hardware communication on Windows requires zero TCP listeners. All interfaces are local:
- Win32 Native APIs
- Performance Data Helper (PDH)
- DirectX Graphics Infrastructure (DXGI)
- Windows Management Instrumentation (WMI)
- HP Firmware ACPI/WMI BIOS Interface (`hpqBIntM`)
- Local Named-Pipe IPC to the privileged Windows Service

### 2.1 HP OMEN / Victus Firmware Interface
On Windows, HP laptops expose thermal, power, and lighting controls through ACPI/WMI BIOS objects registered under `ROOT\WMI`:
- **WMI Namespace:** `ROOT\WMI`
- **WMI Class:** `hpqBIntM`
- **HP BIOS WMI GUID:** `5FB7F034-2C63-45E9-BE91-3D44E2C707E4` (`HPWMI_BIOS_GUID`)
- **HP Firmware Event GUID:** `95F24279-4D7B-4334-9387-ACCDC67EF61C`
- **HP Event Class:** `ROOT\WMI: hpqBEvnt`

Both GUIDs, command signatures, and data buffer contracts are independently confirmed by the Linux kernel's `drivers/platform/x86/hp/hp-wmi.c` and community firmware research (Ohman project).

### 2.2 Validated HP Firmware Command Catalogue
The HP Gaming command family is identified by category code `0x20008` (Gaming) and `0x20009` (Lighting). The service must follow the binary buffer layout:

| Command | Purpose | Planned Operation | Data Buffer Contract & Values |
| :--- | :--- | :--- | :--- |
| `0x10` | Fan count query | Read | Returns number of physical fans (typically 2: CPU fan, GPU fan). |
| `0x11` | Fan speed query | Read | Returns RPM readings for CPU fan and GPU fan. |
| `0x1A` | Performance mode | Read / Write | Modes: Default (`0x00`), Performance (`0x01`), Cool (`0x02`), Quiet (`0x03`). Verified by read-back. |
| `0x21` | GPU thermal mode query | Read | Returns active dGPU thermal policy target. |
| `0x22` | GPU thermal mode setting | Write (conditional) | Sets dGPU thermal limit; requires board profile verification. |
| `0x26` | Maximum fan status query | Read | Returns active state: `0` = Auto fan curve, `1` = Maximum fan speed. |
| `0x27` | Maximum fan setting | Write (conditional) | Writes `0` (Auto) or `1` (Max). Followed by exponential read-back verification. |
| `0x28` | System design data | Read | Reports firmware thermal trip points, supported commands, and fan tables. |
| `0x29` | Power limits | Write (conditional) | PL1/PL2 platform power envelope adjustment on supported firmware. |
| `0x2D` | Victus S fan speed query | Read | Dedicated RPM query for Victus S-series laptops. |
| `0x2E` | Victus S fan levels | Write (conditional) | Multi-step fan level setting on Victus S hardware. |
| `0x2F` | Victus S fan table | Read | Extracts raw fan curve table from firmware. |
| `0x52` | Graphics / MUX mode | Read / Write | Display multiplexer switching: Hybrid (`0x00`), Discrete Only (`0x01`). |
| `0x20009`| 4-Zone RGB Keyboard | Read / Write | 24-byte payload: 4 zones × 3 bytes RGB (`[R, G, B]`) + mode/animation flags. |

### 2.3 Device Identification & Board Profiling
Hardware control is gated strictly on verified motherboard identification via WMI and SetupAPI:

| Device / Property | WMI Class & Property | Purpose in ZenOmen |
| :--- | :--- | :--- |
| HP Manufacturer | `Win32_ComputerSystem.Manufacturer` | Must contain `"HP"` or `"Hewlett-Packard"`. |
| System Model | `Win32_ComputerSystem.Model` | Must contain `"OMEN"` or `"Victus"`. |
| Motherboard Manufacturer | `Win32_BaseBoard.Manufacturer` | Confirms HP board origin. |
| Motherboard Product ID | `Win32_BaseBoard.Product` | Canonical board ID (e.g. `"88F8"`, `"8A24"`, `"8A3E"`). Matched against board catalogue. |
| System Product Identity | `Win32_ComputerSystemProduct` | UUID and SKU identification. |
| BIOS Vendor & Version | `Win32_BIOS.SMBIOSBIOSVersion` | Validates firmware version threshold before sending writes. |
| HP BIOS Mailbox | `ROOT\WMI\hpqBIntM` | Verifies presence of firmware WMI interface. |
| GPU PCI Identity | `Win32_VideoController.PNPDeviceID` | Confirms NVIDIA (`VEN_10DE`), AMD (`VEN_1002`), or Intel (`VEN_8086`). |

**HP Control Safety Invariants:**
1. Identify motherboard product ID (`Win32_BaseBoard.Product`) and BIOS version.
2. Confirm presence of `ROOT\WMI\hpqBIntM`.
3. Perform read-only queries (`0x10`, `0x26`, `0x28`).
4. Compare responses against the verified board profile catalogue (`src/service/windows/board_profile.rs`).
5. **Never** guess embedded-controller (EC) registers or write undocumented offsets.
6. **Never** apply a Victus S command to an unrelated OMEN board.
7. If the board ID is unrecognized, report `fan_control = false` and degrade safely to read-only telemetry.

---

## 3. Windows Native Telemetry APIs

All telemetry queries are executed via direct Windows C/COM APIs without shell tool invocations (`cmd.exe`, `powershell.exe`, `wmic.exe`).

### 3.1 CPU Telemetry
- **API:** `GetSystemTimes(&idle_time, &kernel_time, &user_time)` (`FILETIME`).
- **Calculation:**
  $$\Delta \text{Kernel} = \text{KernelTime}_t - \text{KernelTime}_{t-1}$$
  $$\Delta \text{User} = \text{UserTime}_t - \text{UserTime}_{t-1}$$
  $$\Delta \text{Idle} = \text{IdleTime}_t - \text{IdleTime}_{t-1}$$
  $$\Delta \text{Total} = \Delta \text{Kernel} + \Delta \text{User}$$
  $$\text{CPU Utilization} \% = 100.0 \times \left(1.0 - \frac{\Delta \text{Idle}}{\Delta \text{Total}}\right)$$
  *(Clamped between 0.0% and 100.0%).*
- **Frequency:** `CallNtPowerInformation(ProcessorInformation, ...)` returning `PROCESSOR_POWER_INFORMATION` (`CurrentMhz`, `MaxMhz`).
- **Model Name:** Read from registry `HARDWARE\DESCRIPTION\System\CentralProcessor\0\ProcessorNameString` or `Win32_Processor.Name`.

### 3.2 Memory Telemetry
- **API:** `GlobalMemoryStatusEx(&mem_status)` (`MEMORYSTATUSEX`).
- **Calculations:**
  - `total_phys_mb = mem_status.ullTotalPhys / (1024 * 1024)`
  - `avail_phys_mb = mem_status.ullAvailPhys / (1024 * 1024)`
  - `used_phys_mb = total_phys_mb.saturating_sub(avail_phys_mb)`

### 3.3 GPU Telemetry & VRAM
- **Adapter Enumeration:** DXGI 1.3 `CreateDXGIFactory1`, `IDXGIFactory1::EnumAdapters1`, `IDXGIAdapter1::GetDesc1`.
  - Filter out Microsoft Basic Render Driver (`Flags & DXGI_ADAPTER_FLAG_SOFTWARE`).
  - Dedicated VRAM: `DedicatedVideoMemory / (1024 * 1024)` MB.
  - Total System VRAM: `SharedSystemMemory / (1024 * 1024)` MB.
  - Vendor Identification: `VendorId == 0x10DE` (NVIDIA), `0x1002` (AMD), `0x8086` (Intel).
- **Engine Utilization:** PDH (Performance Data Helper) query on `\GPU Engine(*)\Utilization Percentage`:
  - `PdhOpenQueryW`, `PdhAddCounterW`, `PdhCollectQueryData`, `PdhGetFormattedCounterValue`.
  - Enumerate actual instances via `PdhEnumObjectItemsW`; aggregate engine load by physical adapter LUID.
- **VRAM Budget & Usage:** `IDXGIAdapter3::QueryVideoMemoryInfo` (distinguishing application process budget from whole-adapter performance counters).

### 3.4 Temperature & Fan Sensors
- **Primary:** HP WMI read-only queries (`0x11` for fan speeds) on verified boards.
- **GPU Temperatures:** Direct vendor APIs (NVAPI for NVIDIA, ADLX for AMD) or DXGI adapter sensor queries.
- **Fallback / Evaluation:** LibreHardwareMonitor evaluated as an optional external helper, never a mandatory hidden .NET runtime dependency.
- **Absence Guarantee:** When temperature or fan RPM is not exposed by hardware, return `None`. Never synthesize `0 °C` or `0 RPM`.

### 3.5 Battery Telemetry
- **API:** `GetSystemPowerStatus(&power_status)` (`SYSTEM_POWER_STATUS`).
- **Data Structure:**
  ```rust
  pub struct BatteryTelemetry {
      pub present: bool,
      pub charge_percent: Option<u8>,
      pub charging: Option<bool>,
      pub remaining_seconds: Option<u32>,
  }
  ```
  - `present = power_status.BatteryFlag != 128` (128 = No battery).
  - `charge_percent = if power_status.BatteryLifePercent <= 100 { Some(power_status.BatteryLifePercent) } else { None }`.
  - `charging = Some((power_status.BatteryFlag & 8) != 0)`.
  - `remaining_seconds = if power_status.BatteryLifeTime != 0xFFFFFFFF { Some(power_status.BatteryLifeTime) } else { None }`.

### 3.6 OS & System Diagnostics
- Query WMI `ROOT\CIMV2`:
  - `Win32_OperatingSystem`: `Caption`, `Version`, `BuildNumber`, `OSArchitecture`.
  - `Win32_ComputerSystem`: `Manufacturer`, `Model`, `TotalPhysicalMemory`.
  - `Win32_BaseBoard`: `Manufacturer`, `Product`.
  - `Win32_BIOS`: `SMBIOSBIOSVersion`, `ReleaseDate`.
  - `Win32_VideoController`: `Name`, `DriverVersion`, `AdapterRAM`.

---

## 4. Windows Process Architecture & IPC Security

```text
┌────────────────────────────────────────────────────────┐
│ Process A: zenomen.exe (Unprivileged Desktop GUI)      │
│  - User session permissions (no administrator rights)  │
│  - Slint UI / GPUI-CE presentation layer               │
│  - 1 Hz monitoring worker (Win32/DXGI/PDH telemetry)   │
│  - Client named pipe handler (src/service/client.rs)   │
└───────────────────────────┬────────────────────────────┘
                            │
                            │ Local Named Pipe IPC
                            │ \\.\pipe\ZenOmen.Hardware.v1
                            │ Custom Restrictive DACL
                            │ Interactive Session Authenticated
                            ▼
┌────────────────────────────────────────────────────────┐
│ Process B: zenomen-hw.exe (Elevated Windows Service)   │
│  - Runs under SCM (Service Control Manager)            │
│  - LocalSystem account (or dedicated hardware service) │
│  - Named pipe server (src/service/windows/server.rs)   │
│  - Client session ID verification                      │
│  - HP BIOS WMI Mailbox driver (ROOT\WMI\hpqBIntM)      │
│  - Verified board profile catalogue enforcement        │
│  - Exponential backoff read-back verification          │
└────────────────────────────────────────────────────────┘
```

### 4.1 Local Named Pipe Configuration
- **Pipe Endpoint:** `\\.\pipe\ZenOmen.Hardware.v1`
- **Security Descriptor & DACL:**
  - Built with `InitializeSecurityDescriptor`, `SetSecurityDescriptorDacl`.
  - `GENERIC_ALL` for `LocalSystem` (SID `S-1-5-18`) and `Builtin Administrators` (SID `S-1-5-32-544`).
  - `GENERIC_READ | GENERIC_WRITE` for `Interactive Users` (SID `S-1-5-4`).
  - `Network Logon` (SID `S-1-5-2`) explicitly denied.
- **Client Session Validation:**
  On connection:
  1. Call `ImpersonateNamedPipeClient(pipe_handle)`.
  2. Call `OpenThreadToken(GetCurrentThread(), TOKEN_QUERY, TRUE, &mut token)`.
  3. Query `TokenSessionId` via `GetTokenInformation`.
  4. Call `RevertToSelf()`.
  5. Check caller `SessionId == WTSGetActiveConsoleSessionId()`. If caller is from another session or remote desktop, disconnect pipe immediately.

### 4.2 Strict Hardware Request / Response Protocol
```rust
#[derive(Serialize, Deserialize, Debug)]
pub enum HardwareRequest {
    GetCapabilities,
    GetFanStatus,
    SetFanMode { mode: String },
    GetPowerProfiles,
    GetPowerProfile,
    SetPowerProfile { profile: String },
    ApplyPreset { preset: String },
    ApplyRgbProfile { profile_json: String },
    SetRgbEnabled { enabled: bool },
}

#[derive(Serialize, Deserialize, Debug)]
pub enum HardwareResponse {
    Capabilities(String),
    FanStatus { mode: String, rpms: Vec<u32> },
    Confirmed(String),
    PresetResult(String),
    Error { code: String, message: String },
}
```
- Protocol framing: 4-byte big-endian length prefix, max 65,536 bytes (64 KB).
- 30-second read/write timeouts.
- No arbitrary file paths or unchecked WMI method invocations.

### 4.3 Windows Service Lifecycle
- SCM APIs: `StartServiceCtrlDispatcherW`, `RegisterServiceCtrlHandlerExW`, `SetServiceStatus`, `OpenSCManagerW`, `CreateServiceW`, `StartServiceW`, `DeleteService`.
- Clean shutdown on `SERVICE_CONTROL_STOP` and `SERVICE_CONTROL_SHUTDOWN`.
- CLI commands: `zenomen-hw --install-service`, `zenomen-hw --uninstall-service`.

---

## 5. Desktop Integration & UI Strategy

### 5.1 Desktop Integration APIs
| Feature | Windows API / Registry | Implementation Module |
| :--- | :--- | :--- |
| **Single Instance** | `CreateMutexW` (`Local\ZenOmen.SingleInstance`) | `src/desktop/windows/single_instance.rs` |
| **Present Window** | `FindWindowW` + `SetForegroundWindow` / `ShowWindow` | `src/desktop/windows/single_instance.rs` |
| **Notification Tray**| `Shell_NotifyIconW` (`NOTIFYICONDATAW`, `NIM_ADD`, `NIM_DELETE`) | `src/desktop/windows/tray.rs` |
| **Tray Context Menu**| `CreatePopupMenu`, `TrackPopupMenu`, `WM_RBUTTONUP` | `src/desktop/windows/tray.rs` |
| **User Autostart** | Registry `HKCU\Software\Microsoft\Windows\CurrentVersion\Run` | `src/desktop/windows/autostart.rs` |
| **Browser Releases** | `ShellExecuteW(null, "open", "https://...", null, null, SW_SHOWNORMAL)` | `src/desktop/windows/releases.rs` |
| **App Data Paths** | `SHGetKnownFolderPath(FOLDERID_LocalAppData / FOLDERID_ProgramData)` | `src/desktop/windows/paths.rs` |

### 5.2 UI Framework Strategy (Slint & GPUI-CE Parity Track)
- **Current Production Layer:** Slint 1.18.1 with `backend-winit` and `renderer-femtovg`. Fully builds on Windows (`x86_64-pc-windows-msvc`), upholding the ≤40 MB RSS budget.
- **GPUI-CE Parity Track:** GPUI-CE 0.3.0 requires Rust 1.95+ and Win32/DirectX. The domain state and hardware provider interfaces (`src/core/`, `src/hardware/`) are structured to be 100% UI-agnostic. Slint remains the active production UI while the GPUI component and screen modules are established under `src/ui/`. Slint files are retired only after GPUI passes complete feature parity and visual acceptance tests on both Linux and Windows.

---

## 6. Linux Concerns Reconciliation Inventory

Every concern documented in `.planning/codebase/CONCERNS.md` is addressed in Wave 1:
1. **D-Bus Systemd Service Activation:** Add `SystemdService=zenomen-hw.service` to `data/dbus/com.zenomen.Hardware1.service`.
2. **Systemd Sandboxing:** Add `ReadWritePaths=-/sys/devices/platform/hp-wmi -/sys/class/hwmon` to `data/systemd/zenomen-hw.service`.
3. **RGB State Preservation:** Store last active profile before disable; restore on `SetRgbEnabled(true)`.
4. **Session D-Bus Absence Shutdown:** Explicitly drop `action_sender` in `src/main.rs` to allow clean thread join.
5. **Non-Destructive Autostart:** Update `Hidden=true` / `X-GNOME-Autostart-enabled=false` instead of deleting desktop entry.
6. **Dynamic Benchmark Tick Calculation:** Query `sysconf(_SC_CLK_TCK)` dynamically instead of hardcoding `USER_HZ = 100`.
7. **Offline Guidance Paths:** Replace debug target paths with `sudo systemctl start zenomen-hw`.
8. **SensorRegistry Root Isolation:** Pass configured root to `detect_rgb_interface` in `src/hardware/registry.rs`.
9. **Fan Control Predicate Alignment:** Align discovery in `registry.rs` to HP drivers matching `backend.rs`.
10. **History Window Unit Conversion:** Dynamically recompute sample capacity from configured duration seconds and polling interval in `src/core/telemetry.rs`.

---

## 7. Dual-OS CI & Release Packaging Matrix

### 7.1 CI Quality Gates
- **Linux (`ubuntu-latest`)**: `cargo fmt --all -- --check`, `git diff --check`, `cargo clippy --all-targets --all-features -- -D warnings`, `xvfb-run -a cargo test --all`, `cargo audit`.
- **Windows (`windows-latest`)**: `cargo check --target x86_64-pc-windows-msvc --all-targets --all-features`, `cargo clippy --target x86_64-pc-windows-msvc --all-targets --all-features -- -D warnings`, `cargo test --target x86_64-pc-windows-msvc --lib`.

### 7.2 Release Artifacts & Cryptographic Checksums
- Linux: `zenomen_<version>_amd64.deb`, `ZenOmen_<version>_amd64.AppImage`.
- Windows: `zenomen-<version>-x64.msi` (WiX installer), standalone `zenomen.exe`, `zenomen-hw.exe` (with embedded version resource and multi-res icon).
- Checksum: Consolidated `SHA256SUMS` attached to GitHub Release.

---

## 8. Technical References

- **Microsoft Win32 APIs:**
  - [GetSystemTimes](https://learn.microsoft.com/en-us/windows/win32/api/processthreadsapi/nf-processthreadsapi-getsystemtimes)
  - [GlobalMemoryStatusEx](https://learn.microsoft.com/en-us/windows/win32/api/sysinfoapi/nf-sysinfoapi-globalmemorystatusex)
  - [IDXGIAdapter3::QueryVideoMemoryInfo](https://learn.microsoft.com/en-us/windows/win32/api/dxgi1_4/nf-dxgi1_4-idxgiadapter3-queryvideomemoryinfo)
  - [Windows Performance Counters (PDH)](https://learn.microsoft.com/en-us/windows/win32/perfctrs/about-performance-counters)
  - [Named Pipe Security & Access Rights](https://learn.microsoft.com/en-us/windows/win32/ipc/named-pipe-security-and-access-rights)
  - [Win32_Battery WMI Class](https://learn.microsoft.com/en-us/windows/win32/cimwin32prov/win32-battery)
  - [Enumerating Device Interfaces](https://learn.microsoft.com/en-us/windows-hardware/drivers/install/enumerating-installed-device-interface-classes)
- **HP Firmware & Linux Drivers:**
  - [Linux Kernel hp-wmi Driver](https://github.com/torvalds/linux/blob/master/drivers/platform/x86/hp/hp-wmi.c)
  - [Ohman — HP OMEN/Victus Firmware Research](https://github.com/P4R1H/ohman)
  - [OMEN 4-Zone WMI Lighting Interface](https://github.com/Alez22/omenkey)
- **UI Frameworks & OS Lifecycle:**
  - [GPUI-CE Documentation](https://docs.rs/crate/gpui-ce/latest)
  - [Windows 10 Lifecycle & ESU](https://support.microsoft.com/en-us/windows/deployment/updates-lifecycle/windows-10-support-has-ended-on-october-14-2025)
  - [Windows 11 Release Health](https://learn.microsoft.com/en-us/windows/release-health/windows11-release-information)
