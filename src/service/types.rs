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

/// Lifecycle state of the fan control subsystem on HP hardware.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FanControlState {
    /// Automatic cooling controlled by dynamic firmware curve.
    Automatic,
    /// Actively applying a requested fan state.
    Applying,
    /// Manual fan level applied.
    Manual,
    /// Full fan speed active (Max Fan / 100%).
    MaxFan,
    /// Actively restoring automatic cooling control.
    Restoring,
    /// Restoration to automatic cooling failed (requires alert / manual intervention).
    RestoreFailed,
    /// Hardware or board profile does not support fan control.
    Unsupported,
}

impl FanControlState {
    /// User-facing normalized identifier string.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Automatic => "automatic",
            Self::Applying => "applying",
            Self::Manual => "manual",
            Self::MaxFan => "maxfan",
            Self::Restoring => "restoring",
            Self::RestoreFailed => "restore_failed",
            Self::Unsupported => "unsupported",
        }
    }

    /// Whether this state represents active cooling intervention by software.
    pub fn is_active_override(self) -> bool {
        matches!(self, Self::Manual | Self::MaxFan | Self::Applying)
    }
}

/// Granular result of applying a combined preset (PWR-06).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PresetResult {
    /// Name of the preset (e.g., "gaming", "balanced", "quiet", "auto", "performance").
    pub preset_name: String,
    /// Whether the power profile was successfully written and verified.
    pub power_applied: bool,
    /// Confirmed power profile read back from hardware, if applied.
    pub confirmed_power_profile: Option<String>,
    /// Whether the fan mode was successfully written and verified.
    pub fan_applied: bool,
    /// Confirmed fan mode read back from hardware, if applied.
    pub confirmed_fan_mode: Option<String>,
    /// Whether display refresh rate was successfully changed and verified.
    #[serde(default)]
    pub display_applied: bool,
    /// Confirmed display refresh rate (e.g. 60 or 144 Hz).
    #[serde(default)]
    pub confirmed_refresh_rate: Option<u32>,
    /// True if only one part of the preset succeeded while the other failed.
    pub partial_success: bool,
    /// Human-readable explanation of what was applied or what failed.
    pub message: String,
    /// Status of the power profile transition (e.g. ProfileVerified vs ProfileUnverified).
    #[serde(default)]
    pub power_profile_status: Option<PowerProfileStatus>,
}

impl PresetResult {
    /// Check if the preset was fully applied without errors.
    pub fn is_full_success(&self) -> bool {
        self.power_applied
            && self.fan_applied
            && !self.partial_success
            && (self.confirmed_refresh_rate.is_none() || self.display_applied)
    }
}

/// Typed status of a power/thermal profile change operation (PWR-04).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PowerProfileStatus {
    /// Firmware or WMI transport rejected the command.
    CommandRejected,
    /// Command accepted by firmware via WMI transport.
    CommandAccepted,
    /// Profile transition verified through independent hardware read-back.
    ProfileVerified,
    /// Command accepted by firmware, but independent read-back is unavailable on this board (e.g. 8A3D EC-backed).
    ProfileUnverified,
}

impl PowerProfileStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::CommandRejected => "command_rejected",
            Self::CommandAccepted => "command_accepted",
            Self::ProfileVerified => "profile_verified",
            Self::ProfileUnverified => "profile_unverified",
        }
    }

    pub fn is_accepted(self) -> bool {
        matches!(
            self,
            Self::CommandAccepted | Self::ProfileVerified | Self::ProfileUnverified
        )
    }

    pub fn is_readback_verified(self) -> bool {
        matches!(self, Self::ProfileVerified)
    }
}

/// Detailed result of setting a power profile.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PowerProfileResult {
    /// Name of the profile targeted (e.g., "balanced", "performance", "quiet").
    pub profile: String,
    /// Verification and firmware status.
    pub status: PowerProfileStatus,
    /// Independent read-back confirmation from hardware, if available.
    pub readback_profile: Option<String>,
    /// Human-readable explanation of operation outcome.
    pub message: String,
}

impl PowerProfileResult {
    pub fn verified(profile: impl Into<String>, readback: impl Into<String>) -> Self {
        let profile = profile.into();
        let readback = readback.into();
        Self {
            message: format!("Profile '{profile}' applied and verified via firmware read-back"),
            profile,
            status: PowerProfileStatus::ProfileVerified,
            readback_profile: Some(readback),
        }
    }

    pub fn unverified(profile: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            profile: profile.into(),
            status: PowerProfileStatus::ProfileUnverified,
            readback_profile: None,
            message: message.into(),
        }
    }
}

impl std::fmt::Display for PowerProfileResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.profile, self.message)
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

    /// Firmware-level authorization failure (e.g. BIOS password locked, EC rejected write).
    /// Distinct from `AccessDenied` (OS-level) and `PolkitDenied` (Polkit-level).
    #[error("Firmware authorization failed: {0}")]
    AuthorizationFailed(String),
}

#[cfg(target_os = "linux")]
impl From<HardwareControlError> for zbus::fdo::Error {
    fn from(err: HardwareControlError) -> Self {
        match err {
            HardwareControlError::PolkitDenied(msg)
            | HardwareControlError::AccessDenied(msg)
            | HardwareControlError::AuthorizationFailed(msg) => zbus::fdo::Error::AccessDenied(msg),
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
