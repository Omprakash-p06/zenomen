# Plan 06-04: Windows Hardware Control Service & Named-Pipe IPC Summary

**Status:** Completed
**Phase:** 06-cross-platform-windows-linux-release
**Wave:** 4

## Overview
Plan 06-04 implemented the privileged Windows hardware service (`zenomen-hw.exe`), secure local named-pipe IPC layer (`\\.\pipe\ZenOmen.Hardware.v1`), and the HP BIOS WMI mailbox driver (`hpqBIntM`) matching the exact module hierarchy and function signatures in Sections 4.1, 4.2, and 5.2 of the specification.

## Delivered Artifacts & Implementation Details

1. **Named Pipe IPC & Framing (`src/service/windows/pipe.rs` & `protocol.rs`)**:
   - `PIPE_NAME`: `\\.\pipe\ZenOmen.Hardware.v1`.
   - `create_server_pipe`: Creates server pipe with `PIPE_ACCESS_DUPLEX`, `FILE_FLAG_FIRST_PIPE_INSTANCE`, `FILE_FLAG_OVERLAPPED`, and `PIPE_READMODE_MESSAGE` (buffer 64 KB, max 4 instances).
   - `encode_frame` and `decode_frame`: Length-prefixed binary framing enforcing `MAX_PAYLOAD_SIZE = 65536` bytes (64 KB).
   - Typed request/response protocol (`HardwareRequest` and `HardwareResponse`) matching the domain contract.
2. **Interactive Session Security (`src/service/windows/security.rs`)**:
   - `verify_client_session`: Impersonates named-pipe client, opens thread token, extracts `TokenSessionId`, and verifies caller matches `WTSGetActiveConsoleSessionId()`, immediately rejecting remote or cross-session callers (WIN-03).
3. **Motherboard Safety Profile Catalog (`src/service/windows/profiles.rs`)**:
   - Defined `BoardProfile` catalog supporting canonical HP OMEN and Victus boards:
     - `88F8` (OMEN 15/16 AMD Zen 3): 2 fans, Max Fan support, 4 RGB zones, quiet/balanced/performance profiles.
     - `8A24` (OMEN 16/17 Intel): 2 fans, Max Fan support, 4 RGB zones, quiet/balanced/performance profiles.
     - `8A3E` (Victus 15/16): 2 fans, Max Fan support, 1 RGB zone, quiet/balanced profiles.
     - `88D0` (OMEN 15 2020 Intel): 2 fans, Max Fan support, 4 RGB zones, default/performance/cool profiles.
   - `resolve_board_profile`: Normalized lookup; unrecognized boards return `None` to prevent dangerous out-of-spec writes.
4. **Safety Authorization (`src/service/windows/authorization.rs`)**:
   - `authorize_mutation`: Pre-execution validation blocking unadvertised fan modes, manual curves, unsupported power profiles, and out-of-bounds RGB payloads.
5. **HP BIOS WMI Driver (`src/service/windows/hp_wmi.rs`)**:
   - Driver connecting to `ROOT\WMI` / `hpqBIntM` (`5FB7F034-2C63-45E9-BE91-3D44E2C707E4`).
   - Implemented `query_hp_capabilities`, `query_fan_status`, `set_fan_mode` (with exponential backoff read-back verification), `set_performance_mode`, and `set_rgb_profile`.
6. **Server Dispatch & Client (`src/service/windows/server.rs` & `client.rs`)**:
   - `handle_request`: Dispatches requests through authorization and executes against HP WMI driver.
   - `WindowsServiceClient`: Unprivileged client connecting to named pipe, providing typed calls for fan mode, power profile, combined preset, and RGB.
7. **SCM Lifecycle & CLI Service Integration (`src/service/windows/lifecycle.rs` & `src/bin/zenomen-hw.rs`)**:
   - Implemented `register_service`, `uninstall_service`, and `run_service` managing `ZenOmenHw` service under SCM.
   - Wired `--install-service` and `--uninstall-service` CLI flags into `zenomen-hw.rs`.

## Verification & Test Results

1. **Compilation Matrix**:
   - `cargo check --target x86_64-pc-windows-msvc --bin zenomen-hw`: 0 errors, 0 warnings.
   - `cargo clippy --target x86_64-pc-windows-msvc --bin zenomen-hw -- -D warnings`: 0 warnings.
   - `cargo clippy --all-targets --all-features -- -D warnings`: 0 warnings.
2. **New Test Suite (`tests/windows_service_tests.rs`)**:
   - `test_named_pipe_protocol_roundtrip_all_requests`: Verified framing serialization and deserialization across all request types.
   - `test_protocol_rejects_payload_exceeding_64kb`: Verified that oversized payloads (>64 KB) and corrupt headers are rejected.
   - `test_board_profile_resolves_known_omen_boards`: Verified profile resolution for `88F8`, `8A24` (case-insensitive), and `8A3E`.
   - `test_board_profile_rejects_unknown_motherboard`: Verified unknown motherboard IDs return `None`.
   - `test_authorization_blocks_unsupported_fan_mode`: Verified auto/max authorization and typed error rejection for manual and unknown modes.
   - `test_mock_named_pipe_client_server_communication`: Verified complete client request framing, server dispatch, read-back verification, and client response decoding.
3. **Full Pipeline Verification**:
   - `cargo fmt --all -- --check`: Passed with 0 discrepancies.
   - `git diff --check`: Passed with 0 whitespace errors.
   - `cargo clippy --all-targets --all-features -- -D warnings`: Passed with 0 warnings.
   - `xvfb-run -a cargo test --all`: All 140 automated tests passed across 18 test suites.
