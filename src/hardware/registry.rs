//! Canonical hardware sensor registry.
//!
//! Enforces a single shared registry for both dashboard telemetry and
//! diagnostics (MON-12), resolving CPU temperature via strict driver and label
//! priority (MON-04) and enumerating fan tachometers.

use std::path::Path;

use crate::hardware::HardwareCapabilities;
use crate::hardware::linux::hwmon::{
    FanSensorDescriptor, SensorDescriptor, read_fan_rpm, read_sensor_label, read_temp_millidegrees,
    scan_hwmon_devices,
};

/// Single source of truth for canonical sensor bindings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SensorRegistry {
    /// Canonical CPU temperature sensor binding, selected by priority.
    pub cpu_temp: Option<SensorDescriptor>,
    /// Discovered fan tachometers.
    pub fan_tachometers: Vec<FanSensorDescriptor>,
    /// Writable fan mode interface (`pwm1_enable`) exists.
    pub has_fan_control: bool,
    /// ACPI `platform_profile` is exposed.
    pub has_platform_profiles: bool,
    /// All discovered hwmon device names.
    pub discovered_devices: Vec<String>,
}

impl SensorRegistry {
    /// Scan system hardware sensor hierarchy under standard sysfs paths.
    pub fn scan() -> Self {
        Self::scan_roots(
            Path::new("/sys/class/hwmon"),
            Path::new("/sys/firmware/acpi"),
        )
    }

    /// Scan custom directory roots (enables testing and mocking).
    pub fn scan_roots(hwmon_root: &Path, acpi_root: &Path) -> Self {
        let devices = scan_hwmon_devices(hwmon_root);
        let discovered_devices = devices.iter().map(|d| d.name.clone()).collect();

        // 1. Resolve CPU temperature according to strict priority matrix (MON-04)
        // Level 1: k10temp (AMD Zen) -> Tctl / Tdie / temp1_input
        // Level 2: coretemp (Intel) -> Package id 0 / temp1_input
        // Level 3: acpitz (ACPI thermal zone) -> temp1_input
        let cpu_temp = resolve_cpu_temp(&devices);

        // 2. Discover Fan Tachometers
        let (fan_tachometers, has_fan_control) = resolve_fans(&devices);

        // 3. Check ACPI platform profiles
        let has_platform_profiles = acpi_root.join("platform_profile").exists();

        Self {
            cpu_temp,
            fan_tachometers,
            has_fan_control,
            has_platform_profiles,
            discovered_devices,
        }
    }

    /// Sample CPU temperature from the registered canonical sensor path.
    pub fn read_cpu_temp(&self) -> Option<f32> {
        let sensor = self.cpu_temp.as_ref()?;
        read_temp_millidegrees(&sensor.input_path)
    }

    /// Sample all registered fan tachometers in order.
    pub fn read_fan_rpms(&self) -> Vec<u32> {
        self.fan_tachometers
            .iter()
            .filter_map(|fan| read_fan_rpm(&fan.input_path))
            .collect()
    }

    /// Read active ACPI platform profile if available.
    pub fn read_active_power_profile(&self, acpi_root: &Path) -> Option<String> {
        let content = std::fs::read_to_string(acpi_root.join("platform_profile")).ok()?;
        let trimmed = content.trim();
        (!trimmed.is_empty()).then(|| trimmed.to_string())
    }

    /// Read advertised ACPI platform profile choices if available.
    pub fn read_power_profile_choices(&self, acpi_root: &Path) -> Vec<String> {
        let Ok(content) = std::fs::read_to_string(acpi_root.join("platform_profile_choices"))
        else {
            return Vec::new();
        };
        content.split_whitespace().map(|s| s.to_string()).collect()
    }

    /// Determine overall hardware capabilities based on detected sensors.
    pub fn detect_capabilities(
        &self,
        has_cpu_metrics: bool,
        has_gpu_metrics: bool,
        has_ram_metrics: bool,
    ) -> HardwareCapabilities {
        HardwareCapabilities {
            cpu_metrics: has_cpu_metrics,
            gpu_metrics: has_gpu_metrics,
            ram_metrics: has_ram_metrics,
            fan_rpm: !self.fan_tachometers.is_empty(),
            fan_control: self.has_fan_control,
            power_profiles: self.has_platform_profiles,
            rgb_lighting: false,
            battery_metrics: false,
        }
    }
}

/// Resolve canonical CPU temperature sensor based on driver and label priority (MON-04).
fn resolve_cpu_temp(
    devices: &[crate::hardware::linux::hwmon::HwmonDevice],
) -> Option<SensorDescriptor> {
    // Priority 1: k10temp (AMD)
    if let Some(sensor) = devices
        .iter()
        .find(|d| d.name == "k10temp")
        .and_then(|dev| find_temperature_input(dev, &["Tctl", "Tdie"]))
    {
        return Some(sensor);
    }

    // Priority 2: coretemp (Intel)
    if let Some(sensor) = devices
        .iter()
        .find(|d| d.name == "coretemp")
        .and_then(|dev| find_temperature_input(dev, &["Package id 0"]))
    {
        return Some(sensor);
    }

    // Priority 3: acpitz (ACPI Thermal Zone)
    if let Some(dev) = devices.iter().find(|d| d.name == "acpitz") {
        let temp1 = dev.path.join("temp1_input");
        if temp1.exists() {
            return Some(SensorDescriptor {
                driver: dev.name.clone(),
                label: "ACPI Thermal Zone".to_string(),
                input_path: temp1,
            });
        }
    }

    None
}

/// Helper to search a hwmon device for `temp*_input` matching preferred labels.
fn find_temperature_input(
    device: &crate::hardware::linux::hwmon::HwmonDevice,
    preferred_labels: &[&str],
) -> Option<SensorDescriptor> {
    let Ok(entries) = std::fs::read_dir(&device.path) else {
        return None;
    };

    let mut candidate_inputs = Vec::new();

    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if let Some(prefix) = name
            .strip_suffix("_input")
            .filter(|p| p.starts_with("temp"))
        {
            let label =
                read_sensor_label(&device.path, prefix).unwrap_or_else(|| prefix.to_string());
            let path = entry.path();

            // Check for preferred label match
            if preferred_labels
                .iter()
                .any(|pl| label.eq_ignore_ascii_case(pl))
            {
                return Some(SensorDescriptor {
                    driver: device.name.clone(),
                    label,
                    input_path: path,
                });
            }

            candidate_inputs.push((prefix.to_string(), label, path));
        }
    }

    // Fall back to temp1_input if present
    candidate_inputs.sort_by(|a, b| a.0.cmp(&b.0));
    candidate_inputs
        .into_iter()
        .next()
        .map(|(_, label, input_path)| SensorDescriptor {
            driver: device.name.clone(),
            label,
            input_path,
        })
}

/// Discover fans and PWM control on HP / platform devices.
fn resolve_fans(
    devices: &[crate::hardware::linux::hwmon::HwmonDevice],
) -> (Vec<FanSensorDescriptor>, bool) {
    let mut tachometers = Vec::new();
    let mut has_pwm = false;

    // Prioritize hp / hp-wmi platform devices
    let target_devices: Vec<_> = devices
        .iter()
        .filter(|d| d.name == "hp" || d.name == "hp-wmi" || d.name.contains("platform"))
        .collect();

    let search_pool = if !target_devices.is_empty() {
        target_devices
    } else {
        devices.iter().collect()
    };

    for dev in search_pool {
        if dev.path.join("pwm1_enable").exists() {
            has_pwm = true;
        }

        let Ok(entries) = std::fs::read_dir(&dev.path) else {
            continue;
        };

        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            let parsed_fan = name
                .strip_suffix("_input")
                .and_then(|prefix| prefix.strip_prefix("fan"))
                .and_then(|idx_str| idx_str.parse::<usize>().ok());

            if let Some(index) = parsed_fan {
                let label = format!("Fan {index}");
                tachometers.push(FanSensorDescriptor {
                    index,
                    label,
                    input_path: entry.path(),
                });
            }
        }
    }

    tachometers.sort_by_key(|f| f.index);
    (tachometers, has_pwm)
}
