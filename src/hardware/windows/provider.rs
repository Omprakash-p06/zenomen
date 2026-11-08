//! Windows native hardware provider implementation (WIN-01).

use std::sync::Mutex;

use crate::hardware::provider::{HardwareCapabilities, HardwareProvider, TelemetrySnapshot};
use crate::hardware::windows::{battery, cpu, gpu, memory, sensors};

/// Native hardware monitoring provider for Windows 10/11 systems (WIN-01).
pub struct WindowsHardwareProvider {
    cpu_sampler: Mutex<cpu::WindowsCpuSampler>,
    cpu_model: String,
    primary_gpu: Option<gpu::GpuAdapterInfo>,
    gpu_collector: Mutex<gpu::GpuPerformanceCollector>,
    has_battery: bool,
}

impl WindowsHardwareProvider {
    /// Create a new `WindowsHardwareProvider` initializing hardware discovery.
    pub fn new() -> Self {
        let cpu_model = cpu::read_cpu_model().unwrap_or_else(|| "Unknown CPU".to_string());

        let adapters = gpu::enumerate_gpu_adapters();
        // Prefer discrete GPU if present, else first adapter
        let primary_gpu = adapters
            .iter()
            .find(|a| a.is_discrete())
            .or_else(|| adapters.first())
            .cloned();

        let has_battery = battery::read_battery_presence();

        #[allow(unused_mut)]
        let mut sampler = cpu::WindowsCpuSampler::new();
        #[cfg(windows)]
        if let Some(initial_times) = cpu::query_system_times() {
            sampler.record_initial(initial_times);
        }

        Self {
            cpu_sampler: Mutex::new(sampler),
            cpu_model,
            primary_gpu,
            gpu_collector: Mutex::new(gpu::GpuPerformanceCollector::new()),
            has_battery,
        }
    }
}

impl Default for WindowsHardwareProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl HardwareProvider for WindowsHardwareProvider {
    fn capabilities(&self) -> HardwareCapabilities {
        HardwareCapabilities {
            cpu_metrics: true,
            gpu_metrics: self.primary_gpu.is_some(),
            ram_metrics: true,
            fan_rpm: false,
            fan_control: false,
            power_profiles: false,
            rgb_lighting: false,
            rgb_variant: None,
            battery_metrics: self.has_battery,
        }
    }

    fn sample_telemetry(&self) -> Result<TelemetrySnapshot, String> {
        let mut sampler = self
            .cpu_sampler
            .lock()
            .map_err(|e| format!("CPU sampler lock poisoned: {e}"))?;

        let cpu_utilization = cpu::read_cpu_utilization(&mut sampler).unwrap_or(0.0);
        let cpu_frequency_mhz = cpu::read_cpu_frequency().unwrap_or(0.0) as u32;

        let (ram_used_mb, ram_total_mb) = memory::read_ram_usage();
        let (cpu_temp_c, gpu_temp_c) = sensors::read_temperature_sensors();

        let mut gpu_collector = self
            .gpu_collector
            .lock()
            .map_err(|e| format!("GPU collector lock poisoned: {e}"))?;

        let (gpu_name, gpu_utilization) = if let Some(ref gpu) = self.primary_gpu {
            (Some(gpu.name.clone()), gpu_collector.sample_utilization())
        } else {
            (None, None)
        };

        let battery = battery::sample_battery_telemetry();

        Ok(TelemetrySnapshot {
            cpu_name: self.cpu_model.clone(),
            cpu_utilization,
            cpu_temperature_c: cpu_temp_c,
            cpu_frequency_mhz,
            gpu_name,
            gpu_utilization,
            gpu_temperature_c: gpu_temp_c,
            ram_total_mb: ram_total_mb.unwrap_or(0) as u64,
            ram_used_mb: ram_used_mb.unwrap_or(0) as u64,
            fan_rpms: sensors::read_fan_rpms(),
            fan_mode: String::new(),
            power_profile: String::new(),
            power_profiles: Vec::new(),
            rgb_profile: None,
            battery,
        })
    }
}
