//! Diagnostics report compilation and serialization.
//!
//! Gathers system metadata, DMI hardware identifiers, shared `SensorRegistry` bindings,
//! hardware capabilities, error history, and latest telemetry into a structured report (DIAG-01).
//! Exports the report atomically to a clean JSON file (DIAG-02).

use std::path::Path;
use std::time::SystemTime;

use serde::{Deserialize, Serialize};

use crate::core::state::AppState;
use crate::hardware::registry::SensorRegistry;
use crate::hardware::{HardwareCapabilities, TelemetrySnapshot};

/// Full diagnostic detection and status report.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DiagnosticsReport {
    /// ZenOmen application version (e.g. `0.1.0`).
    pub app_version: String,
    /// Host operating system name (e.g. `Ubuntu 24.04 LTS`).
    pub os_name: String,
    /// Operating system kernel version (e.g. `6.8.0-45-generic`).
    pub kernel_version: String,
    /// DMI / SMBIOS product name (e.g. `OMEN Laptop 15-ek0xxx`).
    pub product_name: Option<String>,
    /// DMI / SMBIOS system vendor (e.g. `HP`).
    pub sys_vendor: Option<String>,
    /// Subsystem capabilities detected on this system.
    pub capabilities: HardwareCapabilities,
    /// Canonical CPU temperature sensor binding.
    pub cpu_sensor: Option<String>,
    /// Registered fan tachometers.
    pub fan_sensors: Vec<String>,
    /// Discovered GPU cards and drivers.
    pub gpu_sensors: Vec<String>,
    /// Latest snapshot readings from the hardware provider.
    pub latest_telemetry: Option<TelemetrySnapshot>,
    /// Most recent telemetry sampling error, if any.
    pub recent_error: Option<String>,
    /// UTC timestamp of report generation in ISO 8601 format.
    pub timestamp: String,
}

/// Collect a diagnostic report using the default system paths.
pub fn collect_diagnostics(
    state: &AppState,
    registry: Option<&SensorRegistry>,
) -> DiagnosticsReport {
    collect_diagnostics_from_roots(
        state,
        registry,
        Path::new("/etc/os-release"),
        Path::new("/proc/sys/kernel/osrelease"),
        Path::new("/sys/class/dmi/id"),
        Path::new("/sys/class/drm"),
    )
}

/// Collect a diagnostic report with customizable paths for testing.
pub fn collect_diagnostics_from_roots(
    state: &AppState,
    registry: Option<&SensorRegistry>,
    os_release_path: &Path,
    kernel_release_path: &Path,
    dmi_root: &Path,
    drm_root: &Path,
) -> DiagnosticsReport {
    let app_version = env!("CARGO_PKG_VERSION").to_string();
    let os_name = parse_os_name(os_release_path);
    let kernel_version = parse_kernel_version(kernel_release_path);
    let product_name = read_trimmed_sysfs(&dmi_root.join("product_name"));
    let sys_vendor = read_trimmed_sysfs(&dmi_root.join("sys_vendor"));

    let (cpu_sensor, fan_sensors) = if let Some(reg) = registry {
        let cpu = reg
            .cpu_temp
            .as_ref()
            .map(|s| format!("{} [{}] ({})", s.driver, s.label, s.input_path.display()));
        let fans = reg
            .fan_tachometers
            .iter()
            .map(|f| format!("fan{} [{}] ({})", f.index, f.label, f.input_path.display()))
            .collect();
        (cpu, fans)
    } else {
        (None, Vec::new())
    };

    let mut gpu_sensors = Vec::new();
    for gpu in crate::hardware::linux::gpu::enumerate_drm_gpus(drm_root) {
        gpu_sensors.push(format!(
            "{} (card: {}, sysfs: {})",
            gpu.name,
            gpu.card_name,
            gpu.sysfs_device_path.display()
        ));
    }
    if gpu_sensors.is_empty()
        && let Some(gpu_name) = state
            .latest_snapshot
            .as_ref()
            .and_then(|s| s.gpu_name.as_ref())
    {
        gpu_sensors.push(gpu_name.clone());
    }

    DiagnosticsReport {
        app_version,
        os_name,
        kernel_version,
        product_name,
        sys_vendor,
        capabilities: state.capabilities,
        cpu_sensor,
        fan_sensors,
        gpu_sensors,
        latest_telemetry: state.latest_snapshot.clone(),
        recent_error: state.last_sampling_error.clone(),
        timestamp: format_iso8601_utc(SystemTime::now()),
    }
}

/// Atomically export a diagnostics report to a formatted JSON file (DIAG-02).
pub fn export_diagnostics_to_file(
    report: &DiagnosticsReport,
    target_path: &Path,
) -> Result<(), String> {
    if let Some(parent) = target_path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }

    let json_bytes = serde_json::to_vec_pretty(report).map_err(|e| e.to_string())?;
    let temp_path = target_path.with_extension(format!("tmp.{}", std::process::id()));

    std::fs::write(&temp_path, json_bytes).map_err(|e| e.to_string())?;
    std::fs::rename(&temp_path, target_path).map_err(|e| e.to_string())?;

    Ok(())
}

fn parse_os_name(os_release_path: &Path) -> String {
    let Ok(content) = std::fs::read_to_string(os_release_path) else {
        return "Linux".to_string();
    };

    for line in content.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix("PRETTY_NAME=") {
            return rest.trim_matches('"').trim_matches('\'').to_string();
        }
    }

    for line in content.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix("NAME=") {
            return rest.trim_matches('"').trim_matches('\'').to_string();
        }
    }

    "Linux".to_string()
}

fn parse_kernel_version(kernel_release_path: &Path) -> String {
    std::fs::read_to_string(kernel_release_path)
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|_| "Unknown".to_string())
}

fn read_trimmed_sysfs(path: &Path) -> Option<String> {
    let content = std::fs::read_to_string(path).ok()?;
    let trimmed = content.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

/// Format a `SystemTime` as ISO 8601 UTC string (`YYYY-MM-DDTHH:MM:SSZ`).
fn format_iso8601_utc(time: SystemTime) -> String {
    let secs = time
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    let s = secs % 60;
    let m = (secs / 60) % 60;
    let h = (secs / 3600) % 24;
    let days = secs / 86400;

    let mut year = 1970;
    let mut days_rem = days;
    loop {
        let leap = if year % 4 == 0 && (year % 100 != 0 || year % 400 == 0) {
            1
        } else {
            0
        };
        let days_in_year = 365 + leap;
        if days_rem < days_in_year {
            let month_days = [31, 28 + leap, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
            let mut month = 1;
            for md in month_days {
                if days_rem < md {
                    break;
                }
                days_rem -= md;
                month += 1;
            }
            let day = days_rem + 1;
            return format!("{year:04}-{month:02}-{day:02}T{h:02}:{m:02}:{s:02}Z");
        }
        days_rem -= days_in_year;
        year += 1;
    }
}
