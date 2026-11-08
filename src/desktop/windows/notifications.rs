//! Windows balloon / toast notification support via Shell_NotifyIconW (SYS-01).

use crate::desktop::DesktopError;

#[cfg(target_os = "windows")]
use windows_sys::Win32::UI::Shell::{
    NIF_INFO, NIIF_INFO, NIM_MODIFY, NOTIFYICONDATAW, Shell_NotifyIconW,
};

/// Display a desktop notification via the shell notification area.
pub fn show_notification(title: &str, message: &str) -> Result<(), DesktopError> {
    show_notification_with_hwnd(title, message, 0)
}

/// Display a desktop notification associated with a specific window/tray handle.
pub fn show_notification_with_hwnd(
    title: &str,
    message: &str,
    hwnd: isize,
) -> Result<(), DesktopError> {
    #[cfg(target_os = "windows")]
    {
        let mut nid: NOTIFYICONDATAW = unsafe { std::mem::zeroed() };
        nid.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
        nid.hWnd = hwnd as windows_sys::Win32::Foundation::HWND;
        nid.uID = 1;
        nid.uFlags = NIF_INFO;
        nid.dwInfoFlags = NIIF_INFO;

        // Copy title (up to 64 chars)
        let title_utf16: Vec<u16> = title.encode_utf16().collect();
        let copy_len = title_utf16.len().min(nid.szInfoTitle.len() - 1);
        nid.szInfoTitle[..copy_len].copy_from_slice(&title_utf16[..copy_len]);

        // Copy message (up to 256 chars)
        let msg_utf16: Vec<u16> = message.encode_utf16().collect();
        let copy_len = msg_utf16.len().min(nid.szInfo.len() - 1);
        nid.szInfo[..copy_len].copy_from_slice(&msg_utf16[..copy_len]);

        let success = unsafe { Shell_NotifyIconW(NIM_MODIFY, &nid) };

        if success == 0 {
            tracing::warn!(
                title,
                message,
                hwnd,
                "Shell_NotifyIconW notification dispatch failed"
            );
        }
        Ok(())
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = hwnd;
        tracing::info!(title, message, "mock desktop notification posted");
        Ok(())
    }
}
