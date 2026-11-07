//! Versioned, typed wire protocol shared by every privileged hardware service.
//!
//! The Linux daemon speaks this contract over D-Bus; the Windows service speaks
//! the same contract over a local named pipe. Nothing in this module may depend
//! on zbus, Win32, sysfs or any other platform API — it is the seam that keeps
//! `ARCH-01` true and lets both backends be tested against one fixture set.
//!
//! Framing is deliberately explicit: a 4-byte little-endian length prefix,
//! followed by a UTF-8 JSON body, capped at [`MAX_FRAME_BYTES`]. A service that
//! accepted an unbounded frame could be made to allocate without limit by a
//! local caller (`WIN-08`).

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::hardware::HardwareCapabilities;
use crate::rgb::RgbProfile;
use crate::service::types::{FanMode, HardwareControlError, PresetResult};

/// Wire version. Bump only for an incompatible change; the service rejects any
/// other value rather than guessing at a payload shape.
pub const PROTOCOL_VERSION: u16 = 1;

/// Hard cap on a single serialized frame (64 KiB).
pub const MAX_FRAME_BYTES: usize = 64 * 1024;

/// Operations a client may request. Every variant is a fixed, typed operation:
/// there is no "run this command", no arbitrary path, and no raw WMI payload.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ServiceRequest {
    /// Which hardware interfaces the machine actually exposes.
    GetCapabilities,
    /// Current fan mode plus one tachometer reading per detected fan.
    GetFanStatus,
    /// Request a validated fan mode (`Auto` or `Max` in v1).
    SetFanMode(FanMode),
    /// ACPI platform profiles the machine advertises.
    GetPowerProfiles,
    /// Currently active platform profile.
    GetPowerProfile,
    /// Apply one advertised platform profile.
    SetPowerProfile(String),
    /// Apply a combined fan + power preset.
    ApplyPreset(String),
    /// Apply a four-zone RGB profile.
    ApplyRgbProfile(RgbProfile),
    /// Enable or disable keyboard lighting, restoring the last profile on enable.
    SetRgbEnabled(bool),
}

/// Successful payloads. Each response variant is the typed counterpart of one
/// request; a client never has to parse a free-form string to know what it got.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ServiceResponse {
    Capabilities(HardwareCapabilities),
    FanStatus {
        mode: String,
        rpms: Vec<u32>,
    },
    PowerProfiles(Vec<String>),
    PowerProfile(String),
    Preset(PresetResult),
    /// Confirmation text for a mutation that was verified by read-back.
    Confirmed(String),
    /// Acknowledged request with no payload.
    Ok,
}

/// Typed failure surface. Raw driver/COM errors are mapped into one of these so
/// the UI can explain the failure without leaking platform detail.
#[derive(Debug, Error, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ServiceError {
    #[error("hardware service is not available: {reason}")]
    Unavailable { reason: String },

    #[error("not authorized: {reason}")]
    Unauthorized { reason: String },

    #[error("invalid request: {reason}")]
    InvalidRequest { reason: String },

    #[error("verification failed for '{target}': expected '{expected}', observed '{observed}'")]
    VerificationFailed {
        target: String,
        expected: String,
        observed: String,
    },

    #[error("unsupported: {reason}")]
    Unsupported { reason: String },

    #[error("operation '{operation}' timed out")]
    Timeout { operation: String },

    #[error("internal service error: {reason}")]
    Internal { reason: String },
}

impl ServiceError {
    /// Build the operator-facing hint shown when the privileged service is down.
    ///
    /// The text is platform-neutral on purpose; each client supplies the exact
    /// recovery command (systemd on Linux, `sc start` on Windows).
    pub fn unavailable_with_hint(reason: impl Into<String>, hint: &str) -> Self {
        Self::Unavailable {
            reason: format!("{} ({hint})", reason.into()),
        }
    }
}

impl From<HardwareControlError> for ServiceError {
    fn from(err: HardwareControlError) -> Self {
        match err {
            HardwareControlError::PolkitDenied(reason)
            | HardwareControlError::AccessDenied(reason) => Self::Unauthorized { reason },
            HardwareControlError::InvalidParameter(reason)
            | HardwareControlError::UnsupportedMode(reason)
            | HardwareControlError::ProtocolError(reason) => Self::InvalidRequest { reason },
            HardwareControlError::InterfaceMissing(reason)
            | HardwareControlError::ConnectionFailed(reason) => Self::Unavailable { reason },
            HardwareControlError::VerificationFailed {
                target,
                actual,
                retries,
            } => Self::VerificationFailed {
                target,
                expected: "requested value".to_string(),
                observed: format!("{actual} after {retries} retries"),
            },
            HardwareControlError::IoError(reason) => Self::Internal { reason },
        }
    }
}

/// Request wrapper carrying the version and a correlation id.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestEnvelope {
    pub protocol_version: u16,
    pub request_id: u64,
    pub request: ServiceRequest,
}

/// Response wrapper echoing the correlation id of the request it answers.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResponseEnvelope {
    pub protocol_version: u16,
    pub request_id: u64,
    pub result: Result<ServiceResponse, ServiceError>,
}

impl RequestEnvelope {
    /// Wrap a request with the current protocol version and a caller-chosen id.
    pub fn new(request_id: u64, request: ServiceRequest) -> Self {
        Self {
            protocol_version: PROTOCOL_VERSION,
            request_id,
            request,
        }
    }

    /// Reject a frame the service cannot safely interpret.
    pub fn validate(&self) -> Result<(), ProtocolError> {
        if self.protocol_version != PROTOCOL_VERSION {
            return Err(ProtocolError::VersionMismatch {
                found: self.protocol_version,
                expected: PROTOCOL_VERSION,
            });
        }
        if self.request_id == 0 {
            return Err(ProtocolError::Malformed(
                "request_id must be non-zero".to_string(),
            ));
        }
        Ok(())
    }
}

impl ResponseEnvelope {
    /// Wrap a result with the protocol version and the answered request id.
    pub fn new(request_id: u64, result: Result<ServiceResponse, ServiceError>) -> Self {
        Self {
            protocol_version: PROTOCOL_VERSION,
            request_id,
            result,
        }
    }
}

/// Framing / versioning failures. These never contain a secret and are safe to
/// log verbatim.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ProtocolError {
    #[error("frame exceeds {max} bytes (got {actual})")]
    FrameTooLarge { max: usize, actual: usize },

    #[error("truncated frame: expected {expected} bytes, got {actual}")]
    Truncated { expected: usize, actual: usize },

    #[error("protocol version {found} is not supported (expected {expected})")]
    VersionMismatch { found: u16, expected: u16 },

    #[error("malformed payload: {0}")]
    Malformed(String),
}

/// Serialize a value into a length-prefixed frame.
pub fn encode_frame<T: Serialize>(value: &T) -> Result<Vec<u8>, ProtocolError> {
    let body = serde_json::to_vec(value).map_err(|e| ProtocolError::Malformed(e.to_string()))?;
    if body.len() > MAX_FRAME_BYTES {
        return Err(ProtocolError::FrameTooLarge {
            max: MAX_FRAME_BYTES,
            actual: body.len(),
        });
    }
    let mut frame = Vec::with_capacity(4 + body.len());
    frame.extend_from_slice(&(body.len() as u32).to_le_bytes());
    frame.extend_from_slice(&body);
    Ok(frame)
}

/// Parse a complete length-prefixed frame.
pub fn decode_frame<T: for<'de> Deserialize<'de>>(frame: &[u8]) -> Result<T, ProtocolError> {
    if frame.len() < 4 {
        return Err(ProtocolError::Truncated {
            expected: 4,
            actual: frame.len(),
        });
    }
    let declared = u32::from_le_bytes([frame[0], frame[1], frame[2], frame[3]]) as usize;
    if declared > MAX_FRAME_BYTES {
        return Err(ProtocolError::FrameTooLarge {
            max: MAX_FRAME_BYTES,
            actual: declared,
        });
    }
    let body = &frame[4..];
    if body.len() != declared {
        return Err(ProtocolError::Truncated {
            expected: declared,
            actual: body.len(),
        });
    }
    serde_json::from_slice(body).map_err(|e| ProtocolError::Malformed(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rgb::RgbColor;

    fn sample_profile() -> RgbProfile {
        RgbProfile {
            id: "test".to_string(),
            name: "Test".to_string(),
            description: String::new(),
            zones: [RgbColor::new(255, 0, 0); 4],
            unsupported_fields: Vec::new(),
        }
    }

    #[test]
    fn request_round_trips_through_a_frame() {
        let envelope = RequestEnvelope::new(7, ServiceRequest::SetFanMode(FanMode::Max));
        envelope.validate().expect("valid request");
        let frame = encode_frame(&envelope).expect("encode");
        let decoded: RequestEnvelope = decode_frame(&frame).expect("decode");
        assert_eq!(decoded.request_id, 7);
        assert!(matches!(
            decoded.request,
            ServiceRequest::SetFanMode(FanMode::Max)
        ));
    }

    #[test]
    fn rgb_profile_round_trips_typed() {
        let envelope = RequestEnvelope::new(9, ServiceRequest::ApplyRgbProfile(sample_profile()));
        let frame = encode_frame(&envelope).expect("encode");
        let decoded: RequestEnvelope = decode_frame(&frame).expect("decode");
        match decoded.request {
            ServiceRequest::ApplyRgbProfile(profile) => {
                assert_eq!(profile.zones.len(), 4);
                assert_eq!(profile.id, "test");
            }
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn response_carries_typed_errors() {
        let envelope = ResponseEnvelope::new(
            3,
            Err(ServiceError::VerificationFailed {
                target: "pwm1_enable".to_string(),
                expected: "2".to_string(),
                observed: "0".to_string(),
            }),
        );
        let frame = encode_frame(&envelope).expect("encode");
        let decoded: ResponseEnvelope = decode_frame(&frame).expect("decode");
        assert!(matches!(
            decoded.result,
            Err(ServiceError::VerificationFailed { .. })
        ));
    }

    #[test]
    fn oversized_frame_is_rejected_before_allocation() {
        let declared = (MAX_FRAME_BYTES + 1) as u32;
        let mut frame = declared.to_le_bytes().to_vec();
        frame.extend_from_slice(b"{}");
        match decode_frame::<RequestEnvelope>(&frame) {
            Err(ProtocolError::FrameTooLarge { actual, .. }) => {
                assert_eq!(actual, MAX_FRAME_BYTES + 1);
            }
            other => panic!("expected FrameTooLarge, got {other:?}"),
        }
    }

    #[test]
    fn truncated_and_version_mismatched_frames_are_rejected() {
        assert!(matches!(
            decode_frame::<RequestEnvelope>(&[0, 0]),
            Err(ProtocolError::Truncated { .. })
        ));

        let mut envelope = RequestEnvelope::new(1, ServiceRequest::GetCapabilities);
        envelope.protocol_version = PROTOCOL_VERSION + 1;
        assert!(matches!(
            envelope.validate(),
            Err(ProtocolError::VersionMismatch { .. })
        ));
    }

    #[test]
    fn zero_request_id_is_rejected() {
        let envelope = RequestEnvelope::new(0, ServiceRequest::GetCapabilities);
        assert!(matches!(
            envelope.validate(),
            Err(ProtocolError::Malformed(_))
        ));
    }
}
