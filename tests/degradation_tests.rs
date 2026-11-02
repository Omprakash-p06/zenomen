//! Degradation matrix integration tests (PERF-05).
//!
//! Asserts that removing any sensor, interface, external tool, or daemon
//! causes zero crashes and cleanly degrades to an explained unavailable state.

use std::fs;
use std::path::PathBuf;
use zenomen::config::load_or_default;
use zenomen::hardware::linux::gpu::parse_nvidia_smi_output;
use zenomen::hardware::registry::SensorRegistry;
use zenomen::hardware::rgb::detect_rgb_interface;
use zenomen::service::backend::HardwareBackend;
use zenomen::service::client::HardwareServiceClient;

fn create_temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("zenomen_deg_{}_{}", name, std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn degradation_01_fan_hwmon_absent_leaves_cpu_temp_working() {
    let root = create_temp_dir("fan_absent");
    let hwmon_root = root.join("hwmon");
    let acpi_root = root.join("acpi");
    fs::create_dir_all(&hwmon_root).unwrap();
    fs::create_dir_all(&acpi_root).unwrap();

    // Create only CPU temp sensor (k10temp) - no fan device
    let k10_dir = hwmon_root.join("hwmon0");
    fs::create_dir_all(&k10_dir).unwrap();
    fs::write(k10_dir.join("name"), "k10temp\n").unwrap();
    fs::write(k10_dir.join("temp1_label"), "Tctl\n").unwrap();
    fs::write(k10_dir.join("temp1_input"), "55000\n").unwrap();

    let registry = SensorRegistry::scan_roots(&hwmon_root, &acpi_root);

    // CPU temp is available
    assert!(registry.cpu_temp.is_some());
    assert_eq!(registry.cpu_temp.as_ref().unwrap().driver, "k10temp");

    // Fans are cleanly unavailable (empty list, no control, no crash)
    assert!(registry.fan_tachometers.is_empty());
    assert!(!registry.has_fan_control);

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn degradation_02_pwm_enable_absent_keeps_fan_rpm_reading() {
    let root = create_temp_dir("pwm_absent");
    let hwmon_root = root.join("hwmon");
    let acpi_root = root.join("acpi");
    fs::create_dir_all(&hwmon_root).unwrap();
    fs::create_dir_all(&acpi_root).unwrap();

    // hp_wmi with fan1_input but NO pwm1_enable
    let hp_dir = hwmon_root.join("hwmon0");
    fs::create_dir_all(&hp_dir).unwrap();
    fs::write(hp_dir.join("name"), "hp\n").unwrap();
    fs::write(hp_dir.join("fan1_input"), "2400\n").unwrap();

    let registry = SensorRegistry::scan_roots(&hwmon_root, &acpi_root);

    // Fan tachometer is found and reads RPM
    assert_eq!(registry.fan_tachometers.len(), 1);
    // Control interface is unavailable
    assert!(!registry.has_fan_control);

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn degradation_03_no_cpu_temp_sensor_available() {
    let root = create_temp_dir("no_cpu_temp");
    let hwmon_root = root.join("hwmon");
    let acpi_root = root.join("acpi");
    fs::create_dir_all(&hwmon_root).unwrap();
    fs::create_dir_all(&acpi_root).unwrap();

    // Device with only fan, no cpu temperature sensors
    let fan_dir = hwmon_root.join("hwmon0");
    fs::create_dir_all(&fan_dir).unwrap();
    fs::write(fan_dir.join("name"), "hp\n").unwrap();
    fs::write(fan_dir.join("fan1_input"), "3000\n").unwrap();

    let registry = SensorRegistry::scan_roots(&hwmon_root, &acpi_root);

    // CPU temp is cleanly None (rendered as "Unavailable" in UI, never fabricated 0)
    assert!(registry.cpu_temp.is_none());
    assert_eq!(registry.fan_tachometers.len(), 1);

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn degradation_04_nvidia_smi_missing_degrades_dgpu_cleanly() {
    // When nvidia-smi is not found or produces empty output, parse_nvidia_smi_output returns None
    let empty_output = "";
    let sample = parse_nvidia_smi_output(empty_output);
    assert!(sample.is_none(), "Missing nvidia-smi must return None without panic");

    let truncated_output = "not,enough,parts\n";
    let sample_trunc = parse_nvidia_smi_output(truncated_output);
    assert!(sample_trunc.is_none(), "Truncated nvidia-smi must return None without panic");

    let malformed_numbers = "NVIDIA GPU,bad,bad,bad,bad\n";
    let sample_nan = parse_nvidia_smi_output(malformed_numbers);
    assert!(sample_nan.is_some(), "Header parsed without panic");
    assert!(sample_nan.unwrap().utilization.is_none(), "Non-numeric utilization degrades cleanly to None");
}

#[test]
fn degradation_05_rgb_zone_files_absent_reports_none_variant() {
    let root = create_temp_dir("rgb_absent");
    let variant = detect_rgb_interface(&root);
    assert_eq!(
        variant, None,
        "Absent RGB sysfs files must return None without panic"
    );

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn degradation_06_hardware_service_absent_degrades_gracefully() {
    // HardwareServiceClient without active system bus daemon reports disconnected status
    let client = HardwareServiceClient::new();
    let status = client.status();

    // Calling fan mode or power profile returns typed error without panic
    let fan_res = client.get_fan_status();
    let profile_res = client.get_power_profile();

    if !status.is_connected() {
        assert!(fan_res.is_err(), "Must return error when daemon is absent");
        assert!(profile_res.is_err(), "Must return error when daemon is absent");
        let err_str = fan_res.unwrap_err();
        assert!(!err_str.is_empty());
    }
}

#[test]
fn degradation_07_unauthorized_fan_write_preserves_hardware_state() {
    let root = create_temp_dir("unauthorized_write");
    let pwm_path = root.join("sys/devices/platform/hp-wmi/pwm1_enable");
    fs::create_dir_all(pwm_path.parent().unwrap()).unwrap();
    fs::write(&pwm_path, "2\n").unwrap(); // initial state: auto (2)

    let backend = HardwareBackend::new(&root);

    // Simulate an invalid mode
    let write_res = backend.set_fan_mode("invalid_mode");
    assert!(write_res.is_err());

    // Hardware file remains byte-identical
    let content = fs::read_to_string(&pwm_path).unwrap();
    assert_eq!(content, "2\n", "File content must not be corrupted on error");

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn degradation_08_corrupt_config_restores_defaults_and_preserves_backup() {
    let temp_dir = create_temp_dir("corrupt_config");
    let config_path = temp_dir.join("config.toml");

    // Write corrupted config
    fs::write(&config_path, "[[[not_valid_toml").unwrap();

    let (loaded, notice) = load_or_default(&config_path);

    // Default configuration must be restored
    assert_eq!(loaded, zenomen::config::AppConfig::default());
    assert!(notice.is_some(), "Notice must be provided explaining the corruption");

    // Backup file must exist
    let files: Vec<PathBuf> = fs::read_dir(&temp_dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .collect();

    assert!(
        files.iter().any(|p| p.to_string_lossy().contains(".bak.")),
        "Backup file must be preserved on disk"
    );

    let _ = fs::remove_dir_all(&temp_dir);
}
