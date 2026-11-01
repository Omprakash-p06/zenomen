//! Provider contract shared by every hardware backend.
//!
//! This file deliberately contains *only* data types and one trait. It is
//! authored in Wave 2 so that `crate::core::events::AppEvent` can name
//! `HardwareCapabilities` and `TelemetrySnapshot` and the crate compiles at the
//! end of this wave; plan 01-03 adds the concrete `MockHardware` implementation
//! without changing these declarations.
//!
//! Two rules come from the project's core value and are encoded here rather
//! than in the UI:
//!
//! * A sensor that is not present is `None` (or an empty collection) — never a
//!   fabricated `0`. The UI renders `Unavailable` from the absence, so the type
//!   system, not a comment, is what prevents invented numbers.
//! * Capabilities are queried per subsystem (`has_fan_control()`, …) rather
//!   than as one aggregate boolean, because each screen has to name the exact
//!   interface that is missing.

use serde::{Deserialize, Serialize};

/// One boolean per hardware interface ZenOmen can use.
///
/// `Default` is "nothing is supported": a machine where no capability was
/// detected must still launch and explain itself (FOUND-04), so the safe
/// default is the one that produces `Unavailable` panels rather than blank
/// panels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct HardwareCapabilities {
    /// CPU model, utilization, frequency and temperature can be read.
    pub cpu_metrics: bool,
    /// GPU name, utilization and temperature can be read.
    pub gpu_metrics: bool,
    /// System RAM total/used can be read.
    pub ram_metrics: bool,
    /// At least one fan tachometer can be read.
    pub fan_rpm: bool,
    /// A writable fan mode interface (`pwm1_enable`) is present.
    pub fan_control: bool,
    /// ACPI `platform_profile` is exposed and readable.
    pub power_profiles: bool,
    /// A supported keyboard RGB interface was detected.
    pub rgb_lighting: bool,
    /// Battery charge/power can be read.
    pub battery_metrics: bool,
}

impl HardwareCapabilities {
    /// Every interface reported as available.
    ///
    /// Used by the Wave 2 placeholder until plan 01-03 installs the mock
    /// provider, and by tests that want the fully-populated UI.
    pub fn all_supported() -> Self {
        Self {
            cpu_metrics: true,
            gpu_metrics: true,
            ram_metrics: true,
            fan_rpm: true,
            fan_control: true,
            power_profiles: true,
            rgb_lighting: true,
            battery_metrics: true,
        }
    }

    /// Nothing reported — the zero-capability machine (FOUND-04).
    ///
    /// Also the `Default`, so a capabilities value that was never filled in
    /// degrades the UI instead of pretending the hardware is there.
    pub fn none() -> Self {
        Self {
            cpu_metrics: false,
            gpu_metrics: false,
            ram_metrics: false,
            fan_rpm: false,
            fan_control: false,
            power_profiles: false,
            rgb_lighting: false,
            battery_metrics: false,
        }
    }

    /// True when the CPU can be monitored.
    pub fn has_cpu_metrics(&self) -> bool {
        self.cpu_metrics
    }

    /// True when the GPU can be monitored.
    pub fn has_gpu_metrics(&self) -> bool {
        self.gpu_metrics
    }

    /// True when system RAM can be monitored.
    pub fn has_ram_metrics(&self) -> bool {
        self.ram_metrics
    }

    /// True when at least one fan tachometer is readable.
    pub fn has_fan_rpm(&self) -> bool {
        self.fan_rpm
    }

    /// True when fan *control* (not just fan speed) is writable.
    pub fn has_fan_control(&self) -> bool {
        self.fan_control
    }

    /// True when ACPI platform profiles are exposed.
    pub fn has_power_profiles(&self) -> bool {
        self.power_profiles
    }

    /// True when keyboard lighting is controllable.
    pub fn has_rgb_lighting(&self) -> bool {
        self.rgb_lighting
    }

    /// True when battery telemetry is readable.
    pub fn has_battery_metrics(&self) -> bool {
        self.battery_metrics
    }

    /// How many of the eight subsystem interfaces are reported.
    ///
    /// Diagnostics uses this for its summary line. It is *not* the sensor
    /// count — see [`TelemetrySnapshot::reported_sensor_count`].
    pub fn reported_sensor_count(&self) -> usize {
        [
            self.cpu_metrics,
            self.gpu_metrics,
            self.ram_metrics,
            self.fan_rpm,
            self.fan_control,
            self.power_profiles,
            self.rgb_lighting,
            self.battery_metrics,
        ]
        .iter()
        .filter(|reported| **reported)
        .count()
    }
}

/// One immutable telemetry sample.
///
/// Optional fields are `None` when the provider could not read that sensor this
/// tick. The UI must render `Unavailable` for those — never a zero.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct TelemetrySnapshot {
    /// CPU model string, e.g. `AMD Ryzen 7 5800H`.
    pub cpu_name: String,
    /// Aggregate CPU utilization in percent (0–100).
    pub cpu_utilization: f32,
    /// Package/CPU temperature in degrees Celsius.
    pub cpu_temperature_c: Option<f32>,
    /// Current CPU frequency in MHz.
    pub cpu_frequency_mhz: u32,
    /// GPU model string.
    pub gpu_name: Option<String>,
    /// GPU utilization in percent (0–100).
    pub gpu_utilization: Option<f32>,
    /// GPU temperature in degrees Celsius.
    pub gpu_temperature_c: Option<f32>,
    /// Installed RAM in mebibytes.
    pub ram_total_mb: u64,
    /// RAM currently in use in mebibytes.
    pub ram_used_mb: u64,
    /// One tachometer reading per detected fan.
    pub fan_rpms: Vec<u32>,
    /// Active fan mode (`auto`, `max`, `manual`, …) or empty when unknown.
    pub fan_mode: String,
    /// Active ACPI platform profile or empty when unknown.
    pub power_profile: String,
    /// Profiles the machine advertises (`platform_profile_choices`).
    pub power_profiles: Vec<String>,
    /// Active keyboard RGB profile name, when lighting is supported.
    pub rgb_profile: Option<String>,
}

impl TelemetrySnapshot {
    /// The number of sensors a fully populated sample is expected to carry.
    ///
    /// Matches the status strip copy in `01-UI-SPEC.md` ("12 of 12 sensors
    /// reported"). Keep the two in step if a sensor is added.
    pub const EXPECTED_SENSOR_COUNT: usize = 12;

    /// How many of the twelve expected sensors this sample actually carries.
    ///
    /// Absence is measured, not guessed: a zero utilization reading still
    /// counts as reported (the CPU is merely idle), which is why presence is
    /// derived from the model name rather than from the value.
    pub fn reported_sensor_count(&self) -> usize {
        [
            !self.cpu_name.is_empty(),
            self.cpu_temperature_c.is_some(),
            self.cpu_frequency_mhz > 0,
            self.gpu_name.is_some(),
            self.gpu_utilization.is_some(),
            self.gpu_temperature_c.is_some(),
            self.ram_total_mb > 0,
            self.ram_used_mb > 0,
            !self.fan_rpms.is_empty(),
            !self.fan_mode.is_empty(),
            !self.power_profile.is_empty(),
            self.rgb_profile.is_some(),
        ]
        .iter()
        .filter(|reported| **reported)
        .count()
    }

    /// True when this snapshot carries no reading at all — the "waiting for
    /// first reading" state, as distinct from "sensor unavailable".
    pub fn has_any_reading(&self) -> bool {
        self.reported_sensor_count() > 0
    }
}

impl Default for HardwareCapabilities {
    fn default() -> Self {
        Self::none()
    }
}

/// The single seam between the core and a hardware backend.
///
/// `Send + Sync` is required because the monitoring worker samples from its own
/// thread; the UI thread never holds one.
pub trait HardwareProvider: Send + Sync {
    /// Which interfaces this machine actually exposes.
    fn capabilities(&self) -> HardwareCapabilities;

    /// Take one telemetry sample. `Err` carries a human-readable reason the
    /// UI can show in the status strip; it must never be a panic.
    fn sample_telemetry(&self) -> Result<TelemetrySnapshot, String>;
}
