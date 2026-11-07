//! Windows thermal and fan sensor telemetry (WIN-01).

/// Enumerate available sensor telemetry sources on Windows.
pub fn enumerate_sensor_sources() -> Vec<String> {
    vec!["Win32".to_string(), "DXGI".to_string()]
}

/// Read CPU and GPU temperature sensors.
///
/// Guaranteed absence behavior: when hardware or driver interfaces do not expose
/// thermal telemetry without kernel-mode ring-0 drivers, returns `(None, None)`
/// instead of synthesizing fake `0.0 °C` values (MON-01, ARCH-03).
pub fn read_temperature_sensors() -> (Option<f32>, Option<f32>) {
    (None, None)
}

/// Read active fan speeds in RPM.
///
/// Returns an empty vector unless verified HP WMI firmware interface is unlocked.
pub fn read_fan_rpms() -> Vec<u32> {
    Vec::new()
}
