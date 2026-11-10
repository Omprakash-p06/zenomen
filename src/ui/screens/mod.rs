//! High-level screen models for the ZenOmen GPUI-CE architecture.

pub mod cooling;
pub mod diagnostics;
pub mod lighting;
pub mod overview;
pub mod performance;
pub mod settings;

pub use cooling::CoolingScreen;
pub use diagnostics::DiagnosticsScreen;
pub use lighting::LightingScreen;
pub use overview::OverviewScreen;
pub use performance::PerformanceScreen;
pub use settings::SettingsScreen;
