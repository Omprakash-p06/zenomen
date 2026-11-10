//! GPUI-CE cross-platform presentation architecture scaffold (Section 5.4).
//!
//! Provides the component contracts, typed element trees, and theme tokens
//! for migrating from Slint to GPUI-CE once feature parity is established.

pub mod app;
pub mod components;
pub mod screens;
pub mod theme;

pub use app::GpuiApp;
pub use components::*;
pub use screens::*;
pub use theme::*;
