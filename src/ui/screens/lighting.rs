//! RGB lighting control screen model for GPUI-CE.

use crate::core::events::AppCommand;
use crate::core::state::AppState;
use crate::rgb::{RgbProfile, builtin_presets};

#[derive(Debug, Clone, PartialEq)]
pub struct LightingScreen {
    pub rgb_supported: bool,
    pub power_enabled: bool,
    pub active_preset_id: Option<String>,
    pub available_presets: Vec<RgbProfile>,
}

impl LightingScreen {
    pub fn from_state(state: &AppState) -> Self {
        Self {
            rgb_supported: state.capabilities.rgb_lighting,
            power_enabled: state.rgb_power_enabled,
            active_preset_id: state.active_rgb_profile.as_ref().map(|p| p.id.clone()),
            available_presets: builtin_presets(),
        }
    }

    pub fn apply_preset_command(&self, profile: RgbProfile) -> AppCommand {
        AppCommand::ApplyRgbProfile(profile)
    }

    pub fn toggle_power_command(&self) -> AppCommand {
        AppCommand::SetRgbPower(!self.power_enabled)
    }
}
