//! Windows single-instance mutex and window activation (SYS-05).
//!
//! Uses a session-scoped Win32 mutex (`Local\ZenOmen.SingleInstance`) to enforce
//! single-instance execution. A secondary instance brings the primary window to the
//! foreground via `FindWindowW` and `SetForegroundWindow`, then terminates immediately.

use crate::desktop::DesktopError;

#[cfg(target_os = "windows")]
use windows_sys::Win32::Foundation::{
    CloseHandle, ERROR_ALREADY_EXISTS, GetLastError, HANDLE, INVALID_HANDLE_VALUE,
};
#[cfg(target_os = "windows")]
use windows_sys::Win32::System::Threading::CreateMutexW;
#[cfg(target_os = "windows")]
use windows_sys::Win32::UI::WindowsAndMessaging::{
    FindWindowW, SW_RESTORE, SetForegroundWindow, ShowWindow,
};

pub const SINGLE_INSTANCE_MUTEX_NAME: &str = r"Local\ZenOmen.SingleInstance";
pub const WINDOW_TITLE: &str = "ZenOmen";

/// RAII wrapper for the single-instance Win32 mutex handle.
#[derive(Debug)]
pub struct MutexHandle(
    #[cfg(target_os = "windows")] pub HANDLE,
    #[cfg(not(target_os = "windows"))] pub (),
);

unsafe impl Send for MutexHandle {}
unsafe impl Sync for MutexHandle {}

impl Drop for MutexHandle {
    fn drop(&mut self) {
        #[cfg(target_os = "windows")]
        if !self.0.is_null() && self.0 != INVALID_HANDLE_VALUE {
            unsafe {
                CloseHandle(self.0);
            }
        }
    }
}

/// Result of checking whether this process is the primary instance on Windows.
pub enum SingleInstanceResult {
    /// Acquired primary instance mutex. Keep this guard alive for the process lifetime.
    Primary(MutexHandle),
    /// Another instance is already running; the primary window has been activated.
    AlreadyRunning,
    /// Single-instance mutex could not be created; running standalone.
    Disabled(String),
}

/// Acquire the session-scoped single-instance mutex.
pub fn acquire_single_instance() -> Result<SingleInstanceResult, DesktopError> {
    #[cfg(target_os = "windows")]
    {
        let mutex_name: Vec<u16> = format!("{SINGLE_INSTANCE_MUTEX_NAME}\0")
            .encode_utf16()
            .collect();
        let handle = unsafe { CreateMutexW(std::ptr::null(), 1, mutex_name.as_ptr()) };

        if handle.is_null() || handle == INVALID_HANDLE_VALUE {
            let code = unsafe { GetLastError() };
            return Ok(SingleInstanceResult::Disabled(format!(
                "CreateMutexW failed with code {code}"
            )));
        }

        let last_error = unsafe { GetLastError() };
        if last_error == ERROR_ALREADY_EXISTS {
            let _ = present_existing_window();
            unsafe {
                CloseHandle(handle);
            }
            return Ok(SingleInstanceResult::AlreadyRunning);
        }

        Ok(SingleInstanceResult::Primary(MutexHandle(handle)))
    }
    #[cfg(not(target_os = "windows"))]
    {
        Ok(SingleInstanceResult::Primary(MutexHandle(())))
    }
}

/// Bring the primary ZenOmen window to the foreground if currently running.
pub fn present_existing_window() -> Result<(), DesktopError> {
    #[cfg(target_os = "windows")]
    {
        let title: Vec<u16> = format!("{WINDOW_TITLE}\0").encode_utf16().collect();
        let hwnd = unsafe { FindWindowW(std::ptr::null(), title.as_ptr()) };

        if !hwnd.is_null() {
            unsafe {
                ShowWindow(hwnd, SW_RESTORE);
                SetForegroundWindow(hwnd);
            }
        }
        Ok(())
    }
    #[cfg(not(target_os = "windows"))]
    {
        Ok(())
    }
}
