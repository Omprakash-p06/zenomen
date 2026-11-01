//! Typed messages between the core and the UI.
//!
//! `AppCommand` travels UI -> core: a user action becomes a command and nothing
//! else. `AppEvent` travels core -> UI: every visible change is the result of
//! one of these, applied inside `slint::invoke_from_event_loop`. There is no
//! other path into the UI, which is what FOUND-05 requires.

use crate::hardware::{HardwareCapabilities, TelemetrySnapshot};

/// The six screens, in sidebar order.
///
/// This is a compile-time set, not data: an unknown index can never become a
/// seventh screen, and `from_index` is the only way to turn a number from the
/// UI back into one (threat T-01-04).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScreenId {
    Overview,
    Cooling,
    Performance,
    Lighting,
    Settings,
    Diagnostics,
}

impl ScreenId {
    /// Sidebar order. Index into this array is the screen's `active-screen`.
    pub const ALL: [Self; 6] = [
        Self::Overview,
        Self::Cooling,
        Self::Performance,
        Self::Lighting,
        Self::Settings,
        Self::Diagnostics,
    ];

    /// Index this screen is routed by in `ui/app.slint`.
    pub fn index(self) -> usize {
        match self {
            Self::Overview => 0,
            Self::Cooling => 1,
            Self::Performance => 2,
            Self::Lighting => 3,
            Self::Settings => 4,
            Self::Diagnostics => 5,
        }
    }

    /// The screen at `index`, or `None` when the index is out of range.
    ///
    /// Callers default to [`ScreenId::Overview`] rather than trusting a number
    /// that came from the UI.
    pub fn from_index(index: usize) -> Option<Self> {
        Self::ALL.get(index).copied()
    }

    /// Human name, used for logs and for the status strip.
    pub fn title(self) -> &'static str {
        match self {
            Self::Overview => "Overview",
            Self::Cooling => "Cooling",
            Self::Performance => "Performance",
            Self::Lighting => "Lighting",
            Self::Settings => "Settings",
            Self::Diagnostics => "Diagnostics",
        }
    }
}

/// A user action, queued for the core. The UI never does the work itself.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AppCommand {
    /// The user selected a screen.
    Navigate(ScreenId),
    /// The user asked for a sample now instead of waiting for the next tick.
    RefreshTelemetry,
    /// The user changed how many samples the charts keep (MON-11).
    SetHistoryWindow(usize),
    /// Request to change the fan mode ("auto", "max").
    SetFanMode(String),
    /// Request to change the ACPI power profile.
    SetPowerProfile(String),
    /// Request to apply a combined preset ("quiet", "balanced", "gaming").
    ApplyPreset(String),
    /// Dismiss the active hardware control error banner.
    DismissControlError,
}

/// Something the core wants the UI to show.
///
/// Each variant maps to one `slint::invoke_from_event_loop` closure in
/// `src/app.rs`; nothing here carries a sysfs path or a command to run.
// `large_enum_variant`: a sample is the only large payload, it is produced once
// per tick and moved straight through a channel — boxing it would add an
// allocation per sample and buy nothing, because these are never accumulated.
#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone, PartialEq)]
pub enum AppEvent {
    /// The active screen changed in the core (for example after a redirect to
    /// Diagnostics from an UnavailablePanel).
    ScreenChanged(ScreenId),
    /// The provider reported which interfaces this machine exposes.
    CapabilitiesLoaded(HardwareCapabilities),
    /// One telemetry sample.
    TelemetryUpdated(TelemetrySnapshot),
    /// Telemetry sampling failed with an error message from the provider.
    TelemetrySamplingFailed(String),
    /// Telemetry history window size was changed (MON-11).
    HistoryWindowChanged(usize),
    /// Fan mode change completed with confirmed mode or error.
    FanModeChanged(Result<String, String>),
    /// Power profile change completed with confirmed profile or error.
    PowerProfileChanged(Result<String, String>),
    /// Combined preset applied with granular PresetResult or error.
    PresetApplied(Result<crate::service::types::PresetResult, String>),
    /// Hardware D-Bus service connection status changed.
    ServiceStatusChanged(crate::service::client::ServiceConnectionStatus),
    /// Polkit authorization readiness status changed.
    PolkitStatusChanged(String),
    /// Control error banner dismissed.
    ControlErrorDismissed,
}
