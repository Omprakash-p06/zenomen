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
- **Method Name:** `hpqBMtdx`
- **WMI GUID:** `{5FB7F034-2C63-45E9-BE91-3D44E2C707E4}`

### WMI Method Signature
```text
uint32 hpqBMtdx(
    [in]  uint32 Sign,
    [in]  uint32 ReqType,
    [in]  uint32 InBufSize,
    [in]  uint8  InBuf[],
    [out] uint32 OutBufSize,
    [out] uint8  OutBuf[]
);
```

- **Signature (`Sign`):** `0x55434553` (ASCII `"SECU"`)
- **Query / Read ReqType:** `0x20008`
- **Mutation / Write ReqType:** `0x20009`

---

## 3. Buffer Protocol Layout

Both read (`0x20008`) and write (`0x20009`) queries exchange fixed-header payload buffers.

### 3.1 Input Buffer Layout
```text
Offset (Bytes)  Field Name      Type     Description
--------------------------------------------------------------------------
0x00 - 0x03     Command ID      u32      Specific feature command code (e.g. 0x1A, 0x27)
0x04 - 0x07     Subcommand      u32      Subcommand or query action code
0x08 - 0x0B     Data Length     u32      Size of effective input parameters
0x0C - 0x8B     Payload         [u8; 128] Feature-specific command parameters
```

### 3.2 Output Buffer Layout
```text
Offset (Bytes)  Field Name      Type     Description
--------------------------------------------------------------------------
0x00 - 0x03     Return Status   u32      0x00 = Success; Non-zero = Error Code
0x04 - 0x07     Output Length   u32      Number of valid payload bytes returned
0x08 - 0x87     Payload Data    [u8; 128] Returned telemetry or status bytes
```

### Status Return Codes
| Return Code | Description | ZenOmen Handling |
|:---|:---|:---|
| `0x00` | Success | Operation confirmed; return payload or ack. |
| `0x01` | Unknown / Unsupported Command | Log warning; mark feature unsupported for board profile. |
| `0x02` | Invalid Input Buffer / Size | Abort mutation; log validation error. |
| `0x04` | Access Denied / BIOS Password Locked | Mark hardware mutation unauthorized. |
| `0x05` | Hardware Busy / EC Timeout | Retry once after 200ms; return timeout error. |

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

ZenOmen matches the motherboard board ID (`BaseBoardProduct` via WMI `Win32_BaseBoard` or SMBIOS Type 2) before executing WMI writes. If a board ID is unverified, all write mutations are rejected to eliminate any risk of firmware corruption or unexpected EC states.

| Board ID | Model Series | Verified Fan Modes | Verified Power Profiles | RGB Zones |
|:---|:---|:---:|:---:|:---:|
| `88EE` | HP OMEN 16 (2022 AMD/Intel) | Auto, Max | Balanced, Performance | 4-Zone |
| `8A23` | HP OMEN 16 (2023 Intel/RTX40) | Auto, Max | Balanced, Performance | 4-Zone |
| `88F8` | HP Victus 16 (2022/2023) | Auto, Max | Balanced, Performance | 1-Zone / Off |
| `8BAB` | HP OMEN 17 (2023 Intel) | Auto, Max | Balanced, Performance | 4-Zone / Per-Key |
| `Unknown`| Unverified Board ID | Read-Only | Read-Only | Read-Only |

---

## 6. Security & Invariant Enforcements

1. **Restricted Execution Context:**
   WMI BIOS method evaluation (`hpqBMtdx`) is restricted strictly to `zenomen-hw.exe` running as `NT AUTHORITY\LocalSystem`. The desktop UI executable (`zenomen.exe`) possesses no direct WMI invocation or elevated privileges.
2. **Payload Fuzzing Defense:**
   The hardware service enforces an allowlist check on command ID, subcommand, and payload bounds (`src/service/windows/authorization.rs`). Arbitrary byte streams passed over the named pipe IPC are rejected immediately.
3. **Firmware Recovery Invariant:**
   On service shutdown (`SERVICE_CONTROL_SHUTDOWN` or service stop), fan modes are reset to `Auto` (`0x00`) to guarantee that cooling remains autonomous and safe when the service is stopped.
