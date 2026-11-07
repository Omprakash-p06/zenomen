//! Provider for a platform whose hardware backend is not implemented yet.
//!
//! Windows is kept *architecture-ready* — it must compile and it must not lie
//! (PROJECT.md: "Windows kept architecture-ready but unadvertised until its
//! interfaces are validated"). This provider is how that is expressed: it
//! reports zero capabilities and refuses every sample, which the UI already
//! renders as `Unavailable` panels. It exists so a Windows build is honest
//! instead of reaching for Linux sysfs paths that do not exist.
//!
//! Do not add speculative hardware reads here. A capability is only added to
//! `capabilities()` once a validated implementation exists behind it.

use crate::hardware::{HardwareCapabilities, HardwareProvider, TelemetrySnapshot};

/// A provider for a platform that has no backend yet.
#[derive(Debug, Clone, Copy, Default)]
pub struct UnavailableHardwareProvider;

impl UnavailableHardwareProvider {
    /// Construct the placeholder provider.
    pub fn new() -> Self {
        Self
    }
}

impl HardwareProvider for UnavailableHardwareProvider {
    fn capabilities(&self) -> HardwareCapabilities {
        HardwareCapabilities::none()
    }

    fn sample_telemetry(&self) -> Result<TelemetrySnapshot, String> {
        Err("hardware telemetry is not implemented for this platform".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reports_nothing_rather_than_guessing() {
        let provider = UnavailableHardwareProvider::new();
        let capabilities = provider.capabilities();

        assert_eq!(capabilities.reported_sensor_count(), 0);
        assert!(!capabilities.has_cpu_metrics());
        assert!(!capabilities.has_battery_metrics());

        let sample = provider.sample_telemetry();
        assert!(sample.is_err(), "an unimplemented platform must not sample");
    }
}
