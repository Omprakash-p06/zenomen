//! Desktop integration module covering single-instance enforcement,
//! StatusNotifierItem system tray, close-to-tray lifecycle, and XDG autostart.

pub mod autostart;
pub mod single_instance;
pub mod system_integration;
pub mod tray;

pub use autostart::{is_autostart_enabled, set_autostart_enabled};
pub use single_instance::{
    ensure_single_instance, ApplicationService, DesktopAction, SingleInstanceResult,
};
pub use system_integration::{
    install as install_system_integration, uninstall as uninstall_system_integration,
    SystemIntegrationError,
};
pub use tray::{register_tray, StatusNotifierItem, TrayDbusMenu};
