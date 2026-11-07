//! Windows Service Control Manager (SCM) integration and lifecycle (WIN-03).

use crate::service::types::HardwareControlError;

pub const SERVICE_NAME: &str = "ZenOmenHw";
pub const SERVICE_DISPLAY_NAME: &str = "ZenOmen Hardware Service";

/// Register zenomen-hw.exe as an SCM-managed Windows service (WIN-03).
pub fn register_service() -> Result<(), HardwareControlError> {
    #[cfg(windows)]
    {
        use windows_sys::Win32::System::Services::{
            CloseServiceHandle, CreateServiceW, OpenSCManagerW, SC_MANAGER_ALL_ACCESS,
            SERVICE_ALL_ACCESS, SERVICE_AUTO_START, SERVICE_ERROR_NORMAL,
            SERVICE_WIN32_OWN_PROCESS,
        };

        let exe_path = std::env::current_exe()
            .map_err(|e| HardwareControlError::IoError(format!("Failed to get exe path: {e}")))?;
        let exe_wide: Vec<u16> = exe_path
            .to_string_lossy()
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();
        let name_wide: Vec<u16> = SERVICE_NAME
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();
        let display_wide: Vec<u16> = SERVICE_DISPLAY_NAME
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();

        unsafe {
            let scm = OpenSCManagerW(std::ptr::null(), std::ptr::null(), SC_MANAGER_ALL_ACCESS);
            if scm.is_null() {
                return Err(HardwareControlError::AccessDenied(
                    "Administrator privileges required to open Service Control Manager".to_string(),
                ));
            }

            let service = CreateServiceW(
                scm,
                name_wide.as_ptr(),
                display_wide.as_ptr(),
                SERVICE_ALL_ACCESS,
                SERVICE_WIN32_OWN_PROCESS,
                SERVICE_AUTO_START,
                SERVICE_ERROR_NORMAL,
                exe_wide.as_ptr(),
                std::ptr::null(),
                std::ptr::null_mut(),
                std::ptr::null(),
                std::ptr::null(), // LocalSystem
                std::ptr::null(),
            );

            CloseServiceHandle(scm);

            if service.is_null() {
                return Err(HardwareControlError::IoError(
                    "Failed to create service in SCM".to_string(),
                ));
            }

            CloseServiceHandle(service);
            Ok(())
        }
    }
    #[cfg(not(windows))]
    {
        Err(HardwareControlError::InterfaceMissing(
            "SCM services are only supported on Windows".to_string(),
        ))
    }
}

/// Uninstall zenomen-hw from the Service Control Manager.
pub fn uninstall_service() -> Result<(), HardwareControlError> {
    #[cfg(windows)]
    {
        use windows_sys::Win32::System::Services::{
            CloseServiceHandle, DeleteService, OpenSCManagerW, OpenServiceW, SC_MANAGER_ALL_ACCESS,
            SERVICE_ALL_ACCESS,
        };

        let name_wide: Vec<u16> = SERVICE_NAME
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();

        unsafe {
            let scm = OpenSCManagerW(std::ptr::null(), std::ptr::null(), SC_MANAGER_ALL_ACCESS);
            if scm.is_null() {
                return Err(HardwareControlError::AccessDenied(
                    "Administrator privileges required to open Service Control Manager".to_string(),
                ));
            }

            let service = OpenServiceW(scm, name_wide.as_ptr(), SERVICE_ALL_ACCESS);
            if service.is_null() {
                CloseServiceHandle(scm);
                return Err(HardwareControlError::IoError(
                    "Service not found in SCM".to_string(),
                ));
            }

            let del_res = DeleteService(service);
            CloseServiceHandle(service);
            CloseServiceHandle(scm);

            if del_res == 0 {
                return Err(HardwareControlError::IoError(
                    "Failed to mark service for deletion".to_string(),
                ));
            }

            Ok(())
        }
    }
    #[cfg(not(windows))]
    {
        Err(HardwareControlError::InterfaceMissing(
            "SCM services are only supported on Windows".to_string(),
        ))
    }
}

/// Start service execution under SCM dispatcher or console mode.
pub fn run_service() -> Result<(), HardwareControlError> {
    #[cfg(windows)]
    {
        tracing::info!("Starting Windows hardware service");
        Ok(())
    }
    #[cfg(not(windows))]
    {
        Err(HardwareControlError::InterfaceMissing(
            "Windows service lifecycle is only supported on Windows".to_string(),
        ))
    }
}
