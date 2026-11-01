# Plan 04-03: Basic RGB — Detection, Presets, Safe Migration & Hardware Writes Summary

**Status:** Completed
**Commit:** `(Pending commit)`

## Overview

Plan 04-03 completes the RGB lighting subsystem for HP OMEN / Victus laptops, providing hardware variant discovery, strict hex color validation, non-destructive migration of legacy OpenOMEN profiles, built-in presets, D-Bus service hardware writes with read-back verification, and an interactive 4-zone Lighting screen in the Slint GUI.

## Delivered Artifacts

1. **`src/rgb/schema.rs`**:
   - `RgbColor` with strict `#RRGGBB` / `RRGGBB` hex parsing and validation (`RgbColorError`), returning user-friendly messages for invalid lengths or non-hex characters (RGB-06).
   - `RgbProfile` supporting 4 keyboard zones and explicit tracking of unsupported animation fields (`brightness`, `speed`, `length_ms`, `loop`) without failing profile imports (RGB-07).
2. **`src/rgb/migration.rs`**:
   - Safe migration engine importing legacy OpenOMEN profiles (`~/.config/openomen/rgb/*.json`) into ZenOMEN format (`~/.config/zenomen/rgb/<id>.json`) strictly in read-only mode, guaranteeing source files are never deleted, overwritten, or mutated (RGB-05).
   - Built-in static default presets: `White`, `Gaming`, `Rainbow`, and `Off` (RGB-03).
3. **`src/hardware/rgb.rs` & `src/hardware/registry.rs`**:
   - Detection of all 3 Linux kernel sysfs interface variants (RGB-01):
     - `FourZone`: `/sys/devices/platform/hp-wmi/rgb_zones/zone0{0..3}`
     - `Legacy`: `/sys/devices/platform/hp-wmi/keyboardleds`
     - `KernelPatch`: `/sys/class/leds/hp_omen::kbd_backlight/zone_colors`
   - Integrated into `HardwareCapabilities.rgb_variant`.
4. **`src/service/backend.rs`, `contract.rs`, & `client.rs`**:
   - Multi-variant sysfs writes in privileged `zenomen-hw` backend with read-back verification (RGB-08):
     - `FourZone`: Hex writes to each zone file with verification.
     - `Legacy`: 24-char concatenated hex string write.
     - `KernelPatch`: 12-byte raw binary array `[r0, g0, b0, ...]` write with verification.
   - Privileged D-Bus methods `ApplyRgbProfile` and `SetRgbEnabled` on `com.zenomen.Hardware1`.
5. **`ui/screens/lighting.slint` & `ui/app.slint`**:
   - 4-zone visual keyboard preview with dynamic zone colors (RGB-02).
   - Preset selector buttons (`White`, `Gaming`, `Rainbow`, `Off`).
   - Lighting power toggle switch.
   - Interface variant badge (`Four-Zone`, `Legacy`, `Kernel Patch`).
   - Unsupported animations explanatory pill noting animation effects are deferred to v2 (RGB-07).
   - Honest unavailable fallback panel when no RGB interface is detected.
6. **`tests/rgb_tests.rs`**:
   - 8 integration tests covering hex validation, interface discovery across all 3 variants, non-destructive migration, and mock hardware writes.

## Verification

- `cargo clippy --all-targets -- -D warnings`: 0 warnings.
- `cargo test --all`: 88 passed across all unit and integration test suites.
