//! Integration and unit tests for ZenOmen configuration management.
//!
//! Verifies:
//! - Roundtrip serialization and deserialization (SYS-06)
//! - Atomic file writing via temporary files (SYS-06)
//! - Automatic recovery and backup creation on corrupted TOML (SYS-07)
//! - Telemetry polling interval clamping within safe bounds [500ms, 5000ms] (SYS-09)

use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};
use zenomen::config::{
    AppConfig, GeneralConfig, MAX_POLLING_INTERVAL_MS, MIN_POLLING_INTERVAL_MS, TelemetryConfig,
    load_or_default, save_atomic,
};

fn temp_test_dir(name: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("zenomen_config_test_{name}_{nanos}"));
    fs::create_dir_all(&dir).expect("create test temp dir");
    dir
}

#[test]
fn config_default_values_match_specification() {
    let config = AppConfig::default();
    assert_eq!(config.general.theme, "dark");
    assert!(!config.general.start_minimized);
    assert!(config.general.close_to_tray);
    assert!(!config.general.autostart);
    assert_eq!(config.telemetry.history_window, 120);
    assert_eq!(config.telemetry.polling_interval_ms, 1000);
}

#[test]
fn config_roundtrip_save_and_load() {
    let dir = temp_test_dir("roundtrip");
    let path = dir.join("config.toml");

    let custom = AppConfig {
        general: GeneralConfig {
            theme: "dark".to_string(),
            start_minimized: true,
            close_to_tray: false,
            autostart: true,
        },
        telemetry: TelemetryConfig {
            history_window: 60,
            polling_interval_ms: 2000,
        },
    };

    save_atomic(&path, &custom).expect("atomic save succeeds");
    assert!(path.exists(), "config file should exist after atomic save");

    let (loaded, notice) = load_or_default(&path);
    assert!(notice.is_none(), "no corruption notice for valid config");
    assert_eq!(loaded, custom, "loaded config must match saved config");

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn config_atomic_write_leaves_no_lingering_temp_file() {
    let dir = temp_test_dir("atomic");
    let path = dir.join("config.toml");
    let tmp_path = path.with_extension("tmp");

    let config = AppConfig::default();
    save_atomic(&path, &config).expect("atomic save");

    assert!(path.exists());
    assert!(!tmp_path.exists(), "temporary file must be renamed away");

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn config_polling_interval_clamped_to_safe_bounds() {
    // Underflow clamped to MIN_POLLING_INTERVAL_MS (500)
    let mut underflow = AppConfig {
        general: GeneralConfig::default(),
        telemetry: TelemetryConfig {
            history_window: 120,
            polling_interval_ms: 100, // < 500ms
        },
    };
    underflow.sanitize();
    assert_eq!(
        underflow.telemetry.polling_interval_ms, MIN_POLLING_INTERVAL_MS,
        "underflow interval must be clamped to 500ms"
    );

    // Overflow clamped to MAX_POLLING_INTERVAL_MS (5000)
    let mut overflow = AppConfig {
        general: GeneralConfig::default(),
        telemetry: TelemetryConfig {
            history_window: 120,
            polling_interval_ms: 10_000, // > 5000ms
        },
    };
    overflow.sanitize();
    assert_eq!(
        overflow.telemetry.polling_interval_ms, MAX_POLLING_INTERVAL_MS,
        "overflow interval must be clamped to 5000ms"
    );
}

#[test]
fn config_corrupt_file_recovery_and_backup() {
    let dir = temp_test_dir("corrupt");
    let path = dir.join("config.toml");

    let corrupt_content = "[[this is totally invalid { toml ::: content";
    fs::write(&path, corrupt_content).expect("write corrupt config");

    let (loaded, notice) = load_or_default(&path);

    // 1. Safe defaults loaded
    assert_eq!(
        loaded,
        AppConfig::default(),
        "corrupt config must load safe defaults"
    );

    // 2. Human-readable notice returned
    assert!(
        notice.is_some(),
        "corruption notice must be returned to user"
    );
    let notice_str = notice.unwrap();
    assert!(
        notice_str.contains("invalid"),
        "notice should describe error"
    );
    assert!(
        notice_str.contains("bak"),
        "notice should reference backup file"
    );

    // 3. Backup file created on disk containing the exact original bad content
    let entries: Vec<_> = fs::read_dir(&dir)
        .expect("read dir")
        .filter_map(|e| e.ok())
        .collect();

    let backup_entry = entries
        .iter()
        .find(|e| e.file_name().to_string_lossy().contains("bak"))
        .expect("backup file with .bak.<timestamp> must exist in directory");

    let backup_content = fs::read_to_string(backup_entry.path()).expect("read backup file content");
    assert_eq!(
        backup_content, corrupt_content,
        "backup file must contain the exact original corrupt content"
    );

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn config_nonexistent_file_returns_defaults_without_notice() {
    let dir = temp_test_dir("nonexistent");
    let path = dir.join("does_not_exist.toml");

    let (loaded, notice) = load_or_default(&path);
    assert_eq!(loaded, AppConfig::default());
    assert!(notice.is_none());

    let _ = fs::remove_dir_all(&dir);
}
