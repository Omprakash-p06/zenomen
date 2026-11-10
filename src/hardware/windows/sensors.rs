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

/// Read ACPI Thermal Zone temperature in Celsius via Windows Performance Counters (PDH).
///
/// Guaranteed absence behavior: when hardware or driver interfaces do not expose
/// thermal telemetry without kernel-mode ring-0 drivers, returns `None`
/// instead of synthesizing fake `0.0 °C` values (MON-01, ARCH-03).
#[cfg(windows)]
pub fn read_acpi_thermal_zone_temp() -> Option<f32> {
    use windows_sys::Win32::System::Performance::{
        PDH_FMT_COUNTERVALUE, PDH_FMT_DOUBLE, PdhAddEnglishCounterW, PdhCloseQuery,
        PdhCollectQueryData, PdhGetFormattedCounterValue, PdhOpenQueryW,
    };

    let mut query: isize = 0;
    let status = unsafe { PdhOpenQueryW(std::ptr::null(), 0, &mut query) };
    if status != 0 {
        return None;
    }

    struct Candidate {
        path: &'static str,
        is_high_precision: bool,
    }

    let candidates = [
        Candidate {
            path: r"\Thermal Zone Information(\_TZ.THRM)\High Precision Temperature",
            is_high_precision: true,
        },
        Candidate {
            path: r"\Thermal Zone Information(\_TZ.THRM)\Temperature",
            is_high_precision: false,
        },
        Candidate {
            path: r"\Thermal Zone Information(\_TZ.TZ00)\High Precision Temperature",
            is_high_precision: true,
        },
        Candidate {
            path: r"\Thermal Zone Information(\_TZ.TZ00)\Temperature",
            is_high_precision: false,
        },
    ];

    let mut counter: isize = 0;
    let mut is_high_precision = false;
    let mut counter_added = false;

    for candidate in &candidates {
        let path_utf16: Vec<u16> = candidate
            .path
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();
        let add_status =
            unsafe { PdhAddEnglishCounterW(query, path_utf16.as_ptr(), 0, &mut counter) };
        if add_status == 0 {
            is_high_precision = candidate.is_high_precision;
            counter_added = true;
            break;
        }
    }

    if !counter_added {
        unsafe {
            PdhCloseQuery(query);
        }
        return None;
    }

    let collect_status = unsafe { PdhCollectQueryData(query) };
    if collect_status != 0 {
        unsafe {
            PdhCloseQuery(query);
        }
        return None;
    }

    let mut value: PDH_FMT_COUNTERVALUE = unsafe { std::mem::zeroed() };
    let fmt_status = unsafe {
        PdhGetFormattedCounterValue(counter, PDH_FMT_DOUBLE, std::ptr::null_mut(), &mut value)
    };

    unsafe {
        PdhCloseQuery(query);
    }

    if fmt_status != 0 {
        return None;
    }

    let raw = unsafe { value.Anonymous.doubleValue };
    if raw <= 0.0 {
        return None;
    }

    let kelvin = if is_high_precision {
        // High Precision Temperature reports deci-Kelvin (tenths of Kelvin)
        (raw / 10.0) as f32
    } else {
        raw as f32
    };

    // Convert Kelvin to Celsius: C = K - 273.15
    let celsius = kelvin - 273.15;
    if !(0.0..=120.0).contains(&celsius) {
        return None;
    }

    Some((celsius * 10.0).round() / 10.0)
}

#[cfg(not(windows))]
pub fn read_acpi_thermal_zone_temp() -> Option<f32> {
    None
}

/// Read CPU and GPU temperature sensors.
///
/// Returns ACPI Thermal Zone temperature for CPU when available via PDH.
/// Guaranteed absence behavior: when hardware or driver interfaces do not expose
/// thermal telemetry without kernel-mode ring-0 drivers, returns `(None, None)`
/// instead of synthesizing fake `0.0 °C` values (MON-01, ARCH-03).
pub fn read_temperature_sensors() -> (Option<f32>, Option<f32>) {
    let cpu = read_acpi_thermal_zone_temp();
    (cpu, None)
}

/// Read structured temperature sensor readings.
pub fn read_temperature_readings() -> Vec<TemperatureReading> {
    let cpu_temp = read_acpi_thermal_zone_temp();
    vec![
        TemperatureReading {
            label: "CPU Thermal Zone".to_string(),
            source: if cpu_temp.is_some() {
                TemperatureSource::AcpiThermalZone
            } else {
                TemperatureSource::Unavailable
            },
            celsius: cpu_temp,
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
