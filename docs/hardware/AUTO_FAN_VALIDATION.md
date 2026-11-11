# ZenOmen Hardware Validation: Automatic Cooling & Fan Control on HP Victus 8A3D / F.22

- **Author:** ZenOmen Hardware Engineering
- **Date:** 2026-11-11
- **Status:** Authoritative Hardware Reference & Physical Validation Specification
- **Target Platform:** HP Victus 15-fb0xxx (Motherboard: `8A3D`, BIOS: `F.22`)

---

## 1. Executive Summary

This document establishes the verified hardware behavior, safety invariants, and thermal characteristics of HP Victus and OMEN gaming laptops, with specific physical validation conducted on the **HP Victus 15-fb0xxx (Motherboard 8A3D, BIOS F.22)**.

A primary design requirement of ZenOmen is **hardware honesty**:
1. Never generalize observations from a single motherboard to an entire product family.
2. Distinguish intentional firmware acoustic policies (e.g., passive zero-RPM cooling at idle) from Embedded Controller (EC) lockups.
3. Understand upstream Linux kernel driver mechanisms (`hp-wmi`) versus ZenOmen's userland Windows ACPI/WMI implementation.
4. Enforce fail-closed safety invariants on unvalidated hardware.

---

## 2. Motherboard Capability Registry (8A3D / F.22)

The following capability classification is physically validated for motherboard **8A3D** running BIOS **F.22**:

| Capability | Hardware Status (8A3D / F.22) | Implementation & Safety Rule |
| :--- | :--- | :--- |
| **CPU Fan RPM Telemetry** | **Physically Validated** | Read via WMI Fast Query `0x2D` / Linux `fan1_input`. Accurate within ±50 RPM. |
| **GPU Fan RPM Telemetry** | **Physically Validated** | Read via WMI Fast Query `0x2D` / Linux `fan2_input`. Accurate within ±50 RPM. |
| **Max Fan Mode (Forced 100%)** | **Physically Validated** | Enabled via WMI Opcode `0x27 = 1`. Forces fans to ~5400 RPM (CPU) and ~5200 RPM (GPU). |
| **Automatic Cooling Restoration** | **Physically Validated** | Restored via board-specific 3-step sequence. Returns curve control to EC. |
| **Manual PWM Fan Curves** | **Unsupported** | Write access not authorized on 8A3D. Must remain disabled in UI. |
| **Thermal Profiles (Quiet/Balanced/Perf)** | **Command Accepted, Readback Unavailable** | WMI write `0x1A` is acknowledged by BIOS. EC status readback unavailable; labeled unverified. |
| **Display Refresh Switching (60Hz / 144Hz)** | **Physically Validated** | Windows `ChangeDisplaySettingsExW` / Linux Wayland/X11 RandR. |
| **RGB Lighting Controls** | **Unsupported** | Physical hardware lacks 4-zone RGB keyboard controller. UI hidden/disabled. |

> [!IMPORTANT]
> Other motherboard and BIOS combinations must remain independently classified until physically validated. Never turn a successful test on one Victus into blanket authorization for other Victus or OMEN devices.

---

## 3. Investigating Automatic Fan Behavior & Zero-RPM State

### 3.1 Understanding Modes vs Telemetry
When interacting with the cooling subsystem, three states must be strictly distinguished:

- **Max:** ZenOmen explicitly requests maximum fan speed from the embedded controller.
- **Auto:** ZenOmen releases manual override and hands cooling curve management back to the EC firmware.
- **0 RPM:** The EC firmware may intentionally halt the fans when thermal conditions permit passive heat dissipation.

### 3.2 The Zero-RPM Idle Phenomenon
On the tested HP Victus 15-fb0xxx (8A3D / F.22), when transitioning from Max Fan to Auto at low idle temperatures (approximately 35°C–37°C CPU and 34°C–36°C GPU in an ambient room of ~22°C):
- The fan telemetry frequently drops to **0 RPM**.
- **This is NOT evidence of a malfunction.** The HP embedded controller cooling table on this device implements an aggressive acoustic quiet mode at low temperatures, turning fans completely off to eliminate bearing noise.
- **Critical Safety Distinction:**
  - **Legitimate Zero-RPM:** Fans remain stopped while cool, but resume spinning automatically as CPU/GPU core temperatures elevate under workload.
  - **Firmware Stall / Freeze:** Fans remain stopped at 0 RPM even as temperatures exceed normal thermal envelopes, potentially risking thermal throttling or shutdown.
- **Threshold Guidance:** Previous informal references suggested a "45°C–50°C fan activation threshold." This must **not** be documented as a confirmed universal threshold without continuous thermal-chamber measurements. The actual curve is a multi-input function inside EC firmware taking into account CPU package power, GPU diode, VRM thermal sensors, and battery charging state.

---

## 4. Upstream Linux `hp-wmi` Driver Behavior vs. Windows Implementation

### 4.1 Linux Kernel `hp-wmi.c` Implementation
The upstream Linux driver (`drivers/platform/x86/hp/hp-wmi.c`) handles Victus S hardware with specialized logic:

1. When automatic cooling is requested via sysfs:
   ```bash
   echo 2 > /sys/devices/platform/hp-wmi/hwmon/hwmonX/pwm1_enable
   ```
2. The driver calls `hp_wmi_apply_fan_settings(PWM_MODE_AUTO)`:
   - It clears max fan mode (`hp_wmi_fan_speed_max_set(priv, 0)`).
   - On Victus S hardware, it calls `hp_wmi_fan_speed_max_reset(priv)`, which executes `hp_wmi_fan_speed_set(priv)` using WMI opcode `0x2E` (`HPWMI_VICTUS_S_FAN_SPEED_SET_QUERY`) with argument `{0, 0}`.

### 4.2 The 8A3D Board-Specific EC Freeze
- On motherboard **8A3D** running BIOS **F.22**, issuing WMI opcode `0x2E` with data `{0, 0}` triggers a physically observed firmware bug: the EC cooling loop enters an inactive state where fans remain frozen at 0 RPM even under 80°C+ thermal load.
- This is a **board-specific embedded controller failure**, not a universal property of the Linux kernel driver or all HP laptops.

### 4.3 ZenOmen's Validated Windows Restoration Sequence
To safely return cooling control to the firmware without triggering the 8A3D EC freeze, ZenOmen implements a dedicated restoration sequence in `src/service/windows/victus_s.rs`:

```text
Step 1: Disable Max Fan
        Issue WMI Opcode 0x27 with parameter 0.

Step 2: Arm Fan-Control Handshake Window
        Issue WMI Opcode 0x10 with parameter 120 (120-second active window).

Step 3: Reapply Active Thermal Profile
        Issue WMI Opcode 0x1A with known active thermal profile (e.g., Balanced = 0x00).
        This forces the EC to re-read its internal thermal curve tables.

Step 4: Strict Prohibition
        NEVER issue opcode 0x2E {0,0} on board 8A3D / F.22.
```

This sequence has been physically validated on board 8A3D with BIOS F.22. It reliably returns fan management to the firmware, allowing fans to stop at idle if cool, and spin up promptly as temperatures rise.

---

## 5. Standardized Physical Validation Procedure

Any verification of automatic fan control must follow this repeatable multi-stage runbook before making claims about cooling health.

### 5.1 Test Environment & Data Collection
For each test run, log the following telemetry stream at 1-second intervals:
1. `timestamp`: Epoch seconds.
2. `hw_id`: Motherboard board name (e.g., `8A3D`).
3. `bios_ver`: BIOS version (e.g., `F.22`).
4. `os`: Operating System and kernel version (e.g., Linux 6.8.0 / Windows 11 24H2).
5. `cpu_temp_c`: Package temperature.
6. `gpu_temp_c`: Edge/junction temperature.
7. `cpu_fan_rpm`: Tachometer reading.
8. `gpu_fan_rpm`: Tachometer reading.
9. `requested_mode`: `Auto` or `Max`.
10. `thermal_profile`: Active profile (Quiet, Balanced, Performance).

### 5.2 Test Stages

#### Stage 1: Cold Idle Verification (Passive Cooling Check)
1. Boot system and let sit idle at desktop for 10 minutes (ambient ~20–23°C).
2. Verify CPU temp stabilizes below 40°C.
3. Observe fan RPM. Telemetry of 0 RPM is expected and confirmed acceptable.
4. Record baseline acoustic level.

#### Stage 2: Max Fan Escalation Check
1. In ZenOmen, toggle **Max Fan** to **ON**.
2. Within 5 seconds, fans must audibly spool up.
3. Telemetry must confirm CPU fan > 5000 RPM and GPU fan > 4800 RPM.
4. Maintain for 60 seconds. Temperatures should drop by 3°C–6°C.

#### Stage 3: Auto Mode Restoration & Thermal Ramping Check
1. In ZenOmen, toggle **Max Fan** to **OFF** (requesting **Auto**).
2. The fans will decelerate. At low temperatures, they may reach 0 RPM.
3. Apply a controlled multi-core CPU workload (e.g., compile cargo release or run `stress -c 8`).
4. Monitor temperatures as they rise past 45°C, 55°C, 65°C, and 75°C:
   - **PASS Criteria:** Fans must resume spinning (tachometer > 1800 RPM) before CPU package exceeds 70°C, and scale RPM upward proportionally with temperature.
   - **FAIL Criteria (EC Freeze):** Temperatures exceed 75°C while fans remain locked at 0 RPM. (Immediately abort workload and re-enable Max Fan).

#### Stage 4: Workload Cessation & Return to Idle
1. Stop the workload.
2. Observe fans dissipating residual heat down to ~40°C.
3. Fans should progressively decelerate and may eventually return to 0 RPM once cold.

---

## 6. Engineering Conclusions for ZenOmen 1.0.0

1. **Passive zero-RPM at idle is normal and desirable** on 8A3D/F.22. Do not treat 0 RPM at <40°C as a bug.
2. **Never claim unverified thresholds** in documentation or UI.
3. **Preserve the validated 8A3D restoration sequence** in the Windows service backend.
4. **On Linux**, users must rely on the upstream kernel driver interface. If running on 8A3D under Linux, document that issuing raw `0x2E {0,0}` can stall the EC, and provide the safe reset workaround.
5. **Fail closed on unknown hardware.** If a motherboard ID does not match the validated registry, do not issue write commands.
