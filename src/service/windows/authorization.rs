//! Request authorization and board safety policy enforcement (WIN-07).

use crate::service::types::HardwareControlError;
use crate::service::windows::profiles::{BoardProfile, BoardValidation};
use crate::service::windows::protocol::HardwareRequest;

/// Validate and authorize a hardware mutation request against the board profile and validation evidence (WIN-07).
///
/// Ensures:
/// 1. Read-only queries are always permitted regardless of validation status.
/// 2. Unverified boards or missing validation records strictly reject all hardware mutations.
/// 3. Fan mode mutations verify `board.has_max_fan` and `validation.fan_write_verified`.
/// 4. Power profile switches verify advertised profiles and `validation.power_write_verified`.
/// 5. Combined presets verify both fan and power write authorization.
/// 6. RGB lighting operations verify zone capacity and `validation.rgb_write_verified`.
pub fn authorize_mutation(
    req: &HardwareRequest,
    board: &BoardProfile,
    validation: Option<&BoardValidation>,
) -> Result<(), HardwareControlError> {
    match req {
        HardwareRequest::GetCapabilities
        | HardwareRequest::GetFanStatus
        | HardwareRequest::GetPowerProfiles
        | HardwareRequest::GetPowerProfile
        | HardwareRequest::GetServiceMemory => {
            // Read-only queries are always permitted
            Ok(())
        }
        HardwareRequest::SetFanMode { mode } => {
            let val = validation.ok_or_else(|| {
                HardwareControlError::UnsupportedMode(format!(
                    "Hardware fan mutation rejected: board {} is unverified on physical hardware",
                    board.board_id
                ))
            })?;
            if !val.board_id.eq_ignore_ascii_case(&board.board_id) {
                return Err(HardwareControlError::UnsupportedMode(format!(
                    "Validation board ID {} mismatch with active board {}",
                    val.board_id, board.board_id
                )));
            }
            if !val.fan_write_verified {
                return Err(HardwareControlError::AuthorizationFailed(format!(
                    "Physical write withheld for board {}: WMI fan write protocol is implemented, but physical mutation is blocked pending hardware validation (fan_write_verified = false). Run 'zenomen-hw --validate-8a3d-fan' to physically validate fan response.",
                    board.board_id
                )));
            }

            let normalized = mode.trim().to_lowercase();
            match normalized.as_str() {
                "auto" => {
                    if !val.auto_fan_restore_verified {
                        return Err(HardwareControlError::AuthorizationFailed(format!(
                            "Physical write withheld for board {}: automatic fan restoration is not verified on physical hardware (bios {})",
                            board.board_id, val.bios_version
                        )));
                    }
                    Ok(())
                }
                "max" => {
                    if !board.has_max_fan {
                        return Err(HardwareControlError::UnsupportedMode(format!(
                            "Motherboard {} does not support Maximum Fan override",
                            board.board_id
                        )));
                    }
                    if !val.max_fan_write_verified {
                        return Err(HardwareControlError::AuthorizationFailed(format!(
                            "Physical write withheld for board {}: Maximum Fan override is not verified on physical hardware (bios {})",
                            board.board_id, val.bios_version
                        )));
                    }
                    Ok(())
                }
                "manual" => {
                    if !board.has_custom_curves {
                        return Err(HardwareControlError::UnsupportedMode(format!(
                            "Motherboard {} does not support manual fan curve control",
                            board.board_id
                        )));
                    }
                    if !val.manual_fan_write_verified {
                        return Err(HardwareControlError::UnsupportedMode(format!(
                            "Manual per-fan PWM / curve control is not verified on physical hardware for board {}",
                            board.board_id
                        )));
                    }
                    Ok(())
                }
                other => Err(HardwareControlError::InvalidParameter(format!(
                    "Unrecognized fan mode '{other}'"
                ))),
            }
        }
        HardwareRequest::SetPowerProfile { profile } => {
            let val = validation.ok_or_else(|| {
                HardwareControlError::UnsupportedMode(format!(
                    "Hardware power mutation rejected: board {} is unverified on physical hardware",
                    board.board_id
                ))
            })?;
            if !val.board_id.eq_ignore_ascii_case(&board.board_id) {
                return Err(HardwareControlError::UnsupportedMode(format!(
                    "Validation board ID {} mismatch with active board {}",
                    val.board_id, board.board_id
                )));
            }
            if !val.power_write_authorized {
                return Err(HardwareControlError::UnsupportedMode(format!(
                    "Motherboard {} power profile write is not verified on physical hardware",
                    board.board_id
                )));
            }

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
            let val = validation.ok_or_else(|| {
                HardwareControlError::UnsupportedMode(format!(
                    "Hardware preset mutation rejected: board {} is unverified on physical hardware",
                    board.board_id
                ))
            })?;
            if !val.board_id.eq_ignore_ascii_case(&board.board_id) {
                return Err(HardwareControlError::UnsupportedMode(format!(
                    "Validation board ID {} mismatch with active board {}",
                    val.board_id, board.board_id
                )));
            }

            let power_permitted = val.power_write_authorized;

            let normalized = preset.trim().to_lowercase();
            match normalized.as_str() {
                "quiet" | "balanced" | "auto" => {
                    if !power_permitted {
                        return Err(HardwareControlError::UnsupportedMode(format!(
                            "Board {} power write is not verified for preset '{normalized}'",
                            board.board_id
                        )));
                    }
                    Ok(())
                }
                "performance" | "gaming" => {
                    if !power_permitted || !val.fan_write_verified {
                        return Err(HardwareControlError::UnsupportedMode(format!(
                            "Board {} fan and power writes must both be verified for preset '{normalized}'",
                            board.board_id
                        )));
                    }
                    Ok(())
                }
                other => Err(HardwareControlError::InvalidParameter(format!(
                    "Unknown combined preset '{other}'"
                ))),
            }
        }
        HardwareRequest::ApplyRgbProfile { profile_json } => {
            let val = validation.ok_or_else(|| {
                HardwareControlError::UnsupportedMode(format!(
                    "Hardware RGB mutation rejected: board {} is unverified on physical hardware",
                    board.board_id
                ))
            })?;
            if !val.board_id.eq_ignore_ascii_case(&board.board_id) {
                return Err(HardwareControlError::UnsupportedMode(format!(
                    "Validation board ID {} mismatch with active board {}",
                    val.board_id, board.board_id
                )));
            }
            if !val.rgb_write_verified {
                return Err(HardwareControlError::UnsupportedMode(format!(
                    "Motherboard {} RGB write is not verified on physical hardware",
                    board.board_id
                )));
            }

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
            let val = validation.ok_or_else(|| {
                HardwareControlError::UnsupportedMode(format!(
                    "Hardware RGB mutation rejected: board {} is unverified on physical hardware",
                    board.board_id
                ))
            })?;
            if !val.board_id.eq_ignore_ascii_case(&board.board_id) {
                return Err(HardwareControlError::UnsupportedMode(format!(
                    "Validation board ID {} mismatch with active board {}",
                    val.board_id, board.board_id
                )));
            }
            if !val.rgb_write_verified {
                return Err(HardwareControlError::UnsupportedMode(format!(
                    "Motherboard {} RGB toggle is not verified on physical hardware",
                    board.board_id
                )));
            }

            if board.rgb_zone_count == 0 {
                return Err(HardwareControlError::UnsupportedMode(
                    "Keyboard RGB lighting is not supported on this model".to_string(),
                ));
            }
            Ok(())
        }
    }
}
