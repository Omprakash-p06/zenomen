//! Desktop integration module covering single-instance enforcement,
//! StatusNotifierItem system tray, close-to-tray lifecycle, and XDG autostart.

pub mod autostart;
pub mod releases;
pub mod single_instance;
pub mod system_integration;
pub mod tray;

pub use autostart::{is_autostart_enabled, set_autostart_enabled};
pub use releases::{RELEASES_URL, ReleasesError, open_releases_page};
pub use single_instance::{
    ApplicationService, DesktopAction, SingleInstanceResult, ensure_single_instance,
};
pub use system_integration::{
    SystemIntegrationError, install as install_system_integration,
    uninstall as uninstall_system_integration,
};
pub use tray::{StatusNotifierItem, TrayDbusMenu, register_tray};
