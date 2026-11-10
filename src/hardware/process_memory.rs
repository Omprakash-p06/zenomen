//! ZenOmen process memory telemetry (WIN-08 / MON-12).
//!
//! Exposes Working Set, Private Bytes, and Peak Working Set for:
//! 1. ZenOmen GUI application process.
//! 2. `zenomen-hw` hardware control service process.
//! 3. Combined ZenOmen footprint.

use serde::{Deserialize, Serialize};

/// Role of a monitored ZenOmen process.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProcessRole {
    Gui,
    HardwareService,
}

impl ProcessRole {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Gui => "gui",
            Self::HardwareService => "hardware_service",
        }
    }
}

/// Memory counters for a single ZenOmen process.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ProcessMemoryInfo {
    pub role: Option<ProcessRole>,
    pub pid: u32,
    pub working_set_bytes: u64,
    pub private_bytes: u64,
    pub peak_working_set_bytes: u64,
    pub timestamp_secs: u64,
    pub available: bool,
}

impl ProcessMemoryInfo {
    /// Return working set in megabytes (MB = 1024 * 1024 bytes).
    pub fn working_set_mb(&self) -> f64 {
        self.working_set_bytes as f64 / (1024.0 * 1024.0)
    }

    /// Return private usage in megabytes.
    pub fn private_mb(&self) -> f64 {
        self.private_bytes as f64 / (1024.0 * 1024.0)
    }

    /// Return peak working set in megabytes.
    pub fn peak_working_set_mb(&self) -> f64 {
        self.peak_working_set_bytes as f64 / (1024.0 * 1024.0)
    }

    pub fn format_working_set(&self) -> String {
        if !self.available {
            "Unavailable".to_string()
        } else {
            format!("{:.1} MB", self.working_set_mb())
        }
    }

    pub fn format_private(&self) -> String {
        if !self.available {
            "Unavailable".to_string()
        } else {
            format!("{:.1} MB", self.private_mb())
        }
    }

    pub fn format_peak(&self) -> String {
        if !self.available {
            "Unavailable".to_string()
        } else {
            format!("{:.1} MB", self.peak_working_set_mb())
        }
    }
}

/// Combined ZenOmen memory telemetry across both processes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ZenomenMemoryTelemetry {
    pub gui: ProcessMemoryInfo,
    pub service: ProcessMemoryInfo,
    pub combined_working_set_bytes: Option<u64>,
    pub combined_private_bytes: Option<u64>,
    pub combined_peak_working_set_bytes: Option<u64>,
    pub is_service_available: bool,
}

impl ZenomenMemoryTelemetry {
    /// Combine GUI and Hardware Service memory measurements.
    ///
    /// Requires both processes to be available and fresh (within 10 seconds).
    /// If the service is offline or stale, service is marked unavailable and
    /// combined metrics are withheld (`None`).
    pub fn new(gui: ProcessMemoryInfo, service: ProcessMemoryInfo) -> Self {
        let is_service_available = service.available && service.pid != 0;

        let (combined_ws, combined_priv, combined_peak) = if gui.available && is_service_available {
            let time_diff = (gui.timestamp_secs as i64 - service.timestamp_secs as i64).abs();
            if time_diff <= 10 {
                (
                    Some(
                        gui.working_set_bytes
                            .saturating_add(service.working_set_bytes),
                    ),
                    Some(gui.private_bytes.saturating_add(service.private_bytes)),
                    Some(
                        gui.peak_working_set_bytes
                            .saturating_add(service.peak_working_set_bytes),
                    ),
                )
            } else {
                // Stale measurements cannot produce honest combined figures
                (None, None, None)
            }
        } else {
            (None, None, None)
        };

        Self {
            gui,
            service,
            combined_working_set_bytes: combined_ws,
            combined_private_bytes: combined_priv,
            combined_peak_working_set_bytes: combined_peak,
            is_service_available,
        }
    }

    pub fn total_working_set_str(&self) -> String {
        match self.combined_working_set_bytes {
            Some(b) => format!("{:.1} MB", b as f64 / (1024.0 * 1024.0)),
            None => {
                if self.gui.available {
                    format!("{:.1} MB (App only)", self.gui.working_set_mb())
                } else {
                    "Unavailable".to_string()
                }
            }
        }
    }

    pub fn total_private_str(&self) -> String {
        match self.combined_private_bytes {
            Some(b) => format!("{:.1} MB", b as f64 / (1024.0 * 1024.0)),
            None => {
                if self.gui.available {
                    format!("{:.1} MB (App only)", self.gui.private_mb())
                } else {
                    "Unavailable".to_string()
                }
            }
        }
    }

    pub fn total_peak_str(&self) -> String {
        match self.combined_peak_working_set_bytes {
            Some(b) => format!("{:.1} MB", b as f64 / (1024.0 * 1024.0)),
            None => {
                if self.gui.available {
                    format!("{:.1} MB (App only)", self.gui.peak_working_set_mb())
                } else {
                    "Unavailable".to_string()
                }
            }
        }
    }
}

/// Sample process memory counters for the current running process.
pub fn sample_current_process_memory(role: ProcessRole) -> ProcessMemoryInfo {
    let pid = std::process::id();
    let timestamp_secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    #[cfg(windows)]
    {
        use windows_sys::Win32::System::ProcessStatus::{
            K32GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS_EX,
        };
        use windows_sys::Win32::System::Threading::GetCurrentProcess;

        unsafe {
            let mut counters: PROCESS_MEMORY_COUNTERS_EX = std::mem::zeroed();
            counters.cb = std::mem::size_of::<PROCESS_MEMORY_COUNTERS_EX>() as u32;

            let handle = GetCurrentProcess();
            let success =
                K32GetProcessMemoryInfo(handle, &mut counters as *mut _ as *mut _, counters.cb);

            if success != 0 {
                ProcessMemoryInfo {
                    role: Some(role),
                    pid,
                    working_set_bytes: counters.WorkingSetSize as u64,
                    private_bytes: counters.PrivateUsage as u64,
                    peak_working_set_bytes: counters.PeakWorkingSetSize as u64,
                    timestamp_secs,
                    available: true,
                }
            } else {
                ProcessMemoryInfo {
                    role: Some(role),
                    pid,
                    timestamp_secs,
                    available: false,
                    ..Default::default()
                }
            }
        }
    }

    #[cfg(target_os = "linux")]
    {
        if let Ok(status) = std::fs::read_to_string("/proc/self/status") {
            let mut ws_kb = 0u64;
            let mut peak_kb = 0u64;
            let mut priv_kb = 0u64;
            for line in status.lines() {
                if let Some(rest) = line.strip_prefix("VmRSS:") {
                    ws_kb = rest
                        .split_whitespace()
                        .next()
                        .and_then(|v| v.parse().ok())
                        .unwrap_or(0);
                } else if let Some(rest) = line.strip_prefix("VmHWM:") {
                    peak_kb = rest
                        .split_whitespace()
                        .next()
                        .and_then(|v| v.parse().ok())
                        .unwrap_or(0);
                } else if let Some(rest) = line.strip_prefix("VmData:") {
                    priv_kb = rest
                        .split_whitespace()
                        .next()
                        .and_then(|v| v.parse().ok())
                        .unwrap_or(0);
                }
            }
            ProcessMemoryInfo {
                role: Some(role),
                pid,
                working_set_bytes: ws_kb * 1024,
                private_bytes: priv_kb * 1024,
                peak_working_set_bytes: peak_kb * 1024,
                timestamp_secs,
                available: ws_kb > 0,
            }
        } else {
            ProcessMemoryInfo {
                role: Some(role),
                pid,
                timestamp_secs,
                available: false,
                ..Default::default()
            }
        }
    }

    #[cfg(not(any(windows, target_os = "linux")))]
    {
        ProcessMemoryInfo {
            role: Some(role),
            pid,
            timestamp_secs,
            available: false,
            ..Default::default()
        }
    }
}

/// Sample process memory counters for a target process by PID.
pub fn sample_process_memory_by_pid(
    pid: u32,
    role: ProcessRole,
) -> Result<ProcessMemoryInfo, String> {
    if pid == 0 {
        return Err("Invalid PID 0".to_string());
    }

    let timestamp_secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    #[cfg(windows)]
    {
        use windows_sys::Win32::Foundation::CloseHandle;
        use windows_sys::Win32::System::ProcessStatus::{
            K32GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS_EX,
        };
        use windows_sys::Win32::System::Threading::{
            OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION,
        };

        unsafe {
            let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
            if handle.is_null() {
                return Err(format!("OpenProcess failed for PID {pid}"));
            }
            let mut counters: PROCESS_MEMORY_COUNTERS_EX = std::mem::zeroed();
            counters.cb = std::mem::size_of::<PROCESS_MEMORY_COUNTERS_EX>() as u32;

            let success =
                K32GetProcessMemoryInfo(handle, &mut counters as *mut _ as *mut _, counters.cb);
            CloseHandle(handle);

            if success != 0 {
                Ok(ProcessMemoryInfo {
                    role: Some(role),
                    pid,
                    working_set_bytes: counters.WorkingSetSize as u64,
                    private_bytes: counters.PrivateUsage as u64,
                    peak_working_set_bytes: counters.PeakWorkingSetSize as u64,
                    timestamp_secs,
                    available: true,
                })
            } else {
                Err(format!("GetProcessMemoryInfo failed for PID {pid}"))
            }
        }
    }

    #[cfg(target_os = "linux")]
    {
        let path = format!("/proc/{pid}/status");
        if let Ok(status) = std::fs::read_to_string(&path) {
            let mut ws_kb = 0u64;
            let mut peak_kb = 0u64;
            let mut priv_kb = 0u64;
            for line in status.lines() {
                if let Some(rest) = line.strip_prefix("VmRSS:") {
                    ws_kb = rest
                        .split_whitespace()
                        .next()
                        .and_then(|v| v.parse().ok())
                        .unwrap_or(0);
                } else if let Some(rest) = line.strip_prefix("VmHWM:") {
                    peak_kb = rest
                        .split_whitespace()
                        .next()
                        .and_then(|v| v.parse().ok())
                        .unwrap_or(0);
                } else if let Some(rest) = line.strip_prefix("VmData:") {
                    priv_kb = rest
                        .split_whitespace()
                        .next()
                        .and_then(|v| v.parse().ok())
                        .unwrap_or(0);
                }
            }
            Ok(ProcessMemoryInfo {
                role: Some(role),
                pid,
                working_set_bytes: ws_kb * 1024,
                private_bytes: priv_kb * 1024,
                peak_working_set_bytes: peak_kb * 1024,
                timestamp_secs,
                available: ws_kb > 0,
            })
        } else {
            Err(format!("Failed to read {path}"))
        }
    }

    #[cfg(not(any(windows, target_os = "linux")))]
    {
        Err("Unsupported platform for process memory querying".to_string())
    }
}
