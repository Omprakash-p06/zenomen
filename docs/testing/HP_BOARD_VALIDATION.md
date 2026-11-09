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
1. Query Fan Mode: Confirm return status is `0x00` and returned byte is `0x00` (Auto) or `0x01` (Max).
2. Query Thermal Profile: Confirm status `0x00` and returned profile is `0x00`, `0x01`, or `0x02`.
3. Query 4-Zone Lighting: Check whether 16-byte color buffer is returned or if command returns `0x01` (Unsupported, indicative of single-zone or non-RGB chassis).

### Phase 4: Controlled Mutation Validation
Once query probing succeeds:
1. **Fan Control Test:**
   - Command: Set Fan Mode to `0x01` (Max).
   - Expected Result: Audibly confirm fans spool up to 100% RPM. Telemetry should show increased RPM within 3 seconds.
   - Command: Set Fan Mode to `0x00` (Auto).
   - Expected Result: Fans spool down and return to silent curve.
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
- **Implemented:** Yes (`RealWmiAdapter`, `hpqBIntM`, `hpqBMtdx`).
- **Unit Tested:** Yes (36 tests in `tests/windows_service_tests.rs`).
- **Physical Validation:** **PENDING EXECUTION BY OPERATOR ON TARGET HARDWARE**.
- **Mutations Allowed:** **NONE** (Safely blocked by `resolve_board_validation("8A3D", "F.22")`).

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

#### Step 2: Run ZenOmen Hardware Daemon in Console Mode
From the build directory:
```powershell
.\target\debug\zenomen-hw.exe --console
```
Observe standard output:
- It should log the detected board `8A3D`, BIOS `F.22`.
- It will log that `fan_write_verified = false`, `power_write_verified = false`.
- It will verify the real WMI connection and start the named pipe server on `\\.\pipe\ZenOmen.Hardware.v1`.

#### Step 3: Run Safe Hardware Query Tests
From a separate PowerShell prompt:
```powershell
# Using the client test or pipe query:
# Fan status query will read actual firmware via WMI command 0x27 subcmd 0x00.
```

#### Step 4: Physical Fan Mutation Verification (When Approved)
If the operator authorizes physical testing:
1. Verify initial fan state is `auto`.
2. Issue `SetFanMode("max")`.
3. Check daemon log:
   - Does `hpqBMtdx` return `0x00` (Success) or `0x01` (Unsupported)?
   - If `0x00`, does the post-write read-back confirm `max`?
   - Do the fans audibly spool up to 100% RPM within 3 seconds?
4. Immediately issue `SetFanMode("auto")` or stop the daemon (daemon automatically restores `auto` on shutdown).
5. Record whether the BIOS accepted or restricted manual fan control.
6. If confirmed working, update `resolve_board_validation` in `src/service/windows/profiles.rs` to mark `fan_write_verified: true` for `8A3D` with BIOS `F.22`.
