//! Client layer connecting to `com.zenomen.Hardware1` on system D-Bus.

use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};
use zbus::blocking::Connection;

use crate::service::types::PresetResult;

/// Connection status of the hardware control D-Bus service.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ServiceConnectionStatus {
    Connected,
    Disconnected,
    Unavailable(String),
}

impl ServiceConnectionStatus {
    pub fn is_connected(&self) -> bool {
        matches!(self, Self::Connected)
    }

    pub fn description(&self) -> &'static str {
        match self {
            Self::Connected => "Connected",
            Self::Disconnected => "Disconnected",
            Self::Unavailable(_) => "Unavailable",
        }
    }
}

/// D-Bus client for interacting with the `zenomen-hw` privileged service.
#[derive(Clone)]
pub struct HardwareServiceClient {
    destination: String,
    path: String,
    interface: String,
    cached_conn: Arc<Mutex<Option<Connection>>>,
}

impl Default for HardwareServiceClient {
    fn default() -> Self {
        Self::new()
    }
}

impl HardwareServiceClient {
    /// Construct a client targeting default `com.zenomen.Hardware1` endpoint.
    pub fn new() -> Self {
        Self {
            destination: "com.zenomen.Hardware1".to_string(),
            path: "/com/zenomen/Hardware1".to_string(),
            interface: "com.zenomen.Hardware1".to_string(),
            cached_conn: Arc::new(Mutex::new(None)),
        }
    }

    /// Connect to system D-Bus, caching the active connection across calls.
    fn connect(&self) -> Result<Connection, String> {
        let mut guard = self.cached_conn.lock().unwrap_or_else(|p| p.into_inner());
        if let Some(conn) = &*guard {
            return Ok(conn.clone());
        }
        let conn = Connection::system()
            .map_err(|e| format!("Failed to connect to system D-Bus: {e}"))?;
        *guard = Some(conn.clone());
        Ok(conn)
    }

    /// Reset cached connection on communication failures.
    fn reset_connection(&self) {
        if let Ok(mut guard) = self.cached_conn.lock() {
            *guard = None;
        }
    }

    /// Check if `com.zenomen.Hardware1` is alive and responsive.
    pub fn status(&self) -> ServiceConnectionStatus {
        let conn = match self.connect() {
            Ok(c) => c,
            Err(e) => return ServiceConnectionStatus::Unavailable(e),
        };

        // Query GetCapabilities to ensure service is actively responding
        match conn.call_method(
            Some(self.destination.as_str()),
            self.path.as_str(),
            Some(self.interface.as_str()),
            "GetCapabilities",
            &(),
        ) {
            Ok(_) => ServiceConnectionStatus::Connected,
            Err(_) => {
                self.reset_connection();
                ServiceConnectionStatus::Disconnected
            }
        }
    }

    /// Read active fan status: (mode, rpms).
    pub fn get_fan_status(&self) -> Result<(String, Vec<u32>), String> {
        let conn = self.connect()?;
        let reply = conn
            .call_method(
                Some(self.destination.as_str()),
                self.path.as_str(),
                Some(self.interface.as_str()),
                "GetFanStatus",
                &(),
            )
            .map_err(|e| format!("GetFanStatus failed: {e}"))?;

        let status: (String, Vec<u32>) = reply
            .body()
            .deserialize()
            .map_err(|e| format!("Failed to parse GetFanStatus response: {e}"))?;

        Ok(status)
    }

    /// Set fan mode with verification.
    /// Mutating method: will prompt for Polkit if challenged.
    pub fn set_fan_mode(&self, mode: &str) -> Result<String, String> {
        let conn = self.connect()?;
        let reply = conn
            .call_method(
                Some(self.destination.as_str()),
                self.path.as_str(),
                Some(self.interface.as_str()),
                "SetFanMode",
                &(mode,),
            )
            .map_err(|e| format!("SetFanMode failed: {e}"))?;

        let confirmed: String = reply
            .body()
            .deserialize()
            .map_err(|e| format!("Failed to parse SetFanMode response: {e}"))?;

        Ok(confirmed)
    }

    /// Read advertised ACPI platform profile choices.
    pub fn get_power_profiles(&self) -> Result<Vec<String>, String> {
        let conn = self.connect()?;
        let reply = conn
            .call_method(
                Some(self.destination.as_str()),
                self.path.as_str(),
                Some(self.interface.as_str()),
                "GetPowerProfiles",
                &(),
            )
            .map_err(|e| format!("GetPowerProfiles failed: {e}"))?;

        let choices: Vec<String> = reply
            .body()
            .deserialize()
            .map_err(|e| format!("Failed to parse GetPowerProfiles response: {e}"))?;

        Ok(choices)
    }

    /// Read currently active ACPI platform profile.
    pub fn get_power_profile(&self) -> Result<String, String> {
        let conn = self.connect()?;
        let reply = conn
            .call_method(
                Some(self.destination.as_str()),
                self.path.as_str(),
                Some(self.interface.as_str()),
                "GetPowerProfile",
                &(),
            )
            .map_err(|e| format!("GetPowerProfile failed: {e}"))?;

        let active: String = reply
            .body()
            .deserialize()
            .map_err(|e| format!("Failed to parse GetPowerProfile response: {e}"))?;

        Ok(active)
    }

    /// Set ACPI platform profile with verification.
    /// Mutating method: will prompt for Polkit if challenged.
    pub fn set_power_profile(&self, profile: &str) -> Result<String, String> {
        let conn = self.connect()?;
        let reply = conn
            .call_method(
                Some(self.destination.as_str()),
                self.path.as_str(),
                Some(self.interface.as_str()),
                "SetPowerProfile",
                &(profile,),
            )
            .map_err(|e| format!("SetPowerProfile failed: {e}"))?;

        let confirmed: String = reply
            .body()
            .deserialize()
            .map_err(|e| format!("Failed to parse SetPowerProfile response: {e}"))?;

        Ok(confirmed)
    }

    /// Apply combined preset (Quiet, Balanced, Gaming) coordinating power profile and fan mode.
    pub fn apply_preset(&self, preset: &str) -> Result<PresetResult, String> {
        let conn = self.connect()?;
        let reply = conn
            .call_method(
                Some(self.destination.as_str()),
                self.path.as_str(),
                Some(self.interface.as_str()),
                "ApplyPreset",
                &(preset,),
            )
            .map_err(|e| format!("ApplyPreset failed: {e}"))?;

        let raw_json: String = reply
            .body()
            .deserialize()
            .map_err(|e| format!("Failed to parse ApplyPreset response: {e}"))?;

        let result: PresetResult = serde_json::from_str(&raw_json)
            .map_err(|e| format!("Invalid PresetResult JSON from service: {e}"))?;

        Ok(result)
    }

    /// Check polkit authorization readiness without user prompting (DIAG-04).
    pub fn check_polkit_status(&self) -> Result<String, String> {
        let conn = self.connect()?;
        let unique_name = conn
            .unique_name()
            .map(|n| n.as_str().to_string())
            .unwrap_or_default();

        let mut subject_details = std::collections::HashMap::new();
        subject_details.insert("name", zbus::zvariant::Value::from(unique_name.as_str()));
        let subject = ("system-bus-name", subject_details);
        let action_details: std::collections::HashMap<&str, &str> = std::collections::HashMap::new();
        let flags: u32 = 0; // Don't trigger interactive prompt
        let cancellation_id = "";

        let reply = conn
            .call_method(
                Some("org.freedesktop.PolicyKit1"),
                "/org/freedesktop/PolicyKit1/Authority",
                Some("org.freedesktop.PolicyKit1.Authority"),
                "CheckAuthorization",
                &(
                    subject,
                    crate::service::polkit::POLKIT_ACTION_CONTROL,
                    action_details,
                    flags,
                    cancellation_id,
                ),
            )
            .map_err(|e| format!("PolicyKit check failed: {e}"))?;

        let (is_authorized, is_challenge, _): (
            bool,
            bool,
            std::collections::HashMap<String, zbus::zvariant::OwnedValue>,
        ) = reply
            .body()
            .deserialize()
            .map_err(|e| format!("Failed to parse PolicyKit response: {e}"))?;

        if is_authorized {
            Ok("Authorized (Active console / Admin retained)".to_string())
        } else if is_challenge {
            Ok("Authentication challenge required (polkit prompt)".to_string())
        } else {
            Ok("Authorization not granted".to_string())
        }
    }

    /// Apply an RGB profile to hardware via com.zenomen.Hardware1.
    pub fn apply_rgb_profile(&self, profile: &crate::rgb::RgbProfile) -> Result<String, String> {
        let conn = self.connect()?;
        let json_str = serde_json::to_string(profile)
            .map_err(|e| format!("Failed to serialize profile: {e}"))?;
        let reply = conn
            .call_method(
                Some(self.destination.as_str()),
                self.path.as_str(),
                Some(self.interface.as_str()),
                "ApplyRgbProfile",
                &(json_str,),
            )
            .map_err(|e| format!("ApplyRgbProfile failed: {e}"))?;

        let msg: String = reply
            .body()
            .deserialize()
            .map_err(|e| format!("Failed to parse ApplyRgbProfile response: {e}"))?;

        Ok(msg)
    }

    /// Set RGB keyboard power state via com.zenomen.Hardware1 (RGB-04).
    pub fn set_rgb_enabled(&self, enabled: bool) -> Result<String, String> {
        let conn = self.connect()?;
        let reply = conn
            .call_method(
                Some(self.destination.as_str()),
                self.path.as_str(),
                Some(self.interface.as_str()),
                "SetRgbEnabled",
                &(enabled,),
            )
            .map_err(|e| format!("SetRgbEnabled failed: {e}"))?;

        let msg: String = reply
            .body()
            .deserialize()
            .map_err(|e| format!("Failed to parse SetRgbEnabled response: {e}"))?;

        Ok(msg)
    }
}
