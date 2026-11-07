//! Global status strip banner component for GPUI-CE.

use crate::service::ServiceConnectionStatus;

#[derive(Debug, Clone, PartialEq)]
pub struct StatusStrip {
    pub service_status: ServiceConnectionStatus,
    pub polkit_status: String,
    pub battery_summary: Option<String>,
    pub fan_mode: String,
    pub power_profile: String,
    pub pending_mutation: Option<String>,
}

impl StatusStrip {
    pub fn new(
        service_status: ServiceConnectionStatus,
        polkit_status: impl Into<String>,
        fan_mode: impl Into<String>,
        power_profile: impl Into<String>,
    ) -> Self {
        Self {
            service_status,
            polkit_status: polkit_status.into(),
            battery_summary: None,
            fan_mode: fan_mode.into(),
            power_profile: power_profile.into(),
            pending_mutation: None,
        }
    }

    pub fn with_battery(mut self, summary: impl Into<String>) -> Self {
        self.battery_summary = Some(summary.into());
        self
    }

    pub fn with_pending(mut self, pending: impl Into<String>) -> Self {
        self.pending_mutation = Some(pending.into());
        self
    }
}
