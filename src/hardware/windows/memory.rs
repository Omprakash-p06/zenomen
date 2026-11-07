//! Windows RAM telemetry sampling via GlobalMemoryStatusEx (WIN-01).

/// Calculate RAM usage in megabytes from physical memory byte counts (WIN-01).
///
/// Uses `ullTotalPhys` and `ullAvailPhys` to determine:
/// - `total_mb = ullTotalPhys / (1024 * 1024)`
/// - `avail_mb = ullAvailPhys / (1024 * 1024)`
/// - `used_mb = total_mb.saturating_sub(avail_mb)`
pub fn calculate_ram_usage(total_phys: u64, avail_phys: u64) -> (Option<u32>, Option<u32>) {
    if total_phys == 0 {
        return (None, None);
    }
    let total_mb = (total_phys / (1024 * 1024)) as u32;
    let avail_mb = (avail_phys / (1024 * 1024)) as u32;
    let used_mb = total_mb.saturating_sub(avail_mb);
    (Some(used_mb), Some(total_mb))
}

/// Query system physical memory usage using Win32 `GlobalMemoryStatusEx`.
pub fn read_ram_usage() -> (Option<u32>, Option<u32>) {
    #[cfg(windows)]
    {
        use windows_sys::Win32::System::SystemInformation::{GlobalMemoryStatusEx, MEMORYSTATUSEX};

        unsafe {
            let mut status: MEMORYSTATUSEX = std::mem::zeroed();
            status.dwLength = std::mem::size_of::<MEMORYSTATUSEX>() as u32;

            if GlobalMemoryStatusEx(&mut status) != 0 {
                calculate_ram_usage(status.ullTotalPhys, status.ullAvailPhys)
            } else {
                (None, None)
            }
        }
    }
    #[cfg(not(windows))]
    {
        (None, None)
    }
}
