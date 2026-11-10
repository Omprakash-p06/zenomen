//! Linux hardware provider implementation.
//!
//! Samples real system telemetry directly from the Linux kernel via procfs,
//! sysfs, dynamic DRM cards, and non-blocking external tool queries (MON-01..MON-08),
//! sharing sensor path bindings via [`SensorRegistry`] (MON-12).

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use crate::hardware::linux::cpu::{
    CpuSampler, read_cpu_freq_from_cpuinfo, read_cpu_freq_mhz, read_cpu_model,
};
use crate::hardware::linux::gpu::{
    GpuDevice, GpuVendor, NvidiaSampler, enumerate_drm_gpus, sample_amd_gpu,
};
use crate::hardware::linux::memory::read_memory_snapshot;
use crate::hardware::registry::SensorRegistry;
use crate::hardware::{HardwareCapabilities, HardwareProvider, TelemetrySnapshot};

/// Provider reading telemetry from standard Linux kernel sysfs and procfs interfaces.
pub struct LinuxHardwareProvider {
    cpu_sampler: Mutex<CpuSampler>,
    cpu_model: String,
    cpufreq_path: PathBuf,
    acpi_root: PathBuf,
    power_supply_root: PathBuf,
    gpu_devices: Vec<GpuDevice>,
    /// Watcher for `nvidia-smi`, present only when an NVIDIA GPU or the tool was
    /// detected. One thread for the provider's lifetime, not one per sample.
    nvidia: Option<NvidiaSampler>,
    registry: Arc<SensorRegistry>,
    capabilities: HardwareCapabilities,
}

impl LinuxHardwareProvider {
    /// Initialize the Linux provider, scanning available hardware interfaces and sensors.
    pub fn new() -> Self {
        Self::with_all_roots(
            Path::new("/sys/class/drm"),
            Path::new("/sys/devices/system/cpu/cpu0/cpufreq/scaling_cur_freq"),
            Path::new("/sys/class/hwmon"),
            Path::new("/sys/firmware/acpi"),
        )
    }

    /// Construct with custom paths for testing or specialized environments.
    pub fn with_custom_paths(drm_dir: &Path, cpufreq_path: &Path) -> Self {
        Self::with_all_roots(
            drm_dir,
            cpufreq_path,
            Path::new("/sys/class/hwmon"),
            Path::new("/sys/firmware/acpi"),
        )
    }

    /// Construct with custom roots for all sysfs subsystems.
    pub fn with_all_roots(
        drm_dir: &Path,
        cpufreq_path: &Path,
        hwmon_root: &Path,
        acpi_root: &Path,
    ) -> Self {
        let cpuinfo = std::fs::read_to_string("/proc/cpuinfo").unwrap_or_default();
        let cpu_model = read_cpu_model(&cpuinfo).unwrap_or_else(|| "Unknown CPU".to_string());

        let gpu_devices = enumerate_drm_gpus(drm_dir);
        let has_nvidia = gpu_devices.iter().any(|d| d.vendor == GpuVendor::Nvidia)
            || Path::new("/usr/bin/nvidia-smi").exists()
            || Path::new("/bin/nvidia-smi").exists();

        let has_cpu = !cpu_model.is_empty();
        let has_gpu = !gpu_devices.is_empty() || has_nvidia;
        let has_ram = Path::new("/proc/meminfo").exists();
        let nvidia = has_nvidia.then(|| NvidiaSampler::new("nvidia-smi"));

        let registry = Arc::new(SensorRegistry::scan_roots(hwmon_root, acpi_root));
        let capabilities = registry.detect_capabilities(has_cpu, has_gpu, has_ram);

        let power_supply_root = hwmon_root
            .parent()
            .map(|p| p.join("power_supply"))
            .unwrap_or_else(|| PathBuf::from("/sys/class/power_supply"));

        Self {
            cpu_sampler: Mutex::new(CpuSampler::new()),
            cpu_model,
            cpufreq_path: cpufreq_path.to_path_buf(),
            acpi_root: acpi_root.to_path_buf(),
            power_supply_root,
            gpu_devices,
            nvidia,
            registry,
            capabilities,
        }
    }

    /// Canonical sensor registry shared between dashboard and diagnostics (MON-12).
    pub fn registry(&self) -> &Arc<SensorRegistry> {
        &self.registry
    }

    /// Discovered GPU devices.
    pub fn gpu_devices(&self) -> &[GpuDevice] {
        &self.gpu_devices
    }

    /// Update runtime capabilities.
    pub fn set_capabilities(&mut self, capabilities: HardwareCapabilities) {
        self.capabilities = capabilities;
    }
}

impl Default for LinuxHardwareProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl HardwareProvider for LinuxHardwareProvider {
    fn capabilities(&self) -> HardwareCapabilities {
        self.capabilities
    }

    fn sample_telemetry(&self) -> Result<TelemetrySnapshot, String> {
        let mut snapshot = TelemetrySnapshot {
            cpu_name: self.cpu_model.clone(),
            ..Default::default()
        };

        // 1. CPU Telemetry (MON-01, MON-02, MON-03, MON-04)
        if let Ok(stat_content) = std::fs::read_to_string("/proc/stat") {
            let mut sampler = self
                .cpu_sampler
                .lock()
                .map_err(|e| format!("mutex poisoned: {e}"))?;
            let (aggregate_pct, _per_core) = sampler.sample_proc_stat(&stat_content);
            snapshot.cpu_utilization = aggregate_pct;
        }

        // Frequency: try cpufreq sysfs first, then fallback to /proc/cpuinfo
        snapshot.cpu_frequency_mhz = read_cpu_freq_mhz(&self.cpufreq_path)
            .or_else(|| {
                std::fs::read_to_string("/proc/cpuinfo")
                    .ok()
                    .and_then(|info| read_cpu_freq_from_cpuinfo(&info))
            })
            .unwrap_or(0);

        // CPU temperature from canonical SensorRegistry (MON-04, MON-12)
        snapshot.cpu_temperature_c = self.registry.read_cpu_temp();

        // 2. RAM Telemetry (MON-07)
        if let Ok(mem) = read_memory_snapshot() {
            snapshot.ram_total_mb = mem.total_mb;
            snapshot.ram_used_mb = mem.used_mb;
        }

        // 3. Fan Tachometers, Fan Mode & Power Profiles
        snapshot.fan_rpms = self.registry.read_fan_rpms();
        if let Some(mode) = self.registry.read_fan_mode() {
            snapshot.fan_mode = mode;
        }
        if let Some(profile) = self.registry.read_active_power_profile(&self.acpi_root) {
            snapshot.power_profile = profile;
        }
        snapshot.power_profiles = self.registry.read_power_profile_choices(&self.acpi_root);

        // 4. GPU Telemetry (MON-05, MON-06)
        // Check NVIDIA first if present (often discrete gaming GPU), then AMD/Intel DRM
        let mut gpu_telemetry = None;

        // Ask the NVIDIA watcher for a sample and take whatever it has. The tick
        // never waits on `nvidia-smi`; a stale sample reads as unavailable.
        if let Some(nvidia) = &self.nvidia {
            nvidia.request();
            if let Some(nv) = nvidia.latest() {
                gpu_telemetry = Some(nv);
            }
        }

        // If no NVIDIA reading, sample discovered DRM devices
        if gpu_telemetry.is_none() {
            for device in &self.gpu_devices {
                match device.vendor {
                    GpuVendor::Amd => {
                        gpu_telemetry = Some(sample_amd_gpu(device));
                        break;
                    }
                    GpuVendor::Intel => {
                        // Intel DRM telemetry fallback
                        gpu_telemetry = Some(crate::hardware::linux::gpu::GpuTelemetry {
                            name: device.name.clone(),
                            utilization: None,
                            temperature_c: None,
                            vram_used_mb: None,
                            vram_total_mb: None,
                        });
                        break;
                    }
                    _ => {}
                }
            }
        }

        if let Some(gpu) = gpu_telemetry {
            snapshot.gpu_name = Some(gpu.name);
            snapshot.gpu_utilization = gpu.utilization;
            snapshot.gpu_temperature_c = gpu.temperature_c;
        }

        // 5. Battery Telemetry (MON-01, MON-02)
        snapshot.battery = sample_battery_telemetry(&self.power_supply_root);

        Ok(snapshot)
    }
}

/// Sample battery status and charge metrics from `/sys/class/power_supply/BAT*`.
fn sample_battery_telemetry(power_supply_dir: &Path) -> Option<crate::hardware::BatteryTelemetry> {
    let entries = std::fs::read_dir(power_supply_dir).ok()?;
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with("BAT") {
            let bat_dir = entry.path();
            let capacity = std::fs::read_to_string(bat_dir.join("capacity"))
                .ok()
                .and_then(|s| s.trim().parse::<u8>().ok())
                .map(|c| c.min(100));

            let status_str = std::fs::read_to_string(bat_dir.join("status"))
                .ok()
                .map(|s| s.trim().to_string());

            let charging = status_str
                .as_ref()
                .map(|s| s.eq_ignore_ascii_case("Charging"));

            let remaining_seconds = std::fs::read_to_string(bat_dir.join("time_to_empty_now"))
                .or_else(|_| std::fs::read_to_string(bat_dir.join("time_to_full_now")))
                .ok()
                .and_then(|s| s.trim().parse::<u64>().ok())
                .map(|s| s as u32);

            return Some(crate::hardware::BatteryTelemetry {
                present: true,
                charge_percent: capacity,
                charging,
                remaining_seconds,
            });
        }
    }
    None
}
