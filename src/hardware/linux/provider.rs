//! Linux hardware provider implementation.
//!
//! Samples real system telemetry directly from the Linux kernel via procfs,
//! sysfs, dynamic DRM cards, and non-blocking external tool queries (MON-01..MON-08).

use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::Duration;

use crate::hardware::linux::cpu::{
    CpuSampler, read_cpu_freq_from_cpuinfo, read_cpu_freq_mhz, read_cpu_model,
};
use crate::hardware::linux::gpu::{
    GpuDevice, GpuVendor, enumerate_drm_gpus, sample_amd_gpu, sample_nvidia_smi,
};
use crate::hardware::linux::memory::read_memory_snapshot;
use crate::hardware::{HardwareCapabilities, HardwareProvider, TelemetrySnapshot};

/// Provider reading telemetry from standard Linux kernel sysfs and procfs interfaces.
pub struct LinuxHardwareProvider {
    cpu_sampler: Mutex<CpuSampler>,
    cpu_model: String,
    cpufreq_path: PathBuf,
    gpu_devices: Vec<GpuDevice>,
    capabilities: HardwareCapabilities,
}

impl LinuxHardwareProvider {
    /// Initialize the Linux provider, scanning available hardware interfaces.
    pub fn new() -> Self {
        Self::with_custom_paths(
            Path::new("/sys/class/drm"),
            Path::new("/sys/devices/system/cpu/cpu0/cpufreq/scaling_cur_freq"),
        )
    }

    /// Construct with custom paths for testing or specialized environments.
    pub fn with_custom_paths(drm_dir: &Path, cpufreq_path: &Path) -> Self {
        let cpuinfo = std::fs::read_to_string("/proc/cpuinfo").unwrap_or_default();
        let cpu_model = read_cpu_model(&cpuinfo).unwrap_or_else(|| "Unknown CPU".to_string());

        let gpu_devices = enumerate_drm_gpus(drm_dir);

        let has_cpu = !cpu_model.is_empty();
        let has_gpu = !gpu_devices.is_empty() || Path::new("/usr/bin/nvidia-smi").exists();
        let has_ram = Path::new("/proc/meminfo").exists();

        let capabilities = HardwareCapabilities {
            cpu_metrics: has_cpu,
            gpu_metrics: has_gpu,
            ram_metrics: has_ram,
            fan_rpm: false,        // Wired in Plan 02-02 via SensorRegistry
            fan_control: false,    // Wired in Phase 3
            power_profiles: false, // Wired in Phase 3
            rgb_lighting: false,   // Wired in Phase 4
            battery_metrics: false,
        };

        Self {
            cpu_sampler: Mutex::new(CpuSampler::new()),
            cpu_model,
            cpufreq_path: cpufreq_path.to_path_buf(),
            gpu_devices,
            capabilities,
        }
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

        // 1. CPU Telemetry (MON-01, MON-02, MON-03)
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

        // 2. RAM Telemetry (MON-07)
        if let Ok(mem) = read_memory_snapshot() {
            snapshot.ram_total_mb = mem.total_mb;
            snapshot.ram_used_mb = mem.used_mb;
        }

        // 3. GPU Telemetry (MON-05, MON-06)
        // Check NVIDIA first if present (often discrete gaming GPU), then AMD/Intel DRM
        let mut gpu_telemetry = None;

        // Try NVIDIA non-blocking query with 200ms timeout
        if let Some(nv) = sample_nvidia_smi(Duration::from_millis(200)) {
            gpu_telemetry = Some(nv);
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

        Ok(snapshot)
    }
}
