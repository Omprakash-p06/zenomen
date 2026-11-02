pub mod backend;
pub mod client;
pub mod contract;
pub mod polkit;
pub mod types;

pub use backend::HardwareBackend;
pub use client::{HardwareServiceClient, ServiceConnectionStatus};
pub use contract::HardwareService;
pub use polkit::{POLKIT_ACTION_CONTROL, PolkitVerifier};
pub use types::{FanMode, HardwareControlError, PresetResult};
