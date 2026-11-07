# Phase 6 Context — Source of Truth

**Created:** 2026-11-07
**Status:** Authoritative input for Phase 6
**Supersedes:** the unpersisted `pasted-text-4.txt` referenced by `06-RESEARCH.md:5`

## Why this file exists

Phase 6 was previously planned against a specification that existed only in a chat
attachment. `06-RESEARCH.md` cited `pasted-text-4.txt`, which was never committed to the
repository. Every downstream artifact therefore inherited silent losses, and no plan
could be verified against its own source.

This file is the committed, authoritative reproduction of the user's directive. If a plan
conflicts with this document, the plan is wrong. Changes to this document require an
explicit user decision and a new entry in the Decisions section.

## Provenance

| Field | Value |
|---|---|
| Captured | 2026-11-07, during `/gsd-debug phase-06-plan-quality` |
| Source | User-supplied directive in the `/gsd-debug` response turn |
| Prior artifacts reconciled | `06-RESEARCH.md`, `ROADMAP.md` Phase 6, `REQUIREMENTS.md`, `ADR-001` |
| Debug session | `.planning/debug/phase-06-plan-quality.md` |

## Decisions captured with the directive

| # | Decision | Consequence |
|---|---|---|
| D1 | Keep the work inside v1 as Phase 6; amend `REQUIREMENTS.md` rather than opening a v2 milestone. | Windows requirements are promoted into v1; traceability gains Phase 6 rows. |
| D2 | **GPUI-CE is the final production UI.** Slint is retired after parity. | A prototype is permitted only as an internal unblocker; it must not become the delivered end state. ADR-001 is superseded. |
| D3 | All plans are produced in one go, at task granularity. | No coarse five-wave decomposition; each task carries CHANGE ID, requirement ID, source, before/after code, tests. |
| D4 | No guaranteed Windows HP hardware access. | Firmware writes ship disabled behind an unverified board profile; infrastructure, fixtures and a validation procedure are still delivered. |
| D5 | One coordinated implementation, not a phased roadmap. | Execution proceeds workstream-by-workstream into one integration branch; internal decomposition is allowed, deferred product milestones are not. |

---

# VERBATIM DIRECTIVE

## PRIMARY DIRECTIVE

Implement the complete ZenOmen modernization in ONE coordinated execution.

Do not create a phased delivery roadmap.
Do not stop after architecture planning.
Do not implement Linux improvements first and postpone Windows.
Do not deliver GPUI as a prototype while leaving Slint as the permanent UI.
Do not leave Windows hardware support as a future milestone.

The final deliverable is one integrated ZenOmen application supporting Linux, Windows 10 and Windows 11.

All required implementation, migration, remediation, testing, documentation, packaging and release preparation must be completed in this execution.

Internal task decomposition is permitted and required. Separate implementation phases, partial product deliveries and deferred implementation milestones are not.

## 1. REQUIRED FINAL PRODUCT

Deliver:

- GPUI-CE as the shared UI framework.
- Linux hardware provider.
- Windows hardware provider.
- Windows 10 and Windows 11 desktop support.
- HP OMEN/Victus hardware discovery.
- Verified Windows fan, power and RGB control where firmware supports it.
- Linux D-Bus/systemd hardware service.
- Windows hardware service with restricted named-pipe IPC.
- Cross-platform telemetry and diagnostics.
- Native tray, autostart and single-instance handling.
- Cross-platform logging.
- Windows installer and uninstaller.
- Existing Linux packaging.
- Complete automated regression coverage.
- Updated architecture maps and documentation.

Do not preserve Slint as the final production UI.

## 2. SOURCE-OF-TRUTH GOVERNANCE

Before implementation:

1. Inspect every document in `.planning/codebase/`.
2. Inspect the actual planning directory.
3. Read all of `CONCERNS.md`.
4. Inspect the complete Rust source tree.
5. Inspect Cargo.toml, build.rs and Cargo.lock.
6. Inspect the actual Git branch, HEAD, working tree and uncommitted changes.
7. Verify every codebase-map claim against source.
8. Reconcile all conflicting requirement IDs and test counts.

Do not assume old line numbers, file counts or symbols are accurate.

Every change must reference an existing source-document finding and requirement ID.

Correct missing and contradictory requirement IDs before implementation.

## 3. IMPLEMENTATION SCOPE

Implement the following together.

### A. GPUI-CE migration

- Replace the complete Slint UI.
- Rebuild every screen and component.
- Preserve existing application functionality.
- Preserve dark theme, typography, spacing and interaction behavior.
- Implement telemetry charts and hardware controls.
- Implement accessible keyboard navigation.
- Support Linux Wayland/X11 and Windows Win32/DirectX.
- Pin compatible GPUI-CE dependencies.
- Remove Slint dependencies and `.slint` files after parity verification.

GPUI-CE requires Rust 1.95+. Verify the actual dependency APIs and native build requirements.

### B. Windows hardware support

Implement native Windows providers for:

- CPU.
- Memory.
- GPU.
- Battery.
- Supported temperatures.
- Fan RPM.
- System identity.
- Hardware capability discovery.

Use documented Win32, DXGI, PDH, WMI and HP firmware interfaces.

Implement HP BIOS WMI integration using verified board profiles.

Do not assume all OMEN/Victus models share firmware commands.

Implement fan, power, RGB and MUX controls only when their command encoding and read-back behavior have been verified.

If actual Windows HP hardware is unavailable, keep unverified firmware writes disabled. Complete the implementation infrastructure, test fixtures and validation procedure without falsely claiming hardware validation.

### C. Windows service and security

Implement:

- Windows Service Control Manager integration.
- Restricted named-pipe IPC.
- Client identity validation.
- Typed request/response protocol.
- Request limits and timeouts.
- Serialized hardware mutations.
- Safe shutdown and recovery.
- Explicit service installation and removal.

The GUI must not require administrator privileges.

### D. Linux remediation

Resolve every unresolved item in the current CONCERNS.md.

Include:
- D-Bus/systemd activation security.
- sysfs write restrictions.
- daemon logging.
- Polkit testing.
- hardware discovery consistency.
- battery capability correctness.
- RGB profile preservation.
- malformed RGB parsing.
- history duration.
- shutdown behavior.
- D-Bus responsiveness.
- single-instance behavior.
- autostart preservation.
- diagnostics durability.
- NVIDIA sampler consistency.
- chart performance.
- benchmark correctness.
- resource measurement.
- live-host test isolation.

Preserve all already-resolved fixes.

### E. Cross-platform desktop integration

Implement:
- Single instance.
- Window activation.
- Tray.
- Notifications.
- Autostart.
- Release-page opening.
- Diagnostics.
- User and service data paths.
- Logging.
- Installer and uninstaller.

Use native APIs and correct permission boundaries for each OS.

### F. Resource and build optimization

Preserve lean dev/test profiles.

Avoid unnecessary dependencies, duplicate implementations, excessive thread creation and repeated process spawning.

Measure memory, CPU and startup performance on both platforms.

Do not claim resource-budget compliance using substituted metrics.

## 4. EXACT IMPLEMENTATION REQUIREMENTS

For every task, specify:

```text
CHANGE ID:
REQUIREMENT ID:
SOURCE DOCUMENT:
SOURCE SECTION:

EXISTING FILE:
EXISTING SYMBOL:

PROBLEM:

PROPOSED CHANGE:

EXACT FILES TO CREATE:
EXACT FILES TO MODIFY:
EXACT FILES TO DELETE:

BEFORE CODE:
AFTER CODE:

DEPENDENCY CHANGES:

TESTS:
EXPECTED RESULTS:

REGRESSION RISKS:

DOCUMENTATION UPDATES:

COMMIT MESSAGE:
```

Do not create unnecessary crates, modules or abstractions.

Every new file must have a clear responsibility.

## 5. PARALLEL EXECUTION

Execute independent work concurrently.

Assign separate agents to:

1. GPUI-CE UI migration.
2. Windows telemetry.
3. Windows HP WMI and hardware service.
4. Linux hardware and daemon remediation.
5. Windows desktop integration and packaging.
6. Test infrastructure and regression verification.
7. Documentation and traceability.

The lead agent owns shared contracts, Cargo configuration, integration and conflict resolution.

Agents must not independently modify shared interfaces without an agreed contract.

Establish interfaces before parallel implementation.

Use isolated branches or worktrees for conflicting source areas.

Merge completed work into one integration branch.

Do not let documentation agents declare unverified implementation complete.

## 6. TESTING

Run:

```bash
cargo fmt --all -- --check
cargo check --all-targets
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all
```

Also verify:

- Linux release build.
- Linux daemon build.
- Windows MSVC GUI build.
- Windows service build.
- Windows native tests.
- GPUI rendering.
- Windows tray and autostart.
- Linux D-Bus and Polkit.
- Hardware provider contracts.
- RGB and fan-control safety.
- Shutdown and service recovery.
- Installer and uninstaller.
- Memory and CPU budgets.

Use Gemini for independent test execution and review of the exact final diff.

Gemini must not silently modify production code.

Hardware-dependent tests must be identified separately from deterministic CI tests.

## 7. GIT DISCIPLINE

One coordinated implementation branch.

Use focused conventional commits for individual completed tasks.

Do not combine unrelated changes into one commit.

Do not push or merge without authorization.

Keep a complete mapping between:
- Task.
- Changed files.
- Tests.
- Commit.
- PR contents.

## 8. DOCUMENTATION

Update the existing documents rather than creating duplicate architecture maps.

Synchronize:
- STACK.md
- ARCHITECTURE.md
- STRUCTURE.md
- CONVENTIONS.md
- INTEGRATIONS.md
- TESTING.md
- CONCERNS.md
- WINDOWS.md
- PROJECT.md
- REQUIREMENTS.md
- ROADMAP.md
- STATE.md
- All affected ADRs.

Record verified implementation, unverified implementation and hardware-dependent limitations separately.

Do not rewrite historical audit findings.

## 9. FINAL VERIFICATION GATE

Do not declare completion until:

1. GPUI replaces Slint in the production application.
2. Linux builds and tests pass.
3. Windows 10 and Windows 11 builds pass.
4. Windows telemetry works through native providers.
5. Windows hardware service and IPC security pass tests.
6. Verified HP hardware controls work on supported board profiles.
7. Unverified firmware controls remain disabled.
8. Every unresolved CONCERNS.md item is fixed or explicitly documented as blocked by physical hardware access.
9. All test counts match actual test enumeration.
10. All codebase maps match the final source.
11. Gemini has independently reviewed the final implementation.
12. The final Git diff contains no unrelated or accidental files.

## 10. EXECUTION RULE

Do not return another multi-phase roadmap.

Do not stop after producing plans.

Execute the entire coordinated implementation in this run. Continue through integration, testing, documentation and final verification.

If an external dependency, missing physical device or unavailable Windows environment prevents verification, implement everything that can be safely completed, mark only the genuinely blocked verification, and provide exact evidence and the remaining hardware-validation procedure.

The objective is one complete cross-platform ZenOmen implementation, not another modernization proposal.
