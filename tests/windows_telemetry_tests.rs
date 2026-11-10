//! Unit tests for Windows native telemetry calculation math and conversion contracts (WIN-01).
#![cfg(target_os = "windows")]

use zenomen::hardware::windows::battery::parse_battery_status;
use zenomen::hardware::windows::cpu::{WindowsCpuTimes, calculate_cpu_utilization};
use zenomen::hardware::windows::gpu::{GpuVendor, recognize_gpu_vendor};
use zenomen::hardware::windows::memory::calculate_ram_usage;

#[test]
fn test_filetime_delta_to_cpu_utilization_nominal() {
    // Simulated GetSystemTimes samples:
    // KernelTime includes IdleTime on Windows.
    // Over a 1-second interval (10,000,000 100ns units):
    // prev: kernel = 50,000, user = 20,000, idle = 40,000
    // curr: kernel = 70,000 (delta 20k), user = 30,000 (delta 10k), idle = 50,000 (delta 10k)
    // delta_total = 20k + 10k = 30k
    // busy = 30k - 10k = 20k
    // util = 20k / 30k * 100 = 66.666...%
    let prev = WindowsCpuTimes {
        kernel: 50_000,
        user: 20_000,
        idle: 40_000,
    };
    let curr = WindowsCpuTimes {
        kernel: 70_000,
        user: 30_000,
        idle: 50_000,
    };

    let util = calculate_cpu_utilization(prev, curr).expect("nominal utilization calculation");
    assert!(
        (util - 66.6667).abs() < 0.01,
        "Expected ~66.67%, got {util}%"
    );

    // 100% busy (zero idle delta)
    let busy_curr = WindowsCpuTimes {
        kernel: 70_000,
        user: 30_000,
        idle: 40_000,
    };
    let busy_util =
        calculate_cpu_utilization(prev, busy_curr).expect("100% utilization calculation");
    assert_eq!(busy_util, 100.0);

    // 0% busy (all delta is idle)
    let idle_curr = WindowsCpuTimes {
        kernel: 80_000, // delta 30k
        user: 20_000,   // delta 0
        idle: 70_000,   // delta 30k
    };
    let idle_util = calculate_cpu_utilization(prev, idle_curr).expect("0% utilization calculation");
    assert_eq!(idle_util, 0.0);
}

#[test]
fn test_filetime_zero_delta_returns_none() {
    let prev = WindowsCpuTimes {
        kernel: 100_000,
        user: 50_000,
        idle: 80_000,
    };

    // Exactly identical timestamps (zero elapsed time)
    assert_eq!(calculate_cpu_utilization(prev, prev), None);

    // Clock warp / backwards times return None safely
    let backwards = WindowsCpuTimes {
        kernel: 90_000,
        user: 40_000,
        idle: 70_000,
    };
    assert_eq!(calculate_cpu_utilization(prev, backwards), None);
}

#[test]
fn test_memory_math_total_and_available() {
    // 16 GB total (17,179,869,184 bytes), 6 GB available (6,442,450,944 bytes)
    let total_bytes: u64 = 16 * 1024 * 1024 * 1024;
    let avail_bytes: u64 = 6 * 1024 * 1024 * 1024;

    let (used, total) = calculate_ram_usage(total_bytes, avail_bytes);
    assert_eq!(total, Some(16384));
    assert_eq!(used, Some(10240)); // 16384 - 6144 = 10240 MB

    // Edge case: 0 total returns None
    assert_eq!(calculate_ram_usage(0, 0), (None, None));

    // Available exceeds total (anomaly / corrupted struct) saturates to 0 used
    let (used_sat, total_sat) = calculate_ram_usage(1000 * 1024 * 1024, 2000 * 1024 * 1024);
    assert_eq!(total_sat, Some(1000));
    assert_eq!(used_sat, Some(0));
}

#[test]
fn test_battery_status_ac_online_and_charging() {
    // Charging at 80%, flag = 8 (Charging) | 1 (High) = 9
    // LifeTime = 0xFFFFFFFF (unknown while charging)
    let bat = parse_battery_status(9, 80, 0xFFFFFFFF).expect("battery present");
    assert!(bat.present);
    assert_eq!(bat.charge_percent, Some(80));
    assert_eq!(bat.charging, Some(true));
    assert_eq!(bat.remaining_seconds, None);

    // Discharging at 45%, flag = 0, remaining = 7200 seconds (2 hours)
    let discharging = parse_battery_status(0, 45, 7200).expect("battery discharging");
    assert!(discharging.present);
    assert_eq!(discharging.charge_percent, Some(45));
    assert_eq!(discharging.charging, Some(false));
    assert_eq!(discharging.remaining_seconds, Some(7200));

    // Unknown percent (255)
    let unknown_pct = parse_battery_status(0, 255, 3600).expect("battery unknown percent");
    assert_eq!(unknown_pct.charge_percent, None);
    assert_eq!(unknown_pct.remaining_seconds, Some(3600));

    // Unknown flag (255) returns charging as None, not Some(true)
    let unknown_flag =
        parse_battery_status(255, 50, 3600).expect("battery present with unknown flag");
    assert_eq!(unknown_flag.charging, None);
}

#[test]
fn test_battery_status_missing_returns_none() {
    // Flag 128 = No system battery (Desktop / battery unplugged)
    assert_eq!(parse_battery_status(128, 255, 0xFFFFFFFF), None);
}

#[test]
fn test_dxgi_vendor_id_recognition() {
    assert_eq!(recognize_gpu_vendor(0x10DE), GpuVendor::Nvidia);
    assert_eq!(recognize_gpu_vendor(0x1002), GpuVendor::Amd);
    assert_eq!(recognize_gpu_vendor(0x8086), GpuVendor::Intel);
    assert_eq!(recognize_gpu_vendor(0x1234), GpuVendor::Unknown(0x1234));
}

#[test]
fn test_gpu_collector_and_vram() {
    use zenomen::hardware::windows::gpu::{
        GpuAdapterInfo, GpuPerformanceCollector, sample_gpu_memory, sample_gpu_utilization,
    };

    let mut collector = GpuPerformanceCollector::new();
    let _util = collector.sample_utilization();
    let _one_off = sample_gpu_utilization();

    // VRAM distinction: Dedicated VRAM total is reported, while used is honest None without query
    let adapter = GpuAdapterInfo {
        name: "NVIDIA GeForce RTX 4070 Laptop GPU".to_string(),
        vendor_id: 0x10DE,
        device_id: 0x2860,
        dedicated_vram_mb: 8192,
        shared_vram_mb: 16384,
        is_software: false,
    };

    let (used, total) = sample_gpu_memory(Some(&adapter));
    assert_eq!(total, Some(8192));
    assert_eq!(used, None); // Honest absence

    let (none_used, none_total) = sample_gpu_memory(None);
    assert_eq!(none_used, None);
    assert_eq!(none_total, None);
}

#[test]
fn test_sensor_telemetry_readings() {
    use zenomen::hardware::windows::sensors::{
        TemperatureSource, enumerate_sensor_sources, read_fan_readings, read_fan_rpms,
        read_temperature_readings, read_temperature_sensors,
    };

    let sources = enumerate_sensor_sources();
    assert!(sources.contains(&"Win32".to_string()));
    assert!(sources.contains(&"DXGI".to_string()));
    assert!(sources.contains(&"PDH".to_string()));

    let (cpu_temp, gpu_temp) = read_temperature_sensors();
    if let Some(c) = cpu_temp {
        assert!(c > 0.0 && c < 120.0);
    }
    assert_eq!(gpu_temp, None);

    let temps = read_temperature_readings();
    assert_eq!(temps.len(), 2);
    if cpu_temp.is_some() {
        assert_eq!(temps[0].source, TemperatureSource::AcpiThermalZone);
        assert_eq!(temps[0].celsius, cpu_temp);
    } else {
        assert_eq!(temps[0].source, TemperatureSource::Unavailable);
        assert_eq!(temps[0].celsius, None);
    }
    assert_eq!(temps[1].source, TemperatureSource::Unavailable);
    assert_eq!(temps[1].celsius, None);

    assert!(read_fan_rpms().is_empty());
    assert!(read_fan_readings().is_empty());
}
