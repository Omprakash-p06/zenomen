//! Named pipe client for the unprivileged desktop GUI (WIN-03).

use crate::rgb::schema::RgbProfile;
use crate::service::client::ServiceConnectionStatus;
use crate::service::types::{HardwareControlError, PresetResult};
#[cfg(windows)]
use crate::service::windows::pipe::PIPE_NAME;
use crate::service::windows::protocol::{HardwareRequest, HardwareResponse};
#[cfg(windows)]
use crate::service::windows::protocol::{decode_frame, encode_frame};

/// Unprivileged client connecting to the privileged Windows hardware service via named pipe.
pub struct WindowsServiceClient;

impl WindowsServiceClient {
    pub fn new() -> Self {
        Self
    }

    /// Check if the privileged hardware service named pipe exists and is listening.
    pub fn is_service_available(&self) -> bool {
        #[cfg(windows)]
        {
            use windows_sys::Win32::Foundation::{
                ERROR_PIPE_BUSY, ERROR_SEM_TIMEOUT, GetLastError,
            };
            use windows_sys::Win32::System::Pipes::WaitNamedPipeW;

            let pipe_name_wide: Vec<u16> =
                PIPE_NAME.encode_utf16().chain(std::iter::once(0)).collect();

            unsafe {
                if WaitNamedPipeW(pipe_name_wide.as_ptr(), 0) != 0 {
                    true
                } else {
                    let err = GetLastError();
                    err == ERROR_PIPE_BUSY || err == ERROR_SEM_TIMEOUT
                }
            }
        }
        #[cfg(not(windows))]
        {
            false
        }
    }

    /// Query service connection status.
    pub fn get_service_status(&self) -> ServiceConnectionStatus {
        if self.is_service_available() {
            ServiceConnectionStatus::Connected
        } else {
            ServiceConnectionStatus::Unavailable(
                "Windows hardware service is offline. Start 'ZenOmen Hardware Service' in services.msc".to_string(),
            )
        }
    }

    /// Send a request and wait for a typed response over the named pipe.
    pub fn send_request(
        &self,
        req: HardwareRequest,
    ) -> Result<HardwareResponse, HardwareControlError> {
        #[cfg(windows)]
        {
            use windows_sys::Win32::Foundation::{
                CloseHandle, GENERIC_READ, GENERIC_WRITE, INVALID_HANDLE_VALUE,
            };
            use windows_sys::Win32::Storage::FileSystem::{
                CreateFileW, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING, ReadFile, WriteFile,
            };

            let pipe_name_wide: Vec<u16> =
                PIPE_NAME.encode_utf16().chain(std::iter::once(0)).collect();

            unsafe {
                let handle = CreateFileW(
                    pipe_name_wide.as_ptr(),
                    GENERIC_READ | GENERIC_WRITE,
                    FILE_SHARE_READ | FILE_SHARE_WRITE,
                    std::ptr::null(),
                    OPEN_EXISTING,
                    0,
                    std::ptr::null_mut(),
                );

                if handle == INVALID_HANDLE_VALUE {
                    return Err(HardwareControlError::ConnectionFailed(
                        "Unable to connect to ZenOmen hardware service named pipe".to_string(),
                    ));
                }

                let tx_bytes = encode_frame(&req)?;
                let mut bytes_written = 0u32;
                let write_res = WriteFile(
                    handle,
                    tx_bytes.as_ptr(),
                    tx_bytes.len() as u32,
                    &mut bytes_written,
                    std::ptr::null_mut(),
                );

                if write_res == 0 {
                    CloseHandle(handle);
                    return Err(HardwareControlError::IoError(
                        "Failed to write request to named pipe".to_string(),
                    ));
                }

                let mut rx_buf = vec![0u8; 65536];
                let mut bytes_read = 0u32;
                let read_res = ReadFile(
                    handle,
                    rx_buf.as_mut_ptr(),
                    rx_buf.len() as u32,
                    &mut bytes_read,
                    std::ptr::null_mut(),
                );

                CloseHandle(handle);

                if read_res == 0 || bytes_read < 4 {
                    return Err(HardwareControlError::IoError(
                        "Failed to read response from named pipe".to_string(),
                    ));
                }

                let (resp, _len): (HardwareResponse, usize) =
                    decode_frame(&rx_buf[..bytes_read as usize])?;
                Ok(resp)
            }
        }
        #[cfg(not(windows))]
        {
            let _ = req;
            Err(HardwareControlError::ConnectionFailed(
                "Windows named pipe is unavailable on this platform".to_string(),
            ))
        }
    }

    /// Set fan mode ("auto" or "max").
    pub fn set_fan_mode(&self, mode: &str) -> Result<String, HardwareControlError> {
        match self.send_request(HardwareRequest::SetFanMode {
            mode: mode.to_string(),
        })? {
            HardwareResponse::FanModeSet(m) => Ok(m),
            HardwareResponse::Error(e) => Err(HardwareControlError::IoError(e)),
            _ => Err(HardwareControlError::ProtocolError(
                "Unexpected response for SetFanMode".to_string(),
            )),
        }
    }

    /// Set ACPI platform performance profile.
    pub fn set_power_profile(&self, profile: &str) -> Result<String, HardwareControlError> {
        match self.send_request(HardwareRequest::SetPowerProfile {
            profile: profile.to_string(),
        })? {
            HardwareResponse::PowerProfileSet(p) => Ok(p),
            HardwareResponse::Error(e) => Err(HardwareControlError::IoError(e)),
            _ => Err(HardwareControlError::ProtocolError(
                "Unexpected response for SetPowerProfile".to_string(),
            )),
        }
    }

    /// Apply combined preset.
    pub fn apply_combined_preset(
        &self,
        preset: &str,
    ) -> Result<PresetResult, HardwareControlError> {
        match self.send_request(HardwareRequest::ApplyPreset {
            preset: preset.to_string(),
        })? {
            HardwareResponse::PresetApplied(result) => Ok(result),
            HardwareResponse::Error(e) => Err(HardwareControlError::IoError(e)),
            _ => Err(HardwareControlError::ProtocolError(
                "Unexpected response for ApplyPreset".to_string(),
            )),
        }
    }

    /// Apply keyboard RGB profile.
    pub fn apply_rgb_profile(&self, profile: &RgbProfile) -> Result<String, HardwareControlError> {
        let json = serde_json::to_string(profile)
            .map_err(|e| HardwareControlError::InvalidParameter(e.to_string()))?;
        match self.send_request(HardwareRequest::ApplyRgbProfile { profile_json: json })? {
            HardwareResponse::RgbProfileApplied(name) => Ok(name),
            HardwareResponse::Error(e) => Err(HardwareControlError::IoError(e)),
            _ => Err(HardwareControlError::ProtocolError(
                "Unexpected response for ApplyRgbProfile".to_string(),
            )),
        }
    }

    /// Toggle keyboard RGB lighting.
    pub fn set_rgb_enabled(&self, enabled: bool) -> Result<bool, HardwareControlError> {
        match self.send_request(HardwareRequest::SetRgbEnabled { enabled })? {
            HardwareResponse::RgbEnabledSet(en) => Ok(en),
            HardwareResponse::Error(e) => Err(HardwareControlError::IoError(e)),
            _ => Err(HardwareControlError::ProtocolError(
                "Unexpected response for SetRgbEnabled".to_string(),
            )),
        }
    }
}

impl Default for WindowsServiceClient {
    fn default() -> Self {
        Self::new()
    }
}
