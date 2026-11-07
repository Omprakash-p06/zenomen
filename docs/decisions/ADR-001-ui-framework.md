# ADR-001: Retain Slint as the UI framework

- **Status:** Proposed (awaiting approval — no production code has been changed)
- **Date:** 2026-11-07
- **Decision owner:** ZenOmen maintainers
- **Supersedes:** none
- **Related:** `.planning/debug/zenomen-modernization-audit.md` § B.1

## Context

ZenOmen's entire presentation layer is written in the Slint DSL: 20 `.slint` files
(`ui/app.slint`, 6 screens, 13 components, 3 theme files) plus a 545-line Rust bridge
(`src/app.rs`) that maps callbacks to `AppCommand` values and `AppEvent` values to
Slint properties.

A modernization review proposed evaluating GPUI Community Edition as a replacement,
citing GPU-accelerated rendering and the note that GPUI-CE requires Rust 1.95+.

Two hard constraints frame the decision:

1. **Memory and idle CPU are stated product requirements** — ≤40 MB peak RSS for the GUI
   process and ≤0.1 % idle CPU. The current build deliberately disables Slint's default
   Qt/Skia backends to protect that budget (`Cargo.toml:11`).
2. **The project's core value is trustworthy hardware control.** A framework migration
   that reintroduces rendering or lifecycle bugs trades a headline promise for aesthetics.

## Options considered

### Option A — Retain Slint (chosen)

- Declared `slint = "1.17"` (caret), locked **1.18.1** (2026-09-21). MSRV for 1.18 is **1.88**.
- Licensed GPL-3.0 / royalty-free / commercial; the GPL option is compatible with this
  project's GPL-3.0-only licence and requires no purchase.
- Both Linux and Windows windowing are supported through the `backend-winit` backend
  already in use; `build.rs` already embeds the Windows icon via `winres`.
- Slint ships an `accessibility` feature (a *default* feature) that this build currently
  disables via `default-features = false`. Accessibility is therefore achievable without
  changing frameworks — it is a build-configuration decision, not a capability gap.

### Option B — Migrate to GPUI Community Edition

- Latest stable **0.3.0** (2026-10-20); pre-1.0, with the README stating breaking changes
  occur between versions. Four versions published since 2025-12; **0.3.2 and 0.3.3 were
  yanked**.
- MSRV **1.95**. Apache-2.0 (GPLv3-compatible), so licensing is not a blocker.
- Linux needs `libxkbcommon`, `wayland`, `x11` and EGL/GLES development libraries;
  Windows uses Win32 + DirectX. Both are documented as supported.
- **No declarative DSL.** Every screen becomes Rust element-tree code, so all 20 `.slint`
  files and the bridge's property surface would be rewritten. Slint's live preview and
  language-server tooling would be lost.
- The crate is ~82 K SLoC of source — a full editor framework, not a small widget kit.

## Decision

**Retain Slint.** Do not migrate the production UI. Do not create a GPUI prototype as
part of this work.

The deciding factors, in order:

1. **The cost is a rewrite, not a dependency swap.** The brief's own constraint —
   "Do not rewrite the UI before the framework decision is justified" — is satisfied
   only by evidence, and there is none: neither framework was measured.
2. **Pre-1.0 churn is disqualifying for the trust promise.** Two yanked releases in ten
   months against a project whose value proposition is reliability.
3. **The MSRV argument is weak.** Slint already forces 1.88; GPUI-CE forces 1.95. The
   delta is seven minor versions, and building this project already requires rustup.
   The development machine runs 1.99.1.
4. **No measured advantage exists.** Memory and startup for GPUI-CE were not measured in
   this audit, and the brief forbids adopting a technology "because it is popular".

## Consequences

**Positive**

- No rewrite; all existing UI behaviour, tests and packaging metadata remain valid.
- The RSS budget's existing feature discipline
  (`std`, `compat-1-2`, `backend-winit`, `renderer-femtovg` only) continues to apply.
- Effort is redirected to source-verified defects: permanent degraded state, a preset
  that reports success on total failure, an orphaned-write path, and per-second
  `nvidia-smi` spawning.

**Negative / accepted**

- The desktop UI stays on a smaller ecosystem than GPUI's Zed-derived tooling.
- Slint's accessibility support remains disabled until someone explicitly enables the
  feature and re-measures the RSS budget. This ADR does not fix that; it records it.
- If the project ever wants GPUI's rendering model, the rewrite cost remains and will be
  larger later.

## Conditions to revisit

Reopen this ADR when **all** of the following hold:

1. A standalone GPUI-CE spike measures RSS and cold-start at feature parity with the
   current build on the same hardware and the same measurement window.
2. GPUI-CE reaches 1.0, or ships six consecutive months without a breaking minor and
   without a yanked release.
3. The UI rewrite is separately scoped and funded — not bundled with a bugfix phase.

## Verification required before this ADR moves to Accepted

- [ ] Record the current release build's peak RSS and idle CPU with
      `scripts/measure-budget.sh` on real hardware (not Xvfb/llvmpipe).
- [ ] Confirm the exact `slint` version policy: either pin `=1.17` or update the
      `Cargo.toml` comment, which currently claims a pin the caret requirement does
      not provide.
- [ ] Decide the accessibility question explicitly (enable the feature and re-measure,
      or record the omission as a known limitation in `README.md`).

## References

- Slint 1.18 release notes — https://slint.dev/blog/slint-1.18-released (2026-09-16)
- Slint CHANGELOG — https://github.com/slint-ui/slint/blob/master/CHANGELOG.md
- Slint 1.18.1 feature flags — https://docs.rs/crate/slint/1.18.1/features
- gpui-ce on docs.rs — https://docs.rs/crate/gpui-ce/latest
- gpui-ce versions — https://crates.io/crates/gpui-ce/versions
- GPUI-CE setup requirements — https://github.com/gpui-ce/gpui-ce/blob/main/SETUP.md
