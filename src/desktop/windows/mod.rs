//! Windows desktop integration module.
//!
//! Provides native single-instance mutex enforcement, Shell_NotifyIconW system tray,
//! HKCU Run registry autostart, ShellExecuteW releases browser launch, and known folder paths.

pub mod autostart;
pub mod installer;
pub mod notifications;
pub mod paths;
pub mod releases;
pub mod single_instance;
pub mod tray;

pub use autostart::{format_autostart_command, is_autostart_enabled, set_autostart_enabled};
pub use installer::{install_hardware_service, uninstall_hardware_service};
pub use notifications::show_notification;
pub use paths::{AppDataPaths, resolve_app_data_paths, resolve_app_data_paths_from};
pub use releases::open_releases_page;
pub use single_instance::{
    MutexHandle, SingleInstanceResult, acquire_single_instance, present_existing_window,
};
pub use tray::{TrayHandle, register_tray_icon, remove_tray_icon};
