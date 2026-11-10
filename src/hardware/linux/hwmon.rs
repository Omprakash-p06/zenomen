//! Linux hwmon sysfs scanner and sensor reader.
//!
//! Enumerate `/sys/class/hwmon/hwmon*` devices, inspect driver names, and read
//! temperature and fan tachometer sensors (MON-04, MON-12).

use std::path::{Path, PathBuf};

/// A discovered hwmon device node under `/sys/class/hwmon`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HwmonDevice {
    pub id: String,
    pub name: String,
    pub path: PathBuf,
}

/// A canonical sensor binding with driver attribution and sysfs path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SensorDescriptor {
    pub driver: String,
    pub label: String,
    pub input_path: PathBuf,
}

/// A fan tachometer binding with index and input path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FanSensorDescriptor {
    pub index: usize,
    pub label: String,
    pub input_path: PathBuf,
}

/// Scan `/sys/class/hwmon` for all registered hwmon devices.
pub fn scan_hwmon_devices(hwmon_root: &Path) -> Vec<HwmonDevice> {
    let mut devices = Vec::new();

    let Ok(entries) = std::fs::read_dir(hwmon_root) else {
        return devices;
    };

    for entry in entries.flatten() {
        let file_name = entry.file_name();
        let id = file_name.to_string_lossy().to_string();

        if !id.starts_with("hwmon") {
            continue;
        }

        let path = entry.path();
        let name = std::fs::read_to_string(path.join("name"))
            .map(|s| s.trim().to_string())
            .unwrap_or_default();

        devices.push(HwmonDevice { id, name, path });
    }

    devices.sort_by_key(|d| {
        d.id.strip_prefix("hwmon")
            .and_then(|num| num.parse::<u32>().ok())
            .unwrap_or(u32::MAX)
    });
    devices
}

/// Read temperature in degrees Celsius from a sysfs `temp*_input` file.
///
/// Converts millidegrees to degrees Celsius (`f32 = mdeg / 1000.0`).
pub fn read_temp_millidegrees(path: &Path) -> Option<f32> {
    let content = std::fs::read_to_string(path).ok()?;
    let mdeg: f32 = content.trim().parse().ok()?;
    Some(mdeg / 1000.0)
}

/// Read fan tachometer speed in RPM from a sysfs `fan*_input` file.
pub fn read_fan_rpm(path: &Path) -> Option<u32> {
    let content = std::fs::read_to_string(path).ok()?;
    content.trim().parse().ok()
}

/// Read associated sensor label file if present (e.g. `temp1_label`).
pub fn read_sensor_label(device_path: &Path, sensor_prefix: &str) -> Option<String> {
    let label_file = device_path.join(format!("{sensor_prefix}_label"));
    std::fs::read_to_string(label_file)
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    struct TestDir(PathBuf);
    impl TestDir {
        fn new(prefix: &str) -> Self {
            let nanos = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let path = std::env::temp_dir().join(format!("{prefix}-{nanos}"));
            fs::create_dir_all(&path).expect("create test dir");
            Self(path)
        }
        fn path(&self) -> &Path {
            &self.0
        }
    }
    impl Drop for TestDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn read_temp_converts_millidegrees_to_celsius() {
        let dir = TestDir::new("zenomen-hwmon-test");
        let temp_file = dir.path().join("temp1_input");
        fs::write(&temp_file, "68500\n").unwrap();

        let temp = read_temp_millidegrees(&temp_file).expect("temp parsed");
        assert!((temp - 68.5).abs() < 0.01);
    }

    #[test]
    fn read_fan_rpm_parses_integer_rpm() {
        let dir = TestDir::new("zenomen-hwmon-test");
        let fan_file = dir.path().join("fan1_input");
        fs::write(&fan_file, "3250\n").unwrap();

        let rpm = read_fan_rpm(&fan_file).expect("rpm parsed");
        assert_eq!(rpm, 3250);
    }

    #[test]
    fn read_sensor_label_returns_trimmed_content() {
        let dir = TestDir::new("zenomen-hwmon-test");
        let label_file = dir.path().join("temp1_label");
        fs::write(&label_file, "Tctl\n").unwrap();

        let label = read_sensor_label(dir.path(), "temp1").expect("label parsed");
        assert_eq!(label, "Tctl");
    }
}
