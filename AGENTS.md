# ZenOmen — Agent Engineering Rules

**ZenOmen** is a lightweight, cross-platform hardware control center for HP OMEN and Victus laptops built in Rust (edition 2024) and Slint.

Every agent working on this repository must strictly adhere to these 10 non-negotiable engineering rules:

1. **Inspect planning documents before changes**
   Read `.planning/codebase/` (ARCHITECTURE.md, CONVENTIONS.md, CONCERNS.md, STACK.md, INTEGRATIONS.md, TESTING.md) before making architectural recommendations or modifying core subsystems.

2. **Verify source before recommendations**
   Never assume old maps or docstrings are current. Always verify claims against the actual Rust source code and existing test fixtures before proposing changes.

3. **Make minimal, focused changes**
   Brownfield engineering discipline: make the smallest correct change that solves the verified defect. Never rewrite working code, refactor directories, or create multi-crate workspaces merely for aesthetic preference.

4. **No unnecessary dependencies or runtimes**
   Reuse existing crates and std libraries. Do not introduce large frameworks, async runtimes (e.g. Tokio), or speculative crates without concrete, verified justification. Keep dependencies lean to honor the ≤40 MB RSS budget.

5. **Maintain strict platform separation**
   Keep Linux-specific hardware code isolated under `src/hardware/linux/`. For unsupported or unvalidated platforms (e.g., Windows), report `UnavailableHardwareProvider` with zero capabilities — never fabricate readings or guess sysfs paths.

6. **Follow existing conventions and domain models**
   Reuse existing domain types (`FanMode`, `PresetResult`, `TelemetrySnapshot`, `AppEvent`). Single-writer UI state: modify `AppState` only through `apply(&AppEvent)`.

7. **Add focused, reliable tests**
   Every new behavior or bugfix must be covered by a unit or integration test. Tests must use isolated temporary directories and mock hardware interfaces rather than touching live hardware or root paths.

8. **Inspect the complete diff before committing**
   Run `git diff --check` and `git status` to verify whitespace, formatting (`cargo fmt --check`), and staging boundaries. Ensure no extraneous files or scratch scripts are committed.

9. **Never fabricate test results or capabilities**
   Never claim tests passed without executing `cargo test`. Never synthesize sensor counts or hardware capabilities that are not backed by readable hardware files.

10. **Synchronize documentation with code changes**
    When changing interfaces, test counts, or build requirements, update corresponding documentation (README.md, docstrings) immediately to prevent documentation drift.
