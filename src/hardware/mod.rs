//! Hardware abstraction boundary.
//!
//! Everything that touches a sensor, a sysfs path or an external command lives
//! behind [`provider::HardwareProvider`]. The UI never imports anything from
//! this module directly — it only ever sees the plain data types re-exported
//! here, and only through `crate::core::events` (FOUND-05).

pub mod provider;

// Re-exported so `crate::core` and `src/app.rs` name the boundary in one hop.
// The trait stays behind `provider::` until a backend implements it (01-03).
pub use provider::{HardwareCapabilities, TelemetrySnapshot};
