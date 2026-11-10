//! HP Victus S board-specific fan protocol (motherboard `8A3D` and siblings).
//!
//! Mirrors the upstream Linux `hp-wmi` driver, which maps DMI board `8A3D` to
//! `victus_s_board_params` with a dedicated fan profile:
//!
//! - fan speed is read with command `0x2D` (coarse 100 RPM units), **not** `0x11`;
//! - manual levels are written with command `0x2E` in fan-table RPM units;
//! - the usable level range comes from the `0x2F` fan table.
//!
//! Evidence: `.planning/references/hp-wmi-upstream.c` (`victus_s_board_params`,
//! `hp_wmi_get_fan_speed_victus_s`, `hp_wmi_setup_fan_settings`,
//! `hp_wmi_fan_speed_set`) and `.planning/references/REFERENCES.md`.
//!
//! **Thermal profile.** Upstream patch 2026-10-24
//! (`lists.openwall.net/linux-kernel/2026/10/24/810`) routes board `8A3D` to
//! `victus_s_ec_board_params`: its active thermal profile lives in EC register
//! `0x95`, and the generic Victus S fan reset (`0x2E {0,0}`) freezes this
//! board's fans at 0 RPM under load. Automatic cooling is restored by clearing
//! Max Fan and re-applying the active profile through `0x1A` instead.
//!
//! This module is pure protocol logic: no COM, no firmware I/O. It is compiled on
//! every platform so the parsing rules stay unit-testable in CI.

use crate::service::types::HardwareControlError;

/// `HPWMI_VICTUS_S_FAN_SPEED_GET_QUERY` — per-fan speed, 128-byte response.
pub const FAN_LEVEL_GET_QUERY: u32 = 0x2D;

/// `HPWMI_VICTUS_S_FAN_SPEED_SET_QUERY` — two fan-level bytes, no response payload.
pub const FAN_LEVEL_SET_QUERY: u32 = 0x2E;

/// `HPWMI_VICTUS_S_GET_FAN_TABLE_QUERY` — 128-byte fan table.
pub const FAN_TABLE_QUERY: u32 = 0x2F;

/// `HP_FAN_SPEED_AUTOMATIC` — sentinel byte that returns a fan to firmware control.
pub const FAN_SPEED_AUTOMATIC: u8 = 0x00;

/// Each `0x2D` level byte represents this many RPM (`fan_data[fan] * 100`).
pub const RPM_PER_LEVEL_UNIT: u32 = 100;

/// Fan table header size: `{ num_fans: u8, unknown: u8 }`.
pub const FAN_TABLE_HEADER_LEN: usize = 2;

/// Fan table entry size: `{ cpu_rpm: u8, gpu_rpm: u8, noise_db: u8 }` (`__packed`).
pub const FAN_TABLE_ENTRY_LEN: usize = 3;

/// Largest response any Victus S fan query returns.
pub const MAX_RESPONSE_LEN: usize = 128;

/// Upstream refreshes manual fan state every 90 s because firmware resets it after 120 s.
pub const KEEPALIVE_INTERVAL_SECS: u64 = 90;

/// Firmware window before it reverts to its automatic fan curve.
pub const FIRMWARE_FAN_STATE_TIMEOUT_SECS: u64 = 120;

/// EC register holding board `8A3D`'s active thermal profile.
///
/// The 2026-10-24 upstream patch routes `8A3D` to `victus_s_ec_thermal_params`
/// with `.ec_tp_offset = HP_OMEN_EC_THERMAL_PROFILE_OFFSET` (`0x95`). Reading it
/// requires an EC driver, which a userland Windows process does not have.
pub const EC_THERMAL_PROFILE_OFFSET: u8 = 0x95;

/// Balanced / standard profile byte on the `8A3D` EC layout.
pub const THERMAL_PROFILE_BALANCED: u8 = 0x00;

/// Performance profile byte on the `8A3D` EC layout.
pub const THERMAL_PROFILE_PERFORMANCE: u8 = 0x01;

/// Quiet / eco profile byte on the `8A3D` EC layout
/// (`HP_VICTUS_THERMAL_PROFILE_QUIET`).
pub const THERMAL_PROFILE_QUIET: u8 = 0x03;

/// Leading byte of the `0x1A` payload (`omen_thermal_profile_set`).
pub const THERMAL_PROFILE_PREFIX: u8 = 0xFF;

/// Boards whose automatic cooling is restored through the EC thermal profile
/// rather than the Victus S fan-level sentinel. The patch routes only `8A3D` to
/// `victus_s_ec_board_params`.
pub const EC_THERMAL_PROFILE_BOARDS: &[&str] = &["8A3D"];

/// DMI board names routed to `victus_s_board_params` by the upstream driver.
///
/// Source: `hp_wmi_feature_boards` in `.planning/references/hp-wmi-upstream.c`.
pub const VICTUS_S_BOARDS: &[&str] = &[
    "8A3D", "8B2F", "8BBE", "8BD4", "8BD5", "8C99", "8C9C", "8DE1",
];

/// One fan-table row: the level range available at a given acoustic point.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FanTableEntry {
    /// CPU fan level in RPM units (`u8`, as stored in the firmware table).
    pub cpu_rpm: u8,
    /// GPU fan level in RPM units.
    pub gpu_rpm: u8,
    /// Reported noise level; not used for control.
    pub noise_db: u8,
}

/// Parsed Victus S fan table (`hpqBIOSInt128` response to command `0x2F`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FanTable {
    /// Fan count as reported by the table header.
    pub num_fans: u8,
    /// Lowest CPU level present in the table.
    pub min_rpm: u8,
    /// Highest CPU level present in the table — the ceiling for manual levels.
    pub max_rpm: u8,
    /// Offset between GPU fan and CPU fan (GPU RPM level - CPU RPM level) derived
    /// from the first row with non-zero GPU RPM. None if no rows have non-zero GPU RPM.
    pub gpu_offset: Option<i16>,
    /// Parsed rows, stopping at the first all-zero row (truncated tables).
    pub entries: Vec<FanTableEntry>,
}

impl FanTable {
    /// Highest level that firmware accepts for a manual fan write.
    pub fn max_level(&self) -> u8 {
        self.max_rpm
    }

    /// Offset between GPU fan and CPU fan, if available from a row with nonzero GPU RPM.
    pub fn gpu_offset(&self) -> Option<i16> {
        self.gpu_offset
    }

    /// Derive the expected GPU level for a given CPU level using the table's GPU offset.
    /// Clamps to 1..=255, or returns `level` if no offset is available.
    pub fn gpu_level_for_cpu(&self, cpu_level: u8) -> u8 {
        match self.gpu_offset {
            Some(offset) => {
                let target = (cpu_level as i16) + offset;
                target.clamp(1, u8::MAX as i16) as u8
            }
            None => cpu_level,
        }
    }
}

/// Return `true` when `board_id` is a board the upstream driver treats as Victus S.
pub fn is_victus_s_board(board_id: &str) -> bool {
    let normalized = board_id.trim().to_uppercase();
    VICTUS_S_BOARDS.iter().any(|board| *board == normalized)
}

/// The thermal profiles a Victus S EC exposes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThermalProfile {
    /// Balanced / standard (`0x00`).
    Balanced,
    /// Performance (`0x01`).
    Performance,
    /// Quiet / eco (`0x03`).
    Quiet,
}

impl ThermalProfile {
    /// Parse a user-facing profile name. `cool` is an OMEN profile and has no
    /// Victus S EC value, so it is rejected here.
    pub fn from_name(name: &str) -> Option<Self> {
        match name.trim().to_lowercase().as_str() {
            "balanced" | "default" | "standard" => Some(Self::Balanced),
            "performance" => Some(Self::Performance),
            "quiet" | "eco" => Some(Self::Quiet),
            _ => None,
        }
    }

    /// Canonical profile name.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Balanced => "balanced",
            Self::Performance => "performance",
            Self::Quiet => "quiet",
        }
    }

    /// Firmware byte written in the `0x1A` payload on board `8A3D`.
    pub fn firmware_value(self) -> u8 {
        match self {
            Self::Balanced => THERMAL_PROFILE_BALANCED,
            Self::Performance => THERMAL_PROFILE_PERFORMANCE,
            Self::Quiet => THERMAL_PROFILE_QUIET,
        }
    }
}

/// Return `true` when automatic cooling must be restored by re-applying the EC
/// thermal profile instead of sending the `0x2E` automatic-level sentinel.
pub fn requires_thermal_profile_restore(board_id: &str) -> bool {
    let normalized = board_id.trim().to_uppercase();
    EC_THERMAL_PROFILE_BOARDS
        .iter()
        .any(|board| *board == normalized)
}

/// `0x1A` payload: a leading `0xFF` followed by the profile byte.
pub fn thermal_profile_payload(profile: ThermalProfile) -> [u8; 2] {
    [THERMAL_PROFILE_PREFIX, profile.firmware_value()]
}

/// Convert a raw `0x2D` level byte into RPM.
pub fn rpm_for_level(level: u8) -> u32 {
    u32::from(level).saturating_mul(RPM_PER_LEVEL_UNIT)
}

/// Explicit state machine tracking the 8A3D physical validation lifecycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ValidationState {
    Idle,
    BaselineCaptured,
    MaxFanRequested,
    MaxFanConfirmed,
    FanAccelerationObserved,
    Restoring,
    RestorationCommandAccepted,
    ObservingAutomaticControl,
    RestorationVerified,
    RestorationUnverified,
    RestorationFailed,
}

impl ValidationState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Idle => "Idle",
            Self::BaselineCaptured => "BaselineCaptured",
            Self::MaxFanRequested => "MaxFanRequested",
            Self::MaxFanConfirmed => "MaxFanConfirmed",
            Self::FanAccelerationObserved => "FanAccelerationObserved",
            Self::Restoring => "Restoring",
            Self::RestorationCommandAccepted => "RestorationCommandAccepted",
            Self::ObservingAutomaticControl => "ObservingAutomaticControl",
            Self::RestorationVerified => "RestorationVerified",
            Self::RestorationUnverified => "RestorationUnverified",
            Self::RestorationFailed => "RestorationFailed",
        }
    }

    pub fn is_terminal(self) -> bool {
        matches!(
            self,
            Self::RestorationVerified | Self::RestorationUnverified | Self::RestorationFailed
        )
    }
}

/// Target thermal profile for 8A3D restoration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProfileRestoreTarget {
    /// Original profile is known with certainty.
    Known(ThermalProfile),
    /// Original profile was unknown; explicit safe fallback authorized.
    SafeFallback(ThermalProfile),
}

impl ProfileRestoreTarget {
    pub fn profile(self) -> ThermalProfile {
        match self {
            Self::Known(p) | Self::SafeFallback(p) => p,
        }
    }

    pub fn is_known(self) -> bool {
        matches!(self, Self::Known(_))
    }
}

impl From<ThermalProfile> for ProfileRestoreTarget {
    fn from(p: ThermalProfile) -> Self {
        ProfileRestoreTarget::Known(p)
    }
}

/// Atomically sampled dual fan tachometer readings.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DualFanTelemetry {
    pub cpu_rpm: u32,
    pub gpu_rpm: u32,
}

/// Parse raw 0x2D response into individual RPMs for a 2-fan Victus S system.
pub fn parse_dual_fan_levels(bytes: &[u8]) -> Result<DualFanTelemetry, HardwareControlError> {
    if bytes.len() < 2 {
        return Err(HardwareControlError::ProtocolError(format!(
            "Victus S fan level response too short: expected at least 2 bytes, got {}",
            bytes.len()
        )));
    }
    Ok(DualFanTelemetry {
        cpu_rpm: rpm_for_level(bytes[0]),
        gpu_rpm: rpm_for_level(bytes[1]),
    })
}

/// Parse raw 0x2D response for `fan_count` fans with strict length checking.
pub fn parse_fan_levels(bytes: &[u8], fan_count: u8) -> Result<Vec<u32>, HardwareControlError> {
    let expected = fan_count as usize;
    if bytes.len() < expected {
        return Err(HardwareControlError::ProtocolError(format!(
            "Victus S fan level response too short: expected at least {expected} bytes for {fan_count} fans, got {}",
            bytes.len()
        )));
    }
    Ok(bytes[..expected]
        .iter()
        .map(|&b| rpm_for_level(b))
        .collect())
}

/// A single timestamped telemetry sample during the 30-second post-restoration observation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObservationSample {
    pub elapsed_secs: u32,
    pub max_fan_active: Result<bool, String>,
    pub cpu_rpm: Result<u32, String>,
    pub gpu_rpm: Result<u32, String>,
}

/// Evaluate 30-second post-restoration telemetry samples to determine the terminal validation state.
pub fn evaluate_observation(
    samples: &[ObservationSample],
    peak_cpu: u32,
    _peak_gpu: u32,
    profile_restored: bool,
) -> ValidationState {
    if !profile_restored {
        return ValidationState::RestorationFailed;
    }

    // Must have at least 30 samples for full observation verification
    if samples.len() < 30 {
        return ValidationState::RestorationUnverified;
    }

    let mut wmi_error_count = 0;
    let mut any_max_fan_active = false;
    let mut last_cpu_rpms = Vec::new();

    for sample in samples {
        match sample.max_fan_active {
            Ok(true) => any_max_fan_active = true,
            Ok(false) => {}
            Err(_) => wmi_error_count += 1,
        }

        match sample.cpu_rpm {
            Ok(rpm) => {
                if sample.elapsed_secs >= 20 {
                    last_cpu_rpms.push(rpm);
                }
            }
            Err(_) => wmi_error_count += 1,
        }

        if sample.gpu_rpm.is_err() {
            wmi_error_count += 1;
        }
    }

    // If Max Fan was observed active at any point during observation, restoration failed.
    if any_max_fan_active {
        return ValidationState::RestorationFailed;
    }

    // Too many WMI errors during observation makes results inconclusive.
    // Allow at most 3 intermittent transient errors out of 30 samples if recovery succeeded.
    if wmi_error_count > 3 {
        return ValidationState::RestorationUnverified;
    }

    // Check if the final sample ended with an error
    if samples.last().is_some_and(|last| {
        last.max_fan_active.is_err() || last.cpu_rpm.is_err() || last.gpu_rpm.is_err()
    }) {
        return ValidationState::RestorationUnverified;
    }

    // Deceleration check: If fans were accelerated to high speed (e.g. peak_cpu >= 3500),
    // fans should show deceleration towards normal curve in the final 10 seconds.
    // If CPU fan remains pinned near peak (within 100 RPM) for all final samples, fail.
    if peak_cpu >= 3500 && !last_cpu_rpms.is_empty() {
        let avg_final_cpu: u32 = last_cpu_rpms.iter().sum::<u32>() / (last_cpu_rpms.len() as u32);
        if avg_final_cpu >= peak_cpu.saturating_sub(100) {
            // Fan remained locked at max speed
            return ValidationState::RestorationFailed;
        }
    }

    ValidationState::RestorationVerified
}

/// Map a 0..=255 PWM-style value onto the table's `1..=max_level` range.
///
/// Integer equivalent of the driver's `pwm_to_rpm()` / `fixp_linear_interpolate`
/// call. `0` is the automatic sentinel and is returned unchanged.
pub fn pwm_to_level(pwm: u8, max_level: u8) -> u8 {
    if pwm == 0 {
        return FAN_SPEED_AUTOMATIC;
    }
    let scaled = (u32::from(pwm) * u32::from(max_level) + 127) / u32::from(u8::MAX);
    scaled.clamp(1, u32::from(max_level)) as u8
}

/// Parse a `0x2F` fan-table response.
///
/// The layout is defined by the upstream driver: a 2-byte header followed by
/// 3-byte packed rows. Parsing stops at the first all-zero row because some
/// devices return a truncated table padded with zeros.
pub fn parse_fan_table(bytes: &[u8]) -> Result<FanTable, HardwareControlError> {
    if bytes.len() < FAN_TABLE_HEADER_LEN {
        return Err(HardwareControlError::ProtocolError(format!(
            "Victus S fan table response too short: {} bytes (need at least {FAN_TABLE_HEADER_LEN})",
            bytes.len()
        )));
    }

    let num_fans = bytes[0];
    if num_fans == 0 {
        return Err(HardwareControlError::ProtocolError(
            "Victus S fan table reports zero fans".to_string(),
        ));
    }

    let mut entries = Vec::new();
    let mut offset = FAN_TABLE_HEADER_LEN;
    while offset + FAN_TABLE_ENTRY_LEN <= bytes.len() {
        let entry = FanTableEntry {
            cpu_rpm: bytes[offset],
            gpu_rpm: bytes[offset + 1],
            noise_db: bytes[offset + 2],
        };
        if entry.cpu_rpm == 0 && entry.gpu_rpm == 0 && entry.noise_db == 0 {
            break;
        }
        entries.push(entry);
        offset += FAN_TABLE_ENTRY_LEN;
    }

    if entries.is_empty() {
        return Err(HardwareControlError::ProtocolError(
            "Victus S fan table contained no usable rows".to_string(),
        ));
    }

    let min_rpm = entries.iter().map(|e| e.cpu_rpm).min().unwrap_or(0);
    let max_rpm = entries.iter().map(|e| e.cpu_rpm).max().unwrap_or(0);

    if max_rpm == 0 {
        return Err(HardwareControlError::ProtocolError(
            "Victus S fan table contained no non-zero RPM entries".to_string(),
        ));
    }

    // On 8A3D and similar boards, row 0 has gpu_rpm == 0.
    // The GPU offset must be calculated from the first row with a non-zero GPU RPM value
    // to avoid negative bogus offsets and arithmetic underflow.
    let gpu_offset = entries
        .iter()
        .find(|e| e.gpu_rpm > 0)
        .map(|e| (e.gpu_rpm as i16) - (e.cpu_rpm as i16));

    Ok(FanTable {
        num_fans,
        min_rpm,
        max_rpm,
        gpu_offset,
        entries,
    })
}

/// Validate a requested manual level against a parsed fan table.
///
/// `FAN_SPEED_AUTOMATIC` is always accepted; any other level must fall inside
/// the firmware's advertised range.
pub fn validate_level(fan: &str, level: u8, table: &FanTable) -> Result<(), HardwareControlError> {
    if level == FAN_SPEED_AUTOMATIC || level <= table.max_rpm {
        return Ok(());
    }
    Err(HardwareControlError::InvalidParameter(format!(
        "{fan} fan level {level} outside the firmware fan-table range 1..={} \
         (0x00 = automatic)",
        table.max_rpm
    )))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a 128-byte fan table with the given `(cpu, gpu, noise)` rows.
    fn table_payload(num_fans: u8, rows: &[(u8, u8, u8)]) -> Vec<u8> {
        let mut bytes = vec![0u8; MAX_RESPONSE_LEN];
        bytes[0] = num_fans;
        for (i, (cpu, gpu, noise)) in rows.iter().enumerate() {
            let offset = FAN_TABLE_HEADER_LEN + i * FAN_TABLE_ENTRY_LEN;
            bytes[offset] = *cpu;
            bytes[offset + 1] = *gpu;
            bytes[offset + 2] = *noise;
        }
        bytes
    }

    #[test]
    fn board_8a3d_is_routed_to_the_victus_s_profile() {
        assert!(is_victus_s_board("8A3D"));
        assert!(is_victus_s_board(" 8a3d "));
        assert!(is_victus_s_board("8C99"));
        // Recognised OMEN boards from the same driver family are not Victus S.
        assert!(!is_victus_s_board("88F8"));
        assert!(!is_victus_s_board("8A24"));
        assert!(!is_victus_s_board("8A4F"));
        assert!(!is_victus_s_board(""));
    }

    #[test]
    fn fan_table_parses_header_and_rows() {
        let bytes = table_payload(2, &[(0x27, 0x23, 0x0C), (0x35, 0x2E, 0x18)]);
        let table = parse_fan_table(&bytes).expect("parse fan table");
        assert_eq!(table.num_fans, 2);
        assert_eq!(table.entries.len(), 2);
        assert_eq!(table.min_rpm, 0x27);
        assert_eq!(table.max_rpm, 0x35);
        assert_eq!(table.max_level(), 0x35);
        assert_eq!(table.gpu_offset, Some(-4));
        assert_eq!(
            table.entries[0],
            FanTableEntry {
                cpu_rpm: 0x27,
                gpu_rpm: 0x23,
                noise_db: 0x0C,
            }
        );
    }

    #[test]
    fn fan_table_zero_gpu_first_row_derives_gpu_offset_from_nonzero_row() {
        // Motherboard 8A3D: row 0 has gpu_rpm == 0, next row has nonzero gpu_rpm
        let bytes = table_payload(
            2,
            &[
                (0x20, 0x00, 0x0A), // row 0: CPU 32, GPU 0
                (0x28, 0x24, 0x10), // row 1: CPU 40, GPU 36 -> offset = -4
                (0x35, 0x31, 0x1A), // row 2: CPU 53, GPU 49
            ],
        );
        let table = parse_fan_table(&bytes).expect("parse 8A3D fan table");
        assert_eq!(table.entries.len(), 3);
        assert_eq!(table.min_rpm, 0x20);
        assert_eq!(table.max_rpm, 0x35);
        assert_eq!(table.gpu_offset, Some(-4));
        assert_eq!(table.gpu_offset(), Some(-4));
        assert_eq!(table.gpu_level_for_cpu(40), 36);
    }

    #[test]
    fn fan_table_rejects_all_zero_rpm_table() {
        let bytes = table_payload(2, &[(0x00, 0x00, 0x0C)]);
        let err = parse_fan_table(&bytes).unwrap_err();
        assert!(matches!(err, HardwareControlError::ProtocolError(_)));
    }

    #[test]
    fn fan_table_stops_at_truncated_all_zero_row() {
        let bytes = table_payload(2, &[(0x30, 0x30, 0x10), (0, 0, 0), (0x60, 0x60, 0x30)]);
        let table = parse_fan_table(&bytes).expect("parse truncated fan table");
        assert_eq!(table.entries.len(), 1);
        assert_eq!(table.max_rpm, 0x30);
    }

    #[test]
    fn fan_table_rejects_zero_fans_and_empty_payloads() {
        assert!(matches!(
            parse_fan_table(&[0u8, 0x00]).unwrap_err(),
            HardwareControlError::ProtocolError(_)
        ));
        assert!(matches!(
            parse_fan_table(&[1u8]).unwrap_err(),
            HardwareControlError::ProtocolError(_)
        ));
        // Header claims a fan but every row is zero.
        assert!(matches!(
            parse_fan_table(&table_payload(1, &[])).unwrap_err(),
            HardwareControlError::ProtocolError(_)
        ));
    }

    #[test]
    fn level_and_rpm_conversions_match_upstream_units() {
        assert_eq!(rpm_for_level(0x27), 3900);
        assert_eq!(rpm_for_level(0x23), 3500);
        assert_eq!(rpm_for_level(0), 0);

        assert_eq!(pwm_to_level(0, 0x35), FAN_SPEED_AUTOMATIC);
        assert_eq!(pwm_to_level(255, 0x35), 0x35);
        assert_eq!(pwm_to_level(1, 0x35), 1);
        // Midpoint stays inside the advertised range.
        let mid = pwm_to_level(128, 0x35);
        assert!(mid > 0 && mid <= 0x35);
    }

    #[test]
    fn validate_level_accepts_automatic_and_rejects_out_of_range() {
        let table = parse_fan_table(&table_payload(2, &[(0x27, 0x23, 0x0C)])).expect("fan table");
        assert!(validate_level("cpu", FAN_SPEED_AUTOMATIC, &table).is_ok());
        assert!(validate_level("cpu", 0x27, &table).is_ok());
        let err = validate_level("gpu", 0x28, &table).unwrap_err();
        assert!(matches!(err, HardwareControlError::InvalidParameter(_)));
    }

    #[test]
    fn thermal_profile_values_match_the_8a3d_ec_layout() {
        assert_eq!(EC_THERMAL_PROFILE_OFFSET, 0x95);
        assert_eq!(ThermalProfile::Balanced.firmware_value(), 0x00);
        assert_eq!(ThermalProfile::Performance.firmware_value(), 0x01);
        assert_eq!(ThermalProfile::Quiet.firmware_value(), 0x03);
        assert_eq!(
            thermal_profile_payload(ThermalProfile::Balanced),
            [0xFF, 0x00]
        );
        assert_eq!(
            thermal_profile_payload(ThermalProfile::Performance),
            [0xFF, 0x01]
        );
        assert_eq!(thermal_profile_payload(ThermalProfile::Quiet), [0xFF, 0x03]);
    }

    #[test]
    fn thermal_profile_names_round_trip_and_reject_unknown() {
        for (name, expected) in [
            ("balanced", ThermalProfile::Balanced),
            (" Standard ", ThermalProfile::Balanced),
            ("performance", ThermalProfile::Performance),
            ("QUIET", ThermalProfile::Quiet),
            ("eco", ThermalProfile::Quiet),
        ] {
            assert_eq!(ThermalProfile::from_name(name), Some(expected), "{name}");
        }

        // "cool" is an OMEN profile; the 8A3D EC has no such value.
        assert_eq!(ThermalProfile::from_name("cool"), None);
        assert_eq!(ThermalProfile::from_name(""), None);

        for profile in [
            ThermalProfile::Balanced,
            ThermalProfile::Performance,
            ThermalProfile::Quiet,
        ] {
            assert_eq!(ThermalProfile::from_name(profile.as_str()), Some(profile));
        }
    }

    #[test]
    fn only_8a3d_restores_cooling_through_the_ec_thermal_profile() {
        assert!(requires_thermal_profile_restore("8A3D"));
        assert!(requires_thermal_profile_restore(" 8a3d "));
        for board in ["8B2F", "8C99", "8DE1", "88F8", ""] {
            assert!(!requires_thermal_profile_restore(board), "{board}");
        }
    }

    #[test]
    fn parse_dual_fan_levels_extracts_rpms_and_validates_length() {
        let ok = parse_dual_fan_levels(&[34, 29]).expect("dual fan");
        assert_eq!(ok.cpu_rpm, 3400);
        assert_eq!(ok.gpu_rpm, 2900);

        let zero_gpu = parse_dual_fan_levels(&[34, 0]).expect("zero gpu");
        assert_eq!(zero_gpu.cpu_rpm, 3400);
        assert_eq!(zero_gpu.gpu_rpm, 0);

        let both_zero = parse_dual_fan_levels(&[0, 0]).expect("both zero");
        assert_eq!(both_zero.cpu_rpm, 0);
        assert_eq!(both_zero.gpu_rpm, 0);

        assert!(matches!(
            parse_dual_fan_levels(&[]).unwrap_err(),
            HardwareControlError::ProtocolError(_)
        ));
        assert!(matches!(
            parse_dual_fan_levels(&[34]).unwrap_err(),
            HardwareControlError::ProtocolError(_)
        ));
    }

    #[test]
    fn parse_fan_levels_bounds_checks_and_converts() {
        let rpms = parse_fan_levels(&[32, 28, 99], 2).expect("parse 2 fans");
        assert_eq!(rpms, vec![3200, 2800]);

        assert!(matches!(
            parse_fan_levels(&[32], 2).unwrap_err(),
            HardwareControlError::ProtocolError(_)
        ));
    }

    #[test]
    fn profile_restore_target_helpers() {
        let known = ProfileRestoreTarget::Known(ThermalProfile::Quiet);
        assert!(known.is_known());
        assert_eq!(known.profile(), ThermalProfile::Quiet);

        let fallback = ProfileRestoreTarget::SafeFallback(ThermalProfile::Balanced);
        assert!(!fallback.is_known());
        assert_eq!(fallback.profile(), ThermalProfile::Balanced);

        let from: ProfileRestoreTarget = ThermalProfile::Performance.into();
        assert!(from.is_known());
        assert_eq!(from.profile(), ThermalProfile::Performance);
    }

    #[test]
    fn evaluate_observation_handles_various_scenarios() {
        let mut clean_samples = Vec::new();
        for s in 1..=30 {
            clean_samples.push(ObservationSample {
                elapsed_secs: s,
                max_fan_active: Ok(false),
                cpu_rpm: Ok(if s < 10 { 4200 - s * 150 } else { 2200 }),
                gpu_rpm: Ok(if s < 10 { 3900 - s * 200 } else { 0 }),
            });
        }

        // 1. Clean run: deceleration from 4700 peak -> Verified
        assert_eq!(
            evaluate_observation(&clean_samples, 4700, 4500, true),
            ValidationState::RestorationVerified
        );

        // 2. Profile restoration failed initially -> Failed
        assert_eq!(
            evaluate_observation(&clean_samples, 4700, 4500, false),
            ValidationState::RestorationFailed
        );

        // 3. Incomplete observation (< 30 samples) -> Unverified
        assert_eq!(
            evaluate_observation(&clean_samples[..15], 4700, 4500, true),
            ValidationState::RestorationUnverified
        );

        // 4. Max Fan re-enabled during observation -> Failed
        let mut failed_max = clean_samples.clone();
        failed_max[10].max_fan_active = Ok(true);
        assert_eq!(
            evaluate_observation(&failed_max, 4700, 4500, true),
            ValidationState::RestorationFailed
        );

        // 5. Fans pinned at peak with no deceleration -> Failed
        let mut pinned_samples = Vec::new();
        for s in 1..=30 {
            pinned_samples.push(ObservationSample {
                elapsed_secs: s,
                max_fan_active: Ok(false),
                cpu_rpm: Ok(4700),
                gpu_rpm: Ok(4500),
            });
        }
        assert_eq!(
            evaluate_observation(&pinned_samples, 4700, 4500, true),
            ValidationState::RestorationFailed
        );

        // 6. Too many WMI errors -> Unverified
        let mut error_samples = clean_samples.clone();
        for item in error_samples.iter_mut().take(5) {
            item.cpu_rpm = Err("WMI error".to_string());
        }
        assert_eq!(
            evaluate_observation(&error_samples, 4700, 4500, true),
            ValidationState::RestorationUnverified
        );

        // 7. Final sample errored -> Unverified
        let mut final_error = clean_samples.clone();
        final_error[29].cpu_rpm = Err("WMI error".to_string());
        assert_eq!(
            evaluate_observation(&final_error, 4700, 4500, true),
            ValidationState::RestorationUnverified
        );
    }
}
