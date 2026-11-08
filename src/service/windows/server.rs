//! Named pipe hardware request dispatch server (WIN-03).

use crate::rgb::schema::RgbProfile;
use crate::service::types::PresetResult;
use crate::service::windows::authorization::authorize_mutation;
use crate::service::windows::hp_wmi::{self, HpWmiConnection};
use crate::service::windows::profiles::{BoardProfile, BoardValidation};
use crate::service::windows::protocol::{HardwareRequest, HardwareResponse};

/// Dispatch and execute a single hardware request against the board profile and WMI driver.
pub fn handle_request(
    req: HardwareRequest,
    board: &BoardProfile,
    validation: Option<&BoardValidation>,
    conn: &HpWmiConnection,
) -> HardwareResponse {
    // 1. Authorize the mutation against board profile safety invariants and physical validation record
    if let Err(e) = authorize_mutation(&req, board, validation) {
        return HardwareResponse::Error(e.to_string());
    }

    // 2. Dispatch authorized request
    match req {
        HardwareRequest::GetCapabilities => match hp_wmi::query_hp_capabilities(conn) {
            Ok(caps) => HardwareResponse::Capabilities(caps),
            Err(e) => HardwareResponse::Error(e.to_string()),
        },
        HardwareRequest::GetFanStatus => match hp_wmi::query_fan_status(conn) {
            Ok((mode, rpms)) => HardwareResponse::FanStatus { mode, rpms },
            Err(e) => HardwareResponse::Error(e.to_string()),
        },
        HardwareRequest::SetFanMode { mode } => match hp_wmi::set_fan_mode(conn, &mode) {
            Ok(m) => HardwareResponse::FanModeSet(m),
            Err(e) => HardwareResponse::Error(e.to_string()),
        },
        HardwareRequest::GetPowerProfiles => {
            HardwareResponse::PowerProfiles(board.supported_power_profiles.clone())
        }
        HardwareRequest::GetPowerProfile => HardwareResponse::PowerProfile(
            board
                .supported_power_profiles
                .first()
                .cloned()
                .unwrap_or_else(|| "balanced".to_string()),
        ),
        HardwareRequest::SetPowerProfile { profile } => {
            match hp_wmi::set_performance_mode(conn, &profile) {
                Ok(p) => HardwareResponse::PowerProfileSet(p),
                Err(e) => HardwareResponse::Error(e.to_string()),
            }
        }
        HardwareRequest::ApplyPreset { preset } => {
            let normalized = preset.trim().to_lowercase();
            let (target_power, target_fan) = match normalized.as_str() {
                "quiet" => ("quiet", "auto"),
                "balanced" => ("balanced", "auto"),
                "gaming" => ("performance", "max"),
                other => {
                    return HardwareResponse::Error(format!("Unknown preset '{other}'"));
                }
            };

            let p_res = hp_wmi::set_performance_mode(conn, target_power);
            let f_res = hp_wmi::set_fan_mode(conn, target_fan);

            let power_applied = p_res.is_ok();
            let fan_applied = f_res.is_ok();
            let partial_success = power_applied ^ fan_applied;

            let message = if power_applied && fan_applied {
                format!("Applied combined preset '{normalized}' successfully")
            } else if power_applied {
                "Applied power profile, but fan mode change failed".to_string()
            } else if fan_applied {
                "Applied fan mode, but power profile change failed".to_string()
            } else {
                "Failed to apply preset".to_string()
            };

            HardwareResponse::PresetApplied(PresetResult {
                preset_name: normalized,
                power_applied,
                confirmed_power_profile: p_res.ok(),
                fan_applied,
                confirmed_fan_mode: f_res.ok(),
                partial_success,
                message,
            })
        }
        HardwareRequest::ApplyRgbProfile { profile_json } => {
            match serde_json::from_str::<RgbProfile>(&profile_json) {
                Ok(profile) => match hp_wmi::set_rgb_profile(conn, &profile) {
                    Ok(name) => HardwareResponse::RgbProfileApplied(name),
                    Err(e) => HardwareResponse::Error(e.to_string()),
                },
                Err(e) => HardwareResponse::Error(format!("Malformed RGB profile JSON: {e}")),
            }
        }
        HardwareRequest::SetRgbEnabled { enabled } => HardwareResponse::RgbEnabledSet(enabled),
    }
}
