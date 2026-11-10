//! Integration and unit tests for ZenOmen RGB keyboard lighting.
//!
//! Verifies:
//! - Multi-variant interface discovery (`FourZone`, `Legacy`, `KernelPatch`) (RGB-01)
//! - Preset definitions (`White`, `Gaming`, `Rainbow`, `Off`) (RGB-03)
//! - Strict `#RRGGBB` hex color validation and error handling (RGB-06)
//! - Non-destructive OpenOMEN profile migration and source integrity (RGB-05)
//! - Forward-compatible unsupported animation field tracking (RGB-07)
//! - Sysfs hardware writes and read-back verification across all 3 variants (RGB-08)

use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use zenomen::hardware::rgb::RgbInterfaceVariant;
use zenomen::rgb::migration::{builtin_presets, migrate_openomen_profiles, parse_openomen_profile};
use zenomen::rgb::schema::{RgbColor, RgbError};
use zenomen::service::backend::HardwareBackend;

fn temp_test_dir(name: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("zenomen_rgb_test_{name}_{nanos}"));
    fs::create_dir_all(&dir).expect("create test temp dir");
    dir
}

#[test]
fn strict_hex_color_validation_success_and_failure() {
    // Valid cases
    let white = RgbColor::from_hex("#FFFFFF").expect("valid 7-char uppercase");
    assert_eq!(white, RgbColor::new(255, 255, 255));
    assert_eq!(white.to_hex(), "#FFFFFF");
    assert_eq!(white.to_hex_raw(), "FFFFFF");

    let no_hash = RgbColor::from_hex("00FF00").expect("valid 6-char without hash");
    assert_eq!(no_hash, RgbColor::new(0, 255, 0));

    let lowercase = RgbColor::from_hex("#0000ff").expect("valid lowercase");
    assert_eq!(lowercase, RgbColor::new(0, 0, 255));

    // Invalid: Empty
    assert!(matches!(RgbColor::from_hex(""), Err(RgbError::EmptyColor)));
    assert!(matches!(
        RgbColor::from_hex("   "),
        Err(RgbError::EmptyColor)
    ));

    // Invalid: Too short / too long (RGB-06)
    assert!(matches!(
        RgbColor::from_hex("#FFF"),
        Err(RgbError::InvalidLength(_))
    ));
    assert!(matches!(
        RgbColor::from_hex("#FFFFFFFF"),
        Err(RgbError::InvalidLength(_))
    ));
    assert!(matches!(
        RgbColor::from_hex("12345"),
        Err(RgbError::InvalidLength(_))
    ));

    // Invalid: Non-hex characters (RGB-06)
    assert!(matches!(
        RgbColor::from_hex("#GGGGGG"),
        Err(RgbError::InvalidHexCharacter(_))
    ));
    assert!(matches!(
        RgbColor::from_hex("ZZ1122"),
        Err(RgbError::InvalidHexCharacter(_))
    ));
    assert!(matches!(
        RgbColor::from_hex("#12 456"),
        Err(RgbError::InvalidHexCharacter(_))
    ));
}

#[test]
fn builtin_presets_match_specification() {
    let presets = builtin_presets();
    assert_eq!(presets.len(), 4);

    let white = presets.iter().find(|p| p.id == "white").expect("white");
    assert!(white.zones.iter().all(|c| c.to_hex() == "#FFFFFF"));

    let off = presets.iter().find(|p| p.id == "off").expect("off");
    assert!(off.is_off());
    assert!(off.zones.iter().all(|c| c.to_hex() == "#000000"));

    let gaming = presets.iter().find(|p| p.id == "gaming").expect("gaming");
    assert_eq!(gaming.zones[0].to_hex(), "#FF0000"); // Left flank
    assert_eq!(gaming.zones[1].to_hex(), "#0000FF"); // Center left (WASD)
    assert_eq!(gaming.zones[2].to_hex(), "#0000FF"); // Center right
    assert_eq!(gaming.zones[3].to_hex(), "#FF0000"); // Right flank

    let rainbow = presets.iter().find(|p| p.id == "rainbow").expect("rainbow");
    assert_eq!(rainbow.zones[0].to_hex(), "#FF0000");
    assert_eq!(rainbow.zones[1].to_hex(), "#FF7F00");
    assert_eq!(rainbow.zones[2].to_hex(), "#00FF00");
    assert_eq!(rainbow.zones[3].to_hex(), "#0000FF");
}

#[test]
fn animation_fields_tracked_as_unsupported_without_failure() {
    let json_content = r##"{
        "name": "Breathing Wave",
        "description": "Legacy animated breathing effect",
        "zones": {
            "0": "#FF0000",
            "1": "#FF7F00",
            "2": "#00FF00",
            "3": "#0000FF"
        },
        "brightness": 80,
        "speed": 5,
        "length_ms": 3000,
        "loop": true
    }"##;

    let profile = parse_openomen_profile("breathing_wave", json_content)
        .expect("should successfully parse profile");
    assert_eq!(profile.name, "Breathing Wave");
    assert_eq!(profile.zones[0], RgbColor::new(255, 0, 0));
    assert_eq!(profile.zones[3], RgbColor::new(0, 0, 255));

    // RGB-07: animation fields detected and tracked
    assert!(
        profile
            .unsupported_fields
            .contains(&"brightness".to_string())
    );
    assert!(profile.unsupported_fields.contains(&"speed".to_string()));
    assert!(
        profile
            .unsupported_fields
            .contains(&"length_ms".to_string())
    );
    assert!(profile.unsupported_fields.contains(&"loop".to_string()));
}

#[test]
fn openomen_migration_is_strictly_non_destructive() {
    let base_dir = temp_test_dir("migration");
    let source_dir = base_dir.join("openomen_rgb");
    let target_dir = base_dir.join("zenomen_rgb");
    fs::create_dir_all(&source_dir).unwrap();

    let profile_1_content =
        r##"{"name": "Profile One", "zones": ["#112233", "#445566", "#778899", "#AABBCC"]}"##;
    let profile_2_content = r##"{"name": "Profile Two", "zones": {"0": "#FFFFFF", "1": "#000000", "2": "#FFFFFF", "3": "#000000"}}"##;
    let corrupted_content = r##"{"name": "Bad", "zones": ["#INVALID", "#FFFFFF"]}"##;

    let p1_path = source_dir.join("profile_one.json");
    let p2_path = source_dir.join("profile_two.json");
    let bad_path = source_dir.join("corrupted.json");

    fs::write(&p1_path, profile_1_content).unwrap();
    fs::write(&p2_path, profile_2_content).unwrap();
    fs::write(&bad_path, corrupted_content).unwrap();

    // Execute migration (RGB-05)
    let report = migrate_openomen_profiles(&source_dir, &target_dir);

    assert_eq!(report.total_scanned, 3);
    assert_eq!(report.imported.len(), 2);
    assert_eq!(report.skipped.len(), 1);

    // Verify migrated target files exist and can be parsed
    assert!(target_dir.join("profile_one.json").exists());
    assert!(target_dir.join("profile_two.json").exists());

    // CRITICAL (RGB-05): Verify source files were NEVER deleted, modified, or overwritten
    assert_eq!(
        fs::read_to_string(&p1_path).unwrap(),
        profile_1_content,
        "Source file 1 must remain completely untouched"
    );
    assert_eq!(
        fs::read_to_string(&p2_path).unwrap(),
        profile_2_content,
        "Source file 2 must remain completely untouched"
    );
    assert_eq!(
        fs::read_to_string(&bad_path).unwrap(),
        corrupted_content,
        "Corrupted source file must remain completely untouched"
    );

    // WR-10: On re-run when targets already exist, profiles are reported as preserved, not imported
    let report2 = migrate_openomen_profiles(&source_dir, &target_dir);
    assert_eq!(report2.total_scanned, 3);
    assert_eq!(report2.imported.len(), 0);
    assert_eq!(report2.preserved.len(), 2);
    assert_eq!(report2.skipped.len(), 1);

    let _ = fs::remove_dir_all(&base_dir);
}

#[test]
#[cfg(not(target_os = "windows"))]
fn interface_variant_discovery_all_three_variants() {
    use zenomen::hardware::rgb::detect_rgb_interface;
    let base_dir = temp_test_dir("discovery");

    // 1. FourZone: /sys/devices/platform/hp-wmi/rgb_zones/zone00
    let fz_root = base_dir.join("four_zone");
    fs::create_dir_all(fz_root.join("sys/devices/platform/hp-wmi/rgb_zones")).unwrap();
    fs::write(
        fz_root.join("sys/devices/platform/hp-wmi/rgb_zones/zone00"),
        "FFFFFF\n",
    )
    .unwrap();
    assert_eq!(
        detect_rgb_interface(&fz_root),
        Some(RgbInterfaceVariant::FourZone)
    );

    // 2. Legacy: /sys/devices/platform/hp-wmi/keyboardleds
    let leg_root = base_dir.join("legacy");
    fs::create_dir_all(leg_root.join("sys/devices/platform/hp-wmi")).unwrap();
    fs::write(
        leg_root.join("sys/devices/platform/hp-wmi/keyboardleds"),
        "FFFFFF000000FFFFFF000000\n",
    )
    .unwrap();
    assert_eq!(
        detect_rgb_interface(&leg_root),
        Some(RgbInterfaceVariant::Legacy)
    );

    // 3. KernelPatch: /sys/class/leds/hp_omen::kbd_backlight/zone_colors
    let kp_root = base_dir.join("kernel_patch");
    fs::create_dir_all(kp_root.join("sys/class/leds/hp_omen::kbd_backlight")).unwrap();
    fs::write(
        kp_root.join("sys/class/leds/hp_omen::kbd_backlight/zone_colors"),
        [0u8; 12],
    )
    .unwrap();
    assert_eq!(
        detect_rgb_interface(&kp_root),
        Some(RgbInterfaceVariant::KernelPatch)
    );

    // 4. None: empty directory
    let none_root = base_dir.join("none");
    fs::create_dir_all(&none_root).unwrap();
    assert_eq!(detect_rgb_interface(&none_root), None);

    let _ = fs::remove_dir_all(&base_dir);
}

#[test]
fn backend_hardware_writes_and_verification_four_zone() {
    let root = temp_test_dir("backend_four_zone");
    let zone_dir = root.join("sys/devices/platform/hp-wmi/rgb_zones");
    fs::create_dir_all(&zone_dir).unwrap();

    for i in 0..4 {
        fs::write(zone_dir.join(format!("zone0{i}")), "000000\n").unwrap();
    }

    let backend = HardwareBackend::new(&root);
    let (variant, path) = backend.find_rgb_interface().expect("interface found");
    assert_eq!(variant, RgbInterfaceVariant::FourZone);
    assert_eq!(path, zone_dir);

    let colors = [
        RgbColor::new(255, 0, 0),     // Zone 0: FF0000
        RgbColor::new(0, 255, 0),     // Zone 1: 00FF00
        RgbColor::new(0, 0, 255),     // Zone 2: 0000FF
        RgbColor::new(255, 255, 255), // Zone 3: FFFFFF
    ];

    backend
        .apply_rgb_hardware(&colors)
        .expect("write and verify succeeds");

    assert_eq!(
        fs::read_to_string(zone_dir.join("zone00")).unwrap().trim(),
        "FF0000"
    );
    assert_eq!(
        fs::read_to_string(zone_dir.join("zone01")).unwrap().trim(),
        "00FF00"
    );
    assert_eq!(
        fs::read_to_string(zone_dir.join("zone02")).unwrap().trim(),
        "0000FF"
    );
    assert_eq!(
        fs::read_to_string(zone_dir.join("zone03")).unwrap().trim(),
        "FFFFFF"
    );

    let read_back = backend.get_rgb_colors().expect("read back colors");
    assert_eq!(read_back, colors);

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn backend_hardware_writes_and_verification_legacy() {
    let root = temp_test_dir("backend_legacy");
    let dev_dir = root.join("sys/devices/platform/hp-wmi");
    fs::create_dir_all(&dev_dir).unwrap();
    let kbd_path = dev_dir.join("keyboardleds");
    fs::write(&kbd_path, "000000000000000000000000\n").unwrap();

    let backend = HardwareBackend::new(&root);
    let (variant, _) = backend
        .find_rgb_interface()
        .expect("legacy interface found");
    assert_eq!(variant, RgbInterfaceVariant::Legacy);

    let colors = [
        RgbColor::new(255, 255, 255),
        RgbColor::new(0, 0, 0),
        RgbColor::new(255, 255, 255),
        RgbColor::new(0, 0, 0),
    ];

    backend
        .apply_rgb_hardware(&colors)
        .expect("legacy write succeeds");
    assert_eq!(
        fs::read_to_string(&kbd_path).unwrap().trim(),
        "FFFFFF000000FFFFFF000000"
    );

    let _ = fs::remove_dir_all(&root);
}

#[test]
#[cfg(not(target_os = "windows"))]
fn backend_hardware_writes_and_verification_kernel_patch() {
    let root = temp_test_dir("backend_kernel_patch");
    let led_dir = root.join("sys/class/leds/hp_omen::kbd_backlight");
    fs::create_dir_all(&led_dir).unwrap();
    let color_path = led_dir.join("zone_colors");
    fs::write(&color_path, [0u8; 12]).unwrap();

    let backend = HardwareBackend::new(&root);
    let (variant, _) = backend.find_rgb_interface().expect("kernel patch found");
    assert_eq!(variant, RgbInterfaceVariant::KernelPatch);

    let colors = [
        RgbColor::new(10, 20, 30),
        RgbColor::new(40, 50, 60),
        RgbColor::new(70, 80, 90),
        RgbColor::new(100, 110, 120),
    ];

    backend
        .apply_rgb_hardware(&colors)
        .expect("binary bytes write succeeds");
    let bytes = fs::read(&color_path).unwrap();
    assert_eq!(
        bytes,
        vec![10, 20, 30, 40, 50, 60, 70, 80, 90, 100, 110, 120]
    );

    let read_back = backend.get_rgb_colors().expect("read back bytes");
    assert_eq!(read_back, colors);

    let _ = fs::remove_dir_all(&root);
}

#[test]
#[cfg(target_os = "linux")]
fn test_rgb_enable_restores_previous_profile_instead_of_hardcoded_white() {
    use zenomen::rgb::schema::RgbProfile;
    use zenomen::service::contract::HardwareService;

    let root = temp_test_dir("backend_restore");
    let dev_dir = root.join("sys/devices/platform/hp-wmi");
    fs::create_dir_all(&dev_dir).unwrap();
    let kbd_path = dev_dir.join("keyboardleds");
    fs::write(&kbd_path, "000000000000000000000000\n").unwrap();

    let backend = HardwareBackend::new(&root);
    let service = HardwareService::new(backend);

    let red_profile = RgbProfile::new(
        "all-red",
        "All Red",
        "All red profile",
        [
            RgbColor::new(255, 0, 0),
            RgbColor::new(255, 0, 0),
            RgbColor::new(255, 0, 0),
            RgbColor::new(255, 0, 0),
        ],
    );

    // 1. Apply Red Profile
    service
        .apply_rgb_profile_internal(&red_profile)
        .expect("profile applied");
    assert_eq!(
        fs::read_to_string(&kbd_path).unwrap().trim(),
        "FF0000FF0000FF0000FF0000"
    );

    // 2. Disable RGB (should turn off / black)
    service
        .set_rgb_enabled_internal(false)
        .expect("rgb disabled");
    assert_eq!(
        fs::read_to_string(&kbd_path).unwrap().trim(),
        "000000000000000000000000"
    );

    // 3. Re-enable RGB (should restore Red profile, NOT hardcoded white)
    service
        .set_rgb_enabled_internal(true)
        .expect("rgb re-enabled");
    assert_eq!(
        fs::read_to_string(&kbd_path).unwrap().trim(),
        "FF0000FF0000FF0000FF0000",
        "Enabling RGB must restore the previously applied profile rather than resetting to white"
    );

    let _ = fs::remove_dir_all(&root);
}
