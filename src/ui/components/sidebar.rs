//! Navigation sidebar component definition for GPUI-CE.

use crate::core::events::{AppCommand, ScreenId};

#[derive(Debug, Clone, PartialEq)]
pub struct SidebarItem {
    pub screen: ScreenId,
    pub title: &'static str,
    pub icon_name: &'static str,
    pub is_active: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SidebarView {
    pub active_screen: ScreenId,
    pub app_version: String,
    pub items: Vec<SidebarItem>,
}

impl SidebarView {
    pub fn new(active_screen: ScreenId, app_version: impl Into<String>) -> Self {
        let items = vec![
            SidebarItem {
                screen: ScreenId::Overview,
                title: "Overview",
                icon_name: "dashboard",
                is_active: active_screen == ScreenId::Overview,
            },
            SidebarItem {
                screen: ScreenId::Cooling,
                title: "Cooling & Fans",
                icon_name: "fan",
                is_active: active_screen == ScreenId::Cooling,
            },
            SidebarItem {
                screen: ScreenId::Performance,
                title: "Performance",
                icon_name: "speed",
                is_active: active_screen == ScreenId::Performance,
            },
            SidebarItem {
                screen: ScreenId::Lighting,
                title: "RGB Lighting",
                icon_name: "palette",
                is_active: active_screen == ScreenId::Lighting,
            },
            SidebarItem {
                screen: ScreenId::Settings,
                title: "Settings",
                icon_name: "settings",
                is_active: active_screen == ScreenId::Settings,
            },
            SidebarItem {
                screen: ScreenId::Diagnostics,
                title: "Diagnostics",
                icon_name: "build",
                is_active: active_screen == ScreenId::Diagnostics,
            },
        ];

        Self {
            active_screen,
            app_version: app_version.into(),
            items,
        }
    }

    pub fn handle_select(&mut self, screen: ScreenId) -> AppCommand {
        self.active_screen = screen;
        for item in &mut self.items {
            item.is_active = item.screen == screen;
        }
        AppCommand::Navigate(screen)
    }
}
