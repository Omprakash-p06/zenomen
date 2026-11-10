//! Windows application data and state directory resolution (SYS-01, SYS-02).
//!
//! Maps standard Windows known folder paths:
//! - `%LOCALAPPDATA%\ZenOmen\` for user configuration, user-facing logs, and diagnostics.
//! - `%PROGRAMDATA%\ZenOmen\` for elevated service-owned state, cache, and logs.

use std::path::PathBuf;

use crate::desktop::DesktopError;

#[cfg(target_os = "windows")]
use windows_sys::Win32::System::Com::CoTaskMemFree;
#[cfg(target_os = "windows")]
use windows_sys::Win32::UI::Shell::{
    FOLDERID_LocalAppData, FOLDERID_ProgramData, SHGetKnownFolderPath,
};

/// Resolved standard application data locations for ZenOmen on Windows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppDataPaths {
    /// User-level configuration and diagnostics: `%LOCALAPPDATA%\ZenOmen`.
    pub local_app_data: PathBuf,
    /// Service-level state, cache, and service logs: `%PROGRAMDATA%\ZenOmen`.
    pub program_data: PathBuf,
}

/// Pure helper for resolving app data paths from optional environment variables.
///
/// Used for testing on any operating system without requiring Win32 APIs.
pub fn resolve_app_data_paths_from(
    local_app_data_env: Option<&str>,
    program_data_env: Option<&str>,
) -> AppDataPaths {
    let local = local_app_data_env
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(r"C:\Users\Default\AppData\Local"))
        .join("ZenOmen");

    let program = program_data_env
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(r"C:\ProgramData"))
        .join("ZenOmen");

    AppDataPaths {
        local_app_data: local,
        program_data: program,
    }
}

/// Resolve and ensure existence of `%LOCALAPPDATA%\ZenOmen` and `%PROGRAMDATA%\ZenOmen`.
pub fn resolve_app_data_paths() -> Result<AppDataPaths, DesktopError> {
    #[cfg(target_os = "windows")]
    {
        let local_path = unsafe { get_known_folder_path(&FOLDERID_LocalAppData) }
            .or_else(|| std::env::var("LOCALAPPDATA").ok().map(PathBuf::from))
            .unwrap_or_else(|| PathBuf::from(r"C:\Users\Default\AppData\Local"))
            .join("ZenOmen");

        let program_path = unsafe { get_known_folder_path(&FOLDERID_ProgramData) }
            .or_else(|| std::env::var("PROGRAMDATA").ok().map(PathBuf::from))
            .unwrap_or_else(|| PathBuf::from(r"C:\ProgramData"))
            .join("ZenOmen");

        if let Err(e) = std::fs::create_dir_all(&local_path) {
            tracing::warn!(path = %local_path.display(), error = %e, "could not create local app data directory");
        }
        if let Err(e) = std::fs::create_dir_all(&program_path) {
            tracing::warn!(path = %program_path.display(), error = %e, "could not create program data directory");
        }

        Ok(AppDataPaths {
            local_app_data: local_path,
            program_data: program_path,
        })
    }
    #[cfg(not(target_os = "windows"))]
    {
        let local = std::env::var("LOCALAPPDATA").ok();
        let program = std::env::var("PROGRAMDATA").ok();
        let paths = resolve_app_data_paths_from(local.as_deref(), program.as_deref());
        Ok(paths)
    }
}

#[cfg(target_os = "windows")]
unsafe fn get_known_folder_path(folder_id: *const windows_sys::core::GUID) -> Option<PathBuf> {
    let mut path_ptr: windows_sys::core::PWSTR = std::ptr::null_mut();
    let hr = unsafe { SHGetKnownFolderPath(folder_id, 0, std::ptr::null_mut(), &mut path_ptr) };

    if hr != 0 || path_ptr.is_null() {
        return None;
    }

    let mut len = 0;
    while unsafe { *path_ptr.add(len) } != 0 {
        len += 1;
    }
    let slice = unsafe { std::slice::from_raw_parts(path_ptr, len) };
    let path_str = String::from_utf16_lossy(slice);

    unsafe {
        CoTaskMemFree(path_ptr as *mut _);
    }

    Some(PathBuf::from(path_str))
}
