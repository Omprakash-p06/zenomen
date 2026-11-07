//! Named pipe DACL security and interactive session verification (WIN-03).

use crate::service::types::HardwareControlError;

#[cfg(windows)]
use windows_sys::Win32::Foundation::HANDLE;

/// Verify that the connecting named-pipe client belongs to the active interactive console session (WIN-03).
///
/// Immediately rejects callers connecting across remote desktop sessions,
/// SSH terminals, or background secondary service accounts.
pub fn verify_client_session(
    #[allow(unused_variables)] pipe: *mut std::ffi::c_void,
) -> Result<(), HardwareControlError> {
    #[cfg(windows)]
    {
        use windows_sys::Win32::Foundation::{CloseHandle, GetLastError};
        use windows_sys::Win32::Security::{
            GetTokenInformation, RevertToSelf, TOKEN_QUERY, TokenSessionId,
        };
        use windows_sys::Win32::System::Pipes::ImpersonateNamedPipeClient;
        use windows_sys::Win32::System::RemoteDesktop::WTSGetActiveConsoleSessionId;
        use windows_sys::Win32::System::Threading::{GetCurrentThread, OpenThreadToken};

        unsafe {
            if ImpersonateNamedPipeClient(pipe as HANDLE) == 0 {
                return Err(HardwareControlError::AccessDenied(format!(
                    "Failed to impersonate named pipe client (error {})",
                    GetLastError()
                )));
            }

            let mut thread_token: HANDLE = std::ptr::null_mut();
            let open_res = OpenThreadToken(GetCurrentThread(), TOKEN_QUERY, 1, &mut thread_token);

            if open_res == 0 {
                RevertToSelf();
                return Err(HardwareControlError::AccessDenied(
                    "Unable to open caller thread token for session verification".to_string(),
                ));
            }

            let mut session_id = 0u32;
            let mut returned_len = 0u32;
            let query_res = GetTokenInformation(
                thread_token,
                TokenSessionId,
                &mut session_id as *mut _ as *mut std::ffi::c_void,
                std::mem::size_of::<u32>() as u32,
                &mut returned_len,
            );

            CloseHandle(thread_token);
            RevertToSelf();

            if query_res == 0 {
                return Err(HardwareControlError::AccessDenied(
                    "Failed to query TokenSessionId from client token".to_string(),
                ));
            }

            let active_console_session = WTSGetActiveConsoleSessionId();
            if session_id != active_console_session {
                return Err(HardwareControlError::AccessDenied(format!(
                    "Caller session ID {session_id} does not match active console session {active_console_session}"
                )));
            }

            Ok(())
        }
    }
    #[cfg(not(windows))]
    {
        Ok(())
    }
}
