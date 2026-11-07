//! Core types, enums, and errors for the verified hardware control subsystem.

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Fan modes supported by the HP WMI / ACPI interface.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FanMode {
    /// BIOS / firmware manages fan speed automatically according to thermal curve.
    Auto,
    /// Maximum cooling fan speed (disables automatic fan control, spinning at peak RPM).
    Max,
    /// Manual control level (not offered as a target choice in v1).
    Manual(u8),
    /// Unknown / raw value returned by kernel.
    Unknown(String),
}

impl FanMode {
    /// Parse kernel sysfs value (typically from `pwm1_enable`).
    /// In hp-wmi:
    /// - 0 = Max / Manual override to 100%
    /// - 1 = Manual (direct PWM)
    /// - 2 = Auto (BIOS default)
    pub fn from_sysfs(val: &str) -> Self {
        match val.trim() {
            "0" => Self::Max,
            "1" => Self::Manual(1),
            "2" => Self::Auto,
            other => {
                if let Ok(num) = other.parse::<u8>() {
                    Self::Manual(num)
                } else {
                    Self::Unknown(other.to_string())
                }
            }
        }
    }

    /// Sysfs string representation to write to `pwm1_enable`.
    /// Returns `None` if the mode cannot be directly written or is not validated.
    pub fn as_sysfs_value(&self) -> Option<&'static str> {
        match self {
            Self::Auto => Some("2"),
            Self::Max => Some("0"),
            Self::Manual(_) | Self::Unknown(_) => None,
        }
    }

    /// User-facing normalized identifier string.
    pub fn as_str(&self) -> &str {
        match self {
            Self::Auto => "auto",
            Self::Max => "max",
            Self::Manual(_) => "manual",
            Self::Unknown(s) => s.as_str(),
        }
    }

    /// Parse a requested mode string from client/user.
    /// Strictly allows "auto" and "max" in v1.
    /// Rejects "manual" with an explicit `UnsupportedMode` error.
    pub fn parse_requested(s: &str) -> Result<Self, HardwareControlError> {
        let normalized = s.trim().to_lowercase();
        match normalized.as_str() {
            "auto" => Ok(Self::Auto),
            "max" => Ok(Self::Max),
            "manual" => Err(HardwareControlError::UnsupportedMode(
                "Manual fan mode without custom curves is not supported in v1".to_string(),
            )),
            other => Err(HardwareControlError::InvalidParameter(format!(
                "Invalid fan mode '{other}'. Supported modes are 'auto' and 'max'"
            ))),
        }
    }
}

/// Granular result of applying a combined preset (PWR-06).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PresetResult {
    /// Name of the preset (e.g., "gaming", "balanced", "quiet").
    pub preset_name: String,
    /// Whether the power profile was successfully written and verified.
    pub power_applied: bool,
    /// Confirmed power profile read back from hardware, if applied.
    pub confirmed_power_profile: Option<String>,
    /// Whether the fan mode was successfully written and verified.
    pub fan_applied: bool,
    /// Confirmed fan mode read back from hardware, if applied.
    pub confirmed_fan_mode: Option<String>,
    /// True if only one part of the preset succeeded while the other failed.
    pub partial_success: bool,
    /// Human-readable explanation of what was applied or what failed.
    pub message: String,
}

impl PresetResult {
    /// Check if the preset was fully applied without errors.
    pub fn is_full_success(&self) -> bool {
        self.power_applied && self.fan_applied && !self.partial_success
    }
}

/// Typed errors for hardware control and service operations.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum HardwareControlError {
    #[error("I/O error: {0}")]
    IoError(String),

    #[error(
        "Hardware write verification failed for target '{target}': read-back was '{actual}' after {retries} retries"
    )]
    VerificationFailed {
        target: String,
        actual: String,
        retries: usize,
    },

    #[error("Unsupported mode: {0}")]
    UnsupportedMode(String),

    #[error("Polkit authorization denied: {0}")]
    PolkitDenied(String),

    #[error("Invalid parameter: {0}")]
    InvalidParameter(String),

    #[error("Hardware interface missing: {0}")]
    InterfaceMissing(String),

    #[error("Connection failed: {0}")]
    ConnectionFailed(String),

    #[error("Protocol error: {0}")]
    ProtocolError(String),

    #[error("Access denied: {0}")]
    AccessDenied(String),
}

impl From<HardwareControlError> for zbus::fdo::Error {
    fn from(err: HardwareControlError) -> Self {
        match err {
            HardwareControlError::PolkitDenied(msg) | HardwareControlError::AccessDenied(msg) => {
                zbus::fdo::Error::AccessDenied(msg)
            }
            HardwareControlError::InvalidParameter(msg)
            | HardwareControlError::ProtocolError(msg) => zbus::fdo::Error::InvalidArgs(msg),
            HardwareControlError::UnsupportedMode(msg) => zbus::fdo::Error::NotSupported(msg),
            HardwareControlError::InterfaceMissing(msg) => zbus::fdo::Error::FileNotFound(msg),
            HardwareControlError::VerificationFailed { .. }
            | HardwareControlError::IoError(_)
            | HardwareControlError::ConnectionFailed(_) => {
                zbus::fdo::Error::Failed(err.to_string())
            }
        }
    }
}
