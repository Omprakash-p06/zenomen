//! Windows system tray integration via Shell_NotifyIconW and TrackPopupMenu (SYS-01).
//!
//! Hosts a hidden Win32 message window to handle tray callback events, context
//! menu interaction, and window presentation actions.

use crate::desktop::DesktopError;
use crate::desktop::single_instance::DesktopAction;
use std::sync::mpsc;

#[cfg(target_os = "windows")]
use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, WPARAM};
#[cfg(target_os = "windows")]
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
#[cfg(target_os = "windows")]
use windows_sys::Win32::UI::Shell::{
    NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, NOTIFYICONDATAW, Shell_NotifyIconW,
};
#[cfg(target_os = "windows")]
use windows_sys::Win32::UI::WindowsAndMessaging::{
    AppendMenuW, CreatePopupMenu, CreateWindowExW, DefWindowProcW, DestroyMenu, DestroyWindow,
    DispatchMessageW, GWLP_USERDATA, GetCursorPos, GetMessageW, GetWindowLongPtrW, IDI_APPLICATION,
    LoadIconW, MF_SEPARATOR, MF_STRING, PostMessageW, PostQuitMessage, RegisterClassW,
    SetForegroundWindow, SetWindowLongPtrW, TPM_BOTTOMALIGN, TPM_RETURNCMD, TPM_RIGHTBUTTON,
    TrackPopupMenu, TranslateMessage, WM_CLOSE, WM_DESTROY, WM_LBUTTONUP, WM_RBUTTONUP, WNDCLASSW,
};

pub const WM_TRAY_CALLBACK: u32 = 0x0400 + 101;
pub const TRAY_CLASS_NAME: &str = "ZenOmenTrayMsgWindow";

#[cfg(target_os = "windows")]
const MENU_ID_OPEN: usize = 1;
#[cfg(target_os = "windows")]
const MENU_ID_FAN_AUTO: usize = 2;
#[cfg(target_os = "windows")]
const MENU_ID_FAN_MAX: usize = 3;
#[cfg(target_os = "windows")]
const MENU_ID_QUIT: usize = 4;

/// RAII handle to the registered system tray icon.
pub struct TrayHandle {
    #[cfg(target_os = "windows")]
    pub nid: NOTIFYICONDATAW,
    #[cfg(target_os = "windows")]
    pub hwnd: HWND,
    #[cfg(not(target_os = "windows"))]
    pub dummy: (),
}

impl std::fmt::Debug for TrayHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        #[cfg(target_os = "windows")]
        {
            f.debug_struct("TrayHandle")
                .field("hwnd", &self.hwnd)
                .finish()
        }
        #[cfg(not(target_os = "windows"))]
        {
            f.debug_struct("TrayHandle").finish()
        }
    }
}

unsafe impl Send for TrayHandle {}
unsafe impl Sync for TrayHandle {}

impl TrayHandle {
    /// Display a balloon/toast notification bound to this tray icon instance.
    pub fn show_notification(&self, title: &str, message: &str) -> Result<(), DesktopError> {
        #[cfg(target_os = "windows")]
        {
            crate::desktop::windows::notifications::show_notification_with_hwnd(
                title,
                message,
                self.hwnd as isize,
            )
        }
        #[cfg(not(target_os = "windows"))]
        {
            crate::desktop::windows::notifications::show_notification(title, message)
        }
    }
}

impl Drop for TrayHandle {
    fn drop(&mut self) {
        let _ = remove_tray_icon(self);
    }
}

/// Register a native notification area icon and associated context menu.
pub fn register_tray_icon(
    action_sender: mpsc::SyncSender<DesktopAction>,
) -> Result<TrayHandle, DesktopError> {
    #[cfg(target_os = "windows")]
    {
        let (init_tx, init_rx) = mpsc::channel();

        // Spawn a background thread that creates the window and services its Win32 message loop.
        // In Win32, window message queues are strictly thread-affine (CR-01).
        std::thread::Builder::new()
            .name("zenomen-win32-tray-pump".to_string())
            .spawn(move || {
                let hinstance = unsafe { GetModuleHandleW(std::ptr::null()) };
                let class_name: Vec<u16> = format!("{TRAY_CLASS_NAME}\\0").encode_utf16().collect();

                let wnd_class = WNDCLASSW {
                    style: 0,
                    lpfnWndProc: Some(tray_window_proc),
                    cbClsExtra: 0,
                    cbWndExtra: 0,
                    hInstance: hinstance,
                    hIcon: unsafe { LoadIconW(std::ptr::null_mut(), IDI_APPLICATION) },
                    hCursor: std::ptr::null_mut(),
                    hbrBackground: std::ptr::null_mut(),
                    lpszMenuName: std::ptr::null(),
                    lpszClassName: class_name.as_ptr(),
                };

                unsafe {
                    RegisterClassW(&wnd_class);
                }

                let hwnd = unsafe {
                    CreateWindowExW(
                        0,
                        class_name.as_ptr(),
                        class_name.as_ptr(),
                        0,
                        0,
                        0,
                        0,
                        0,
                        std::ptr::null_mut(),
                        std::ptr::null_mut(),
                        hinstance,
                        std::ptr::null_mut(),
                    )
                };

                if hwnd.is_null() {
                    let code = unsafe { windows_sys::Win32::Foundation::GetLastError() };
                    let _ = init_tx.send(Err(DesktopError::Win32 {
                        code,
                        message: "Failed to create hidden tray message window".to_string(),
                    }));
                    return;
                }

                // Store action_sender pointer in GWLP_USERDATA
                let sender_box = Box::new(action_sender);
                let sender_raw = Box::into_raw(sender_box);
                unsafe {
                    SetWindowLongPtrW(hwnd, GWLP_USERDATA, sender_raw as isize);
                }

                let mut nid: NOTIFYICONDATAW = unsafe { std::mem::zeroed() };
                nid.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
                nid.hWnd = hwnd;
                nid.uID = 1;
                nid.uFlags = NIF_ICON | NIF_MESSAGE | NIF_TIP;
                nid.uCallbackMessage = WM_TRAY_CALLBACK;
                nid.hIcon = unsafe { LoadIconW(std::ptr::null_mut(), IDI_APPLICATION) };

                let tip = "ZenOmen - Hardware Control";
                let tip_utf16: Vec<u16> = tip.encode_utf16().collect();
                let copy_len = tip_utf16.len().min(nid.szTip.len() - 1);
                nid.szTip[..copy_len].copy_from_slice(&tip_utf16[..copy_len]);

                let success = unsafe { Shell_NotifyIconW(NIM_ADD, &nid) };
                if success == 0 {
                    let code = unsafe { windows_sys::Win32::Foundation::GetLastError() };
                    unsafe {
                        let _ = Box::from_raw(sender_raw);
                        DestroyWindow(hwnd);
                    }
                    let _ = init_tx.send(Err(DesktopError::Win32 {
                        code,
                        message: "Shell_NotifyIconW NIM_ADD failed".to_string(),
                    }));
                    return;
                }

                let _ = init_tx.send(Ok(TrayHandle { nid, hwnd }));

                let mut msg = unsafe { std::mem::zeroed() };
                while unsafe { GetMessageW(&mut msg, hwnd, 0, 0) } > 0 {
                    unsafe {
                        TranslateMessage(&msg);
                        DispatchMessageW(&msg);
                    }
                }
            })
            .map_err(|e| {
                DesktopError::Other(format!("Failed to spawn tray message loop thread: {e}"))
            })?;

        init_rx.recv().map_err(|_| {
            DesktopError::Other("Tray initialization thread disconnected unexpectedly".to_string())
        })?
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = action_sender;
        Ok(TrayHandle { dummy: () })
    }
}

/// Remove the registered notification area icon.
pub fn remove_tray_icon(handle: &TrayHandle) -> Result<(), DesktopError> {
    #[cfg(target_os = "windows")]
    {
        unsafe {
            Shell_NotifyIconW(NIM_DELETE, &handle.nid);
            if !handle.hwnd.is_null() {
                PostMessageW(handle.hwnd, WM_CLOSE, 0, 0);
            }
        }
        Ok(())
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = handle;
        Ok(())
    }
}

#[cfg(target_os = "windows")]
unsafe extern "system" fn tray_window_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_TRAY_CALLBACK => {
            let event = lparam as u32;
            let user_data = unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) };
            if user_data == 0 {
                return 0;
            }
            let sender = unsafe { &*(user_data as *const mpsc::SyncSender<DesktopAction>) };

            if event == WM_LBUTTONUP {
                let _ = sender.try_send(DesktopAction::PresentWindow);
            } else if event == WM_RBUTTONUP {
                let hmenu = unsafe { CreatePopupMenu() };
                if !hmenu.is_null() {
                    let open_text: Vec<u16> = "Open ZenOmen\0".encode_utf16().collect();
                    let auto_text: Vec<u16> = "Fan Mode: Auto\0".encode_utf16().collect();
                    let max_text: Vec<u16> = "Fan Mode: Max\0".encode_utf16().collect();
                    let quit_text: Vec<u16> = "Quit ZenOmen\0".encode_utf16().collect();

                    unsafe {
                        AppendMenuW(hmenu, MF_STRING, MENU_ID_OPEN, open_text.as_ptr());
                        AppendMenuW(hmenu, MF_STRING, MENU_ID_FAN_AUTO, auto_text.as_ptr());
                        AppendMenuW(hmenu, MF_STRING, MENU_ID_FAN_MAX, max_text.as_ptr());
                        AppendMenuW(hmenu, MF_SEPARATOR, 0, std::ptr::null());
                        AppendMenuW(hmenu, MF_STRING, MENU_ID_QUIT, quit_text.as_ptr());

                        let mut pt = POINT { x: 0, y: 0 };
                        GetCursorPos(&mut pt);
                        SetForegroundWindow(hwnd);

                        let selected = TrackPopupMenu(
                            hmenu,
                            TPM_RIGHTBUTTON | TPM_BOTTOMALIGN | TPM_RETURNCMD,
                            pt.x,
                            pt.y,
                            0,
                            hwnd,
                            std::ptr::null(),
                        ) as usize;

                        match selected {
                            MENU_ID_OPEN => {
                                let _ = sender.try_send(DesktopAction::PresentWindow);
                            }
                            MENU_ID_FAN_AUTO => {
                                let _ = sender.try_send(DesktopAction::SetFanModeAuto);
                            }
                            MENU_ID_FAN_MAX => {
                                let _ = sender.try_send(DesktopAction::SetFanModeMax);
                            }
                            MENU_ID_QUIT => {
                                let _ = sender.try_send(DesktopAction::Quit);
                            }
                            _ => {}
                        }

                        DestroyMenu(hmenu);
                    }
                }
            }
            0
        }
        WM_CLOSE => {
            unsafe {
                DestroyWindow(hwnd);
            }
            0
        }
        WM_DESTROY => {
            unsafe {
                PostQuitMessage(0);
            }
            0
        }
        _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}
