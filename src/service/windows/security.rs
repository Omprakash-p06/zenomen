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
        use windows_sys::Win32::System::Pipes::{
            GetNamedPipeClientSessionId, ImpersonateNamedPipeClient,
        };
        use windows_sys::Win32::System::RemoteDesktop::WTSGetActiveConsoleSessionId;
        use windows_sys::Win32::System::Threading::{GetCurrentThread, OpenThreadToken};

        unsafe {
            let mut pipe_session_id = 0u32;
            if GetNamedPipeClientSessionId(pipe as HANDLE, &mut pipe_session_id) == 0 {
                return Err(HardwareControlError::AccessDenied(format!(
                    "Failed to query named pipe client session ID (error {})",
                    GetLastError()
                )));
            }

            let active_console_session = WTSGetActiveConsoleSessionId();
            if pipe_session_id != active_console_session {
                return Err(HardwareControlError::AccessDenied(format!(
                    "Caller pipe session ID {pipe_session_id} does not match active console session {active_console_session}"
                )));
            }

            // In elevated SCM service mode (LocalSystem), SeImpersonatePrivilege is held,
            // allowing deep token inspection as defense-in-depth.
            // In unprivileged console testing (--console), ImpersonateNamedPipeClient may fail
            // with error 1368 (ERROR_CANNOT_IMPERSONATE) or 1314 (ERROR_PRIVILEGE_NOT_HELD);
            // in that case, the GetNamedPipeClientSessionId verification above provides enforcement.
            if ImpersonateNamedPipeClient(pipe as HANDLE) != 0 {
                let mut thread_token: HANDLE = std::ptr::null_mut();
                let open_res =
                    OpenThreadToken(GetCurrentThread(), TOKEN_QUERY, 1, &mut thread_token);

                if open_res == 0 {
                    let err = GetLastError();
                    if RevertToSelf() == 0 {
                        tracing::error!(
                            "FATAL: RevertToSelf failed after OpenThreadToken failure (error {})",
                            GetLastError()
                        );
                        std::process::exit(1);
                    }
                    return Err(HardwareControlError::AccessDenied(format!(
                        "Unable to open caller thread token for session verification (error {err})"
                    )));
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
                if RevertToSelf() == 0 {
                    tracing::error!(
                        "FATAL: RevertToSelf failed after querying token (error {})",
                        GetLastError()
                    );
                    std::process::exit(1);
                }

                if query_res == 0 {
                    return Err(HardwareControlError::AccessDenied(
                        "Failed to query TokenSessionId from client token".to_string(),
                    ));
                }

                if session_id != active_console_session {
                    return Err(HardwareControlError::AccessDenied(format!(
                        "Caller token session ID {session_id} does not match active console session {active_console_session}"
                    )));
                }
            } else {
                let imp_err = GetLastError();
                tracing::debug!(
                    error_code = imp_err,
                    "ImpersonateNamedPipeClient skipped in console/dev mode; session validated via GetNamedPipeClientSessionId"
                );
            }

            Ok(())
        }
    }
    #[cfg(not(windows))]
    {
        Ok(())
    }
}

/// Canonical Discretionary Access Control List (DACL) SDDL string for ZenOmen named pipe:
/// - (A;;GA;;;SY) : Generic All for NT AUTHORITY\SYSTEM
/// - (A;;GA;;;BA) : Generic All for BUILTIN\Administrators
/// - (A;;GRGW;;;IU) : Generic Read + Write for NT AUTHORITY\Interactive Users
pub const PIPE_SDDL: &str = "D:(A;;GA;;;SY)(A;;GA;;;BA)(A;;GRGW;;;IU)";

/// Create a SECURITY_ATTRIBUTES struct with explicit DACL restricting named pipe access.
#[cfg(windows)]
pub fn create_pipe_security_descriptor() -> Option<*mut std::ffi::c_void> {
    use windows_sys::Win32::Security::Authorization::{
        ConvertStringSecurityDescriptorToSecurityDescriptorW, SDDL_REVISION_1,
    };
    use windows_sys::Win32::Security::PSECURITY_DESCRIPTOR;

    let sddl_wide: Vec<u16> = PIPE_SDDL.encode_utf16().chain(std::iter::once(0)).collect();
    let mut p_sd: PSECURITY_DESCRIPTOR = std::ptr::null_mut();

    let success = unsafe {
        ConvertStringSecurityDescriptorToSecurityDescriptorW(
            sddl_wide.as_ptr(),
            SDDL_REVISION_1,
            &mut p_sd,
            std::ptr::null_mut(),
        )
    };

    if success != 0 && !p_sd.is_null() {
        Some(p_sd)
    } else {
        None
    }
}
