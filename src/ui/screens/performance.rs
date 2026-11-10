//! Power profile and performance screen model for GPUI-CE.

use crate::core::events::AppCommand;
use crate::core::state::AppState;

#[derive(Debug, Clone, PartialEq)]
pub struct PerformanceScreen {
    pub active_power_profile: String,
    pub power_profile_verified: bool,
    pub power_profiles_supported: bool,
    pub profile_choices: Vec<String>,
}

impl PerformanceScreen {
    pub fn from_state(state: &AppState) -> Self {
        Self {
            active_power_profile: state.active_power_profile.clone(),
            power_profile_verified: state.power_profile_verified,
            power_profiles_supported: state.capabilities.power_profiles,
            profile_choices: if state.power_profile_choices.is_empty() {
                vec![
                    "quiet".to_string(),
                    "balanced".to_string(),
                    "performance".to_string(),
                ]
            } else {
                state.power_profile_choices.clone()
            },
        }
    }

    pub fn set_profile_command(&self, profile: &str) -> AppCommand {
        AppCommand::SetPowerProfile(profile.to_string())
    }
}
