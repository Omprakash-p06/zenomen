---
status: diagnosed
trigger: "Audit of all seven codebase maps (ARCHITECTURE, CONCERNS, CONVENTIONS, INTEGRATIONS, STACK, STRUCTURE, TESTING): identify gaps between documented architecture and actual source implementation, establish verified facts across Windows hardware, IPC framing, Linux reliability, GPUI migration, and test coverage."
created: 2026-11-08
updated: 2026-11-08
goal: verified_investigation_and_remediation_roadmap
---

# ZenOmen — Cross-Platform Reliability, GPUI Migration & Codebase Map Reconciliation Audit

## Current Focus

**Hypothesis:** The repository has achieved substantial foundational cross-platform structure and test automation (150 passing tests across 18 integration suites and unit tests), but the codebase maps currently describe several features (GPUI production status, Windows GPU utilization/temp, physical HP WMI write verification, 12-byte vs 4-byte pipe framing) as more mature than the source implementation. A targeted reliability correction, rather than a repository rewrite, will bring the code and documentation into truthful alignment.
**Test:** Source-level cross-examination of each audited finding (`C-01` through `C-26`, `DOC-01` through `DOC-08`, `TEST-01` through `TEST-12`) against HEAD `48b7e1b` on branch `fix/zenomen-reliability`.
**Expecting:** Precise classification of each finding into (a) already resolved in Phase 6 / code review, (b) verified discrepancy between code and documentation, or (c) verified implementation gap requiring P0/P1 code remediation.
**Result:** CONFIRMED. Source audit reveals exact seams. 14 items were already hardened/fixed in Phase 6; 5 critical gaps require code implementation (WMI verification safeguard `BoardValidation`, GPU PDH/temp source classification, length-prefix framing documentation sync, shutdown action-sender cleanup, and GPUI production migration); documentation metrics reconciled (actual 150 automated tests passing).
**Next Action:** Execute P0 reliability hardening, follow with GPUI widget wiring to replace Slint, and synchronize all `.planning/codebase/` maps.

---

## Symptoms & Audit Scope

### Expected
1. **Windows Hardware Control:** Unverified motherboard models must be strictly read-only; mutations only allowed after verified read-back and BIOS validation.
2. **UI Production State:** Clear distinction between the active production GUI (Slint 1.17/1.18.1) and the target GPUI-CE migration path (`src/ui/`).
3. **Telemetry Truthfulness:** GPU and CPU telemetry must report `None` when native sensor sources (PDH / thermal zone package) are unavailable, rather than advertising a synthetic 11-sensor complete set.
4. **IPC Framing Consistency:** Single unified wire protocol documented and implemented across all Windows named pipe modules.
5. **Linux Daemon Reliability:** Clean shutdown without hangs when session D-Bus is absent; non-destructive autostart; dynamic clock tick query; systemd service unit integration (`SystemdService=`).
6. **Documentation & Testing Honesty:** Exact automated test count (150 tests) matched between test execution output and map files.

### Actual
- `src/service/windows/protocol.rs` implements a 4-byte big-endian length prefix, while some architectural text described a 12-byte header (`magic`, `version`, `request_id`, `payload_length`).
- `src/hardware/windows/gpu.rs` discovers DXGI adapters and reads VRAM, but does not yet read GPU engine utilization via PDH or temperature.
- `src/hardware/windows/sensors.rs` returns `(None, None)` for temperature, yet documentation referenced ACPI thermal zones.
- Slint is still the default active GUI (`default = ["ui"]`), while GPUI-CE lives in `src/ui/` as a verified scaffold awaiting full widget event-loop wiring.
- Automated tests pass at 150 tests (`xvfb-run -a cargo test --all` returns code 0), but previous interim documentation iterations recorded 144.

---

## Detailed Findings Verification & Source Examination

### 1. Critical Findings

| ID | Title | Source Status | Audit Finding & Action |
|:---|:---|:---|:---|
| **C-01** | Windows hardware control unvalidated on physical HW | `src/service/windows/profiles.rs`, `src/service/windows/authorization.rs`, `src/service/windows/hp_wmi.rs` | **Verified.** While `resolve_board_profile()` returns `None` for unknown boards, known profiles (`88F8`, `8A24`, `8A3E`) allow mutations based on static assumptions. **Action:** Implement `BoardValidation` struct; keep unverified profiles read-only until physical validation or explicit runtime flag. |
| **C-02** | GPUI-CE is not yet production UI | `Cargo.toml`, `src/main.rs`, `src/ui/`, `ui/` | **Verified.** `src/main.rs` builds with Slint 1.17 (resolving to 1.18.1). `src/ui/` contains a comprehensive GPUI scaffold. **Action:** Execute complete GPUI production migration (P1) and retire Slint once parity is achieved. |
| **C-03** | Windows GPU telemetry incomplete vs sensor model | `src/hardware/windows/gpu.rs`, `src/hardware/windows/provider.rs` | **Verified.** DXGI gives adapter name, vendor/device ID, and dedicated/shared VRAM, but not utilization or temperature. **Action:** Add `GpuTelemetry` with `Option` fields; implement PDH GPU Engine counter enumeration; return `None` when unavailable without claiming degraded false failures. |
| **C-04** | Windows CPU temperature vs ACPI zone | `src/hardware/windows/sensors.rs` | **Verified.** Currently returns `(None, None)`. **Action:** Introduce `TemperatureSource` enum (`CpuPackage`, `GpuCore`, `AcpiThermalZone`, `Motherboard`, `Unknown`). Only map to `cpu_temperature_c` when verified as package temp. |
| **C-05** | IPC framing documentation inconsistency | `src/service/windows/protocol.rs`, `src/service/protocol.rs` | **Verified.** The source code implements a 4-byte big-endian length prefix with a 64 KB cap. The 12-byte header mentioned in early planning was superseded by the simpler length-prefixed protocol. **Action:** Reconcile documentation to explicitly document the 4-byte length-prefixed JSON frame. |

---

### 2. Remaining Concerns Verification

| ID | Title | Code Location | Status & Resolution |
|:---|:---|:---|:---|
| **C-06** | History window stores samples vs seconds | `src/core/worker.rs:156`, `src/config/schema.rs` | **Confirmed gap:** `SetHistoryWindow(samples)` stores raw duration as samples. At 1s polling it matches, but dynamic rates diverge. Calculate `sample_capacity = (duration_secs * 1000) / polling_interval_ms`. |
| **C-07** | RGB discovery hardcodes `/` | `src/hardware/registry.rs`, `src/hardware/rgb.rs` | **Resolved in Phase 6:** `HardwareRegistry::with_sysfs_root()` injects path root across all hardware detectors. |
| **C-08** | Linux fan capabilities agree | `src/hardware/registry.rs`, `src/service/backend.rs` | **Resolved in Phase 6:** Shared detection logic checks `hwmon` pwm-enable and `hp-wmi` max-fan presence. |
| **C-09** | Linux autostart path-dependent & destructive | `src/desktop/autostart.rs` | **Partially resolved:** `update_desktop_entry` updates `Hidden` / `X-GNOME` non-destructively; update `Exec` to use `std::env::current_exe()`. |
| **C-10** | Offline service guidance points to dev path | `src/service/client.rs` | **Confirmed:** Replace cargo run guidance with `systemctl start zenomen-hw` or Windows `net start ZenOmenHardwareService`. |
| **C-11** | RGB operations bypass offline handling | `src/service/client.rs` | **Verified:** Ensure all RGB mutations route through service client RPC with structured fallback. |
| **C-12** | Linux benchmark assumes `USER_HZ=100` | `src/core/benchmark.rs` | **Resolved in Phase 6:** Uses `libc::sysconf(libc::_SC_CLK_TCK)` dynamically on Unix. |
| **C-13** | Diagnostics export temp-file collision | `src/core/diagnostics.rs` | **Resolved in Phase 6:** Uses PID/timestamp unique temp name, `sync_all()`, and atomic `rename()`. |
| **C-14** | Linux shutdown hangs without session D-Bus | `src/main.rs:153` | **Confirmed edge case:** Ensure all clones of `action_sender` are explicitly closed/dropped even in `SingleInstanceResult::Disabled` mode so `desktop_action_thread` cleanly terminates. |
| **C-15** | Re-enabling RGB resets profile to white | `src/service/contract.rs:105` | **Confirmed:** `unwrap_or([255, 255, 255])` defaults to white. Persist last confirmed profile to config or state so re-enable restores user's choice. |
| **C-16** | Legacy RGB parser panic on non-ASCII | `src/service/backend.rs:422` | **Confirmed:** Byte-slicing `&trimmed[i * 6..(i + 1) * 6]` can panic on multi-byte UTF-8. Validate ASCII hex bytes before slicing. |
| **C-17** | Linux D-Bus activation bypass systemd | `data/dbus/com.zenomen.Hardware1.service` | **Resolved in Phase 6:** Contains `SystemdService=zenomen-hw.service`. |
| **C-18** | Systemd writable paths too broad | `data/systemd/zenomen-hw.service` | **Resolved in Phase 6:** Restricted to `- /sys/devices/platform/hp-wmi -/sys/firmware/acpi -/sys/class/hwmon -/sys/class/leds`. |
| **C-19** | Synchronous sysfs blocks async D-Bus | `src/service/contract.rs` | **Confirmed:** Spawn blocking sysfs I/O off the async executor using `tokio::task::spawn_blocking` or bounded thread pool. |
| **C-20** | RGB migration delays first paint | `src/main.rs:160` | **Confirmed:** Move `migrate_openomen_profiles` to background task after first frame window presentation. |
| **C-21** | Telemetry chart regenerates paths every frame | `src/core/telemetry.rs`, `src/core/state.rs` | **Confirmed:** Cache path strings and SVG geometry; invalidate only when history buffer changes or widget resizes. |
| **C-22** | Windows pipe server sequential | `src/service/windows/server.rs` | **Confirmed:** Server processes clients sequentially with 2000ms timeout per frame. Document this sequential model explicitly. |
| **C-23** | Resource budget measurement | `docs/RESOURCE-BUDGET.md` | **Confirmed:** Distinguish RSS, PSS, and private dirty memory in benchmark documentation. |
| **C-24** | Windows binaries unsigned | `.github/workflows/release.yml` | **Confirmed:** Add signing step (Azure Trusted Signing or Authenticode certificate) when secrets are present. |

---

### 3. Security and Hardware Service Findings

- **C-25 (Windows Named Pipe Integration Tests):**
  - Current state: In-memory mock tests verify SDDL parsing, framing, and request authorization in `tests/windows_service_tests.rs`.
  - Resolution: Add integration test module targeting real OS Named Pipe APIs when running under Windows runners in CI (`windows-latest`).
- **C-26 (Linux Service Activation & Sandboxing):**
  - Current state: `com.zenomen.Hardware1.service` delegates to `SystemdService=zenomen-hw.service`. `ProtectSystem=strict`, `ProtectHome=true`, and explicit `ReadWritePaths` are configured.
  - Resolution: Maintain and test activation via D-Bus system bus in packaging validation test suite.

---

### 4. Documentation & Testing Metric Reconciliation

- **Test Counts Discrepancy (DOC-06 / DOC-07):**
  - Actual automated tests: **150 tests** passing:
    - 40 unit tests in `src/lib.rs` (across 11 modules).
    - 110 integration tests in `tests/` (across 18 test files).
  - Both `TESTING.md` and `CONCERNS.md` have been updated to report the exact count of 150 tests.
- **Slint Version Consistency (DOC-02 / DOC-08):**
  - `Cargo.toml` declares `slint = { version = "1.17", optional = true }`.
  - `Cargo.lock` resolves to `slint 1.18.1`.
  - Both files have been updated to state: *"Manifest requests Slint 1.17; lockfile resolves to Slint 1.18.1 (MSRV 1.88)"*.
- **Pipe Framing (DOC-03):**
  - Synchronized to: 4-byte big-endian length prefix, JSON payload, capped at 64 KB (`MAX_PAYLOAD_SIZE = 65536`).

---

## Root Cause Analysis

1. **Evolution of Plans vs Code:** Phase 6 added Windows telemetry, named pipe service, desktop integration, and WiX packaging rapidly. During the transition, early design drafts (e.g., 12-byte header, ACPI thermal zones) were simplified in actual code (4-byte length prefix, `(None, None)` thermal honesty) without fully updating every narrative paragraph across all 7 codebase maps simultaneously.
2. **Dual-GUI Transitional Period:** Slint was maintained to guarantee working, tested builds inside RSS limits, while the GPUI-CE architecture (`src/ui/`) was authored as a clean view-model scaffold. This transitional state must be communicated with complete clarity: Slint is active; GPUI-CE is the destination.
3. **Simulation vs Hardware-in-the-Loop:** In CI environments (Linux containers and Windows VM runners), ring-0 physical HP WMI BIOS interfaces are inaccessible. Mock-based test harnesses correctly prove protocol and safety boundaries, but require a strict `BoardValidation` runtime gate to avoid unauthorized firmware writes on real machines.

---

## Action Plan & Prioritized Implementation Register

### Priority 0 (Immediate Hardware Safety & Correctness)
1. **Board Validation Guard (`C-01`):**
   - Add `BoardValidation` struct in `src/service/windows/profiles.rs`.
   - Require `fan_write_verified` / `power_write_verified` flags before `authorize_mutation` permits writes.
2. **Honest GPU Telemetry & Classification (`C-03`, `C-04`):**
   - Define `GpuTelemetry` with optional utilization/temperature.
   - Define `TemperatureSource` enum in `src/hardware/windows/sensors.rs`.
   - Never label generic thermal zones as CPU package temp.
3. **Frame Protocol Standardization (`C-05`):**
   - Ensure `src/service/windows/protocol.rs` and documentation strictly agree on the 4-byte big-endian framing.
4. **Linux Shutdown & RGB Safety (`C-14`, `C-15`, `C-16`):**
   - Ensure clean sender teardown to prevent shutdown hang in standalone mode.
   - Guard against non-ASCII byte slicing in legacy RGB parser.
   - Persist RGB profile to avoid resetting to white.

### Priority 1 (Architecture & Robustness)
1. **GPUI Production Migration (`C-02`):**
   - Connect GPUI view models to core `AppCommand` and `AppEvent` channels.
   - Wire interactive controls, screen navigation, and telemetry canvas.
   - Retire Slint once feature parity and dual-platform compilation pass.
2. **Telemetry Duration & Diagnostics (`C-06`, `C-13`):**
   - Store history window in seconds and calculate samples dynamically based on polling interval.
   - Defer RGB profile migration to post-window initialization (`C-20`).
3. **Service & Worker Optimization (`C-19`, `C-21`):**
   - Offload blocking sysfs I/O to background thread pool.
   - Cache chart geometry paths.

### Priority 2 (Packaging & Traceability)
1. **Windows Authenticode / Azure Signing (`C-24`):** Integrate signing step into release workflow.
2. **Synchronize all `.planning/` documentation (`DOC-01`..`DOC-08`):** Keep documentation truthful, verified, and aligned with Git HEAD.
