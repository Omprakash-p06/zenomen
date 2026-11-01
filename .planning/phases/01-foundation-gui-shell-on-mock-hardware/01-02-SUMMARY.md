---
phase: 01-foundation-gui-shell-on-mock-hardware
plan: "02"
subsystem: ui/core
tags:
  - rust
  - slint
  - gui-shell
  - sidebar-navigation
  - keyboard-navigation
  - event-bridge
  - capability-degradation
  - components

requires:
  - "01-01: theme token globals (ThemeColors/ThemeTypography/ThemeMetrics), AppWindow, build.rs Slint compilation"
provides:
  - "Twelve-component Slint library (sidebar, header, status strip, cards, pills, panels, icons, sparkline, chart, buttons)"
  - "AppWindow shell: 200px sidebar + 64px header + 40px status strip + one scrolling content viewport with a screen router"
  - "Keyboard-traversable sidebar: Up/Down, Return/Space to activate, Escape returns focus to the active row"
  - "Six screens (Overview, Cooling, Performance, Lighting, Settings, Diagnostics) with per-subsystem UnavailablePanel gating"
  - "Typed bridge: AppCommand (UI -> core) and AppEvent (core -> UI) with AppState as the single source of truth"
  - "src/app.rs ZenOmenApp: bounded command channel, try_send back-pressure, all UI writes inside slint::invoke_from_event_loop"
  - "Status strip bound to core state (status-text / status-degraded) with a View Diagnostics action"
affects:
  - 01-03 (installs the mock provider behind the drain-thread seam and fills the telemetry properties)
  - 02 (Linux telemetry feeds AppEvent::TelemetryUpdated instead of the placeholder capabilities)
  - 03 (fan/power writes will queue AppCommands through the same channel)
  - 04 (settings screen gains the history-window control already declared as AppCommand::SetHistoryWindow)

actuals:
  tokens: 29892
  tasks: 4
  commits: 4

tech-stack:
  added: []
  patterns:
    - "Event bridge: UI callbacks only queue AppCommand on a bounded channel; every visible change is an AppEvent applied inside invoke_from_event_loop"
    - "Empty string means 'not measured' and the screens render 'Waiting for first reading'; a missing interface renders 'Unavailable' — never a fabricated 0"
    - "Capability gating per subsystem: each screen names the exact interface that is missing instead of one coarse unsupported state"
    - "Declarative component library over tokens: no literal hex or pixel value in a screen file"

key-files:
  created:
    - src/core/mod.rs
    - src/core/events.rs
    - src/core/state.rs
    - src/app.rs
    - ui/components/action-button.slint
    - ui/components/capability-notice.slint
    - ui/components/icons.slint
    - ui/components/metric-card.slint
    - ui/components/screen-header.slint
    - ui/components/section-card.slint
    - ui/components/sidebar.slint
    - ui/components/sparkline.slint
    - ui/components/status-pill.slint
    - ui/components/status-strip.slint
    - ui/components/telemetry-chart.slint
    - ui/components/unavailable-panel.slint
    - ui/components/value-readout.slint
    - ui/screens/overview.slint
    - ui/screens/cooling.slint
    - ui/screens/performance.slint
    - ui/screens/lighting.slint
    - ui/screens/settings.slint
    - ui/screens/diagnostics.slint
  modified:
    - src/hardware/mod.rs
    - src/hardware/provider.rs
    - src/main.rs
    - ui/app.slint

key-decisions:
  - "The status strip sits directly under the header, not at the bottom of the window as the plan's Task 2 text says: 01-UI-SPEC.md fixes the reading order as header -> global status -> screen anchor."
  - "The Overview grid uses explicit absolute positioning with a computed column count (4 columns >= 928px, 3 columns >= 692px, 2 columns >= 456px) because Slint GridLayout row/col bindings must be compile-time constants."
  - "Telemetry properties are strings formatted by the core; the UI appends units itself, so the core never decides typography and the UI never parses a number."
  - "Navigation is written back to the window only on AppEvent::ScreenChanged, never on every event: the sidebar moves immediately on click while the command travels asynchronously, and republishing the core's pointer on a telemetry tick would snap the user back."
  - "AppState::default() reports no capabilities (HardwareCapabilities::none() is the Default), while ui/app.slint's property defaults stay true for the Phase 1 mock-nominal look; ZenOmenApp::initialize publishes the real set before the first frame so the two never disagree on screen."
  - "The command channel is a sync_channel(8) drained by a named thread; a full queue drops the request with a debug log rather than blocking the UI thread (threat T-01-03)."
  - "ZenOmenApp holds the window as slint::Weak so a worker thread can never keep a closed window alive."

patterns-established:
  - "Verification by execution: every screen, the keyboard path and the degraded path were proven with Xvfb screenshots, not only by compiling"
  - "Seam-first stubs: the drain thread, TelemetryUpdated and the history window exist as declared contracts with targeted allow(dead_code, reason = ...) rather than being invented in 01-03"

requirements-completed:
  - FOUND-02
  - FOUND-04
  - FOUND-05

coverage:
  - id: D1
    description: "All six screens are reachable from the sidebar by mouse and by keyboard traversal (Up/Down, Return/Space, Escape)"
    requirement: FOUND-02
    verification:
      - kind: e2e
        ref: "Xvfb run: clicking each of the six sidebar rows rendered its screen (screenshots of all six); keyboard Down Down Return logged 'screen requested screen=\"Performance\"' through the command drain and the window rendered Performance"
        status: pass
    human_judgment: false
  - id: D2
    description: "Sidebar is a fixed 200px column with a 2px gold rail on the active row and a visible focus ring for keyboard users"
    verification:
      - kind: e2e
        ref: "Screenshots at 1180x760 and 940x620: sidebar width unchanged, gold rail follows the active row, focus ring drawn on the row that holds keyboard focus"
        status: pass
    human_judgment: false
  - id: D3
    description: "A zero-capability run launches, renders subsystem-specific UnavailablePanels and never blanks a panel or panics"
    requirement: FOUND-04
    verification:
      - kind: e2e
        ref: "Temporary app.initialize(HardwareCapabilities::none()) run: Overview shows 'No hardware interfaces detected', the strip turns gold with a warning glyph and View Diagnostics, Diagnostics lists all eight interfaces Unavailable with a per-interface reason; process stayed up and exited only on kill"
        status: pass
    human_judgment: false
  - id: D4
    description: "UI state changes only through typed core events; no hardware access exists in the UI layer"
    requirement: FOUND-05
    verification:
      - kind: other
        ref: "src/app.rs: every UI write happens inside slint::invoke_from_event_loop; callbacks only try_send an AppCommand. No ui/*.slint file references a path, sensor or process. cargo clippy --all-targets --all-features -- -D warnings exit 0"
        status: pass
    human_judgment: true
    rationale: "Absence of hardware access is proven by inspection plus the architecture (no hardware import in any UI file); a future contributor could still add one, which is why Phase 2 keeps this under review."
  - id: D5
    description: "The window reflows from 1180x760 to the 940x620 minimum without clipping, dropping the Overview grid from four columns to three"
    verification:
      - kind: e2e
        ref: "Xlib configure to 940x620: window reports 940x620, Overview reflowed to three columns with the fourth card wrapping, the content viewport gained a scrollbar, nothing clipped"
        status: pass
    human_judgment: false

duration: 22min
completed: 2026-11-01
status: complete
commits: 4
plan_head_before: f7bede7db13bf260cc8324ad2378f7888284627e
plan_head_after: bcf587aa120e3b6125838117b4fc92ff1ad01f7d
---

# Phase 01 Plan 02: GUI Shell, Six Screens, Keyboard Navigation & Typed Event Bridge Summary

**A six-screen Slint shell with a 200px keyboard-traversable sidebar, per-subsystem degradation panels, and a typed `AppCommand`/`AppEvent` bridge that keeps every visible value in the Rust core.**

## Performance

- **Duration:** 22 min (first task commit 11:07 IST to last 11:29 IST)
- **Started:** 2026-11-01T05:37:42Z (Task 1 commit; the plan's first files were authored in a prior session — see Issues)
- **Completed:** 2026-11-01T05:59:25Z
- **Tasks:** 4 of 4
- **Files modified:** 30 files changed, 2821 insertions / 21 deletions across the 4 task commits

## Accomplishments

- **The shell is real, not a mock-up.** One `ScrollView` viewport with a router on `current-screen`, a fixed 200px sidebar with a 2px gold rail, a 64px header carrying the per-screen title and primary action, and a 40px status strip directly under it. The router falls back to Overview for any out-of-range index, so a bad number can never blank the window (T-01-04).
- **Keyboard navigation works through the whole stack.** Up/Down move the sidebar selection, Return/Space activates it, Escape returns focus to the active row. The activation travels the real path — Slint callback → `AppCommand::Navigate` → drain thread → `AppEvent::ScreenChanged` → `invoke_from_event_loop` → window — which is what the Xvfb log shows: `screen requested screen="Performance"`.
- **Every subsystem explains itself.** Each screen gates on its own capability flag and renders an `UnavailablePanel` naming the exact missing interface ("hp-wmi exposes no writable pwm1_enable here", not "unsupported"). Diagnostics turns the eight flags into a status table with the same reasons.
- **The bridge is the only way in.** `AppState` folds `AppEvent`s; `ZenOmenApp` publishes them inside `slint::invoke_from_event_loop`; UI callbacks do nothing but `try_send` on a `sync_channel(8)`, dropping rather than blocking when the core is behind. No UI file names a path, a sensor or a process.
- **The empty state is honest by construction.** Telemetry properties are empty strings until a sample exists and the screens render "Waiting for first reading"; a missing interface renders "Unavailable"; a zero-total RAM reading renders nothing rather than "0 MB".
- **Seven unit tests pin the state semantics** — waiting vs. degraded, one missing interface, a partial sample (11 of 12), and a full sample (12 of 12) — so 01-03 inherits a tested contract instead of prose.

## Task Commits

Each task was committed atomically:

1. **Task 1: Pre-declare hardware telemetry types and author the component library** — `cc8738e` (feat)
2. **Task 2: Implement sidebar navigation and the keyboard-traversable app shell** — `13beb61` (feat)
3. **Task 3: Implement all six screens with subsystem-specific degradation** — `6a8f355` (feat)
4. **Task 4: Build the event bridge and Slint event-loop dispatcher** — `bcf587a` (feat)

**Plan metadata:** see the `docs(01-02)` commit that adds this SUMMARY, STATE.md and ROADMAP.md.

_Measured, not narrated: `git rev-list --count f7bede7..bcf587a` = 4 (the ledger recorded in `.git/gsd-plan-head-before-01-02`)._

## Files Created/Modified

- `src/core/events.rs` — `ScreenId` (six variants, `ALL`/`index`/`from_index`/`title`), `AppCommand::{Navigate, RefreshTelemetry, SetHistoryWindow}`, `AppEvent::{ScreenChanged, CapabilitiesLoaded, TelemetryUpdated}`
- `src/core/state.rs` — `AppState` (active screen, capabilities, latest snapshot, `HISTORY_WINDOW = 120`), `apply`, `status_text`, `is_degraded`, plus 7 tests
- `src/core/mod.rs` — module surface and re-exports for `AppCommand`, `AppEvent`, `ScreenId`, `AppState`
- `src/app.rs` — `ZenOmenApp` (`Weak<AppWindow>` + `SyncSender<AppCommand>` + `Arc<Mutex<AppState>>`), `install_callbacks`, `initialize`, `handle_event`, `apply_to_ui`, formatting helpers
- `src/hardware/provider.rs` — `Default for HardwareCapabilities` now delegates to `none()`; `HardwareProvider` carries an `allow(dead_code, reason)` until 01-03 implements it
- `src/hardware/mod.rs` — re-exports the data types only; the trait stays behind `provider::`
- `src/main.rs` — window creation, `set_app_version`, bridge wiring, `initialize(all_supported())`, `zenomen-command-drain` thread, clean shutdown log
- `ui/app.slint` — shell (sidebar + header + status strip + `ScrollView`), `current-screen` fallback router, capability/telemetry/status properties, screen-titles array, Escape focus handling
- `ui/components/sidebar.slint` — 200px column, gold rail, `focus-active-row()`, arrow/Return/Space handling
- `ui/components/{screen-header,status-strip,status-pill,section-card,metric-card,value-readout,capability-notice,unavailable-panel,action-button,icons,sparkline,telemetry-chart}.slint` — the component library
- `ui/screens/{overview,cooling,performance,lighting,settings,diagnostics}.slint` — the six screens

## Decisions Made

- **Status strip under the header.** The plan's Task 2 text put it at the bottom; `01-UI-SPEC.md` fixes the reading order as header → global status → screen anchor, and a degraded warning at the bottom of a scrolled view is easy to miss.
- **Absolute positioning for the Overview grid.** Slint `GridLayout` needs compile-time `row`/`col` values, so the card grid computes `columns` from width and positions each card arithmetically at the 928/692/456px breakpoints. Verified at 1180×760 (4 columns) and 940×620 (3 columns).
- **The core formats, the UI styles.** `AppEvent` carries numbers; `apply_to_ui` turns them into unit-less strings (`"38"`, `"2450"`) and the screens append `%`/`RPM`/`°C`. That keeps typography in Slint and parsing out of it.
- **Navigation is echoed only on `ScreenChanged`.** Publishing `active-screen` on every event would fight the sidebar, which moves the moment it is clicked while the command is still in flight.
- **`AppState` starts empty, the window starts nominal.** `HardwareCapabilities::none()` is the `Default` (absence must not be fabricated), while the Slint properties default to `true` for the Phase 1 mock look; `initialize` publishes the real set synchronously before the first frame, so the window never shows a state the core disagrees with.
- **Drop, don't block, on a full command queue.** `try_send` with a debug log; a repeated refresh press is worth nothing once one is pending (T-01-03).

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 2 - Missing Critical] `action-button.slint` was not in the plan's file list**
- **Found during:** Task 1
- **Issue:** the header's primary action and the status strip's "View Diagnostics" need the same button geometry and focus ring. Duplicating it in two files would have drifted.
- **Fix:** authored `SecondaryButton`/`PrimaryButton` in one component and imported it in both places.
- **Files modified:** `ui/components/action-button.slint`, `ui/components/status-strip.slint`, `ui/components/screen-header.slint`
- **Committed in:** `cc8738e` (Task 1)

**2. [Rule 1 - Bug] Slint `GridLayout` cannot bind `row`/`col` to a computed column count**
- **Found during:** Task 3
- **Issue:** the responsive Overview grid (4 → 3 → 2 columns) needs a data-driven column count, but Slint requires `row:`/`col:` to be compile-time constants, and `x` cannot be set on a layout child.
- **Fix:** replaced the grid with a `for` loop over cards positioned absolutely from `columns`, `card-width` and the 16px gap, with breakpoints at 928/692/456px.
- **Files modified:** `ui/screens/overview.slint`
- **Verification:** screenshots at 1180×760 (4 columns) and 940×620 (3 columns), no clipping.
- **Committed in:** `6a8f355` (Task 3)

**3. [Rule 1 - Bug] A Display-size "Waiting for first reading" ellipsized inside `MetricCard`**
- **Found during:** Task 3
- **Issue:** the card's value slot uses `ValueReadout` at Display size (32px); the empty-state sentence truncated, violating the contract that a value slot never truncates.
- **Fix:** `MetricCard` renders the empty state as body-size wrapped prose in the same slot, and gained a `detail` property (used by the GPU card for its temperature and the fan card for fan 2). Height went 148px → 176px to hold the extra line.
- **Files modified:** `ui/components/metric-card.slint`, `ui/components/value-readout.slint`
- **Committed in:** `6a8f355` (Task 3)

**4. [Rule 1 - Bug] `ValueReadout` elided numeric values**
- **Found during:** Task 3
- **Issue:** `overflow: elide` on a numeric readout can silently render "24…" for "2450" — a truncated reading is a wrong reading.
- **Fix:** removed elision from the value slot; the container sizes to the text.
- **Files modified:** `ui/components/value-readout.slint`
- **Committed in:** `6a8f355` (Task 3)

**5. [Rule 1 - Bug] Slint key names and the modulo operator differ from the plan's sketch**
- **Found during:** Task 2
- **Issue:** the plan's prose said `Key.Up`/`Key.Down` and used `%`; Slint spells them `Key.UpArrow`/`Key.DownArrow` and has no `%` operator (`Math.mod` instead).
- **Fix:** used the real names and `Math.mod` for index wrapping.
- **Files modified:** `ui/components/sidebar.slint`
- **Committed in:** `13beb61` (Task 2)

**6. [Rule 2 - Missing Critical] Clippy `large_enum_variant` on `AppEvent`**
- **Found during:** Task 4
- **Issue:** `TelemetryUpdated(TelemetrySnapshot)` makes the enum 216 bytes; `-D warnings` failed the gate.
- **Fix:** allowed the lint with a justification instead of boxing — a sample is produced once per tick and moved straight through a channel, so boxing would add an allocation per sample and buy nothing. The variant payload keeps the type the plan declares for 01-03.
- **Files modified:** `src/core/events.rs`
- **Committed in:** `bcf587a` (Task 4)

**7. [Rule 2 - Missing Critical] Dead-code warnings for the 01-03 seam**
- **Found during:** Task 4
- **Issue:** `HardwareProvider`, `AppEvent::TelemetryUpdated` and `AppState::history_window` are declared contracts with no consumer until 01-03; the plan requires a warning-free build.
- **Fix:** consumed what could be consumed (`has_any_reading` now decides "waiting" vs. "N of 12", `none()` is the `Default`, `ScreenId::title` logs the navigation, `ScreenId::index` writes `active-screen`) and put `allow(dead_code, reason = "...")` on the three genuine seams.
- **Files modified:** `src/core/state.rs`, `src/core/events.rs`, `src/hardware/provider.rs`
- **Committed in:** `bcf587a` (Task 4)

**8. [Rule 2 - Missing Critical] Status strip had no action wiring**
- **Found during:** Task 4
- **Issue:** the strip could render degraded but offered no route to the reason, which the UI-SPEC's alert table requires.
- **Fix:** `show-action: root.status-degraded` and an `action` handler that navigates to Diagnostics through the normal command path.
- **Files modified:** `ui/app.slint`
- **Verification:** zero-capability run, clicking "View Diagnostics" logged `screen requested screen="Diagnostics"` and rendered the capability table.
- **Committed in:** `bcf587a` (Task 4)

---

**Total deviations:** 8 auto-fixed (4 bugs, 4 missing critical)
**Impact on plan:** No scope creep. Four deviations are places where the plan's literal text did not match the Slint API; the rest are correctness or contract requirements the plan's own must-haves imply.

## Issues Encountered

- **The plan's first session terminated mid-Task-1.** The component library and provider types were on disk but uncommitted. They were reviewed file by file and committed as-is (`cc8738e`) after `cargo check` passed; Task 2 onwards was executed in this session.
- **XTEST keyboard input silently goes nowhere under bare Xvfb without an explicit X focus.** The first keyboard verification produced no navigation and no log line. `Display.set_input_focus(window, X.RevertToParent, X.CurrentTime)` before sending keys fixed it; the second run logged the Performance navigation. There is no window manager on the Xvfb display to assign focus.
- **`xkbcommon` prints Compose-file parse errors under Xvfb** (`/usr/share/X11/locale/iso8859-1/Compose` is not valid UTF-8 in this container). Environment artifact of the minimal image, same as 01-01; the window renders normally and CI installs the full X11 stack.
- **`winit` warns about XSETTINGS and a 0mm display size under Xvfb.** Also environment-only; not application output.

## Known Stubs

| Stub | File | Reason |
|------|------|--------|
| `zenomen-command-drain` thread logs commands and only echoes navigation back as `ScreenChanged` | `src/main.rs` | Intentional seam: plan 01-03 turns it into the mock provider's command loop (`RefreshTelemetry` → sample) without touching the UI. |
| `app.initialize(HardwareCapabilities::all_supported())` declares the capability set instead of detecting it | `src/main.rs` | Phase 1 runs on mock hardware; 01-03 sends `AppEvent::CapabilitiesLoaded` from the provider instead. |
| All telemetry properties stay empty, so every screen shows "Waiting for first reading" | `ui/app.slint`, `src/app.rs` | No provider exists yet. The formatting path is implemented and unit-tested; only the source of samples is missing. |
| `sparkline.slint` / `telemetry-chart.slint` render a "Waiting for first reading" placeholder instead of a plotted series | `ui/components/sparkline.slint`, `ui/components/telemetry-chart.slint` | Chart geometry arrives with the history buffer in 01-03 (MON-11); `AppState::history_window` already fixes the 120-sample bound. |
| `zone-colors` is never populated, so Lighting shows its empty state | `src/app.rs`, `ui/screens/lighting.slint` | `TelemetrySnapshot` carries a profile name only; per-zone colours belong to the RGB phase (04). |
| `AppCommand::SetHistoryWindow` is handled by logging | `src/main.rs` | Declared contract; the Settings control that emits it lands with the settings persistence phase. |

No stub prevents this plan's goal: the shell, the navigation and the bridge are complete and were exercised end to end. Each stub is the documented input to a later plan, and the UI renders an honest empty state rather than a fake value in the meantime.

## Known Gaps

- **FOUND-02/04/05 are verified on the mock-nominal and zero-capability paths only.** Real capability detection (a machine that genuinely lacks fan control, or genuinely has it) is Phase 2; the screens' gating logic is proven, the detection is not yet written.
- **Focus order between the sidebar and the content area is one-directional in practice.** Escape returns focus to the sidebar and arrow keys work there, but there is no `Tab` cycle from the content back into the sidebar without Escape — acceptable for a shell with no focusable content controls yet, and revisited when the controls arrive.
- **The status strip's action button sits immediately after the text rather than at the right edge.** Functional and legible, but the UI-SPEC's right-aligned action is not reproduced; a later visual pass can right-align it.

## User Setup Required

None. No new dependencies, no service configuration, no environment variables. The app still builds and runs with the toolchain and X11/GL packages already used by 01-01.

## Next Phase Readiness

- **Ready for 01-03:** the bridge is the whole contract. Implement `HardwareProvider` for the mock, replace `initialize(all_supported())` with the provider's `capabilities()`, and send `AppEvent::TelemetryUpdated` from the drain thread on a 1 Hz tick — every screen already renders whatever arrives, and the seven state tests describe the expected behaviour.
- **Ready for 02:** Linux telemetry replaces the mock behind the same trait; `TelemetrySnapshot` already carries the fields the UI consumes (and `reported_sensor_count` measures absence instead of guessing it).
- **Watch:** `apply_to_ui` formats numbers with `{:.0}`; if a later phase wants per-sensor precision, that is the single place to change, and the screens need no edit.

## Self-Check: PASSED

Created files verified present (`[ -f ... ]`):

```
FOUND: src/core/mod.rs
FOUND: src/core/events.rs
FOUND: src/core/state.rs
FOUND: src/app.rs
FOUND: ui/components/sidebar.slint
FOUND: ui/components/unavailable-panel.slint
FOUND: ui/components/metric-card.slint
FOUND: ui/components/status-strip.slint
FOUND: ui/screens/overview.slint
FOUND: ui/screens/cooling.slint
FOUND: ui/screens/performance.slint
FOUND: ui/screens/lighting.slint
FOUND: ui/screens/settings.slint
FOUND: ui/screens/diagnostics.slint
FOUND: .planning/phases/01-foundation-gui-shell-on-mock-hardware/01-02-SUMMARY.md
```

Task commits verified reachable from HEAD (`git merge-base --is-ancestor`):

```
FOUND: cc8738e
FOUND: 13beb61
FOUND: 6a8f355
FOUND: bcf587a
```

Gate evidence:

```
$ cargo check
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.22s      (exit 0, no warnings)

$ cargo fmt --all -- --check
FMT_RC=0

$ cargo clippy --all-targets --all-features -- -D warnings
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.30s      (exit 0, no warnings)

$ cargo test
running 7 tests
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

$ RUST_LOG=debug xvfb-run --auto-servernum ./target/debug/zenomen
 INFO zenomen: starting ZenOmen version="0.1.0"
DEBUG zenomen: screen requested screen="Overview"
DEBUG zenomen: screen requested screen="Cooling"
DEBUG zenomen: screen requested screen="Performance"
DEBUG zenomen: screen requested screen="Lighting"
DEBUG zenomen: screen requested screen="Settings"
DEBUG zenomen: screen requested screen="Diagnostics"
DEBUG zenomen: screen requested screen="Performance"      (keyboard: Down Down Return)

$ xvfb-run ... (temporary HardwareCapabilities::none() build)
DEBUG zenomen: screen requested screen="Diagnostics"      (status strip action)
window: 1180x760 -> configure 940x620 -> 940x620, Overview reflowed 4 -> 3 columns
```

---
*Phase: 01-foundation-gui-shell-on-mock-hardware*
*Completed: 2026-11-01*
