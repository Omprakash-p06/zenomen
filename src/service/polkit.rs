//! Polkit authorization verification over system D-Bus.

use std::collections::HashMap;
use zbus::zvariant::{OwnedValue, Value};
use zbus::Connection;

use crate::service::types::HardwareControlError;

/// Action ID defined in `data/polkit/com.zenomen.hardware.policy`.
pub const POLKIT_ACTION_CONTROL: &str = "com.zenomen.hardware.control";

/// Result from polkit `CheckAuthorization` call: (is_authorized, is_challenge, details).
#[derive(Debug, Clone)]
pub struct AuthorizationResult {
    pub is_authorized: bool,
    pub is_challenge: bool,
    pub details: HashMap<String, String>,
}

/// Polkit verifier communicating with `org.freedesktop.PolicyKit1.Authority`.
pub struct PolkitVerifier;

impl PolkitVerifier {
    /// Verify if `caller_bus_name` is authorized to perform `action_id`.
    ///
    /// If `allow_user_interaction` is true (flag = 1), polkit is allowed to trigger
    /// an authentication agent dialog (e.g., password prompt) for the active session.
    pub async fn check_authorization(
        connection: &Connection,
        caller_bus_name: &str,
        action_id: &str,
        allow_user_interaction: bool,
    ) -> Result<bool, HardwareControlError> {
        let mut subject_details: HashMap<&str, Value<'_>> = HashMap::new();
        subject_details.insert("name", Value::from(caller_bus_name));

        let subject = ("system-bus-name", subject_details);
        let action_details: HashMap<&str, &str> = HashMap::new();
        let flags: u32 = if allow_user_interaction { 1 } else { 0 };
        let cancellation_id = "";

        let reply = connection
            .call_method(
                Some("org.freedesktop.PolicyKit1"),
                "/org/freedesktop/PolicyKit1/Authority",
                Some("org.freedesktop.PolicyKit1.Authority"),
                "CheckAuthorization",
                &(subject, action_id, action_details, flags, cancellation_id),
            )
            .await
            .map_err(|e| {
                HardwareControlError::PolkitDenied(format!(
                    "Failed to query PolicyKit1 Authority: {e}"
                ))
            })?;

        let (is_authorized, _is_challenge, _details): (bool, bool, HashMap<String, OwnedValue>) =
            reply.body().deserialize().map_err(|e| {
                HardwareControlError::PolkitDenied(format!(
                    "Invalid response body from PolicyKit1: {e}"
                ))
            })?;

        if is_authorized {
            Ok(true)
        } else {
            Err(HardwareControlError::PolkitDenied(format!(
                "Action '{action_id}' was not authorized for caller '{caller_bus_name}'"
            )))
        }
    }
}
