# ADR-002: Direct HP BIOS Interface via WMI hpqBIntM vs Kernel Drivers

- **Status:** Accepted (2026-11-07)
- **Date:** 2026-11-07
- **Decision Owner:** ZenOmen Maintainers & Hardware Team
- **Related:** `docs/architecture/HP_WMI_PROTOCOL.md`, `docs/architecture/WINDOWS_HARDWARE.md`

## Context

On Linux, ZenOmen interacts with HP hardware via the in-tree `hp-wmi` platform kernel driver exposed through sysfs. On Windows, vendor software (HP OMEN Gaming Hub) controls cooling, power, and lighting by calling BIOS WMI interfaces.

To achieve hardware parity on Windows 10 and 11, three approaches were evaluated:
1. **Third-Party Ring-0 Kernel Drivers (WinRing0 / InpOut32 / OpenLibSys):**
   Direct port I/O and EC register manipulation from user space.
2. **Proprietary Vendor DLL Interception:**
   Reverse-engineering and dynamically loading HP's proprietary background service libraries (`HpSystemDiagnostics.dll`, etc.).
3. **Direct ACPI BIOS Invocation via WMI `hpqBIntM` (`{5FB7F034-2C63-45E9-BE91-3D44E2C707E4}`):**
   Pure user-space WMI method execution (`hpqBMtdx`) provided by Microsoft Windows and HP's standard UEFI ACPI tables.

## Decision

**ZenOmen adopts Option 3: Direct ACPI BIOS Invocation via WMI `hpqBIntM`.**

### Rationale:
1. **Zero Kernel Driver Vulnerabilities:**
   WinRing0 and related ring-0 drivers have severe CVE histories (arbitrary kernel memory read/write) and are blocked by modern Windows 11 Driver Signature Enforcement (HVCI / Memory Integrity). Shipping a ring-0 driver would prevent ZenOmen from running on modern secured Windows 11 configurations.
2. **Vendor Native Compatibility:**
   HP's UEFI ACPI tables implement the `hpqBMtdx` method natively. Invoking this method operates through standard Windows WMI infrastructure (`IWbemServices::ExecMethod`), exactly as OEM management tools do.
3. **Clean-Room Implementation:**
   ZenOmen requires no proprietary DLL dependencies or reverse-engineered closed-source binaries. All interactions conform to documented ACPI WMI buffers.

## Consequences

**Positive:**
- Seamless compatibility with Windows 10 22H2 through Windows 11 26H2 with Memory Integrity / HVCI enabled.
- Zero kernel drivers to compile, sign, or maintain across Windows versions.
- Safe firmware boundaries: ACPI EC handlers maintain hardware safety limits.

**Negative / Accepted:**
- Elevated privilege (`LocalSystem`) is required to invoke mutating methods under `root\wmi`. This necessitated creating the `zenomen-hw` Windows service.
- Board IDs must be checked against an allowlist to prevent calling unsupported subcommands on unverified models.
