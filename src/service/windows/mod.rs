//! Windows privileged hardware control service and named pipe IPC (WIN-03, WIN-04).

pub mod authorization;
pub mod client;
pub mod hp_wmi;
pub mod lifecycle;
pub mod pipe;
pub mod profiles;
pub mod protocol;
pub mod security;
pub mod server;
pub mod wmi_diag;
#[cfg(windows)]
pub mod wmi_ffi;

pub use client::WindowsServiceClient;
pub use profiles::{BoardProfile, resolve_board_profile};
pub use protocol::{HardwareRequest, HardwareResponse};
