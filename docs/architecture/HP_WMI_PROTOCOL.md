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
  - `Command`: `UInt32` = `0x20008` (`HPWMI_GM`) for **every** gaming/thermal/fan operation, read and write.
    `0x20009` is the **keyboard-lighting** command family and is never a write flag.
  - `CommandType`: `UInt32` = Opcode (`0x10` Fan Count, `0x11` Fan Speeds, `0x1A` Thermal Profile, `0x26` Max Fan Read, `0x27` Max Fan Set, `0x28` System Design Data, `0x2D`/`0x2E`/`0x2F` Victus S fan levels/table)
  - `Size`: `UInt32` = Number of *meaningful* payload bytes in `hpqBData` (not the buffer size)
  - `hpqBData`: `UInt8Array` = Effective command payload bytes. Omit the property entirely when `Size == 0`;
    a zero-element SAFEARRAY is not equivalent and is rejected by the provider.

- **Method selection:** by requested **output** size, mirroring `encode_outsize_for_pvsz()` in the Linux driver:
  `0 → hpqBIOSInt0` (no `Data` property), `≤4 → hpqBIOSInt4`, `≤128 → hpqBIOSInt128`,
  `≤1024 → hpqBIOSInt1024`, `≤4096 → hpqBIOSInt4096`.

- **Output Object Class (`hpqBDataOut128`):**
  - `Sign`: `UInt8Array[4]` = Magic signature verification
  - `rwReturnCode`: `UInt32` = Firmware return status code
  - `Data`: `UInt8Array[128]` = Returned firmware payload bytes

---

## 3. Protocol Return Codes

The firmware status is returned in the `rwReturnCode` property of `OutData`:

These values are `enum hp_return_value` in the upstream Linux driver and are corroborated by
independent reverse engineering. `0` is the only success value.

| Return Code | Name | Description | ZenOmen Handling |
|:---|:---|:---|:---|
| `0` | Success | Operation accepted and confirmed. | Extract `Data` payload and proceed. |
| `2` | Wrong Signature | The `Sign` field was not accepted as `SECU`; the envelope is malformed. | `HardwareControlError::ProtocolError`. |
| `3` | Unknown Command | Command family not recognised (e.g. using `0x20009` for a fan write). | `HardwareControlError::UnsupportedMode`. |
| `4` | Unknown Command Type | Opcode not supported on this board. | `HardwareControlError::UnsupportedMode`. |
| `5` | Invalid Parameters | Payload size or values invalid for this opcode. | `HardwareControlError::InvalidParameter`. |
| other | Undocumented | Not in the published enum. | `HardwareControlError::ProtocolError`. |

> Do not map these onto generic COM errors. The failing layer (COM HRESULT vs. firmware status)
> and the command type must both appear in the error message.

---

## 4. Hardware Feature Commands

### 4.1 Fan Mode Query & Mutation
- **Read current mode:** `CommandType = 0x26` (`HPWMI_FAN_SPEED_MAX_GET_QUERY`), 4-byte output, state in byte 0.
- **Write mode:** `CommandType = 0x27` (`HPWMI_FAN_SPEED_MAX_SET_QUERY`), **4-byte little-endian integer**
  (`[0,0,0,0]` = Auto, `[1,0,0,0]` = Max), zero-length output.
- **Payload Data Values:** `0x00` = Auto (firmware-managed curve), `0x01` = Max (100% fan RPM).

`0x27` is write-only. Reading with `0x27` returns no payload — a former defect in this codebase.
There is no `0x02 = Quiet` fan mode in this protocol.

### 4.2 Power & Thermal Profiles
- **Command ID:** `0x1A` (`HPWMI_SET_PERFORMANCE_MODE`).
- **Write payload:** two bytes `{0xFF, profile}` — the leading `0xFF` mirrors OMEN Gaming Hub and is
  required by the upstream driver. Zero-length output.
- **Profile values (Victus S):** `0x00` = Default/Balanced, `0x01` = Performance,
  `0x03` = Quiet (board `8A3D` EC layout, 2026-10-24 patch). Legacy OMEN V0 boards use
  `0x00`/`0x01`/`0x02` (Default/Performance/Cool).
- **Read path:** Victus S boards expose the active profile through the EC, not WMI — `0x95`
  for board `8A3D` (`victus_s_ec_thermal_params`), `0x59` where `omen_v1_thermal_params`
  applies. ZenOmen therefore reports `UnsupportedMode` instead of guessing a value.

### 4.3 4-Zone RGB Keyboard Lighting — **not implemented**

Lighting lives in the `CommandType` family `0x20009` (a *family*, not an opcode). No verified
per-opcode payload contract exists for the supported boards, so `set_rgb_profile` returns
`UnsupportedMode` rather than sending an unvalidated command. Board `8A3D` advertises
`rgb_zone_count = 0` and is gated by `BoardValidation` regardless.

### 4.4 Victus S fan levels & Fan Table parsing (board `8A3D` and siblings)

The upstream driver maps board `8A3D` to `victus_s_board_params`, so this board must **not** be
driven through generic OMEN fan logic.

| Operation | CommandType | Input | Output |
|:---|:---|:---|:---|
| Fan count (also arms manual state) | `0x10` | 1 byte (0) | 4 bytes, count = byte 0 |
| Victus S fan speed | `0x2D` | 1 byte (0) | 128 bytes, RPM = `byte[fan] * 100` |
| Victus S fan levels (write) | `0x2E` | 2 bytes `{cpu, gpu}` | none |
| Fan table | `0x2F` | 4 bytes | 128 bytes: header `{num_fans, ?}` then `{cpu, gpu, noise}×3` rows |

#### Fan Table & GPU Offset Derivation

Victus S fan tables on boards such as `8A3D` expose entries where row 0 contains `gpu_rpm == 0`.
Deriving a GPU level delta directly from row 0 would yield an invalid negative offset.
In ZenOmen (`FanTable::gpu_offset`), the GPU level offset relative to CPU level is derived from the
**first row with a non-zero GPU RPM**:
$$\text{gpu\_offset} = \text{row.gpu\_level as } i16 - \text{row.cpu\_level as } i16$$
For coupled manual level setting, `gpu_level_for_cpu` computes:
$$\text{gpu\_level} = \text{cpu\_level.saturating\_add\_signed}(\text{gpu\_offset}).\text{clamp}(0, 255)$$
If all rows report 0 GPU RPM or table is absent, `gpu_offset` is `None` and CPU and GPU levels are
treated independently without synthetic offsets.

#### Fan Control State Lifecycle

Fan control transitions are tracked via `FanControlState`:
- `Automatic`: Running default firmware/EC automatic curve.
- `Applying`: Firmware write currently in flight.
- `Manual`: Custom fan levels active.
- `MaxFan`: Full-speed 100% cooling override active (`0x27 = 1`).
- `Restoring`: Restoration sequence currently executing.
- `RestoreFailed`: Restoration refused or failed; hardware remains in last safe state without corrupting firmware.
- `Unsupported`: Board or firmware does not support fan control.

Manual-level write ordering (upstream `hp_wmi_fan_speed_set`):

1. `0x10` — refresh/arm the firmware's user-defined fan state.
2. `0x27 = 0` — Max Fan must be explicitly disabled.
3. `0x2E {cpu, gpu}` — both levels in one request, in fan-table RPM units (`0x00` = automatic).

Firmware reverts to its automatic curve after ~120 s; the driver refreshes every 90 s.
On Victus S boards other than `8A3D`, restoring Auto uses `0x27 = 0` **and** `0x2E {0, 0}`.
Board `8A3D` must **never** use that second step — see §4.5.

### 4.5 Automatic cooling restoration on `8A3D` (EC thermal profile)

Upstream patch 2026-10-24 (`lists.openwall.net/linux-kernel/2026/10/24/810`; posted, not
merged in the vendored snapshot) routes board `8A3D` to `victus_s_ec_board_params` instead of
`victus_s_board_params`:

| Fact | Value |
|:---|:---|
| Active profile storage | EC register `0x95` (`HP_OMEN_EC_THERMAL_PROFILE_OFFSET`) |
| Balanced / performance / quiet | `0x00` / `0x01` / `0x03` |
| Profile write | `0x1A`, 2-byte payload `{0xFF, value}` (WMI, not an EC write) |
| Hazard | `hp_wmi_fan_speed_max_reset()` (`0x27 = 0` + `0x2E {0,0}`) freezes the fans at 0 RPM under load |

Upstream's replacement sequence for `PWM_MODE_AUTO` on this board:

1. `0x27 = 0` — clear Max Fan.
2. `0x10` — fan-count trigger (as `platform_profile_victus_s_set_ec` does).
3. `0x1A {0xFF, active_profile}` — re-apply the active profile so the EC curves regain control.
4. (upstream only) `0x22` — re-write GPU CTGP/PPAB bytes. ZenOmen never changes those, so it
   does not rewrite them.

**ZenOmen policy** (`restore_automatic_fan_control`):
The active profile on `8A3D` lives in EC register `0x95`, which a userland Windows process cannot
access directly. Therefore, restoration takes `Option<ThermalProfile>`:

- `Some(profile)` → Executes the verified sequence: clear Max Fan (`0x27 = 0`), arm state (`0x10`), and re-apply active profile (`0x1A {0xFF, profile.value()}`). Never writes `0x2E`. If any step fails, the function halts immediately without writing subsequent opcodes.
- `None` → Immediately transitions state to `RestoreFailed` and returns `HardwareControlError::UnsupportedMode` with **zero firmware writes**. Refusing is safe (Max Fan remains active); sending `0x2E {0,0}` or guessing a profile is unsafe.
- **Shutdown Invariant:** On service shutdown, `restore_fan_mode_on_shutdown` checks `validation.fan_write_verified`. Since `fan_write_verified` is strictly `false` for unratified boards (`8A3D`), shutdown writes are safely skipped.

### 4.6 Opcode 0x4C Thermal Profile Query Candidate

- **Opcode:** `0x4C` (`HPWMI_THERMAL_PROFILE_QUERY`).
- **Command Family:** `HPWMI_GM` (`0x20008`) / `REQ_READ` (`0x01`).
- **Upstream semantics:** Used in Linux `hp-wmi` (`thermal_profile_get()`) to read active thermal profile integers on platforms with ACPI-managed thermal modes.
- **Diagnostics vs. Production:** Opcode `0x4C` is implemented strictly as a read-only probe in `zenomen-hw --wmi-diag` / `probe_thermal_profile_query`. It is **never** used to infer write authorization, and does not unlock mutations for board `8A3D`.
- **Validation Constraint:** Motherboard `8A3D` remains strictly blocked from fan and power writes (`fan_write_verified: false`) until physical verification per `docs/testing/HP_BOARD_VALIDATION.md` is approved and completed.

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
   WMI BIOS method evaluation (`hpqBIOSInt0/4/128/1024/4096` via `IWbemServices::ExecMethod`) is restricted strictly to `zenomen-hw.exe` running as `NT AUTHORITY\LocalSystem`. The desktop UI executable (`zenomen.exe`) possesses no direct WMI invocation or elevated privileges.
2. **Payload Fuzzing & Mutation Gating Defense:**
   The hardware service enforces board-specific validation flags (`BoardValidation`) in `src/service/windows/authorization.rs`. All mutation requests (`SetFanMode` for all modes including `"auto"`, `SetPowerProfile`, `ApplyPreset`, `ApplyRgbProfile`) are blocked unless the corresponding validation flag (`fan_write_verified`, `power_write_verified`, etc.) is explicitly true.
3. **Firmware Recovery Invariant:**
   On clean service shutdown, `zenomen-hw.exe` attempts to restore fan control to `Auto` only if `fan_write_verified` is true, avoiding unvalidated mutations during shutdown. On `8A3D`, unverified writes are safely skipped.
