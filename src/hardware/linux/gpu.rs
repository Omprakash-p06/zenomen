//! Linux GPU enumeration and telemetry reading.
//!
//! Complies with MON-05 and MON-06:
//! - Dynamically discovers DRM cards in `/sys/class/drm/card*` without hardcoding `card0`.
//! - Supports AMD (amdgpu sysfs), Intel, and NVIDIA (cached watcher around nvidia-smi).
//! - Missing or sleeping GPUs degrade to `None` without blocking or erroring.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Mutex, mpsc};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

/// Detected GPU vendor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GpuVendor {
    Amd,
    Intel,
    Nvidia,
    Unknown(u32),
}

impl GpuVendor {
    pub fn from_pci_vendor_id(id: u32) -> Self {
        match id {
            0x1002 => Self::Amd,
            0x8086 => Self::Intel,
            0x10de => Self::Nvidia,
            other => Self::Unknown(other),
        }
    }

    pub fn default_name(&self) -> &'static str {
        match self {
            Self::Amd => "AMD Radeon Graphics",
            Self::Intel => "Intel HD/Iris/Arc Graphics",
            Self::Nvidia => "NVIDIA Dedicated GPU",
            Self::Unknown(_) => "Unknown GPU",
        }
    }
}

/// A discovered GPU card on the system.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GpuDevice {
    pub name: String,
    pub vendor: GpuVendor,
    pub card_name: String,
    pub sysfs_device_path: PathBuf,
    pub hwmon_path: Option<PathBuf>,
}

/// Instantaneous GPU telemetry reading.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct GpuTelemetry {
    pub name: String,
    pub utilization: Option<f32>,
    pub temperature_c: Option<f32>,
    pub vram_used_mb: Option<u64>,
    pub vram_total_mb: Option<u64>,
}

/// Dynamically enumerate DRM GPU devices under `/sys/class/drm`.
///
/// Filters strictly for `card[0-9]+` names, skipping connector endpoints like
/// `card1-DP-1` or `card1-eDP-1` (MON-05).
pub fn enumerate_drm_gpus(drm_dir: &Path) -> Vec<GpuDevice> {
    let mut devices = Vec::new();

    let Ok(entries) = std::fs::read_dir(drm_dir) else {
        return devices;
    };

    for entry in entries.flatten() {
        let file_name = entry.file_name();
        let name_str = file_name.to_string_lossy();

        if !is_card_device_name(&name_str) {
            continue;
        }

        let device_path = entry.path().join("device");
        if !device_path.exists() {
            continue;
        }

        let vendor_id = read_hex_file(&device_path.join("vendor")).unwrap_or(0);
        let vendor = GpuVendor::from_pci_vendor_id(vendor_id);

        let hwmon_path = find_hwmon_dir(&device_path);
        let name = vendor.default_name().to_string();

        devices.push(GpuDevice {
            name,
            vendor,
            card_name: name_str.to_string(),
            sysfs_device_path: device_path,
            hwmon_path,
        });
    }

    // Sort stably by card name (e.g. card0, card1)
    devices.sort_by(|a, b| a.card_name.cmp(&b.card_name));
    devices
}

/// Check if a directory entry is a primary card device (e.g. `card0`, `card1`)
/// rather than a subconnector (e.g. `card1-DP-1`, `renderD128`).
pub fn is_card_device_name(name: &str) -> bool {
    if !name.starts_with("card") {
        return false;
    }
    let suffix = &name[4..];
    !suffix.is_empty() && suffix.chars().all(|c| c.is_ascii_digit())
}

/// Find the `hwmon/hwmon*` directory associated with a device.
fn find_hwmon_dir(device_path: &Path) -> Option<PathBuf> {
    let hwmon_root = device_path.join("hwmon");
    let entries = std::fs::read_dir(hwmon_root).ok()?;
    for entry in entries.flatten() {
        let fname = entry.file_name();
        if fname.to_string_lossy().starts_with("hwmon") {
            return Some(entry.path());
        }
    }
    None
}

/// Read a hexadecimal number from a file (e.g. `0x1002\n`).
fn read_hex_file(path: &Path) -> Option<u32> {
    let content = std::fs::read_to_string(path).ok()?;
    let trimmed = content.trim();
    if let Some(hex_part) = trimmed
        .strip_prefix("0x")
        .or_else(|| trimmed.strip_prefix("0X"))
    {
        u32::from_str_radix(hex_part, 16).ok()
    } else {
        trimmed.parse::<u32>().ok()
    }
}

/// Sample telemetry for an AMD DRM GPU using sysfs.
pub fn sample_amd_gpu(device: &GpuDevice) -> GpuTelemetry {
    let mut telemetry = GpuTelemetry {
        name: device.name.clone(),
        ..Default::default()
    };

    // Utilization from gpu_busy_percent (0-100)
    let busy_path = device.sysfs_device_path.join("gpu_busy_percent");
    telemetry.utilization = std::fs::read_to_string(busy_path)
        .ok()
        .and_then(|c| c.trim().parse::<f32>().ok())
        .map(|pct| pct.clamp(0.0, 100.0));

    // Temperature from linked hwmon device (temp1_input in millidegrees)
    if let Some(hwmon) = &device.hwmon_path {
        let temp_path = hwmon.join("temp1_input");
        telemetry.temperature_c = std::fs::read_to_string(temp_path)
            .ok()
            .and_then(|c| c.trim().parse::<f32>().ok())
            .map(|mdeg| mdeg / 1000.0);
    }

    // VRAM usage (bytes -> MB)
    let vram_used_path = device.sysfs_device_path.join("mem_info_vram_used");
    telemetry.vram_used_mb = std::fs::read_to_string(vram_used_path)
        .ok()
        .and_then(|c| c.trim().parse::<u64>().ok())
        .map(|bytes| bytes / (1024 * 1024));

    let vram_total_path = device.sysfs_device_path.join("mem_info_vram_total");
    telemetry.vram_total_mb = std::fs::read_to_string(vram_total_path)
        .ok()
        .and_then(|c| c.trim().parse::<u64>().ok())
        .map(|bytes| bytes / (1024 * 1024));

    telemetry
}

/// One long-lived `nvidia-smi` watcher, shared by every telemetry tick.
///
/// Querying used to spawn a named thread *and* a process per tick. When the
/// driver was suspended in D3cold the query exceeded its 200 ms budget and the
/// thread was detached rather than joined, so a slow dGPU accumulated one thread
/// and one process per second for the life of the app. A single watcher thread
/// holds no per-sample resource, so a hung query now costs staleness instead of
/// threads — and the tick itself never blocks.
pub struct NvidiaSampler {
    latest: Arc<Mutex<Option<(Instant, GpuTelemetry)>>>,
    /// Capacity one: many ticks during a slow query coalesce into one request.
    wake: mpsc::SyncSender<()>,
    /// Kept so the watcher lives as long as the provider. Dropping the sender
    /// ends it; the handle itself is only held for that lifetime.
    _thread: JoinHandle<()>,
}

impl NvidiaSampler {
    /// A sample older than this is reported as unavailable rather than current.
    ///
    /// Without it, a driver that stops answering would leave the last reading on
    /// screen indefinitely, which is the fabrication the project forbids.
    const MAX_AGE: Duration = Duration::from_secs(3);

    /// Start the watcher. `program` is injectable so tests never invoke the tool.
    pub fn new<P: Into<PathBuf>>(program: P) -> Self {
        let program = program.into();
        let latest: Arc<Mutex<Option<(Instant, GpuTelemetry)>>> = Arc::new(Mutex::new(None));
        let (wake, requests) = mpsc::sync_channel::<()>(1);
        let slot = Arc::clone(&latest);

        let thread = std::thread::Builder::new()
            .name("zenomen-nv-watch".to_string())
            .spawn(move || {
                // Ends when the provider drops the sender.
                while requests.recv().is_ok() {
                    let Some(sample) = query_nvidia_smi(&program) else {
                        continue;
                    };
                    let mut guard = slot.lock().unwrap_or_else(|e| e.into_inner());
                    *guard = Some((Instant::now(), sample));
                }
            })
            .expect("spawning the nvidia-smi watcher must succeed");

        Self {
            latest,
            wake,
            _thread: thread,
        }
    }

    /// Ask for a fresh sample. Never blocks: a pending request absorbs this one.
    pub fn request(&self) {
        let _ = self.wake.try_send(());
    }

    /// The most recent sample, or `None` when there is none or it has gone stale.
    pub fn latest(&self) -> Option<GpuTelemetry> {
        let guard = self.latest.lock().unwrap_or_else(|e| e.into_inner());
        let (at, sample) = guard.as_ref()?;
        (at.elapsed() <= Self::MAX_AGE).then(|| sample.clone())
    }
}

/// Run one `nvidia-smi` query and parse it. Any failure is `None`, never a panic
/// and never a fabricated reading.
fn query_nvidia_smi(program: &Path) -> Option<GpuTelemetry> {
    let output = Command::new(program)
        .args([
            "--query-gpu=name,utilization.gpu,temperature.gpu,memory.used,memory.total",
            "--format=csv,noheader,nounits",
        ])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    parse_nvidia_smi_output(&String::from_utf8_lossy(&output.stdout))
}

/// Parse CSV output from `nvidia-smi`:
/// `NVIDIA GeForce RTX 3060 Laptop GPU, 15, 62, 1200, 6144`
pub fn parse_nvidia_smi_output(output: &str) -> Option<GpuTelemetry> {
    let line = output.lines().next()?.trim();
    if line.is_empty() {
        return None;
    }

    let parts: Vec<&str> = line.split(',').map(str::trim).collect();
    if parts.len() < 5 {
        return None;
    }

    let name = parts[0].to_string();
    let utilization = parts[1].parse::<f32>().ok();
    let temperature_c = parts[2].parse::<f32>().ok();
    let vram_used_mb = parts[3].parse::<u64>().ok();
    let vram_total_mb = parts[4].parse::<u64>().ok();

    Some(GpuTelemetry {
        name,
        utilization,
        temperature_c,
        vram_used_mb,
        vram_total_mb,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn card_device_name_filters_connectors() {
        assert!(is_card_device_name("card0"));
        assert!(is_card_device_name("card1"));
        assert!(is_card_device_name("card12"));
        assert!(!is_card_device_name("card1-DP-1"));
        assert!(!is_card_device_name("card0-eDP-1"));
        assert!(!is_card_device_name("renderD128"));
        assert!(!is_card_device_name("version"));
    }

    #[test]
    fn parses_nvidia_smi_output_correctly() {
        let output = "NVIDIA GeForce RTX 3060 Laptop GPU, 24, 65, 1450, 6144\n";
        let telemetry = parse_nvidia_smi_output(output).expect("must parse");
        assert_eq!(telemetry.name, "NVIDIA GeForce RTX 3060 Laptop GPU");
        assert_eq!(telemetry.utilization, Some(24.0));
        assert_eq!(telemetry.temperature_c, Some(65.0));
        assert_eq!(telemetry.vram_used_mb, Some(1450));
        assert_eq!(telemetry.vram_total_mb, Some(6144));
    }

    #[test]
    fn parses_vendor_id_properly() {
        assert_eq!(GpuVendor::from_pci_vendor_id(0x1002), GpuVendor::Amd);
        assert_eq!(GpuVendor::from_pci_vendor_id(0x8086), GpuVendor::Intel);
        assert_eq!(GpuVendor::from_pci_vendor_id(0x10de), GpuVendor::Nvidia);
        assert_eq!(
            GpuVendor::from_pci_vendor_id(0x1234),
            GpuVendor::Unknown(0x1234)
        );
    }
}
