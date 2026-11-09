pub mod backend;
pub mod client;
pub mod contract;
pub mod polkit;
pub mod protocol;
pub mod types;
pub mod windows;

pub use backend::HardwareBackend;
pub use client::{HardwareServiceClient, ServiceConnectionStatus};
pub use contract::HardwareService;
pub use polkit::{POLKIT_ACTION_CONTROL, PolkitVerifier};
pub use types::{FanControlState, FanMode, HardwareControlError, PresetResult};
