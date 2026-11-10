//! Desktop actions shared across Linux and Windows desktop integration.

/// Actions requested by remote instances, system tray menus, or desktop events.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DesktopAction {
    PresentWindow,
    Quit,
    SetFanModeAuto,
    SetFanModeMax,
}
