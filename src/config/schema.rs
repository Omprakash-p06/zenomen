//! Configuration schema for ZenOmen.
//!
//! Strongly-typed configuration structures covering general UI behavior
//! and telemetry polling rates (SYS-06, SYS-08, SYS-09).

use serde::{Deserialize, Serialize};

/// Minimum allowed telemetry polling interval in milliseconds (SYS-09).
pub const MIN_POLLING_INTERVAL_MS: u64 = 500;
/// Maximum allowed telemetry polling interval in milliseconds (SYS-09).
pub const MAX_POLLING_INTERVAL_MS: u64 = 5000;
/// Default telemetry polling interval in milliseconds.
pub const DEFAULT_POLLING_INTERVAL_MS: u64 = 1000;

/// Root configuration structure for ZenOmen.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct AppConfig {
    #[serde(default)]
    pub general: GeneralConfig,
    #[serde(default)]
    pub telemetry: TelemetryConfig,
    #[serde(default)]
    pub display: DisplayConfig,
}

/// General application preferences.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GeneralConfig {
    /// Active theme name (default "dark" - SYS-08).
    #[serde(default = "default_theme")]
    pub theme: String,
    /// Whether to start minimized in tray without showing window on cold start (SYS-03).
    #[serde(default)]
    pub start_minimized: bool,
    /// Whether closing window hides it into tray instead of exiting (SYS-04).
    #[serde(default = "default_true")]
    pub close_to_tray: bool,
    /// Whether application is configured to launch at login (SYS-02).
    #[serde(default)]
    pub autostart: bool,
}

impl Default for GeneralConfig {
    fn default() -> Self {
        Self {
            theme: default_theme(),
            start_minimized: false,
            close_to_tray: true,
            autostart: false,
        }
    }
}

fn default_theme() -> String {
    "dark".to_string()
}

fn default_true() -> bool {
    true
}

/// Telemetry sampling and history window configuration.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TelemetryConfig {
    /// Telemetry chart samples window (default 120 - MON-11).
    #[serde(default = "default_history_window")]
    pub history_window: usize,
    /// Telemetry polling rate in milliseconds, clamped to safe bounds [500, 5000] (SYS-09).
    #[serde(default = "default_polling_interval")]
    pub polling_interval_ms: u64,
}

impl Default for TelemetryConfig {
    fn default() -> Self {
        Self {
            history_window: default_history_window(),
            polling_interval_ms: default_polling_interval(),
        }
    }
}

fn default_history_window() -> usize {
    120
}

fn default_polling_interval() -> u64 {
    DEFAULT_POLLING_INTERVAL_MS
}

/// Display refresh rate and combined system profile preferences.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DisplayConfig {
    /// Preferred refresh rate in Hz for quiet/battery saving profiles (default 60).
    #[serde(default = "default_quiet_refresh_rate")]
    pub preferred_quiet_refresh_rate: u32,
    /// Preferred refresh rate in Hz for active/performance profiles (default 144).
    #[serde(default = "default_active_refresh_rate")]
    pub preferred_active_refresh_rate: u32,
    /// Restore display refresh rate on application exit.
    #[serde(default)]
    pub restore_refresh_rate_on_exit: bool,
    /// Preset to automatically apply upon application startup (e.g. "balanced", "quiet", "performance").
    #[serde(default)]
    pub apply_profile_on_startup: Option<String>,
}

fn default_quiet_refresh_rate() -> u32 {
    60
}

fn default_active_refresh_rate() -> u32 {
    144
}

impl Default for DisplayConfig {
    fn default() -> Self {
        Self {
            preferred_quiet_refresh_rate: default_quiet_refresh_rate(),
            preferred_active_refresh_rate: default_active_refresh_rate(),
            restore_refresh_rate_on_exit: false,
            apply_profile_on_startup: None,
        }
    }
}

impl AppConfig {
    /// Validate configuration and clamp any fields outside safe bounds.
    pub fn sanitize(&mut self) {
        // Enforce safe polling interval bounds (SYS-09)
        self.telemetry.polling_interval_ms = self
            .telemetry
            .polling_interval_ms
            .clamp(MIN_POLLING_INTERVAL_MS, MAX_POLLING_INTERVAL_MS);

        // Sanitize history window to one of supported intervals
        if ![30, 60, 120, 300].contains(&self.telemetry.history_window) {
            self.telemetry.history_window = 120;
        }

        // Enforce supported theme
        if self.general.theme.trim().is_empty() {
            self.general.theme = "dark".to_string();
        }

        // Enforce non-zero refresh rates
        if self.display.preferred_quiet_refresh_rate == 0 {
            self.display.preferred_quiet_refresh_rate = 60;
        }
        if self.display.preferred_active_refresh_rate == 0 {
            self.display.preferred_active_refresh_rate = 144;
        }
    }
}
