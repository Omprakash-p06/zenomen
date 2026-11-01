//! ZenOmen library crate.
//!
//! Exposes core application state, hardware provider abstraction, and the
//! Slint UI bridge.

pub mod app;
pub mod config;
pub mod core;
pub mod desktop;
pub mod hardware;
pub mod rgb;
pub mod service;

slint::include_modules!();
