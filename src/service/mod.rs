pub mod backend;
pub mod client;
#[cfg(target_os = "linux")]
pub mod contract;
#[cfg(target_os = "linux")]
pub mod polkit;
pub mod protocol;
pub mod types;
#[cfg(target_os = "windows")]
pub mod windows;

pub use backend::HardwareBackend;
pub use client::{HardwareServiceClient, ServiceConnectionStatus};
#[cfg(target_os = "linux")]
pub use contract::HardwareService;
#[cfg(target_os = "linux")]
pub use polkit::{POLKIT_ACTION_CONTROL, PolkitVerifier};
pub use types::{FanControlState, FanMode, HardwareControlError, PresetResult};
