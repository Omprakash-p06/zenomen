//! Windows hardware service installation and removal hooks (SYS-01).

use crate::desktop::DesktopError;

/// Register the zenomen-hw privileged Windows service in the Service Control Manager.
pub fn install_hardware_service() -> Result<(), DesktopError> {
    #[cfg(target_os = "windows")]
    {
        crate::service::windows::lifecycle::register_service()
            .map_err(|e| DesktopError::Other(format!("Failed to register service in SCM: {e}")))
    }
    #[cfg(not(target_os = "windows"))]
    {
        Ok(())
    }
}

/// Stop and remove the zenomen-hw privileged Windows service from the Service Control Manager.
pub fn uninstall_hardware_service() -> Result<(), DesktopError> {
    #[cfg(target_os = "windows")]
    {
        crate::service::windows::lifecycle::uninstall_service()
            .map_err(|e| DesktopError::Other(format!("Failed to uninstall service from SCM: {e}")))
    }
    #[cfg(not(target_os = "windows"))]
    {
        Ok(())
    }
}
