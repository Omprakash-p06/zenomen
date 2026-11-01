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
        "Hardware write verification failed for target '0': read-back was '2' after 3 retries".to_string(),
    )));
    assert_eq!(state.fan_mode, "max"); // preserved
    assert_eq!(state.pending_fan_mode, None);
    assert!(state.last_control_error.is_some());
    assert!(state.last_control_error.as_ref().unwrap().contains("verification failed"));

    // 3. Dismiss error clears error banner
    state.apply(&AppEvent::ControlErrorDismissed);
    assert_eq!(state.last_control_error, None);

    // 4. Switch back to auto
    state.apply(&AppEvent::FanModeChanged(Ok("auto".to_string())));
    assert_eq!(state.fan_mode, "auto");
    assert_eq!(state.last_control_error, None);
}

#[test]
fn test_service_status_state_transitions() {
    let mut state = AppState::default();
    assert_eq!(state.service_status, ServiceConnectionStatus::Disconnected);

    // Connected event
    state.apply(&AppEvent::ServiceStatusChanged(ServiceConnectionStatus::Connected));
    assert_eq!(state.service_status, ServiceConnectionStatus::Connected);
    assert!(state.service_status.is_connected());

    // Disconnected event
    state.apply(&AppEvent::ServiceStatusChanged(ServiceConnectionStatus::Disconnected));
    assert_eq!(state.service_status, ServiceConnectionStatus::Disconnected);
    assert!(!state.service_status.is_connected());
}

#[test]
fn test_client_handles_offline_daemon_gracefully() {
    let client = HardwareServiceClient::new();

    // When zenomen-hw daemon is offline in unit test environment:
    let status = client.status();
    assert!(!status.is_connected());

    // Calling fan mode when offline returns an Err without panicking
    let res = client.set_fan_mode("max");
    assert!(res.is_err());
}
