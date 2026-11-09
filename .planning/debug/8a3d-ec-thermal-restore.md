---
status: resolved
trigger: "Audit request: upstream Linux patch 2026-10-24 says board 8A3D now uses an EC thermal profile, and that the generic fan reset can leave fans at 0 RPM under load. Verify ZenOmen's restore path against it."
created: 2026-11-09
updated: 2026-11-09
goal: find_and_fix
---

# 8A3D: EC thermal profile and safe automatic-cooling restore

## Symptoms

- Reported hazard: on board `8A3D`, disabling Max Fan with the generic Victus S
  reset (`0x27 = 0` + `0x2E {0,0}`) can leave both fans frozen at 0 RPM under load.
- `set_fan_mode("auto")` on a Victus S board uses exactly that sequence.
- `SetFanMode { mode: "auto" }` is authorized even when
  `validation.fan_write_verified == false` (only `"max"` is gated), so the
  sequence is reachable on an unvalidated board.

## Current Focus

- hypothesis: confirmed — our auto restore sends `0x2E {0,0}` on `8A3D`.
- next_action: replace the `8A3D` restore with Max-Fan clear + EC thermal-profile
  re-apply (`0x1A`), and refuse when the active profile is unknown.

## Evidence

- 2026-11-09: fetched the reported patch
  `https://lists.openwall.net/linux-kernel/2026/10/24/810`
  (`[PATCH] platform/x86: hp-wmi: fix platform profile values for Victus 15-fb0xxx (8A3D)`,
  Sourav Nayak, Message-ID `<20261024202658.13149-1-nonameblank007@gmail.com>`,
  base-commit `5b0764a53235d3c77e1ed674e836e1b3fe1a4b1f`). The page exists and its
  content matches the audit's description.
- 2026-11-09: vendored master
  (`.planning/references/hp-wmi-upstream.c`) still routes `8A3D` to
  `victus_s_board_params` → `victus_s_thermal_params`, whose
  `.ec_tp_offset = HP_EC_OFFSET_UNKNOWN`. So the patch is **posted, not merged**.
- Patch content (verbatim facts):
  - adds `victus_s_ec_thermal_params` with
    `.ec_tp_offset = HP_OMEN_EC_THERMAL_PROFILE_OFFSET`, i.e. **`0x95`** (not `0x59`);
  - adds `victus_s_ec_board_params` and routes only the `8A3D` DMI entry to it;
  - maps `tp == HP_VICTUS_THERMAL_PROFILE_QUIET (0x03)` to `PLATFORM_PROFILE_LOW_POWER`
    on readback, so `8A3D` values are balanced `0x00`, performance `0x01`, quiet `0x03`;
  - in `hp_wmi_apply_fan_settings` (`PWM_MODE_AUTO`), for
    `hp_wmi_thermal_profile() == &victus_s_ec_thermal_params` it replaces
    `hp_wmi_fan_speed_max_reset()` with `hp_wmi_fan_speed_max_set(0)` followed by
    `platform_profile_victus_s_set_ec(active_platform_profile)`;
  - commit message: "hp_wmi_fan_speed_max_reset() caused fans to freeze during
    loads at 0RPM".
- `platform_profile_victus_s_set_ec` order (master): `0x10` fan-count trigger →
  `omen_thermal_profile_set(tp)` (`0x1A`, payload `{0xFF, tp}`) → GPU thermal
  modes (`0x22`). It never sends `0x2E`.
- `0x59` is `HP_VICTUS_S_EC_THERMAL_PROFILE_OFFSET`, used by
  `omen_v1_thermal_params`; it is **not** the `8A3D` offset. Our earlier comment
  claiming "Victus S thermal profile is EC 0x59" was wrong.
- Repo state: `hp_wmi::set_fan_mode("auto")` → `set_victus_fan_levels(0, 0)` →
  `0x10`, `0x27 = 0`, `0x2F`, `0x2E {0,0}`. `authorization::authorize_mutation`
  returns `Ok(())` for `"auto"` without checking `fan_write_verified`.

## Eliminated

- hypothesis: "the patch re-applies the profile through the EC."
  Rejected: the patch *reads* the profile from EC `0x95` but **writes** it with
  WMI `0x1A` (`omen_thermal_profile_set`). There is no EC write in the path.
- hypothesis: "0x59 is the 8A3D EC offset." Rejected by the patch diff.

## Resolution

- **Root cause:** `set_fan_mode("auto")` used the generic Victus S second step
  (`0x2E {0,0}`) on `8A3D` — the exact sequence the 2026-10-24 patch identifies as freezing
  the fans — and the path was reachable because `SetFanMode { auto }` is authorized without
  `fan_write_verified`.
- **Fix:**
  - `victus_s.rs`: board-specific facts (`EC_THERMAL_PROFILE_OFFSET = 0x95`,
    balanced/performance/quiet `0x00`/`0x01`/`0x03`, `ThermalProfile`,
    `requires_thermal_profile_restore`, `thermal_profile_payload`).
  - `hp_wmi::restore_automatic_fan_control` replaces the auto branch: `8A3D` clears Max Fan,
    triggers `0x10`, re-applies `0x1A {0xFF, profile}`, and never sends `0x2E`; without a
    known profile it returns `UnsupportedMode` **before** any write. Other Victus S boards
    keep `0x2E {0,0}`; other boards keep `0x27 = 0`.
  - `read_performance_mode_from_firmware` no longer claims EC `0x59` for Victus S.
- **Tests:** unit tests in `victus_s.rs` covering fan table parsing and `gpu_offset` from first non-zero GPU row; 70 tests in `tests/windows_service_tests.rs` (refusal, profile re-apply, failed Max Fan clear, unknown profile writes nothing, other Victus S boards, generic board, request-path reachability, fan telemetry, fan table parsing, guarded fan control transitions, unverified write refusal, lifecycle shutdown skipping unverified writes).
- **Verification:** `cargo fmt --all -- --check` clean; `cargo clippy --all-targets --all-features -- -D warnings` clean; `cargo test --all` 233 passed / 0 failed (`windows_service_tests` 70/70).
- **Closure:**
  - `authorize_mutation` now strictly enforces `val.fan_write_verified` for ALL `SetFanMode` requests, including `"auto"`. Unvalidated boards (`8A3D`) return `HardwareControlError::UnsupportedMode` before making any adapter or firmware calls.
  - Opcode `0x4C` (`HPWMI_THERMAL_PROFILE_QUERY`) candidate probe added to `hp_wmi::probe_thermal_profile_query` and `--wmi-diag`. It remains diagnostic-only and never authorizes writes.
  - `restore_automatic_fan_control` failure semantics are fully tested (Max Fan clear failure halts, arm failure halts, profile write failure propagated, missing profile refused).
  - `fan_write_verified` remains strictly `false` for `8A3D` / BIOS `F.22`. No writes are permitted on physical hardware.
  - Upstream patch 2026-10-24 is marked as proposed / unmerged.
