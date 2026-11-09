---
status: resolved
trigger: "GSD Debug review: Windows native runtime validation, named-pipe security hardening (IU SDDL vs session validation, failed impersonation & RevertToSelf fail-closed semantics), test count accounting inconsistency (153 vs 154), and defining Phase 7 scope."
created: 2026-11-08
updated: 2026-11-08
goal: find_and_fix
---

# Windows Security, Test Accounting, and Runtime Validation Audit

## Executive Summary

Following the comprehensive review of the Windows cross-platform implementation, this debug session systematically addressed:
1. **Test Count Accounting Discrepancy**: Resolved mismatch between `TESTING.md` (153), `CONVENTIONS.md` (153), and `CONCERNS.md` (154). Empirical evaluation via `cargo test -- --list` and test inventory establishes the true baseline: **154 passing tests** (40 unit tests in `src/lib.rs` and 114 integration tests across 18 test suites in `tests/`).
2. **Native Windows Compilation & Test Compatibility**: Fixed Windows-specific compilation failures and panics where Linux-specific tests (`linux_procfs_tests`, `linux_gpu_tests`, `logging_tests`, mock sysfs paths with `:` colons in `rgb_tests`, and Linux host assertions in `sensor_registry_tests`) lacked OS target gating, enabling full clean `cargo test --all` on native Windows.
3. **Atomic File Write Bug on Windows**: Discovered and fixed a root-cause `ERROR_ACCESS_DENIED (5)` failure in `src/config/mod.rs:save_atomic` caused by invoking `file.sync_all()` (`FlushFileBuffers`) on a file opened with read-only access (`fs::File::open`), which Windows NTFS explicitly forbids. Resolved by using `fs::File::create` with write permissions.
4. **Race Condition in Sticky Hardware Test**: Eliminated a thread scheduling race condition in `tests/hardware_service_tests.rs:test_write_and_verify_mismatch_failure` on Windows by synchronizing thread startup and using tight `std::thread::yield_now()` loops.
5. **Windows Named Pipe Security Hardening**: Hardened `verify_client_session` in `src/service/windows/security.rs`. Checked `RevertToSelf()` return values on all exit paths; if `RevertToSelf` returns 0, the process immediately logs a critical error and terminates (`std::process::exit(1)`) to fail-closed, upholding Microsoft security invariants.
6. **Clippy & Code Style Zero-Warning Enforcement**: Fixed `clippy::not_unsafe_ptr_arg_deref` in `src/service/windows/lifecycle.rs` by scoping `report_service_status` to module visibility. Passed `cargo fmt --all -- --check` and `cargo clippy --all-targets --all-features -- -D warnings` with zero warnings.
7. **Phase 7 Definition**: Formally established Phase 7 in `.planning/ROADMAP.md` and `.planning/codebase/CONCERNS.md`: **"Windows Native Runtime Validation and Security Hardening"**, clearly delineating compiled/mock-tested contracts from hardware-verified releases.

---

## 1. Test Accounting Analysis

- **`cargo test -- --list` Source Breakdown:**
  - `src/lib.rs`: 40 unit tests across 11 modules (`state`: 10, `telemetry`: 6, `worker`: 2, `polkit`: 1, `protocol`: 6, `system_integration`: 1, `unavailable`: 1, `cpu`: 5, `gpu`: 3, `memory`: 2, `hwmon`: 3).
  - `tests/`: 114 integration tests across 18 test files:
    - `benchmark_tests.rs`: 5 tests
    - `config_tests.rs`: 6 tests
    - `degradation_tests.rs`: 8 tests
    - `desktop_integration_tests.rs`: 12 tests
    - `diagnostics_tests.rs`: 2 tests
    - `fan_control_tests.rs`: 5 tests
    - `hardware_service_tests.rs`: 9 tests
    - `linux_gpu_tests.rs`: 7 tests
    - `linux_procfs_tests.rs`: 4 tests
    - `logging_tests.rs`: 1 test
    - `mock_hardware_tests.rs`: 3 tests
    - `packaging_tests.rs`: 5 tests
    - `power_preset_tests.rs`: 6 tests
    - `rgb_tests.rs`: 9 tests
    - `sensor_registry_tests.rs`: 11 tests
    - `telemetry_history_tests.rs`: 5 tests
    - `windows_service_tests.rs`: 8 tests
    - `windows_telemetry_tests.rs`: 8 tests
  - **Sum:** 40 unit + 114 integration = **154 total tests**.
- **Resolution:** Updated `TESTING.md:60` and `CONVENTIONS.md:48, 194` from 153/113 to 154/114 to agree with `CONCERNS.md:13` and the actual source inventory.

---

## 2. Security Analysis & Remediation

### A. Named Pipe DACL & SDDL (`IU` vs Interactive Active Console Session)
- **Documented SDDL:** `D:(A;;GA;;;SY)(A;;GA;;;BA)(A;;GRGW;;;IU)`
- **Observation:** `IU` represents the *Interactive Users* well-known group, which includes all interactive logon sessions on the machine, including secondary local console switch sessions and Remote Desktop (RDP) sessions.
- **Enforcement:** SDDL alone is necessary but insufficient. ZenOmen layers `verify_client_session(pipe)` on top of the DACL before decoding length-prefixed frames.
- **Fail-Closed Hardening:**
  - If `ImpersonateNamedPipeClient` fails, the server rejects the connection immediately, closes the pipe handle, and returns without executing the client request under the service's own `LocalSystem` identity.
  - If `OpenThreadToken` fails or `GetTokenInformation` fails, `RevertToSelf()` is executed. If `RevertToSelf()` returns 0, the thread could theoretically remain in the caller's security context; the implementation now explicitly calls `std::process::exit(1)` to abort immediately.
  - The caller's `TokenSessionId` is strictly asserted to match `WTSGetActiveConsoleSessionId()`. If not matched, `AccessDenied` is returned and the pipe is disconnected.

---

## 3. Phase 7 Scope: Windows Native Runtime Validation and Security Hardening

Added to `.planning/ROADMAP.md` as the official next phase before release verification:

1. **Native Windows 10/11 Test Environments**:
   - Validation on clean Windows 10 22H2 and Windows 11 23H2/24H2 test machines.
2. **SCM Service Lifecycle**:
   - Live installation via `sc create`, startup pending/running transitions, stop pending/stopped transitions, and service recovery restart policies.
3. **Named Pipe Multi-Session Security Verification**:
   - Test suite executing across session boundaries:
     - Active console interactive user (permitted).
     - Elevated administrator (permitted).
     - Secondary local session / Fast User Switching (rejected).
     - Remote Desktop (RDP) session (rejected).
     - Secondary service account / Network Service (rejected).
     - Simulated impersonation failure and revert-to-self failure (fail-closed exit).
4. **Persistent GPU PDH Collector Multi-Engine Validation**:
   - Verification across AMD, Intel, and NVIDIA configurations to confirm `\GPU Engine(*)\Utilization Percentage` aggregation behaves accurately without multi-engine inflation.
5. **Connection & Handle Resilience**:
   - Soak testing with rapid connect/disconnect bursts, broken pipes, and oversized payloads to guarantee zero handle leaks.
6. **Physical HP Hardware & BIOS WMI Discovery**:
   - Read-only discovery on physical OMEN and Victus chassis.
   - Individual board profile verification per `docs/testing/HP_BOARD_VALIDATION.md` before enabling mutation gates.
