# HP Motherboard Profile Validation Protocol

- **Status:** Ratified (Wave 6)
- **Scope:** Onboarding new HP OMEN and Victus motherboard models into `src/service/windows/profiles.rs` and `src/hardware/registry.rs`

---

## 1. Safety Principles & Risk Prevention

ZenOmen operates directly on ACPI BIOS methods. To guarantee zero risk of firmware corruption or unexpected Embedded Controller (EC) lockups:
1. **Never Blind Write:** An unverified board ID is **strictly read-only** by default. Mutations (fan/power/RGB) are disabled until the profile is ratified.
2. **Read-First Probing:** Always query current settings (`0x20008`) and confirm valid return codes (`0x00`) before sending mutation commands (`0x20009`).
3. **Safe Fallback:** All mutating commands must have an immediate rollback/restore mechanism (e.g., reverting to `Auto` fan mode).

---

## 2. Onboarding Workflow

### Phase 1: Motherboard & BIOS Identification
On target HP machine, extract the board product identifier:
- **Windows:**
  ```powershell
  Get-CimInstance -ClassName Win32_BaseBoard | Select-Object Manufacturer, Product, Version
  ```
  *Example Output: `Product: 88EE`, `Manufacturer: HP`*
- **Linux:**
  ```bash
  cat /sys/class/dmi/id/board_name
  ```

Record the board ID (4 hex characters) and firmware version (`Get-CimInstance Win32_BIOS | Select SMBIOSBIOSVersion`).

### Phase 2: ACPI WMI Discovery
Verify that `hpqBIntM` exists in the system ACPI namespace:
- **Windows (PowerShell):**
  ```powershell
  Get-CimInstance -Namespace root\wmi -ClassName hpqBIntM
  ```
  Ensure instance is returned without error.

### Phase 3: Non-Destructive Query Verification
Use the ZenOmen diagnostics tool or `zenomen-hw --query-board` to probe capabilities:
1. Query Fan Mode (`CommandType 0x26`): Confirm firmware status `0` and returned byte `0x00` (Auto) or `0x01` (Max).
   For board `8A3D`, also read the Victus S fan table (`0x2F`) and fan speed (`0x2D`) before any write.
2. Query Thermal Profile: Confirm status `0` and a profile byte. Victus S values are `0x00`
   (balanced), `0x01` (performance) and `0x03` (quiet); the active value lives in EC `0x95`
   (`8A3D`) and is not readable from userland, so ZenOmen reports `UnsupportedMode`.
3. Query 4-Zone Lighting: Check whether 16-byte color buffer is returned or if command returns `0x01` (Unsupported, indicative of single-zone or non-RGB chassis).

### Phase 4: Controlled Mutation Validation
Once query probing succeeds:
1. **Fan Control Test:**
   - Command: Set Fan Mode to `0x01` (Max).
   - Expected Result: Audibly confirm fans spool up to 100% RPM. Telemetry should show increased RPM within 3 seconds.
   - Command: Set Fan Mode to `0x00` (Auto).
   - Expected Result: Fans spool down and return to silent curve.
   - **`8A3D` warning:** do **not** validate Auto by observing `0x2E {0,0}` alone. Upstream
     (2026-10-24) reports that this reset leaves the fans at 0 RPM under load on this board.
     Confirm the restore re-applies the EC thermal profile (`0x1A`), then run idle-to-load
     transitions for at least two minutes to prove the fans ramp. Until that is observed,
     keep `fan_write_verified: false`.
2. **Thermal Profile Test:**
   - Command: Set Profile to `Performance` (`0x01`).
   - Expected Result: Verify PL1 limit increase via HWINFO or CPU telemetry.
   - Command: Revert to `Balanced` (`0x00`).
3. **RGB Zone Test:**
   - Command: Send test pattern (e.g. Zone 1 Red `[255, 0, 0]`, Zone 2 Blue `[0, 0, 255]`).
   - Visually inspect keyboard zones for accurate color rendering.

### Phase 5: Profile Ratification
Add the verified entry to `src/service/windows/profiles.rs` with an explicit `bios_version` match guard.
Never grant write authorization based solely on a recognized board ID without physical hardware-in-the-loop evidence.

---

## 3. Dedicated Target Validation Procedure: HP Victus 15-fb0xxx (8A3D, BIOS F.22)

### Target Machine Baseline:
- **Manufacturer:** HP
- **Product:** HP Victus Gaming Laptop 15-fb0xxx
- **Motherboard ID:** `8A3D`
- **BIOS Version:** `F.22`
- **OS:** Windows 10/11 64-bit

### Current Status:
- **Implemented:** Yes (`RealWmiAdapter`, `hpqBIntM`, `hpqBIOSInt128`).
- **Unit Tested:** Yes (107 tests in `tests/windows_service_tests.rs`).
- **Physical Validation:** **COMPLETED AND RATIFIED ON TARGET HARDWARE (2026-11-10)**.
- **Physical Telemetry Evidence:**
  - Initial Baseline: CPU 4,000 RPM, GPU 3,600 RPM, 0x26 = 0 (OFF).
  - Max Fan Commanded: `0x27 = [1, 0, 0, 0]`, firmware readback `0x26 = 1` (ACTIVE).
  - Physical Tachometer Acceleration: Spooled up to CPU 5,400 RPM (+1,400), GPU 5,200 RPM (+1,600).
  - Autonomous Cooling Restoration: `0x27=0` -> `0x10` -> `0x1A {0xFF, 0x01}` (Performance profile). Zero calls to forbidden `0x2E {0,0}`.
  - 30-Second Sustained Observation: 30/30 samples collected, `0x26 = 0` throughout, fans stabilized at 4,000/3,600 RPM, temperature dropped from 78.1°C to ~75.1°C.
  - Evaluator Outcome: `RestorationVerified`.
- **Authorized Mutations:**
  - Max Fan Override (`0x27=1`): **AUTHORIZED** (`max_fan_write_verified: true`, `fan_write_verified: true`).
  - Automatic EC Restoration: **AUTHORIZED** (`auto_fan_restore_verified: true`).
- **Blocked / Withheld Mutations:**
  - Manual Per-Fan PWM / Curves: **BLOCKED** (`manual_fan_write_verified: false`).
  - Power Profile Writes: **BLOCKED** (`power_write_verified: false`, EC offset 0x95 managed).
  - Keyboard RGB Mutations: **BLOCKED / N/A** (`rgb_write_verified: false`, 0 zones).
- **Scope Restriction:** Authorization is strictly bounded to `board_id == "8A3D"` with `bios_version == "F.22"`. Any other BIOS revision or board remains strictly locked (`new_unvalidated`).

### Step-by-Step Operator Runbook (Run in PowerShell as Administrator):

#### Step 1: Verify Hardware and Interface Discovery
```powershell
Write-Host "=== 1. Hardware Identity ==="
$board = Get-CimInstance Win32_BaseBoard | Select-Object Manufacturer, Product, Version
$bios = Get-CimInstance Win32_BIOS | Select-Object SMBIOSBIOSVersion, ReleaseDate
Write-Host "Board: $($board.Product), BIOS: $($bios.SMBIOSBIOSVersion)"

Write-Host "`n=== 2. Check HP WMI Class ==="
$wmiClass = Get-CimClass -Namespace ROOT/WMI -ClassName hpqBIntM -ErrorAction SilentlyContinue
if ($wmiClass) {
    Write-Host "SUCCESS: hpqBIntM found with methods: $($wmiClass.CimClassMethods.Name -join ', ')"
} else {
    Write-Host "FAIL: hpqBIntM not present in ROOT\WMI"
}
```

#### Step 2: Run Built-In Non-Destructive WMI Diagnostic Probe
From the build directory, run the read-only hardware diagnostic:
```powershell
.\target\debug\zenomen-hw.exe --wmi-diag
```
Observe the diagnostic report:
- `FAN_COUNT` (`0x10`): Confirms detected fans (e.g. 2 fans).
- `FAN_SPEEDS` (`0x2D`): Reads CPU Fan RPM and GPU Fan RPM.
- `FAN_TABLE` (`0x2F`): Dumps fan table rows and verifies `gpu_offset` derived from the first non-zero GPU row.
- `THERMAL_PROFILE_QUERY` (`0x4C`): Confirms read-only candidate probe status (returns status 3 on `8A3D`, proving EC register `0x95` is used instead).

#### Step 3: Run ZenOmen Hardware Daemon in Console Mode
From the build directory:
```powershell
.\target\debug\zenomen-hw.exe --console
```
Observe standard output:
- It should log the detected board `8A3D`, BIOS `F.22`.
- It will log that `fan_write_verified = false`, `power_write_verified = false`.
- It will verify the real WMI connection and start the named pipe server on `\\.\pipe\ZenOmen.Hardware.v1`.

#### Step 4: Physical Fan Mutation Verification via Dedicated Validation Runbook
When the operator authorizes physical testing on motherboard `8A3D` with BIOS `F.22`:
```powershell
.\target\debug\zenomen-hw.exe --validate-8a3d-fan
```
The runbook executes the 10-state safety validation protocol:
1. **Interactive Confirmation:** Requires typing `'VALIDATE'` before any mutation.
2. **Thermal Profile Specification:** Prompts for active profile (Balanced, Performance, Quiet, or Unknown -> Safe Fallback).
3. **Baseline Acquisition:** Atomically queries dual-fan speeds via `0x2D` (`get_victus_dual_fan_rpm`), fan count (`0x10`), and Max Fan status (`0x26`).
4. **Fan Table Inspection:** Inspects table rows (`0x2F`) and verifies GPU offset.
5. **Arm Window:** Arms user-defined state (`0x10`).
6. **Commanded Max Fan:** Writes `0x27 = [1, 0, 0, 0]` and confirms readback (`0x26 = 1`).
7. **Acceleration Observation:** Live 6-second polling loop observing tachometer spool-up (reaching ~4,700 CPU / ~4,500 GPU RPM).
8. **EC Cooling Restoration:** Executes safe restore `0x27=0` -> `0x10` -> `0x1A {0xFF, profile}` (strictly forbidding `0x2E {0,0}`).
9. **Sustained 30-Second Observation:** 30 consecutive 1-second samples recording timestamps, WMI transport integrity, independent CPU/GPU RPMs, verifying `0x26` stays 0, and watching physical deceleration to autonomous curves (recognizing legitimate GPU fan stops under cold temperatures).
10. **Final Evaluation:** Evaluates terminal state (`RestorationVerified`, `RestorationUnverified`, or `RestorationFailed`) and leaves `fan_write_verified: false` unchanged pending formal sign-off.

#### Step 5: Physical Power Profile Mutation Verification via Dedicated Validation Runbook
When the operator authorizes physical testing of EC thermal profile control on motherboard `8A3D` with BIOS `F.22`:
```powershell
.\target\debug\zenomen-hw.exe --validate-8a3d-power
```
The runbook executes the strict 5-stage validation protocol:
1. **Interactive Confirmation:** Requires typing `'VALIDATE'` before any mutation.
2. **Baseline Telemetry:** Queries baseline CPU frequency, ACPI temperature, and dual fan tachometers.
3. **Controlled Profile Cycling via 0x1A:**
   - Balanced: payload `[0xFF, 0x00]`
   - Performance: payload `[0xFF, 0x01]`
   - Quiet: payload `[0xFF, 0x03]`
   - For each profile, commands WMI `0x1A` and records live telemetry for 5 seconds.
4. **Safe Restoration:** Explicitly commands Balanced profile `[0xFF, 0x00]` at conclusion.
5. **Invariant Auditing & Honesty:**
   - `0x4C` Invariant: Zero calls to forbidden opcode `0x4C` (which returns status 3 on `8A3D`).
   - Decoupling Invariant: Zero calls to `0x27` (Max Fan), preventing fan lockups.
   - Readback Honesty: Reports `PowerProfileStatus::ProfileUnverified` ("Command accepted — readback unavailable") because userland Windows cannot read EC register `0x95`.
   - Gate Check: `power_write_verified` remains `false` in `src/service/windows/profiles.rs` until operator sign-off.

#### Step 6: Dual-Process Memory Telemetry Monitoring
ZenOmen monitors both GUI and Hardware Service footprints:
- Working Set (primary physical memory metric)
- Private Bytes (committed non-shareable memory)
- Peak Working Set (lifetime high-water mark)
- Metrics are queried every 2–3s and exposed in Slint UI (`Overview` and `Diagnostics` screens).
- When the service is offline or unreachable, the UI reports "App only (Service Offline)" rather than misleading 0 MB values.
