//! Integration tests for desktop integration, single-instance D-Bus service,
//! StatusNotifierItem system tray, XDG autostart, and application icon assets.

use std::fs;
use std::path::PathBuf;
#[cfg(target_os = "linux")]
use std::sync::mpsc;
#[cfg(target_os = "linux")]
use zbus::zvariant::Value;
#[cfg(target_os = "linux")]
use zenomen::desktop::DesktopAction;
use zenomen::desktop::autostart::{
    AUTOSTART_DESKTOP_ENTRY, is_autostart_enabled_at, set_autostart_enabled_at,
};
#[cfg(target_os = "linux")]
use zenomen::desktop::single_instance::ApplicationService;
#[cfg(target_os = "linux")]
use zenomen::desktop::tray::{StatusNotifierItem, TrayDbusMenu};

#[test]
fn test_autostart_lifecycle_with_custom_path() {
    let temp_dir =
        std::env::temp_dir().join(format!("zenomen_test_autostart_{}", std::process::id()));
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
    assert!(desktop_path.exists());
    assert!(!is_autostart_enabled_at(&desktop_path));

    let disabled_content =
        fs::read_to_string(&desktop_path).expect("reading disabled autostart file");
    assert!(disabled_content.contains("Hidden=true"));
    assert!(disabled_content.contains("X-GNOME-Autostart-enabled=false"));

    // Cleanup
    let _ = fs::remove_dir_all(temp_dir);
}

#[test]
fn test_autostart_toggle_preserves_custom_file_lines() {
    let temp_dir = std::env::temp_dir().join(format!(
        "zenomen_test_custom_autostart_{}",
        std::process::id()
    ));
    let desktop_path = temp_dir.join("zenomen.desktop");
    fs::create_dir_all(&temp_dir).unwrap();

    let custom_content = "[Desktop Entry]\nName=ZenOmen\nComment=Custom Comment\nX-Custom-Key=CustomValue\nExec=zenomen --minimized\n";
    fs::write(&desktop_path, custom_content).unwrap();

    // Disable
    set_autostart_enabled_at(&desktop_path, false).expect("disabling must succeed");
    assert!(!is_autostart_enabled_at(&desktop_path));
    let disabled = fs::read_to_string(&desktop_path).unwrap();
    assert!(disabled.contains("Comment=Custom Comment"));
    assert!(disabled.contains("X-Custom-Key=CustomValue"));
    assert!(disabled.contains("Hidden=true"));

    // Re-enable
    set_autostart_enabled_at(&desktop_path, true).expect("enabling must succeed");
    assert!(is_autostart_enabled_at(&desktop_path));
    let enabled = fs::read_to_string(&desktop_path).unwrap();
    assert!(enabled.contains("Comment=Custom Comment"));
    assert!(enabled.contains("X-Custom-Key=CustomValue"));
    assert!(enabled.contains("Hidden=false"));

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
#[cfg(target_os = "linux")]
fn test_single_instance_application_service_dispatch() {
    let (action_tx, action_rx) = mpsc::sync_channel(5);
    let service = ApplicationService::new(action_tx);

    // Call present_window via D-Bus interface simulation
    assert!(service.present_window().is_ok());
    assert_eq!(action_rx.try_recv().unwrap(), DesktopAction::PresentWindow);

    // Call quit via D-Bus interface simulation
    assert!(service.quit().is_ok());
    assert_eq!(action_rx.try_recv().unwrap(), DesktopAction::Quit);

    // Call activate via D-Bus interface simulation
    assert!(service.activate().is_ok());
    assert_eq!(action_rx.try_recv().unwrap(), DesktopAction::PresentWindow);
}

#[test]
#[cfg(target_os = "linux")]
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
#[cfg(target_os = "linux")]
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
    assert!(
        menu.event(1, "clicked".to_string(), Value::from(0), 0)
            .is_ok()
    );
    assert_eq!(action_rx.try_recv().unwrap(), DesktopAction::PresentWindow);

    // Click Quit (id 2)
    assert!(
        menu.event(2, "clicked".to_string(), Value::from(0), 0)
            .is_ok()
    );
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

#[test]
fn test_releases_url_format_and_safety() {
    use zenomen::desktop::RELEASES_URL;
    assert!(RELEASES_URL.starts_with("https://"));
    assert!(RELEASES_URL.ends_with("/releases/latest"));
    assert!(!RELEASES_URL.contains("/releases/download/"));
}

#[test]
fn test_open_releases_page_failure_when_launcher_missing() {
    use zenomen::desktop::releases::open_releases_page_with;

    let res = open_releases_page_with("nonexistent-xdg-open-command-for-testing");
    assert!(
        res.is_err(),
        "open_releases_page_with must return Err when launcher cannot be found"
    );
}

#[test]
fn test_windows_path_resolution_local_appdata() {
    use zenomen::desktop::windows::paths::resolve_app_data_paths_from;

    let paths = resolve_app_data_paths_from(
        Some(r"C:\Users\Tester\AppData\Local"),
        Some(r"C:\ProgramData"),
    );

    assert!(paths.local_app_data.ends_with("ZenOmen"));
    assert!(paths.program_data.ends_with("ZenOmen"));
    let local_str = paths.local_app_data.to_string_lossy();
    assert!(local_str.contains("Tester"));
    assert!(local_str.contains("AppData"));
    let prog_str = paths.program_data.to_string_lossy();
    assert!(prog_str.contains("ProgramData"));

    // Fallback when None passed
    let fallback = resolve_app_data_paths_from(None, None);
    assert!(fallback.local_app_data.ends_with("ZenOmen"));
    assert!(fallback.program_data.ends_with("ZenOmen"));
}

#[test]
fn test_windows_releases_url_formatting() {
    use zenomen::desktop::windows::releases::RELEASES_URL;
    assert_eq!(
        RELEASES_URL,
        "https://github.com/Omprakash-p06/zenomen/releases/latest"
    );
    assert!(RELEASES_URL.starts_with("https://github.com/"));
    assert!(!RELEASES_URL.contains("/download/"));
    assert!(!RELEASES_URL.contains(".exe"));
}

#[test]
fn test_windows_autostart_registry_key_escaping() {
    use std::path::Path;
    use zenomen::desktop::windows::autostart::{
        AUTOSTART_KEY, AUTOSTART_VALUE_NAME, format_autostart_command,
    };

    assert_eq!(
        AUTOSTART_KEY,
        r"Software\Microsoft\Windows\CurrentVersion\Run"
    );
    assert_eq!(AUTOSTART_VALUE_NAME, "ZenOmen");

    let exe_with_spaces = Path::new(r"C:\Program Files\ZenOmen\zenomen.exe");
    let cmd = format_autostart_command(exe_with_spaces);
    assert_eq!(cmd, r#""C:\Program Files\ZenOmen\zenomen.exe" --minimized"#);

    let simple_exe = Path::new(r"C:\ZenOmen\zenomen.exe");
    let cmd_simple = format_autostart_command(simple_exe);
    assert_eq!(cmd_simple, r#""C:\ZenOmen\zenomen.exe" --minimized"#);
}
