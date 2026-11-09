---
status: resolved
trigger: "GSD Critical Fix: Correct HP BIOS WMI Method Resolution and Validate Victus 15-fb0xxx. GetMethod(hpqBMtdx) failed: 0x80041002 (WBEM_E_NOT_FOUND)."
created: 2026-11-09
updated: 2026-11-09
goal: find_and_fix
---

# HP BIOS WMI Method Resolution and Victus 15-fb0xxx Validation

## 1. Symptoms & Incident Report

- **Reported Error:**
  ```text
  HP WMI interface connected and class verified
  GetFanStatus: WMI unavailable: GetMethod(hpqBMtdx) failed: 0x80041002
  ```
- **Error Meaning:** `0x80041002` is `WBEM_E_NOT_FOUND`.
- **Root Cause Confirmed:** `hpqBMtdx` does not exist on `ROOT\WMI:hpqBIntM`. The real callable methods on `hpqBIntM` are `hpqBIOSInt0`, `hpqBIOSInt4`, `hpqBIOSInt128`, `hpqBIOSInt1024`, and `hpqBIOSInt4096`. Input parameters are encapsulated in `hpqBDataIn` as `"InData"`, and output parameters in `hpqBDataOut128` (or matching size variant) as `"OutData"`.

## 2. Completed Implementation & Verification

1. **Empirical WMI Discovery Completed (Task 1):**
   - Verified that `ROOT\WMI:hpqBIntM` (GUID `{5FB7F034-2C63-45E9-BE91-3D44E2C707E4}`) exposes `hpqBIOSInt128([in] hpqBDataIn InData, [out] hpqBDataOut128 OutData)`.
   - Verified `hpqBDataIn`: `Sign` (UInt8Array `b"SECU"`), `Command` (UInt32: `0x20008` read, `0x20009` write), `CommandType` (UInt32 opcode), `Size` (UInt32), `hpqBData` (UInt8Array).
   - Verified `hpqBDataOut128`: `Sign` (UInt8Array), `rwReturnCode` (UInt32: 0=Success), `Data` (UInt8Array[128]).
2. **COM FFI Hardened (`src/service/windows/wmi_ffi.rs` - Tasks 2 & 3):**
   - Fixed missing `DeleteInstanceAsync` slot in `IWbemServicesVtbl` to ensure exact COM ABI vtable alignment.
   - Added `ScopedVariant::from_unknown`, `ScopedVariant::from_bytes`, `extract_bytes`, and `extract_u32`.
   - Ensured clean COM memory management with `ComPtr` and `VariantClear` (no double frees).
   - Added `AddRef` on `VT_UNKNOWN` to ensure correct COM reference counting.
3. **Real Firmware Method Invocation Implemented (`src/service/windows/hp_wmi.rs` - Task 2):**
   - Added `bios_call` to `WmiAdapter` and implemented in `RealWmiAdapter` using `hpqBIOSInt128`.
   - Retained backward-compatible `exec_method` shim and deterministic `FakeWmiAdapter` for tests.
   - Mapped `rwReturnCode` to typed `HardwareControlError` variants.
4. **Read-Only WMI Diagnostic CLI (`src/service/windows/wmi_diag.rs` & `zenomen-hw --wmi-diag` - Task 4):**
   - Provides structured diagnostic reporting across all 9 categories (`NamespaceMissing`, `ClassMissing`, `InstanceMissing`, `MethodMissing`, `InputClassMissing`, `OutputClassMissing`, `ComFailure`, `ProviderFailure`, `AccessDenied`, `Healthy`).
   - Verified running `zenomen-hw.exe --wmi-diag` reports all 5 methods and identifies `AccessDenied (0x80041003: Elevation required)` when run unelevated.
5. **Board Safety Enforced (Task 6):**
   - Target board `8A3D` BIOS `F.22` remains strictly unvalidated for writes (`fan_write_verified: false`). All mutations remain blocked by the authorization gate.
6. **Protocol Specification Updated (`docs/architecture/HP_WMI_PROTOCOL.md` - Task 5):**
   - Replaced obsolete flat-buffer `hpqBMtdx` documentation with the verified `hpqBIOSInt128` specification.
7. **Verification Suite Clean (Task 7):**
   - `cargo fmt --all -- --check` passed.
   - `cargo clippy --all-targets --all-features -- -D warnings` passed with 0 warnings.
   - `cargo test --all` passed (all 36 Windows service tests, 8 Windows telemetry tests, 9 hardware service tests, and mock tests pass).
