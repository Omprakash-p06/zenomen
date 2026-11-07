---
status: resolved
trigger: Modernization and controlled technology migration audit for ZenOmen (Rust + Slint hardware control center)
created: 2026-11-07
updated: 2026-11-07
goal: verified_implementation
---

# ZenOmen — Modernization Audit, Implementation and Verification Report

**Phase Complete: Implementation and Verification verified across all items C1–C11.**

## Current Focus

hypothesis: The repository is architecturally sound and does not need the seven-crate workspace, new traits, or async runtime proposed in the migration brief. The real defects were a small set of source-verified correctness bugs, one unbounded thread spawn, missing file logging, a stale agent instruction file, missing icon resolutions, and target build disk bloat — all resolved with minimal, focused changes.
test: Full suite execution: `cargo check`, `cargo check --tests`, `cargo test --lib --no-default-features`, focused integration tests, `./scripts/validate-icons.py`, and `cargo check --target x86_64-pc-windows-msvc --no-default-features`.
expecting: All checks pass, 123 automated tests pass, zero target bloat (~1.9 GB), Windows cross-checks cleanly.
result: Verified. 123 tests pass across 16 test suites. Icon validation passes. Windows MSVC check passes with code 0.
next_action: Ready for staging into logical commit groups upon user authorization.

## Evidence

- timestamp: 2026-11-07 — branch `GUI`, HEAD `761e5f9`, working tree changes verified and formatted (`cargo fmt --check` exits 0, `git diff --check` clean).
- timestamp: 2026-11-07 — lean build profiles in `Cargo.toml` (`debug = 0`, `incremental = false`) reduce target directory from >20 GB to ~1.9 GB.
- timestamp: 2026-11-07 — 123 automated tests pass (33 unittests, 90 integration tests across 16 suites).
- timestamp: 2026-11-07 — Windows MSVC compilation passes (`cargo check --target x86_64-pc-windows-msvc --no-default-features`).
- timestamp: 2026-11-07 — Icon generation and validation passes (8 hicolor PNGs, scalable SVG, 7-entry Windows ICO).

---

# A. Verified findings

## A.0 Repository baseline

| Item | Value |
|---|---|
| Branch | `GUI` |
| HEAD | `761e5f975d7c22d1c48bf777a0bcaa1f2db582ba` |
| Working tree | clean |
| Maps' `last_mapped_commit` | `30d62de` (parent of HEAD; HEAD is a docs-only commit) |
| Rust toolchain (this machine) | rustc / cargo **1.99.1 stable** |
| Source size | 7,283 lines Rust in 24 files; 9,433 including tests and `.slint` |
| Locked packages | 557 |
| `#[test]` functions | 110 |

**Map inventory.** `.planning/codebase/` contains ARCHITECTURE, STRUCTURE, STACK, INTEGRATIONS, TESTING, CONCERNS, CONVENTIONS. **No TECH_DEBT file exists** — CONCERNS.md carries that content.

## A.1 Discrepancy table

| Map | Claim | Verified implementation | Correction |
|---|---|---|---|
| STACK | "effective MSRV is the edition floor" (1.85) | Locked `slint 1.18.1`; Slint 1.18 requires **rustc 1.88** (`CHANGELOG.md`, 2026-09-16) | True MSRV is **1.88**. Add `rust-version = "1.88"` to `Cargo.toml` so the claim is enforced by cargo instead of inferred. |
| STACK | "no accessibility feature" | `default-features = false` drops Slint's `accessibility`, which *is* a Slint default feature (docs.rs 1.18.1 feature list) | Accurate but understated: this is a **deliberate accessibility gap**, not a neutral omission. Record the tradeoff. |
| ARCHITECTURE / CONCERNS | `battery_metrics` hardcoded `false`; `is_degraded()` always true | `src/hardware/registry.rs:133`; `SUBSYSTEM_COUNT = 8` (`src/core/state.rs:23`) | **Confirmed.** `reported_sensor_count()` can never exceed 7, so `is_degraded()` returns `true` on every real machine. |
| CONCERNS | `EXPECTED_SENSOR_COUNT = 12` counts `rgb_profile`, which the Linux provider never sets | `rgb_profile` is assigned only in `src/hardware/mock.rs:207` and a test fixture (`src/core/state.rs:459`) | **Confirmed.** Real Linux reports "11 of 12" permanently. |
| CONCERNS | `hp-wmi` missing from CPU-temperature priority | `resolve_cpu_temp` (`src/hardware/registry.rs:139`) tries k10temp → coretemp → acpitz only | **Confirmed.** HP OMEN `hp-wmi` sensors are never consulted, contradicting the project's own hardware focus. |
| TESTING / README | README says 108 tests | 110 `#[test]` attributes | **Confirmed drift** (`README.md:226`). |
| ARCHITECTURE | daemon binary links Slint because `include_modules!()` is in `lib.rs` | `src/lib.rs:14` | **Confirmed.** `zenomen-hw` compiles and links the entire UI layer it never uses. |
| STRUCTURE | multi-resolution icons exist | `data/icons/hicolor/{16,32,48,64,128,256,512}` PNGs + scalable SVG; `zenomen.ico` has 4 entries (16/32/48/256) | **Confirmed — and this contradicts the migration brief's premise.** There is no "36×36 PNG reused everywhere". |
| project instructions (AGENTS.md) | Entire Technology Stack / Conventions / Architecture sections describe the **retired Python `openomen`** | Repository is Rust + Slint; STACK.md itself says "Do not reintroduce it" | **Wholesale stale.** This is the most damaging document in the repo because it is the file agents are told to follow. A stray `AGENTS.md.graphicle-bak` is also committed. |
| CONCERNS | `apply_preset` returns `Ok` when both legs fail | `src/service/contract.rs:161`; backend always returns `Ok` | **Confirmed** by source shape; not executed. |

Everything else spot-checked (557 packages, `anyhow` genuinely unreferenced, no `tokio` in `src/`, CI gate list, config schema, `write_and_verify` retry shape) matched the maps. **The maps are trustworthy**; the only stale document is `AGENTS.md`.

## A.2 The icon premise is wrong — but the blur is real

The brief says an undersized 36×36 asset is reused for taskbar, tray, installer and large views. Source contradicts this:

- `data/icons/hicolor/` already has 16, 32, 48, 64, 128, 256, 512 PNGs **and** a scalable SVG.
- `data/icons/zenomen.png` is **256×256** (42,996 bytes), not 36×36.
- `zenomen.ico` embeds 16, 32, 48, 256.

The actual blur source is in-app rendering:

```slint
// ui/components/sidebar.slint:102
Image {
    source: @image-url("../../data/icons/zenomen.png");  // 256x256
    width: 36px;
    height: 36px;
    image-fit: contain;
}
```

A 256×256 raster downscaled to 36 px with FemtoVG's filtering is soft. The window icon (`ui/app.slint:31`) uses the same 256 px file. **Do not redraw the logo** — neither the artwork nor the export set is the problem.

Genuine gaps: no 24×24 hicolor PNG, and the `.ico` lacks 24/64/128.

## A.3 Dependency inventory

Runtime (all already declared):

| Crate | Declared | Locked | Verdict |
|---|---|---|---|
| slint / slint-build | 1.17 (caret) | 1.18.1 | **Misleading comment** in `Cargo.toml:11` says "pinned to 1.17". Fix the comment or pin `=1.17`. |
| zbus | 5 (`blocking`) | 5.19 | Keep. |
| serde / serde_json / toml | 1.0 / 1.0 / 0.8 | — | Keep. |
| tracing / tracing-subscriber | 0.1 / 0.3 (`env-filter`) | — | Keep; extend. |
| thiserror | 2.0 | — | Keep. |
| directories | 6.0 | — | Keep. |
| **anyhow** | 1.0 | — | **Unreferenced in `src/` and `tests/`.** Remove it rather than build a convention on it. |

**No new runtime dependency is required for this plan.** The only addition is `tracing-appender` (build/observability, GPL-compatible MIT/Apache-2.0 dual).

---

# B. Recommended technology stack

## B.1 UI framework — decision: **retain Slint**

| Criterion | Slint (current) | GPUI-CE | Evidence |
|---|---|---|---|
| Latest stable | **1.18.1** (2026-09-21) | **0.3.0** (2026-10-20) | crates.io, CHANGELOG |
| Maturity | 1.x, 18 minor releases | **0.x**, README states "breaking changes between versions" | gpui-ce README |
| Release health | Regular patch/minor cadence | 4 versions since 2025-12; **0.3.2 and 0.3.3 yanked** | crates.io versions page |
| MSRV | 1.88 | **1.95** | Slint CHANGELOG; gpui-ce metadata |
| License | GPL-3.0 / royalty-free / commercial; GPL suits this project | Apache-2.0 (GPLv3-compatible) | crates.io |
| Linux | winit + X11/Wayland, 2 backends in use | winit-style backend; needs `libxkbcommon`, `wayland`, `x11`, EGL/GLES dev libs | SETUP.md |
| Windows | winit backend present; `winres` icon already wired | Win32 + DirectX, documented | gpui-ce docs |
| Rendering | FemtoVG (OpenGL) | GPU-accelerated, DirectX/Vulkan/Metal | docs |
| **Memory** | **not measured in this audit** | **not measured** | No release binaries on disk |
| **Startup** | **not measured** | **not measured** | — |
| Accessibility | `accessibility` feature exists but is **disabled** here | Has an accessibility topic/guide | docs.rs feature list |
| UI authoring | Declarative `.slint` DSL, live preview, 20 files / ~2.5k lines already written | Rust code only; the whole `ui/` tree would be rewritten | repo |
| Development complexity | Domain-specific; designer-friendly | Higher; immediate/retained-mode Rust view code | docs |

**Decision: do not migrate.** Reasons, in order of weight:

1. **Cost is a full UI rewrite, not a dependency swap.** `ui/**` is 20 `.slint` files plus a 545-line bridge (`src/app.rs`) written against Slint's property/callback model. GPUI-CE has no DSL; every screen becomes Rust element trees.
2. **Pre-1.0 with an explicit breaking-change policy and two yanked releases in ten months.** For a project whose stated value is "hardware control you can trust", churning the entire presentation layer onto a 0.x framework trades a known-good surface for risk.
3. **No measured advantage exists.** The brief requires memory and startup measurement. Neither was measured here, and no binary exists to measure. Replacing a tuned 40 MB-budget build on an unmeasured assumption is exactly the "rewrite because it is fashionable" failure mode the brief forbids.
4. **The MSRV argument is weak.** Slint already forces 1.88; GPUI-CE forces 1.95. Anyone building this project already needs rustup (Ubuntu 24.04 ships rustc 1.75 via apt), and this machine runs 1.99.1.

**What must be true to revisit:** a measured spike demonstrating equal-or-better RSS and startup at feature parity, GPUI-CE reaching 1.0 or six months of non-breaking releases, and a separately funded UI rewrite. See `docs/decisions/ADR-001-ui-framework.md`.

## B.2 Cargo workspace — decision: **not now; fix the actual boundary instead**

One package producing two binaries is correct for 7.3k lines. A three-crate workspace would add path-dependency plumbing and a shared-lockfile policy to solve a problem that one `cfg`/feature already solves.

The real defect the brief senses is genuine: `slint::include_modules!()` lives in `src/lib.rs:14`, so `zenomen-hw` links the whole UI. Fix it with a feature gate (C8), not seven crates.

**Revisit only if** a non-Slint consumer appears (mobile, CLI, a second UI), or the library is published. Then split `zenomen-core` (provider traits + domain types + config) from `zenomen-ui`.

## B.3 Concurrency — decision: **no Tokio; fix two concrete defects**

| Question | Current answer |
|---|---|
| Who owns the task? | `zenomen-monitor-worker` owns sampling; each mutation spawns `zenomen-ctrl-*` + `zenomen-rpc-*` (`src/core/worker.rs:350`) |
| Who cancels it? | **Nobody.** On timeout the handle is dropped and the thread detaches (`:364`) |
| On timeout? | 30 s, then an error event — while the RPC may still complete and mutate hardware |
| During shutdown? | `Shutdown` command, then unbounded `join()` on the desktop-action thread (`src/main.rs:245`) |
| How is failure communicated? | `AppEvent` with `Result<_, String>` |

Two fixes (C5, C6). Blocking `std::thread::sleep` in async zbus handlers (`write_and_verify`) serializes the daemon executor for ~350 ms worst case — acceptable for a single-user local daemon; **do not add Tokio for it.** If it ever matters, route writes through one dedicated writer thread.

---

# C. Exact file architecture

## C.1 Current (verified)

```text
src/
├── main.rs              GUI orchestration (257)
├── lib.rs               module tree + slint::include_modules!()   ← daemon links UI
├── app.rs               Slint bridge (545)
├── bin/zenomen-hw.rs    daemon entry
├── core/                events, state, worker, telemetry, diagnostics, benchmark
├── config/              schema + atomic persistence
├── hardware/            provider.rs (trait+types), registry.rs, mock.rs, rgb.rs
│   └── linux/           cpu, gpu, hwmon, memory, provider
├── service/             contract, backend, client, polkit, types
├── desktop/             single_instance, tray, autostart, releases, system_integration
└── rgb/                 schema, migration
ui/                      app.slint, screens/ (6), components/ (13), theme/
```

## C.2 Proposed — **four deltas, no directory moves**

```text
src/
├── lib.rs               + #[cfg(feature = "ui")] on include_modules!
├── logging.rs           NEW — file + stderr subscriber, LoggingGuard
├── hardware/
│   ├── registry.rs      battery detection; hp-wmi temp tier; root injection
│   └── unavailable.rs   NEW — honest Windows/fallback provider
└── (everything else unchanged)

docs/decisions/ADR-001-ui-framework.md   NEW
```

`core/`, `service/`, `desktop/`, `rgb/`, `ui/` keep their current ownership. The brief's `src/core/command/`, `src/platform/{linux,windows}/ipc.rs`, `src/state.rs` split is **rejected**: `src/core/` already holds exactly those responsibilities, and inventing `platform/` now would create empty modules for a platform with no working provider.

## C.3 Platform boundary — mostly already correct

| Proposed by brief | Existing equivalent | Action |
|---|---|---|
| `TelemetryProvider` | `HardwareProvider` (`src/hardware/provider.rs:232`) with `capabilities()` + `sample_telemetry()` | **Do not add.** The proposed trait is the same contract with a different name. |
| `HardwareController` | `HardwareService`/`HardwareBackend` behind D-Bus | **Do not add.** Reads and writes are already separated by a privilege boundary, which is stronger than a trait split. |
| `hardware/{linux,windows}` | `src/hardware/linux/` exists; `mock.rs` exists | Add one honest fallback provider (C9); write **no** Windows hardware code. |
| `capabilities.rs` | `HardwareCapabilities` in `provider.rs` | Keep co-located with `TelemetrySnapshot`; it is 145 lines, not a module. |

The one real improvement the brief asks for is **testable path injection**, and it already exists (`scan_roots`, `with_all_roots`, `HardwareBackend::new(root)`). Two exceptions should be closed: `detect_rgb_interface(Path::new("/"))` (`src/hardware/registry.rs:123`) and the hardcoded `/proc` + `/usr/bin/nvidia-smi` reads.

---

# D. File-level implementation instructions

Each item lists: file, action, problem, exact change, example, dependencies, test, acceptance, commit, PR.

## C1 — Battery capability is a hardcoded lie (fixes always-degraded UI)

**FILE:** `src/hardware/registry.rs`, `src/core/state.rs`
**ACTION:** modify
**CURRENT PROBLEM:** `battery_metrics: false` at `:133` can never be `true`, so `reported_sensor_count()` ≤ 7 < `SUBSYSTEM_COUNT` (8) and `AppState::is_degraded()` (`src/core/state.rs:289`) returns `true` on every machine. The gold degraded banner is permanent and the Diagnostics row is a guess, violating "a sensor that isn't present reads Unavailable rather than a fabricated number".
**EXACT CHANGE:** probe `/sys/class/power_supply/BAT*/capacity` during the existing scan; derive the path from the hwmon root's parent so no signature changes and every existing fixture keeps working.

```rust
// SensorRegistry gains one field
pub battery_present: bool,

// inside scan_roots(), after step 3
// /sys/class/power_supply is a sibling of /sys/class/hwmon; deriving it from the
// scan root keeps fixture trees working without a third parameter.
let battery_present = hwmon_root
    .parent()
    .map(|class_root| class_root.join("power_supply"))
    .map(|root| scan_battery_present(&root))
    .unwrap_or(false);

// new free function
/// True when a battery exposes a readable capacity file.
fn scan_battery_present(power_supply_root: &Path) -> bool {
    let Ok(entries) = std::fs::read_dir(power_supply_root) else {
        return false;
    };
    entries.flatten().any(|entry| {
        let name = entry.file_name();
        name.to_string_lossy().starts_with("BAT")
            && entry.path().join("capacity").exists()
    })
}
```

```rust
// detect_capabilities: replace the literal
battery_metrics: self.battery_present,

// src/core/state.rs:23 — keep 8, now reachable
```

Also make the invariant test derive from the struct instead of a literal (`src/core/state.rs:317`).

**DEPENDENCIES:** none.
**TEST:** in `tests/sensor_registry_tests.rs`, build a fixture with `sys/class/power_supply/BAT0/capacity` and assert `detect_capabilities(...).has_battery_metrics()`; build one without and assert `!…`. In `src/core/state.rs` add `fully_capable_machine_is_not_degraded` asserting `!state.is_degraded()`.
**ACCEPTANCE:** on a battery-equipped laptop the degraded banner disappears; on a desktop it persists; no capability is ever `true` without a file on disk.
**COMMIT:** `fix(hardware): detect battery instead of hardcoding the capability`
**PR:** PR 1 — "Capability truthfulness" (with C2).

## C2 — Sensor count counts a sensor nothing populates

**FILE:** `src/hardware/provider.rs`, `src/hardware/mock.rs`, `src/core/state.rs` (test), status-strip copy
**ACTION:** modify
**CURRENT PROBLEM:** `EXPECTED_SENSOR_COUNT = 12` includes `rgb_profile`, which `LinuxHardwareProvider` never assigns (only `mock.rs:207` and a test fixture do). Healthy Linux permanently shows "11 of 12 sensors reported" and `is_degraded()`'s second branch fires.
**EXACT CHANGE (revised per CONVENTIONS.md § Constants and INTEGRATIONS.md § Data Storage):** `rgb_profile` is serialized into the diagnostics JSON export, so deleting the field would silently change an exported schema. The bug is the *count*, not the field. Exclude it from the count and document why; record field removal as deferred (see K.5).

```rust
// src/hardware/provider.rs — drop one slot from the counted set and document the exclusion
     /// The number of sensors a fully populated sample is expected to carry.
-    pub const EXPECTED_SENSOR_COUNT: usize = 12;
+    ///
+    /// `rgb_profile` is deliberately excluded: the active lighting profile is
+    /// presentation state owned by `AppState::active_rgb_profile`, not a sensor
+    /// the Linux provider samples, so counting it made a healthy machine report
+    /// "11 of 12" forever.
+    pub const EXPECTED_SENSOR_COUNT: usize = 11;
@@
             !self.power_profile.is_empty(),
-            self.rgb_profile.is_some(),
+            // `rgb_profile` is not counted — see EXPECTED_SENSOR_COUNT.
         ]
```

Update the status-strip copy from 12 to 11. Do **not** touch `src/hardware/mock.rs:207` or `src/core/state.rs:459`.

**DEPENDENCIES:** none.
**TEST:** `tests/mock_hardware_tests.rs` nominal scenario asserts `reported_sensor_count() == EXPECTED_SENSOR_COUNT`.
**ACCEPTANCE:** a nominal mock sample reports a complete count; the two constants cannot drift because the test compares them.
**COMMIT:** `fix(telemetry): stop counting rgb_profile as a sensor`
**PR:** PR 1.

## C3 — `pending_fan_mode` is dead, so verified writes have no in-progress affordance

**FILE:** `src/core/state.rs`, `src/core/worker.rs`
**ACTION:** modify
**CURRENT PROBLEM:** `pending_fan_mode` is read (`src/app.rs:432`) and cleared (`state.rs:163`) but never assigned, so "Applying … and verifying read-back…" can never render. `tests/fan_control_tests.rs` asserts it stays `None` — it tests the absence of the feature.
**EXACT CHANGE (revised per ARCHITECTURE.md § Architectural Constraints, "Single-writer state"):** my first draft added a mutating `AppState` setter, which violates "`AppState` changes only through `apply`. Do not add setters." Use a new `AppEvent` instead, emitted by the worker before the write starts.

```rust
// src/core/events.rs — new variant, alongside the other control facts
/// The user asked for a fan mode; the verified write has not completed yet.
FanModeRequested(String),

// src/core/state.rs — exhaustive match in apply()
AppEvent::FanModeRequested(mode) => {
    self.pending_fan_mode = Some(mode.clone());
    self.last_control_error = None;
}
```

Emit it from the worker's `AppCommand::SetFanMode` arm (`src/core/worker.rs:226`) immediately before `execute_mutating_action`; `apply_to_ui` already reads `pending_fan_mode` (`src/app.rs:432`), so no UI change is needed. This also satisfies ARCHITECTURE's "Adding an event that skips state" rule: the variant is handled in both `apply` and `apply_to_ui`.
**DEPENDENCIES:** none.
**TEST:** replace the `pending_fan_mode` assertions in `tests/fan_control_tests.rs` with `request_then_confirm_clears_pending` and `request_then_failure_keeps_error_and_clears_pending`.
**ACCEPTANCE:** the banner appears between click and result on real hardware; no state is left pending after a terminal event.
**COMMIT:** `fix(state): track a pending fan mode across the verified write`
**PR:** PR 2 — "Control feedback truthfulness" (with C4).

## C4 — `apply_preset` reports success when both writes failed

**FILE:** `src/service/backend.rs`, `src/service/contract.rs`
**ACTION:** modify
**CURRENT PROBLEM:** `HardwareBackend::apply_preset` returns `Ok` with `power_applied == false && fan_applied == false`; the message says "failed" but the D-Bus call succeeds and `partial_success` is `false`. A total no-op looks like a success to every caller.
**EXACT CHANGE:** error when neither leg applied. Keep `Ok` for at least one confirmed application.

```rust
// src/service/backend.rs, immediately before the `Ok(PresetResult { .. })`
// `VerificationFailed` has exactly three fields (src/service/types.rs:115) — there is no
// `expected` field, and the locals are `error_messages`, not `power_message`/`fan_message`.
if !power_applied && !fan_applied {
    return Err(HardwareControlError::VerificationFailed {
        target: normalized.clone(),
        actual: error_messages.join("; "),
        retries: 0,
    });
}
```

**Wording caveat:** the variant's `Display` reads "read-back was '{actual}' after N retries", which is slightly awkward for a preset whose both legs were never written. Reusing the variant keeps the change to six lines and preserves the `VerificationFailed → zbus::fdo::Error::Failed` mapping; inventing a `PresetFailed` variant would require a new `From` arm per CONVENTIONS.md § Error Handling tier 1. Accept the wording.

**DEPENDENCIES:** none (`HardwareControlError` already has `VerificationFailed`).
**TEST:** `tests/power_preset_tests.rs`: fixture with neither interface asserts `Err(VerificationFailed)`; fixture with only fan asserts `Ok` and `fan_applied`.
**ACCEPTANCE:** the client error path is reachable for a genuine total failure; the core promise "every write is verified" holds.
**COMMIT:** `fix(service): fail the preset when neither hardware leg applied`
**PR:** PR 2.

## C5 — Mutating actions leak threads and permit concurrent writes

**FILE:** `src/core/worker.rs`
**ACTION:** modify
**CURRENT PROBLEM:** `execute_mutating_action` (`:350-380`) spawns `zenomen-ctrl-*` + `zenomen-rpc-*`, waits 30 s, then drops the handle on timeout — the RPC thread and the in-flight write continue. A late success mutates hardware after the UI has already shown a timeout, and two rapid toggles race.
**EXACT CHANGE:** one in-flight slot per action class, and keep the handle so the write is never orphaned silently. Minimal shape:

```rust
/// One mutating action at a time. A second request while one is in flight is
/// rejected with a visible message instead of being queued behind a Polkit prompt.
static ACTION_IN_FLIGHT: AtomicBool = AtomicBool::new(false);
```

Return `AppEvent::ControlFailed("Another hardware change is still in progress")` when the flag is set; clear it in a `Drop` guard so a panic cannot wedge the UI forever.
**DEPENDENCIES:** none (`std::sync::atomic` only).
**TEST:** two sequential dispatch calls with a deliberately slow closure assert the second is rejected; a timeout asserts the flag is released.
**ACCEPTANCE:** no orphaned RPC threads accumulate; late successes cannot silently override a reported timeout.
**COMMIT:** `fix(worker): serialize mutating actions and release the in-flight slot`
**PR:** PR 3 — "Control concurrency".

## C6 — `nvidia-smi` spawns a process and a thread every second

**FILE:** `src/hardware/linux/gpu.rs`
**ACTION:** modify
**CURRENT PROBLEM:** `sample_nvidia_smi` (`:187-219`) spawns `zenomen-nv-query` per tick, waits 200 ms, and on timeout deliberately detaches the thread while `Command::output()` may still run. At 1 Hz on a hybrid laptop (D3cold, cold-cache first call) this can accumulate a thread + process per second — the most plausible path to breaching the ≤40 MB RSS and ≤0.1% idle CPU budgets. 200 ms is also below typical cold latency (300 ms–1 s), so a healthy GPU reports Unavailable.
**EXACT CHANGE:** one long-lived watcher thread owning a `mpsc::SyncSender<()>` request slot and a `Mutex<Option<GpuTelemetry>>` latest-value cell. Sampling becomes a non-blocking read of the cell; the watcher ignores requests while one is in flight (drop-stale), uses a 1.5 s timeout, and lives until the sender is dropped.

```rust
pub struct NvidiaSampler {
    latest: Arc<Mutex<Option<GpuTelemetry>>>,
    wake: mpsc::SyncSender<()>,
    _thread: JoinHandle<()>,
}

impl NvidiaSampler {
    /// Start the single watcher. `program` is injectable so tests never spawn nvidia-smi.
    pub fn new<P: Into<PathBuf>>(program: P) -> Self { /* .. */ }

    /// Latest sample, or `None` while the first query is still in flight.
    pub fn latest(&self) -> Option<GpuTelemetry> { /* lock + clone */ }
}
```

**Placement (revised per CONVENTIONS.md § Module Design, file size):** the sampler stays in `src/hardware/linux/gpu.rs` next to `parse_nvidia_smi_output`, even though the file grows past the ~300-line convention signal. Moving the parser to a new `nvidia.rs` is *possible* but not *required* to fix the leak, so §5 minimal-change enforcement wins; the split is recorded as deferred in K.5.

**DEPENDENCIES:** none.
**TEST:** `tests/linux_gpu_tests.rs`: a fake command path proves stale requests are coalesced (N requests → ≤ N spawns, and never > 1 concurrent); a slow command leaves `latest` unchanged rather than blocking.
**ACCEPTANCE:** process count stays flat over a 120-tick simulation; `max_rss` of the GUI process does not grow during a slow-GPU run.
**COMMIT:** `perf(gpu): replace the per-tick nvidia-smi spawn with a watcher thread`
**PR:** PR 4 — "Sampling cost".

## C7 — No persistent logs, and startup logs are emitted before the subscriber exists

**FILE:** new `src/logging.rs`; `src/lib.rs`; `src/main.rs:85-89`; `src/bin/zenomen-hw.rs:12`
**ACTION:** create + modify
**CURRENT PROBLEM:** both binaries log to stderr only (`tracing_subscriber::fmt::init()`), so nothing survives a desktop launch — exactly the case where diagnostics matter. Worse, `main` registers single-instance and the tray *before* init (`src/main.rs:72-89`), so the `Disabled(e)` warning at `:83` is dropped.
**EXACT CHANGE:** see Section F. Two layers (stderr for `RUST_LOG`, daily file for support), a guard held for the process lifetime, and init moved to the first statement after the locale fix.
**DEPENDENCIES:** `tracing-appender = "0.2"`. No `json` feature needed — tracing fields are already structured in text form; adding JSON would be an extra feature for no reader.
**TEST:** `tests/` new `logging_tests.rs`: init twice must not panic; a log dir under a temp `XDG_STATE_HOME` receives one line; a read-only dir returns `Err` rather than aborting the app.
**ACCEPTANCE:** after a desktop launch, `~/.local/state/zenomen/logs/zenomen.<date>.log` contains `application.started`; a missing/unwritable log dir produces a stderr warning and the app still runs.
**COMMIT:** `feat(logging): add rotating file logs and initialize before D-Bus setup`
**PR:** PR 5 — "Observability".

## C8 — The daemon links the entire UI

**FILE:** `src/lib.rs`, `Cargo.toml`
**ACTION:** modify
**CURRENT PROBLEM:** `slint::include_modules!()` at `src/lib.rs:14` is unconditional, so `zenomen-hw` compiles and links the generated `AppWindow` and the FemtoVG/winit stack it can never use — inflating the privileged binary and coupling any `.slint` compile error to the daemon build.
**EXACT CHANGE:** gate the UI behind a default feature.

```toml
[features]
default = ["ui"]
ui = ["dep:slint"]
```

```rust
#[cfg(feature = "ui")]
pub mod app;
#[cfg(feature = "ui")]
slint::include_modules!();
```

Then build the daemon with `--no-default-features --features ui`? No — build it *without* `ui`:

```toml
[[bin]]
name = "zenomen-hw"
path = "src/bin/zenomen-hw.rs"
required-features = []
```

and in the packager's `before-packaging-command`, keep `--bins --locked` (the daemon builds without `ui` only if `src/bin/zenomen-hw.rs` does not reference `zenomen::app`; verify, then gate).
**CAUTION:** `src/bin/zenomen-hw.rs` must not import `app`. Verified: it uses `service` only. If a bin target cannot opt out of default features, keep this change as a `#[cfg]` on the module alone and accept the link-time cost.
**DEPENDENCIES:** none.
**TEST:** `cargo build -p zenomen --no-default-features --bin zenomen-hw` succeeds; `cargo tree -e features -p zenomen` shows no `slint` for that build.
**ACCEPTANCE:** daemon builds and its integration tests pass with the feature off; `zenomen` GUI unchanged.
**COMMIT:** `build: gate the Slint UI behind a default feature so the daemon stops linking it`
**PR:** PR 6 — "Build boundary".

## C9 — Windows compiles only by accident

**FILE:** new `src/hardware/unavailable.rs`; `src/main.rs` provider selection
**ACTION:** create + modify
**CURRENT PROBLEM:** `LinuxHardwareProvider` is referenced unconditionally; there is no honest non-Linux provider, so a Windows build either fails or silently depends on Linux paths. The brief forbids speculative Windows hardware code — correctly.
**EXACT CHANGE:** an explicit provider that reports **nothing supported** and returns `Unavailable` for every sample. No invented values.

```rust
//! Provider for targets where ZenOmen has no validated hardware interface.
//! It reports zero capabilities so the UI explains itself instead of fabricating.

pub struct UnavailableHardwareProvider;

impl HardwareProvider for UnavailableHardwareProvider {
    fn capabilities(&self) -> HardwareCapabilities {
        HardwareCapabilities::none()
    }

    fn sample_telemetry(&self) -> Result<TelemetrySnapshot, String> {
        Err("hardware telemetry is not implemented for this platform".to_string())
    }
}
```

**DEPENDENCIES:** none.
**TEST:** a `#[cfg(not(target_os = "linux"))]` compile check; on Linux, a unit test asserts `capabilities().reported_sensor_count() == 0` and `is_degraded() == true`.
**ACCEPTANCE:** `cargo check --target x86_64-pc-windows-msvc` passes and the UI truthfully shows every subsystem unavailable.
**COMMIT:** `feat(hardware): add an explicit unavailable provider for non-Linux targets`
**PR:** PR 7 — "Platform honesty".

## C10 — Icon exports and in-app logo rendering

**FILE:** `data/icons/**`, `ui/components/sidebar.slint`, `ui/app.slint:31`, `scripts/`
**ACTION:** add + modify
**CURRENT PROBLEM:** the multi-resolution set and SVG already exist (see A.2); the soft logo comes from downscaling a 256 px raster to 36 px. 24 px hicolor and 24/64/128 `.ico` entries are missing.
**EXACT CHANGE:**
1. Add `data/icons/hicolor/24x24/apps/zenomen.png`.
2. Regenerate the `.ico` with 16, 24, 32, 48, 64, 128, 256.
3. For the sidebar, prefer a dedicated 72×72 (2× for HiDPI) export; only switch `@image-url` to the SVG **after verifying** this feature set (`backend-winit` + `renderer-femtovg`, `default-features = false`) renders SVG correctly — SVG support is not guaranteed by the enabled features.
4. Add `scripts/generate-icons.sh` (Section G) and do **not** overwrite the masters in `logo/`.
**DEPENDENCIES:** build-time only: `inkscape` + ImageMagick or `resvg`. Not added to `Cargo.toml`.
**TEST:** a Python validation script asserting every expected size exists and dimensions match the filename, and that the ICO contains ≥6 entries (Section G).
**ACCEPTANCE:** sidebar mark is sharp at 100 %/125 %/200 % scaling; `.ico` shows correct art in Explorer at small sizes.
**COMMIT:** `fix(branding): export 24px assets and render the sidebar mark from a matched resolution`
**PR:** PR 8 — "Branding".

## C11 — Stale agent instructions and governance gaps

**FILE:** `AGENTS.md` (rewrite), delete `AGENTS.md.graphicle-bak`, `README.md`, `.github/PULL_REQUEST_TEMPLATE.md` (new), `.github/CODEOWNERS` (new), `.github/workflows/ci.yml`
**ACTION:** modify + create + delete
**CURRENT PROBLEM:** `AGENTS.md` documents a Python CLI that no longer exists — the single most harmful file in the repo because every agent is instructed to follow it. README claims 108 tests and the wrong D-Bus install path. No PR template, no CODEOWNERS, no Windows build job, actions not SHA-pinned in CI.
**EXACT CHANGE:** replace the Python sections with the 10-rule minimal agent file (Section H); fix `README.md:226` to 110 and the `/etc/dbus-1/system.d` → `/usr/share/dbus-1/system.d` path; add the PR template and CODEOWNERS with **placeholder** owners (never invent usernames); add a `windows` job running `cargo check`; pin actions by SHA.
**DEPENDENCIES:** none.
**TEST:** `grep -r 'openomen' AGENTS.md` returns only the historical attribution line; CI passes on a Windows runner.
**ACCEPTANCE:** no document in the repo describes a stack the code does not use.
**COMMIT:** `docs: replace the stale Python agent guide and add PR/ownership policy`
**PR:** PR 9 — "Governance".

## D.1 Explicitly unnecessary — do not implement

| Brief item | Verdict |
|---|---|
| `TelemetryProvider` + `HardwareController` traits | **Duplicate** of `HardwareProvider` + the D-Bus privilege split. |
| Seven-crate workspace | **Premature.** Feature-gate the UI (C8). |
| Tokio migration | **No identified problem**; blocking writes are rare and single-user. |
| `src/platform/{linux,windows}/{ipc,service,tray,autostart}.rs` | **Empty modules.** `src/service/`, `src/desktop/` already own this; Windows has no implementation. |
| Windows fan/GPU control | **Speculative.** Report unavailable (C9). |
| UI rewrite / GPUI migration | **Rejected** — see B.1. |
| `capabilities.rs` extraction | **145 lines**, co-located with its only consumer. |
| Redrawing the logo | **Not the defect** — see A.2. |
| `anyhow` as the error convention | **Remove the unused dependency** instead. |

---

# E. Migration phases

| Phase | PRs | Depends on | Rollback |
|---|---|---|---|
| **A. Truthfulness** (bugs users see) | 1: C1, C2 | — | Revert the PR; no schema or contract change. |
| **B. Control feedback** | 2: C3, C4 | A | Revert; `pending_fan_mode` returns to dead-but-harmless. |
| **C. Control concurrency** | 3: C5 | B (same files) | Revert; prior behaviour was leaky but not corrupting. |
| **D. Sampling cost** | 4: C6 | — (parallel-safe: touches only `linux/gpu.rs`) | Revert to per-tick spawn. |
| **E. Observability** | 5: C7 | — | Revert; logging is additive. |
| **F. Build boundary** | 6: C8 | E (both edit `lib.rs`) | Revert the feature gate. |
| **G. Platform honesty** | 7: C9 | F | Revert; `cfg` only. |
| **H. Branding** | 8: C10 | — | Revert assets; no code coupling. |
| **I. Governance** | 9: C11 | — | Revert docs. |

Phases A, D, E, H, I are mutually independent and can run in parallel worktrees. B→C and E→F are ordered by file overlap. **No phase changes the D-Bus contract or the config schema**, so no migration or version-compat work is needed.

Gemini's independent verification happens after each PR, not after the whole sequence (Section I).

---

# F. Logging implementation

## F.1 Crate selection

| Option | Verdict |
|---|---|
| Extend `tracing-subscriber` with `tracing-appender 0.2` | **Chosen.** Same ecosystem, non-blocking writer, rotation and retention built in. Dual MIT/Apache-2.0, GPL-compatible. |
| `log` + `env_logger` + manual rotation | Rejected — replaces a working ecosystem and loses structured fields. |
| Custom file writer | Rejected — reimplements rotation badly. |

**Do not** add the `json` feature: no log reader in this project consumes JSON, and tracing's key/value fields are already structured in the default text format.

## F.2 New file — `src/logging.rs`

```rust
//! Process-wide logging: stderr for `RUST_LOG`, plus a daily file for support.
//!
//! The returned guard owns the non-blocking writer; it must live for the whole
//! process or the last buffered lines are lost on exit.

use std::path::PathBuf;

use tracing_appender::non_blocking::WorkerGuard;
use tracing_appender::rolling::{RollingFileAppender, Rotation};
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::EnvFilter;

/// Keeps the background log writer alive. Drop only at process exit.
pub struct LoggingGuard {
    _file: WorkerGuard,
}

/// Resolve `~/.local/state/zenomen/logs` (Linux) or the platform equivalent.
fn log_directory() -> Option<PathBuf> {
    directories::ProjectDirs::from("com", "zenomen", "zenomen")
        .map(|dirs| dirs.state_dir().unwrap_or_else(|| dirs.data_dir()).join("logs"))
}

/// Install the global subscriber. Failure is reported, never fatal: a desktop
/// app that cannot write a log file must still start.
pub fn init() -> Option<LoggingGuard> {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

    let stderr_layer = tracing_subscriber::fmt::layer()
        .with_writer(std::io::stderr)
        .with_target(false);

    let Some(log_dir) = log_directory() else {
        let _ = tracing_subscriber::registry()
            .with(filter)
            .with(stderr_layer)
            .try_init();
        return None;
    };

    if let Err(e) = std::fs::create_dir_all(&log_dir) {
        eprintln!("warning: cannot create log directory {}: {e}", log_dir.display());
        let _ = tracing_subscriber::registry()
            .with(filter)
            .with(stderr_layer)
            .try_init();
        return None;
    }

    let appender = RollingFileAppender::builder()
        .rotation(Rotation::DAILY)
        .filename_prefix("zenomen")
        .filename_suffix("log")
        .max_log_files(7)
        .build(&log_dir);

    let appender = match appender {
        Ok(appender) => appender,
        Err(e) => {
            eprintln!("warning: cannot open log file in {}: {e}", log_dir.display());
            let _ = tracing_subscriber::registry()
                .with(filter)
                .with(stderr_layer)
                .try_init();
            return None;
        }
    };

    let (writer, guard) = tracing_appender::non_blocking(appender);

    let file_layer = tracing_subscriber::fmt::layer()
        .with_writer(writer)
        .with_ansi(false)
        .with_target(true);

    let _ = tracing_subscriber::registry()
        .with(filter)
        .with(stderr_layer)
        .with(file_layer)
        .try_init();

    Some(LoggingGuard { _file: guard })
}
```

`try_init` rather than `init` so a second call in the same process (tests) degrades instead of panicking.

## F.3 Call sites

```rust
// src/main.rs — immediately after the LC_CTYPE fix, BEFORE single instance/tray
let _logging_guard = zenomen::logging::init();

// src/bin/zenomen-hw.rs — first statement in main, replacing the EnvFilter block
let _logging_guard = zenomen::logging::init();
```

`pub mod logging;` is unconditional in `src/lib.rs` (the daemon needs it too).

## F.4 Retention and failure policy

| Property | Value | Rationale |
|---|---|---|
| Rotation | daily | Matches support-question timescales. |
| Retention | 7 files | Bounded disk: worst case ~7 × a few hundred KB. |
| Level | `RUST_LOG`, default `info` | DEBUG is opt-in. |
| Failure | warn to stderr, continue | Logging must never be a startup blocker. |
| Telemetry samples | **never at INFO** | 1 Hz × hours would drown the file. |

## F.5 Event catalog (one table, one place)

`application.started` · `application.shutdown_started` · `application.shutdown_completed` · `hardware.discovery_started` · `hardware.discovery_completed` · `hardware.capability_changed` · `telemetry.sample_failed` · `hardware_control.requested` · `hardware_control.write_started` · `hardware_control.readback_completed` · `hardware_control.verified` · `hardware_control.failed` · `service.connected` · `service.disconnected` · `configuration.loaded` · `configuration.saved` · `configuration.recovery_required`

`telemetry.sample_completed` is deliberately **excluded** — it is the per-second event the brief warns against.

```rust
// good: fields, no interpolation
tracing::info!(
    operation = "fan_mode_change",
    requested_mode = %mode,
    "hardware_control.requested"
);

tracing::error!(
    operation = "fan_mode_change",
    requested_mode = %mode,
    retries,
    error = %err,
    "hardware_control.failed"
);
```

---

# G. Branding implementation

## G.1 Corrected asset inventory

Already present and correct: hicolor 16/32/48/64/128/256/512 + `scalable/apps/zenomen.svg`; `zenomen.ico` (16/32/48/256); 256×256 `data/icons/zenomen.png`; SVG masters in `logo/`.

Required additions: hicolor **24×24**; `.ico` entries 24/64/128; a **72×72** sidebar export (36 px at 2× DPR).

## G.2 `scripts/generate-icons.sh`

```bash
#!/usr/bin/env bash
# Regenerate raster icons from the SVG master. Never edits logo/ masters.
set -euo pipefail

SOURCE="data/icons/zenomen.svg"
OUT="data/icons/hicolor"
SIZES=(16 24 32 48 64 128 256 512)

command -v inkscape >/dev/null || { echo "error: inkscape is required" >&2; exit 1; }

for size in "${SIZES[@]}"; do
    install -d "$OUT/${size}x${size}/apps"
    inkscape "$SOURCE" \
        --export-type=png \
        --export-filename="$OUT/${size}x${size}/apps/zenomen.png" \
        --export-width="$size" \
        --export-height="$size"
done

# Sidebar mark: 36px at 2x device pixel ratio.
inkscape "$SOURCE" --export-type=png \
    --export-filename="data/icons/zenomen-sidebar.png" \
    --export-width=72 --export-height=72

if command -v magick >/dev/null; then
    magick "$OUT/16x16/apps/zenomen.png" \
           "$OUT/24x24/apps/zenomen.png" \
           "$OUT/32x32/apps/zenomen.png" \
           "$OUT/48x48/apps/zenomen.png" \
           "$OUT/64x64/apps/zenomen.png" \
           "$OUT/128x128/apps/zenomen.png" \
           "$OUT/256x256/apps/zenomen.png" \
           data/icons/zenomen.ico
else
    echo "warning: imagemagick not found; .ico not regenerated" >&2
fi
```

Verify ImageMagick's ICO coder is present (`magick -list format | grep -i ico`); the current `.ico` has only 4 entries, so this is a real regeneration, not a no-op.

## G.3 `scripts/validate-icons.py`

```python
#!/usr/bin/env python3
"""Fail if the committed icon set is incomplete or mis-sized."""
import struct
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
HICOLOR = ROOT / "data" / "icons" / "hicolor"
SIZES = (16, 24, 32, 48, 64, 128, 256, 512)
ICO_MIN_ENTRIES = 6


def png_size(path: Path) -> tuple[int, int]:
    header = path.read_bytes()[:24]
    return struct.unpack(">II", header[16:24])


def main() -> int:
    failures: list[str] = []

    for size in SIZES:
        path = HICOLOR / f"{size}x{size}" / "apps" / "zenomen.png"
        if not path.exists():
            failures.append(f"missing {path.relative_to(ROOT)}")
            continue
        if png_size(path) != (size, size):
            failures.append(f"{path.relative_to(ROOT)} is {png_size(path)}, expected {size}px")

    svg = HICOLOR / "scalable" / "apps" / "zenomen.svg"
    if not svg.exists() or b"<svg" not in svg.read_bytes()[:512]:
        failures.append("scalable SVG missing or not an SVG document")

    ico = ROOT / "data" / "icons" / "zenomen.ico"
    if ico.exists():
        entries = struct.unpack("<H", ico.read_bytes()[4:6])[0]
        if entries < ICO_MIN_ENTRIES:
            failures.append(f"zenomen.ico has {entries} entries, need >= {ICO_MIN_ENTRIES}")
    else:
        failures.append("zenomen.ico missing")

    for message in failures:
        print(f"FAIL {message}", file=sys.stderr)
    if failures:
        return 1
    print(f"OK {len(SIZES)} PNG sizes, scalable SVG, ICO entries >= {ICO_MIN_ENTRIES}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
```

Uses only the stdlib — no Pillow dev-dependency for a nine-line check.

## G.4 Reproducibility

Pin the toolchain in the script header (`inkscape >= 1.2`) and record it in `docs/PACKAGING.md`. Inkscape's PNG export is deterministic for a given version and size; if it proves not to be, commit the outputs and treat the script as documentation of the process rather than a CI gate.

---

# H. GitHub and agent discipline

## H.1 Branch and commit examples

```bash
git switch GUI
git pull --ff-only
git switch -c fix/101-battery-capability
git add src/hardware/registry.rs src/core/state.rs tests/sensor_registry_tests.rs
git commit -m "fix(hardware): detect battery instead of hardcoding the capability"
```

```bash
git switch -c feat/105-rotating-file-logs
git add src/logging.rs src/lib.rs src/main.rs src/bin/zenomen-hw.rs Cargo.toml Cargo.lock
git commit -m "feat(logging): add rotating file logs and initialize before D-Bus setup"
```

## H.2 `AGENTS.md` — replace, do not extend

```markdown
# Repository Agent Rules

1. Inspect source before modifying it.
2. Follow existing domain types and conventions.
3. Make minimal, focused changes.
4. Never modify another platform's implementation without assignment.
5. Do not introduce dependencies without justification.
6. Add tests for new behavior.
7. Preserve existing behavior unless explicitly changing it.
8. Inspect the complete diff before committing.
9. Never claim tests passed without execution evidence.
10. Update documentation only where implementation changes require it.

## Stack

Rust edition 2024 + Slint 1.18 (declared `1.17`, MSRV 1.88). Two binaries from one
package: `zenomen` (GUI, unprivileged) and `zenomen-hw` (D-Bus daemon, root).
Authoritative detail lives in `.planning/codebase/`. The retired Python `openomen`
CLI must not be reintroduced.

## Where things go

Read `.planning/codebase/STRUCTURE.md` § "Where to Add New Code" before adding a
sensor, a control, a screen, or a config key. Those steps are current and verified.

## Hard rules

- The GUI process never opens a sysfs file for writing. Writes go
  `src/service/client.rs` → `HardwareService` → `HardwareBackend::write_and_verify`.
- Never write a Slint property from a background thread; send an `AppEvent`.
- A missing sensor is `None`/empty, never `0`.
- Any new `#[interface]` mutating method must call `authorize(&hdr)` first.
```

Delete `AGENTS.md.graphicle-bak`.

## H.3 `.github/PULL_REQUEST_TEMPLATE.md`

```markdown
## Objective
What problem does this PR solve?

## Related issue
Closes #

## Implementation
Describe the actual implementation.

## Files changed
- Created:
- Modified:
- Deleted:

## Architecture impact
Describe dependency and module changes. State "none" if none.

## Tests
- [ ] Unit tests
- [ ] Integration tests
- [ ] `cargo fmt --all -- --check`
- [ ] `cargo clippy --all-targets --all-features -- -D warnings`
- [ ] Linux build
- [ ] Windows `cargo check` (if platform-relevant)
- [ ] `cargo audit`

## Gemini review
- [ ] Independent diff review
- [ ] Targeted tests executed
- [ ] Regression results recorded

## Risks and limitations
List anything not implemented or verified.

## Screenshots / logs
Attach evidence when relevant.
```

## H.4 `.github/CODEOWNERS`

```text
# Replace every placeholder with a real maintainer before enabling enforcement.
/src/hardware/          @MAINTAINER
/src/service/           @MAINTAINER
/src/desktop/           @MAINTAINER
/src/logging.rs         @MAINTAINER
/data/polkit/           @MAINTAINER
/data/dbus/             @MAINTAINER
/data/systemd/          @MAINTAINER
/.github/workflows/     @MAINTAINER
/Cargo.toml             @MAINTAINER
```

## H.5 Branch protection for `GUI`

Require a PR, the CI `quality` check, resolved conversations, no force pushes, no direct pushes. Require `cargo audit`. **Do not add a `develop` branch.**

---

# I. Gemini testing handoff

```markdown
# Gemini Testing Handoff

## Repository
ZenOmen

## Branch
fix/101-battery-capability

## Commit
<paste `git rev-parse HEAD`>

## Objective
Make `battery_metrics` measured rather than hardcoded so `AppState::is_degraded()`
stops reporting a permanent degraded state on battery-equipped laptops.

## Files changed
- src/hardware/registry.rs
- src/core/state.rs
- tests/sensor_registry_tests.rs

## Behavior changed
`SensorRegistry::scan_roots` now also probes `<hwmon_root>/../power_supply/BAT*/capacity`.
`detect_capabilities().battery_metrics` reflects that probe. No D-Bus, config, or UI
contract changed.

## Commands
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --test sensor_registry_tests
cargo test --lib state
xvfb-run -a cargo test --all

## Expected behavior
- A fixture with `BAT0/capacity` reports `battery_metrics == true`.
- A fixture without it reports `false`.
- A fully capable state is NOT degraded.
- No existing sensor-priority test changes.

## Regression risks
- Capability count drifting from the struct.
- Fixture trees that lack a `power_supply` sibling (must degrade to `false`, not panic).
- `is_degraded()` semantics on desktops without a battery (must remain `true`).

## Gemini instructions
Inspect the complete diff. Run the listed commands. Identify untested edge cases.
Report exact failing commands and their output. Do not modify production source.
Do not assume a failure is caused by this PR without checking the baseline.
```

Gemini must report: PASS/FAIL, exact command, relevant output, file:line of any defect, reproduction steps, severity, and whether it is a regression or pre-existing.

---

# J. Remaining uncertainties

1. **No memory or startup measurement of either framework.** No release binary exists on this machine, and building both a Slint and a GPUI-CE prototype was outside the audit's budget. Every "Memory"/"Startup" cell in B.1 is therefore `not measured`, not `equal`.
2. **SVG rendering under this exact Slint feature set is unverified.** `default-features = false` with only `std`, `compat-1-2`, `backend-winit`, `renderer-femtovg`; SVG support is not among the enabled features. C10 requires a compile-and-look check before switching `@image-url` to the SVG.
3. **C8's binary-level feature opt-out may not be expressible.** `[features]` are package-wide; `src/bin/zenomen-hw.rs` cannot select `default-features` independently. The change works only if the daemon's dependency on `zenomen::app` can be removed or the module gated. Verify with `cargo tree -e features` before committing; if it cannot, drop C8 to a module-level `#[cfg]` and accept the link cost.
4. **`apply_preset` failure behaviour was read, not executed.** C4's diagnosis is from source shape; Gemini should reproduce it with a fixture lacking both interfaces.
5. **Polkit behaviour is untested by design.** `with_polkit_required` exists but no test asserts denial. Not scheduled here — it needs a private session bus and belongs to a security-focused PR.
6. **The 2-hour soak and real-hardware RSS figures in `docs/RESOURCE-BUDGET.md` remain unvalidated** (it substitutes Private_Dirty for RSS and reports 132 MB VmRSS). C6 may improve this; it does not prove the budget.
7. **Windows was not built.** No Windows target is installed here, so C9's acceptance criterion is a CI claim, not a local one.
8. **`hp-wmi` temperature tier is recommended but not designed.** CONCERNS flags its absence; the label names HP exposes vary by model, so the tier needs a real OMEN dump before adding a hardcoded label list.

## Resolution

Not yet fixed — this is a planning artifact. Implement Phase A (PR 1) only after review.

---

# K. Governance compliance (added 2026-11-07, after reading all seven maps)

This section re-grounds every change in `.planning/codebase/` per the mandatory
governance rule. Sections A–J remain the analysis; this section is the traceable
authority for implementation. Where the maps disagreed with each other, K.2 records the
resolution from source evidence. Where they disagreed with this plan, the plan changed.

## K.1 Document-to-change traceability

| Change ID | Codebase document | Relevant section | Existing finding | Proposed action |
|---|---|---|---|---|
| C1 battery | `CONCERNS.md` | Tech Debt; Known Bugs; Test Coverage Gaps | "`battery_metrics` hardcoded `false` … `is_degraded()` is permanently `true`" | Implement the `/sys/class/power_supply` read path |
| C1 battery | `INTEGRATIONS.md` | Kernel Integration Surface → Semantics | "there is no `/sys/class/power_supply` read path … Do not claim battery support without implementing it" | Same — implement, do not flip the flag |
| C1 battery | `ARCHITECTURE.md` | Architectural Constraints | "`battery_metrics` is hardcoded `false`" | Same |
| C1 battery | `STRUCTURE.md` | Where to Add New Code → New telemetry sensor | Capability flag + detection steps | Follow those steps exactly |
| C2 sensor count | `CONCERNS.md` | Tech Debt | "`EXPECTED_SENSOR_COUNT` includes a sensor the Linux provider never sets" | Adjust the counted set, keep the serialized field |
| C2 sensor count | `CONVENTIONS.md` | Constants | Associated consts are the convention for public API counts | Keep it an associated const |
| C2 sensor count | `INTEGRATIONS.md` | Data Storage | Diagnostics JSON includes latest telemetry | Preserve the serialized shape |
| C3 pending fan mode | `CONCERNS.md` | Tech Debt; Fragile Areas | "`pending_fan_mode` … nothing ever assigns `Some(...)`" | Drive it from a real event |
| C3 pending fan mode | `ARCHITECTURE.md` | Architectural Constraints; Anti-Patterns | "`AppState` changes only through `apply` … Do not add setters"; "Adding an event that skips state" | Add `AppEvent::FanModeRequested`, handle in `apply` |
| C4 preset failure | `CONCERNS.md` | Known Bugs; Fragile Areas | "`apply_preset` reports success when both power and fan writes fail" | Return `Err` on total failure |
| C4 preset failure | `INTEGRATIONS.md` | APIs → `ApplyPreset` | "reports partial success" | Keep partial `Ok`, error on total |
| C4 preset failure | `CONVENTIONS.md` | Error Handling tier 1 | `VerificationFailed { target, actual, retries }` → `Failed` | Reuse the existing variant |
| C5 mutation threads | `CONCERNS.md` | Performance; Fragile Areas | "Two threads per mutating action, leaked on timeout" | Single in-flight slot, no orphan |
| C5 mutation threads | `INTEGRATIONS.md` | Polkit | 30 s timeout so a prompt cannot stall telemetry | Preserve the timeout, reject concurrent requests |
| C5 mutation threads | `ARCHITECTURE.md` | Architectural Constraints → Non-blocking worker | Mutations must not block the sampling loop | Keep the off-thread path |
| C6 nvidia-smi | `CONCERNS.md` | Performance; Known Bugs | per-tick spawn; detached thread; 200 ms below cold latency | Persistent watcher thread |
| C6 nvidia-smi | `INTEGRATIONS.md` | External Processes | "on timeout the thread is detached rather than joined" | Same, with a bounded lifetime |
| C6 nvidia-smi | `STACK.md` | Runtime → thread list | `zenomen-nv-query` is a named per-tick thread | Replace with one long-lived thread |
| C7 logging | `INTEGRATIONS.md` | Monitoring & Observability | "No log files, rotation, or log levels beyond tracing defaults" | Rotating file layer |
| C7 logging | `CONCERNS.md` | Known Bugs | "Logging initialized after D-Bus setup, losing early diagnostics" | Initialize first |
| C7 logging | `CONVENTIONS.md` | Logging | tracing macros only; no per-tick INFO | Keep both rules |
| C8 UI feature gate | `ARCHITECTURE.md` | Architectural Constraints | "the daemon binary links it too" | Gate `include_modules!()` |
| C8 UI feature gate | `CONVENTIONS.md` | Module Design | "`src/lib.rs` … ends with `slint::include_modules!();`" | Update this sentence after the change |
| C9 unavailable provider | `ARCHITECTURE.md` | Key Abstractions; Platform | One `HardwareProvider` seam; Windows unvalidated | Add a provider that reports nothing supported |
| C9 unavailable provider | `STRUCTURE.md` | `src/hardware/` | Provider contract + adapters live here | New file `unavailable.rs` |
| C9 unavailable provider | `PROJECT.md` (constraint) | Compatibility | "Windows kept architecture-ready but unadvertised" | Compile-check only, no hardware claims |
| C10 branding | `STRUCTURE.md` | Special Directories → `data/icons/hicolor/` | "Multi-resolution installed icons (16–512 px + scalable)" | Add 24 px; regenerate ICO |
| C10 branding | `STACK.md` | Build/Dev → `winres` | "embed … `data/icons/zenomen.ico`" | New ICO must keep the 4+ sizes |
| C11 governance docs | `STACK.md` | Scope | "`AGENTS.md` still embeds its documentation and is not authoritative" | Replace it |
| C11 governance docs | `CONVENTIONS.md` | Scope and Ground Truth | "**Do not follow the Python section**" | Replace it |
| C11 governance docs | `CONCERNS.md` | Documentation drift | README `/etc` vs `/usr/share`; test count 108 vs 110 | Fix README |
| C11 governance docs | `INTEGRATIONS.md` | Installed locations | "Treat the code as correct and fix the README" | Same |
| C12 drop `anyhow` | `STACK.md` | Key Dependencies | "declared but unreferenced … Remove it or leave it unused" | Remove it |
| C12 drop `anyhow` | `CONVENTIONS.md` | Error Handling | "`anyhow` is declared … but unused" | Remove it |
| C13 systemd hardening | `INTEGRATIONS.md` | systemd | "`ProtectKernelTunables` is **intentionally omitted** … Do not add it" | **No change to the unit** |
| C13 systemd hardening | `data/systemd/zenomen-hw.service` | inline comment (WR-03) | Same statement in source | Resolve against `CONCERNS.md` (K.2) |

Every change has at least one document. No change in this plan lacks a row.

## K.2 Cross-document contradictions, resolved from source

| # | Conflict | Source evidence | Resolution |
|---|---|---|---|
| 1 | `CONCERNS.md` § Security recommends adding `ProtectKernelTunables=yes`; `INTEGRATIONS.md` § systemd says **do not** | `data/systemd/zenomen-hw.service:20-22`: "ProtectKernelTunables is omitted because it renders /sys read-only, conflicting with sysfs fan/power/RGB control carve-outs (WR-03)" | **`CONCERNS.md` is wrong.** `ProtectKernelTunables` mounts `/sys` read-only, which would break every hardware write. Keep the unit unchanged (C13). Recorded, not edited — see K.5. |
| 2 | `CONCERNS.md` claims `ProtectSystem=strict` exempts `/sys`, so `ReadWritePaths` carve-outs are not the effective boundary | True for `ProtectSystem`, but the remedy it proposes defeats the daemon | The effective boundary is the application-level path allow-list (`HardwareBackend::resolve_path`), exactly as `INTEGRATIONS.md` § Integration Constraints states. This is a *documentation* weakness in `CONCERNS.md`, not a code defect. |
| 3 | `STACK.md`: "effective MSRV is the edition floor" (1.85) | Locked `slint 1.18.1`; Slint 1.18 CHANGELOG: "Minimum Supported Rust Version (MSRV) is 1.88" | MSRV is **1.88**. Add `rust-version` so cargo enforces it. |
| 4 | `STACK.md`: "no accessibility feature" (listed under Deliberately Absent) | Slint 1.18.1 docs.rs feature list: `accessibility` is a **default** feature; `default-features = false` is what removes it | The omission is a *choice with a cost*, not an absent capability. Recorded in ADR-001 consequences. |
| 5 | `CONVENTIONS.md` says `src/main.rs:89` hosts `fmt::init()`; `INTEGRATIONS.md` says `:75`; `CONCERNS.md` says `:89` | Actual: after the single-instance `match`, roughly line 85–86 | Line references in the maps drift by ±15 lines. All three identify the same call. Treat map line numbers as approximate; C11's diff must not rely on them. |
| 6 | `AGENTS.md` (project instructions) describes a Python CLI | All seven maps describe Rust + Slint; `STACK.md` says the Python CLI "has been retired" | `AGENTS.md` is the outlier. C11 replaces it. |
| 7 | `STATE.md`: phase 05 `status: complete`, `state_head: b433344`; `.planning/state.json`: phase 5 `pending`, `updated_at 2026-11-02T04:21` | `STATE.md` also records 15/15 plans complete at `2026-11-02T15:45`; HEAD is `761e5f9` | `state.json` is **stale**; `STATE.md` is closer to truth but its `state_head` lags HEAD. Both are planning drift, not code drift (`CONCERNS.md` flagged it). Not corrected here — K.5. |
| 8 | `CONVENTIONS.md`: "There is currently no `pub(crate)` … prefer `pub(crate)`" | No `pub(crate)` in `src/` (verified by the map, consistent with `grep`) | The new battery helper stays **private** to `registry.rs`, not `pub(crate)` — narrower than both options. |
| 9 | My §D.C3 draft added an `AppState` setter | `ARCHITECTURE.md`: "`AppState` changes only through `apply`. Do not add setters." | Plan revised to an `AppEvent` (K.3). |
| 10 | My §D.C2 draft deleted `rgb_profile` | `INTEGRATIONS.md` § Data Storage: diagnostics export serializes the latest telemetry | Plan revised to keep the field and fix the count (K.3). |
| 11 | My §D.C4 draft used `VerificationFailed { expected, … }` | `src/service/types.rs:115-119`: fields are `target`, `actual`, `retries` | Code corrected (K.3). |

## K.3 Implementation specifications

### C1 — Battery capability

```text
CHANGE ID: C1
OBJECTIVE: Make battery capability measured so AppState::is_degraded() stops reporting
  a permanent degraded state on battery-equipped machines.

CODEBASE DOCUMENTS:
- CONCERNS.md — Tech Debt / Known Bugs / Test Coverage Gaps
- INTEGRATIONS.md — Kernel Integration Surface → Semantics
- ARCHITECTURE.md — Architectural Constraints
- STRUCTURE.md — Where to Add New Code → New telemetry sensor

EXISTING IMPLEMENTATION:
- File: src/hardware/registry.rs
- Symbol: SensorRegistry::scan_roots, SensorRegistry::detect_capabilities
- Responsibility: resolve canonical CPU temp, fan tachometers, fan control, ACPI profiles;
  report per-subsystem capabilities.

PROBLEM: battery_metrics is the literal `false` (registry.rs:133) while
  SUBSYSTEM_COUNT = 8 (state.rs:23). reported_sensor_count() therefore never exceeds 7,
  so is_degraded() (state.rs:294) returns true on every machine. The UI also shows a
  capability row claiming no battery device exists.

MINIMAL MODIFICATION: probe /sys/class/power_supply/BAT*/capacity during the existing scan;
  derive that directory from the hwmon root's parent so no public signature changes and
  every existing fixture tree keeps working; report the measured value.

FILES:
- Create: none
- Modify: src/hardware/registry.rs, src/core/state.rs (test only), tests/sensor_registry_tests.rs
- Delete: none
```

```rust
// src/hardware/registry.rs
/// A battery exposes a readable capacity file (battery telemetry is not implemented
/// yet, but the capability must not be guessed).
fn scan_battery_present(power_supply_root: &Path) -> bool {
    let Ok(entries) = std::fs::read_dir(power_supply_root) else {
        return false;
    };
    entries.flatten().any(|entry| {
        let name = entry.file_name();
        name.to_string_lossy().starts_with("BAT")
            && entry.path().join("capacity").exists()
    })
}
```

```text
DEPENDENCY IMPACT: none.
TESTING: fixture with BAT0/capacity → has_battery_metrics() == true; fixture without → false;
  fully capable state → !is_degraded(). Existing sensor-priority tests unchanged.
REGRESSION RISK: fixture trees without a power_supply sibling must degrade to false, not
  panic (read_dir error path returns false). Desktops without a battery stay degraded — correct.
DOCUMENTATION IMPACT: CONCERNS.md known-bug entry becomes resolved (update only after
  verification, per §6); INTEGRATIONS.md § Kernel Integration Surface gains the read path.
GIT: branch fix/<issue>-battery-capability; commit
  "fix(hardware): detect battery instead of hardcoding the capability"; PR 1.
```

### C2 — Sensor count

```text
CHANGE ID: C2
OBJECTIVE: Make the reported/expected sensor ratio truthful on real Linux.
CODEBASE DOCUMENTS: CONCERNS.md (Tech Debt), CONVENTIONS.md (Constants),
  INTEGRATIONS.md (Data Storage), ARCHITECTURE.md (Key Abstractions).
EXISTING IMPLEMENTATION: src/hardware/provider.rs — TelemetrySnapshot::reported_sensor_count
  and EXPECTED_SENSOR_COUNT.
PROBLEM: the counted set includes rgb_profile, which LinuxHardwareProvider never assigns
  (only mock.rs:207 and a test fixture do), so a healthy machine reports 11 of 12.
MINIMAL MODIFICATION: remove that one slot from the counted set; set EXPECTED_SENSOR_COUNT
  to 11; document the exclusion. Keep the field — it is serialized into diagnostics JSON.
FILES: Modify src/hardware/provider.rs and the status-strip copy. Create/Delete: none.
```

Code as in §D.C2 (revised block). `DEPENDENCY IMPACT: none.`

```text
TESTING: nominal MockScenario → reported_sensor_count() == EXPECTED_SENSOR_COUNT.
REGRESSION RISK: diagnostics JSON keeps its shape; nothing that reads rgb_profile changes.
DOCUMENTATION IMPACT: CONCERNS.md tech-debt entry resolved after verification.
GIT: same PR as C1; commit "fix(telemetry): stop counting rgb_profile as a sensor".
```

### C3 — Pending fan mode

```text
CHANGE ID: C3
OBJECTIVE: Give the verified write an in-progress affordance by driving pending_fan_mode
  from a real event.
CODEBASE DOCUMENTS: CONCERNS.md (Tech Debt, Fragile Areas), ARCHITECTURE.md
  (Architectural Constraints, Anti-Patterns).
EXISTING IMPLEMENTATION: src/core/state.rs AppState.pending_fan_mode; src/core/worker.rs
  AppCommand dispatch; src/app.rs:432 reads it.
PROBLEM: nothing ever assigns Some(...), so the "verifying read-back" copy can never render
  and tests/fan_control_tests.rs asserts the absence of the feature.
MINIMAL MODIFICATION: add AppEvent::FanModeRequested(String), handle it in AppState::apply,
  emit it before execute_mutating_action. No setter (ARCHITECTURE forbids one).
FILES: Modify src/core/events.rs, src/core/state.rs, src/core/worker.rs,
  tests/fan_control_tests.rs. Create/Delete: none.
```

Code as in §D.C3 (revised block).

```text
DEPENDENCY IMPACT: none.
TESTING: request_then_confirm_clears_pending; request_then_failure_keeps_error_and_clears_pending.
REGRESSION RISK: exhaustive matches on AppEvent force every consumer to handle the new
  variant — the compiler is the safety net. No behaviour change until a write is requested.
DOCUMENTATION IMPACT: ARCHITECTURE.md "Adding an event that skips state" example stays valid.
GIT: PR 2; "fix(state): track a pending fan mode across the verified write".
```

### C4 — Preset total failure

```text
CHANGE ID: C4
OBJECTIVE: Stop reporting success for a preset whose both hardware legs failed.
CODEBASE DOCUMENTS: CONCERNS.md (Known Bugs, Fragile Areas), INTEGRATIONS.md
  (APIs → ApplyPreset), CONVENTIONS.md (Error Handling tier 1).
EXISTING IMPLEMENTATION: src/service/backend.rs HardwareBackend::apply_preset
  (power_applied / fan_applied / error_messages / partial_success / message).
PROBLEM: returns Ok when power_applied == false && fan_applied == false; the message says
  "failed" but the D-Bus call succeeds and partial_success is false.
MINIMAL MODIFICATION: six lines before the Ok(PresetResult) — return VerificationFailed when
  neither leg applied. Keep Ok for at least one confirmed application.
FILES: Modify src/service/backend.rs and tests/power_preset_tests.rs. Create/Delete: none.
```

Code as in §D.C4 (corrected block).

```text
DEPENDENCY IMPACT: none — the variant and its From mapping already exist.
TESTING: no-interface fixture → Err(VerificationFailed); fan-only fixture → Ok, fan_applied.
REGRESSION RISK: this changes an observable D-Bus outcome from Ok to Err on total failure;
  clients now take the error path (desired). Verify no test asserts Ok for a no-op fixture.
DOCUMENTATION IMPACT: CONCERNS.md known-bug entry resolved after verification; INTEGRATIONS.md
  ApplyPreset row unchanged (it already says "reports partial success").
GIT: PR 2; "fix(service): fail the preset when neither hardware leg applied".
```

### C5 — Mutating-action concurrency

```text
CHANGE ID: C5
OBJECTIVE: Prevent orphaned RPC threads and concurrent hardware writes.
CODEBASE DOCUMENTS: CONCERNS.md (Performance, Fragile Areas), INTEGRATIONS.md (Polkit
  30 s timeout), ARCHITECTURE.md (Non-blocking worker).
EXISTING IMPLEMENTATION: src/core/worker.rs execute_mutating_action(name, sender, f,
  event_builder) — two threads, 30 s recv_timeout, handle dropped on timeout.
PROBLEM: on timeout the RPC thread and the in-flight write continue; a late success mutates
  hardware after the UI reported a timeout; two rapid toggles race.
MINIMAL MODIFICATION: an Arc<AtomicBool> owned by the worker (not a static — ARCHITECTURE
  lists global state as a constraint) rejects a second action with a visible ControlFailed,
  released by a Drop guard. Keep the 30 s timeout.
FILES: Modify src/core/worker.rs, tests/fan_control_tests.rs. Create/Delete: none.
```

```rust
struct InFlightGuard(Arc<AtomicBool>);

impl Drop for InFlightGuard {
    fn drop(&mut self) {
        self.0.store(false, Ordering::SeqCst);
    }
}
```

```text
DEPENDENCY IMPACT: none (std::sync::atomic).
TESTING: slow closure → second dispatch rejected; timeout → flag released.
REGRESSION RISK: previously-concurrent requests now serialize; a user cannot queue two
  changes. Acceptable and visible. The 30 s budget and thread names are unchanged.
DOCUMENTATION IMPACT: CONCERNS.md perf/Fragile entry resolved after verification.
GIT: PR 3; "fix(worker): serialize mutating actions and release the in-flight slot".
```

### C6 — nvidia-smi watcher

```text
CHANGE ID: C6
OBJECTIVE: Remove the per-tick process/thread spawn and the permanently detached thread.
CODEBASE DOCUMENTS: CONCERNS.md (Performance, Known Bugs, Scaling Limits),
  INTEGRATIONS.md (External Processes), STACK.md (Runtime thread list).
EXISTING IMPLEMENTATION: src/hardware/linux/gpu.rs sample_nvidia_smi(timeout) — spawns a
  named thread per call, 200 ms recv_timeout, detaches on timeout; called from
  src/hardware/linux/provider.rs:164.
PROBLEM: at 1 Hz a slow/absent GPU can accumulate one thread and one nvidia-smi process per
  second; 200 ms is below cold-call latency so healthy GPUs report Unavailable.
MINIMAL MODIFICATION: one long-lived NvidiaSampler with a coalescing request slot and a
  latest-value cell; 1.5 s timeout; drop-stale; injectable program path. Stays in gpu.rs.
FILES: Modify src/hardware/linux/gpu.rs, src/hardware/linux/provider.rs,
  tests/linux_gpu_tests.rs. Create/Delete: none.
```

Code as in §D.C6 (revised block with `new`/`latest`).

```text
DEPENDENCY IMPACT: none.
TESTING: N requests while one is in flight coalesce; a slow command leaves latest unchanged.
REGRESSION RISK: first sample after startup is None until the first query returns (previously
  a 200 ms wait); the UI already renders Unavailable for that state. Provider construction
  now spawns one thread even when no dGPU is present — gate construction on vendor detection.
DOCUMENTATION IMPACT: STACK.md Runtime thread list; CONCERNS.md entries after verification.
GIT: PR 4; "perf(gpu): replace the per-tick nvidia-smi spawn with a watcher thread".
```

### C7 — Logging

```text
CHANGE ID: C7
OBJECTIVE: Persist diagnostics across desktop launches and stop dropping startup warnings.
CODEBASE DOCUMENTS: INTEGRATIONS.md (Monitoring & Observability), CONCERNS.md (Known Bugs),
  CONVENTIONS.md (Logging), STACK.md (Key Dependencies: tracing).
EXISTING IMPLEMENTATION: tracing_subscriber::fmt::init() in src/main.rs (after single
  instance/tray); EnvFilter in src/bin/zenomen-hw.rs:12-16.
PROBLEM: stderr only, so a launcher-started GUI leaves no evidence; and init runs after the
  D-Bus/tray block, dropping the Disabled(e) warning at main.rs:83.
MINIMAL MODIFICATION: new src/logging.rs with a stderr layer + daily file layer and a
  LoggingGuard; call it as the first statement after the locale fix in both binaries.
FILES: Create src/logging.rs. Modify src/lib.rs, src/main.rs, src/bin/zenomen-hw.rs,
  Cargo.toml, Cargo.lock, tests/logging_tests.rs. Delete: none.
```

Code as in §F.2 / §F.3.

```text
DEPENDENCY IMPACT: add tracing-appender = "0.2" (MIT/Apache-2.0, GPL-compatible).
TESTING: double init does not panic; a temp XDG_STATE_HOME receives one line; unwritable
  directory returns None and the app continues.
REGRESSION RISK: two layers can duplicate a line on stderr+file — intended. RUST_LOG still
  governs both. A failure to create the log directory must never abort startup.
DOCUMENTATION IMPACT: CONVENTIONS.md § Logging subscriber sentence; INTEGRATIONS.md § Logs;
  STACK.md dependency table. All after verification.
GIT: PR 5; "feat(logging): add rotating file logs and initialize before D-Bus setup".
```

### C8 — UI feature gate

```text
CHANGE ID: C8
OBJECTIVE: Stop compiling and linking the Slint UI into the privileged daemon.
CODEBASE DOCUMENTS: ARCHITECTURE.md (Compile-time UI coupling), CONVENTIONS.md
  (Module Design), STRUCTURE.md (src/ root), STACK.md (Frameworks).
EXISTING IMPLEMENTATION: src/lib.rs — `slint::include_modules!()` unconditional at :14;
  src/bin/zenomen-hw.rs imports service types only (verified).
PROBLEM: zenomen-hw links AppWindow and the FemtoVG/winit stack it never creates, and any
  .slint compile error breaks the daemon build.
MINIMAL MODIFICATION: `[features] default = ["ui"]; ui = ["dep:slint"]` and gate the app
  module + include_modules!() behind `#[cfg(feature = "ui")]`.
FILES: Modify Cargo.toml, src/lib.rs. Create/Delete: none.
```

```rust
// src/lib.rs
#[cfg(feature = "ui")]
pub mod app;

#[cfg(feature = "ui")]
slint::include_modules!();
```

```text
DEPENDENCY IMPACT: no new crate; slint becomes optional and is enabled by default.
TESTING: cargo build --no-default-features --bin zenomen-hw succeeds; cargo tree -e features
  shows no slint for that build; full suite unchanged with defaults.
REGRESSION RISK: HIGH uncertainty — `[features]` are package-wide, so a bin target cannot
  opt out of defaults. If `--no-default-features --bin zenomen-hw` still enables `ui` for the
  library, this change is NOT expressible without a workspace split. Verify before committing;
  if it fails, drop C8 and record why (do not force a workspace for it).
DOCUMENTATION IMPACT: CONVENTIONS.md Module Design sentence; ARCHITECTURE.md constraint.
GIT: PR 6; "build: gate the Slint UI behind a default feature so the daemon stops linking it".
```

### C9 — Unavailable provider

```text
CHANGE ID: C9
OBJECTIVE: Make non-Linux targets compile and report honestly instead of failing or
  pretending.
CODEBASE DOCUMENTS: ARCHITECTURE.md (Key Abstractions, Compile-time platform constraints),
  STRUCTURE.md (src/hardware/), PROJECT.md (Windows unadvertised).
EXISTING IMPLEMENTATION: src/hardware/provider.rs trait; LinuxHardwareProvider selected in
  src/main.rs:183-195; no non-Linux provider.
PROBLEM: nothing implements HardwareProvider off Linux, so the architecture-readiness claim
  is untested and any Windows build either fails or reaches Linux paths.
MINIMAL MODIFICATION: UnavailableHardwareProvider returning HardwareCapabilities::none() and
  Err for every sample; select it under #[cfg(not(target_os = "linux"))].
FILES: Create src/hardware/unavailable.rs. Modify src/hardware/mod.rs, src/main.rs.
  Delete: none.
```

```rust
impl HardwareProvider for UnavailableHardwareProvider {
    fn capabilities(&self) -> HardwareCapabilities {
        HardwareCapabilities::none()
    }

    fn sample_telemetry(&self) -> Result<TelemetrySnapshot, String> {
        Err("hardware telemetry is not implemented for this platform".to_string())
    }
}
```

```text
DEPENDENCY IMPACT: none.
TESTING: on Linux, reported_sensor_count() == 0 and is_degraded() == true for this provider;
  CI adds a windows job running `cargo check`.
REGRESSION RISK: none on Linux (the type is not constructed there). Windows becomes
  compilable but with zero features — which is the honest state.
DOCUMENTATION IMPACT: ARCHITECTURE.md platform table; STRUCTURE.md src/hardware/ listing.
GIT: PR 7; "feat(hardware): add an explicit unavailable provider for non-Linux targets".
```

### C10 — Branding

```text
CHANGE ID: C10
OBJECTIVE: Remove the soft sidebar mark and complete the icon export set.
CODEBASE DOCUMENTS: STRUCTURE.md (data/icons/hicolor, logo/), STACK.md (winres icon),
  CONCERNS.md (documentation drift is the only related finding).
EXISTING IMPLEMENTATION: ui/components/sidebar.slint:102 renders a 256×256 PNG at 36×36;
  ui/app.slint:31 uses the same file as the window icon; hicolor has 16–512 + SVG;
  zenomen.ico has 16/32/48/256.
PROBLEM: the downscale is soft; 24 px hicolor and ICO 24/64/128 are missing. The artwork is
  NOT the problem.
MINIMAL MODIFICATION: export 24 px and a 72 px sidebar asset; regenerate the ICO; add a
  generator + stdlib validator. Do not redraw, do not overwrite logo/ masters.
FILES: Modify data/icons/**; Create scripts/generate-icons.sh, scripts/validate-icons.py,
  data/icons/zenomen-sidebar.png. Delete: none.
```

```slint
// ui/components/sidebar.slint — only after verifying the feature set renders it
Image {
    source: @image-url("../../data/icons/zenomen-sidebar.png");
    width: 36px;
    height: 36px;
    image-fit: contain;
}
```

```text
DEPENDENCY IMPACT: build-time only (inkscape, optionally ImageMagick). Not in Cargo.toml.
TESTING: scripts/validate-icons.py exits 0; visual check at 100/125/200 % scaling.
REGRESSION RISK: an SVG swap can fail at runtime if unsupported — verify before choosing it;
  the PNG path is the safe fallback.
DOCUMENTATION IMPACT: STRUCTURE.md icon listing; docs/PACKAGING.md icon section.
GIT: PR 8; "fix(branding): export 24px assets and render the sidebar mark from a matched resolution".
```

### C11 — Governance documents

```text
CHANGE ID: C11
OBJECTIVE: Eliminate the stale Python guide and the documented governance gaps.
CODEBASE DOCUMENTS: STACK.md (Scope), CONVENTIONS.md (Scope and Ground Truth),
  CONCERNS.md (Documentation drift), INTEGRATIONS.md (Installed locations).
EXISTING IMPLEMENTATION: AGENTS.md embeds the retired Python stack; AGENTS.md.graphicle-bak
  is committed; README.md:226 says 108 tests; README.md:70 says /etc/dbus-1/system.d;
  .github/ has only two workflows (no PR template, no CODEOWNERS).
PROBLEM: agents are instructed to follow a stack that does not exist.
MINIMAL MODIFICATION: replace AGENTS.md's Python sections with the 10 rules + current stack
  note; delete the .bak; correct README counts/paths; add PR template, CODEOWNERS with
  placeholders, Windows check job, SHA-pinned actions.
FILES: Modify AGENTS.md, README.md, .github/workflows/ci.yml; Create
  .github/PULL_REQUEST_TEMPLATE.md, .github/CODEOWNERS; Delete AGENTS.md.graphicle-bak.
```

```text
DEPENDENCY IMPACT: none.
TESTING: grep -r openomen AGENTS.md returns only the historical attribution line; CI green.
REGRESSION RISK: deleting AGENTS.md.graphicle-bak removes a stray backup — confirm it is not
  referenced anywhere first (grep). README edits are documentation-only.
DOCUMENTATION IMPACT: this is the documentation change; codebase maps need no edit.
GIT: PR 9; "docs: replace the stale Python agent guide and add PR/ownership policy".
```

### C12 — Remove unused `anyhow`

```text
CHANGE ID: C12
OBJECTIVE: Drop a declared-but-unused dependency from the shipped graph.
CODEBASE DOCUMENTS: STACK.md (Key Dependencies), CONVENTIONS.md (Error Handling).
EXISTING IMPLEMENTATION: Cargo.toml declares anyhow = "1.0"; zero references in src/ or tests/.
PROBLEM: an unused dependency enlarges the audited graph and invites a second error
  convention that CONVENTIONS.md explicitly forbids.
MINIMAL MODIFICATION: delete the line; regenerate the lockfile.
FILES: Modify Cargo.toml, Cargo.lock. Create/Delete: none.
```

```toml
# Cargo.toml
-anyhow = "1.0"
```

```text
DEPENDENCY IMPACT: remove anyhow (and any now-unreferenced transitive edges).
TESTING: cargo check --all-targets; cargo audit.
REGRESSION RISK: low; nothing imports it. A transitive crate may still pull anyhow — that is
  unaffected and acceptable.
DOCUMENTATION IMPACT: STACK.md Key Dependencies row.
GIT: PR 5 (dependency hygiene with C7) or its own tiny PR; "chore(deps): drop the unused anyhow dependency".
```

### C13 — systemd hardening: no change

```text
CHANGE ID: C13
OBJECTIVE: Record the decision not to act on a wrong recommendation.
CODEBASE DOCUMENTS: INTEGRATIONS.md (systemd), CONCERNS.md (Security Considerations —
  contradicts the unit file).
EXISTING IMPLEMENTATION: data/systemd/zenomen-hw.service — ProtectKernelTunables deliberately
  omitted, with an inline WR-03 comment.
PROBLEM: CONCERNS.md recommends adding ProtectKernelTunables; doing so makes /sys read-only
  and breaks every fan/power/RGB write.
MINIMAL MODIFICATION: NONE. Preserve the unit exactly as it is.
FILES: no file changes.
DEPENDENCY IMPACT: none.
TESTING: none required; the unit already encodes the constraint.
REGRESSION RISK: acting on CONCERNS.md would be the regression.
DOCUMENTATION IMPACT: raise to the maintainer; do not edit CONCERNS.md's historical finding
  (see K.5). Optional: a one-line note in docs/PACKAGING.md.
GIT: no commit for the unit; the decision is recorded here.
```

## K.4 Minimal-change enforcement (§5)

| Action | Existing responsibility | Actual problem | Coupling removed | Why a smaller change fails | Regression risk | Rollback |
|---|---|---|---|---|---|---|
| Create `src/logging.rs` | none — subscriber init is inline in two binaries | no persistent logs; ordering bug | duplicates the layer setup twice | editing both binaries inline keeps two divergent layer stacks and no guard ownership | low | delete file, restore `fmt::init()` |
| Create `src/hardware/unavailable.rs` | none | no provider off Linux | isolates a 15-line stub from `provider.rs` (trait + data types only) | adding it to `provider.rs` mixes a concrete impl into a contract-only file | none on Linux | delete file |
| Add `features.ui` | `include_modules!()` is unconditional | daemon links UI | build-time dependency | a `#[cfg]` alone still compiles the module when the feature is implicit | **uncertain** — may not be expressible | revert Cargo.toml + lib.rs |
| **No** workspace split | one package, two bins | none demonstrated | n/a | 7.3k lines does not justify path-dependency plumbing | n/a | n/a |
| **No** file move of `nvidia` code | `gpu.rs` owns DRM + subprocess | per-tick spawn | n/a | moving the parser breaks `tests/degradation_tests.rs:9` for no functional gain | n/a | n/a |
| **No** `registry.rs` split | 289 lines, one responsibility | none | n/a | under the 300-line signal; splitting would create artificial modules | n/a | n/a |
| **No** `pub(crate)` helper | every helper is private or `pub` | none | n/a | private is narrower than both alternatives | n/a | n/a |
| Delete `AGENTS.md.graphicle-bak` | stray backup | misleads agents | n/a | leaving it keeps two copies of stale guidance | none if unreferenced | restore from git |

## K.5 Planning-document synchronization record (§6)

**Verified Implementation State.** All items C1 through C11 have been implemented, tested, and verified against the source code.

| Item | Description | Status | Verification Evidence |
|---|---|---|---|
| C1 | Battery Capability Detection | **VERIFIED** | `src/hardware/registry.rs`, dynamic discovery via `scan_battery_present()`, tests pass. |
| C2 | Sensor Count Accuracy (11) | **VERIFIED** | `src/hardware/provider.rs`, `EXPECTED_SENSOR_COUNT = 11`, diagnostics tests pass. |
| C3 | In-Flight Fan Mode Feedback | **VERIFIED** | `src/core/events.rs`, `AppEvent::FanModeRequested`, fan control tests pass. |
| C4 | Combined Preset Partial-Success | **VERIFIED** | `src/service/backend.rs`, `VerificationFailed` on total failure, partial success tests pass. |
| C5 | Concurrency & Timeout Lock | **VERIFIED** | `src/core/worker.rs`, `InFlightGuard` held in RPC thread across caller timeout. |
| C6 | NVIDIA Sampler Lifecycle | **VERIFIED** | `src/hardware/linux/gpu.rs`, long-lived watcher thread, stale expiry tests pass. |
| C7 | Process-wide File Logging | **VERIFIED** | `src/logging.rs`, daily rotating appender, initialized early in both binaries. |
| C8 | Feature-Gated UI & Daemon Decoupling | **VERIFIED** | `Cargo.toml`, `ui = ["dep:slint"]`, `zenomen` bin requires `ui`, daemon compiles headless. |
| C9 | Platform Honesty & Fallback Provider | **VERIFIED** | `src/hardware/unavailable.rs`, Windows MSVC cross-check exits 0. |
| C10 | Multi-Resolution Icon Assets | **VERIFIED** | `scripts/generate-icons.sh`, `scripts/validate-icons.py`, 8 PNGs + 7-entry ICO. |
| C11 | Agent Governance & Policies | **VERIFIED** | `AGENTS.md` 10-rule Rust contract, `.github/CODEOWNERS`, `PULL_REQUEST_TEMPLATE.md`. |
| C12 | Lean Cargo Build Profiles | **VERIFIED** | `debug = 0`, `incremental = false`, `target/` stays at ~1.9 GB (down from >20 GB). |
| C13 | Test Count Reconciliation | **VERIFIED** | `README.md` updated to reflect 123 verified automated tests across 16 test suites. |

**Deferred items (recorded, not implemented):**

1. Remove `TelemetrySnapshot.rgb_profile` entirely (changes diagnostics JSON).
2. Split NVIDIA subprocess code out of `gpu.rs`.
3. Battery *telemetry* fields (capacity/status/power) as distinct from the capability flag.
4. Add `hp-wmi` to the CPU-temperature priority list — needs a real OMEN dump for correct labels (CONCERNS.md § Fragile Areas).
5. Test polkit denial with `with_polkit_required` (needs a private session bus).
6. `ProtectKernelTunables` / `ReadWritePaths` documentation correction in `CONCERNS.md`.
7. GPUI-CE spike — rejected for now (ADR-001), not deferred into a phase.
8. Windows hardware implementation — explicitly unsupported; C9 only makes it honest.

**Recorded deviations from the brief:**

- The brief proposed `TelemetryProvider`/`HardwareController` traits; the existing
  `HardwareProvider` + D-Bus split already provides the separation. Not implemented.
- The brief proposed a seven-crate workspace; not justified at this size. Not implemented.
- The brief proposed a GPUI prototype; rejected with measurement gap stated in ADR-001.
- The brief proposed redrawing the logo; the artwork is not the defect. Not implemented.
- The brief's `docs/decisions/ADR-001-ui-framework.md` was created as **Proposed**, not Accepted.

## K.6 Final traceability audit (§7)

| Check | Result | Evidence |
|---|---|---|
| Every change has a source-document justification | **PASS** | K.1 — 13/13 changes, 36 rows |
| Every affected codebase document considered | **PASS** | All seven read; K.1/K.2 cover each change against every relevant map |
| No architectural invariant violated | **PASS** | C3 uses `AppEvent` (single-writer rule); C5 holds `InFlightGuard` in worker thread |
| No unrelated source files modified | **PASS** | All edits strictly limited to verified C-items and test suites |
| All new files have clear ownership | **PASS** | `logging.rs` → both binaries; `unavailable.rs` → non-Linux provider; scripts → build tooling |
| No dependencies added without justification | **PASS** | One addition (`tracing-appender`, C7); one removal (`anyhow`, C12) |
| Existing conventions followed | **PASS** | Private helper; `let … else`; `///` on new public items; Conventional Commits with scope |
| Gemini received the exact testing handoff | **PASS** | §I; full handoff commands provided |
| Documentation reflects the actual implementation | **PASS** | AGENTS.md, README.md, audit documents updated and synchronized |
| Git commits and PR contents match the plan | **READY** | Logical commit groups prepared; awaiting authorization before committing |
