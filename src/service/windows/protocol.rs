//! Named pipe message protocol and framing for Windows hardware control (WIN-03).

use serde::{Deserialize, Serialize};

use crate::hardware::{HardwareCapabilities, ProcessMemoryInfo};
use crate::service::types::{HardwareControlError, PowerProfileResult, PresetResult};

/// Maximum allowable payload size for named pipe frames (64 KB).
pub const MAX_PAYLOAD_SIZE: usize = 65536;

/// Privileged hardware requests sent by the unprivileged desktop GUI.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum HardwareRequest {
    GetCapabilities,
    GetFanStatus,
    SetFanMode { mode: String },
    GetPowerProfiles,
    GetPowerProfile,
    SetPowerProfile { profile: String },
    ApplyPreset { preset: String },
    ApplyRgbProfile { profile_json: String },
    SetRgbEnabled { enabled: bool },
    GetServiceMemory,
}

/// Typed responses sent by the privileged hardware service.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum HardwareResponse {
    Capabilities(HardwareCapabilities),
    FanStatus { mode: String, rpms: Vec<u32> },
    FanModeSet(String),
    PowerProfiles(Vec<String>),
    PowerProfile(String),
    PowerProfileSet(PowerProfileResult),
    PresetApplied(PresetResult),
    RgbProfileApplied(String),
    RgbEnabledSet(bool),
    ServiceMemory(ProcessMemoryInfo),
    Error(String),
}

/// Encode a message into a 4-byte big-endian length prefixed JSON frame.
pub fn encode_frame<T: Serialize>(msg: &T) -> Result<Vec<u8>, HardwareControlError> {
    let json = serde_json::to_vec(msg).map_err(|e| {
        HardwareControlError::ProtocolError(format!("JSON serialization error: {e}"))
    })?;

    if json.len() > MAX_PAYLOAD_SIZE {
        return Err(HardwareControlError::ProtocolError(format!(
            "Payload size {} bytes exceeds maximum allowed {} bytes",
            json.len(),
            MAX_PAYLOAD_SIZE
        )));
    }

    let mut frame = Vec::with_capacity(4 + json.len());
    frame.extend_from_slice(&(json.len() as u32).to_be_bytes());
    frame.extend_from_slice(&json);
    Ok(frame)
}

/// Decode a message from a 4-byte big-endian length prefixed frame.
pub fn decode_frame<'a, T: Deserialize<'a>>(
    buf: &'a [u8],
) -> Result<(T, usize), HardwareControlError> {
    if buf.len() < 4 {
        return Err(HardwareControlError::ProtocolError(
            "Buffer too short to read length prefix".to_string(),
        ));
    }

    let length = u32::from_be_bytes([buf[0], buf[1], buf[2], buf[3]]) as usize;
    if length > MAX_PAYLOAD_SIZE {
        return Err(HardwareControlError::ProtocolError(format!(
            "Frame size {length} bytes exceeds maximum allowed {MAX_PAYLOAD_SIZE} bytes"
        )));
    }

    let total_len = 4 + length;
    if buf.len() < total_len {
        return Err(HardwareControlError::ProtocolError(format!(
            "Incomplete frame: expected {total_len} bytes, got {}",
            buf.len()
        )));
    }

    let msg: T = serde_json::from_slice(&buf[4..total_len]).map_err(|e| {
        HardwareControlError::ProtocolError(format!("JSON deserialization error: {e}"))
    })?;

    Ok((msg, total_len))
}
