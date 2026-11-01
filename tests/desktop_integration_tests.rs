//! Integration tests for desktop integration, single-instance D-Bus service,
//! StatusNotifierItem system tray, XDG autostart, and application icon assets.

use std::fs;
use std::path::PathBuf;
use std::sync::mpsc;
use zbus::zvariant::Value;
use zenomen::desktop::autostart::{
    is_autostart_enabled_at, set_autostart_enabled_at, AUTOSTART_DESKTOP_ENTRY,
};
use zenomen::desktop::single_instance::{ApplicationService, DesktopAction};
use zenomen::desktop::tray::{StatusNotifierItem, TrayDbusMenu};

#[test]
fn test_autostart_lifecycle_with_custom_path() {
    let temp_dir = std::env::temp_dir().join(format!("zenomen_test_autostart_{}", std::process::id()));
    let desktop_path = temp_dir.join("zenomen.desktop");

    // Initially should be disabled
    assert!(!is_autostart_enabled_at(&desktop_path));

    // Enable autostart
    set_autostart_enabled_at(&desktop_path, true).expect("enabling autostart must succeed");
    assert!(desktop_path.is_file());
    assert!(is_autostart_enabled_at(&desktop_path));

    // Verify file content matches XDG standards
    let content = fs::read_to_string(&desktop_path).expect("reading autostart file");
    assert!(content.contains("[Desktop Entry]"));
    assert!(content.contains("Name=ZenOmen"));
    assert!(content.contains("Exec=zenomen --minimized"));
    assert!(content.contains("Icon=zenomen"));
    assert!(content.contains("Categories=Utility;HardwareSettings;"));

    // Disable autostart
    set_autostart_enabled_at(&desktop_path, false).expect("disabling autostart must succeed");
    assert!(!desktop_path.exists());
    assert!(!is_autostart_enabled_at(&desktop_path));

    // Cleanup
    let _ = fs::remove_dir_all(temp_dir);
}

#[test]
fn test_autostart_disabled_when_hidden_flag_present() {
    let temp_dir = std::env::temp_dir().join(format!("zenomen_test_hidden_{}", std::process::id()));
    let desktop_path = temp_dir.join("zenomen.desktop");
    fs::create_dir_all(&temp_dir).unwrap();

    let content_with_hidden = format!("{AUTOSTART_DESKTOP_ENTRY}Hidden=true\n");
    fs::write(&desktop_path, content_with_hidden).unwrap();

    assert!(!is_autostart_enabled_at(&desktop_path));

    let _ = fs::remove_dir_all(temp_dir);
}

#[test]
fn test_single_instance_application_service_dispatch() {
    let (action_tx, action_rx) = mpsc::sync_channel(5);
    let service = ApplicationService::new(action_tx);

    // Call present_window via D-Bus interface simulation
    assert!(service.present_window().is_ok());
    assert_eq!(
        action_rx.try_recv().unwrap(),
        DesktopAction::PresentWindow
    );

    // Call quit via D-Bus interface simulation
    assert!(service.quit().is_ok());
    assert_eq!(action_rx.try_recv().unwrap(), DesktopAction::Quit);

    // Call activate via D-Bus interface simulation
    assert!(service.activate().is_ok());
    assert_eq!(
        action_rx.try_recv().unwrap(),
        DesktopAction::PresentWindow
    );
}

#[test]
fn test_status_notifier_item_properties_and_events() {
    let (action_tx, action_rx) = mpsc::sync_channel(5);
    let sni = StatusNotifierItem::new(action_tx.clone());

    // Properties check
    assert_eq!(sni.category(), "ApplicationStatus");
    assert_eq!(sni.id(), "zenomen");
    assert_eq!(sni.title(), "ZenOmen");
    assert_eq!(sni.status(), "Active");
    assert_eq!(sni.icon_name(), "zenomen");
    assert_eq!(sni.window_id(), 0);
    assert_eq!(sni.menu().as_str(), "/MenuBar");
    assert!(!sni.item_is_menu());

    // Activation methods check
    assert!(sni.activate(100, 200).is_ok());
    assert_eq!(action_rx.try_recv().unwrap(), DesktopAction::PresentWindow);

    assert!(sni.secondary_activate(100, 200).is_ok());
    assert_eq!(action_rx.try_recv().unwrap(), DesktopAction::PresentWindow);

    assert!(sni.context_menu(100, 200).is_ok());
    assert_eq!(action_rx.try_recv().unwrap(), DesktopAction::PresentWindow);
}

#[test]
fn test_tray_dbusmenu_layout_and_events() {
    let (action_tx, action_rx) = mpsc::sync_channel(5);
    let menu = TrayDbusMenu::new(action_tx);

    assert_eq!(menu.version(), 3);
    assert_eq!(menu.status(), "normal");

    let layout_res = menu.get_layout(0, -1, vec![]);
    assert!(layout_res.is_ok());
    let (revision, layout) = layout_res.unwrap();
    assert_eq!(revision, 1);
    assert_eq!(layout.0, 0); // Root ID
    assert_eq!(layout.2.len(), 2); // 2 children: Open and Quit

    // Click Open (id 1)
    assert!(menu.event(1, "clicked".to_string(), Value::from(0), 0).is_ok());
    assert_eq!(action_rx.try_recv().unwrap(), DesktopAction::PresentWindow);

    // Click Quit (id 2)
    assert!(menu.event(2, "clicked".to_string(), Value::from(0), 0).is_ok());
    assert_eq!(action_rx.try_recv().unwrap(), DesktopAction::Quit);

    // Pre-show hook check (AboutToShow)
    assert!(!menu.about_to_show(0).unwrap());
}

#[test]
fn test_icon_assets_exist_and_are_valid() {
    // 1. Scalable SVG
    let svg_path = PathBuf::from("data/icons/zenomen.svg");
    assert!(svg_path.is_file(), "data/icons/zenomen.svg must exist");
    let svg_content = fs::read_to_string(&svg_path).unwrap();
    assert!(svg_content.contains("<svg"));
    assert!(svg_content.contains("ZenOmen logo"));
    // Ensure flame was removed
    assert!(!svg_content.contains("id=\"flame\""));
    assert!(!svg_content.contains("id=\"flameP\""));

    // 2. Standard PNG icons
    let sizes = [16, 32, 48, 64, 128, 256, 512];
    for s in sizes {
        let p = PathBuf::from(format!("data/icons/hicolor/{s}x{s}/apps/zenomen.png"));
        assert!(p.is_file(), "Icon file {p:?} must exist");
        assert!(fs::metadata(&p).unwrap().len() > 0);
    }

    // 3. Multi-resolution .ico for Windows .exe
    let ico_path = PathBuf::from("data/icons/zenomen.ico");
    assert!(ico_path.is_file(), "data/icons/zenomen.ico must exist");
    assert!(fs::metadata(&ico_path).unwrap().len() > 1000);
}
