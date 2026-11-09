---
status: resolved
trigger: "GSD Debug: HP Victus 15-fb0xxx (board 8A3D) fan control fails with WBEM_E_INVALID_PARAMETER (0x80041008) and fan_write_verified=false."
created: 2026-11-09
updated: 2026-11-09
goal: find_and_fix
---

# Board 8A3D HP BIOS WMI fan path: root causes and remediation

## 1. Symptoms

- `zenomen-hw.exe --wmi-diag` reaches `ROOT\WMI`, finds `hpqBIntM`, `hpqBDataIn`,
  `hpqBDataOut128`, and all five `hpqBIOSInt*` methods.
- Instance enumeration returns `0x80041003` unless the process is elevated (expected).
- Once elevated, `hpqBIOSInt128` fails with `0x80041008` (`WBEM_E_INVALID_PARAMETER`).
- `query_fan_status` therefore never returns a mode; `SetFanMode` is refused by the
  `fan_write_verified = false` gate.

## 2. Method

Downloaded and interpreted the primary sources into `.planning/references/`
(see `REFERENCES.md` for the per-source evidence map):

1. upstream Linux `hp-wmi.c` — board `8A3D` → `victus_s_board_params`;
2. `VictusModeSwitch` HP BIOS protocol notes — independent confirmation of the envelope;
3. `FanControl.HpOmenPlugin` `OmenBios.cs` — a working `hpqBDataIn` construction;
4. Microsoft `IWbemServices::ExecMethod` reference.

Each claim below was then checked against the actual Rust source.

## 3. Root causes (verified against source)

### RC-1 — Writes used the wrong command family

`hp_wmi.rs` had `REQ_WRITE = 0x0002_0009` and passed it as `Command`. Upstream uses
`HPWMI_GM = 0x20008` for **reads and writes**; `0x20009` is the keyboard-lighting family.
Every fan write was therefore addressed to a family the firmware does not accept
(status `3`, unknown command).

### RC-2 — `hpqBData` was sent as a zero-element SAFEARRAY

Read paths passed `&[]`, producing a `SAFEARRAY` with `cElements = 0` for the
variable-length `hpqBData` property. The working reference omits `hpqBData` entirely and
sends `Size = 0`. A malformed/unset property makes the provider reject the whole input
object — exactly `0x80041008`.

**Live evidence (this host, `--wmi-diag`):** the real `hpqBDataIn` CIM types are
`Sign = uint8[] (0x2011)`, `Command/CommandType/Size = uint32 (0x0013)`,
`hpqBData = uint8[] (0x2011)`.

This **disproves** the pasted investigation's `Sign = UInt32`/fixed-size-array theory: the
envelope types were already correct. `hpqBData` is a *variable-length* array, so an empty
array is genuinely ambiguous and omission is the correct encoding. With RC-3 in place the
failed `Put` was invisible, which is why the error surfaced only as `0x80041008`.

### RC-3 — every `Put` HRESULT was discarded

Only the `InData` `Put` was checked. `Sign`, `Command`, `CommandType`, `Size` and
`hpqBData` could all fail silently, so any type mismatch surfaced only later as an opaque
`0x80041008`. This is the reason the failure was undiagnosable, not just a symptom.

### RC-4 — fan mode was read with the write opcode

`read_fan_mode_from_firmware` used `0x27` (`MAX_FAN_SET`) to read. `0x26`
(`MAX_FAN_SPEED_MAX_GET_QUERY`) is the read opcode; `0x27` is write-only with zero-length
output, so the read always failed with "zero-length payload".

### RC-5 — wrong `0x27` payload length

The Max Fan write sent a single byte. Firmware (and OMEN Gaming Hub) expect a 4-byte
little-endian integer `[0|1, 0, 0, 0]`.

### RC-6 — fabricated firmware return-code mapping

The old mapping was `2 = invalid parameters`, `4 = access denied`, `5 = busy/timeout`.
Upstream `hp_return_value` is `2 = wrong signature`, `3 = unknown command`,
`4 = unknown command type`, `5 = invalid parameters`. A signature or envelope problem was
reported as "invalid parameters", actively misdirecting the investigation.

### RC-7 — no Victus S implementation at all

Board `8A3D` is a Victus S board. Fan RPM must come from `0x2D` (100 RPM units), manual
levels from `0x2E`, and the level range from the `0x2F` fan table. The code hardcoded
`fan_rpm: false`, `has_custom_curves: false`, and returned `Vec::new()` for RPM, so the
device was treated as a generic HP OMEN with no tachometers.

### RC-8 — missing firmware status treated as success

`rc_var.extract_u32().unwrap_or(0)` turned a failed `Get(rwReturnCode)` into status `0`.
That can report a successful write that never happened.

### RC-9 — thermal-profile write payload was one byte

`0x1A` requires `{0xFF, profile}`. Also, Victus S reads the active profile from EC register
`0x59`, not WMI, so the WMI read path is not valid for this board.

## 4. Remediation

- `src/service/windows/hp_wmi.rs`
  - one command family (`HPWMI_GM`) for reads and writes; `0x20009` renamed
    `HPWMI_KB_LIGHTING` and no longer used for fan writes;
  - `hpqBDataIn` built exactly like the working reference: `Size` always set,
    `hpqBData` omitted when empty, **every `Put` checked** and reported with
    `GetErrorInfo` text;
  - `0x26` for reads, `0x27` with a 4-byte integer for writes;
  - upstream return-code mapping in one shared `firmware_error()` used by real and fake
    adapters;
  - `get_fan_count`, `get_fan_rpm`, `get_victus_fan_levels`, `get_victus_fan_table`,
    `get_max_fan_state`, `set_max_fan_state`, `set_victus_fan_levels`;
  - `query_fan_status` now reports real RPMs; fan count is cached so the 1 Hz telemetry
    poll never re-arms the firmware's 120 s manual-fan window;
  - missing `rwReturnCode`/`Data` is an error, not a fabricated success;
  - method selection by output size (`hpqBIOSInt0` has no `Data`);
  - `set_rgb_profile` refuses instead of sending a family ID as a command type.
- `src/service/windows/victus_s.rs` (new) — board routing, fan-table parsing, level
  conversion, 90 s/120 s constants, level-range validation.
- `src/service/windows/wmi_ffi.rs` — `GetErrorInfo` description capture.
- `src/service/windows/wmi_diag.rs` — correct FAN_COUNT call; new `hpqBDataIn` CIM type
  dump so a future type mismatch is provable rather than inferred.
- `src/service/windows/profiles.rs` — `has_custom_curves: true` for `8A3D`
  (protocol capability, not write authorization).
- `src/service/windows/server.rs` — shutdown Auto-restore only on write-validated boards;
  `8A3D` was never in Max mode, so no unvalidated write is issued.

## 5. Verification actually performed

- `cargo fmt --all -- --check` — clean.
- `cargo clippy --all-targets --all-features -- -D warnings` — clean.
- `cargo test --all` — all suites pass, including `windows_service_tests` 43/43.
- `zenomen-hw.exe --wmi-diag` (this host, **unelevated**) — namespace and class discovery
  verified; instance enumeration correctly reports `AccessDenied (0x80041003)`; the new
  property-type dump printed the CIM types quoted under RC-2.

## 6. Not verified (and why the write gate stays closed)

- No elevated `hpqBIOSInt128` call was executed from this session, so the payload fix is
  validated against upstream + a working reference, **not** against live firmware.
- No physical write, no RPM measurement, no fan-table read on `8A3D` hardware.
- The 120 s firmware timeout and the fan-table level range are inherited from upstream
  (validated on other boards), not measured on BIOS F.22.

`resolve_board_validation("8A3D", _)` therefore still returns
`fan_write_verified: false`, `power_write_verified: false`, `rgb_write_verified: false`.

## 7. Next evidence needed

1. Elevated `--wmi-diag` run, recording the printed `hpqBDataIn` CIM types and the
   FAN_COUNT probe result.
2. Read-only `0x2F` fan table + `0x2D` fan speed on the target laptop.
3. Controlled `0x26`/`0x27` Max Fan toggle with audible/RPM confirmation, then Auto restore.
4. Only then a conservative `0x2E` level test and a ratified `BoardValidation` record.
