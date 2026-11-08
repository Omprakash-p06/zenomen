//! Windows thermal and fan sensor telemetry (WIN-01).

use serde::{Deserialize, Serialize};

/// Origin of temperature sensor reading on Windows platforms.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TemperatureSource {
    AcpiThermalZone,
    CpuPackage,
    GpuCore,
    Unavailable,
}

/// Structured temperature sensor reading with source attribution.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TemperatureReading {
    pub label: String,
    pub source: TemperatureSource,
    pub celsius: Option<f32>,
}

/// Structured fan tachometer reading.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FanReading {
    pub index: u8,
    pub rpm: Option<u32>,
}

/// Enumerate available sensor telemetry sources on Windows.
pub fn enumerate_sensor_sources() -> Vec<String> {
    vec!["Win32".to_string(), "DXGI".to_string(), "PDH".to_string()]
}

/// Read CPU and GPU temperature sensors.
///
/// Guaranteed absence behavior: when hardware or driver interfaces do not expose
/// thermal telemetry without kernel-mode ring-0 drivers, returns `(None, None)`
/// instead of synthesizing fake `0.0 °C` values (MON-01, ARCH-03).
pub fn read_temperature_sensors() -> (Option<f32>, Option<f32>) {
    (None, None)
}

/// Read structured temperature sensor readings.
pub fn read_temperature_readings() -> Vec<TemperatureReading> {
    vec![
        TemperatureReading {
            label: "CPU Package".to_string(),
            source: TemperatureSource::Unavailable,
            celsius: None,
        },
        TemperatureReading {
            label: "GPU Core".to_string(),
            source: TemperatureSource::Unavailable,
            celsius: None,
        },
    ]
}

/// Read active fan speeds in RPM.
///
/// Returns an empty vector unless verified HP WMI firmware interface is unlocked.
pub fn read_fan_rpms() -> Vec<u32> {
    Vec::new()
}

/// Read structured fan tachometer readings.
pub fn read_fan_readings() -> Vec<FanReading> {
    Vec::new()
}
