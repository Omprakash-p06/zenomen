//! XDG autostart management for ZenOmen.
//!
//! Manages `~/.config/autostart/zenomen.desktop` in accordance with the
//! freedesktop.org Desktop Application Autostart Specification.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// Get the default path for the user's XDG autostart directory desktop entry.
pub fn default_autostart_path() -> Option<PathBuf> {
    directories::BaseDirs::new()
        .map(|dirs| dirs.config_dir().join("autostart").join("zenomen.desktop"))
}

/// Standard content of `zenomen.desktop` for login autostart.
pub const AUTOSTART_DESKTOP_ENTRY: &str = r#"[Desktop Entry]
Type=Application
Name=ZenOmen
GenericName=Hardware Control Center
Comment=Lightweight hardware control center for HP OMEN and Victus laptops
Exec=zenomen --minimized
Icon=zenomen
Terminal=false
Categories=Utility;HardwareSettings;
StartupNotify=false
X-GNOME-Autostart-enabled=true
"#;

/// Check if autostart is enabled at the default XDG autostart location.
pub fn is_autostart_enabled() -> bool {
    default_autostart_path()
        .map(|p| is_autostart_enabled_at(&p))
        .unwrap_or(false)
}

/// Check if autostart is enabled at a specific file path.
pub fn is_autostart_enabled_at(path: &Path) -> bool {
    if !path.is_file() {
        return false;
    }

    match fs::read_to_string(path) {
        Ok(content) => {
            for line in content.lines() {
                let trimmed = line.trim();
                if let Some((key, val)) = trimmed.split_once('=') {
                    let key = key.trim();
                    let val = val.trim();
                    if key.eq_ignore_ascii_case("Hidden") && val.eq_ignore_ascii_case("true") {
                        return false;
                    }
                    if key.eq_ignore_ascii_case("X-GNOME-Autostart-enabled")
                        && val.eq_ignore_ascii_case("false")
                    {
                        return false;
                    }
                }
            }
            true
        }
        Err(_) => false,
    }
}

/// Enable or disable login autostart at the default XDG location.
pub fn set_autostart_enabled(enabled: bool) -> io::Result<()> {
    let path = default_autostart_path().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::NotFound,
            "Could not determine user config directory",
        )
    })?;
    set_autostart_enabled_at(&path, enabled)
}

/// Enable or disable login autostart at a specific path (useful for testing).
pub fn set_autostart_enabled_at(path: &Path, enabled: bool) -> io::Result<()> {
    if enabled {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        // Write to temporary file first then atomically rename
        let tmp_path = path.with_extension("tmp");
        fs::write(&tmp_path, AUTOSTART_DESKTOP_ENTRY)?;
        fs::rename(tmp_path, path)?;
        tracing::info!(path = %path.display(), "XDG autostart desktop entry enabled");
    } else if path.exists() {
        fs::remove_file(path)?;
        tracing::info!(path = %path.display(), "XDG autostart desktop entry removed");
    }
    Ok(())
}
