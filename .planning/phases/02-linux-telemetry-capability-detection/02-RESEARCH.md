# Phase 02: Linux Telemetry & Capability Detection — Research

**Phase:** 02 - Linux Telemetry & Capability Detection
**Confidence:** HIGH

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| **MON-01** | CPU model and aggregate utilization calculated via previous-tick deltas on a worker thread | Read `/proc/stat` total and idle jiffies between ticks; calculate utilization percentage delta without blocking sleeps. Model string from `/proc/cpuinfo`. |
| **MON-02** | Per-core CPU utilization | Parse individual `cpu0`, `cpu1`, ... lines from `/proc/stat` and maintain per-core previous-tick state in the provider. |
| **MON-03** | Current CPU frequency | Read `/sys/devices/system/cpu/cpu*/cpufreq/scaling_cur_freq` (in kHz) or `/proc/cpuinfo` `cpu MHz`. |
| **MON-04** | CPU temperature chosen by hwmon driver and label priority | Scan `/sys/class/hwmon/hwmon*`; prioritize drivers `k10temp` (Tctl/Tccd1), `coretemp` (Package id 0), then ACPI thermal zones. |
| **MON-05** | GPU name, utilization, temperature across NVIDIA, AMD, Intel (dynamic DRM paths, no `card0` hardcoding) | Enumerate `/sys/class/drm/card*` inspecting `device/vendor` and `device/gpu_busy_percent`; fallback to `nvidia-smi` CLI or sysfs when proprietary driver is present. Handle disconnected / unavailable drivers gracefully. |
| **MON-06** | GPU VRAM usage and power draw | Query DRM amdgpu VRAM sysfs (`mem_info_vram_used`, `mem_info_vram_total`) or `nvidia-smi` query format (`memory.used`, `memory.total`, `power.draw`). Model as optional fields. |
| **MON-07** | Total, available, used RAM from `/proc/meminfo` (`MemAvailable` math) | Parse `MemTotal` and `MemAvailable`; compute `used = MemTotal - MemAvailable`. Never use naive `MemFree`. |
| **MON-08** | Absent sensor/control displays "Unavailable" with reason, never 0 | Contract preserved from Phase 1: `Option<T>` for sensor fields, granular capability flags in `HardwareCapabilities`. |
| **MON-10** | Manual refresh telemetry on demand | Supported by `AppCommand::RefreshTelemetry` breaking channel wait on worker thread (already verified in Phase 1; wired to Linux provider in Phase 2). |
| **MON-11** | Configurable telemetry history window length | Settings UI / command `AppCommand::SetHistoryWindow(usize)` updates `AppState::history_window` and `TelemetryHistory` ring buffer capacity dynamically. |
| **MON-12** | Single shared `SensorRegistry` for dashboard and diagnostics | Centralized registry struct scanning sysfs once or on-demand, caching canonical sensor associations so both views reference identical path, driver, and label bindings. |
| **DIAG-01** | Detection report covering version, OS, kernel, hardware IDs, detected sensors, capabilities | Read `/etc/os-release`, `uname -r`, DMI `/sys/class/dmi/id/{product_name,sys_vendor}`, combine with `SensorRegistry` and `HardwareCapabilities`. |
| **DIAG-02** | Report includes recent errors and recent readings | Collect snapshot history and last error from `AppState` / provider, format into structured JSON / text export. |
</phase_requirements>

## Standard Stack & Data Sources

### 1. CPU Metrics (`/proc/stat`, `/proc/cpuinfo`, `/sys/devices/system/cpu`)
- **Aggregate & Per-Core Utilization:**
  - Read `/proc/stat`:
    ```text
    cpu  user nice system idle iowait irq softirq steal guest guest_nice
    cpu0 user nice system idle iowait irq softirq steal guest guest_nice
    ```
  - `idle_all = idle + iowait`
  - `system_all = system + irq + softirq`
  - `virtual_all = guest + guest_nice`
  - `total = user + nice + system_all + idle_all + steal + virtual_all`
  - `delta_total = total - prev_total`, `delta_idle = idle_all - prev_idle_all`
  - `utilization = 100.0 * (1.0 - delta_idle / delta_total)`
  - Crucial: calculate deltas against stored previous tick state in `LinuxHardwareProvider`. Zero sleeping or blocking calls on the thread!
- **Model String:** Read `model name` line from `/proc/cpuinfo`.
- **CPU Frequency:** Read `/sys/devices/system/cpu/cpu0/cpufreq/scaling_cur_freq` (divide by 1000 for MHz) or average across all online cores.

### 2. RAM Metrics (`/proc/meminfo`)
- Read `/proc/meminfo`:
  ```text
  MemTotal:       16082444 kB
  MemAvailable:    8942120 kB
  ```
- Calculation:
  - `total_mb = MemTotal / 1024`
  - `available_mb = MemAvailable / 1024`
  - `used_mb = total_mb - available_mb`
  - Utilization `% = (used_mb / total_mb) * 100.0`
- Complies strictly with MON-07 (never substitute `MemFree`, which excludes buffers/cache reclaimable by Linux).

### 3. Hardware Sensors & Temperature (`/sys/class/hwmon`)
- Iterate `/sys/class/hwmon/hwmon*`:
  - Read `name`:
    - `k10temp` (AMD Zen desktop/mobile APUs)
    - `coretemp` (Intel Core mobile/desktop)
    - `hp` / `hp-wmi` (HP OMEN / Victus fan tachometers `fan1_input`, `fan2_input`, `pwm1_enable`)
    - `amdgpu` (Radeon iGPU / dGPU temperature `temp1_input`)
    - `acpitz` (ACPI thermal zone fallback)
  - Driver & Label Priority Matrix for CPU Temperature (MON-04, MON-12):
    1. Driver `k10temp` with label `Tctl` or `Tdie` (or `temp1_input`)
    2. Driver `coretemp` with label `Package id 0` (or `temp1_input`)
    3. Driver `acpitz` `temp1_input`
  - Fan Speeds:
    - Look for `fan*_input` in `hp` / `hp-wmi` / platform devices.
    - Values are raw RPM integers.

### 4. GPU Metrics (DRM sysfs & NVIDIA)
- **DRM enumeration (dynamic, never hardcode `card0`):**
  - Iterate `/sys/class/drm/card*`:
    - Filter out sub-connectors (e.g. `card1-DP-1`, only match `card[0-9]+$`).
    - Read `device/vendor`:
      - `0x1002` = AMD
      - `0x8086` = Intel
      - `0x10de` = NVIDIA
    - If AMD (`amdgpu`):
      - Utilization: read `device/gpu_busy_percent` (contains 0-100 integer string).
      - Temperature: link to associated `hwmon` device in `device/hwmon/hwmon*/temp1_input` (divide millidegrees by 1000).
      - VRAM: read `device/mem_info_vram_used` and `device/mem_info_vram_total`.
- **NVIDIA Proprietary Driver Fallback:**
  - When NVIDIA GPU is present and driver is bound, query via non-blocking command execution or NVML / `nvidia-smi`:
  - `nvidia-smi --query-gpu=name,utilization.gpu,temperature.gpu,memory.used,memory.total --format=csv,noheader,nounits`
  - Timeout after 200ms; if NVIDIA driver fails or is in sleep state (e.g., hybrid graphics dynamic power management), return `None` (gap/unavailable) without hanging the sampling worker!

### 5. Shared `SensorRegistry` (MON-12, DIAG-01)
- Single source of truth instantiated in `LinuxHardwareProvider`.
- Struct:
  ```rust
  pub struct SensorRegistry {
      pub cpu_temp_sensor: Option<SensorPath>,
      pub fan_sensors: Vec<SensorPath>,
      pub gpu_sensors: Vec<GpuSensorInfo>,
  }
  ```
- Evaluated on startup and available for query by both runtime telemetry sampling and the Diagnostics screen / diagnostics export.

### 6. Diagnostics Export (DIAG-01, DIAG-02)
- Diagnostics exporter function: `export_diagnostics(state: &AppState, registry: &SensorRegistry) -> DiagnosticsReport`.
- Serializes to formatted JSON or markdown/text file (e.g., `zenomen-diagnostics-{timestamp}.json`).
- Includes:
  - ZenOmen Version, App commit
  - Host OS: `/etc/os-release` `PRETTY_NAME`, Kernel `uname -r`
  - DMI / Board info: `/sys/class/dmi/id/product_name`, `/sys/class/dmi/id/sys_vendor`
  - Capability flags & status
  - Discovered sensors with paths and drivers
  - Recent errors and latest snapshot

## Architecture Patterns

```
+-------------------------------------------------------------+
|                     LinuxHardwareProvider                   |
|  +---------------------+        +-------------------------+ |
|  |   SensorRegistry    |        |   Sampling State        | |
|  | - CPU Temp Path     |        | - Previous CPU Jiffies  | |
|  | - Fan RPM Paths     |        | - Previous Core Jiffies | |
|  | - GPU DRM/NV Devices|        | - Last Error / Status   | |
|  +---------------------+        +-------------------------+ |
+-------------------------------------------------------------+
                              |
                     sample_telemetry()
                              |
                              v
+-------------------------------------------------------------+
|                 spawn_monitoring_worker                     |
|           (1 Hz tick loop + RefreshTelemetry)               |
+-------------------------------------------------------------+
                              |
                      AppEvent Channel
                              v
+-------------------------------------------------------------+
|                       ZenOmenApp                            |
|             AppState update + Slint Dispatch                |
+-------------------------------------------------------------+
```

## Don't Hand-Roll & Common Pitfalls

1. **Do NOT sleep to calculate CPU deltas:** Never do `thread::sleep(100ms)` during sampling! Store previous jiffies in a struct field and calculate the delta between 1 Hz ticks.
2. **Do NOT hardcode `/sys/class/drm/card0`:** Discrete GPUs or laptops with integrated AMD/Intel and discrete NVIDIA often assign `card1` to the display and `card0` or `card2` to accelerators. Always enumerate directory entries dynamically.
3. **Do NOT block on `nvidia-smi`:** NVIDIA GPUs on laptops frequently enter D3cold runtime suspend. If `nvidia-smi` hangs or takes seconds, sampling will stall. Run it with a strict timeout or check PCIe power state (`power/runtime_status`) first.
4. **Do NOT use `MemFree` for available RAM:** Linux disk cache uses available memory. `MemTotal - MemFree` falsely reports 95%+ RAM usage. Always use `MemTotal - MemAvailable`.
5. **Do NOT diverge diagnostics and dashboard sensors:** The exact sysfs path used for the overview CPU temperature must come from the `SensorRegistry` that Diagnostics inspects.

## Verification Strategy

1. **Unit tests with simulated procfs/sysfs:**
   - Parse `/proc/stat` samples and test delta calculation accuracy.
   - Parse `/proc/meminfo` strings with standard and edge-case values.
   - Test `SensorRegistry` priority rules against mocked directory hierarchies.
2. **Headless Linux execution:**
   - Provider initialization on the current Linux host (reads real `k10temp`, `hp-wmi`, `/proc/stat`, `/proc/meminfo`).
   - Clean degradation on missing or sleeping GPUs.
3. **Export format verification:**
   - Verify generated diagnostics export against required schema fields.
