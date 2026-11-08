# Windows 10/11 Implementation & Function Contracts

This document formalizes the architectural contracts, Win32 API bindings, error handling guarantees, and security policies governing ZenOmen's native Windows implementation (Windows 10 22H2 19045+ and Windows 11 24H2 26100+).

---

## 1. Subsystem Boundaries & Architecture

ZenOmen on Windows operates across two isolated privilege tiers:
1. **Unprivileged Client Process (`zenomen.exe`)**:
   - Collects read-only hardware telemetry (CPU, RAM, GPU, Battery) using unprivileged Win32 and DXGI APIs.
   - Manages desktop lifecycle (single-instance mutex, notification area tray icon, HKCU autostart, notifications).
   - Communicates with the privileged hardware daemon over a local named pipe (`\\.\pipe\ZenOmen.Hardware.v1`).
2. **Privileged SCM Hardware Service (`zenomen-hw.exe`)**:
   - Installed as a Windows Service Control Manager (SCM) service (`ZenOmenHardwareService`) running as `LocalSystem`.
   - Dispatches Win32 service control events via `StartServiceCtrlDispatcherW` and `RegisterServiceCtrlHandlerExW`.
   - Exposes a hardened named pipe server listening on `\\.\pipe\ZenOmen.Hardware.v1`.
   - Enforces physical hardware validation (`BoardValidation`) before dispatching any firmware mutations.

---

## 2. Motherboard Profiles & Physical Hardware Validation

### 2.1. Board Profile Catalogue
Motherboard product IDs read from `HARDWARE\DESCRIPTION\System\BIOS\BaseBoardProduct` are resolved via `resolve_board_profile`:
- `88F8`: HP OMEN 15/16 AMD (2 fans, max fan support, 4-zone RGB, balanced/performance)
- `8A24`: HP OMEN 16 Intel (2 fans, max fan support, 4-zone RGB, balanced/performance)
- `8A3E`: HP Victus 15/16 (2 fans, no max fan, 1-zone RGB, quiet/default/performance)
- Unknown motherboards return `None`, defaulting to safe read-only operation.

### 2.2. Physical Hardware Validation Contract (`BoardValidation`)
Firmware mutations (fan mode, power profile, presets, RGB) are gated by `BoardValidation`:
- `new_unvalidated(board_id, bios_version)`: Default baseline where all write capabilities (`fan_write_verified`, `power_write_verified`, `rgb_write_verified`) are `false`. Read requests succeed, but mutations return `HardwareControlError::UnsupportedMode`.
- `new_verified(board_id, bios_version)`: Explicitly verified hardware record for tested models and BIOS revisions.
- `authorize_mutation(req, board, validation)`: Evaluates request against both profile invariants and physical validation flags.

---

## 3. IPC Framing & Security Contracts

### 3.1. Wire Protocol Framing
Named pipe communication uses a strict **4-byte big-endian length prefix** followed by UTF-8 JSON payload:
- **Maximum Payload Size (`MAX_PAYLOAD_SIZE`)**: 64 KB (65,536 bytes).
- Payloads exceeding 64 KB are rejected with `HardwareControlError::ProtocolError`.
- No framing header overhead beyond the 4-byte length prefix.

### 3.2. Access Control & Session Verification
- **Security Descriptor (SDDL)**: `D:(A;;GA;;;SY)(A;;GA;;;BA)(A;;GRGW;;;IU)` granting Generic All to SYSTEM (`SY`) and Builtin Administrators (`BA`), and Generic Read/Write to Interactive Users (`IU`).
- **Session Verification (`verify_client_session`)**:
  - Service impersonates the connecting client via `ImpersonateNamedPipeClient`.
  - Queries client token `TokenSessionId` via `GetTokenInformation`.
  - Compares against `WTSGetActiveConsoleSessionId()`.
  - Reverts impersonation via `RevertToSelf()` and closes token handles.
  - Rejects cross-session connections from terminal services / RDP to prevent unauthorized control.
- **Pipe Flags**: Created with `PIPE_ACCESS_DUPLEX | PIPE_TYPE_MESSAGE | PIPE_READMODE_MESSAGE | PIPE_WAIT`. `FILE_FLAG_OVERLAPPED` is omitted to guarantee synchronous, blocking I/O compatibility.

---

## 4. Service Control Manager (SCM) Lifecycle

### 4.1. SCM Dispatcher
- `run_service()` initializes `StartServiceCtrlDispatcherW` with `service_main`.
- If invoked directly outside SCM (returning `ERROR_FAILED_SERVICE_CONTROLLER_CONNECT`), automatically falls back to standalone console mode.
- `zenomen-hw.exe --console` or `-c` explicitly starts standalone console mode.
- `service_main` registers `service_ctrl_handler` via `RegisterServiceCtrlHandlerExW` and reports `SERVICE_START_PENDING`, `SERVICE_RUNNING`, `SERVICE_STOP_PENDING`, and `SERVICE_STOPPED` using `SetServiceStatus`.

---

## 5. Telemetry & Sensor Contracts

### 5.1. Honest Absence Policy (MON-01, ARCH-03)
When sensor interfaces are unavailable on Windows, the provider returns honest `None` rather than fabricating synthetic `0.0 °C` or dummy values:
- **CPU & GPU Temperature**: Unprivileged Windows APIs lack thermal telemetry without vendor ring-0 drivers; `read_temperature_sensors()` returns `(None, None)`.
- **Fan Tachometers**: `read_fan_rpms()` returns an empty vector until unmasked by a verified hardware service connection.
- **VRAM Distinction**: `sample_gpu_memory()` reports total dedicated VRAM while returning `None` for used VRAM until queried.

### 5.2. CPU Utilization Math
- Queries `GetSystemTimes` for 100ns system time deltas.
- On Windows, `KernelTime` includes `IdleTime`. True busy time is `(delta_kernel + delta_user) - delta_idle`.
- Handled edge cases: zero delta returns `None`, backwards times return `None`.

### 5.3. GPU Telemetry
- **Adapter Discovery**: DXGI 1.3 `CreateDXGIFactory1` and `EnumAdapters1` enumerates physical GPUs, decodes UTF-16 descriptions, and filters software rasterizers (`DXGI_ADAPTER_FLAG_SOFTWARE`).
- **GPU Utilization (`GpuPerformanceCollector`)**: Uses persistent PDH query on `\GPU Engine(*)\Utilization Percentage` with double-precision counter formatting.

---

## 6. Desktop Integration Contracts

### 6.1. Single-Instance Mutex
- Session-scoped named mutex `Local\ZenOmen.SingleInstance`.
- If already running, activates existing window via `FindWindowW` and `ShowWindow(SW_RESTORE)`.

### 6.2. System Tray & Message Pump
- Thread affinity preserved: `register_tray_icon` spawns background pump thread `zenomen-win32-tray-pump` which registers `WNDCLASSW` and executes `GetMessageW` loop.
- Context menu (`TrackPopupMenu`) actions: Open Window, Fan Auto, Fan Max, Quit.

### 6.3. Desktop Notifications
- Implemented via `Shell_NotifyIconW(NIM_MODIFY)`.
- **Window Handle Binding**: Explicitly sets `nid.hWnd = hwnd` from `TrayHandle` to ensure notification balloons bind to the active tray icon instance.
