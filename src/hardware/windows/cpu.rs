//! Windows CPU telemetry and frequency sampling (WIN-01).

/// Raw CPU kernel, user, and idle time counters.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct WindowsCpuTimes {
    pub idle: u64,
    pub kernel: u64,
    pub user: u64,
}

/// State tracker for calculating CPU utilization across delta intervals.
#[derive(Debug, Clone, Copy, Default)]
pub struct WindowsCpuSampler {
    prev_times: Option<WindowsCpuTimes>,
}

impl WindowsCpuSampler {
    pub fn new() -> Self {
        Self::default()
    }

    /// Reset sampler state.
    pub fn reset(&mut self) {
        self.prev_times = None;
    }

    /// Update with current system times and calculate utilization percentage.
    pub fn sample_with_times(&mut self, current: WindowsCpuTimes) -> Option<f32> {
        let prev = self.prev_times?;
        self.prev_times = Some(current);
        calculate_cpu_utilization(prev, current)
    }

    /// Record initial sample if no previous times exist.
    pub fn record_initial(&mut self, current: WindowsCpuTimes) {
        if self.prev_times.is_none() {
            self.prev_times = Some(current);
        }
    }
}

/// Calculate CPU utilization from two consecutive Windows `GetSystemTimes` samples (WIN-01).
///
/// Formula:
/// $$\Delta \text{Kernel} = \text{current.kernel} - \text{prev.kernel}$$
/// $$\Delta \text{User} = \text{current.user} - \text{prev.user}$$
/// $$\Delta \text{Idle} = \text{current.idle} - \text{prev.idle}$$
/// $$\Delta \text{Total} = \Delta \text{Kernel} + \Delta \text{User}$$
/// $$\text{Util} = 100.0 \times \left(1.0 - \frac{\Delta \text{Idle}}{\Delta \text{Total}}\right)$$
///
/// Note: On Windows, `KernelTime` returned by `GetSystemTimes` includes `IdleTime`.
/// Therefore $\Delta \text{Total} = \Delta \text{Kernel} + \Delta \text{User}$, and
/// busy time is $\Delta \text{Total} - \Delta \text{Idle}$.
pub fn calculate_cpu_utilization(prev: WindowsCpuTimes, current: WindowsCpuTimes) -> Option<f32> {
    let delta_kernel = current.kernel.checked_sub(prev.kernel)?;
    let delta_user = current.user.checked_sub(prev.user)?;
    let delta_idle = current.idle.checked_sub(prev.idle)?;

    let delta_total = delta_kernel.checked_add(delta_user)?;
    if delta_total == 0 || delta_idle > delta_total {
        return None;
    }

    let busy = delta_total - delta_idle;
    let utilization = (busy as f64 / delta_total as f64) * 100.0;
    Some((utilization as f32).clamp(0.0, 100.0))
}

#[cfg(windows)]
pub fn query_system_times() -> Option<WindowsCpuTimes> {
    use windows_sys::Win32::Foundation::FILETIME;
    use windows_sys::Win32::System::Threading::GetSystemTimes;

    let mut idle = FILETIME {
        dwLowDateTime: 0,
        dwHighDateTime: 0,
    };
    let mut kernel = FILETIME {
        dwLowDateTime: 0,
        dwHighDateTime: 0,
    };
    let mut user = FILETIME {
        dwLowDateTime: 0,
        dwHighDateTime: 0,
    };

    let ok = unsafe { GetSystemTimes(&mut idle, &mut kernel, &mut user) };
    if ok == 0 {
        return None;
    }

    let filetime_to_u64 =
        |ft: FILETIME| -> u64 { ((ft.dwHighDateTime as u64) << 32) | (ft.dwLowDateTime as u64) };

    Some(WindowsCpuTimes {
        idle: filetime_to_u64(idle),
        kernel: filetime_to_u64(kernel),
        user: filetime_to_u64(user),
    })
}

/// Read current CPU utilization percentage using `GetSystemTimes` (WIN-01).
pub fn read_cpu_utilization(sampler: &mut WindowsCpuSampler) -> Option<f32> {
    #[cfg(windows)]
    {
        let times = query_system_times()?;
        if sampler.prev_times.is_none() {
            sampler.record_initial(times);
            return None;
        }
        sampler.sample_with_times(times)
    }
    #[cfg(not(windows))]
    {
        let _ = sampler;
        None
    }
}

/// Read CPU frequency in MHz via Windows power information.
pub fn read_cpu_frequency() -> Option<f32> {
    #[cfg(windows)]
    {
        // Try CallNtPowerInformation with ProcessorInformation (11)
        #[repr(C)]
        struct ProcessorPowerInformation {
            number: u32,
            max_mhz: u32,
            current_mhz: u32,
            mhz_limit: u32,
            max_idle_state: u32,
            current_idle_state: u32,
        }

        type CallNtPowerInformationFn = unsafe extern "system" fn(
            information_level: u32,
            input_buffer: *const std::ffi::c_void,
            input_buffer_length: u32,
            output_buffer: *mut std::ffi::c_void,
            output_buffer_length: u32,
        ) -> i32;

        unsafe {
            use windows_sys::Win32::Foundation::FreeLibrary;
            use windows_sys::Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryA};
            let module = LoadLibraryA(c"powrprof.dll".as_ptr() as *const u8);
            if module.is_null() {
                return None;
            }

            let mut mhz = None;
            let func = GetProcAddress(module, c"CallNtPowerInformation".as_ptr() as *const u8);
            if let Some(call_power_info) = func {
                let call: CallNtPowerInformationFn = std::mem::transmute(call_power_info);
                let mut info: ProcessorPowerInformation = std::mem::zeroed();
                let status = call(
                    11, // ProcessorInformation
                    std::ptr::null(),
                    0,
                    &mut info as *mut _ as *mut std::ffi::c_void,
                    std::mem::size_of::<ProcessorPowerInformation>() as u32,
                );
                if status >= 0 && info.current_mhz > 0 {
                    mhz = Some(info.current_mhz as f32);
                }
            }
            FreeLibrary(module);
            mhz
        }
    }
    #[cfg(not(windows))]
    {
        None
    }
}

/// Read CPU model name from registry or CPUID.
pub fn read_cpu_model() -> Option<String> {
    #[cfg(windows)]
    {
        use windows_sys::Win32::System::Registry::{
            HKEY_LOCAL_MACHINE, KEY_READ, RegCloseKey, RegOpenKeyExA, RegQueryValueExA,
        };

        unsafe {
            let subkey = c"HARDWARE\\DESCRIPTION\\System\\CentralProcessor\\0";
            let value_name = c"ProcessorNameString";
            let mut hkey = std::ptr::null_mut();

            if RegOpenKeyExA(
                HKEY_LOCAL_MACHINE,
                subkey.as_ptr() as *const u8,
                0,
                KEY_READ,
                &mut hkey,
            ) == 0
            {
                let mut buf = [0u8; 256];
                let mut buf_len = buf.len() as u32;
                let mut val_type = 0u32;

                let query_res = RegQueryValueExA(
                    hkey,
                    value_name.as_ptr() as *const u8,
                    std::ptr::null_mut(),
                    &mut val_type,
                    buf.as_mut_ptr(),
                    &mut buf_len,
                );
                RegCloseKey(hkey);

                if query_res == 0 && buf_len > 0 {
                    let nul_pos = buf[..buf_len as usize]
                        .iter()
                        .position(|&b| b == 0)
                        .unwrap_or(buf_len as usize);
                    if let Ok(name) = std::str::from_utf8(&buf[..nul_pos]) {
                        let trimmed = name.trim();
                        if !trimmed.is_empty() {
                            return Some(trimmed.to_string());
                        }
                    }
                }
            }
        }
    }

    // Fallback: CPUID brand string query (standard on x86_64)
    #[cfg(target_arch = "x86_64")]
    {
        let highest_ext = std::arch::x86_64::__cpuid(0x80000000).eax;
        if highest_ext >= 0x80000004 {
            let mut brand_bytes = [0u8; 48];
            for i in 0..3 {
                let leaf = 0x80000002 + i;
                let cpuid = std::arch::x86_64::__cpuid(leaf);
                let offset = (i as usize) * 16;
                brand_bytes[offset..offset + 4].copy_from_slice(&cpuid.eax.to_le_bytes());
                brand_bytes[offset + 4..offset + 8].copy_from_slice(&cpuid.ebx.to_le_bytes());
                brand_bytes[offset + 8..offset + 12].copy_from_slice(&cpuid.ecx.to_le_bytes());
                brand_bytes[offset + 12..offset + 16].copy_from_slice(&cpuid.edx.to_le_bytes());
            }
            if let Ok(brand_str) = std::str::from_utf8(&brand_bytes) {
                let cleaned = brand_str.trim_matches(char::from(0)).trim();
                if !cleaned.is_empty() {
                    return Some(cleaned.to_string());
                }
            }
        }
    }

    None
}
