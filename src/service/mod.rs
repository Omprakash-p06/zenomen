//! Hardware service subsystem for privileged operations over system D-Bus.

pub mod backend;
pub mod contract;
pub mod polkit;
pub mod types;

pub use backend::HardwareBackend;
pub use contract::HardwareService;
pub use polkit::{PolkitVerifier, POLKIT_ACTION_CONTROL};
pub use types::{FanMode, HardwareControlError, PresetResult};
