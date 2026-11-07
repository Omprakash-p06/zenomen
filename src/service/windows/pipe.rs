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
pub fn create_server_pipe() -> Result<*mut std::ffi::c_void, HardwareControlError> {
    #[cfg(windows)]
    {
        use windows_sys::Win32::Foundation::INVALID_HANDLE_VALUE;
        use windows_sys::Win32::Storage::FileSystem::{
            FILE_FLAG_FIRST_PIPE_INSTANCE, FILE_FLAG_OVERLAPPED, PIPE_ACCESS_DUPLEX,
        };
        use windows_sys::Win32::System::Pipes::{
            CreateNamedPipeW, PIPE_READMODE_MESSAGE, PIPE_TYPE_MESSAGE, PIPE_WAIT,
        };

        // Wide string for PIPE_NAME
        let pipe_name_wide: Vec<u16> = PIPE_NAME.encode_utf16().chain(std::iter::once(0)).collect();

        let handle = unsafe {
            CreateNamedPipeW(
                pipe_name_wide.as_ptr(),
                PIPE_ACCESS_DUPLEX | FILE_FLAG_FIRST_PIPE_INSTANCE | FILE_FLAG_OVERLAPPED,
                PIPE_TYPE_MESSAGE | PIPE_READMODE_MESSAGE | PIPE_WAIT,
                4, // Max instances
                PIPE_BUFFER_SIZE,
                PIPE_BUFFER_SIZE,
                PIPE_TIMEOUT_MS,
                std::ptr::null(), // Default security attributes
            )
        };

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
