//! Cooling and fan control screen model for GPUI-CE.

use crate::core::events::AppCommand;
use crate::core::state::AppState;

#[derive(Debug, Clone, PartialEq)]
pub struct CoolingScreen {
    pub fan_mode: String,
    pub pending_fan_mode: Option<String>,
    pub fan_control_supported: bool,
    pub fan_rpm_supported: bool,
    pub fan_readouts: Vec<(String, u32)>,
}

impl CoolingScreen {
    pub fn from_state(state: &AppState) -> Self {
        let fan_readouts = state
            .latest_snapshot
            .as_ref()
            .map(|s| {
                s.fan_rpms
                    .iter()
                    .enumerate()
                    .map(|(i, &rpm)| (format!("Fan {}", i + 1), rpm))
                    .collect()
            })
            .unwrap_or_default();

        Self {
            fan_mode: state.fan_mode.clone(),
            pending_fan_mode: state.pending_fan_mode.clone(),
            fan_control_supported: state.capabilities.fan_control,
            fan_rpm_supported: state.capabilities.fan_rpm,
            fan_readouts,
        }
    }

    pub fn set_mode_command(&self, mode: &str) -> AppCommand {
        AppCommand::SetFanMode(mode.to_string())
    }
}
