//! Integration and unit tests for combined system profile orchestrator and display coordination.

use zenomen::config::AppConfig;
use zenomen::core::profile_orchestrator::{
    SystemPresetTargets, get_active_refresh_rate, resolve_preset_targets,
};

#[test]
fn test_resolve_preset_targets_defaults() {
    let config = AppConfig::default();

    let quiet = resolve_preset_targets("quiet", &config).expect("resolve quiet");
    assert_eq!(
        quiet,
        SystemPresetTargets {
            name: "quiet".to_string(),
            target_refresh_rate: 60,
            target_power_profile: "quiet".to_string(),
            target_fan_mode: "auto".to_string(),
        }
    );

    let balanced = resolve_preset_targets("balanced", &config).expect("resolve balanced");
    assert_eq!(
        balanced,
        SystemPresetTargets {
            name: "balanced".to_string(),
            target_refresh_rate: 144,
            target_power_profile: "balanced".to_string(),
            target_fan_mode: "auto".to_string(),
        }
    );

    let auto = resolve_preset_targets("auto", &config).expect("resolve auto");
    assert_eq!(
        auto,
        SystemPresetTargets {
            name: "auto".to_string(),
            target_refresh_rate: 144,
            target_power_profile: "balanced".to_string(),
            target_fan_mode: "auto".to_string(),
        }
    );

    let performance = resolve_preset_targets("performance", &config).expect("resolve performance");
    assert_eq!(
        performance,
        SystemPresetTargets {
            name: "performance".to_string(),
            target_refresh_rate: 144,
            target_power_profile: "performance".to_string(),
            target_fan_mode: "max".to_string(),
        }
    );

    let gaming = resolve_preset_targets("gaming", &config).expect("resolve gaming alias");
    assert_eq!(gaming.name, "performance");
    assert_eq!(gaming.target_fan_mode, "max");
}

#[test]
fn test_resolve_preset_targets_custom_config() {
    let mut config = AppConfig::default();
    config.display.preferred_quiet_refresh_rate = 75;
    config.display.preferred_active_refresh_rate = 165;

    let quiet = resolve_preset_targets("quiet", &config).expect("resolve quiet");
    assert_eq!(quiet.target_refresh_rate, 75);

    let perf = resolve_preset_targets("performance", &config).expect("resolve perf");
    assert_eq!(perf.target_refresh_rate, 165);
}

#[test]
fn test_resolve_preset_targets_invalid() {
    let config = AppConfig::default();
    let err = resolve_preset_targets("turbo_overclock", &config).unwrap_err();
    assert!(err.contains("Unknown combined preset"));
}

#[cfg(target_os = "windows")]
#[test]
fn test_windows_display_enumeration_and_probes() {
    use zenomen::hardware::windows::display::{
        get_active_refresh_rate, get_primary_display, is_refresh_rate_supported,
    };

    let primary = get_primary_display().expect("query primary display on Windows");
    assert!(!primary.device_name.is_empty());
    assert!(primary.width > 0);
    assert!(primary.height > 0);
    assert!(primary.current_refresh_rate > 0);
    assert!(!primary.supported_refresh_rates.is_empty());

    let active_hz = get_active_refresh_rate().expect("active refresh rate");
    assert_eq!(active_hz, primary.current_refresh_rate);

    // Primary display on this Victus hardware supports 60 and 144 Hz
    assert!(is_refresh_rate_supported(primary.current_refresh_rate).unwrap_or(false));
    assert!(!is_refresh_rate_supported(9999).unwrap_or(true));
}

#[test]
fn test_get_active_refresh_rate_wrapper() {
    let hz = get_active_refresh_rate();
    #[cfg(target_os = "windows")]
    assert!(hz.is_some(), "Windows should report active refresh rate");
    #[cfg(not(target_os = "windows"))]
    assert!(hz.is_none());
}
