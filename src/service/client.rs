//! Client layer connecting to `com.zenomen.Hardware1` on system D-Bus (Linux)
//! or `ZenOmenHardwareService` via local named pipe (Windows).

#[cfg(target_os = "linux")]
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};
#[cfg(target_os = "linux")]
use zbus::blocking::Connection;

use crate::hardware::ProcessMemoryInfo;
use crate::service::types::{PowerProfileResult, PresetResult};
#[cfg(target_os = "windows")]
use crate::service::windows::client::WindowsServiceClient;
#[cfg(target_os = "windows")]
use crate::service::windows::protocol::{HardwareRequest, HardwareResponse};

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

/// Client for interacting with the privileged hardware service.
///
/// On Linux: communicates with `com.zenomen.Hardware1` on system D-Bus.
/// On Windows: communicates with `ZenOmenHardwareService` via named pipe `\\.\pipe\ZenOmen.Hardware.v1`.
#[derive(Clone)]
pub struct HardwareServiceClient {
    #[cfg(target_os = "linux")]
    destination: String,
    #[cfg(target_os = "linux")]
    path: String,
    #[cfg(target_os = "linux")]
    interface: String,
    #[cfg(target_os = "linux")]
    cached_conn: Arc<Mutex<Option<Connection>>>,
    #[cfg(target_os = "windows")]
    windows_client: WindowsServiceClient,
}

impl Default for HardwareServiceClient {
    fn default() -> Self {
        Self::new()
    }
}

impl HardwareServiceClient {
    /// Construct a client targeting default hardware service endpoint.
    pub fn new() -> Self {
        #[cfg(target_os = "linux")]
        {
            Self {
                destination: "com.zenomen.Hardware1".to_string(),
                path: "/com/zenomen/Hardware1".to_string(),
                interface: "com.zenomen.Hardware1".to_string(),
                cached_conn: Arc::new(Mutex::new(None)),
            }
        }
        #[cfg(target_os = "windows")]
        {
            Self {
                windows_client: WindowsServiceClient::new(),
            }
        }
    }

    /// Connect to system D-Bus, caching the active connection across calls.
    #[cfg(target_os = "linux")]
    fn connect(&self) -> Result<Connection, String> {
        let mut guard = self.cached_conn.lock().unwrap_or_else(|p| p.into_inner());
        if let Some(conn) = &*guard {
            return Ok(conn.clone());
        }
        let conn =
            Connection::system().map_err(|e| format!("Failed to connect to system D-Bus: {e}"))?;
        *guard = Some(conn.clone());
        Ok(conn)
    }

    /// Reset cached connection on communication failures.
    #[cfg(target_os = "linux")]
    fn reset_connection(&self) {
        if let Ok(mut guard) = self.cached_conn.lock() {
            *guard = None;
        }
    }

    /// Check if the hardware service is alive and responsive.
    pub fn status(&self) -> ServiceConnectionStatus {
        #[cfg(target_os = "windows")]
        {
            self.windows_client.get_service_status()
        }
        #[cfg(target_os = "linux")]
        {
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
    }

    /// Read active fan status: (mode, rpms).
    pub fn get_fan_status(&self) -> Result<(String, Vec<u32>), String> {
        #[cfg(target_os = "windows")]
        {
            match self
                .windows_client
                .send_request(HardwareRequest::GetFanStatus)
            {
                Ok(HardwareResponse::FanStatus { mode, rpms }) => Ok((mode, rpms)),
                Ok(HardwareResponse::Error(e)) => Err(e),
                Ok(_) => Err("Unexpected response for GetFanStatus".to_string()),
                Err(e) => Err(e.to_string()),
            }
        }
        #[cfg(target_os = "linux")]
        {
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
    }

    /// Set fan mode with verification.
    /// Mutating method: will prompt for Polkit if challenged on Linux, or check DACL/token on Windows.
    pub fn set_fan_mode(&self, mode: &str) -> Result<String, String> {
        #[cfg(target_os = "windows")]
        {
            self.windows_client.set_fan_mode(mode).map_err(|e| {
                tracing::error!(
                    function = "HardwareServiceClient::set_fan_mode",
                    mode,
                    error = %e,
                    "SetFanMode failed"
                );
                e.to_string()
            })
        }
        #[cfg(target_os = "linux")]
        {
            let conn = self.connect_service()?;
            let reply = conn
                .call_method(
                    Some(self.destination.as_str()),
                    self.path.as_str(),
                    Some(self.interface.as_str()),
                    "SetFanMode",
                    &(mode,),
                )
                .map_err(|e| self.format_service_error("SetFanMode", &e))?;

            let confirmed: String = reply
                .body()
                .deserialize()
                .map_err(|e| format!("Failed to parse SetFanMode response: {e}"))?;

            Ok(confirmed)
        }
    }

    /// Read advertised ACPI platform profile choices.
    pub fn get_power_profiles(&self) -> Result<Vec<String>, String> {
        #[cfg(target_os = "windows")]
        {
            match self
                .windows_client
                .send_request(HardwareRequest::GetPowerProfiles)
            {
                Ok(HardwareResponse::PowerProfiles(profiles)) => Ok(profiles),
                Ok(HardwareResponse::Error(e)) => Err(e),
                Ok(_) => Err("Unexpected response for GetPowerProfiles".to_string()),
                Err(e) => Err(e.to_string()),
            }
        }
        #[cfg(target_os = "linux")]
        {
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
    }

    /// Read currently active ACPI platform profile.
    pub fn get_power_profile(&self) -> Result<String, String> {
        #[cfg(target_os = "windows")]
        {
            match self
                .windows_client
                .send_request(HardwareRequest::GetPowerProfile)
            {
                Ok(HardwareResponse::PowerProfile(profile)) => Ok(profile),
                Ok(HardwareResponse::Error(e)) => Err(e),
                Ok(_) => Err("Unexpected response for GetPowerProfile".to_string()),
                Err(e) => Err(e.to_string()),
            }
        }
        #[cfg(target_os = "linux")]
        {
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
    }

    /// Set ACPI platform profile with verification.
    /// Mutating method: will prompt for Polkit if challenged on Linux, or check DACL/token on Windows.
    pub fn set_power_profile(&self, profile: &str) -> Result<String, String> {
        #[cfg(target_os = "windows")]
        {
            self.windows_client
                .set_power_profile(profile)
                .map(|res| res.profile)
                .map_err(|e| {
                    tracing::error!(
                        function = "HardwareServiceClient::set_power_profile",
                        profile,
                        error = %e,
                        "SetPowerProfile failed"
                    );
                    e.to_string()
                })
        }
        #[cfg(target_os = "linux")]
        {
            let conn = self.connect_service()?;
            let reply = conn
                .call_method(
                    Some(self.destination.as_str()),
                    self.path.as_str(),
                    Some(self.interface.as_str()),
                    "SetPowerProfile",
                    &(profile,),
                )
                .map_err(|e| self.format_service_error("SetPowerProfile", &e))?;

            let confirmed: String = reply
                .body()
                .deserialize()
                .map_err(|e| format!("Failed to parse SetPowerProfile response: {e}"))?;

            Ok(confirmed)
        }
    }

    /// Set ACPI platform profile returning detailed typed result (PWR-04).
    pub fn set_power_profile_detailed(&self, profile: &str) -> Result<PowerProfileResult, String> {
        #[cfg(target_os = "windows")]
        {
            self.windows_client
                .set_power_profile(profile)
                .map_err(|e| e.to_string())
        }
        #[cfg(target_os = "linux")]
        {
            let confirmed = self.set_power_profile(profile)?;
            Ok(PowerProfileResult::verified(confirmed.clone(), confirmed))
        }
    }

    /// Read process memory telemetry for the privileged hardware service daemon.
    pub fn get_service_memory(&self) -> Result<ProcessMemoryInfo, String> {
        #[cfg(target_os = "windows")]
        {
            self.windows_client
                .get_service_memory()
                .map_err(|e| e.to_string())
        }
        #[cfg(target_os = "linux")]
        {
            Ok(ProcessMemoryInfo {
                role: Some(crate::hardware::ProcessRole::HardwareService),
                available: false,
                ..Default::default()
            })
        }
    }

    /// Apply combined preset (Quiet, Balanced, Gaming) coordinating power profile and fan mode.
    pub fn apply_preset(&self, preset: &str) -> Result<PresetResult, String> {
        #[cfg(target_os = "windows")]
        {
            self.windows_client
                .apply_combined_preset(preset)
                .map_err(|e| {
                    tracing::error!(
                        function = "HardwareServiceClient::apply_preset",
                        preset,
                        error = %e,
                        "ApplyPreset failed"
                    );
                    e.to_string()
                })
        }
        #[cfg(target_os = "linux")]
        {
            let conn = self.connect_service()?;
            let reply = conn
                .call_method(
                    Some(self.destination.as_str()),
                    self.path.as_str(),
                    Some(self.interface.as_str()),
                    "ApplyPreset",
                    &(preset,),
                )
                .map_err(|e| self.format_service_error("ApplyPreset", &e))?;

            let raw_json: String = reply
                .body()
                .deserialize()
                .map_err(|e| format!("Failed to parse ApplyPreset response: {e}"))?;

            let result: PresetResult = serde_json::from_str(&raw_json)
                .map_err(|e| format!("Invalid PresetResult JSON from service: {e}"))?;

            Ok(result)
        }
    }

    /// Helper to ensure service connection or produce actionable offline guidance.
    #[cfg(target_os = "linux")]
    fn connect_service(&self) -> Result<Connection, String> {
        if !self.status().is_connected() {
            return Err("Hardware control daemon (zenomen-hw) is not running on system D-Bus. Start it with 'sudo systemctl start zenomen-hw' or install with 'sudo zenomen --install-system-integration'.".to_string());
        }
        self.connect()
    }

    /// Format D-Bus errors with user-actionable explanations when service is missing.
    #[cfg(target_os = "linux")]
    fn format_service_error(&self, op: &str, e: &zbus::Error) -> String {
        let err_str = e.to_string();
        if err_str.contains("ServiceUnknown")
            || err_str.contains("not provided by any .service files")
        {
            "Hardware control daemon (zenomen-hw) is not running on system D-Bus. Start it with 'sudo systemctl start zenomen-hw' or install with 'sudo zenomen --install-system-integration'.".to_string()
        } else {
            format!("{op} failed: {err_str}")
        }
    }

    /// Check authorization readiness without user prompting (DIAG-04).
    pub fn check_polkit_status(&self) -> Result<String, String> {
        #[cfg(target_os = "windows")]
        {
            if self.windows_client.is_service_available() {
                Ok("Authorized (Console Session / Named Pipe)".to_string())
            } else {
                Ok("Service offline (ZenOmenHardwareService)".to_string())
            }
        }
        #[cfg(target_os = "linux")]
        {
            let conn = self.connect()?;
            let unique_name = conn
                .unique_name()
                .map(|n| n.as_str().to_string())
                .unwrap_or_default();

            let mut subject_details = std::collections::HashMap::new();
            subject_details.insert("name", zbus::zvariant::Value::from(unique_name.as_str()));
            let subject = ("system-bus-name", subject_details);
            let action_details: std::collections::HashMap<&str, &str> =
                std::collections::HashMap::new();
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
                std::collections::HashMap<String, String>,
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
    }

    /// Apply an RGB profile to hardware.
    pub fn apply_rgb_profile(&self, profile: &crate::rgb::RgbProfile) -> Result<String, String> {
        #[cfg(target_os = "windows")]
        {
            self.windows_client.apply_rgb_profile(profile).map_err(|e| {
                tracing::error!(
                    function = "HardwareServiceClient::apply_rgb_profile",
                    profile = %profile.name,
                    error = %e,
                    "ApplyRgbProfile failed"
                );
                e.to_string()
            })
        }
        #[cfg(target_os = "linux")]
        {
            let conn = self.connect_service()?;
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
                .map_err(|e| self.format_service_error("ApplyRgbProfile", &e))?;

            let msg: String = reply
                .body()
                .deserialize()
                .map_err(|e| format!("Failed to parse ApplyRgbProfile response: {e}"))?;

            Ok(msg)
        }
    }

    /// Set RGB keyboard power state (RGB-04).
    pub fn set_rgb_enabled(&self, enabled: bool) -> Result<String, String> {
        #[cfg(target_os = "windows")]
        {
            self.windows_client
                .set_rgb_enabled(enabled)
                .map(|b| {
                    if b {
                        "RGB enabled".to_string()
                    } else {
                        "RGB disabled".to_string()
                    }
                })
                .map_err(|e| {
                    tracing::error!(
                        function = "HardwareServiceClient::set_rgb_enabled",
                        enabled,
                        error = %e,
                        "SetRgbEnabled failed"
                    );
                    e.to_string()
                })
        }
        #[cfg(target_os = "linux")]
        {
            let conn = self.connect_service()?;
            let reply = conn
                .call_method(
                    Some(self.destination.as_str()),
                    self.path.as_str(),
                    Some(self.interface.as_str()),
                    "SetRgbEnabled",
                    &(enabled,),
                )
                .map_err(|e| self.format_service_error("SetRgbEnabled", &e))?;

            let msg: String = reply
                .body()
                .deserialize()
                .map_err(|e| format!("Failed to parse SetRgbEnabled response: {e}"))?;

            Ok(msg)
        }
    }
}
