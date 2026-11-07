//! Request authorization and board safety policy enforcement (WIN-07).

use crate::service::types::HardwareControlError;
use crate::service::windows::profiles::BoardProfile;
use crate::service::windows::protocol::HardwareRequest;

/// Validate and authorize a hardware mutation request against the board profile (WIN-07).
///
/// Ensures:
/// 1. Fan mode mutations are restricted to supported modes (auto / max).
/// 2. If "max" fan mode is requested, verifies `board.has_max_fan`.
/// 3. Power profile switches only target profiles advertised in `board.supported_power_profiles`.
/// 4. RGB profiles validate against board zone capacity.
pub fn authorize_mutation(
    req: &HardwareRequest,
    board: &BoardProfile,
) -> Result<(), HardwareControlError> {
    match req {
        HardwareRequest::GetCapabilities
        | HardwareRequest::GetFanStatus
        | HardwareRequest::GetPowerProfiles
        | HardwareRequest::GetPowerProfile => {
            // Read-only queries are always permitted
            Ok(())
        }
        HardwareRequest::SetFanMode { mode } => {
            let normalized = mode.trim().to_lowercase();
            match normalized.as_str() {
                "auto" => Ok(()),
                "max" => {
                    if board.has_max_fan {
                        Ok(())
                    } else {
                        Err(HardwareControlError::UnsupportedMode(format!(
                            "Motherboard {} does not support Maximum Fan override",
                            board.board_id
                        )))
                    }
                }
                "manual" => Err(HardwareControlError::UnsupportedMode(
                    "Manual fan curve setting is not supported on this platform".to_string(),
                )),
                other => Err(HardwareControlError::InvalidParameter(format!(
                    "Unrecognized fan mode '{other}'"
                ))),
            }
        }
        HardwareRequest::SetPowerProfile { profile } => {
            let normalized = profile.trim().to_lowercase();
            if board
                .supported_power_profiles
                .iter()
                .any(|p| p.eq_ignore_ascii_case(&normalized))
            {
                Ok(())
            } else {
                Err(HardwareControlError::UnsupportedMode(format!(
                    "Power profile '{normalized}' is not supported by board {}. Supported: {:?}",
                    board.board_id, board.supported_power_profiles
                )))
            }
        }
        HardwareRequest::ApplyPreset { preset } => {
            let normalized = preset.trim().to_lowercase();
            match normalized.as_str() {
                "quiet" | "balanced" | "gaming" => Ok(()),
                other => Err(HardwareControlError::InvalidParameter(format!(
                    "Unknown combined preset '{other}'"
                ))),
            }
        }
        HardwareRequest::ApplyRgbProfile { profile_json } => {
            if profile_json.trim().is_empty() {
                return Err(HardwareControlError::InvalidParameter(
                    "RGB profile payload cannot be empty".to_string(),
                ));
            }
            if board.rgb_zone_count == 0 {
                return Err(HardwareControlError::UnsupportedMode(
                    "Keyboard RGB lighting is not supported on this model".to_string(),
                ));
            }
            Ok(())
        }
        HardwareRequest::SetRgbEnabled { .. } => {
            if board.rgb_zone_count == 0 {
                return Err(HardwareControlError::UnsupportedMode(
                    "Keyboard RGB lighting is not supported on this model".to_string(),
                ));
            }
            Ok(())
        }
    }
}
