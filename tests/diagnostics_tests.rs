use std::fs;
use std::path::{Path, PathBuf};

use zenomen::core::diagnostics::{
    DiagnosticsReport, collect_diagnostics, collect_diagnostics_from_roots,
    export_diagnostics_to_file,
};
use zenomen::core::state::AppState;
use zenomen::hardware::linux::hwmon::{FanSensorDescriptor, SensorDescriptor};
use zenomen::hardware::registry::SensorRegistry;
use zenomen::hardware::{HardwareCapabilities, TelemetrySnapshot};

struct TestDir {
    path: PathBuf,
}

impl TestDir {
    fn new(prefix: &str) -> Self {
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("zenomen_test_{prefix}_{timestamp}"));
        fs::create_dir_all(&path).unwrap();
        Self { path }
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TestDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

#[test]
fn collect_diagnostics_with_mock_roots() {
    let fixture = TestDir::new("diagnostics");

    let os_release_file = fixture.path().join("os-release");
    fs::write(
        &os_release_file,
        "NAME=\"Ubuntu\"\nVERSION=\"24.04\"\nPRETTY_NAME=\"Ubuntu 24.04 LTS\"\n",
    )
    .unwrap();

    let kernel_file = fixture.path().join("osrelease");
    fs::write(&kernel_file, "6.8.0-45-generic\n").unwrap();

    let dmi_dir = fixture.path().join("dmi");
    fs::create_dir_all(&dmi_dir).unwrap();
    fs::write(dmi_dir.join("product_name"), "OMEN Laptop 15-en1xxx\n").unwrap();
    fs::write(dmi_dir.join("sys_vendor"), "HP\n").unwrap();

    let drm_dir = fixture.path().join("drm");
    fs::create_dir_all(&drm_dir).unwrap();

    let state = AppState {
        capabilities: HardwareCapabilities {
            cpu_metrics: true,
            gpu_metrics: true,
            ram_metrics: true,
            fan_rpm: true,
            fan_control: true,
            power_profiles: true,
            rgb_lighting: false,
            rgb_variant: None,
            battery_metrics: true,
        },
        latest_snapshot: Some(TelemetrySnapshot {
            cpu_name: "AMD Ryzen 7 5800H".to_string(),
            cpu_utilization: 24.5,
            cpu_temperature_c: Some(58.0),
            cpu_frequency_mhz: 3200,
            gpu_name: Some("NVIDIA GeForce RTX 3060 Mobile".to_string()),
            gpu_utilization: Some(15.0),
            gpu_temperature_c: Some(52.0),
            ram_total_mb: 16384,
            ram_used_mb: 4096,
            fan_rpms: vec![2400, 2500],
            fan_mode: "auto".to_string(),
            power_profile: "balanced".to_string(),
            power_profiles: vec![
                "quiet".to_string(),
                "balanced".to_string(),
                "performance".to_string(),
            ],
            rgb_profile: None,
        }),
        last_sampling_error: Some("Transitory sensor read timeout".to_string()),
        ..AppState::default()
    };

    let registry = SensorRegistry {
        cpu_temp: Some(SensorDescriptor {
            driver: "k10temp".to_string(),
            label: "Tctl".to_string(),
            input_path: PathBuf::from("/sys/class/hwmon/hwmon1/temp1_input"),
        }),
        fan_tachometers: vec![
            FanSensorDescriptor {
                index: 1,
                label: "CPU Fan".to_string(),
                input_path: PathBuf::from("/sys/class/hwmon/hwmon2/fan1_input"),
            },
            FanSensorDescriptor {
                index: 2,
                label: "GPU Fan".to_string(),
                input_path: PathBuf::from("/sys/class/hwmon/hwmon2/fan2_input"),
            },
        ],
        has_fan_control: true,
        has_platform_profiles: true,
        discovered_devices: vec!["k10temp".to_string(), "hp".to_string()],
    };

    let report = collect_diagnostics_from_roots(
        &state,
        Some(&registry),
        &os_release_file,
        &kernel_file,
        &dmi_dir,
        &drm_dir,
    );

    assert_eq!(report.app_version, env!("CARGO_PKG_VERSION"));
    assert_eq!(report.os_name, "Ubuntu 24.04 LTS");
    assert_eq!(report.kernel_version, "6.8.0-45-generic");
    assert_eq!(
        report.product_name.as_deref(),
        Some("OMEN Laptop 15-en1xxx")
    );
    assert_eq!(report.sys_vendor.as_deref(), Some("HP"));
    assert!(report.capabilities.has_cpu_metrics());
    assert!(report.capabilities.has_fan_control());
    assert!(!report.capabilities.has_rgb_lighting());
    assert_eq!(
        report.cpu_sensor.as_deref(),
        Some("k10temp [Tctl] (/sys/class/hwmon/hwmon1/temp1_input)")
    );
    assert_eq!(report.fan_sensors.len(), 2);
    assert_eq!(
        report.fan_sensors[0],
        "fan1 [CPU Fan] (/sys/class/hwmon/hwmon2/fan1_input)"
    );
    assert_eq!(
        report.gpu_sensors,
        vec!["NVIDIA GeForce RTX 3060 Mobile".to_string()]
    );
    assert_eq!(
        report.recent_error.as_deref(),
        Some("Transitory sensor read timeout")
    );
    assert!(report.timestamp.contains('T') && report.timestamp.ends_with('Z'));

    // Export to file
    let export_path = fixture.path().join("export/diagnostics.json");
    let export_result = export_diagnostics_to_file(&report, &export_path);
    assert!(export_result.is_ok());
    assert!(export_path.exists());

    let content = fs::read_to_string(&export_path).unwrap();
    let parsed: DiagnosticsReport = serde_json::from_str(&content).unwrap();
    assert_eq!(parsed, report);
}

#[test]
fn collect_diagnostics_live_system_does_not_panic() {
    let state = AppState::default();
    let report = collect_diagnostics(&state, None);
    assert!(!report.app_version.is_empty());
    assert!(!report.os_name.is_empty());
    assert!(!report.kernel_version.is_empty());
}
