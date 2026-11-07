//! Application configuration and preferences screen model for GPUI-CE.

use crate::config::AppConfig;
use crate::core::events::AppCommand;

#[derive(Debug, Clone, PartialEq)]
pub struct SettingsScreen {
    pub start_minimized: bool,
    pub close_to_tray: bool,
    pub autostart: bool,
    pub polling_interval_ms: u64,
    pub history_window: usize,
}

impl SettingsScreen {
    pub fn from_config(config: &AppConfig) -> Self {
        Self {
            start_minimized: config.general.start_minimized,
            close_to_tray: config.general.close_to_tray,
            autostart: config.general.autostart,
            polling_interval_ms: config.telemetry.polling_interval_ms,
            history_window: config.telemetry.history_window,
        }
    }

    pub fn set_start_minimized_command(&self, enabled: bool) -> AppCommand {
        AppCommand::SetStartMinimized(enabled)
    }

    pub fn set_close_to_tray_command(&self, enabled: bool) -> AppCommand {
        AppCommand::SetCloseToTray(enabled)
    }

    pub fn set_autostart_command(&self, enabled: bool) -> AppCommand {
        AppCommand::SetAutostart(enabled)
    }

    pub fn set_polling_interval_command(&self, interval_ms: u64) -> AppCommand {
        AppCommand::SetPollingInterval(interval_ms)
    }

    pub fn set_history_window_command(&self, window: usize) -> AppCommand {
        AppCommand::SetHistoryWindow(window)
    }
}
