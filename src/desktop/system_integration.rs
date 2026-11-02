//! System integration installer for AppImage and standalone distribution.
//!
//! Installs the systemd unit, D-Bus configuration, polkit policy, and
//! freedesktop desktop entry with root privileges (REL-03, T-05-02, T-05-03).

use std::fs;
use std::path::Path;
use std::process::Command;
use thiserror::Error;

pub const DESKTOP_ENTRY_TARGET: &str = "/usr/share/applications/zenomen.desktop";
pub const SYSTEMD_UNIT_TARGET: &str = "/usr/lib/systemd/system/zenomen-hw.service";
pub const DBUS_CONF_TARGET: &str = "/usr/share/dbus-1/system.d/com.zenomen.Hardware1.conf";
pub const POLKIT_POLICY_TARGET: &str = "/usr/share/polkit-1/actions/com.zenomen.hardware.policy";

pub const EMBEDDED_DESKTOP_ENTRY: &str = include_str!("../../data/applications/zenomen.desktop");
pub const EMBEDDED_SYSTEMD_UNIT: &str = include_str!("../../data/systemd/zenomen-hw.service");
pub const EMBEDDED_DBUS_CONF: &str = include_str!("../../data/dbus/com.zenomen.Hardware1.conf");
pub const EMBEDDED_POLKIT_POLICY: &str =
    include_str!("../../data/polkit/com.zenomen.hardware.policy");

#[derive(Debug, Error)]
pub enum SystemIntegrationError {
    #[error("System integration command requires root privileges (run with sudo)")]
    NotRoot,

    #[error("I/O error while writing system files: {0}")]
    Io(#[from] std::io::Error),

    #[error("System command failed: {0}")]
    Command(String),
}

/// Check if running with effective root permissions.
pub fn is_root() -> bool {
    #[cfg(unix)]
    {
        unsafe extern "C" {
            fn geteuid() -> u32;
        }
        unsafe { geteuid() == 0 }
    }
    #[cfg(not(unix))]
    {
        false
    }
}

/// Atomically write a string to a target path via a temporary file in the same directory.
pub fn atomic_write_file(target: &Path, content: &str) -> Result<(), std::io::Error> {
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent)?;
    }
    let temp_path = target.with_extension(format!("tmp.{}", std::process::id()));
    fs::write(&temp_path, content)?;
    if let Err(e) = fs::rename(&temp_path, target) {
        let _ = fs::remove_file(&temp_path);
        return Err(e);
    }
    Ok(())
}

/// Install embedded system integration files to target paths and reload/enable systemd unit.
pub fn install() -> Result<(), SystemIntegrationError> {
    if !is_root() {
        return Err(SystemIntegrationError::NotRoot);
    }

    install_to_prefix(Path::new("/"))?;

    // Reload systemd, enable and restart zenomen-hw
    run_systemctl(&["daemon-reload"])?;
    run_systemctl(&["enable", "zenomen-hw"])?;
    run_systemctl(&["restart", "zenomen-hw"])?;

    tracing::info!("system integration successfully installed and daemon started");
    Ok(())
}

/// Install files under a specific prefix root (useful for mock testing).
pub fn install_to_prefix(prefix: &Path) -> Result<(), std::io::Error> {
    let desktop_dest = prefix.join(DESKTOP_ENTRY_TARGET.trim_start_matches('/'));
    let unit_dest = prefix.join(SYSTEMD_UNIT_TARGET.trim_start_matches('/'));
    let dbus_dest = prefix.join(DBUS_CONF_TARGET.trim_start_matches('/'));
    let polkit_dest = prefix.join(POLKIT_POLICY_TARGET.trim_start_matches('/'));

    atomic_write_file(&desktop_dest, EMBEDDED_DESKTOP_ENTRY)?;
    atomic_write_file(&unit_dest, EMBEDDED_SYSTEMD_UNIT)?;
    atomic_write_file(&dbus_dest, EMBEDDED_DBUS_CONF)?;
    atomic_write_file(&polkit_dest, EMBEDDED_POLKIT_POLICY)?;

    Ok(())
}

/// Uninstall system integration files and disable systemd unit.
pub fn uninstall() -> Result<(), SystemIntegrationError> {
    if !is_root() {
        return Err(SystemIntegrationError::NotRoot);
    }

    // Stop and disable systemd unit
    let _ = run_systemctl(&["stop", "zenomen-hw"]);
    let _ = run_systemctl(&["disable", "zenomen-hw"]);

    uninstall_from_prefix(Path::new("/"))?;

    let _ = run_systemctl(&["daemon-reload"]);
    tracing::info!("system integration successfully removed");
    Ok(())
}

/// Remove installed files under a specific prefix root.
pub fn uninstall_from_prefix(prefix: &Path) -> Result<(), std::io::Error> {
    let targets = [
        prefix.join(DESKTOP_ENTRY_TARGET.trim_start_matches('/')),
        prefix.join(SYSTEMD_UNIT_TARGET.trim_start_matches('/')),
        prefix.join(DBUS_CONF_TARGET.trim_start_matches('/')),
        prefix.join(POLKIT_POLICY_TARGET.trim_start_matches('/')),
    ];

    for target in targets {
        if target.exists() {
            fs::remove_file(target)?;
        }
    }

    Ok(())
}

fn run_systemctl(args: &[&str]) -> Result<(), SystemIntegrationError> {
    let output = Command::new("systemctl")
        .args(args)
        .output()
        .map_err(|e| SystemIntegrationError::Command(format!("failed to execute systemctl: {e}")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(SystemIntegrationError::Command(format!(
            "systemctl {} failed: {}",
            args.join(" "),
            stderr.trim()
        )));
    }
    Ok(())
}
