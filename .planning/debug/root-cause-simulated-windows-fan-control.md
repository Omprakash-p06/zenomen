---
status: resolved
trigger: "Root cause resolution: Replace simulated Windows HP fan control with genuine COM WMI firmware control and independent read-back verification."
created: 2026-11-08
updated: 2026-11-09
goal: replace_simulated_wmi_with_verified_firmware_control
---

# Root Cause Resolution: Replacing Simulated Windows Fan Control with Real Firmware Control

## 1. Defect Analysis & Root Cause

An audit of `src/service/windows/hp_wmi.rs` and related modules identified four critical deficiencies:
1. `connect_hp_wmi()` returned `Ok(HpWmiConnection { is_available: true })` unconditionally without establishing a COM apartment or connecting to `ROOT\WMI`.
2. Hardware capabilities were hardcoded to all-true regardless of what firmware exposed.
3. Fan mode and power profile were stored in process-local `static Mutex<Option<String>>` variables; queries returned this static state instead of querying the Embedded Controller (EC).
4. `resolve_board_validation` automatically granted `fan_write_verified: true` to boards `8A3D` and `8A3E` merely based on board ID recognition, bypassing physical hardware validation.

This created false positive reporting where the UI displayed "Max - Active" and the daemon reported success while the physical fans remained unmutated.

## 2. Architecture & Implementation Remediation

### Phase 1 & 2: Real Win32 COM WMI Implementation (`wmi_ffi.rs` & `hp_wmi.rs`)
- Created `src/service/windows/wmi_ffi.rs` implementing strict Win32 COM FFI bindings matching `WbemCli.h`:
  - `IWbemLocator` with `ConnectServer` to `ROOT\WMI`.
  - `IWbemServices` with `GetObject`, `ExecQuery`, and `ExecMethod`.
  - `IWbemClassObject` with `GetMethod`, `SpawnInstance`, `Put`, and `Get`.
  - `IEnumWbemClassObject` with `Next`.
  - RAII wrappers: `ComPtr<T>` (automatic `Release`), `Bstr` (`SysFreeString`), `ScopedVariant` (`VariantClear`).
- Created `WmiAdapter` trait in `src/service/windows/hp_wmi.rs`:
  - `RealWmiAdapter`: Initialises COM, verifies presence of `hpqBIntM`, and invokes `hpqBIntM::hpqBMtdx` with signature `0x55434553` ("SECU").
  - `FakeWmiAdapter`: Deterministic FIFO mock adapter for unit and CI testing.

### Phase 3 & 4: Mandatory Pre-Write and Post-Write Read-Back
- Deleted process-local statics `CURRENT_FAN_MODE` and `CURRENT_POWER_PROFILE`.
- `query_fan_status` reads actual firmware state via command `0x27` subcommand `0x00`. Returns honest empty tachometer vector when unmeasured.
- `set_fan_mode`:
  1. Reads current firmware mode before writing.
  2. If already matching target, skips redundant write.
  3. Writes command `0x27` subcommand `0x01`.
  4. Waits 150 ms for EC to apply changes.
  5. Performs independent post-write read-back.
  6. Compares actual state with requested state; returns `HardwareControlError::VerificationFailed` on mismatch.
- Added firmware safety invariant in `server.rs`: on service shutdown or SCM stop, fan mode is restored to `Auto` (`0x00`).

### Phase 5: Board Safety Policy Hardening (`profiles.rs` & `authorization.rs`)
- In `src/service/windows/profiles.rs`, `resolve_board_validation`:
  - Board `8A3D` (HP Victus 15-fb0xxx, BIOS F.22) is explicitly marked **unvalidated for writes** (`fan_write_verified: false`, `power_write_verified: false`).
  - Read-only probing is permitted; write mutations return `HardwareControlError::UnsupportedMode`.
  - Automatic write verification is removed for `8A3E` and `88D0`.
  - Validation requires an explicit `bios_version` match guard.

### Phase 6 & 7: Test Coverage & Verification
- Rewrote `tests/windows_service_tests.rs`:
  - 36 automated unit and service-level integration tests.
  - Tests verify: WMI unavailable error propagation, empty tachometer honesty, invalid mode rejection before WMI, unknown command rejection, access denied handling, pre-write skipping, read-back verification mismatch failure, and authorization gating for board 8A3D.
- Full workspace tests: 150 passed; 0 failed; 0 ignored across 18 test suites.
- Clippy zero-warning check: `cargo clippy --all-targets --all-features -- -D warnings` passed cleanly.
- Formatting check: `cargo fmt --all -- --check` passed cleanly.

### Phase 8 & 9: Physical Validation Runbook & Documentation
- Updated `docs/testing/HP_BOARD_VALIDATION.md` with Section 3: A dedicated physical hardware-in-the-loop validation procedure for HP Victus 15-fb0xxx (board 8A3D, BIOS F.22).
- Updated `docs/architecture/HP_WMI_PROTOCOL.md`, `docs/architecture/WINDOWS_FUNCTION_CONTRACTS.md`, `docs/testing/WINDOWS_IMPLEMENTATION_AUDIT.md`, `.planning/STATE.md`, and `.planning/ROADMAP.md`.

