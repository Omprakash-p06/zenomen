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

/// Update existing desktop entry lines or append Hidden / X-GNOME flags non-destructively.
fn update_desktop_entry(content: &str, enabled: bool) -> String {
    let mut lines: Vec<String> = content.lines().map(|s| s.to_string()).collect();
    let mut has_hidden = false;
    let mut has_gnome = false;

    for line in &mut lines {
        let trimmed = line.trim();
        if let Some((k, _)) = trimmed.split_once('=') {
            if k.trim().eq_ignore_ascii_case("Hidden") {
                *line = format!("Hidden={}", !enabled);
                has_hidden = true;
            } else if k.trim().eq_ignore_ascii_case("X-GNOME-Autostart-enabled") {
                *line = format!("X-GNOME-Autostart-enabled={}", enabled);
                has_gnome = true;
            }
        }
    }

    if !has_hidden {
        lines.push(format!("Hidden={}", !enabled));
    }
    if !has_gnome {
        lines.push(format!("X-GNOME-Autostart-enabled={}", enabled));
    }

    let mut result = lines.join("\n");
    if !result.ends_with('\n') {
        result.push('\n');
    }
    result
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

/// Enable or disable login autostart at a specific path non-destructively.
pub fn set_autostart_enabled_at(path: &Path, enabled: bool) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    let new_content = if path.is_file() {
        let existing = fs::read_to_string(path)?;
        update_desktop_entry(&existing, enabled)
    } else if enabled {
        AUTOSTART_DESKTOP_ENTRY.to_string()
    } else {
        return Ok(());
    };

    let tmp_path = path.with_extension(format!("tmp.{}", std::process::id()));
    fs::write(&tmp_path, new_content)?;
    fs::rename(tmp_path, path)?;
    tracing::info!(path = %path.display(), enabled, "XDG autostart desktop entry updated");
    Ok(())
}
