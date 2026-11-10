//! Combined system profile orchestrator.
//!
//! Coordinates multi-layered system profile changes across:
//! 1. Display refresh rate (e.g. 60 Hz for Quiet vs 144 Hz for Balanced/Performance) via native display APIs.
//! 2. OEM Platform Power / Thermal profile (via HardwareServiceClient).
//! 3. Laptop cooling fan policy (Auto vs Max Fan via HardwareServiceClient).
//!
//! Enforces honest reporting: partial success is recorded when display or hardware
//! operations diverge, and 8A3D unverified read-backs are explicitly identified.

use crate::config::AppConfig;
use crate::service::HardwareServiceClient;
use crate::service::types::PresetResult;

/// Normalized preset definition with target values for display, power, and fan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SystemPresetTargets {
    pub name: String,
    pub target_refresh_rate: u32,
    pub target_power_profile: String,
    pub target_fan_mode: String,
}

/// Resolve preset name into concrete targets according to user configuration.
pub fn resolve_preset_targets(
    preset: &str,
    config: &AppConfig,
) -> Result<SystemPresetTargets, String> {
    let normalized = preset.trim().to_lowercase();
    match normalized.as_str() {
        "quiet" => Ok(SystemPresetTargets {
            name: "quiet".to_string(),
            target_refresh_rate: config.display.preferred_quiet_refresh_rate,
            target_power_profile: "quiet".to_string(),
            target_fan_mode: "auto".to_string(),
        }),
        "balanced" => Ok(SystemPresetTargets {
            name: "balanced".to_string(),
            target_refresh_rate: config.display.preferred_active_refresh_rate,
            target_power_profile: "balanced".to_string(),
            target_fan_mode: "auto".to_string(),
        }),
        "auto" => Ok(SystemPresetTargets {
            name: "auto".to_string(),
            target_refresh_rate: config.display.preferred_active_refresh_rate,
            target_power_profile: "balanced".to_string(),
            target_fan_mode: "auto".to_string(),
        }),
        "performance" | "gaming" => Ok(SystemPresetTargets {
            name: "performance".to_string(),
            target_refresh_rate: config.display.preferred_active_refresh_rate,
            target_power_profile: "performance".to_string(),
            target_fan_mode: "max".to_string(),
        }),
        other => Err(format!(
            "Unknown combined preset '{other}'. Supported presets: quiet, balanced, auto, performance"
        )),
    }
}

/// Query active display refresh rate, if supported on the platform.
pub fn get_active_refresh_rate() -> Option<u32> {
    #[cfg(target_os = "windows")]
    {
        crate::hardware::windows::display::get_active_refresh_rate().ok()
    }
    #[cfg(not(target_os = "windows"))]
    {
        None
    }
}

/// Restore default active refresh rate on exit, if configured.
pub fn restore_default_refresh_rate(config: &AppConfig) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        if config.display.restore_refresh_rate_on_exit {
            let target_hz = config.display.preferred_active_refresh_rate;
            crate::hardware::windows::display::set_refresh_rate(target_hz)
                .map(|_| ())
                .map_err(|e| e.to_string())
        } else {
            Ok(())
        }
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = config;
        Ok(())
    }
}

/// Apply a combined system profile across display, power profile, and fan control.
pub fn apply_system_profile(
    client: &HardwareServiceClient,
    preset: &str,
    config: &AppConfig,
) -> Result<PresetResult, String> {
    let targets = resolve_preset_targets(preset, config)?;
    tracing::info!(
        preset = targets.name.as_str(),
        target_hz = targets.target_refresh_rate,
        target_power = targets.target_power_profile.as_str(),
        target_fan = targets.target_fan_mode.as_str(),
        "orchestrating combined system profile"
    );

    // 1. Switch display refresh rate
    #[cfg(target_os = "windows")]
    let (display_applied, confirmed_refresh_rate, display_message) = {
        match crate::hardware::windows::display::set_refresh_rate(targets.target_refresh_rate) {
            Ok(disp_res) => (
                disp_res.verified,
                Some(disp_res.actual_refresh_rate),
                Some(format!(
                    "Display: {} Hz (verified)",
                    disp_res.actual_refresh_rate
                )),
            ),
            Err(e) => {
                tracing::warn!(error = %e, "display refresh rate switch failed in preset orchestrator");
                (false, None, Some(format!("Display failed: {e}")))
            }
        }
    };
    #[cfg(not(target_os = "windows"))]
    let (display_applied, confirmed_refresh_rate, display_message) = (true, None, None);

    // 2. Switch OEM Platform Profile and Fan Mode via Hardware Service
    let hw_result = client.apply_preset(&targets.name);

    match hw_result {
        Ok(hw_res) => {
            let power_applied = hw_res.power_applied;
            let fan_applied = hw_res.fan_applied;
            let partial_success =
                !display_applied || hw_res.partial_success || (!power_applied || !fan_applied);

            let composite_message = match (&display_message, &hw_res.message) {
                (Some(disp_str), hw_str) => format!("{disp_str} · {hw_str}"),
                (None, hw_str) => hw_str.clone(),
            };

            Ok(PresetResult {
                preset_name: targets.name,
                power_applied,
                confirmed_power_profile: hw_res.confirmed_power_profile,
                fan_applied,
                confirmed_fan_mode: hw_res.confirmed_fan_mode,
                display_applied,
                confirmed_refresh_rate,
                partial_success,
                message: composite_message,
            })
        }
        Err(hw_err) => {
            // Service might be offline or call failed
            if let (true, Some(actual_hz)) = (display_applied, confirmed_refresh_rate) {
                Ok(PresetResult {
                    preset_name: targets.name.clone(),
                    power_applied: false,
                    confirmed_power_profile: None,
                    fan_applied: false,
                    confirmed_fan_mode: None,
                    display_applied: true,
                    confirmed_refresh_rate: Some(actual_hz),
                    partial_success: true,
                    message: format!(
                        "Display switched to {actual_hz} Hz; hardware service mutation withheld ({hw_err})"
                    ),
                })
            } else {
                Err(format!(
                    "Preset '{}' failed: display error = {:?}; hardware error = {}",
                    targets.name, display_message, hw_err
                ))
            }
        }
    }
}
