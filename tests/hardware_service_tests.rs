//! Integration tests for zenomen-hw service, verified sysfs writes, and polkit contract.

use std::fs;
use std::path::PathBuf;
use std::time::Duration;
use zenomen::service::backend::{HardwareBackend, write_and_verify};
use zenomen::service::types::{FanMode, HardwareControlError};

struct MockSysfs {
    root: PathBuf,
}

impl MockSysfs {
    fn new(test_name: &str) -> Self {
        let root = std::env::temp_dir().join(format!(
            "zenomen_test_hw_{test_name}_{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);

        // Build mock hp-wmi hwmon directory
        let hwmon_dir = root.join("sys/devices/platform/hp-wmi/hwmon/hwmon5");
        fs::create_dir_all(&hwmon_dir).expect("failed to create mock hwmon dir");

        fs::write(hwmon_dir.join("name"), "hp\n").unwrap();
        fs::write(hwmon_dir.join("pwm1_enable"), "2\n").unwrap();
        fs::write(hwmon_dir.join("fan1_input"), "2400\n").unwrap();
        fs::write(hwmon_dir.join("fan2_input"), "2600\n").unwrap();

        // Build mock ACPI platform_profile directory
        let acpi_dir = root.join("sys/firmware/acpi");
        fs::create_dir_all(&acpi_dir).expect("failed to create mock acpi dir");

        fs::write(
            acpi_dir.join("platform_profile_choices"),
            "cool quiet balanced performance\n",
        )
        .unwrap();
        fs::write(acpi_dir.join("platform_profile"), "balanced\n").unwrap();

        Self { root }
    }

    fn backend(&self) -> HardwareBackend {
        HardwareBackend::new(&self.root)
    }
}

impl Drop for MockSysfs {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

#[test]
fn test_fan_mode_parsing_and_conversion() {
    assert_eq!(FanMode::from_sysfs("0"), FanMode::Max);
    assert_eq!(FanMode::from_sysfs("1"), FanMode::Manual(1));
    assert_eq!(FanMode::from_sysfs("2"), FanMode::Auto);
    assert_eq!(FanMode::from_sysfs("2\n"), FanMode::Auto);

    assert_eq!(FanMode::Auto.as_sysfs_value(), Some("2"));
    assert_eq!(FanMode::Max.as_sysfs_value(), Some("0"));
    assert_eq!(FanMode::Manual(1).as_sysfs_value(), None);

    // parse_requested validation
    assert_eq!(FanMode::parse_requested("auto").unwrap(), FanMode::Auto);
    assert_eq!(FanMode::parse_requested("Auto ").unwrap(), FanMode::Auto);
    assert_eq!(FanMode::parse_requested("MAX").unwrap(), FanMode::Max);

    // Manual mode is unsupported in v1 without curves
    let manual_err = FanMode::parse_requested("manual").unwrap_err();
    assert!(matches!(
        manual_err,
        HardwareControlError::UnsupportedMode(_)
    ));

    // Arbitrary / invalid modes rejected
    let bad_err = FanMode::parse_requested("turbo").unwrap_err();
    assert!(matches!(bad_err, HardwareControlError::InvalidParameter(_)));

    let attack_err = FanMode::parse_requested("../bad").unwrap_err();
    assert!(matches!(
        attack_err,
        HardwareControlError::InvalidParameter(_)
    ));
}

#[test]
fn test_write_and_verify_success() {
    let temp_file = std::env::temp_dir().join(format!(
        "zenomen_test_verify_success_{}",
        std::process::id()
    ));
    fs::write(&temp_file, "initial").unwrap();

    let res = write_and_verify(&temp_file, "updated", 3, Duration::from_millis(5));
    assert!(res.is_ok());

    let content = fs::read_to_string(&temp_file).unwrap();
    assert_eq!(content.trim(), "updated");

    let _ = fs::remove_file(temp_file);
}

#[test]
fn test_write_and_verify_mismatch_failure() {
    let temp_file = std::env::temp_dir().join(format!(
        "zenomen_test_verify_mismatch_{}",
        std::process::id()
    ));
    fs::write(&temp_file, "sticky_value").unwrap();

    // Simulate sticky hardware register that reverts writes
    let target = temp_file.clone();
    let stop = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let stop_thread = stop.clone();
    let handle = std::thread::spawn(move || {
        while !stop_thread.load(std::sync::atomic::Ordering::Relaxed) {
            let _ = fs::write(&target, "sticky_value");
            std::thread::sleep(Duration::from_millis(1));
        }
    });

    let res = write_and_verify(&temp_file, "wanted_value", 3, Duration::from_millis(5));
    stop.store(true, std::sync::atomic::Ordering::Relaxed);
    let _ = handle.join();

    match res {
        Err(HardwareControlError::VerificationFailed {
            target,
            actual,
            retries,
        }) => {
            assert_eq!(target, "wanted_value");
            assert_ne!(actual, target);
            assert_eq!(retries, 3);
        }
        other => panic!("Expected VerificationFailed, got {other:?}"),
    }

    let _ = fs::remove_file(temp_file);
}

#[test]
fn test_write_and_verify_io_error() {
    let nonexistent = PathBuf::from("/nonexistent_dir_12345/sensor");
    let res = write_and_verify(&nonexistent, "1", 2, Duration::from_millis(5));
    assert!(matches!(res.unwrap_err(), HardwareControlError::IoError(_)));
}

#[test]
fn test_mock_backend_fan_control() {
    let mock = MockSysfs::new("fan_ctrl");
    let backend = mock.backend();

    // Verify initial state
    let initial_mode = backend.get_fan_mode().expect("should read fan mode");
    assert_eq!(initial_mode, FanMode::Auto);

    let rpms = backend.get_fan_rpms();
    assert_eq!(rpms, vec![2400, 2600]);

    // Set fan mode to max
    let res = backend.set_fan_mode("max").expect("should set max");
    assert_eq!(res, "max");
    assert_eq!(backend.get_fan_mode().unwrap(), FanMode::Max);

    // Set fan mode back to auto
    let res = backend.set_fan_mode("auto").expect("should set auto");
    assert_eq!(res, "auto");
    assert_eq!(backend.get_fan_mode().unwrap(), FanMode::Auto);

    // Manual mode rejected
    assert!(backend.set_fan_mode("manual").is_err());
}

#[test]
fn test_mock_backend_power_profiles() {
    let mock = MockSysfs::new("pwr_profiles");
    let backend = mock.backend();

    let choices = backend
        .get_power_profile_choices()
        .expect("choices should be readable");
    assert_eq!(choices, vec!["cool", "quiet", "balanced", "performance"]);

    let active = backend
        .get_power_profile()
        .expect("active profile readable");
    assert_eq!(active, "balanced");

    // Switch to performance
    let res = backend
        .set_power_profile("performance")
        .expect("should switch to performance");
    assert_eq!(res, "performance");
    assert_eq!(backend.get_power_profile().unwrap(), "performance");

    // Invalid profile not in choices is rejected
    let err = backend.set_power_profile("ultra-boost").unwrap_err();
    assert!(matches!(err, HardwareControlError::InvalidParameter(_)));

    // Profile containing directory traversal or newlines is rejected
    assert!(backend.set_power_profile("../quiet").is_err());
    assert!(backend.set_power_profile("performance\n").is_err());
}

#[test]
fn test_mock_backend_combined_presets_full_success() {
    let mock = MockSysfs::new("presets_ok");
    let backend = mock.backend();

    // Gaming preset: performance + max fan
    let result = backend.apply_preset("gaming").expect("apply preset");
    assert_eq!(result.preset_name, "gaming");
    assert!(result.power_applied);
    assert!(result.fan_applied);
    assert!(!result.partial_success);
    assert!(result.is_full_success());
    assert_eq!(backend.get_power_profile().unwrap(), "performance");
    assert_eq!(backend.get_fan_mode().unwrap(), FanMode::Max);

    // Balanced preset: balanced + auto fan
    let result = backend.apply_preset("balanced").expect("apply preset");
    assert_eq!(result.preset_name, "balanced");
    assert!(result.is_full_success());
    assert_eq!(backend.get_power_profile().unwrap(), "balanced");
    assert_eq!(backend.get_fan_mode().unwrap(), FanMode::Auto);

    // Quiet preset: quiet (or cool) + auto fan
    let result = backend.apply_preset("quiet").expect("apply preset");
    assert_eq!(result.preset_name, "quiet");
    assert!(result.is_full_success());
    assert_eq!(backend.get_power_profile().unwrap(), "quiet");
    assert_eq!(backend.get_fan_mode().unwrap(), FanMode::Auto);
}

#[test]
fn test_mock_backend_combined_preset_partial_success() {
    let mock = MockSysfs::new("presets_partial");
    let backend = mock.backend();

    // Remove fan pwm control to simulate machine with power profiles but unwritable fan control
    let pwm_path = mock
        .root
        .join("sys/devices/platform/hp-wmi/hwmon/hwmon5/pwm1_enable");
    fs::remove_file(pwm_path).unwrap();

    let result = backend.apply_preset("gaming").expect("apply preset");
    assert_eq!(result.preset_name, "gaming");
    assert!(result.power_applied);
    assert!(!result.fan_applied);
    assert!(result.partial_success);
    assert!(!result.is_full_success());
    assert!(result.message.contains("partially applied"));
}

#[test]
fn test_configuration_and_policy_assets_exist() {
    let dbus_conf = fs::read_to_string("data/dbus/com.zenomen.Hardware1.conf").expect("dbus conf");
    assert!(dbus_conf.contains("com.zenomen.Hardware1"));
    assert!(dbus_conf.contains("<policy user=\"root\">"));

    let polkit_policy =
        fs::read_to_string("data/polkit/com.zenomen.hardware.policy").expect("polkit policy");
    assert!(polkit_policy.contains("com.zenomen.hardware.control"));
    assert!(polkit_policy.contains("auth_admin_keep"));

    let systemd_service =
        fs::read_to_string("data/systemd/zenomen-hw.service").expect("systemd unit");
    assert!(systemd_service.contains("Type=dbus"));
    assert!(systemd_service.contains("BusName=com.zenomen.Hardware1"));
    assert!(systemd_service.contains("ProtectSystem=strict"));
    assert!(
        systemd_service
            .contains("ReadWritePaths=-/sys/devices/platform/hp-wmi -/sys/firmware/acpi")
    );
    assert!(systemd_service.contains("NoNewPrivileges=true"));
}
