//! System integration installer for AppImage and standalone distribution.
//!
//! Installs the systemd unit, D-Bus configuration, polkit policy, and
//! freedesktop desktop entry with root privileges (REL-03, T-05-02, T-05-03).

use std::fs;
use std::io::Write;
use std::path::Path;
use std::process::Command;
use thiserror::Error;

pub const DESKTOP_ENTRY_TARGET: &str = "/usr/share/applications/zenomen.desktop";
pub const SYSTEMD_UNIT_TARGET: &str = "/usr/lib/systemd/system/zenomen-hw.service";
pub const DBUS_CONF_TARGET: &str = "/usr/share/dbus-1/system.d/com.zenomen.Hardware1.conf";
pub const DBUS_SERVICE_TARGET: &str =
    "/usr/share/dbus-1/system-services/com.zenomen.Hardware1.service";
pub const POLKIT_POLICY_TARGET: &str = "/usr/share/polkit-1/actions/com.zenomen.hardware.policy";
pub const DAEMON_BIN_TARGET: &str = "/usr/bin/zenomen-hw";

pub const EMBEDDED_DESKTOP_ENTRY: &str = include_str!("../../data/applications/zenomen.desktop");
pub const EMBEDDED_SYSTEMD_UNIT: &str = include_str!("../../data/systemd/zenomen-hw.service");
pub const EMBEDDED_DBUS_CONF: &str = include_str!("../../data/dbus/com.zenomen.Hardware1.conf");
pub const EMBEDDED_DBUS_SERVICE: &str =
    include_str!("../../data/dbus/com.zenomen.Hardware1.service");
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
/// Ensures changes are synced to disk (IN-01).
pub fn atomic_write_file(target: &Path, content: &str) -> Result<(), std::io::Error> {
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent)?;
    }
    let temp_path = target.with_extension(format!("tmp.{}", std::process::id()));
    {
        let mut file = fs::File::create(&temp_path)?;
        file.write_all(content.as_bytes())?;
        file.sync_all()?;
    }
    if let Err(e) = fs::rename(&temp_path, target) {
        let _ = fs::remove_file(&temp_path);
        return Err(e);
    }
    if let Some(dir) = target.parent().and_then(|p| fs::File::open(p).ok()) {
        let _ = dir.sync_all();
    }
    Ok(())
}

/// Ensure the daemon binary is present in system path (CR-02).
pub fn install_daemon_binary(target_bin: &Path) -> Result<(), SystemIntegrationError> {
    let current_exe = std::env::current_exe().ok();
    let candidate = current_exe
        .as_ref()
        .map(|exe| exe.with_file_name("zenomen-hw"));
    install_daemon_binary_from(candidate.as_deref(), target_bin)
}

/// Core installation logic decoupled from current executable discovery for testability.
pub fn install_daemon_binary_from(
    candidate: Option<&Path>,
    target_bin: &Path,
) -> Result<(), SystemIntegrationError> {
    if let Some(candidate) = candidate.filter(|p| p.is_file()) {
        tracing::info!(
            src = %candidate.display(),
            dst = %target_bin.display(),
            "installing zenomen-hw daemon binary to system path"
        );
        if let Some(parent) = target_bin.parent() {
            fs::create_dir_all(parent)?;
        }
        let temp_bin = target_bin.with_extension(format!("tmp.{}", std::process::id()));
        fs::copy(candidate, &temp_bin)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(&temp_bin)?.permissions();
            perms.set_mode(0o755);
            fs::set_permissions(&temp_bin, perms)?;
        }
        fs::rename(&temp_bin, target_bin)?;
        Ok(())
    } else if target_bin.exists() {
        Ok(())
    } else {
        Err(SystemIntegrationError::Command(format!(
            "zenomen-hw binary not found at {}; install the .deb package or place zenomen-hw alongside zenomen",
            target_bin.display(),
        )))
    }
}

/// Install embedded system integration files to target paths and reload/enable systemd unit.
pub fn install() -> Result<(), SystemIntegrationError> {
    if !is_root() {
        return Err(SystemIntegrationError::NotRoot);
    }

    // Ensure daemon binary exists before starting unit (CR-02)
    install_daemon_binary(Path::new(DAEMON_BIN_TARGET))?;

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
    let dbus_srv_dest = prefix.join(DBUS_SERVICE_TARGET.trim_start_matches('/'));
    let polkit_dest = prefix.join(POLKIT_POLICY_TARGET.trim_start_matches('/'));

    atomic_write_file(&desktop_dest, EMBEDDED_DESKTOP_ENTRY)?;
    atomic_write_file(&unit_dest, EMBEDDED_SYSTEMD_UNIT)?;
    atomic_write_file(&dbus_dest, EMBEDDED_DBUS_CONF)?;
    atomic_write_file(&dbus_srv_dest, EMBEDDED_DBUS_SERVICE)?;
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
        prefix.join(DBUS_SERVICE_TARGET.trim_start_matches('/')),
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
    let systemctl_bin = if Path::new("/usr/bin/systemctl").exists() {
        "/usr/bin/systemctl"
    } else if Path::new("/bin/systemctl").exists() {
        "/bin/systemctl"
    } else {
        "systemctl"
    };

    let mut cmd = Command::new(systemctl_bin);
    cmd.arg("--no-ask-password");
    cmd.args(args);

    let output = cmd.output().map_err(|e| {
        SystemIntegrationError::Command(format!("failed to execute {systemctl_bin}: {e}"))
    })?;

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

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_install_daemon_binary_from_copies_and_overwrites() {
        let base_dir =
            std::env::temp_dir().join(format!("zenomen_test_install_{}", std::process::id()));
        let _ = fs::remove_dir_all(&base_dir);
        fs::create_dir_all(&base_dir).unwrap();

        let candidate = base_dir.join("zenomen-hw");
        let target = base_dir.join("bin").join("zenomen-hw");

        fs::write(&candidate, b"new-daemon-content").unwrap();

        // 1. Installs when target does not exist
        assert!(install_daemon_binary_from(Some(&candidate), &target).is_ok());
        assert_eq!(fs::read(&target).unwrap(), b"new-daemon-content");

        // 2. Overwrites when target already exists
        fs::write(&candidate, b"upgraded-daemon-content").unwrap();
        assert!(install_daemon_binary_from(Some(&candidate), &target).is_ok());
        assert_eq!(fs::read(&target).unwrap(), b"upgraded-daemon-content");

        // 3. Succeeds if candidate is None but target already exists
        assert!(install_daemon_binary_from(None, &target).is_ok());

        // 4. Fails if candidate is None and target does not exist
        let non_existent = base_dir.join("bin").join("non_existent");
        assert!(install_daemon_binary_from(None, &non_existent).is_err());

        let _ = fs::remove_dir_all(&base_dir);
    }
}
