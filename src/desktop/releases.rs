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

/// Open the official GitHub Releases page using a specified launcher command.
///
/// Spawns the launcher asynchronously and reaps the child process on a background
/// thread to avoid leaving zombie processes (IN-11).
pub fn open_releases_page_with(launcher: &str) -> Result<(), ReleasesError> {
    let mut child = Command::new(launcher)
        .arg(RELEASES_URL)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;

    std::thread::spawn(move || {
        let _ = child.wait();
    });

    Ok(())
}

/// Open the official GitHub Releases page in the user's default web browser.
pub fn open_releases_page() -> Result<(), ReleasesError> {
    open_releases_page_with("xdg-open")
}
