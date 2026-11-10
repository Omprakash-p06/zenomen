//! Win32 named-pipe server creation and message framing (WIN-03).

use crate::service::types::HardwareControlError;
use crate::service::windows::protocol::{decode_frame, encode_frame};

/// Canonical local named-pipe endpoint for ZenOmen privileged hardware service.
pub const PIPE_NAME: &str = r"\\.\pipe\ZenOmen.Hardware.v1";

/// Maximum buffer size for named pipe communication (64 KB).
pub const PIPE_BUFFER_SIZE: u32 = 65536;

/// Default timeout for client connections in milliseconds (30 seconds).
pub const PIPE_TIMEOUT_MS: u32 = 30000;

/// Create an instance of the server named pipe with security attributes.
pub fn create_server_pipe(
    _first_instance: bool,
) -> Result<*mut std::ffi::c_void, HardwareControlError> {
    #[cfg(windows)]
    {
        use windows_sys::Win32::Foundation::{INVALID_HANDLE_VALUE, LocalFree};
        use windows_sys::Win32::Security::SECURITY_ATTRIBUTES;
        use windows_sys::Win32::Storage::FileSystem::{
            FILE_FLAG_FIRST_PIPE_INSTANCE, PIPE_ACCESS_DUPLEX,
        };
        use windows_sys::Win32::System::Pipes::{
            CreateNamedPipeW, PIPE_READMODE_MESSAGE, PIPE_TYPE_MESSAGE, PIPE_WAIT,
        };

        // Wide string for PIPE_NAME
        let pipe_name_wide: Vec<u16> = PIPE_NAME.encode_utf16().chain(std::iter::once(0)).collect();

        let sd = crate::service::windows::security::create_pipe_security_descriptor();
        let mut sa: SECURITY_ATTRIBUTES = unsafe { std::mem::zeroed() };
        let sa_ptr = if let Some(p_sd) = sd {
            sa.nLength = std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32;
            sa.lpSecurityDescriptor = p_sd;
            sa.bInheritHandle = 0;
            &mut sa as *mut _
        } else {
            std::ptr::null_mut()
        };

        let mut open_mode = PIPE_ACCESS_DUPLEX;
        if _first_instance {
            open_mode |= FILE_FLAG_FIRST_PIPE_INSTANCE;
        }

        let handle = unsafe {
            CreateNamedPipeW(
                pipe_name_wide.as_ptr(),
                open_mode,
                PIPE_TYPE_MESSAGE | PIPE_READMODE_MESSAGE | PIPE_WAIT,
                4, // Max instances
                PIPE_BUFFER_SIZE,
                PIPE_BUFFER_SIZE,
                PIPE_TIMEOUT_MS,
                sa_ptr,
            )
        };

        if let Some(p_sd) = sd {
            unsafe {
                LocalFree(p_sd);
            }
        }

        if handle == INVALID_HANDLE_VALUE {
            return Err(HardwareControlError::IoError(format!(
                "Failed to create named pipe {PIPE_NAME}"
            )));
        }

        Ok(handle)
    }
    #[cfg(not(windows))]
    {
        Err(HardwareControlError::InterfaceMissing(
            "Named pipes are only available on Windows".to_string(),
        ))
    }
}

/// Read a length-prefixed frame from a stream or byte slice.
pub fn read_frame_from_slice<'a, T: serde::Deserialize<'a>>(
    buf: &'a [u8],
) -> Result<(T, usize), HardwareControlError> {
    decode_frame(buf)
}

/// Write a message into a length-prefixed byte buffer.
pub fn write_frame_to_vec<T: serde::Serialize>(msg: &T) -> Result<Vec<u8>, HardwareControlError> {
    encode_frame(msg)
}
