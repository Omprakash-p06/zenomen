//! Process-wide logging: stderr for `RUST_LOG`, plus a daily file for support.
//!
//! The returned guard owns the non-blocking writer; it must live for the whole
//! process or the last buffered lines are lost on exit.
//!
//! Logging must never be a startup blocker: a desktop app that cannot create its
//! log directory still has to launch, so every failure here degrades to a
//! stderr-only subscriber and a warning.

use std::path::PathBuf;

use tracing_appender::non_blocking::{NonBlocking, WorkerGuard};
use tracing_appender::rolling::{RollingFileAppender, Rotation};
use tracing_subscriber::EnvFilter;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

/// Keeps the background log writer alive. Drop only at process exit.
pub struct LoggingGuard {
    _file: WorkerGuard,
}

/// Resolve the platform state directory plus `logs`.
///
/// Linux: `~/.local/state/zenomen/logs`. Other platforms: the equivalent
/// `directories` location, so the daemon and the GUI agree on one path.
fn log_directory() -> Option<PathBuf> {
    directories::ProjectDirs::from("com", "zenomen", "zenomen").map(|dirs| {
        dirs.state_dir()
            .unwrap_or_else(|| dirs.data_dir())
            .join("logs")
    })
}

/// Open the rotating log file, or explain why it could not be opened.
///
/// Returning `None` is the whole failure contract: the caller then installs a
/// stderr-only subscriber. Nothing here aborts startup.
fn open_file_appender() -> Option<(NonBlocking, WorkerGuard)> {
    let log_dir = log_directory()?;

    if let Err(e) = std::fs::create_dir_all(&log_dir) {
        eprintln!(
            "warning: cannot create log directory {}: {e}",
            log_dir.display()
        );
        return None;
    }

    let appender = RollingFileAppender::builder()
        .rotation(Rotation::DAILY)
        .filename_prefix("zenomen")
        .filename_suffix("log")
        // Bounded disk: seven days is enough to answer a support question.
        .max_log_files(7)
        .build(&log_dir);

    let appender = match appender {
        Ok(appender) => appender,
        Err(e) => {
            eprintln!(
                "warning: cannot open log file in {}: {e}",
                log_dir.display()
            );
            return None;
        }
    };

    Some(tracing_appender::non_blocking(appender))
}

/// Install stderr logging, adding a rotating file layer when one can be opened.
///
/// `try_init` rather than `init` so a second call in the same process degrades
/// instead of panicking.
pub fn init() -> Option<LoggingGuard> {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

    let stderr_layer = tracing_subscriber::fmt::layer()
        .with_writer(std::io::stderr)
        .with_target(false);

    let Some((writer, guard)) = open_file_appender() else {
        let _ = tracing_subscriber::registry()
            .with(filter)
            .with(stderr_layer)
            .try_init();
        return None;
    };

    let file_layer = tracing_subscriber::fmt::layer()
        .with_writer(writer)
        .with_ansi(false)
        .with_target(true);

    let _ = tracing_subscriber::registry()
        .with(filter)
        .with(stderr_layer)
        .with(file_layer)
        .try_init();

    Some(LoggingGuard { _file: guard })
}
