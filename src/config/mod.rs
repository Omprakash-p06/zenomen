//! Configuration management for ZenOmen.
//!
//! Handles persistent storage in `~/.config/zenomen/config.toml`, atomic saving
//! to prevent file corruption, and automatic fallback with backup creation
//! when corrupted configurations are encountered (SYS-06, SYS-07).

pub mod schema;

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub use schema::{
    AppConfig, DEFAULT_POLLING_INTERVAL_MS, GeneralConfig, MAX_POLLING_INTERVAL_MS,
    MIN_POLLING_INTERVAL_MS, TelemetryConfig,
};

/// Standard ProjectDirs helper for ZenOmen user paths (IN-02).
pub fn project_dirs() -> Option<directories::ProjectDirs> {
    directories::ProjectDirs::from("com", "zenomen", "zenomen")
}

/// Get the default path for ZenOmen's configuration file.
pub fn default_config_path() -> Option<PathBuf> {
    project_dirs().map(|dirs| dirs.config_dir().join("config.toml"))
}

/// Load configuration from disk, or return safe defaults if not found.
///
/// If the configuration file exists but cannot be parsed as valid TOML, it creates
/// a timestamped backup copy (`config.toml.bak.<timestamp>`), falls back to safe
/// defaults, and returns a human-readable explanation (SYS-07).
pub fn load_or_default(path: &Path) -> (AppConfig, Option<String>) {
    if !path.exists() {
        return (AppConfig::default(), None);
    }

    let content = match fs::read_to_string(path) {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!(path = %path.display(), error = %e, "could not read config file; using defaults");
            return (AppConfig::default(), None);
        }
    };

    match toml::from_str::<AppConfig>(&content) {
        Ok(mut config) => {
            config.sanitize();
            (config, None)
        }
        Err(e) => {
            let timestamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0);
            let backup_path = path.with_extension(format!("bak.{timestamp}"));

            if let Err(copy_err) = fs::copy(path, &backup_path) {
                tracing::error!(
                    error = %copy_err,
                    "failed to create backup of corrupted config file"
                );
            }

            let msg = format!(
                "Config file was invalid ({e}). Preserved as {} and restored safe defaults.",
                backup_path
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
            );
            tracing::warn!(notice = %msg, "loaded fallback configuration");

            (AppConfig::default(), Some(msg))
        }
    }
}

/// Atomically write configuration to disk.
///
/// Writes to a temporary file (`.tmp`), flushes and syncs to storage media,
/// and renames to the target path to guarantee atomic replacement (SYS-06).
pub fn save_atomic(path: &Path, config: &AppConfig) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    let mut sanitized = config.clone();
    sanitized.sanitize();

    let toml_str = toml::to_string_pretty(&sanitized)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e.to_string()))?;

    let tmp_path = path.with_extension("tmp");
    fs::write(&tmp_path, toml_str)?;

    let file = fs::File::open(&tmp_path)?;
    file.sync_all()?;
    drop(file);

    fs::rename(tmp_path, path)?;
    tracing::info!(path = %path.display(), "configuration atomically saved");
    Ok(())
}
