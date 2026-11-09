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
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
                CloseHandle, ERROR_PIPE_BUSY, GENERIC_READ, GENERIC_WRITE, GetLastError,
                INVALID_HANDLE_VALUE,
            };
            use windows_sys::Win32::Storage::FileSystem::{
                CreateFileW, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING, ReadFile,
                SECURITY_IMPERSONATION, SECURITY_SQOS_PRESENT, WriteFile,
            };
            use windows_sys::Win32::System::Pipes::WaitNamedPipeW;

            let pipe_name_wide: Vec<u16> =
                PIPE_NAME.encode_utf16().chain(std::iter::once(0)).collect();

            unsafe {
                let mut handle = INVALID_HANDLE_VALUE;
                for _ in 0..5 {
                    handle = CreateFileW(
                        pipe_name_wide.as_ptr(),
                        GENERIC_READ | GENERIC_WRITE,
                        FILE_SHARE_READ | FILE_SHARE_WRITE,
                        std::ptr::null(),
                        OPEN_EXISTING,
                        SECURITY_SQOS_PRESENT | SECURITY_IMPERSONATION,
                        std::ptr::null_mut(),
                    );

                    if handle != INVALID_HANDLE_VALUE {
                        break;
                    }

                    let err = GetLastError();
                    if err == ERROR_PIPE_BUSY
                        || err == windows_sys::Win32::Foundation::ERROR_FILE_NOT_FOUND
                    {
                        WaitNamedPipeW(pipe_name_wide.as_ptr(), 500);
                        continue;
                    }
                    break;
                }

                if handle == INVALID_HANDLE_VALUE {
                    let err = GetLastError();
                    let os_msg = std::io::Error::from_raw_os_error(err as i32);
                    tracing::error!(
                        function = "WindowsServiceClient::send_request",
                        pipe = PIPE_NAME,
                        error_code = err,
                        error = %os_msg,
                        "CreateFileW failed to connect to hardware service named pipe (ensure zenomen-hw is running)"
                    );
                    return Err(HardwareControlError::ConnectionFailed(format!(
                        "Unable to connect to ZenOmen hardware service named pipe: {os_msg} (error code {err}). Is 'zenomen-hw' running?"
                    )));
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
                    let err = GetLastError();
                    let os_msg = std::io::Error::from_raw_os_error(err as i32);
                    CloseHandle(handle);
                    tracing::error!(
                        function = "WindowsServiceClient::send_request",
                        pipe = PIPE_NAME,
                        error_code = err,
                        error = %os_msg,
                        "WriteFile failed to write request to named pipe"
                    );
                    return Err(HardwareControlError::IoError(format!(
                        "WriteFile failed on named pipe: {os_msg} (error code {err})"
                    )));
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
                    let err = GetLastError();
                    let os_msg = std::io::Error::from_raw_os_error(err as i32);
                    tracing::error!(
                        function = "WindowsServiceClient::send_request",
                        pipe = PIPE_NAME,
                        bytes_read,
                        error_code = err,
                        error = %os_msg,
                        "ReadFile failed or incomplete response from named pipe"
                    );
                    return Err(HardwareControlError::IoError(format!(
                        "ReadFile failed on named pipe: {os_msg} (bytes read: {bytes_read}, error code {err})"
                    )));
                }

                let (resp, _len): (HardwareResponse, usize) =
                    decode_frame(&rx_buf[..bytes_read as usize]).map_err(|e| {
                        tracing::error!(
                            function = "WindowsServiceClient::send_request",
                            pipe = PIPE_NAME,
                            bytes_read,
                            error = %e,
                            "Failed to decode response frame from named pipe"
                        );
                        e
                    })?;
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
        tracing::info!(
            function = "WindowsServiceClient::set_fan_mode",
            mode,
            "requesting fan mode change"
        );
        match self.send_request(HardwareRequest::SetFanMode {
            mode: mode.to_string(),
        }) {
            Ok(HardwareResponse::FanModeSet(m)) => {
                tracing::info!(
                    function = "WindowsServiceClient::set_fan_mode",
                    mode = %m,
                    "fan mode updated successfully"
                );
                Ok(m)
            }
            Ok(HardwareResponse::Error(e)) => {
                tracing::error!(
                    function = "WindowsServiceClient::set_fan_mode",
                    mode,
                    error = %e,
                    "service returned error response for SetFanMode"
                );
                Err(HardwareControlError::IoError(e))
            }
            Ok(other) => {
                tracing::error!(
                    function = "WindowsServiceClient::set_fan_mode",
                    mode,
                    response = ?other,
                    "unexpected response for SetFanMode"
                );
                Err(HardwareControlError::ProtocolError(format!(
                    "Unexpected response for SetFanMode: {other:?}"
                )))
            }
            Err(e) => {
                tracing::error!(
                    function = "WindowsServiceClient::set_fan_mode",
                    mode,
                    error = %e,
                    "send_request failed for SetFanMode"
                );
                Err(e)
            }
        }
    }

    /// Set ACPI platform performance profile.
    pub fn set_power_profile(&self, profile: &str) -> Result<String, HardwareControlError> {
        tracing::info!(
            function = "WindowsServiceClient::set_power_profile",
            profile,
            "requesting power profile change"
        );
        match self.send_request(HardwareRequest::SetPowerProfile {
            profile: profile.to_string(),
        }) {
            Ok(HardwareResponse::PowerProfileSet(p)) => {
                tracing::info!(
                    function = "WindowsServiceClient::set_power_profile",
                    profile = %p,
                    "power profile updated successfully"
                );
                Ok(p)
            }
            Ok(HardwareResponse::Error(e)) => {
                tracing::error!(
                    function = "WindowsServiceClient::set_power_profile",
                    profile,
                    error = %e,
                    "service returned error response for SetPowerProfile"
                );
                Err(HardwareControlError::IoError(e))
            }
            Ok(other) => {
                tracing::error!(
                    function = "WindowsServiceClient::set_power_profile",
                    profile,
                    response = ?other,
                    "unexpected response for SetPowerProfile"
                );
                Err(HardwareControlError::ProtocolError(format!(
                    "Unexpected response for SetPowerProfile: {other:?}"
                )))
            }
            Err(e) => {
                tracing::error!(
                    function = "WindowsServiceClient::set_power_profile",
                    profile,
                    error = %e,
                    "send_request failed for SetPowerProfile"
                );
                Err(e)
            }
        }
    }

    /// Apply combined preset.
    pub fn apply_combined_preset(
        &self,
        preset: &str,
    ) -> Result<PresetResult, HardwareControlError> {
        tracing::info!(
            function = "WindowsServiceClient::apply_combined_preset",
            preset,
            "requesting preset application"
        );
        match self.send_request(HardwareRequest::ApplyPreset {
            preset: preset.to_string(),
        }) {
            Ok(HardwareResponse::PresetApplied(result)) => {
                tracing::info!(
                    function = "WindowsServiceClient::apply_combined_preset",
                    preset,
                    success = result.is_full_success(),
                    partial = result.partial_success,
                    message = %result.message,
                    "preset applied"
                );
                Ok(result)
            }
            Ok(HardwareResponse::Error(e)) => {
                tracing::error!(
                    function = "WindowsServiceClient::apply_combined_preset",
                    preset,
                    error = %e,
                    "service returned error response for ApplyPreset"
                );
                Err(HardwareControlError::IoError(e))
            }
            Ok(other) => {
                tracing::error!(
                    function = "WindowsServiceClient::apply_combined_preset",
                    preset,
                    response = ?other,
                    "unexpected response for ApplyPreset"
                );
                Err(HardwareControlError::ProtocolError(format!(
                    "Unexpected response for ApplyPreset: {other:?}"
                )))
            }
            Err(e) => {
                tracing::error!(
                    function = "WindowsServiceClient::apply_combined_preset",
                    preset,
                    error = %e,
                    "send_request failed for ApplyPreset"
                );
                Err(e)
            }
        }
    }

    /// Apply keyboard RGB profile.
    pub fn apply_rgb_profile(&self, profile: &RgbProfile) -> Result<String, HardwareControlError> {
        let json = serde_json::to_string(profile)
            .map_err(|e| HardwareControlError::InvalidParameter(e.to_string()))?;
        tracing::info!(
            function = "WindowsServiceClient::apply_rgb_profile",
            profile = %profile.name,
            "requesting RGB profile application"
        );
        match self.send_request(HardwareRequest::ApplyRgbProfile { profile_json: json }) {
            Ok(HardwareResponse::RgbProfileApplied(name)) => {
                tracing::info!(
                    function = "WindowsServiceClient::apply_rgb_profile",
                    profile = %name,
                    "RGB profile applied successfully"
                );
                Ok(name)
            }
            Ok(HardwareResponse::Error(e)) => {
                tracing::error!(
                    function = "WindowsServiceClient::apply_rgb_profile",
                    profile = %profile.name,
                    error = %e,
                    "service returned error response for ApplyRgbProfile"
                );
                Err(HardwareControlError::IoError(e))
            }
            Ok(other) => {
                tracing::error!(
                    function = "WindowsServiceClient::apply_rgb_profile",
                    profile = %profile.name,
                    response = ?other,
                    "unexpected response for ApplyRgbProfile"
                );
                Err(HardwareControlError::ProtocolError(format!(
                    "Unexpected response for ApplyRgbProfile: {other:?}"
                )))
            }
            Err(e) => {
                tracing::error!(
                    function = "WindowsServiceClient::apply_rgb_profile",
                    profile = %profile.name,
                    error = %e,
                    "send_request failed for ApplyRgbProfile"
                );
                Err(e)
            }
        }
    }

    /// Toggle keyboard RGB lighting.
    pub fn set_rgb_enabled(&self, enabled: bool) -> Result<bool, HardwareControlError> {
        tracing::info!(
            function = "WindowsServiceClient::set_rgb_enabled",
            enabled,
            "requesting RGB enable toggle"
        );
        match self.send_request(HardwareRequest::SetRgbEnabled { enabled }) {
            Ok(HardwareResponse::RgbEnabledSet(en)) => {
                tracing::info!(
                    function = "WindowsServiceClient::set_rgb_enabled",
                    enabled = en,
                    "RGB enabled toggled successfully"
                );
                Ok(en)
            }
            Ok(HardwareResponse::Error(e)) => {
                tracing::error!(
                    function = "WindowsServiceClient::set_rgb_enabled",
                    enabled,
                    error = %e,
                    "service returned error response for SetRgbEnabled"
                );
                Err(HardwareControlError::IoError(e))
            }
            Ok(other) => {
                tracing::error!(
                    function = "WindowsServiceClient::set_rgb_enabled",
                    enabled,
                    response = ?other,
                    "unexpected response for SetRgbEnabled"
                );
                Err(HardwareControlError::ProtocolError(format!(
                    "Unexpected response for SetRgbEnabled: {other:?}"
                )))
            }
            Err(e) => {
                tracing::error!(
                    function = "WindowsServiceClient::set_rgb_enabled",
                    enabled,
                    error = %e,
                    "send_request failed for SetRgbEnabled"
                );
                Err(e)
            }
        }
    }
}

impl Default for WindowsServiceClient {
    fn default() -> Self {
        Self::new()
    }
}
