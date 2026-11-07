//! Windows native hardware monitoring and device discovery.

pub mod battery;
pub mod cpu;
pub mod device;
pub mod gpu;
pub mod memory;
pub mod provider;
pub mod sensors;

pub use provider::WindowsHardwareProvider;
