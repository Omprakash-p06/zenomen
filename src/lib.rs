//! ZenOmen library crate.
//!
//! Exposes core application state, hardware provider abstraction, and the
//! Slint UI bridge.

pub mod app;
pub mod core;
pub mod hardware;
pub mod service;

slint::include_modules!();
