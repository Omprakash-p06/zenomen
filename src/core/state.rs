//! The core's own record of what the window is showing.
//!
//! This is the single source of truth for UI state (FOUND-05): the UI holds
//! formatted strings, this holds the values those strings came from. Every
//! field changes only through [`AppState::apply`], so a screen can never show
//! something the core does not also know.
//!
//! Nothing here reads a sensor or formats a number for a screen — that is
//! `src/app.rs`'s job. This module answers two questions instead: what is true
//! right now, and what should the status strip say about it.

use crate::core::events::{AppEvent, ScreenId};
use crate::core::telemetry::{
    CHART_HEIGHT, CHART_WIDTH, TelemetryHistory, TelemetryPaths, TelemetryPoint,
};
use crate::hardware::{HardwareCapabilities, TelemetrySnapshot};

/// One flag per interface described by [`HardwareCapabilities`].
///
/// Kept next to the "is this machine degraded" question because that is the
/// only place the total matters; `HardwareCapabilities::reported_sensor_count`
/// supplies the numerator.
const SUBSYSTEM_COUNT: usize = 8;

/// How many samples the sparklines keep: 120 at 1 Hz is the two-minute window
/// fixed by the project's memory constraint (MON-11).
pub const HISTORY_WINDOW: usize = 120;

/// Everything the core knows about the current session.
#[derive(Debug, Clone)]
pub struct AppState {
    /// Screen the core believes is visible. Starts at Overview, like the UI.
    pub active_screen: ScreenId,
    /// Interfaces detected on this machine. Defaults to "none detected", so a
    /// state built before the provider reports is honest rather than optimistic.
    pub capabilities: HardwareCapabilities,
    /// Most recent sample, or `None` before the first one arrives. `None` is
    /// what makes the strip read "waiting for first reading" instead of a
    /// fabricated "0 of 12 sensors".
    pub latest_snapshot: Option<TelemetrySnapshot>,
    /// Samples retained per chart. Bounded by construction (MON-11).
    pub history_window: usize,
    /// Bounded telemetry sample buffer.
    pub history: TelemetryHistory,
    /// Most recent SVG path strings for the chart and sparklines.
    pub paths: TelemetryPaths,
    /// Last error encountered while sampling telemetry from the provider.
    pub last_sampling_error: Option<String>,
    /// Currently verified fan mode ("auto", "max", "manual", or empty if unread).
    pub fan_mode: String,
    /// Inflight fan mode change requested by user.
    pub pending_fan_mode: Option<String>,
    /// Currently active ACPI power profile.
    pub active_power_profile: String,
    /// Advertised ACPI power profile choices.
    pub power_profile_choices: Vec<String>,
    /// Granular result of the last applied combined preset.
    pub last_preset_result: Option<crate::service::types::PresetResult>,
    /// Hardware D-Bus service connection status.
    pub service_status: crate::service::client::ServiceConnectionStatus,
    /// Polkit authorization readiness description (DIAG-04).
    pub polkit_status: String,
    /// Last error encountered during hardware write / verification.
    pub last_control_error: Option<String>,
    /// Persistent application configuration (SYS-06).
    pub config: crate::config::AppConfig,
    /// Notification if loaded config was corrupt and safe defaults were used (SYS-07).
    pub config_corrupt_notice: Option<String>,
    /// Currently active RGB keyboard lighting profile (RGB-02).
    pub active_rgb_profile: Option<crate::rgb::RgbProfile>,
    /// RGB keyboard lighting power state (RGB-04).
    pub rgb_power_enabled: bool,
    /// Detected RGB keyboard interface variant (RGB-01).
    pub rgb_variant: Option<crate::hardware::rgb::RgbInterfaceVariant>,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            active_screen: ScreenId::Overview,
            capabilities: HardwareCapabilities::default(),
            latest_snapshot: None,
            history_window: HISTORY_WINDOW,
            history: TelemetryHistory::new(),
            paths: TelemetryPaths::default(),
            last_sampling_error: None,
            fan_mode: String::new(),
            pending_fan_mode: None,
            active_power_profile: String::new(),
            power_profile_choices: Vec::new(),
            last_preset_result: None,
            service_status: crate::service::client::ServiceConnectionStatus::Disconnected,
            polkit_status: "Service disconnected".to_string(),
            last_control_error: None,
            config: crate::config::AppConfig::default(),
            config_corrupt_notice: None,
            active_rgb_profile: Some(crate::rgb::builtin_presets().swap_remove(0)),
            rgb_power_enabled: true,
            rgb_variant: None,
        }
    }
}

impl AppState {
    /// Fold one event into the state. The only mutator.
    pub fn apply(&mut self, event: &AppEvent) {
        match event {
            AppEvent::ScreenChanged(screen) => self.active_screen = *screen,
            AppEvent::CapabilitiesLoaded(capabilities) => {
                self.capabilities = *capabilities;
                self.rgb_variant = capabilities.rgb_variant;
            }
            AppEvent::TelemetryUpdated(snapshot) => {
                self.last_sampling_error = None;
                self.latest_snapshot = Some(snapshot.clone());
                let point = TelemetryPoint::from_snapshot(snapshot);
                self.history.push(point);
                let (cpu_path, gpu_path, ram_path) =
                    self.history.generate_chart_paths(CHART_WIDTH, CHART_HEIGHT);
                let (cpu_sparkline, gpu_sparkline, ram_sparkline) =
                    self.history.generate_sparkline_paths();
                self.paths = TelemetryPaths {
                    cpu_path,
                    gpu_path,
                    ram_path,
                    cpu_sparkline,
                    gpu_sparkline,
                    ram_sparkline,
                };
            }
            AppEvent::TelemetrySamplingFailed(error) => {
                self.last_sampling_error = Some(error.clone());
            }
            AppEvent::HistoryWindowChanged(window_size) => {
                self.history_window = *window_size;
                self.history.set_capacity(*window_size);
                let (cpu_path, gpu_path, ram_path) =
                    self.history.generate_chart_paths(CHART_WIDTH, CHART_HEIGHT);
                let (cpu_sparkline, gpu_sparkline, ram_sparkline) =
                    self.history.generate_sparkline_paths();
                self.paths = TelemetryPaths {
                    cpu_path,
                    gpu_path,
                    ram_path,
                    cpu_sparkline,
                    gpu_sparkline,
                    ram_sparkline,
                };
            }
            AppEvent::FanModeChanged(res) => {
                self.pending_fan_mode = None;
                match res {
                    Ok(mode) => {
                        self.fan_mode = mode.clone();
                        self.last_control_error = None;
                    }
                    Err(e) => {
                        self.last_control_error = Some(e.clone());
                    }
                }
            }
            AppEvent::PowerProfileChanged(res) => {
                match res {
                    Ok(profile) => {
                        self.active_power_profile = profile.clone();
                        self.last_control_error = None;
                    }
                    Err(e) => {
                        self.last_control_error = Some(e.clone());
                    }
                }
            }
            AppEvent::PresetApplied(res) => {
                match res {
                    Ok(preset_res) => {
                        if preset_res.fan_applied {
                            self.fan_mode = if preset_res.preset_name == "gaming" {
                                "max".to_string()
                            } else {
                                "auto".to_string()
                            };
                        }
                        self.last_preset_result = Some(preset_res.clone());
                        if !preset_res.partial_success {
                            self.last_control_error = None;
                        } else {
                            self.last_control_error = Some(preset_res.message.clone());
                        }
                    }
                    Err(e) => {
                        self.last_control_error = Some(e.clone());
                    }
                }
            }
            AppEvent::ServiceStatusChanged(status) => {
                self.service_status = status.clone();
                if !status.is_connected() {
                    self.polkit_status = "Service disconnected".to_string();
                }
            }
            AppEvent::PolkitStatusChanged(status_desc) => {
                self.polkit_status = status_desc.clone();
            }
            AppEvent::RgbProfileApplied(res) => {
                match res {
                    Ok(profile) => {
                        self.active_rgb_profile = Some(profile.clone());
                        self.rgb_power_enabled = !profile.is_off();
                        self.last_control_error = None;
                    }
                    Err(e) => {
                        self.last_control_error = Some(e.clone());
                    }
                }
            }
            AppEvent::RgbPowerChanged(res) => {
                match res {
                    Ok(enabled) => {
                        self.rgb_power_enabled = *enabled;
                        self.last_control_error = None;
                    }
                    Err(e) => {
                        self.last_control_error = Some(e.clone());
                    }
                }
            }
            AppEvent::ControlErrorDismissed => {
                self.last_control_error = None;
            }
            AppEvent::ConfigLoaded(config) => {
                self.config = config.clone();
                self.history_window = config.telemetry.history_window;
                self.history.set_capacity(config.telemetry.history_window);
                let (cpu_path, gpu_path, ram_path) =
                    self.history.generate_chart_paths(CHART_WIDTH, CHART_HEIGHT);
                let (cpu_sparkline, gpu_sparkline, ram_sparkline) =
                    self.history.generate_sparkline_paths();
                self.paths = TelemetryPaths {
                    cpu_path,
                    gpu_path,
                    ram_path,
                    cpu_sparkline,
                    gpu_sparkline,
                    ram_sparkline,
                };
            }
            AppEvent::ConfigCorrupted(notice) => {
                self.config_corrupt_notice = Some(notice.clone());
            }
        }
    }

    /// The status strip's one line of text.
    ///
    /// Before the first sample it says so rather than claiming a count: the
    /// project's core value forbids a number the app has not measured.
    pub fn status_text(&self) -> String {
        if let Some(error) = &self.last_sampling_error {
            return format!("Mock provider · sampling error: {error}");
        }

        match &self.latest_snapshot {
            Some(snapshot) if snapshot.has_any_reading() => format!(
                "Mock provider · {} of {} sensors reported",
                snapshot.reported_sensor_count(),
                TelemetrySnapshot::EXPECTED_SENSOR_COUNT
            ),
            // No sample yet, or a sample that carried nothing: both are "not
            // measured", and neither may be rendered as "0 of 12".
            _ => "Mock provider · waiting for first reading".to_string(),
        }
    }

    /// True when the machine exposes fewer interfaces than the app can use, or
    /// when the latest sample is missing sensors the machine is supposed to have.
    ///
    /// A machine that has not been sampled yet is not degraded — it is simply
    /// early. Marking it degraded would put a gold warning on every cold start.
    pub fn is_degraded(&self) -> bool {
        if self.last_sampling_error.is_some() {
            return true;
        }

        if self.capabilities.reported_sensor_count() < SUBSYSTEM_COUNT {
            return true;
        }

        match &self.latest_snapshot {
            Some(snapshot) if snapshot.has_any_reading() => {
                snapshot.reported_sensor_count() < TelemetrySnapshot::EXPECTED_SENSOR_COUNT
            }
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fully_capable() -> HardwareCapabilities {
        HardwareCapabilities::all_supported()
    }

    #[test]
    fn subsystem_count_matches_capability_flags() {
        // `SUBSYSTEM_COUNT` is a literal; this keeps it in step with the struct
        // if an interface is ever added or removed.
        assert_eq!(fully_capable().reported_sensor_count(), SUBSYSTEM_COUNT);
    }

    #[test]
    fn starts_waiting_for_a_reading_and_reports_nothing_supported() {
        let state = AppState::default();
        assert_eq!(state.active_screen, ScreenId::Overview);
        assert!(state.latest_snapshot.is_none());
        assert_eq!(
            state.status_text(),
            "Mock provider · waiting for first reading"
        );
        // An empty capability set means "nothing detected", which is exactly the
        // zero-capability machine FOUND-04 has to explain — so: degraded.
        assert!(state.is_degraded());
    }

    #[test]
    fn capabilities_loaded_clears_degradation() {
        let mut state = AppState::default();
        state.apply(&AppEvent::CapabilitiesLoaded(fully_capable()));

        assert!(state.capabilities.has_cpu_metrics());
        assert!(!state.is_degraded());
        // Still no sample, so the copy has not moved on.
        assert_eq!(
            state.status_text(),
            "Mock provider · waiting for first reading"
        );
    }

    #[test]
    fn one_missing_interface_is_degraded() {
        let mut capabilities = fully_capable();
        capabilities.fan_control = false;
        let mut state = AppState::default();

        state.apply(&AppEvent::CapabilitiesLoaded(capabilities));

        assert!(state.is_degraded());
    }

    #[test]
    fn a_full_sample_reports_twelve_of_twelve() {
        let mut state = AppState::default();
        state.apply(&AppEvent::CapabilitiesLoaded(fully_capable()));
        state.apply(&AppEvent::TelemetryUpdated(full_snapshot()));

        assert_eq!(
            state.status_text(),
            "Mock provider · 12 of 12 sensors reported"
        );
        assert!(!state.is_degraded());
    }

    #[test]
    fn a_partial_sample_is_degraded_but_still_counted() {
        let mut snapshot = full_snapshot();
        snapshot.gpu_name = None;
        let mut state = AppState::default();

        state.apply(&AppEvent::CapabilitiesLoaded(fully_capable()));
        state.apply(&AppEvent::TelemetryUpdated(snapshot));

        assert_eq!(
            state.status_text(),
            "Mock provider · 11 of 12 sensors reported"
        );
        assert!(state.is_degraded());
    }

    #[test]
    fn screen_changed_moves_the_pointer() {
        let mut state = AppState::default();
        state.apply(&AppEvent::ScreenChanged(ScreenId::Diagnostics));

        assert_eq!(state.active_screen, ScreenId::Diagnostics);
    }

    #[test]
    fn sampling_failure_marks_degraded_and_shows_error() {
        let mut state = AppState::default();
        state.apply(&AppEvent::CapabilitiesLoaded(fully_capable()));
        state.apply(&AppEvent::TelemetrySamplingFailed("hwmon timeout".into()));

        assert!(state.is_degraded());
        assert_eq!(
            state.status_text(),
            "Mock provider · sampling error: hwmon timeout"
        );
    }

    #[test]
    fn history_window_changed_resizes_history_and_updates_paths() {
        let mut state = AppState::default();
        state.apply(&AppEvent::HistoryWindowChanged(60));
        assert_eq!(state.history_window, 60);
        assert_eq!(state.history.capacity(), 60);
    }

    fn full_snapshot() -> TelemetrySnapshot {
        TelemetrySnapshot {
            cpu_name: "AMD Ryzen 7 5800H".to_string(),
            cpu_utilization: 38.0,
            cpu_temperature_c: Some(68.0),
            cpu_frequency_mhz: 3200,
            gpu_name: Some("NVIDIA GeForce RTX 3060".to_string()),
            gpu_utilization: Some(62.0),
            gpu_temperature_c: Some(71.0),
            ram_total_mb: 16_384,
            ram_used_mb: 9_820,
            fan_rpms: vec![2_450, 2_400],
            fan_mode: "auto".to_string(),
            power_profile: "balanced".to_string(),
            power_profiles: vec!["quiet".to_string(), "balanced".to_string()],
            rgb_profile: Some("aurora".to_string()),
        }
    }
}
