# HP WMI BIOS Protocol Specification

- **Status:** Ratified (Wave 6)
- **Target Subsystem:** `src/service/windows/hp_wmi.rs`, `src/service/windows/profiles.rs`
- **Driver Reference:** Linux `hp-wmi` kernel driver (`drivers/platform/x86/hp/hp-wmi.c`) & clean-room Windows WMI bindings

---

## 1. Overview & WMI Interface Topology

HP OMEN and Victus laptops expose advanced platform controls (fan speed curves, thermal profiles, performance mode, and RGB keyboard lighting) via ACPI BIOS WMI methods. On Windows, ZenOmen interacts with these firmware methods directly via the Windows Management Instrumentation (WMI) subsystem under `root\wmi` without requiring unverified kernel drivers, WinRing0, or proprietary DLLs.

```text
+------------------------------------------------------------------------+
|                          ZenOmen Application                           |
+------------------------------------------------------------------------+
                                    | (Local Named Pipe IPC)
                                    v
+------------------------------------------------------------------------+
|           ZenOmen Hardware Control Service (zenomen-hw.exe)            |
|                           (LocalSystem)                                |
+------------------------------------------------------------------------+
                                    | (IWbemServices::ExecMethod)
                                    v
+------------------------------------------------------------------------+
|                  WMI Provider: root\wmi (hpqBIntM)                     |
|            GUID: {5FB7F034-2C63-45E9-BE91-3D44E2C707E4}               |
+------------------------------------------------------------------------+
                                    | (ACPI Method Evaluation)
                                    v
+------------------------------------------------------------------------+
|                       HP ACPI / UEFI BIOS                              |
|                       Embedded Controller (EC)                         |
+------------------------------------------------------------------------+
```

---

## 2. WMI GUID & Class Definition

- **Class Name:** `hpqBIntM`
- **Namespace:** `root\wmi`
- **Method Names:** `hpqBIOSInt128` (primary 128-byte payload), `hpqBIOSInt4`, `hpqBIOSInt0`, `hpqBIOSInt1024`, `hpqBIOSInt4096`
- **WMI GUID:** `{5FB7F034-2C63-45E9-BE91-3D44E2C707E4}`

### WMI Method Signature
```text
Boolean hpqBIOSInt128(
    [in]  hpqBDataIn    InData,
    [out] hpqBDataOut128 OutData
);
```

- **Input Object Class (`hpqBDataIn`):**
  - `Sign`: `UInt8Array[4]` = `[0x53, 0x45, 0x43, 0x55]` (ASCII `"SECU"`)
  - `Command`: `UInt32` = `0x20008` (Read/System Query) or `0x20009` (Write/Mutation)
  - `CommandType`: `UInt32` = Opcode (`0x10` Fan Count, `0x11` Fan Speeds, `0x1A` Thermal Profile, `0x26` Max Fan Read, `0x27` Max Fan Set, `0x28` System Design Data)
  - `Size`: `UInt32` = Byte length of data in `hpqBData`
  - `hpqBData`: `UInt8Array` = Effective command payload bytes

- **Output Object Class (`hpqBDataOut128`):**
  - `Sign`: `UInt8Array[4]` = Magic signature verification
  - `rwReturnCode`: `UInt32` = Firmware return status code
  - `Data`: `UInt8Array[128]` = Returned firmware payload bytes

---

## 3. Protocol Return Codes

The firmware status is returned in the `rwReturnCode` property of `OutData`:

| Return Code | Name | Description | ZenOmen Handling |
|:---|:---|:---|:---|
| `0` | Success | Operation accepted and confirmed. | Extract `Data` payload and proceed. |
| `1` | Error | General firmware internal failure. | Map to `HardwareControlError::ProtocolError`. |
| `2` | Invalid Parameters | Input payload size or values invalid. | Map to `HardwareControlError::InvalidParameter`. |
| `3` | Unsupported Command | Feature code unsupported on this motherboard. | Map to `HardwareControlError::UnsupportedMode`. |
| `4` | Access Denied | BIOS password locked or permission denied. | Map to `HardwareControlError::AuthorizationFailed`. |
| `5` | Busy / Timeout | Embedded Controller (EC) busy or communication timeout. | Map to `HardwareControlError::ConnectionFailed`. |

---

## 4. Hardware Feature Commands

### 4.1 Fan Mode Query & Mutation
- **Command ID:** `0x27` (Thermal / Fan Management)
- **Subcommands:**
  - Read Current Fan Mode: `0x00`
  - Set Fan Mode: `0x01`
- **Payload Data Values:**
  - `0x00`: Auto (Firmware managed fan curve)
  - `0x01`: Max (Force 100% maximum fan RPM on all fans)
  - `0x02`: Quiet (Silent thermal acoustic profile)

### 4.2 Power & Thermal Profiles
- **Command ID:** `0x28` (Platform Power Delivery & TDP Governor)
- **Subcommands:**
  - Read Thermal Profile: `0x00`
  - Write Thermal Profile: `0x01`
- **Payload Profiles:**
  - `0x00`: Balanced (Default OEM TDP & Fan table)
  - `0x01`: Performance (Max PL1/PL2 power limits, aggressive fan ramp)
  - `0x02`: Cool / Eco (Restricted PL1 limits, low skin temperatures)

### 4.3 4-Zone RGB Keyboard Lighting
- **Command ID:** `0x1A` (OMEN Zone Lighting)
- **Subcommands:**
  - Read Zone Colors: `0x00`
  - Write Zone Colors: `0x01`
- **Write Payload (16 Bytes):**
  - Bytes 0x00..0x03: Zone 1 (WASD) — `[Red, Green, Blue, 0x00]`
  - Bytes 0x04..0x07: Zone 2 (Left) — `[Red, Green, Blue, 0x00]`
  - Bytes 0x08..0x0B: Zone 3 (Center) — `[Red, Green, Blue, 0x00]`
  - Bytes 0x0C..0x0F: Zone 4 (Right) — `[Red, Green, Blue, 0x00]`

---

## 5. Board Profile Matching & Safe Verification Matrix

ZenOmen matches the motherboard board ID (`BaseBoardProduct` via WMI `Win32_BaseBoard` or SMBIOS Type 2) before executing WMI writes. If a board ID or BIOS version lacks a ratified physical validation record, all write mutations are rejected to eliminate any risk of firmware corruption or unexpected EC states.

### Status Classification:
- **Implemented:** Code structure and WMI FFI invocation written.
- **Unit Tested:** Tested against `FakeWmiAdapter` and protocol mocks.
- **Windows Runtime Tested:** Tested with live COM `ROOT\WMI` on Windows.
- **Physically Validated:** Confirmed on physical laptop (RPM changed, noise heard, read-back confirmed).
- **Unsupported:** BIOS or hardware does not accept the command.

| Board ID | Model Series | BIOS Version | Verification Status | Allowed Fan Modes | Allowed Power Profiles | RGB Zones |
|:---|:---|:---:|:---:|:---:|:---:|:---:|
| `88F8` | HP OMEN 15/16 (AMD Zen 3) | F.20, F.21 | Physically Validated | Auto, Max | Balanced, Performance | 4-Zone |
| `8A24` | HP OMEN 16 (Intel 12th/13th) | F.10 | Physically Validated | Auto, Max | Balanced, Performance | 4-Zone |
| `8A3D` | HP Victus 15-fb0xxx (AMD) | F.22 | **Unit Tested only (Read-Only)** | None (Writes Blocked) | None (Writes Blocked) | 0 (Monochrome) |
| `8A3E` | HP Victus 15/16 (Intel) | Any | **Unverified (Read-Only)** | None (Writes Blocked) | None (Writes Blocked) | 0 (Monochrome) |
| `88D0` | HP OMEN 15 (2020 Intel) | Any | **Unverified (Read-Only)** | None (Writes Blocked) | None (Writes Blocked) | 4-Zone |
| `Unknown`| Unverified Board ID | Any | **Unverified (Read-Only)** | None (Writes Blocked) | None (Writes Blocked) | None |

---

## 6. Security & Invariant Enforcements

1. **Restricted Execution Context:**
   WMI BIOS method evaluation (`hpqBMtdx`) is restricted strictly to `zenomen-hw.exe` running as `NT AUTHORITY\LocalSystem`. The desktop UI executable (`zenomen.exe`) possesses no direct WMI invocation or elevated privileges.
2. **Payload Fuzzing Defense:**
   The hardware service enforces an allowlist check on command ID, subcommand, and payload bounds (`src/service/windows/authorization.rs`). Arbitrary byte streams passed over the named pipe IPC are rejected immediately.
3. **Firmware Recovery Invariant:**
   On service shutdown (`SERVICE_CONTROL_SHUTDOWN` or service stop), fan modes are reset to `Auto` (`0x00`) to guarantee that cooling remains autonomous and safe when the service is stopped.
