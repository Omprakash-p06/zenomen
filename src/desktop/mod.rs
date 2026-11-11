//! Desktop integration module covering single-instance enforcement,
//! StatusNotifierItem / Win32 system tray, close-to-tray lifecycle, and autostart.

pub mod actions;
pub mod autostart;
pub mod releases;
#[cfg(target_os = "linux")]
pub mod single_instance;
pub mod system_integration;
#[cfg(target_os = "linux")]
pub mod tray;
pub mod windows;

pub use actions::DesktopAction;
pub use releases::{RELEASES_URL, ReleasesError, open_releases_page};
pub use system_integration::{
    SystemIntegrationError, install as install_system_integration, request_service_elevation,
    uninstall as uninstall_system_integration,
};

#[cfg(target_os = "linux")]
pub use single_instance::{ApplicationService, SingleInstanceResult, ensure_single_instance};
#[cfg(target_os = "linux")]
pub use tray::{StatusNotifierItem, TrayDbusMenu, register_tray};

#[cfg(target_os = "windows")]
pub use windows::{
    AppDataPaths, MutexHandle, TrayHandle, acquire_single_instance, format_autostart_command,
    install_hardware_service, is_autostart_enabled as is_windows_autostart_enabled,
    open_releases_page as open_windows_releases_page, present_existing_window, register_tray_icon,
    remove_tray_icon, resolve_app_data_paths, resolve_app_data_paths_from,
    set_autostart_enabled as set_windows_autostart_enabled, show_notification,
};

/// Check if login autostart is enabled on the current operating system.
pub fn is_autostart_enabled() -> bool {
    #[cfg(target_os = "windows")]
    {
        windows::is_autostart_enabled()
    }
    #[cfg(not(target_os = "windows"))]
    {
        autostart::is_autostart_enabled()
    }
}

/// Enable or disable login autostart on the current operating system.
pub fn set_autostart_enabled(enabled: bool) -> std::io::Result<()> {
    #[cfg(target_os = "windows")]
    {
        windows::set_autostart_enabled(enabled).map_err(|e| std::io::Error::other(e.to_string()))
    }
    #[cfg(not(target_os = "windows"))]
    {
        autostart::set_autostart_enabled(enabled)
    }
}

/// Errors encountered in desktop integration operations.
#[derive(Debug, thiserror::Error)]
pub enum DesktopError {
    #[error("Win32 error ({code}): {message}")]
    Win32 { code: u32, message: String },
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Registry error: {0}")]
    Registry(String),
    #[error("Notification error: {0}")]
    Notification(String),
    #[error("Desktop error: {0}")]
    Other(String),
}
