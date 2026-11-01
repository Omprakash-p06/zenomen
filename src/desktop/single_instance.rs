//! Single-instance application management via session D-Bus.
//!
//! Enforces single-instance execution (SYS-05). When a user launches a second
//! instance of ZenOmen, it invokes `PresentWindow` on the primary instance
//! over session D-Bus, bringing the existing window to the front and exiting cleanly.

use std::sync::mpsc::SyncSender;
use zbus::blocking::Connection;

pub const APP_BUS_NAME: &str = "com.zenomen.App";
pub const APP_OBJECT_PATH: &str = "/com/zenomen/App";
pub const APP_INTERFACE: &str = "com.zenomen.App";

/// Actions requested by remote instances or desktop events.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DesktopAction {
    PresentWindow,
    Quit,
}

/// D-Bus interface exposed by the primary application instance.
pub struct ApplicationService {
    action_sender: SyncSender<DesktopAction>,
}

impl ApplicationService {
    pub fn new(action_sender: SyncSender<DesktopAction>) -> Self {
        Self { action_sender }
    }
}

#[zbus::interface(name = "com.zenomen.App")]
impl ApplicationService {
    /// Focus and un-minimize the existing window.
    pub fn present_window(&self) -> zbus::fdo::Result<()> {
        tracing::info!("received PresentWindow RPC from secondary instance");
        let _ = self.action_sender.send(DesktopAction::PresentWindow);
        Ok(())
    }

    /// Cleanly quit the running application.
    pub fn quit(&self) -> zbus::fdo::Result<()> {
        tracing::info!("received Quit RPC from desktop environment");
        let _ = self.action_sender.send(DesktopAction::Quit);
        Ok(())
    }
}

/// Result of checking whether this process is the primary instance.
pub enum SingleInstanceResult {
    /// Successfully acquired `com.zenomen.App` on session bus.
    Primary(Connection),
    /// Another instance is already running; `PresentWindow` was invoked.
    AlreadyRunning,
    /// Session D-Bus is unavailable or failed; proceeding standalone.
    Disabled(String),
}

/// Ensure only one instance of ZenOmen runs at a time.
pub fn ensure_single_instance(action_sender: SyncSender<DesktopAction>) -> SingleInstanceResult {
    let conn = match Connection::session() {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!(error = %e, "could not connect to session D-Bus; single-instance disabled");
            return SingleInstanceResult::Disabled(e.to_string());
        }
    };

    // RequestName flags: DBUS_NAME_FLAG_DO_NOT_QUEUE = 4
    let reply: Result<u32, _> = conn
        .call_method(
            Some("org.freedesktop.DBus"),
            "/org/freedesktop/DBus",
            Some("org.freedesktop.DBus"),
            "RequestName",
            &(APP_BUS_NAME, 4u32),
        )
        .and_then(|r| r.body().deserialize());

    match reply {
        Ok(1) | Ok(4) => {
            // 1 = PRIMARY_OWNER, 4 = ALREADY_OWNER
            let service = ApplicationService::new(action_sender);
            if let Err(e) = conn.object_server().at(APP_OBJECT_PATH, service) {
                tracing::warn!(error = %e, "failed to register ApplicationService on session bus");
            }
            SingleInstanceResult::Primary(conn)
        }
        Ok(2) | Ok(3) => {
            // 2 = IN_QUEUE, 3 = EXISTS -> primary owner already exists!
            tracing::info!("another ZenOmen instance is already running; focusing existing window");
            let _ = conn.call_method(
                Some(APP_BUS_NAME),
                APP_OBJECT_PATH,
                Some(APP_INTERFACE),
                "PresentWindow",
                &(),
            );
            SingleInstanceResult::AlreadyRunning
        }
        Err(e) => {
            tracing::warn!(error = %e, "error requesting com.zenomen.App name; proceeding as primary");
            SingleInstanceResult::Disabled(e.to_string())
        }
        _ => SingleInstanceResult::AlreadyRunning,
    }
}
