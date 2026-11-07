//! Integration tests for fan mode control, read-back verification state, and UI state mapping.

use zenomen::core::events::AppEvent;
use zenomen::core::state::AppState;
use zenomen::service::client::{HardwareServiceClient, ServiceConnectionStatus};

#[test]
fn test_fan_mode_state_transitions() {
    let mut state = AppState::default();
    assert_eq!(state.fan_mode, "");
    assert_eq!(state.pending_fan_mode, None);
    assert_eq!(state.last_control_error, None);

    // 1. Success event updates fan mode and clears errors
    state.apply(&AppEvent::FanModeChanged(Ok("max".to_string())));
    assert_eq!(state.fan_mode, "max");
    assert_eq!(state.pending_fan_mode, None);
    assert_eq!(state.last_control_error, None);

    // 2. Failure event (e.g. VerificationFailed) preserves active mode and sets error
    state.apply(&AppEvent::FanModeChanged(Err(
        "Hardware write verification failed for target '0': read-back was '2' after 3 retries"
            .to_string(),
    )));
    assert_eq!(state.fan_mode, "max"); // preserved
    assert_eq!(state.pending_fan_mode, None);
    assert!(state.last_control_error.is_some());
    assert!(
        state
            .last_control_error
            .as_ref()
            .unwrap()
            .contains("verification failed")
    );

    // 3. Dismiss error clears error banner
    state.apply(&AppEvent::ControlErrorDismissed);
    assert_eq!(state.last_control_error, None);

    // 4. Switch back to auto
    state.apply(&AppEvent::FanModeChanged(Ok("auto".to_string())));
    assert_eq!(state.fan_mode, "auto");
    assert_eq!(state.last_control_error, None);
}

#[test]
fn test_pending_fan_mode_marks_the_write_in_flight() {
    let mut state = AppState::default();
    state.apply(&AppEvent::FanModeChanged(Ok("auto".to_string())));
    state.apply(&AppEvent::ControlErrorDismissed);

    // The request announces itself before the D-Bus call returns.
    state.apply(&AppEvent::FanModeRequested("max".to_string()));
    assert_eq!(state.pending_fan_mode.as_deref(), Some("max"));
    // The verified mode has not changed yet — only the request is known.
    assert_eq!(state.fan_mode, "auto");

    // Confirmed read-back clears the in-flight marker.
    state.apply(&AppEvent::FanModeChanged(Ok("max".to_string())));
    assert_eq!(state.pending_fan_mode, None);
    assert_eq!(state.fan_mode, "max");
}

#[test]
fn test_pending_fan_mode_is_cleared_by_a_failed_verification() {
    let mut state = AppState::default();

    state.apply(&AppEvent::FanModeRequested("max".to_string()));
    assert_eq!(state.pending_fan_mode.as_deref(), Some("max"));

    state.apply(&AppEvent::FanModeChanged(Err(
        "Hardware write verification failed for target '0'".to_string(),
    )));

    assert_eq!(state.pending_fan_mode, None);
    assert!(state.last_control_error.is_some());
}

#[test]
fn test_service_status_state_transitions() {
    let mut state = AppState::default();
    assert_eq!(state.service_status, ServiceConnectionStatus::Disconnected);

    // Connected event
    state.apply(&AppEvent::ServiceStatusChanged(
        ServiceConnectionStatus::Connected,
    ));
    assert_eq!(state.service_status, ServiceConnectionStatus::Connected);
    assert!(state.service_status.is_connected());

    // Disconnected event
    state.apply(&AppEvent::ServiceStatusChanged(
        ServiceConnectionStatus::Disconnected,
    ));
    assert_eq!(state.service_status, ServiceConnectionStatus::Disconnected);
    assert!(!state.service_status.is_connected());
}

#[test]
fn test_client_handles_offline_daemon_gracefully() {
    let client = HardwareServiceClient::new();

    // Guard against running against an active production daemon on host system (WR-11)
    let status = client.status();
    if status.is_connected() {
        eprintln!(
            "skipping test_client_handles_offline_daemon_gracefully: production service is active on host"
        );
        return;
    }

    assert!(!status.is_connected());

    // Calling fan mode when offline returns an Err without panicking
    let res = client.set_fan_mode("max");
    assert!(res.is_err());
    assert!(res.unwrap_err().contains("sudo systemctl start zenomen-hw"));
}
