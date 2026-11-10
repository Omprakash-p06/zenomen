//! Root GPUI application element tree and event bridge (Section 5.4).
//!
//! Maintains strict separation between GPUI view-models and the core domain state.
//! Consumes typed [`AppEvent`] messages and dispatches typed [`AppCommand`] requests.

use crate::config::AppConfig;
use crate::core::events::{AppCommand, AppEvent, ScreenId};
use crate::core::state::AppState;
use crate::hardware::registry::SensorRegistry;
use crate::ui::components::SidebarView;
use crate::ui::screens::{
    CoolingScreen, DiagnosticsScreen, LightingScreen, OverviewScreen, PerformanceScreen,
    SettingsScreen,
};
use crate::ui::theme::Theme;
use std::sync::mpsc::SyncSender;

/// The primary application view model for the GPUI-CE desktop client.
pub struct GpuiApp {
    pub theme: Theme,
    pub state: AppState,
    pub active_screen: ScreenId,
    pub sidebar: SidebarView,
    pub overview: OverviewScreen,
    pub cooling: CoolingScreen,
    pub performance: PerformanceScreen,
    pub lighting: LightingScreen,
    pub settings: SettingsScreen,
    pub diagnostics: DiagnosticsScreen,
    pub command_sender: Option<SyncSender<AppCommand>>,
}

impl GpuiApp {
    pub fn new(command_sender: Option<SyncSender<AppCommand>>) -> Self {
        let state = AppState::default();
        let config = AppConfig::default();
        let active_screen = ScreenId::Overview;

        let sidebar = SidebarView::new(active_screen, env!("CARGO_PKG_VERSION"));
        let overview = OverviewScreen::from_state(&state);
        let cooling = CoolingScreen::from_state(&state);
        let performance = PerformanceScreen::from_state(&state);
        let lighting = LightingScreen::from_state(&state);
        let settings = SettingsScreen::from_config(&config);
        let diagnostics = DiagnosticsScreen::from_state(&state, None);

        Self {
            theme: Theme::default(),
            state,
            active_screen,
            sidebar,
            overview,
            cooling,
            performance,
            lighting,
            settings,
            diagnostics,
            command_sender,
        }
    }

    /// Dispatch a typed AppCommand to the core worker.
    pub fn send_command(&self, cmd: AppCommand) {
        if let Some(ref sender) = self.command_sender {
            let _ = sender.try_send(cmd);
        }
    }

    /// Fold an incoming AppEvent into the active UI state and sub-screens.
    pub fn handle_event(&mut self, event: AppEvent, registry: Option<&SensorRegistry>) {
        match event {
            AppEvent::TelemetryUpdated(snapshot) => {
                self.state.latest_snapshot = Some(snapshot);
                self.overview = OverviewScreen::from_state(&self.state);
                self.cooling = CoolingScreen::from_state(&self.state);
                self.performance = PerformanceScreen::from_state(&self.state);
            }
            AppEvent::TelemetrySamplingFailed(err) => {
                self.state.last_sampling_error = Some(err);
            }
            AppEvent::CapabilitiesLoaded(caps) => {
                self.state.capabilities = caps;
                self.cooling = CoolingScreen::from_state(&self.state);
                self.lighting = LightingScreen::from_state(&self.state);
            }
            AppEvent::FanModeRequested(mode) => {
                self.state.pending_fan_mode = Some(mode);
                self.cooling = CoolingScreen::from_state(&self.state);
            }
            AppEvent::FanModeChanged(res) => {
                match res {
                    Ok(mode) => {
                        self.state.fan_mode = mode;
                        self.state.pending_fan_mode = None;
                    }
                    Err(e) => {
                        self.state.last_control_error = Some(e);
                        self.state.pending_fan_mode = None;
                    }
                }
                self.cooling = CoolingScreen::from_state(&self.state);
            }
            AppEvent::PowerProfileChanged(res) => {
                match res {
                    Ok(result) => {
                        self.state.active_power_profile = result.profile;
                        self.state.power_profile_verified = result.status.is_readback_verified();
                    }
                    Err(e) => self.state.last_control_error = Some(e),
                }
                self.performance = PerformanceScreen::from_state(&self.state);
            }
            AppEvent::RgbPowerChanged(res) => {
                match res {
                    Ok(power) => self.state.rgb_power_enabled = power,
                    Err(e) => self.state.last_control_error = Some(e),
                }
                self.lighting = LightingScreen::from_state(&self.state);
            }
            AppEvent::RgbProfileApplied(res) => {
                match res {
                    Ok(profile) => self.state.active_rgb_profile = Some(profile),
                    Err(e) => self.state.last_control_error = Some(e),
                }
                self.lighting = LightingScreen::from_state(&self.state);
            }
            AppEvent::ServiceStatusChanged(status) => {
                self.state.service_status = status;
                self.diagnostics = DiagnosticsScreen::from_state(&self.state, registry);
            }
            AppEvent::PolkitStatusChanged(status) => {
                self.state.polkit_status = status;
                self.diagnostics = DiagnosticsScreen::from_state(&self.state, registry);
            }
            AppEvent::ConfigLoaded(config) => {
                self.settings = SettingsScreen::from_config(&config);
            }
            AppEvent::ScreenChanged(screen) => {
                self.active_screen = screen;
                self.sidebar = SidebarView::new(screen, env!("CARGO_PKG_VERSION"));
            }
            _ => {}
        }
    }
}
