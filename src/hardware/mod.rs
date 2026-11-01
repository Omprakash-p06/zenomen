//! Hardware abstraction boundary.
//!
//! Everything that touches a sensor, a sysfs path or an external command lives
//! behind [`provider::HardwareProvider`]. The UI never imports anything from
//! this module directly — it only ever sees the plain data types re-exported
//! here, and only through `crate::core::events` (FOUND-05).

// The mock provider is complete in task 1 but nothing constructs it until the
// monitoring worker lands in task 3; the targeted allow keeps this commit
// warning-free without pretending the module is finished.
#[allow(dead_code, reason = "wired into the monitoring worker in task 3")]
pub mod mock;
pub mod provider;

// Re-exported so `crate::core` and `src/app.rs` name the boundary in one hop.
pub use provider::{HardwareCapabilities, TelemetrySnapshot};
