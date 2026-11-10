//! ZenOmen library crate.
//!
//! Exposes core application state, hardware provider abstraction, and the
//! Slint UI bridge.

// The UI is a default feature, not a hard dependency: `zenomen-hw` runs as root
// and must not compile or link the Slint toolkit it never creates a window with.
#[cfg(feature = "ui")]
pub mod app;
pub mod config;
pub mod core;
pub mod desktop;
pub mod hardware;
pub mod logging;
pub mod rgb;
pub mod service;
#[cfg(feature = "ui")]
pub mod ui;

#[cfg(feature = "ui")]
slint::include_modules!();
