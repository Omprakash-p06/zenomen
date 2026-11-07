//! Integration tests verifying single shared SensorRegistry and priority resolution.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use zenomen::hardware::HardwareProvider;
use zenomen::hardware::linux::provider::LinuxHardwareProvider;
use zenomen::hardware::registry::SensorRegistry;

struct TestHwmonTree(PathBuf);

impl TestHwmonTree {
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

    fn create_hwmon(
        &self,
        id: &str,
        name: &str,
        temp_inputs: &[(&str, Option<&str>, &str)],
        fan_inputs: &[(&str, &str)],
        has_pwm: bool,
    ) {
        let dev_dir = self.0.join(id);
        fs::create_dir_all(&dev_dir).expect("create dev dir");
        fs::write(dev_dir.join("name"), format!("{name}\n")).expect("write name");

        for (input_name, label, value) in temp_inputs {
            fs::write(
                dev_dir.join(format!("{input_name}_input")),
                format!("{value}\n"),
            )
            .expect("write temp input");
            if let Some(lbl) = label {
                fs::write(
                    dev_dir.join(format!("{input_name}_label")),
                    format!("{lbl}\n"),
                )
                .expect("write temp label");
            }
        }

        for (fan_name, value) in fan_inputs {
            fs::write(
                dev_dir.join(format!("{fan_name}_input")),
                format!("{value}\n"),
            )
            .expect("write fan input");
        }

        if has_pwm {
            fs::write(dev_dir.join("pwm1_enable"), "2\n").expect("write pwm");
        }
    }
}

impl Drop for TestHwmonTree {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn priority_matrix_selects_k10temp_over_acpitz() {
    let tree = TestHwmonTree::new("zenomen-k10-priority");
    let empty_acpi = tree.path().join("acpi");
    fs::create_dir_all(&empty_acpi).unwrap();

    // hwmon0 is acpitz (lower priority)
    tree.create_hwmon("hwmon0", "acpitz", &[("temp1", None, "55000")], &[], false);

    // hwmon1 is k10temp (highest priority for AMD)
    tree.create_hwmon(
        "hwmon1",
        "k10temp",
        &[("temp1", Some("Tctl"), "68000")],
        &[],
        false,
    );

    let registry = SensorRegistry::scan_roots(tree.path(), &empty_acpi);
    let cpu_sensor = registry
        .cpu_temp
        .as_ref()
        .expect("cpu sensor must be resolved");

    assert_eq!(cpu_sensor.driver, "k10temp");
    assert_eq!(cpu_sensor.label, "Tctl");

    let temp = registry.read_cpu_temp().expect("temp read");
    assert!((temp - 68.0).abs() < 0.01);
}

#[test]
fn priority_matrix_selects_coretemp_over_acpitz() {
    let tree = TestHwmonTree::new("zenomen-coretemp-priority");
    let empty_acpi = tree.path().join("acpi");
    fs::create_dir_all(&empty_acpi).unwrap();

    // hwmon0 is acpitz
    tree.create_hwmon("hwmon0", "acpitz", &[("temp1", None, "50000")], &[], false);

    // hwmon1 is coretemp with Package id 0
    tree.create_hwmon(
        "hwmon1",
        "coretemp",
        &[("temp1", Some("Package id 0"), "72000")],
        &[],
        false,
    );

    let registry = SensorRegistry::scan_roots(tree.path(), &empty_acpi);
    let cpu_sensor = registry
        .cpu_temp
        .as_ref()
        .expect("cpu sensor must be resolved");

    assert_eq!(cpu_sensor.driver, "coretemp");
    assert_eq!(cpu_sensor.label, "Package id 0");

    let temp = registry.read_cpu_temp().expect("temp read");
    assert!((temp - 72.0).abs() < 0.01);
}

#[test]
fn priority_matrix_falls_back_to_acpitz_when_cpu_driver_missing() {
    let tree = TestHwmonTree::new("zenomen-acpitz-fallback");
    let empty_acpi = tree.path().join("acpi");
    fs::create_dir_all(&empty_acpi).unwrap();

    tree.create_hwmon("hwmon0", "acpitz", &[("temp1", None, "51000")], &[], false);

    let registry = SensorRegistry::scan_roots(tree.path(), &empty_acpi);
    let cpu_sensor = registry
        .cpu_temp
        .as_ref()
        .expect("cpu sensor must be resolved");

    assert_eq!(cpu_sensor.driver, "acpitz");
    let temp = registry.read_cpu_temp().expect("temp read");
    assert!((temp - 51.0).abs() < 0.01);
}

#[test]
fn missing_sensors_return_none_without_synthesizing_zero() {
    let tree = TestHwmonTree::new("zenomen-missing-sensors");
    let empty_acpi = tree.path().join("acpi");
    fs::create_dir_all(&empty_acpi).unwrap();

    // Empty hwmon tree
    let registry = SensorRegistry::scan_roots(tree.path(), &empty_acpi);
    assert!(registry.cpu_temp.is_none());
    assert!(registry.read_cpu_temp().is_none());
    assert!(registry.fan_tachometers.is_empty());
    assert!(registry.read_fan_rpms().is_empty());
}

#[test]
fn discovers_fans_and_pwm_control_on_hp_wmi() {
    let tree = TestHwmonTree::new("zenomen-hp-fans");
    let empty_acpi = tree.path().join("acpi");
    fs::create_dir_all(&empty_acpi).unwrap();

    tree.create_hwmon(
        "hwmon0",
        "hp",
        &[],
        &[("fan1", "2400"), ("fan2", "2300")],
        true,
    );

    let registry = SensorRegistry::scan_roots(tree.path(), &empty_acpi);
    assert_eq!(registry.fan_tachometers.len(), 2);
    assert!(registry.has_fan_control);

    let rpms = registry.read_fan_rpms();
    assert_eq!(rpms, vec![2400, 2300]);
    assert_eq!(registry.read_fan_mode(), Some("auto".to_string()));
}

#[test]
fn battery_presence_is_measured_not_hardcoded() {
    let tree = TestHwmonTree::new("zenomen-battery-present");
    // Nested layout so the derived `power_supply` sibling stays inside the
    // fixture instead of touching the shared temp directory.
    let hwmon_root = tree.path().join("hwmon");
    let acpi_root = tree.path().join("acpi");
    fs::create_dir_all(&hwmon_root).unwrap();
    fs::create_dir_all(&acpi_root).unwrap();
    fs::create_dir_all(tree.path().join("power_supply/BAT0")).unwrap();
    fs::write(tree.path().join("power_supply/BAT0/capacity"), "87\n").unwrap();

    let registry = SensorRegistry::scan_roots(&hwmon_root, &acpi_root);

    assert!(registry.has_battery);
    assert!(
        registry
            .detect_capabilities(true, true, true)
            .has_battery_metrics()
    );
}

#[test]
fn battery_capability_is_false_without_a_battery_device() {
    let tree = TestHwmonTree::new("zenomen-battery-absent");
    let hwmon_root = tree.path().join("hwmon");
    let acpi_root = tree.path().join("acpi");
    fs::create_dir_all(&hwmon_root).unwrap();
    fs::create_dir_all(&acpi_root).unwrap();

    let registry = SensorRegistry::scan_roots(&hwmon_root, &acpi_root);

    assert!(!registry.has_battery);
    assert!(
        !registry
            .detect_capabilities(true, true, true)
            .has_battery_metrics()
    );
}

#[test]
fn a_fully_capable_machine_is_not_degraded() {
    use zenomen::core::events::AppEvent;
    use zenomen::core::state::AppState;

    let mut state = AppState::default();
    state.apply(&AppEvent::CapabilitiesLoaded(
        zenomen::hardware::HardwareCapabilities::all_supported(),
    ));

    assert!(!state.is_degraded());
}

#[test]
fn live_host_initialization_resolves_real_sensors() {
    let provider = LinuxHardwareProvider::new();
    let capabilities = provider.capabilities();

    // On this host (AMD Ryzen laptop), CPU metrics must be reported
    assert!(capabilities.has_cpu_metrics());
    assert!(capabilities.has_ram_metrics());

    let snapshot = provider.sample_telemetry().expect("sampling succeeds");
    assert!(!snapshot.cpu_name.is_empty());

    // Dashboard and registry agree on CPU temperature sensor
    if let Some(sensor) = &provider.registry().cpu_temp {
        assert_eq!(sensor.driver, "k10temp");
        assert!(snapshot.cpu_temperature_c.is_some());
    }
}
