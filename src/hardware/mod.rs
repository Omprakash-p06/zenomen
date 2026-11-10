//! Hardware abstraction boundary.
//!
//! Everything that touches a sensor, a sysfs path or an external command lives
//! behind [`provider::HardwareProvider`]. The UI never imports anything from
//! this module directly — it only ever sees the plain data types re-exported
//! here, and only through `crate::core::events` (FOUND-05).

pub mod linux;
pub mod mock;
pub mod process_memory;
pub mod provider;
pub mod registry;
pub mod rgb;
pub mod unavailable;
#[cfg(target_os = "windows")]
pub mod windows;

// Re-exported so `crate::core` and `src/app.rs` name the boundary in one hop.
#[cfg(target_os = "linux")]
pub use linux::LinuxHardwareProvider;
pub use mock::{MockHardware, MockScenario};
pub use process_memory::{ProcessMemoryInfo, ProcessRole, ZenomenMemoryTelemetry};
pub use provider::{BatteryTelemetry, HardwareCapabilities, HardwareProvider, TelemetrySnapshot};
pub use registry::SensorRegistry;
pub use rgb::RgbInterfaceVariant;
#[cfg(not(any(target_os = "linux", target_os = "windows")))]
pub use unavailable::UnavailableHardwareProvider;
#[cfg(target_os = "windows")]
pub use windows::WindowsHardwareProvider;
