//! Core of the application: the typed messages exchanged with the UI and the
//! state those messages produce.
//!
//! Nothing in this module touches hardware. `src/app.rs` owns the Slint bridge,
//! `src/hardware` owns the sensor boundary, and this module is the contract
//! between them (FOUND-05).

pub mod benchmark;
pub mod events;
pub mod state;
pub mod telemetry;
pub mod worker;

pub use benchmark::{BenchmarkTimer, MemoryMetrics, log_benchmark, read_memory_metrics};
pub use events::{AppCommand, AppEvent, ScreenId};
pub use state::AppState;
pub use telemetry::{TelemetryHistory, TelemetryPaths, TelemetryPoint};
pub use worker::spawn_monitoring_worker;
