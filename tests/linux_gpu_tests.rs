//! Tests for Linux GPU DRM enumeration and telemetry parsing.

use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};
use zenomen::hardware::linux::gpu::{
    GpuVendor, enumerate_drm_gpus, is_card_device_name, parse_nvidia_smi_output,
};

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

    fn path(&self) -> &std::path::Path {
        &self.0
    }
}

impl Drop for TestDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn card_device_filter_matches_only_primary_cards() {
    assert!(is_card_device_name("card0"));
    assert!(is_card_device_name("card1"));
    assert!(is_card_device_name("card2"));
    assert!(is_card_device_name("card10"));

    assert!(!is_card_device_name("card"));
    assert!(!is_card_device_name("card0-eDP-1"));
    assert!(!is_card_device_name("card1-DP-1"));
    assert!(!is_card_device_name("card1-HDMI-A-1"));
    assert!(!is_card_device_name("renderD128"));
    assert!(!is_card_device_name("controlD64"));
    assert!(!is_card_device_name("version"));
}

#[test]
fn dynamic_drm_enumeration_finds_all_cards_without_hardcoding_card0() {
    let dir = TestDir::new("zenomen-drm-test");
    let drm_path = dir.path();

    // Create simulated DRM hierarchy with card1 (no card0) and connector
    let card1_dir = drm_path.join("card1");
    let card1_device = card1_dir.join("device");
    fs::create_dir_all(&card1_device).expect("dir created");
    fs::write(card1_device.join("vendor"), "0x1002\n").expect("vendor written");

    let connector_dir = drm_path.join("card1-DP-1");
    fs::create_dir_all(&connector_dir).expect("connector dir created");

    let devices = enumerate_drm_gpus(drm_path);
    assert_eq!(devices.len(), 1, "Only card1 should be discovered");
    assert_eq!(devices[0].card_name, "card1");
    assert_eq!(devices[0].vendor, GpuVendor::Amd);
}

#[test]
fn parse_nvidia_smi_csv_handles_nominal_and_empty_output() {
    let output = "NVIDIA GeForce RTX 3070 Ti Laptop GPU, 42, 68, 2500, 8192\n";
    let telemetry = parse_nvidia_smi_output(output).expect("parses nominal output");

    assert_eq!(telemetry.name, "NVIDIA GeForce RTX 3070 Ti Laptop GPU");
    assert_eq!(telemetry.utilization, Some(42.0));
    assert_eq!(telemetry.temperature_c, Some(68.0));
    assert_eq!(telemetry.vram_used_mb, Some(2500));
    assert_eq!(telemetry.vram_total_mb, Some(8192));

    assert!(parse_nvidia_smi_output("").is_none());
    assert!(parse_nvidia_smi_output("NVIDIA Driver Not Loaded\n").is_none());
}
