//! Tests for mock hardware provider scenarios (`Nominal`, `ZeroCapabilities`, `SensorDataGap`).

use zenomen::hardware::{HardwareProvider, MockHardware, MockScenario};

#[test]
fn nominal_provider_emits_valid_values_across_sensors() {
    let mock = MockHardware::new(MockScenario::Nominal);
    let caps = mock.capabilities();

    assert!(caps.has_cpu_metrics(), "Nominal must report CPU metrics");
    assert!(caps.has_gpu_metrics(), "Nominal must report GPU metrics");
    assert!(caps.has_ram_metrics(), "Nominal must report RAM metrics");
    assert!(caps.has_fan_rpm(), "Nominal must report fan RPM");
    assert!(caps.has_fan_control(), "Nominal must report fan control");
    assert!(
        caps.has_power_profiles(),
        "Nominal must report power profiles"
    );
    assert!(caps.has_rgb_lighting(), "Nominal must report RGB lighting");
    assert!(
        caps.has_battery_metrics(),
        "Nominal must report battery metrics"
    );

    // Verify telemetry samples
    let sample = mock
        .sample_telemetry()
        .expect("Nominal sample must succeed");
    assert!(!sample.cpu_name.is_empty(), "CPU name must be populated");
    assert!(sample.cpu_utilization > 0.0, "CPU utilization must be > 0");
    assert!(sample.gpu_name.is_some(), "GPU name must be present");
    assert!(
        sample.gpu_utilization.is_some(),
        "GPU utilization must be present"
    );
    assert!(sample.ram_total_mb > 0, "RAM total must be non-zero");
    assert!(sample.ram_used_mb > 0, "RAM used must be non-zero");
    assert!(!sample.fan_rpms.is_empty(), "Fan RPMs must not be empty");
    assert!(
        !sample.power_profiles.is_empty(),
        "Power profiles must not be empty"
    );
}

#[test]
fn zero_capabilities_provider_emits_all_false_flags() {
    let mock = MockHardware::new(MockScenario::ZeroCapabilities);
    let caps = mock.capabilities();

    assert!(
        !caps.has_cpu_metrics(),
        "ZeroCapabilities must report no CPU metrics"
    );
    assert!(
        !caps.has_gpu_metrics(),
        "ZeroCapabilities must report no GPU metrics"
    );
    assert!(
        !caps.has_ram_metrics(),
        "ZeroCapabilities must report no RAM metrics"
    );
    assert!(
        !caps.has_fan_rpm(),
        "ZeroCapabilities must report no fan RPM"
    );
    assert!(
        !caps.has_fan_control(),
        "ZeroCapabilities must report no fan control"
    );
    assert!(
        !caps.has_power_profiles(),
        "ZeroCapabilities must report no power profiles"
    );
    assert!(
        !caps.has_rgb_lighting(),
        "ZeroCapabilities must report no RGB lighting"
    );
    assert!(
        !caps.has_battery_metrics(),
        "ZeroCapabilities must report no battery metrics"
    );
    assert_eq!(
        caps.reported_sensor_count(),
        0,
        "Reported sensor count must be 0"
    );

    // Verify telemetry snapshot carries no fabricated numbers
    let sample = mock
        .sample_telemetry()
        .expect("ZeroCapabilities sample must succeed");
    assert!(sample.cpu_name.is_empty());
    assert!(sample.gpu_name.is_none());
    assert!(sample.gpu_utilization.is_none());
    assert_eq!(sample.ram_total_mb, 0);
    assert_eq!(sample.ram_used_mb, 0);
    assert!(sample.fan_rpms.is_empty());
    assert_eq!(sample.reported_sensor_count(), 0);
}

#[test]
fn sensor_data_gap_scenario_periodically_emits_none() {
    let mock = MockHardware::new(MockScenario::SensorDataGap);

    let mut saw_present = false;
    let mut saw_none = false;

    // Sample across 15 ticks (period is 10 ticks, gap is 3 ticks)
    for _ in 0..15 {
        let sample = mock
            .sample_telemetry()
            .expect("Sampling SensorDataGap must succeed");
        if sample.gpu_utilization.is_some() {
            saw_present = true;
        } else {
            saw_none = true;
        }
    }

    assert!(
        saw_present,
        "SensorDataGap must emit valid GPU utilization during nominal ticks"
    );
    assert!(
        saw_none,
        "SensorDataGap must emit None GPU utilization during gap ticks"
    );
}
