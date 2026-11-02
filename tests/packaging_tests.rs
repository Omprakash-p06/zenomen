//! Integration and unit tests for system integration and packaging (REL-03, T-05-02).

use std::fs;
use std::path::PathBuf;
use zenomen::desktop::system_integration::{
    DBUS_CONF_TARGET, DESKTOP_ENTRY_TARGET, EMBEDDED_DBUS_CONF, EMBEDDED_DESKTOP_ENTRY,
    EMBEDDED_POLKIT_POLICY, EMBEDDED_SYSTEMD_UNIT, POLKIT_POLICY_TARGET, SYSTEMD_UNIT_TARGET,
    SystemIntegrationError, atomic_write_file, install, install_to_prefix, is_root, uninstall,
    uninstall_from_prefix,
};

#[test]
fn test_embedded_assets_match_disk_files() {
    let desktop_disk =
        fs::read_to_string("data/applications/zenomen.desktop").expect("desktop entry exists");
    assert_eq!(
        EMBEDDED_DESKTOP_ENTRY, desktop_disk,
        "Embedded desktop entry must match data/applications/zenomen.desktop"
    );

    let systemd_disk =
        fs::read_to_string("data/systemd/zenomen-hw.service").expect("systemd unit exists");
    assert_eq!(
        EMBEDDED_SYSTEMD_UNIT, systemd_disk,
        "Embedded systemd unit must match data/systemd/zenomen-hw.service"
    );

    let dbus_disk =
        fs::read_to_string("data/dbus/com.zenomen.Hardware1.conf").expect("dbus conf exists");
    assert_eq!(
        EMBEDDED_DBUS_CONF, dbus_disk,
        "Embedded D-Bus conf must match data/dbus/com.zenomen.Hardware1.conf"
    );

    let polkit_disk = fs::read_to_string("data/polkit/com.zenomen.hardware.policy")
        .expect("polkit policy exists");
    assert_eq!(
        EMBEDDED_POLKIT_POLICY, polkit_disk,
        "Embedded polkit policy must match data/polkit/com.zenomen.hardware.policy"
    );
}

#[test]
fn test_root_privilege_enforcement_when_non_root() {
    if !is_root() {
        let install_result = install();
        match install_result {
            Err(SystemIntegrationError::NotRoot) => {}
            other => panic!("Expected NotRoot error when non-root, got: {:?}", other),
        }

        let uninstall_result = uninstall();
        match uninstall_result {
            Err(SystemIntegrationError::NotRoot) => {}
            other => panic!("Expected NotRoot error when non-root, got: {:?}", other),
        }
    }
}

#[test]
fn test_install_and_uninstall_to_prefix_mock() {
    let temp_dir = std::env::temp_dir().join(format!("zenomen_test_prefix_{}", std::process::id()));
    let _ = fs::remove_dir_all(&temp_dir);

    // Install to temporary mock root
    install_to_prefix(&temp_dir).expect("install_to_prefix should succeed");

    let desktop_file = temp_dir.join(DESKTOP_ENTRY_TARGET.trim_start_matches('/'));
    let unit_file = temp_dir.join(SYSTEMD_UNIT_TARGET.trim_start_matches('/'));
    let dbus_file = temp_dir.join(DBUS_CONF_TARGET.trim_start_matches('/'));
    let polkit_file = temp_dir.join(POLKIT_POLICY_TARGET.trim_start_matches('/'));

    assert!(
        desktop_file.is_file(),
        "desktop file should exist in prefix"
    );
    assert!(unit_file.is_file(), "unit file should exist in prefix");
    assert!(dbus_file.is_file(), "dbus file should exist in prefix");
    assert!(polkit_file.is_file(), "polkit file should exist in prefix");

    assert_eq!(
        fs::read_to_string(&desktop_file).unwrap(),
        EMBEDDED_DESKTOP_ENTRY
    );
    assert_eq!(
        fs::read_to_string(&unit_file).unwrap(),
        EMBEDDED_SYSTEMD_UNIT
    );
    assert_eq!(fs::read_to_string(&dbus_file).unwrap(), EMBEDDED_DBUS_CONF);
    assert_eq!(
        fs::read_to_string(&polkit_file).unwrap(),
        EMBEDDED_POLKIT_POLICY
    );

    // Uninstall from prefix
    uninstall_from_prefix(&temp_dir).expect("uninstall_from_prefix should succeed");

    assert!(!desktop_file.exists(), "desktop file should be deleted");
    assert!(!unit_file.exists(), "unit file should be deleted");
    assert!(!dbus_file.exists(), "dbus file should be deleted");
    assert!(!polkit_file.exists(), "polkit file should be deleted");

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_atomic_write_file_creates_content_cleanly() {
    let temp_dir = std::env::temp_dir().join(format!("zenomen_test_atomic_{}", std::process::id()));
    let target = temp_dir.join("sub/dir/test.txt");

    let content = "Hello ZenOmen Packaging Test";
    atomic_write_file(&target, content).expect("atomic write must succeed");

    assert!(target.is_file());
    assert_eq!(fs::read_to_string(&target).unwrap(), content);

    // Verify no lingering temp file left behind
    let parent = target.parent().unwrap();
    let entries: Vec<PathBuf> = fs::read_dir(parent)
        .unwrap()
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .collect();

    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0], target);

    let _ = fs::remove_dir_all(&temp_dir);
}
