//! Desktop integration module covering single-instance enforcement,
//! StatusNotifierItem system tray, close-to-tray lifecycle, and XDG autostart.

pub mod autostart;
pub mod single_instance;
pub mod tray;

pub use autostart::{is_autostart_enabled, set_autostart_enabled};
pub use single_instance::{
    ensure_single_instance, ApplicationService, DesktopAction, SingleInstanceResult,
};
pub use tray::{register_tray, StatusNotifierItem, TrayDbusMenu};
