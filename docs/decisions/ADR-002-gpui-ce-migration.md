# ADR-002: Migrate the production UI to GPUI-CE

- **Status:** Accepted (user decision, 2026-11-07)
- **Date:** 2026-11-07
- **Decision owner:** ZenOmen maintainers
- **Supersedes:** ADR-001 (Retain Slint as the UI framework)
- **Related:** `.planning/phases/06-cross-platform-windows-linux-release/06-CONTEXT.md`, `REQUIREMENTS.md` `UI-01`…`UI-04`

## Context

ADR-001 proposed retaining Slint. It was never accepted — its status remained
"Proposed". Phase 6 was then planned as though the decision were settled, and
`06-RESEARCH.md` cited it as authority.

The Phase 6 directive (persisted in `06-CONTEXT.md`) explicitly overrides that
position:

> Do not deliver GPUI as a prototype while leaving Slint as the permanent UI.

The product target is one application with one UI stack on Linux and Windows 10/11.
GPUI-CE 0.3.0 documents Win32 + DirectX on Windows and Wayland/X11 on Linux.

## Decision

**GPUI-CE is the production UI for all supported platforms.** Slint is retired.

- `gpui-ce` is pinned to an exact version (`=0.3.0` unless a verified compatible
  patch exists) and paired with a compatible platform crate version.
- Migration is staged internally (shell/theme/components → screens → removal of
  Slint), but staging is scheduling, not a deferred product decision.
- Slint source and dependencies are deleted only after a parity checklist passes
  on Linux and Windows. Until then both builds exist in the tree behind the `ui`
  feature; the release artifact always uses GPUI.
- A time-boxed feasibility gate runs first. If GPUI-CE cannot build on a target
  with the pinned version, that is a blocker escalated to the user with evidence;
  it is not a reason to silently retain Slint.

## Parity checklist (release gate)

| # | Check | Evidence |
|---|---|---|
| 1 | Six screens reachable from sidebar and keyboard | UI test + manual capture |
| 2 | Overview/cooling/performance/lighting/settings/diagnostics data identical to Slint implementation | Side-by-side capture |
| 3 | Dark theme tokens, typography and spacing match | Visual comparison |
| 4 | Telemetry charts render the same three series with gap handling | Unit test on path generation |
| 5 | Fan/power/RGB actions produce the same `AppCommand`s | Event-level tests |
| 6 | Diagnostics export unchanged | JSON golden test |
| 7 | Linux X11 and Wayland launch | CI + manual |
| 8 | Windows 10/11 Win32/DirectX launch | Windows CI + manual |
| 9 | Keyboard navigation and focus order | Manual + automated where possible |
| 10 | GUI RSS ≤ 40 MB, idle CPU ≤ 0.1 % | `scripts/measure-budget.sh` / Windows process APIs |

## Consequences

**Positive:** one UI stack, GPU-accelerated rendering, removes the Slint↔Rust
bridge (`src/app.rs`, ~545 lines), removes `slint-build` from the build graph,
and eliminates the dual-backend risk of shipping two UIs.

**Negative / risks:**

| Risk | Mitigation |
|---|---|
| GPUI-CE is pre-1.0 and yanked releases exist | Pin exact versions; commit `Cargo.lock`; upgrade only with a passing parity run |
| Rust 1.95+ required | Toolchain is 1.99.1 here; record `rust-version` and CI toolchain |
| Native deps (xkbcommon, wayland, EGL, DXGI) | Feasibility gate builds on both targets before screen work |
| Memory budget regression | Measure at each screen milestone, not only at the end |
| Declarative `.slint` files must be rewritten as Rust element trees | Port screen-by-screen using the existing `.slint` as the layout spec |
| Windows runtime verification unavailable locally | Windows builds are type-checked in CI; runtime verification is explicitly reported as blocked |

## Rollback

The migration lives on `feat/cross-platform-release`. Slint files remain in the
tree until the parity checklist passes; reverting the GPUI commits restores a
working Slint build without data loss.
