//! Releases update action (REL-06).
//!
//! Provides a safe, browser-delegated "Check for updates" link that directs the
//! user to the official GitHub Releases page.
//!
//! Deliberately implements zero HTTP networking, downloading, or self-updating code.

use std::io;
use std::process::{Command, Stdio};
use thiserror::Error;

/// Target URL for the latest GitHub Releases page.
pub const RELEASES_URL: &str = "https://github.com/Omprakash-p06/zenomen/releases/latest";

/// Errors encountered when attempting to launch the external web browser.
#[derive(Debug, Error)]
pub enum ReleasesError {
    #[error("Failed to spawn browser launcher: {0}")]
    Spawn(#[from] io::Error),
}

/// Open the official GitHub Releases page in the user's default web browser.
///
/// Spawns `xdg-open` asynchronously without blocking and without waiting for
/// the child process.
pub fn open_releases_page() -> Result<(), ReleasesError> {
    Command::new("xdg-open")
        .arg(RELEASES_URL)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;

    Ok(())
}
