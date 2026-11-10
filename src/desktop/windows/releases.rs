//! Windows browser launcher for official GitHub Releases page (REL-06).
//!
//! Uses `ShellExecuteW` to open the system default web browser to the latest releases.
//! Deliberately implements zero HTTP networking, downloading, or self-updating code.

use crate::desktop::releases::ReleasesError;
#[cfg(target_os = "windows")]
use std::io;

pub const RELEASES_URL: &str = "https://github.com/Omprakash-p06/zenomen/releases/latest";

/// Open the official GitHub Releases page in the user's default web browser using `ShellExecuteW`.
pub fn open_releases_page() -> Result<(), ReleasesError> {
    #[cfg(target_os = "windows")]
    {
        use windows_sys::Win32::UI::Shell::ShellExecuteW;
        use windows_sys::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

        let verb: Vec<u16> = "open\0".encode_utf16().collect();
        let url: Vec<u16> = format!("{RELEASES_URL}\0").encode_utf16().collect();

        let h_inst = unsafe {
            ShellExecuteW(
                std::ptr::null_mut(),
                verb.as_ptr(),
                url.as_ptr(),
                std::ptr::null(),
                std::ptr::null(),
                SW_SHOWNORMAL,
            )
        };

        let code = h_inst as isize;
        if code <= 32 {
            return Err(ReleasesError::Spawn(io::Error::from_raw_os_error(
                code as i32,
            )));
        }

        Ok(())
    }
    #[cfg(not(target_os = "windows"))]
    {
        crate::desktop::releases::open_releases_page_with("xdg-open")
    }
}
