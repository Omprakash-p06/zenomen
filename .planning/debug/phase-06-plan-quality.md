---
status: diagnosed
trigger: "Phase 6 plans are underwhelming: barely any information and not properly specified. Plans must be exhaustive, source-verified, and aligned with all seven .planning/codebase documents plus the supplied Windows 10/11 research specification."
created: 2026-11-07
updated: 2026-11-07
goal: find_root_cause_only
---

# Phase 6 Plan Quality — Root Cause Investigation

## Current Focus

hypothesis: Phase 6 plans were authored from a high-level roadmap summary rather than extracted from the codebase maps and the supplied research spec. The generator (a) invented requirement IDs that do not exist in REQUIREMENTS.md, (b) wrote illustrative structs/trait methods instead of the actual contracts in src/, (c) compressed an 8-phase research programme into 5 dependency-chained plans, and (d) never ran a plan-check/verification gate.
test: Compare every plan claim against REQUIREMENTS.md, the actual Rust contracts, Cargo.toml, and the coverage matrix of the supplied research spec.
expecting: Multiple concrete mismatches: invented IDs, wrong struct shapes, missing features, wrong file/line anchors, contradictory scope decisions.
result: CONFIRMED — every pre-collected finding re-verified against source; 3 anchors were themselves wrong (E9, E18, E20); 8 additional causes found, 2 of them severity-critical (see Root Cause Report).
next_action: Replan Phase 6 after the user decides UI framework (Slint vs GPUI-CE), milestone placement (v1 amendment vs v2), and Windows hardware access strategy. Nothing in this session was remediated by design.

## Symptoms

**Expected:**
- Each plan specifies exact files, symbols, and verified line anchors; exact struct/trait shapes that match current source; explicit task ordering; per-task acceptance criteria; named tests; dependency rationale.
- Every requirement in the roadmap maps to an ID defined in REQUIREMENTS.md.
- Coverage includes Windows CPU/GPU temperature and fan RPM, diagnostics, app-data paths, notifications, installer/signing/uninstaller, Windows 10 + 11 CI matrix, soak testing, and documentation/ADR updates.
- Plans explicitly reconcile or supersede ADR-001 and the stale `.planning/WINDOWS.md` ledger.

**Actual:**
- Five plans, 171–244 lines each, mostly prose steps with illustrative code.
- Requirement IDs SEC-01/02/03, WIN-03/04/05, ARCH-01, TEST-01 cited but undefined in REQUIREMENTS.md.
- Plan 06-02 rewrites `TelemetrySnapshot` into a different shape than the real struct.
- Plan 06-03 implements `sample()` but the trait requires `sample_telemetry() -> Result<TelemetrySnapshot, String>`.
- Plan 06-01 adds `libc::sysconf` but `libc` is not a dependency and Cargo.toml is not in `files_modified`.
- No plan covers Windows CPU/GPU temperature, sensor provider fallback, diagnostics export, `%LOCALAPPDATA%`/`%PROGRAMDATA%`, notifications, MSI installer, code signing, or uninstaller.
- No plan updates ROADMAP traceability, REQUIREMENTS.md, ADRs, or `docs/architecture/*`.
- No plan-check, dependency-graph validation, or verification gate appears to have run.

**Error messages:** None (documentation artifacts).

**Timeline:** Plans created 2026-11-07 18:10 (`745ba8d`); "expanded with exhaustive technical specifications" 2026-11-07 18:16 (`c5c3ac0`).

**Reproduction:** Read `06-01`…`06-05-PLAN.md` and compare to `.planning/codebase/{STACK,INTEGRATIONS,ARCHITECTURE,STRUCTURE,CONVENTIONS,TESTING,CONCERNS}.md`, `.planning/REQUIREMENTS.md`, and the supplied Windows research specification.

## Evidence

- timestamp: 2026-11-07 — `REQUIREMENTS.md` defines only `WIN-01` and `WIN-02`, both under "v2 Requirements / Platform (deferred)". It defines no `WIN-03/04/05`, `SEC-01/02/03`, `ARCH-01`, or `TEST-01`, yet `.planning/ROADMAP.md:139` and all five plans cite them.
- timestamp: 2026-11-07 — `REQUIREMENTS.md` explicitly lists "Windows hardware control in v1" as **Out of Scope** and states the v2 `WIN-01`/`WIN-02` items are deferred. Phase 6 plans contradict this without amending REQUIREMENTS.md.
- timestamp: 2026-11-07 — `REQUIREMENTS.md` Traceability ends at Phase 5 and claims "Unmapped: 0 ✓"; there is no Phase 6 row despite ROADMAP listing Phase 6.
- timestamp: 2026-11-07 — Real `TelemetrySnapshot` (`src/hardware/provider.rs:152-181`) has `cpu_name: String`, `cpu_temperature_c: Option<f32>`, `cpu_frequency_mhz: u32`, `ram_total_mb: u64`, `ram_used_mb: u64`, `power_profiles: Vec<String>`, `rgb_profile: Option<String>`; it has no `timestamp`, `vram_used_mb`, `vram_total_mb`, or battery fields. Plan 06-02 Step 1 prints a different struct with `timestamp`, `vram_*`, `battery_*`, `cpu_temperature: Option<f32>`, `ram_total_mb: Option<u32>` and would break every consumer.
- timestamp: 2026-11-07 — Real `HardwareProvider` (`src/hardware/provider.rs:235-242`) exposes `capabilities()` and `sample_telemetry() -> Result<TelemetrySnapshot, String>`. Plans 06-03/06-04 describe `sample()` and a `HardwareBackendTrait` that does not exist.
- timestamp: 2026-11-07 — Real `HardwareService` (`src/service/contract.rs:14-18`) is `{ backend: HardwareBackend, conn: Arc<RwLock<Option<Connection>>>, require_polkit: bool }`. Plan 06-01 anchors it at `contract.rs:40-50` and proposes `backend: Arc<dyn HardwareBackendTrait>` plus a new field; the anchor is wrong and the shape is invented.
- timestamp: 2026-11-07 — `Cargo.toml` has no `libc` dependency; Plan 06-01 Step 5 adds `libc::_SC_CLK_TCK` without listing `Cargo.toml` in `files_modified`. It also has no `windows-sys`/`windows` dependency; Plan 06-03 adds `windows-sys 0.59` without pinning the lockfile or verifying feature names.
- timestamp: 2026-11-07 — `EXPECTED_SENSOR_COUNT = 11` already exists (`src/hardware/provider.rs:191`); Plan 06-02 Step 1.2 says to add it.
- timestamp: 2026-11-07 — `HardwareCapabilities` already carries `battery_metrics` (`src/hardware/registry.rs:148`), so Plan 06-02's "expand the contract" framing is stale.
- timestamp: 2026-11-07 — Source has 23 `.slint` files and 124 `#[test]` functions. `06-RESEARCH.md:16` claims 123 passing tests; `docs/decisions/ADR-001-ui-framework.md` says 20 `.slint` files; `.planning/debug/zenomen-modernization-audit.md` says 110 `#[test]`. Three different counts, none reconciled.
- timestamp: 2026-11-07 — `06-RESEARCH.md:16` claims "123 automated tests passing" but `cargo` is not installed in this environment and no run is recorded in this session; the claim is unverified source drift (AGENTS.md rule 9).
- timestamp: 2026-11-07 — Coverage gap: supplied research spec §3.1/§3.3/§3.4 requires Windows CPU temperature, GPU temperature, fan RPM and sensor-source fallback. Plan 06-03 has no temperature or fan-RPM deliverable and its `files_modified` has no `sensors.rs` or `device.rs`.
- timestamp: 2026-11-07 — Coverage gap: research spec §3.6 requires Windows diagnostics via `Win32_OperatingSystem`/`Win32_BIOS`/`Win32_VideoController`; no plan lists `src/core/diagnostics.rs` or a Windows diagnostics path.
- timestamp: 2026-11-07 — Coverage gap: research spec §4.3/§5.3 requires `SHGetKnownFolderPath`, `%LOCALAPPDATA%`/`%PROGRAMDATA%`, notifications, and Windows Event Log; Plan 06-05 creates only tray/autostart/single_instance/releases, with no `notifications.rs`, `paths.rs`, or `installer.rs`.
- timestamp: 2026-11-07 — Coverage gap: research spec §8 Phase 6 requires a signed x64 installer, service install/remove, icon/version resources, upgrade/rollback, and uninstaller. Plan 06-05 delivers only raw `.exe` artifacts plus SHA256SUMS; no MSI/WiX, signing, or rollback.
- timestamp: 2026-11-07 — Coverage gap: research spec §9 requires Windows 10 + Windows 11 matrices, UI runtime tests, installer tests, and a two-hour soak. Plan 06-05 CI runs only `windows-latest` and only `cargo test --target x86_64-pc-windows-msvc --lib`.
- timestamp: 2026-11-07 — Coverage gap: research spec §10 requires new ADRs and `docs/architecture/{CROSS_PLATFORM_ARCHITECTURE,WINDOWS_HARDWARE,HP_WMI_PROTOCOL,IPC_SECURITY}.md`, plus `docs/testing/*`. No plan creates them.
- timestamp: 2026-11-07 — Scope compression: the supplied spec defines 8 phase scopes (0–7) including a separate full-regression phase. ROADMAP folds them into 5 strictly serialized waves; no phase covers cross-platform verification.
- timestamp: 2026-11-07 — Contradiction: supplied spec's final decision is GPUI-CE for both OSes; `06-RESEARCH.md §5` overrides it with "Slint Retained" citing ADR-001, which is still **Status: Proposed** and its "Related" source is the prior audit. No plan records this supersession as a decision.
- timestamp: 2026-11-07 — `.planning/WINDOWS.md` ledger still has 2 open defects from Phase 1; no plan reconciles it (research spec §6.1 requires reconciliation).
- timestamp: 2026-11-07 — Plan 06-04 protocol enum uses `mode: String` / `profile_json: String`, contradicting the spec's typed-enum requirement, and omits protocol version, request ID, response ID, bounded size field, deadlines, and typed errors from the declared protocol.

## Eliminated

- hypothesis: "The plans are short because the codebase map is missing." — Eliminated: all seven `.planning/codebase/*.md` documents exist, are current (`last_mapped_commit: 4baf4bd`, 2026-11-07), and total >160 KB. The plans simply do not cite them.
- hypothesis: "Phase 6 plans were generated by `/gsd-plan-phase` with its normal verification loop." — Eliminated by artifact evidence: no `06-CONTEXT.md`, no `06-SPEC.md`, no plan-check report, and requirement IDs that do not exist. A gate that checked IDs against REQUIREMENTS.md would have failed.

## Next Action

Verify the remaining findings (Cargo feature names, module cfg-gating, service/zBus gating) and emit a Root Cause Report with prioritized causes. Do not edit the plans in this session; remediation is a replan.

---

## Evidence — 2026-11-07 re-verification pass

- timestamp: 2026-11-07 — **The research doc's primary source does not exist.** `06-RESEARCH.md:5` names its method as analysis of "the comprehensive cross-platform roadmap specification (`pasted-text-4.txt`)". No file named `pasted-text-4*` exists anywhere on the filesystem (`find / -name "pasted-text-4*"` → 0 hits). Phase 6's requirement set is therefore only reproducible from a chat transcript; no reviewer, agent, or gate can diff plans against it.
- timestamp: 2026-11-07 — `06-RESEARCH.md` is itself lossy relative to the spec it summarizes: §3.1 CPU = utilization/frequency/model, §3.2 RAM, §3.3 GPU = name/VRAM/utilization, §3.4 battery. **No temperature section exists anywhere in §3**, and §2.2 lists fan RPM only as a WMI command table row, never as a telemetry deliverable. The temperature/fan-RPM gap therefore originates upstream of the plans, not in them.
- timestamp: 2026-11-07 — ROADMAP.md:141 (Phase 6 Success Criterion 3) says Windows telemetry covers "CPU, RAM, GPU, battery, and supported sensors"; no temperature, fan RPM, or sensor-source fallback. Criterion 1 (ROADMAP.md:139) enumerates **6** CONCERNS items as "All open items in CONCERNS.md"; CONCERNS.md has **44** headings, 3 marked `[RESOLVED]`, i.e. ~41 open/noted items including 11 explicitly tagged "Remaining Future Work".
- timestamp: 2026-11-07 — `06-01-PLAN.md:94-100` would **regress shipped functionality**: it adds `ProtectKernelTunables=true` and narrows `ReadWritePaths` to `-hp-wmi -hwmon`. The live unit already has `ProtectSystem=strict` and `ProtectHome=true` (`data/systemd/zenomen-hw.service:15-16`), a wider `ReadWritePaths=-/sys/devices/platform/hp-wmi -/sys/firmware/acpi -/sys/class/hwmon -/sys/class/leds` (line 19), and an inline comment at line 21-22 stating ProtectKernelTunables "is omitted because it renders /sys read-only, conflicting with sysfs fan/power/RGB control carve-outs". Adopting the plan drops `/sys/firmware/acpi` (power profiles → PWR-03) and `/sys/class/leds` (RGB → RGB-04) and re-adds the exact hardening the unit's author rejected.
- timestamp: 2026-11-07 — **Plan 06-03's dependency declaration is invalid.** `06-03-PLAN.md:81-89` requests `windows-sys` feature `Win32_Graphics_Dxgi`. `windows-sys` 0.59.0 contains **zero** occurrences of `Dxgi` in its `Cargo.toml` (also 0 in 0.52.0 and 0.61.2), and no `IDXGIFactory1`, `IDXGIAdapter1`, or `CreateDXGIFactory1` symbol anywhere in `src/` (verified in `~/.cargo/registry/src/index.crates.io-*/windows-sys-0.59.0`). Cargo would fail at feature resolution before compiling, so the plan's own gate `cargo check --target x86_64-pc-windows-msvc` (06-03:231) cannot pass.
- timestamp: 2026-11-07 — **Plan 06-04's core deliverable is unimplementable with the declared dependency.** `windows-sys` 0.59.0 has no `IWbemLocator` symbol (0 grep hits in `src/`), so the HP BIOS WMI mailbox driver (`hpqBIntM.Execute`, 06-04:115-127) requires the COM-capable `windows` crate or hand-written raw COM. Plan 06-04's `files_modified` (06-04:18-26) does not list `Cargo.toml` at all, despite needing `Win32_System_Services`, `Win32_System_RemoteDesktop`, `Win32_Security_Authorization`, and `Win32_System_Wmi` features. Plan 06-05 (06-05:18-29) likewise omits `Cargo.toml` while needing `Win32_UI_Shell` (`Shell_NotifyIconW`, `ShellExecuteW`, `TrackPopupMenu`), `Win32_System_Registry` (`RegOpenKeyExW`), `Win32_System_Threading` (`CreateMutexA`), and `Win32_UI_WindowsAndMessaging`.
- timestamp: 2026-11-07 — Confirmed-present in windows-sys 0.59.0: `GetSystemTimes` (`src/Windows/Win32/System/Threading/mod.rs`), `PdhOpenQueryW` (`src/Windows/Win32/System/Performance/mod.rs`), `Win32_System_Power`, `Win32_System_SystemInformation`, `Win32_System_Services`, `Win32_System_RemoteDesktop`, `Win32_UI_Shell`, `Win32_System_Wmi`, `Win32_System_Registry`. So plan 06-03's CPU/RAM/battery half is feasible; its GPU half and all of 06-04's WMI layer are not.
- timestamp: 2026-11-07 — `Cargo.lock` already resolves `windows-sys` at **0.52.0 and 0.61.2** transitively (lock lines 119, 204, 262, 800, 952, 1397). Adding a pinned `0.59` creates a third major-version copy, contrary to AGENTS.md rule 4 and the ≤40 MB RSS budget's spirit.
- timestamp: 2026-11-07 — `06-02-PLAN.md:101` instructs "Add `pub fn has_battery_metrics(&self) -> bool`". It already exists at `src/hardware/provider.rs:122-124`. Same plan's `:102` instructs keeping `EXPECTED_SENSOR_COUNT` at 11 — already the value at `src/hardware/provider.rs:191`.
- timestamp: 2026-11-07 — `TelemetrySnapshot` ground truth: `src/hardware/provider.rs:152-184`, **14** fields (`cpu_name`, `cpu_utilization`, `cpu_temperature_c`, `cpu_frequency_mhz`, `gpu_name`, `gpu_utilization`, `gpu_temperature_c`, `ram_total_mb`, `ram_used_mb`, `fan_rpms`, `fan_mode`, `power_profile`, `power_profiles`, `rgb_profile`) with `#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]`. Note the pre-collected E4 enumerated only 7 of 14 fields and omitted `cpu_utilization`, `gpu_name`, `gpu_utilization`, `gpu_temperature_c`, `fan_rpms`, `fan_mode`, `power_profile`.
- timestamp: 2026-11-07 — `06-02-PLAN.md:78-98` replaces that type with 16 differently-named/retyped fields: `cpu_name`→`cpu_model: Option<String>`, `cpu_frequency_mhz: u32`→`cpu_frequency: Option<f32>` (unit change MHz→ambiguous), `ram_total_mb: u64`→`Option<u32>`, drops `power_profiles` and `rgb_profile`, adds `timestamp`/`vram_used_mb`/`vram_total_mb`, and drops the `PartialEq` and `Default` derives. Verified consumers that break: `src/hardware/mock.rs:162` (`TelemetrySnapshot::default()`), `src/app.rs:449-450` (`snapshot.gpu_utilization`, `.gpu_temperature_c`), and every `LinuxHardwareProvider`/`UnavailableHardwareProvider` construction site.
- timestamp: 2026-11-07 — `06-01-PLAN.md:106` anchors `HardwareService` at `src/service/contract.rs:40-50`; those lines are `with_polkit_required`/`backend()`. The struct is at `contract.rs:14-18` with exactly `backend: HardwareBackend`, `conn: Arc<RwLock<Option<Connection>>>`, `require_polkit: bool`. The plan's replacement (`Arc<dyn HardwareBackendTrait>`, `polkit: PolkitVerifier`, `last_active_rgb_profile`) invents a trait and discards the connection handle polkit actually needs.
- timestamp: 2026-11-07 — `06-02-PLAN.md:138-165` invents `TelemetryHistory { capacity, duration_seconds, samples, cached_svg_paths }` plus `new_with_duration`/`update_interval`/`set_duration`. Real type (`src/core/telemetry.rs:93-96`) is `{ points: VecDeque<TelemetryPoint>, capacity: usize }` with `new()`/`with_capacity()`; `set_duration` is called at 06-02:163 but never defined in the plan. The command is `AppCommand::SetHistoryWindow(usize)` meaning **samples** (`src/core/events.rs:77`), while the plan reinterprets it as seconds without changing the variant or its call sites (`src/core/worker.rs:156-158`).
- timestamp: 2026-11-07 — Requirement-ID mismatch, verified by exhaustive diff: REQUIREMENTS.md defines 71 v1 IDs (`FOUND/MON/COOL/PWR/RGB/SVC/SYS/DIAG/REL/PERF`) plus 9 v2 IDs. **`SEC-01`, `SEC-02`, `SEC-03`, `WIN-03`, `WIN-04`, `WIN-05`, `ARCH-01`, `TEST-01` exist nowhere** in it, yet ROADMAP.md:137 and 06-01:9-11 (SEC-*, `requirements`), 06-02:14-15 (ARCH-01, TEST-01), 06-04:16-17 (WIN-03/04), 06-05:17 (WIN-05) all cite them.
- timestamp: 2026-11-07 — Requirement misuse, verified: 06-02 cites `MON-10` for history-window work, but history window is **MON-11** (`REQUIREMENTS.md:30`) and MON-10 is manual refresh (`:29`). 06-02 also claims `MON-02` (per-core CPU utilization, `:21`) but no step delivers per-core. 06-03 claims `MON-01`–`MON-08` while delivering no temperature (MON-04, MON-05), no per-core (MON-02), and no available-RAM (MON-07 says "total, available and used").
- timestamp: 2026-11-07 — Platform-pinned requirement text silently reinterpreted: SYS-02 requires "via XDG autostart" (`REQUIREMENTS.md:74`) but 06-05:120-125 implements HKCU Run; SYS-05 requires "via D-Bus activation (`com.zenomen.App`)" (`:77`) but 06-05:96-114 implements `CreateMutexW`; SVC-02 requires "All privileged hardware writes go through the `com.zenomen.Hardware1` D-Bus system service" (`:65`) but 06-04 builds a named pipe — and 06-04 cites only `SVC-01`, avoiding the conflict rather than amending it.
- timestamp: 2026-11-07 — Count drift is worse than "three counts": README claims 108 tests (per `zenomen-modernization-audit.md:57`), the audit claims 110 `#[test]` attributes and 123 passing, `06-RESEARCH.md:16` claims 123 passing, and the tree actually contains **124** `#[test]` functions. `.slint` files: ADR-001:11 and `06-RESEARCH.md:141` say **20**; the tree has **23**. `06-RESEARCH.md:142` says "Slint 1.18"; `Cargo.toml:17` pins **1.17**. `cargo` is not installed (`which cargo` → not found), so no count is runnable here.
- timestamp: 2026-11-07 — Traceability is stale project-wide, which is why Phase 6's absence went unnoticed: ROADMAP.md Progress marks Phases 1–5 **Complete**, while REQUIREMENTS.md:163-233 still marks almost every v1 row **Pending** (e.g. `MON-01`…`MON-08`, `REL-01`…`REL-08`, `PERF-01`). The table stops at `PERF-06 | Phase 5` (`:233`) with no Phase 6 rows, then asserts "v1 requirements: 71 / Mapped: 71 / Unmapped: 0 ✓" (`:236-238`). Phase 6 cites two **v2** IDs (WIN-01/02), which are outside that 71, so the zero-unmapped claim is false by construction.
- timestamp: 2026-11-07 — No gate artifact exists despite gates being enabled: `.planning/config.json` sets `workflow.research=true`, `plan_check=true`, `verifier=true`, `post_planning_gaps=true`, yet `.planning/phases/06-…/` contains only 5 plans + `06-RESEARCH.md` — no `06-CONTEXT.md`, no `06-SPEC.md`, no plan-check report, no `VERIFICATION.md`. `granularity: "coarse"` and `mode: "yolo"` are set; `plan_bounce: false`.
- timestamp: 2026-11-07 — No cross-platform seam exists in source: `src/hardware/mod.rs:15-25` uses a single `#[cfg(not(target_os = "linux"))] → UnavailableHardwareProvider` branch, and the whole tree has only **4** `cfg(target_os = "linux")` sites (`src/main.rs:27,195,218`, `src/hardware/mod.rs:16`) and **zero** `cfg(windows)`. `zbus` is an unconditional dependency (`Cargo.toml:29`) and `src/service/` and `src/desktop/` contain no cfg gates at all. No plan schedules a `PlatformService`/`PlatformDesktop` trait as a first-class artifact, though 06-03/04/05 depend on one.
- timestamp: 2026-11-07 — `.planning/WINDOWS.md` is the GSD **Broken Windows defect register** (`open_count: 2`, ids 1–2, both phase 01), not a Windows-platform document — a naming trap for anyone reading the trigger. CONCERNS.md:99-102 already describes it as a *false positive*: "Stale planning ledger entry … reports an open defect that is already resolved, so `/gsd-ship` can block on a false positive." No Phase 6 plan lists it, and `workflow.windows_enforce` would block ship while `open_count > 0`.
- timestamp: 2026-11-07 — `06-RESEARCH.md:52` corrupts its own command catalogue: the RGB row's "Command ID" column contains `0x20009`, which is a command *category*, not a command ID. The same error is copied into `06-04-PLAN.md:125` ("`0x20009`: Apply 4-zone RGB"). A downstream implementer cannot build the mailbox payload from this.

## Verification Matrix

| # | Pre-collected finding | Verdict | Proof |
|---|---|---|---|
| E1 | Missing requirement IDs cited by ROADMAP + plans | **TRUE** | REQUIREMENTS.md:12-139 defines only FOUND/MON/COOL/PWR/RGB/SVC/SYS/DIAG/REL/PERF/UPD/WIN-01-02/COMP; no SEC/ARCH/TEST/WIN-03-05. Anchor fix: ROADMAP.md:**137**, not 139 |
| E2 | Windows control listed Out of Scope in v1, contradicts Phase 6 | **TRUE** | REQUIREMENTS.md:153 vs ROADMAP.md:132-143 |
| E3 | Traceability ends at Phase 5, claims Unmapped 0 | **TRUE** | REQUIREMENTS.md:233-238 |
| E4 | 06-02 rewrites `TelemetrySnapshot` wrongly | **TRUE** (finding's own field list incomplete — 7 of 14) | provider.rs:152-184 vs 06-02:78-98 |
| E5 | Plans use `sample()` / `HardwareBackendTrait`, real is `sample_telemetry()` | **TRUE** | provider.rs:235-242; 06-02:118, 06-03:222, 06-01:109 |
| E6 | `HardwareService` anchor + shape invented | **TRUE** | contract.rs:14-18 vs 06-01:106-113 |
| E7 | `libc` absent, `Cargo.toml` not in `files_modified` | **TRUE** | Cargo.toml:24-39; 06-01:17-30, 165-181 |
| E8 | `EXPECTED_SENSOR_COUNT = 11` already exists | **TRUE** | provider.rs:191 (assoc const) |
| E9 | `HardwareCapabilities.battery_metrics` already exists at `registry.rs:148` | **PARTIAL — fact TRUE, anchor WRONG** | Declared provider.rs:**46**; registry.rs:149 only constructs it |
| E10 | Three unreconciled test/`.slint` counts | **TRUE, understated** | 4 test counts (108/110/123/124), 2 `.slint` counts (20/23) |
| E11 | "123 tests passing" unverified, cargo absent | **TRUE** | 06-RESEARCH.md:16; `which cargo` → none |
| E12 | No Windows CPU/GPU temperature, fan RPM, fallback | **TRUE — and roots in 06-RESEARCH.md §3, which has no temperature section** | 06-RESEARCH.md:65-99; 06-03:20-30 |
| E13 | No Windows diagnostics path | **TRUE** | no plan names diagnostics; only `tests/diagnostics_tests.rs` exists (Linux) |
| E14 | No paths/notifications/Event Log modules | **TRUE** | 06-05:18-29 |
| E15 | No MSI/WiX, signing, rollback, uninstaller | **PARTIAL** | 06-05:182-186 = raw exe+SHA256; but 06-04:149-150 does add service `--install-service/--uninstall-service`. No app installer/uninstaller, no signing, no rollback |
| E16 | CI matrix is `windows-latest` + `--lib` only | **TRUE** | 06-05:160,175-179; `.github/workflows/release.yml:22` is ubuntu-only today |
| E17 | No ADRs, no `docs/architecture/*`, no `docs/testing/*` | **TRUE** | `docs/` = decisions/, PACKAGING.md, PARITY.md, RESOURCE-BUDGET.md only |
| E18 | 8-phase spec compressed into 5 serial waves, no verification phase | **PARTIAL — the 5 serialized waves are verified; "8 phase scopes" is UNVERIFIABLE** | ROADMAP.md:147-157 (each wave "blocked on" the previous); config `granularity: coarse` |
| E19 | Spec's final decision = GPUI-CE, overridden by research via Proposed ADR-001 | **PARTIAL** | ADR-001:3 `Status: Proposed`, :6 Related = audit; 06-RESEARCH:140-144. The spec's *final* decision is UNVERIFIABLE (source artifact absent). No plan records the supersession — TRUE |
| E20 | `.planning/WINDOWS.md` 2 open defects, unreconciled | **PARTIAL — counts TRUE, framing wrong** | open_count 2, ids 1-2 phase 01. It is the Broken-Windows *defect* register, not a Windows-platform doc; CONCERNS.md:99-102 calls both entries false positives |
| E21 | 06-04 protocol stringly-typed, missing version/id/bounds/deadlines/typed errors | **PARTIAL** | 06-04:89-111 — no correlation ID, no version field, `mode: String`/`profile_json: String`, `Error{code:String}`. Bounds (64 KB, :71) and timeouts (30 s, :72) *are* specified |

Unverifiable and why: E18's "8 phase scopes", E19's "spec's final decision", and every "spec requires §X" citation — the spec artifact is absent from the repository (see R1).

## Root Cause Report

### Primary causes (ranked by leverage)

**R1 — The Phase 6 requirement source was never persisted (highest leverage).**
`06-RESEARCH.md:5` names `pasted-text-4.txt` as the authority. No such file exists anywhere on disk. Mechanism: the spec existed only in the planning conversation, so (a) plan authors had to rely on a lossy prose summary, (b) no gate or reviewer could diff coverage against it, and (c) the deliverable list decayed at every hand-off. This single fact explains most downstream symptoms: it is impossible to satisfy "exhaustive, aligned with the supplied specification" when the specification is not an addressable artifact. Blast radius: the entire phase; unverifiable acceptance forever.

**R2 — Two lossy summarization hops were treated as authoritative (the mechanism by which R1 became specific defects).**
Chain: chat spec → `06-RESEARCH.md` (already has **no** temperature section, no fan-RPM deliverable, no installer/signing/Win10-matrix/soak work) → `ROADMAP.md:139-143` five success criteria → plans. Each hop dropped deliverables and none was checked against the previous hop. Proof: ROADMAP Criterion 1 says "**All** open items in CONCERNS.md" then enumerates 6 while CONCERNS.md has ~41 open/noted entries; Criterion 3 lists "CPU, RAM, GPU, battery" with no temperature or fan RPM; research §3 has no temperature anywhere. Plans cannot cover what the roadmap never carried. This is the direct cause of E12–E17.

**R3 — Domain contracts were authored from memory, not read from source.**
Every one of the five plans invents or re-derives types that already exist: `TelemetrySnapshot` retyped (06-02:78-98 vs provider.rs:152-184), `HardwareService` re-shaped (06-01:106 vs contract.rs:14-18), `TelemetryHistory` re-shaped (06-02:138-165 vs telemetry.rs:93-96), `HardwareProvider::sample()` invented (provider.rs:241), `HardwareBackendTrait` invented, `has_battery_metrics` and `EXPECTED_SENSOR_COUNT` re-added although they exist (provider.rs:122, :191), wrong file anchor for `battery_metrics` (E9). Mechanism: `config.plan_review.source_grounding: true` / `source_grounding_authority: "grep"` was configured but its output is absent — no plan carries a single verified `path:line` for the symbol it proposes to change. Blast radius: compiling but wrong plans; `TelemetrySnapshot::default()` (mock.rs:162), `app.rs:449-450` and both providers break on adoption.

**R4 — The declared dependency cannot implement the declared APIs (hard build failure).**
`06-03:81` requests `windows-sys` feature `Win32_Graphics_Dxgi`, which does not exist in 0.59.0 (0 occurrences; also absent in 0.52.0/0.61.2) — cargo fails at resolution. `windows-sys` 0.59.0 exposes no DXGI symbols at all and no `IWbemLocator`, so 06-03's GPU enumeration and all of 06-04's HP WMI mailbox are unimplementable as specified. 06-04 and 06-05 additionally need Win32 features yet omit `Cargo.toml` from `files_modified`, and `Cargo.lock` already carries 0.52.0 + 0.61.2, so 0.59 would add a third copy. Mechanism: dependency feasibility was assumed, never resolved — no `cargo check --target x86_64-pc-windows-msvc` was ever run (cargo not installed in the planning environment either). Blast radius: waves 3–5 cannot build as written.

**R5 — A plan that claims to harden would regress two shipped features.**
`06-01:94-100` adds `ProtectKernelTunables=true` and narrows write paths to `hp-wmi`+`hwmon`. The live unit (`data/systemd/zenomen-hw.service:19-22`) is wider on purpose and documents that ProtectKernelTunables makes `/sys` read-only and breaks fan/power/RGB writes. Adoption silently kills power-profile applies (PWR-03) and RGB writes (RGB-04) — the plan contradicts a design comment that was written for exactly this reason. Mechanism: CONCERNS items were remediated from the ROADMAP summary (R2) instead of from the current unit file. Blast radius: two product features; also duplicates `ProtectSystem`/`ProtectHome` already present at :15-16.

**R6 — No review gate actually executed, so IDs, coverage, and feasibility were never checked.**
`config.json` enables `research`, `plan_check`, `verifier`, `post_planning_gaps`, yet the phase directory holds no `06-CONTEXT.md`, `06-SPEC.md`, plan-check report, or `VERIFICATION.md`. Decisive proof that the ID-consistency check did not run: eight cited IDs (`SEC-01..03`, `WIN-03..05`, `ARCH-01`, `TEST-01`) exist nowhere in REQUIREMENTS.md, and the ROADMAP itself was never validated against it (ROADMAP.md:137). Contributing configuration: `granularity: "coarse"` (5 waves from 8 spec scopes), `plan_bounce: false`, `mode: "yolo"` (no human checkpoint). Mechanism: gates are advisory in practice whenever their artifacts are optional; a passing gate leaves a trace, an absent trace means no gate.

### Contributing factors

- **C1 — Stale traceability made Phase 6 avoiding notice cheap.** Phases 1–5 are "Complete" in ROADMAP Progress while REQUIREMENTS.md:163-233 marks nearly all their rows "Pending"; the table has no Phase 6 section yet still asserts "Unmapped: 0 ✓". A table this wrong cannot flag a new phase's missing rows.
- **C2 — Scope contradiction never amended.** REQUIREMENTS.md:153 puts "Windows hardware control in v1" Out of Scope and :113 defers WIN-01/02 to a non-roadmapped v2; Phase 6 is a v1 phase delivering exactly that. No milestone amendment or v2 promotion was written.
- **C3 — Linux-pinned requirement text reinterpreted silently.** SYS-02 (XDG autostart), SYS-05 (D-Bus activation), SVC-02 (all writes via D-Bus) are contradicted by 06-05/06-04, and 06-04 simply omits SVC-02 from its `requirements:` list. Requirements were treated as labels, not contracts.
- **C4 — No cross-platform seam is scheduled.** `src/service/` and `src/desktop/` have zero cfg gates, `zbus` is unconditional, and `src/hardware/mod.rs:15-25` has a two-way platform branch. Plans 06-03..05 all presuppose a `PlatformService`/`PlatformDesktop` abstraction that no plan creates.
- **C5 — Numerical drift is systemic and unpoliced.** 108/110/123/124 tests; 20/23 `.slint` files; Slint 1.18 claimed vs 1.17 pinned. Every planning document quotes a stale count, so "verified" claims in plans are indistinguishable from recalled ones.
- **C6 — The UI-framework decision is still Proposed.** ADR-001:3 `Status: Proposed`, dated the same day, `Related:` = the prior modernization audit; `06-RESEARCH:140-144` nevertheless treats it as settled and refuses GPUI-CE. A plan may not cite an unapproved ADR as authority, and no plan records the supersession of the spec's UI direction.
- **C7 — Vocabulary trap: `.planning/WINDOWS.md` is a defect register, not Windows-platform notes.** Anyone reconciling "the Windows ledger" will look for platform documentation that does not exist; conversely the 2 real open entries (one a known false positive per CONCERNS.md:99-102) will block `/gsd-ship` via `workflow.windows_enforce`.
- **C8 — The research doc's own WMI catalogue is wrong.** `06-RESEARCH.md:52` puts category `0x20009` in the command-ID column; `06-04:125` copies the error. Even a plan written to the research doc would mis-frame the mailbox payload.

### Why the eliminated hypotheses remain eliminated

- "Codebase map missing" — re-confirmed: 7 documents, 2304 lines, `last_mapped_commit` current. Plans cite none of them by anchor.
- "Normal `/gsd-plan-phase` verification ran" — re-confirmed: no CONTEXT/SPEC/plan-check/VERIFICATION artifacts, and nonexistent requirement IDs survived, which an ID check would have rejected (now strengthened by R6).

## Resolution

root_cause: Phase 6 was planned against a **non-persisted, twice-summarized requirement source**. The authoritative spec (`pasted-text-4.txt`, per `06-RESEARCH.md:5`) exists nowhere on disk, so coverage could never be verified; `06-RESEARCH.md` and then `ROADMAP.md:139-143` each dropped deliverables (temperature, fan RPM, diagnostics, installer/signing/uninstaller, Win10+11 matrix, soak, ADRs) before any plan was written. Plans were then authored from memory rather than from source (invented `TelemetrySnapshot`/`HardwareService`/`TelemetryHistory` shapes, `sample()`, `HardwareBackendTrait`, re-added existing consts, wrong anchors), assigned eight requirement IDs that do not exist, and never subjected to the enabled plan-check/verifier gates. Two hard blockers sit on top: `windows-sys` 0.59 provides no DXGI and no `IWbemLocator` (and `Win32_Graphics_Dxgi` is not a valid feature, so resolution fails), and `06-01` would re-add `ProtectKernelTunables=true` and drop `/sys/firmware/acpi` + `/sys/class/leds`, regressing power-profile and RGB writes that the live unit deliberately protects.
fix: not applied (goal = find_root_cause_only; remediation is a deliberate replan).
verification: Every pre-collected finding re-checked against source; 3 were themselves mis-anchored (E9, E18, E20 framing) and are corrected above; 21 new evidence entries and 8 additional causes recorded.
files_changed: []

