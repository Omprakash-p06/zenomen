//! Hardware abstraction boundary.
//!
//! Everything that touches a sensor, a sysfs path or an external command lives
//! behind [`provider::HardwareProvider`]. The UI never imports anything from
//! this module directly — it only ever sees the plain data types re-exported
//! here, and only through `crate::core::events` (FOUND-05).

pub mod provider;

pub use provider::{HardwareCapabilities, HardwareProvider, TelemetrySnapshot};
