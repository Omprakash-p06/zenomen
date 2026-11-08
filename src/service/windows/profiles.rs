//! Motherboard profile catalog for HP OMEN and Victus laptops (WIN-07).

use serde::{Deserialize, Serialize};

/// Motherboard profile containing validated firmware capabilities.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BoardProfile {
    /// Motherboard product identifier (e.g. "88F8", "8A24").
    pub board_id: String,
    /// Marketing or platform family name.
    pub model_family: String,
    /// Number of physical fans.
    pub fan_count: u8,
    /// Whether firmware command 0x27 (Max Fan) is supported.
    pub has_max_fan: bool,
    /// Whether Victus S custom fan curve tables are supported.
    pub has_custom_curves: bool,
    /// Supported ACPI platform power profiles.
    pub supported_power_profiles: Vec<String>,
    /// Number of addressable RGB lighting zones.
    pub rgb_zone_count: u8,
}

/// Hardware verification evidence record for HP OMEN / Victus motherboards (WIN-07).
///
/// Firmware mutations (fan, power, RGB) require an explicit validation record matching
/// both `board_id` and `bios_version`. Without verified flags, mutations are blocked
/// and the board is restricted to read-only operation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BoardValidation {
    pub board_id: String,
    pub bios_version: String,
    pub fan_read_verified: bool,
    pub fan_write_verified: bool,
    pub power_read_verified: bool,
    pub power_write_verified: bool,
    pub rgb_read_verified: bool,
    pub rgb_write_verified: bool,
    pub validation_record_version: u32,
}

impl BoardValidation {
    /// Create an unvalidated baseline record for a discovered board and BIOS.
    /// All write capabilities default to false until validated on physical hardware.
    pub fn new_unvalidated(board_id: &str, bios_version: &str) -> Self {
        Self {
            board_id: board_id.trim().to_uppercase(),
            bios_version: bios_version.trim().to_string(),
            fan_read_verified: false,
            fan_write_verified: false,
            power_read_verified: false,
            power_write_verified: false,
            rgb_read_verified: false,
            rgb_write_verified: false,
            validation_record_version: 1,
        }
    }

    /// Create a fully verified validation record for physical test benches.
    pub fn new_verified(board_id: &str, bios_version: &str) -> Self {
        Self {
            board_id: board_id.trim().to_uppercase(),
            bios_version: bios_version.trim().to_string(),
            fan_read_verified: true,
            fan_write_verified: true,
            power_read_verified: true,
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
            supported_power_profiles: vec![
                "quiet".to_string(),
                "balanced".to_string(),
                "performance".to_string(),
            ],
            rgb_zone_count: 4,
        }),
        "8A3E" => Some(BoardProfile {
            board_id: "8A3E".to_string(),
            model_family: "HP Victus 15/16 (AMD/Intel)".to_string(),
            fan_count: 2,
            has_max_fan: true,
            has_custom_curves: false,
            supported_power_profiles: vec!["quiet".to_string(), "balanced".to_string()],
            rgb_zone_count: 1,
        }),
        "88D0" => Some(BoardProfile {
            board_id: "88D0".to_string(),
            model_family: "HP OMEN 15 (2020 Intel)".to_string(),
            fan_count: 2,
            has_max_fan: true,
            has_custom_curves: false,
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
