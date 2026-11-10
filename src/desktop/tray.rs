//! Native freedesktop StatusNotifierItem (SNI) system tray implementation over `zbus`.
//!
//! Exposes `org.kde.StatusNotifierItem` and `com.canonical.dbusmenu` directly on
//! the user session bus, eliminating heavy toolkit dependencies (GTK, Qt, Tao)
//! and maintaining the strict <=40 MB peak RSS budget (SYS-01, SYS-10, PERF-01).

use std::collections::HashMap;
use std::sync::mpsc::SyncSender;
use zbus::blocking::Connection;
use zbus::zvariant::{ObjectPath, Value};

use crate::desktop::single_instance::DesktopAction;

pub const SNI_OBJECT_PATH: &str = "/StatusNotifierItem";
pub const MENU_OBJECT_PATH: &str = "/MenuBar";

/// StatusNotifierItem implementation.
pub struct StatusNotifierItem {
    action_sender: SyncSender<DesktopAction>,
}

impl StatusNotifierItem {
    pub fn new(action_sender: SyncSender<DesktopAction>) -> Self {
        Self { action_sender }
    }
}

#[zbus::interface(name = "org.kde.StatusNotifierItem")]
impl StatusNotifierItem {
    #[zbus(property)]
    pub fn category(&self) -> String {
        "ApplicationStatus".to_string()
    }

    #[zbus(property)]
    pub fn id(&self) -> String {
        "zenomen".to_string()
    }

    #[zbus(property)]
    pub fn title(&self) -> String {
        "ZenOmen".to_string()
    }

    #[zbus(property)]
    pub fn status(&self) -> String {
        "Active".to_string()
    }

    #[zbus(property)]
    pub fn window_id(&self) -> i32 {
        0
    }

    #[zbus(property)]
    pub fn icon_name(&self) -> String {
        "zenomen".to_string()
    }

    #[zbus(property)]
    pub fn overlay_icon_name(&self) -> String {
        String::new()
    }

    #[zbus(property)]
    pub fn attention_icon_name(&self) -> String {
        String::new()
    }

    #[zbus(property)]
    pub fn attention_movie_name(&self) -> String {
        String::new()
    }

    #[zbus(property)]
    pub fn menu(&self) -> ObjectPath<'_> {
        ObjectPath::from_str_unchecked(MENU_OBJECT_PATH)
    }

    #[zbus(property)]
    pub fn item_is_menu(&self) -> bool {
        false
    }

    /// User left-clicked or activated the tray item.
    pub fn activate(&self, _x: i32, _y: i32) -> zbus::fdo::Result<()> {
        let _ = self.action_sender.send(DesktopAction::PresentWindow);
        Ok(())
    }

    /// User secondary-activated (middle clicked).
    pub fn secondary_activate(&self, _x: i32, _y: i32) -> zbus::fdo::Result<()> {
        let _ = self.action_sender.send(DesktopAction::PresentWindow);
        Ok(())
    }

    /// User right-clicked (context menu request).
    pub fn context_menu(&self, _x: i32, _y: i32) -> zbus::fdo::Result<()> {
        let _ = self.action_sender.send(DesktopAction::PresentWindow);
        Ok(())
    }

    pub fn scroll(&self, _delta: i32, _orientation: String) -> zbus::fdo::Result<()> {
        Ok(())
    }
}

/// Lightweight DBusMenu service providing tray popup actions.
pub struct TrayDbusMenu {
    action_sender: SyncSender<DesktopAction>,
}

impl TrayDbusMenu {
    pub fn new(action_sender: SyncSender<DesktopAction>) -> Self {
        Self { action_sender }
    }
}

pub type DbusMenuItem<'a> = (i32, HashMap<String, Value<'a>>, Vec<Value<'a>>);
pub type DbusMenuLayout<'a> = (u32, DbusMenuItem<'a>);

#[zbus::interface(name = "com.canonical.dbusmenu")]
impl TrayDbusMenu {
    #[zbus(property)]
    pub fn version(&self) -> u32 {
        3
    }

    #[zbus(property)]
    pub fn status(&self) -> String {
        "normal".to_string()
    }

    /// Return menu structure.
    pub fn get_layout(
        &self,
        _parent_id: i32,
        _recursion_depth: i32,
        _property_names: Vec<String>,
    ) -> zbus::fdo::Result<DbusMenuLayout<'_>> {
        let mut root_props = HashMap::new();
        root_props.insert("children-display".to_string(), Value::from("submenu"));

        let mut item1_props = HashMap::new();
        item1_props.insert("label".to_string(), Value::from("Open ZenOmen"));

        let mut item2_props = HashMap::new();
        item2_props.insert("label".to_string(), Value::from("Quit ZenOmen"));

        let item1 = Value::from((1i32, item1_props, Vec::<Value<'_>>::new()));
        let item2 = Value::from((2i32, item2_props, Vec::<Value<'_>>::new()));

        let children = vec![item1, item2];
        Ok((1, (0, root_props, children)))
    }

    /// Trigger menu event on item click.
    pub fn event(
        &self,
        id: i32,
        event_id: String,
        _data: Value<'_>,
        _timestamp: u32,
    ) -> zbus::fdo::Result<()> {
        if event_id == "clicked" {
            match id {
                1 => {
                    let _ = self.action_sender.send(DesktopAction::PresentWindow);
                }
                2 => {
                    let _ = self.action_sender.send(DesktopAction::Quit);
                }
                _ => {}
            }
        }
        Ok(())
    }

    /// Pre-display hook queried by Ayatana/KDE/XFCE panel applets before opening menu.
    pub fn about_to_show(&self, _id: i32) -> zbus::fdo::Result<bool> {
        Ok(false)
    }
}

/// Register the StatusNotifierItem and DBusMenu on the session bus.
pub fn register_tray(
    conn: &Connection,
    action_sender: SyncSender<DesktopAction>,
) -> Result<(), String> {
    let sni = StatusNotifierItem::new(action_sender.clone());
    let menu = TrayDbusMenu::new(action_sender);

    conn.object_server()
        .at(SNI_OBJECT_PATH, sni)
        .map_err(|e| format!("Failed to register SNI object: {e}"))?;

    conn.object_server()
        .at(MENU_OBJECT_PATH, menu)
        .map_err(|e| format!("Failed to register DBusMenu object: {e}"))?;

    // Attempt registration with StatusNotifierWatcher (if desktop environment supports it)
    match conn.call_method(
        Some("org.kde.StatusNotifierWatcher"),
        "/StatusNotifierWatcher",
        Some("org.kde.StatusNotifierWatcher"),
        "RegisterStatusNotifierItem",
        &(SNI_OBJECT_PATH,),
    ) {
        Ok(_) => {
            tracing::info!("StatusNotifierItem successfully registered with desktop watcher");
        }
        Err(e) => {
            // Stock GNOME without AppIndicator extension does not run a StatusNotifierWatcher.
            // Per SYS-10, the app continues without errors; D-Bus single instance provides fallback.
            tracing::info!(reason = %e, "StatusNotifierWatcher not present; tray icon optional");
        }
    }

    Ok(())
}
