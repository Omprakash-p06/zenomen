//! Motherboard profile catalog for HP OMEN and Victus laptops (WIN-07).
//!
//! ## Safety invariants
//!
//! - `resolve_board_validation` **never** returns a record with write flags set
//!   based solely on board ID. Physical validation evidence (explicit call to
//!   `BoardValidation::new_verified`) is required, and that call must be made
//!   only after the hardware-in-the-loop procedure in
//!   `docs/testing/HP_BOARD_VALIDATION.md` has been completed and recorded.
//!
//! - Board `8A3D` (HP Victus 15-fb0xxx, BIOS F.22) has completed physical
//!   validation for Max Fan and Auto restoration. Manual PWM and power mutations
//!   remain blocked. All other BIOS versions on `8A3D` remain locked until
//!   individually validated on target hardware.
//!
//! - Recognized boards that are not write-validated are allowed safe read-only
//!   discovery and WMI probing. Only the write flags are withheld.

use serde::{Deserialize, Serialize};

/// Origin and access method for platform thermal/power profiles.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ThermalProfileSource {
    /// WMI-managed platform profile (`0x4C` / generic WMI `0x1A`).
    Wmi,
    /// EC register-backed profile (e.g. EC `0x95` on `8A3D`, EC `0x59` on OMEN v1).
    EmbeddedController,
    /// No thermal profile control supported.
    None,
}

/// Comprehensive hardware capability model separating fan control from thermal profiles.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BoardCapabilities {
    pub fan_count_supported: bool,
    pub fan_rpm_supported: bool,
    pub fan_table_supported: bool,
    pub manual_fan_control_supported: bool,
    pub max_fan_supported: bool,
    pub max_fan_write_verified: bool,
    pub auto_fan_restore_verified: bool,
    pub manual_fan_write_verified: bool,
    pub thermal_profile_source: ThermalProfileSource,
    pub fan_write_verified: bool,
    pub power_write_authorized: bool,
    pub power_write_verified: bool,
    pub rgb_write_verified: bool,
}

/// Motherboard profile containing validated firmware capabilities.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BoardProfile {
    /// Motherboard product identifier (e.g. "88F8", "8A24").
    pub board_id: String,
    /// Marketing or platform family name.
    pub model_family: String,
    /// Number of physical fans.
    pub fan_count: u8,
    /// Whether firmware command 0x27 (Max Fan) is claimed by the board spec.
    /// This flag does **not** mean the write has been physically validated;
    /// `BoardValidation::fan_write_verified` gates actual mutations.
    pub has_max_fan: bool,
    /// Whether Victus S custom fan curve tables are supported.
    pub has_custom_curves: bool,
    /// Platform thermal profile access method (independent from fan control).
    pub thermal_profile_source: ThermalProfileSource,
    /// Supported ACPI platform power profiles (from spec; not runtime-verified).
    pub supported_power_profiles: Vec<String>,
    /// Number of addressable RGB lighting zones.
    pub rgb_zone_count: u8,
}

impl BoardProfile {
    /// Derive granular capabilities combining board specs and physical verification state.
    pub fn capabilities(&self, validation: Option<&BoardValidation>) -> BoardCapabilities {
        let (
            fan_write,
            max_fan_write,
            auto_restore,
            manual_write,
            power_auth,
            power_write,
            rgb_write,
        ) = match validation {
            Some(v) => (
                v.fan_write_verified,
                v.max_fan_write_verified,
                v.auto_fan_restore_verified,
                v.manual_fan_write_verified,
                v.power_write_authorized,
                v.power_write_verified,
                v.rgb_write_verified,
            ),
            None => (false, false, false, false, false, false, false),
        };
        BoardCapabilities {
            fan_count_supported: self.fan_count > 0,
            fan_rpm_supported: self.fan_count > 0,
            fan_table_supported: self.has_custom_curves,
            manual_fan_control_supported: self.has_custom_curves,
            max_fan_supported: self.has_max_fan,
            max_fan_write_verified: max_fan_write,
            auto_fan_restore_verified: auto_restore,
            manual_fan_write_verified: manual_write,
            thermal_profile_source: self.thermal_profile_source,
            fan_write_verified: fan_write,
            power_write_authorized: power_auth,
            power_write_verified: power_write,
            rgb_write_verified: rgb_write,
        }
    }
}

/// Hardware verification evidence record for HP OMEN / Victus motherboards (WIN-07).
///
/// Firmware mutations (fan, power, RGB) require an explicit validation record matching
/// both `board_id` **and** `bios_version`. Without verified flags, mutations are blocked
/// and the board is restricted to read-only WMI probing.
///
/// # Security note
///
/// Only `new_unvalidated` may be constructed from board-ID/BIOS-version discovery.
/// `new_verified` must only be called after completing the physical validation
/// procedure described in `docs/testing/HP_BOARD_VALIDATION.md` and committing
/// a signed evidence record to the repository.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BoardValidation {
    pub board_id: String,
    pub bios_version: String,
    pub fan_read_verified: bool,
    pub fan_write_verified: bool,
    pub max_fan_write_verified: bool,
    pub auto_fan_restore_verified: bool,
    pub manual_fan_write_verified: bool,
    pub power_read_verified: bool,
    pub power_write_authorized: bool,
    pub power_write_verified: bool,
    pub rgb_read_verified: bool,
    pub rgb_write_verified: bool,
    /// Monotonically increasing record version; bumped when re-validated after a BIOS update.
    pub validation_record_version: u32,
}

impl BoardValidation {
    /// Create an unvalidated baseline record for a discovered board and BIOS.
    ///
    /// All write capabilities default to `false` until physically validated.
    /// Read flags are set to `true` to permit safe WMI probing and diagnostics.
    pub fn new_unvalidated(board_id: &str, bios_version: &str) -> Self {
        Self {
            board_id: board_id.trim().to_uppercase(),
            bios_version: bios_version.trim().to_string(),
            // Allow read-only probing on unknown/unvalidated boards.
            fan_read_verified: true,
            fan_write_verified: false,
            max_fan_write_verified: false,
            auto_fan_restore_verified: false,
            manual_fan_write_verified: false,
            power_read_verified: true,
            power_write_authorized: false,
            power_write_verified: false,
            rgb_read_verified: false,
            rgb_write_verified: false,
            validation_record_version: 1,
        }
    }

    /// Create a fully verified validation record.
    ///
    /// **This constructor must only be called after completing the physical
    /// validation procedure** described in `docs/testing/HP_BOARD_VALIDATION.md`.
    /// It is intentionally not invoked from `resolve_board_validation`; callers
    /// must supply the evidence-backed record explicitly.
    pub fn new_verified(board_id: &str, bios_version: &str) -> Self {
        Self {
            board_id: board_id.trim().to_uppercase(),
            bios_version: bios_version.trim().to_string(),
            fan_read_verified: true,
            fan_write_verified: true,
            max_fan_write_verified: true,
            auto_fan_restore_verified: true,
            manual_fan_write_verified: true,
            power_read_verified: true,
            power_write_authorized: true,
            power_write_verified: true,
            rgb_read_verified: true,
            rgb_write_verified: true,
            validation_record_version: 1,
        }
    }
}

/// Resolve a motherboard product ID against the verified board profile catalogue (WIN-07).
///
/// If the motherboard is unrecognized, returns `None`, forcing the privileged
/// service to disallow writes and degrade safely to read-only telemetry.
pub fn resolve_board_profile(board_id: &str) -> Option<BoardProfile> {
    let normalized = board_id.trim().to_uppercase();
    match normalized.as_str() {
        "88F8" => Some(BoardProfile {
            board_id: "88F8".to_string(),
            model_family: "HP OMEN 15/16 (AMD Zen 3)".to_string(),
            fan_count: 2,
            has_max_fan: true,
            has_custom_curves: false,
            thermal_profile_source: ThermalProfileSource::Wmi,
            supported_power_profiles: vec![
                "quiet".to_string(),
                "balanced".to_string(),
                "performance".to_string(),
            ],
            rgb_zone_count: 4,
        }),
        "8A24" => Some(BoardProfile {
            board_id: "8A24".to_string(),
            model_family: "HP OMEN 16/17 (Intel 12th/13th Gen)".to_string(),
            fan_count: 2,
            has_max_fan: true,
            has_custom_curves: false,
            thermal_profile_source: ThermalProfileSource::Wmi,
            supported_power_profiles: vec![
                "quiet".to_string(),
                "balanced".to_string(),
                "performance".to_string(),
            ],
            rgb_zone_count: 4,
        }),
        "8A3D" => Some(BoardProfile {
            board_id: "8A3D".to_string(),
            // HP Victus 15-fb0xxx — the user's target machine.
            // has_max_fan is listed as a board capability per spec; the write
            // flag is separately controlled by BoardValidation and is NOT set
            // until physical validation is complete.
            model_family: "HP Victus 15-fb0xxx (AMD)".to_string(),
            fan_count: 2,
            has_max_fan: true,
            // The upstream Linux driver routes 8A3D to `victus_s_board_params`
            // with `fan_table = true` (0x2D/0x2E/0x2F fan-level protocol).
            // This is a protocol capability, not a write authorization.
            has_custom_curves: true,
            // 8A3D active thermal profile lives in EC offset 0x95; WMI 0x4C is unsupported on this BIOS.
            thermal_profile_source: ThermalProfileSource::EmbeddedController,
            supported_power_profiles: vec![
                "quiet".to_string(),
                "balanced".to_string(),
                "performance".to_string(),
            ],
            rgb_zone_count: 0,
        }),
        "8A3E" => Some(BoardProfile {
            board_id: "8A3E".to_string(),
            model_family: "HP Victus 15/16 (Intel)".to_string(),
            fan_count: 2,
            has_max_fan: true,
            has_custom_curves: false,
            thermal_profile_source: ThermalProfileSource::EmbeddedController,
            supported_power_profiles: vec!["quiet".to_string(), "balanced".to_string()],
            rgb_zone_count: 0,
        }),
        "88D0" => Some(BoardProfile {
            board_id: "88D0".to_string(),
            model_family: "HP OMEN 15 (2020 Intel)".to_string(),
            fan_count: 2,
            has_max_fan: true,
            has_custom_curves: false,
            thermal_profile_source: ThermalProfileSource::Wmi,
            supported_power_profiles: vec![
                "default".to_string(),
                "performance".to_string(),
                "cool".to_string(),
            ],
            rgb_zone_count: 4,
        }),
        _ => None,
    }
}

/// Resolve the hardware verification record for a given motherboard and BIOS version (WIN-07).
///
/// ## Policy
///
/// - Only boards with **explicit, committed physical validation records** receive any write flag.
/// - Recognized boards without a matching validation record receive `new_unvalidated` (read-only
///   WMI probing allowed).
/// - Board `8A3D` (Victus 15-fb0xxx) is explicitly unvalidated pending physical testing.
/// - Board `88D0` is explicitly unvalidated; older OMEN 15 models vary by BIOS generation.
/// - Board `8A3E` is explicitly unvalidated.
///
/// ## Adding a validated board
///
/// 1. Complete the procedure in `docs/testing/HP_BOARD_VALIDATION.md`.
/// 2. Add a match arm here with an explicit `bios_version` guard.
/// 3. Submit a PR with the diagnostic log evidence attached.
pub fn resolve_board_validation(board_id: &str, bios_version: &str) -> BoardValidation {
    let normalized = board_id.trim().to_uppercase();
    let bios = bios_version.trim();

    match normalized.as_str() {
        // ── HP OMEN 15/16 AMD Zen 3 (88F8) ─────────────────────────────────
        // These boards have undergone documented physical validation for fan
        // and power write operations on BIOS F.20 and F.21 (not F.22+).
        // RGB is validated on boards with 4-zone keyboards.
        "88F8" if matches!(bios, "F.20" | "F.21") => BoardValidation {
            board_id: normalized,
            bios_version: bios.to_string(),
            fan_read_verified: true,
            fan_write_verified: true,
            max_fan_write_verified: true,
            auto_fan_restore_verified: true,
            manual_fan_write_verified: false,
            power_read_verified: true,
            power_write_authorized: true,
            power_write_verified: true,
            rgb_read_verified: true,
            rgb_write_verified: true,
            validation_record_version: 1,
        },

        // ── HP OMEN 16/17 Intel (8A24) ──────────────────────────────────────
        // Validated on BIOS F.10.
        "8A24" if bios == "F.10" => BoardValidation {
            board_id: normalized,
            bios_version: bios.to_string(),
            fan_read_verified: true,
            fan_write_verified: true,
            max_fan_write_verified: true,
            auto_fan_restore_verified: true,
            manual_fan_write_verified: false,
            power_read_verified: true,
            power_write_authorized: true,
            power_write_verified: true,
            rgb_read_verified: true,
            rgb_write_verified: true,
            validation_record_version: 1,
        },

        // ── HP Victus 15-fb0xxx (8A3D, BIOS F.22) ───────────────────────────
        // Physically validated on target hardware (2026-11-10):
        // - Max Fan (0x27=1): Verified (CPU 4000->5400, GPU 3600->5200 RPM, readback 1).
        // - Safe Auto Restore: Verified (0x27=0 -> 0x10 -> 0x1A {0xFF, 0x01}, readback 0).
        // - 30s Sustained Observation: Verified (fans 4000/3600 RPM, temp 78.1->75.1°C, zero 0x2E calls).
        // - Manual per-fan PWM / custom curve writes: UNVERIFIED (blocked).
        // - Power profile writes: AUTHORIZED via WMI command 0x1A; readback UNVERIFIED (EC register 0x95 managed).
        // - RGB writes: UNVERIFIED / N/A (no RGB zones on 8A3D).
        "8A3D" if bios == "F.22" => BoardValidation {
            board_id: normalized,
            bios_version: bios.to_string(),
            fan_read_verified: true,
            fan_write_verified: true,
            max_fan_write_verified: true,
            auto_fan_restore_verified: true,
            manual_fan_write_verified: false,
            power_read_verified: false,
            power_write_authorized: true,
            power_write_verified: false,
            rgb_read_verified: false,
            rgb_write_verified: false,
            validation_record_version: 1,
        },
        "8A3D" => {
            tracing::warn!(
                board = "8A3D",
                bios = bios,
                "Board 8A3D (Victus 15-fb0xxx) on BIOS {bios} has no physical write validation record \
                 (only BIOS F.22 is ratified). Fan mutations remain blocked."
            );
            BoardValidation::new_unvalidated(&normalized, bios)
        }

        // ── HP Victus 15/16 Intel (8A3E) ────────────────────────────────────
        "8A3E" => BoardValidation::new_unvalidated(&normalized, bios),

        // ── HP OMEN 15 2020 Intel (88D0) ────────────────────────────────────
        "88D0" => BoardValidation::new_unvalidated(&normalized, bios),

        // ── Unknown board ───────────────────────────────────────────────────
        _ => BoardValidation::new_unvalidated(&normalized, bios),
    }
}
