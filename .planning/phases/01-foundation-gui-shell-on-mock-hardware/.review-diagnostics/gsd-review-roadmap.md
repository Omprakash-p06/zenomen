### Phase 1: Foundation — GUI Shell on Mock Hardware
**Goal**: A runnable ZenOmen application whose entire UI is driven by mock hardware data, with the resource instrumentation needed to hold the 40 MB budget later.
**Mode:** mvp
**Depends on**: Nothing (first phase)
**UI hint**: yes
**Requirements**: FOUND-01, FOUND-02, FOUND-03, FOUND-04, FOUND-05, MON-09, PERF-03
**Success Criteria** (what must be TRUE):
  1. Launching the app opens the Overview page in the ZenOmen dark theme, and all six screens are reachable by sidebar click and by keyboard alone.
  2. The Overview shows plausible CPU, GPU, RAM, fan and power readings sourced entirely from the mock provider, including a rolling chart fed by a fixed-capacity history buffer (120 samples).
  3. Running the app with a mock provider configured to report zero capabilities still launches and explains what is unsupported, with no panic and no blank panels.
  4. Window frame time stays stable while the monitoring worker publishes updates — no hardware read or external command is reachable from a UI callback.
  5. Memory and CPU instrumentation logs metrics distinguishing cold start, warm dashboard, minimized tray, and peak RSS; CI fails on a formatting violation, a clippy warning, a failing test, or a dependency advisory (`cargo audit`).
**Plans**: 3 plans

Plans:
- [ ] 01-01: Rust + Slint scaffold, build pipeline, theme tokens and CI gates (`fmt`, `clippy -D warnings`, `cargo audit`)
- [ ] 01-02: Application shell — navigation, six screens, Overview bound to core events (`AppCommand`/`AppEvent`)
- [ ] 01-03: Mock hardware provider, monitoring worker, bounded history (120 samples) and benchmark instrumentation

