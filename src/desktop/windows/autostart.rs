//! Windows user autostart configuration via HKCU Run registry key (SYS-03).
//!
//! Stores the command line under `HKEY_CURRENT_USER\Software\Microsoft\Windows\CurrentVersion\Run`.
//! Runs unprivileged and requires zero administrator elevation.

use crate::desktop::DesktopError;
use std::path::Path;

#[cfg(target_os = "windows")]
use windows_sys::Win32::System::Registry::{
    HKEY_CURRENT_USER, KEY_READ, KEY_WRITE, REG_SZ, RegCloseKey, RegDeleteValueW, RegOpenKeyExW,
    RegQueryValueExW, RegSetValueExW,
};

pub const AUTOSTART_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
pub const AUTOSTART_VALUE_NAME: &str = "ZenOmen";

/// Format the autostart command line with `--minimized` flag.
pub fn format_autostart_command(exe_path: &Path) -> String {
    format!("\"{}\" --minimized", exe_path.display())
}

/// Enable or disable user-level autostart via HKCU Run registry key.
pub fn set_autostart_enabled(enabled: bool) -> Result<(), DesktopError> {
    #[cfg(target_os = "windows")]
    {
        let subkey: Vec<u16> = format!("{AUTOSTART_KEY}\0").encode_utf16().collect();
        let value_name: Vec<u16> = format!("{AUTOSTART_VALUE_NAME}\0").encode_utf16().collect();

        let mut hkey = std::ptr::null_mut();
        let status =
            unsafe { RegOpenKeyExW(HKEY_CURRENT_USER, subkey.as_ptr(), 0, KEY_WRITE, &mut hkey) };

        if status != 0 {
            return Err(DesktopError::Registry(format!(
                "Failed to open HKCU Run key for writing (code {status})"
            )));
        }

        let result = if enabled {
            let exe_path = std::env::current_exe().map_err(|e| {
                DesktopError::Other(format!("Failed to determine current exe: {e}"))
            })?;
            let command = format_autostart_command(&exe_path);
            let command_utf16: Vec<u16> = format!("{command}\0").encode_utf16().collect();
            let byte_len = (command_utf16.len() * size_of::<u16>()) as u32;

            let set_res = unsafe {
                RegSetValueExW(
                    hkey,
                    value_name.as_ptr(),
                    0,
                    REG_SZ,
                    command_utf16.as_ptr() as *const u8,
                    byte_len,
                )
            };
            if set_res != 0 {
                Err(DesktopError::Registry(format!(
                    "Failed to write ZenOmen to HKCU Run (code {set_res})"
                )))
            } else {
                Ok(())
            }
        } else {
            let del_res = unsafe { RegDeleteValueW(hkey, value_name.as_ptr()) };
            // 2 = ERROR_FILE_NOT_FOUND, ignore if already removed
            if del_res != 0 && del_res != 2 {
                Err(DesktopError::Registry(format!(
                    "Failed to delete ZenOmen from HKCU Run (code {del_res})"
                )))
            } else {
                Ok(())
            }
        };

        unsafe {
            RegCloseKey(hkey);
        }

        result
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = enabled;
        Ok(())
    }
}

/// Check whether ZenOmen is currently registered in HKCU Run registry key.
pub fn is_autostart_enabled() -> bool {
    #[cfg(target_os = "windows")]
    {
        let subkey: Vec<u16> = format!("{AUTOSTART_KEY}\0").encode_utf16().collect();
        let value_name: Vec<u16> = format!("{AUTOSTART_VALUE_NAME}\0").encode_utf16().collect();

        let mut hkey = std::ptr::null_mut();
        let status =
            unsafe { RegOpenKeyExW(HKEY_CURRENT_USER, subkey.as_ptr(), 0, KEY_READ, &mut hkey) };

        if status != 0 {
            return false;
        }

        let query_status = unsafe {
            RegQueryValueExW(
                hkey,
                value_name.as_ptr(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            )
        };

        unsafe {
            RegCloseKey(hkey);
        }

        query_status == 0
    }
    #[cfg(not(target_os = "windows"))]
    {
        false
    }
}
