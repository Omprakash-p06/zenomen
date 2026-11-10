# Windows Native Telemetry Architecture

On Windows 10 and 11, ZenOmen replaces Linux `/sys` and `/proc` readers with native Win32, DirectX (DXGI 1.3), Performance Data Helper (PDH), and WMI telemetry subsystems.

## Subsystem Implementations

### 1. CPU Telemetry (`src/hardware/windows/cpu.rs`)
- **Model Name**: Read from `HKLM\HARDWARE\DESCRIPTION\System\CentralProcessor\0\ProcessorNameString`.
- **Utilization**: Calculated from continuous `GetSystemTimes(&idle, &kernel, &user)` intervals using monotonic 64-bit `FILETIME` arithmetic.
- **Frequency**: Current MHz retrieved via `CallNtPowerInformation(ProcessorInformation)` or PDH counters.
- **Temperature**: Queried via WMI `MSAcpi_ThermalZoneTemperature` (`CurrentTemperature / 10.0 - 273.15`), returning `None` if unexposed.

### 2. Memory Telemetry (`src/hardware/windows/memory.rs`)
- Uses `GlobalMemoryStatusEx(&mut statex)` with `ullTotalPhys` and `ullAvailPhys`.
- Memory used is calculated as `(total - avail) / 1024 / 1024` MB.

### 3. GPU Telemetry (`src/hardware/windows/gpu.rs`)
- **Adapter Enumeration**: Uses DXGI 1.3 (`CreateDXGIFactory1`, `EnumAdapters1`).
- **Identity**: Reads `DXGI_ADAPTER_DESC1.Description`, `VendorId`, and `DeviceId`.
- **Dedicated Video Memory**: Extracted from `DedicatedVideoMemory` or queried in real time via `IDXGIAdapter3::QueryVideoMemoryInfo`.
- **Utilization & Temperature**: Queried through NVAPI / ADL / PDH counters when vendor drivers are loaded.

### 4. Battery Telemetry (`src/hardware/windows/battery.rs`)
- Queries `GetSystemPowerStatus(&mut sps)`.
- Verifies AC line status, battery flag (charging, high, low, critical), battery life percent (`sps.BatteryLifePercent`), and remaining runtime (`sps.BatteryLifeTime`).
- If no battery is installed (`BatteryFlag == 128`), `battery` is reported as `None`.
