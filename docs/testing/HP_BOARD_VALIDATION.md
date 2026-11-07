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
Add the verified entry to `src/service/windows/profiles.rs`:
```rust
BoardProfile {
    board_id: "88EE",
    model_name: "HP OMEN 16 (2022 AMD)",
    supported_fan_modes: &[FanMode::Auto, FanMode::Max],
    supported_power_presets: &[PowerPreset::Balanced, PowerPreset::Performance],
    rgb_zones: 4,
}
```
Submit PR accompanied by diagnostic log evidence.
