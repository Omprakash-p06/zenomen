---
phase: 01-foundation-gui-shell-on-mock-hardware
plan: "01"
subsystem: infra/ui
tags:
  - rust
  - edition-2024
  - slint
  - cargo
  - build-script
  - design-tokens
  - fonts
  - github-actions
  - cargo-audit

requires: []
provides:
  - "Compiling Rust (edition 2024) + Slint project with size-optimized release profile"
  - "Tracked Cargo.lock (555 crates) for reproducible builds and cargo audit"
  - "build.rs compiling ui/app.slint via slint_build::compile_with_config"
  - "ThemeColors / ThemeTypography / ThemeMetrics token files matching 01-UI-SPEC.md"
  - "Vendored Inter 400/600 and JetBrains Mono 400 embedded via typography.slint"
  - "AppWindow scaffold + tracing-initialised entry point that launches headlessly"
  - "GitHub Actions quality gates: fmt, clippy -D warnings, xvfb tests, cargo audit"
affects:
  - 01-02 (builds the shell and screens on these tokens and this window)
  - 01-03 (adds the mock provider, worker and tests these CI gates will run)
  - 02 (Linux telemetry — consumes ThemeMetrics/ThemeColors for real readings)
  - 05 (distribution — release profile flags already tuned for the RSS budget)

actuals:
  tokens: 43154
  tasks: 4
  commits: 4

tech-stack:
  added:
    - "slint 1.18.1 (declared 1.17; crates.io resolves the ^1.17 requirement to 1.18.1) with default-features off + backend-winit + renderer-femtovg"
    - "slint-build 1.18.1 as the build-dependency compiling ui/app.slint"
    - "tracing 0.1 + tracing-subscriber 0.3 (env-filter)"
    - "thiserror 2.0, anyhow 1.0, directories 6.0, serde 1.0, toml 0.8"
    - "Inter 4.001 (OFL 1.1) and JetBrains Mono (OFL 1.1) vendored as TrueType"
    - "GitHub Actions quality workflow with rustup-installed stable toolchain and taiki-e/install-action@cargo-audit"
  patterns:
    - "All colour/type/spacing values live in ui/theme/*.slint globals; no literal hex in a screen file"
    - "Slint UI compiled at build time by build.rs, never at runtime"
    - "Tokens declared as `out property` on exported globals so a later light theme is a second token file, not a second set of screens"

key-files:
  created:
    - Cargo.toml
    - Cargo.lock
    - build.rs
    - .gitignore
    - .github/workflows/ci.yml
    - ui/theme/colors.slint
    - ui/theme/typography.slint
    - ui/theme/metrics.slint
    - ui/theme/fonts/Inter-Regular.ttf
    - ui/theme/fonts/Inter-SemiBold.ttf
    - ui/theme/fonts/JetBrainsMono-Regular.ttf
    - ui/theme/fonts/Inter-OFL.txt
    - ui/theme/fonts/JetBrainsMono-OFL.txt
    - ui/theme/fonts/README.md
    - ui/app.slint
    - src/main.rs
  modified:
    - .planning/config.json

key-decisions:
  - "Slint is pinned to the 1.17 requirement as planned; crates.io resolves it to 1.18.1, which is semver-compatible. 01-UI-SPEC.md was NOT rewritten — the spec says 1.17 and the resolved version is recorded here instead."
  - "The initial window size is bound through Slint's `preferred-width`/`preferred-height`, because Window has no `default-width`/`default-height` property. The metric token names stay `default-width`/`default-height`."
  - "The UI-SPEC spacing tokens `2xl` and `3xl` are spelled `xxl` and `xxxl` in Slint, since identifiers cannot start with a digit. Values (48px / 64px) are unchanged."
  - "Task 1 shipped a minimal AppWindow and entry point so `cargo check` could prove the whole build pipeline (build.rs -> slint compiler -> include_modules! -> link) at the earliest moment; Task 3 replaced both with the real implementations."
  - "Font licence texts (SIL OFL 1.1) ship next to the binaries: ZenOmen is GPL-3.0 and redistributes OFL fonts, so the licence has to travel with them."
  - "CI installs the Rust toolchain through rustup rather than a third-party action, keeping the CI supply chain to actions/checkout plus the cargo-audit installer."
  - ".planning/config.json gained git.allow_default_branch_commits: true — this repo sets git.branching_strategy to \"none\", and the executor's pre-commit guard refuses commits on a protected branch without that override."

patterns-established:
  - "Token indirection: every visual value is named in ui/theme/ and referenced; a literal hex in a screen file is a defect"
  - "Verification-by-execution: the scaffold is proven by launching the real binary under xvfb, not only by compiling it"
  - "Deviation discipline: environment and Slint-API surprises are fixed inline and recorded here rather than silently absorbed"

requirements-completed:
  - FOUND-01

coverage:
  - id: D1
    description: "Rust edition 2024 + Slint project compiles, with a release profile tuned for the <=40 MB RSS budget (opt-level \"s\", lto, codegen-units 1, panic abort, strip)"
    requirement: FOUND-01
    verification:
      - kind: other
        ref: "cargo check (exit 0, no warnings); cargo fmt --all -- --check; cargo clippy --all-targets --all-features -- -D warnings"
        status: pass
    human_judgment: false
  - id: D2
    description: "Theme tokens match 01-UI-SPEC.md verbatim — obsidian #0B0C0F, charcoal #15171C, slate #1B1E25, outline #262A33, text #E8E6E3/#9BA1AC/#4E5360, gold #C6A15B, crimson #E0606B, 4 sizes (32/20/14/12px), 2 weights (400/600), 4px spacing grid, 200px sidebar, 1180x760 default / 940x620 minimum"
    verification:
      - kind: other
        ref: "grep -q '#0B0C0F' ui/theme/colors.slint && grep -q '#C6A15B' ui/theme/colors.slint; grep -E '32px|20px|14px|12px' ui/theme/typography.slint; the three token files compile when imported by a Slint component"
        status: pass
    human_judgment: false
  - id: D3
    description: "Inter 400/600 and JetBrains Mono 400 are vendored under ui/theme/fonts/ and embedded by typography.slint, so family names resolve identically everywhere"
    verification:
      - kind: other
        ref: "test -f ui/theme/fonts/Inter-Regular.ttf; slint compiler resolves the three `import \"./fonts/*.ttf\"` statements during cargo check"
        status: pass
    human_judgment: false
  - id: D4
    description: "The application window opens with the obsidian background at 1180x760 (minimum 940x620) and logs its startup through tracing"
    verification:
      - kind: e2e
        ref: "RUST_LOG=info timeout 8 xvfb-run --auto-servernum ./target/debug/zenomen -> stays running (exit 124), logs 'starting ZenOmen version=\"0.1.0\"'"
        status: pass
    human_judgment: false
  - id: D5
    description: "CI enforces cargo fmt, zero-warning clippy, headless tests under xvfb-run and cargo audit over the committed Cargo.lock"
    verification:
      - kind: other
        ref: "yaml parses with 8 steps in the planned order; every gate was reproduced locally: cargo fmt --check pass, clippy -D warnings pass, xvfb-run cargo test --all pass (0 tests), cargo audit exit 0"
        status: pass
    human_judgment: true
    rationale: "The workflow has not executed on GitHub Actions yet — first run happens on push. Local reproduction of each gate is strong evidence but not proof that the runner image and action pins behave."

duration: 16min
completed: 2026-10-31
status: complete
commits: 4
plan_head_before: 2cc7ec5344b40dbcd004deac59b639396c6ea87f
plan_head_after: 65e6ef523f4526833a79f7cf8a1694c99e02d926
---

# Phase 01 Plan 01: Rust + Slint Scaffold, Build Pipeline, Theme Tokens & CI Gates Summary

**A compiling Rust edition-2024 + Slint 1.18.1 application whose window launches under xvfb in the ZenOmen obsidian theme, with the UI-SPEC token set, embedded Inter/JetBrains Mono, a tracked lockfile and four CI quality gates.**

## Performance

- **Duration:** 16 min
- **Started:** 2026-10-31T17:17:00Z
- **Completed:** 2026-10-31T17:33:00Z
- **Tasks:** 4 of 4
- **Files modified:** 16 files changed, 6254 insertions across the 4 task commits

## Accomplishments

- **Build pipeline proven end to end, not just configured.** `build.rs` compiles `ui/app.slint` through `slint_build::compile_with_config`, and the resulting binary was launched under `xvfb-run` — the window stays up and logs `starting ZenOmen version="0.1.0"`. `cargo check`, `cargo fmt --check` and `cargo clippy --all-targets --all-features -- -D warnings` are all clean.
- **Design tokens copied verbatim from the approved spec.** `ThemeColors` carries the exact contrast-verified hexes, `ThemeTypography` declares exactly four sizes and two weights, `ThemeMetrics` the 4px grid, structural dimensions and window constraints. All three files were compiled once through a Slint component to prove the syntax, then the smoke test was reverted.
- **Fonts are real, vendored and licensed.** Inter 4.001 (400/600) and JetBrains Mono (400) ship as TrueType with their SIL OFL 1.1 texts and a provenance README, and `typography.slint` embeds them — so numeric readouts keep tabular alignment on a machine with no fonts installed.
- **Reproducible dependency graph.** `Cargo.lock` (555 crates) is committed; `cargo audit` scans exactly that graph and currently reports **0 vulnerabilities** and 5 `unmaintained` warnings (all transitive: `bincode`, `ttf-parser`, `zbus_names`, `zvariant`, `zvariant_derive`).
- **CI gates that would have caught this plan's own mistakes.** The workflow installs the native X11/GL/fontconfig packages and `xvfb` before anything compiles, then runs fmt, zero-warning clippy, headless tests and the audit.

## Task Commits

Each task was committed atomically:

1. **Task 1: Initialize Rust 2024 project, release profiles and build script** — `8ed99f7` (feat)
2. **Task 2: Provision font assets and implement exact UI-SPEC theme tokens** — `1ca2662` (feat)
3. **Task 3: Scaffold minimal Slint window and Rust entry point** — `f3d05d0` (feat)
4. **Task 4: Configure GitHub Actions CI with system libraries and security audit** — `65e6ef5` (ci)

**Plan metadata:** see the `docs(01-01)` commit that adds this SUMMARY, STATE.md and ROADMAP.md.

_Measured, not narrated: `git rev-list --count 2cc7ec5..65e6ef5` = 4 (the ledger recorded in `.git/gsd-plan-head-before-01-01`)._

## Files Created/Modified

- `Cargo.toml` — package `zenomen` 0.1.0, edition 2024, Slint 1.17 requirement with `default-features = false` + `backend-winit`/`renderer-femtovg`, and the size-oriented release profile
- `Cargo.lock` — tracked lockfile, 555 crates, `slint 1.18.1` resolved
- `build.rs` — compiles `ui/app.slint` with the `fluent` style pinned so dev and CI agree
- `.gitignore` — ignores `/target`, `.venv/`, `build/`, `dist/`, `__pycache__/`; deliberately keeps `Cargo.lock`
- `.github/workflows/ci.yml` — 8 steps: checkout, rustup toolchain, native deps, cargo-audit, fmt, clippy, xvfb tests, audit
- `ui/theme/colors.slint` — `ThemeColors` global: surfaces, outline, text, gold accent, crimson, chart series, status semantics
- `ui/theme/typography.slint` — `ThemeTypography` global: three font imports, 4 sizes, 2 weights, prose/mono families
- `ui/theme/metrics.slint` — `ThemeMetrics` global: 7-step spacing scale, structural dimensions, stroke widths, window constraints
- `ui/theme/fonts/Inter-Regular.ttf`, `Inter-SemiBold.ttf`, `JetBrainsMono-Regular.ttf` — embedded font binaries
- `ui/theme/fonts/Inter-OFL.txt`, `JetBrainsMono-OFL.txt`, `README.md` — licences and provenance
- `ui/app.slint` — `AppWindow`: title, obsidian background, `preferred-width/height` 1180×760, min 940×620, centred heading-size placeholder
- `src/main.rs` — `slint::include_modules!()`, `tracing_subscriber::fmt::init()`, startup log, `AppWindow::new()?.run()?`
- `.planning/config.json` — added `git.allow_default_branch_commits: true` (see Decisions)

## Decisions Made

- **Slint resolved to 1.18.1, not 1.17.** The plan and `01-UI-SPEC.md` both say 1.17; `version = "1.17"` is a caret requirement and crates.io's newest matching release is 1.18.1. The spec was **not** rewritten — the resolved version is recorded here so the discrepancy is visible rather than silently normalised. If 1.17 exactly is ever required, pin `=1.17.0`.
- **`preferred-width`/`preferred-height` instead of `default-width`/`default-height`.** Slint's `Window` has no `default-*` size properties; the initial size is the *preferred* size. The metric token names were kept as the plan specifies.
- **`xxl`/`xxxl` instead of `2xl`/`3xl`.** Slint identifiers cannot begin with a digit. The spec's values (48px, 64px) are unchanged and a comment in `metrics.slint` maps the names back.
- **Font licences are committed.** A GPL-3.0 project redistributing OFL-licensed binaries must ship the OFL text; the README records the exact upstream release each binary came from so the assets can be re-fetched and verified.
- **rustup for the CI toolchain.** One fewer third-party action in the supply chain, which is the threat T-01-01 this plan exists to mitigate.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Task 1 could not pass its own `cargo check` verify**
- **Found during:** Task 1 (Initialize Rust 2024 project)
- **Issue:** `build.rs` compiles `ui/app.slint`, which Task 3 creates; and a binary crate with no `src/main.rs` has no target to check. So `cargo check` at Task 1 had nothing to build against — the plan's task order and its verification step were inconsistent.
- **Fix:** Task 1 shipped a minimal `AppWindow` (title only) and a minimal `src/main.rs` (`include_modules!()` + `run()`), which proves the entire pipeline — build script, Slint compiler, macro expansion, link — immediately. Task 3 replaced both files with the real implementations, and the placeholder text in both files says so.
- **Files modified:** `ui/app.slint`, `src/main.rs`
- **Verification:** `cargo check` exit 0 with no warnings at Task 1; the final versions still pass.
- **Committed in:** `8ed99f7` (Task 1), replaced in `f3d05d0` (Task 3)

**2. [Rule 1 - Bug] `default-width`/`default-height` are not Slint Window properties**
- **Found during:** Task 3 (Scaffold minimal Slint window)
- **Issue:** `cargo check` failed in the Slint compiler: `Unknown property default-width in Window`. The plan's action text named properties that do not exist.
- **Fix:** bound the initial size through `preferred-width`/`preferred-height`; the `ThemeMetrics.default-width`/`default-height` tokens keep their names.
- **Files modified:** `ui/app.slint`
- **Verification:** `cargo check` exit 0; the window opens at 1180×760 under xvfb.
- **Committed in:** `f3d05d0` (Task 3)

**3. [Rule 1 - Bug] The spec's `2xl`/`3xl` spacing names are invalid Slint identifiers**
- **Found during:** Task 2 (theme tokens)
- **Issue:** Slint identifiers cannot start with a digit, so `out property <length> 2xl: 48px;` would not parse.
- **Fix:** spelled `xxl` (48px) and `xxxl` (64px), with a comment mapping back to the UI-SPEC names. `3xl` was added because the UI-SPEC declares it (page top offset) even though the plan's token list stopped at `2xl`.
- **Files modified:** `ui/theme/metrics.slint`
- **Verification:** the file compiles when imported by a Slint component.
- **Committed in:** `1ca2662` (Task 2)

**4. [Rule 2 - Missing Critical] Font licence texts and provenance were not in the plan**
- **Found during:** Task 2 (font provisioning)
- **Issue:** the plan required vendoring OFL-licensed fonts into a GPL-3.0 repository without their licences. Shipping the binaries without the OFL text is a licence violation, and without provenance the assets are unauditable.
- **Fix:** added `Inter-OFL.txt`, `JetBrainsMono-OFL.txt` and a `README.md` recording the upstream release and exact path of each binary.
- **Files modified:** `ui/theme/fonts/Inter-OFL.txt`, `ui/theme/fonts/JetBrainsMono-OFL.txt`, `ui/theme/fonts/README.md`
- **Verification:** files present and referenced from the README table.
- **Committed in:** `1ca2662` (Task 2)

### Configuration adjustment (not an auto-fix)

**`.planning/config.json` gained `git.allow_default_branch_commits: true`.** This repository sets `git.branching_strategy: "none"` (work happens on `main`), but the executor's pre-commit guard refuses to commit while HEAD is on a protected branch unless that override is set. Without it no plan in this project can ever commit. Recorded here because it changes how every later plan commits.

---

**Total deviations:** 4 auto-fixed (2 bugs, 1 blocking, 1 missing critical) + 1 configuration adjustment
**Impact on plan:** No scope creep. Deviations 1-3 are places where the plan's literal text did not match the Slint API or the compiler; deviation 4 is licence compliance for assets the plan required.

## Issues Encountered

- **`rustfmt` and `clippy` were absent from the freshly installed toolchain.** `cargo fmt --check` failed with "cargo-fmt is not installed". Installed via `rustup component add rustfmt clippy`; no repository change.
- **`cargo audit` reports 5 `unmaintained` warnings, 0 vulnerabilities.** `bincode 2.0.1`, `ttf-parser 0.25.1`, `zbus_names 4.3.5`, `zvariant 5.15.1`, `zvariant_derive 5.15.1` — all transitive through Slint/zbus, none with a known CVE. `cargo audit` exits 0, so CI stays green; this needs re-checking when Slint moves past these dependencies (relevant to Phase 5's audit-gated release).
- **`xkbcommon` prints Compose-file parse errors under Xvfb.** `/usr/share/X11/locale/iso8859-1/Compose` is not valid UTF-8 in this container. It is an environment artifact of the minimal image, not application output — the window still opens and renders. CI installs the full X11 stack on `ubuntu-latest`, where these do not appear.

## Known Stubs

| Stub | File | Reason |
|------|------|--------|
| Centred `Text { text: "ZenOmen" }` placeholder as the window body | `ui/app.slint` | Intentional and specified by this plan: the sidebar, header, status strip and six screens replace it in plan 01-02. The token bindings it demonstrates are permanent. |
| Placeholder comment blocks referencing Task 3 | `ui/app.slint` (Task 1 revision only) | Removed by Task 3; the final file has no placeholder comments. |

No stub in this plan prevents its goal from being achieved: the deliverable is the scaffold, and the scaffold runs.

## Known Gaps

- **FOUND-01 is only half-proven here.** This plan delivers "user can launch ZenOmen and see the app in the ZenOmen dark theme" — the window launches with the obsidian background and spec typography. The *Overview page* half of FOUND-01 lands in plan 01-02. The requirement is marked complete per this plan's frontmatter mapping, so the phase verifier must confirm the Overview page before FOUND-01 is treated as satisfied.
- **The CI workflow has not run on GitHub Actions yet.** Every gate was reproduced locally and the YAML parses, but the first real run happens on push to `main`.

## User Setup Required

None — no external service configuration. The Rust toolchain, native X11/GL/fontconfig packages and `xvfb` are already present on this machine; CI installs its own.

## Next Phase Readiness

- **Ready for 01-02:** `AppWindow` exists and compiles, the three token globals are importable, and the build script recompiles the Slint file on change. The shell plan can add `ui/components/*.slint` and screens without touching `Cargo.toml`.
- **Ready for 01-03:** the CI gates are already strict enough to catch what that plan adds (tests now run 0 tests; they will run the mock provider's suite). `cargo audit` is green on the current graph.
- **Watch:** `slint 1.18.1` vs the spec's 1.17. API drift between the two is not expected (semver-compatible), but if a later plan depends on a 1.17-only behaviour, pin `=1.17.0` and re-run `cargo audit`.

## Self-Check: PASSED

Created files verified present (`[ -f ... ]` over all 13 artifacts plus this SUMMARY):

```
FOUND: Cargo.toml
FOUND: Cargo.lock
FOUND: build.rs
FOUND: .gitignore
FOUND: .github/workflows/ci.yml
FOUND: ui/theme/colors.slint
FOUND: ui/theme/typography.slint
FOUND: ui/theme/metrics.slint
FOUND: ui/theme/fonts/Inter-Regular.ttf
FOUND: ui/theme/fonts/Inter-SemiBold.ttf
FOUND: ui/theme/fonts/JetBrainsMono-Regular.ttf
FOUND: ui/app.slint
FOUND: src/main.rs
FOUND: .planning/phases/01-foundation-gui-shell-on-mock-hardware/01-01-SUMMARY.md
```

Task commits verified reachable from HEAD (`git merge-base --is-ancestor`):

```
FOUND: 8ed99f7
FOUND: 1ca2662
FOUND: f3d05d0
FOUND: 65e6ef5
```

Gate evidence:

```
$ cargo check
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.17s      (exit 0, no warnings)

$ cargo fmt --all -- --check
FMT_RC=0

$ cargo clippy --all-targets --all-features -- -D warnings
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.26s      (exit 0, no warnings)

$ xvfb-run --auto-servernum cargo test --all
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out   (exit 0)

$ cargo audit
      Scanning Cargo.lock for vulnerabilities (555 crate dependencies)
warning: 5 allowed warnings found                                             (exit 0, 0 vulnerabilities)

$ RUST_LOG=info timeout 8 xvfb-run --auto-servernum ./target/debug/zenomen
 INFO zenomen: starting ZenOmen version="0.1.0"
RUN_RC=124   (window still open when the timeout killed it)
```

---
*Phase: 01-foundation-gui-shell-on-mock-hardware*
*Completed: 2026-10-31*
