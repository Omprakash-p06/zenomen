# Windows Test Matrix & Verification Protocols

- **Status:** Ratified (Wave 6)
- **Target OS:** Windows 10 (22H2 Build 19045+) & Windows 11 (24H2 Build 26100+)
- **Architecture:** `x86_64-pc-windows-msvc`

---

## 1. Supported Platform Baseline Matrix

| OS Platform | Target Build / Architecture | Support Tier | Graphics / Hardware APIs | IPC Mechanism |
|:---|:---|:---:|:---|:---|
| **Windows 10** | 22H2 (Build 19045+, x86_64) | Tier 1 (Full) | DXGI 1.3+, PDH, Win32 APIs | Named Pipe (`ZenOmen.Hardware.v1`) |
| **Windows 11** | 24H2 (Build 26100, x86_64) | Tier 1 (Full) | DXGI 1.4+, PerMonitorV2 High-DPI | Named Pipe (`ZenOmen.Hardware.v1`) |
| **Windows 11** | 25H2 / 26H2 Insider Preview | Tier 2 (Validated) | DXGI 1.6+, Modern App Container | Named Pipe (`ZenOmen.Hardware.v1`) |

---

## 2. Automated Test Matrix

The following test suites validate the Windows implementation either in CI (cross-compilation / mock environments) or on dedicated physical test benches:

| Test Suite | Purpose | Execution Command | Pass Criteria |
|:---|:---|:---|:---|
| `tests/windows_telemetry_tests.rs` | Unit testing of native Windows telemetry providers | `cargo test --test windows_telemetry_tests` | 6/6 tests passing: CPU counters, RAM, GPU, Battery, Sensors, Provider |
| `tests/windows_service_tests.rs` | Named pipe protocol, DACL security, command validation | `cargo test --test windows_service_tests` | 6/6 tests passing: Pipe roundtrip, invalid command rejection, auth |
| `tests/desktop_integration_tests.rs` | Single instance mutex, tray message pump, autostart registry, paths | `cargo test --test desktop_integration_tests` | 12/12 tests passing: Mutex, tray icon, autostart toggle, app data paths |
| `tests/core_contract_tests.rs` | Platform-agnostic domain contracts and invariant rules | `cargo test --test core_contract_tests` | 6/6 tests passing: InFlightGuard, dynamic buffer scaling, event loop |

---

## 3. Physical Hardware Validation Steps

When running on a physical HP OMEN or Victus machine running Windows 10 or 11:

### Step 1: Automated Self-Test
Run the test binary with test environment variables:
```cmd
set RUST_LOG=info
cargo test --test windows_telemetry_tests -- --nocapture
```
**Verification:** Ensure CPU load matches Task Manager (within +/- 3%), available RAM matches `GlobalMemoryStatusEx`, and GPU VRAM matches DXGI telemetry.

### Step 2: Named Pipe Service Verification
Install and start the service in an elevated PowerShell:
```powershell
.\zenomen-hw.exe --install-service
Start-Service ZenOmenHardwareService
Get-Service ZenOmenHardwareService
```
**Verification:** Service state should be `Running`. Check Windows Event Viewer or `%PROGRAMDATA%\ZenOmen\logs\hardware-service.log` for successful initialization.

### Step 3: Interactive GUI Verification
Run `zenomen.exe` as standard user (non-elevated):
```powershell
.\zenomen.exe
```
**Verification:**
- GUI window opens without requiring UAC elevation.
- Overview screen graphs update every 1000ms.
- Toggle Fan Mode to "Max": Verify fan RPM increases audible speed within 2 seconds.
- Toggle Fan Mode back to "Auto": Verify fan curve returns to quiet OEM profile.
- System tray icon is visible with context menu ("Open ZenOmen", "Fan Mode: Auto", "Fan Mode: Max", "Quit ZenOmen").

### Step 4: Service Uninstallation & Clean Removal
```powershell
Stop-Service ZenOmenHardwareService
.\zenomen-hw.exe --uninstall-service
```
**Verification:** Service registration is removed cleanly; fan state resets to firmware default (`Auto`).
