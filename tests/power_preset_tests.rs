//! Integration tests for ACPI power profile selection, combined presets, and hardware service diagnostics.

use std::fs;
use std::path::PathBuf;
use zenomen::core::collect_diagnostics_from_roots;
use zenomen::core::events::AppEvent;
use zenomen::core::state::AppState;
use zenomen::service::backend::HardwareBackend;
use zenomen::service::types::{FanMode, HardwareControlError, PresetResult};

struct MockSysfs {
    root: PathBuf,
}

impl MockSysfs {
    fn new(test_name: &str) -> Self {
        let root = std::env::temp_dir().join(format!(
            "zenomen_test_pwr_{test_name}_{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);

        // Build mock hp-wmi hwmon directory
        let hwmon_dir = root.join("sys/devices/platform/hp-wmi/hwmon/hwmon5");
        fs::create_dir_all(&hwmon_dir).expect("failed to create mock hwmon dir");

        fs::write(hwmon_dir.join("name"), "hp\n").unwrap();
        fs::write(hwmon_dir.join("pwm1_enable"), "2\n").unwrap();
        fs::write(hwmon_dir.join("fan1_input"), "2500\n").unwrap();

        // Build mock ACPI platform_profile directory
        let acpi_dir = root.join("sys/firmware/acpi");
        fs::create_dir_all(&acpi_dir).expect("failed to create mock acpi dir");

        fs::write(
            acpi_dir.join("platform_profile_choices"),
            "cool quiet balanced performance\n",
        )
        .unwrap();
        fs::write(acpi_dir.join("platform_profile"), "balanced\n").unwrap();

        // Minimal DMI / DRM mock roots for diagnostics
        let dmi_dir = root.join("sys/class/dmi/id");
        fs::create_dir_all(&dmi_dir).unwrap();
        fs::write(dmi_dir.join("product_name"), "OMEN 16\n").unwrap();
        fs::write(dmi_dir.join("sys_vendor"), "HP\n").unwrap();

        let drm_dir = root.join("sys/class/drm");
        fs::create_dir_all(&drm_dir).unwrap();

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
fn test_power_profile_dynamic_enumeration_and_switch() {
    let mock = MockSysfs::new("enum_switch");
    let backend = mock.backend();

    // 1. Choices are read dynamically (PWR-01)
    let choices = backend
        .get_power_profile_choices()
        .expect("choices must be readable");
    assert_eq!(choices, vec!["cool", "quiet", "balanced", "performance"]);

    // 2. Active profile read (PWR-02)
    assert_eq!(backend.get_power_profile().unwrap(), "balanced");

    // 3. Switch to performance with verification (PWR-03, PWR-04)
    let switched = backend
        .set_power_profile("performance")
        .expect("switch must succeed");
    assert_eq!(switched, "performance");
    assert_eq!(backend.get_power_profile().unwrap(), "performance");

    // 4. Case-insensitive choice handling
    let switched = backend
        .set_power_profile("COOL")
        .expect("case-insensitive switch");
    assert_eq!(switched, "cool");
    assert_eq!(backend.get_power_profile().unwrap(), "cool");

    // 5. Unadvertised profile rejected with InvalidParameter
    let err = backend.set_power_profile("overclock").unwrap_err();
    assert!(matches!(err, HardwareControlError::InvalidParameter(_)));

    // 6. Security rejection: traversal and command injection characters
    assert!(backend.set_power_profile("../quiet").is_err());
    assert!(backend.set_power_profile("balanced; rm -rf /").is_err());
    assert!(backend.set_power_profile("quiet\0bad").is_err());
}

#[test]
fn test_combined_presets_full_workflow() {
    let mock = MockSysfs::new("presets_flow");
    let backend = mock.backend();

    // Gaming: performance + max fan
    let res = backend.apply_preset("gaming").expect("gaming preset");
    assert_eq!(res.preset_name, "gaming");
    assert!(res.power_applied);
    assert!(res.fan_applied);
    assert!(!res.partial_success);
    assert!(res.is_full_success());
    assert_eq!(backend.get_power_profile().unwrap(), "performance");
    assert_eq!(backend.get_fan_mode().unwrap(), FanMode::Max);

    // Balanced: balanced + auto fan
    let res = backend.apply_preset("balanced").expect("balanced preset");
    assert_eq!(res.preset_name, "balanced");
    assert!(res.is_full_success());
    assert_eq!(backend.get_power_profile().unwrap(), "balanced");
    assert_eq!(backend.get_fan_mode().unwrap(), FanMode::Auto);

    // Quiet: quiet + auto fan
    let res = backend.apply_preset("quiet").expect("quiet preset");
    assert_eq!(res.preset_name, "quiet");
    assert!(res.is_full_success());
    assert_eq!(backend.get_power_profile().unwrap(), "quiet");
    assert_eq!(backend.get_fan_mode().unwrap(), FanMode::Auto);
}

#[test]
fn test_preset_result_partial_success_honest_reporting() {
    let mock = MockSysfs::new("partial_reporting");
    let backend = mock.backend();

    // Remove fan control file to simulate hardware failure on fan write
    let pwm_path = mock
        .root
        .join("sys/devices/platform/hp-wmi/hwmon/hwmon5/pwm1_enable");
    fs::remove_file(pwm_path).unwrap();

    let res = backend.apply_preset("gaming").expect("apply preset");
    assert_eq!(res.preset_name, "gaming");
    assert!(res.power_applied);
    assert!(!res.fan_applied);
    assert!(res.partial_success);
    assert!(!res.is_full_success());
    assert!(res.message.contains("partially applied"));
    assert!(res.message.contains("Fan mode 'max' failed"));
}

#[test]
fn test_state_handles_power_and_preset_events() {
    let mut state = AppState::default();

    // Power profile changed
    state.apply(&AppEvent::PowerProfileChanged(
        Ok("performance".to_string()),
    ));
    assert_eq!(state.active_power_profile, "performance");
    assert_eq!(state.last_control_error, None);

    // Preset applied with full success
    let full_preset = PresetResult {
        preset_name: "gaming".to_string(),
        power_applied: true,
        confirmed_power_profile: Some("performance".to_string()),
        fan_applied: true,
        confirmed_fan_mode: Some("max".to_string()),
        partial_success: false,
        message: "Preset 'gaming' successfully applied".to_string(),
    };
    state.apply(&AppEvent::PresetApplied(Ok(full_preset.clone())));
    assert_eq!(state.fan_mode, "max");
    assert_eq!(state.active_power_profile, "performance");
    assert_eq!(state.last_preset_result, Some(full_preset));
    assert_eq!(state.last_control_error, None);

    // Preset applied with partial success
    let partial_preset = PresetResult {
        preset_name: "balanced".to_string(),
        power_applied: true,
        confirmed_power_profile: Some("balanced".to_string()),
        fan_applied: false,
        confirmed_fan_mode: None,
        partial_success: true,
        message: "Preset 'balanced' partially applied".to_string(),
    };
    state.apply(&AppEvent::PresetApplied(Ok(partial_preset.clone())));
    assert_eq!(state.last_preset_result, Some(partial_preset));
    assert_eq!(
        state.last_control_error,
        Some("Preset 'balanced' partially applied".to_string())
    );
}

#[test]
fn test_preset_without_any_interface_fails_instead_of_reporting_success() {
    // A sysfs root with neither `pwm1_enable` nor `platform_profile_choices`.
    let root = std::env::temp_dir().join(format!("zenomen_test_pwr_nohw_{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).unwrap();

    let backend = HardwareBackend::new(&root);
    let result = backend.apply_preset("gaming");
    let _ = fs::remove_dir_all(&root);

    match result {
        Err(HardwareControlError::VerificationFailed {
            target,
            actual,
            retries,
        }) => {
            assert_eq!(target, "gaming");
            assert_eq!(retries, 0);
            // Both legs must be named, so the operator can see what is missing.
            assert!(actual.contains("Power profile"), "actual: {actual}");
            assert!(actual.contains("Fan mode"), "actual: {actual}");
        }
        other => panic!("expected VerificationFailed, got {other:?}"),
    }
}

#[test]
fn test_diagnostics_report_includes_hardware_service() {
    let mock = MockSysfs::new("diag_service");
    let state = AppState::default();

    let os_rel = mock.root.join("etc/os-release");
    fs::create_dir_all(os_rel.parent().unwrap()).unwrap();
    fs::write(&os_rel, "PRETTY_NAME=\"Ubuntu 24.04\"\n").unwrap();

    let kern_rel = mock.root.join("proc/sys/kernel/osrelease");
    fs::create_dir_all(kern_rel.parent().unwrap()).unwrap();
    fs::write(&kern_rel, "6.8.0-generic\n").unwrap();

    let dmi_dir = mock.root.join("sys/class/dmi/id");
    let drm_dir = mock.root.join("sys/class/drm");

    let report =
        collect_diagnostics_from_roots(&state, None, &os_rel, &kern_rel, &dmi_dir, &drm_dir);

    // DIAG-04: Diagnostics report includes hardware service information
    assert!(report.hardware_service.is_some());
    let svc = report.hardware_service.as_ref().unwrap();
    assert_eq!(svc.service_name, "com.zenomen.Hardware1");
    assert_eq!(svc.status, "Disconnected");
    assert!(svc.polkit_authorization.contains("disconnected"));

    // Serialize to JSON and verify structure
    let json = serde_json::to_string_pretty(&report).unwrap();
    assert!(json.contains("com.zenomen.Hardware1"));
    assert!(json.contains("hardware_service"));
}
