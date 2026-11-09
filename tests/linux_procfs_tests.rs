//! Tests for Linux procfs CPU and memory parsers.
#![cfg(target_os = "linux")]

use zenomen::hardware::linux::cpu::{CpuSampler, read_cpu_model};
use zenomen::hardware::linux::memory::parse_proc_meminfo;
use zenomen::hardware::{HardwareProvider, LinuxHardwareProvider};

#[test]
fn cpu_sampler_calculates_jiffies_deltas_correctly() {
    let mut sampler = CpuSampler::new();

    let sample1 = "\
cpu  1000 0 500 8500 0 0 0 0 0 0
cpu0 500 0 250 4250 0 0 0 0 0 0
cpu1 500 0 250 4250 0 0 0 0 0 0
";
    // Total = 10000, Idle = 8500
    let (agg1, cores1) = sampler.sample_proc_stat(sample1);
    assert_eq!(agg1, 0.0, "First sample should establish baseline");
    assert_eq!(cores1.len(), 2);

    let sample2 = "\
cpu  1200 0 600 8600 0 0 0 0 0 0
cpu0 600 0 300 4300 0 0 0 0 0 0
cpu1 600 0 300 4300 0 0 0 0 0 0
";
    // Delta total = 10400 - 10000 = 400
    // Delta idle = 8600 - 8500 = 100
    // Busy delta = 300 / 400 = 75%
    let (agg2, cores2) = sampler.sample_proc_stat(sample2);
    assert!((agg2 - 75.0).abs() < 0.1, "Expected 75%, got {agg2}");
    assert_eq!(cores2.len(), 2);
    assert!((cores2[0] - 75.0).abs() < 0.1);
    assert!((cores2[1] - 75.0).abs() < 0.1);
}

#[test]
fn proc_meminfo_computes_used_from_memavailable_not_memfree() {
    let meminfo = "\
MemTotal:       32000000 kB
MemFree:         2000000 kB
MemAvailable:   24000000 kB
Buffers:         1000000 kB
Cached:          7000000 kB
";
    let snapshot = parse_proc_meminfo(meminfo).expect("must parse meminfo");
    let expected_total_mb = 32000000 / 1024;
    let expected_avail_mb = 24000000 / 1024;
    let expected_used_mb = expected_total_mb - expected_avail_mb;

    assert_eq!(snapshot.total_mb, expected_total_mb);
    assert_eq!(snapshot.available_mb, expected_avail_mb);
    assert_eq!(snapshot.used_mb, expected_used_mb);

    // If naive MemFree was used, used would be 32000000 - 2000000 = 30000000 kB (~29296 MB)
    let naive_used_mb = (32000000 - 2000000) / 1024;
    assert_ne!(snapshot.used_mb, naive_used_mb);
}

#[test]
fn cpu_model_extraction_handles_proc_cpuinfo() {
    let cpuinfo = "\
processor\t: 0
vendor_id\t: AuthenticAMD
cpu family\t: 25
model\t\t: 80
model name\t: AMD Ryzen 7 5800H with Radeon Graphics
stepping\t: 0
microcode\t: 0x9000000
";
    let model = read_cpu_model(cpuinfo).expect("model extracted");
    assert_eq!(model, "AMD Ryzen 7 5800H with Radeon Graphics");
}

#[test]
fn linux_hardware_provider_samples_cleanly_on_linux() {
    let provider = LinuxHardwareProvider::new();
    let capabilities = provider.capabilities();

    // On Linux systems, procfs CPU and RAM metrics should be detected
    assert!(capabilities.has_cpu_metrics());
    assert!(capabilities.has_ram_metrics());

    let snapshot = provider
        .sample_telemetry()
        .expect("sampling should succeed");
    assert!(!snapshot.cpu_name.is_empty());
    assert!(snapshot.ram_total_mb > 0);
    assert!(snapshot.ram_used_mb > 0);
}
