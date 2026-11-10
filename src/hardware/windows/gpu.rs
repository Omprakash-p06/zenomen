//! Windows GPU discovery, adapter enumeration, and telemetry (WIN-01).

/// Recognized discrete and integrated GPU vendors.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GpuVendor {
    Nvidia,
    Amd,
    Intel,
    Unknown(u32),
}

/// Recognize GPU vendor from standard PCI Vendor ID (WIN-01).
pub fn recognize_gpu_vendor(vendor_id: u32) -> GpuVendor {
    match vendor_id {
        0x10DE => GpuVendor::Nvidia,
        0x1002 => GpuVendor::Amd,
        0x8086 => GpuVendor::Intel,
        other => GpuVendor::Unknown(other),
    }
}

/// Hardware information for an enumerated display adapter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GpuAdapterInfo {
    pub name: String,
    pub vendor_id: u32,
    pub device_id: u32,
    pub dedicated_vram_mb: u32,
    pub shared_vram_mb: u32,
    pub is_software: bool,
}

impl GpuAdapterInfo {
    pub fn vendor(&self) -> GpuVendor {
        recognize_gpu_vendor(self.vendor_id)
    }

    pub fn is_discrete(&self) -> bool {
        matches!(self.vendor(), GpuVendor::Nvidia | GpuVendor::Amd) && self.dedicated_vram_mb > 512
    }
}

#[cfg(windows)]
#[repr(C)]
struct DxgiAdapterDesc1 {
    description: [u16; 128],
    vendor_id: u32,
    device_id: u32,
    sub_sys_id: u32,
    revision: u32,
    dedicated_video_memory: usize,
    dedicated_system_memory: usize,
    shared_system_memory: usize,
    adapter_luid_low: u32,
    adapter_luid_high: i32,
    flags: u32,
}

#[cfg(windows)]
const DXGI_ADAPTER_FLAG_SOFTWARE: u32 = 2;

/// Enumerate physical GPU adapters using DXGI 1.3 / Direct3D (WIN-01).
pub fn enumerate_gpu_adapters() -> Vec<GpuAdapterInfo> {
    #[cfg(windows)]
    {
        use windows_sys::Win32::Foundation::FreeLibrary;
        use windows_sys::Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryA};

        type CreateDXGIFactory1Fn = unsafe extern "system" fn(
            riid: *const std::ffi::c_void,
            factory: *mut *mut std::ffi::c_void,
        ) -> i32;

        unsafe {
            let dxgi_dll = LoadLibraryA(c"dxgi.dll".as_ptr() as *const u8);
            if dxgi_dll.is_null() {
                return Vec::new();
            }

            let create_factory_ptr =
                GetProcAddress(dxgi_dll, c"CreateDXGIFactory1".as_ptr() as *const u8);
            let Some(create_factory) = create_factory_ptr else {
                FreeLibrary(dxgi_dll);
                return Vec::new();
            };

            let create_factory_fn: CreateDXGIFactory1Fn = std::mem::transmute(create_factory);

            // IID_IDXGIFactory1: 770aae78-f26f-4dba-a829-253c83d1b387
            let iid_idxgi_factory1: [u8; 16] = [
                0x78, 0xae, 0x0a, 0x77, 0x6f, 0xf2, 0xba, 0x4d, 0xa8, 0x29, 0x25, 0x3c, 0x83, 0xd1,
                0xb3, 0x87,
            ];

            let mut factory_ptr: *mut std::ffi::c_void = std::ptr::null_mut();
            let hr = create_factory_fn(
                iid_idxgi_factory1.as_ptr() as *const _,
                &mut factory_ptr as *mut _,
            );
            if hr < 0 || factory_ptr.is_null() {
                FreeLibrary(dxgi_dll);
                return Vec::new();
            }

            // IDXGIFactory1 vtable:
            // 0: QueryInterface, 1: AddRef, 2: Release
            // ... EnumAdapters1 is at index 12
            let vtable = *(factory_ptr as *const *const usize);
            type EnumAdapters1Fn = unsafe extern "system" fn(
                this: *mut std::ffi::c_void,
                adapter_index: u32,
                adapter: *mut *mut std::ffi::c_void,
            ) -> i32;
            type ReleaseFn = unsafe extern "system" fn(this: *mut std::ffi::c_void) -> u32;

            let enum_adapters1: EnumAdapters1Fn = std::mem::transmute(*vtable.add(12));
            let factory_release: ReleaseFn = std::mem::transmute(*vtable.add(2));

            let mut adapters = Vec::new();
            let mut index = 0u32;

            loop {
                let mut adapter_ptr: *mut std::ffi::c_void = std::ptr::null_mut();
                let status = enum_adapters1(factory_ptr, index, &mut adapter_ptr);
                if status < 0 || adapter_ptr.is_null() {
                    break;
                }

                // IDXGIAdapter1 vtable:
                // ... GetDesc1 is at index 10
                let adapter_vtable = *(adapter_ptr as *const *const usize);
                type GetDesc1Fn = unsafe extern "system" fn(
                    this: *mut std::ffi::c_void,
                    desc: *mut DxgiAdapterDesc1,
                ) -> i32;
                type AdapterReleaseFn =
                    unsafe extern "system" fn(this: *mut std::ffi::c_void) -> u32;

                let get_desc1: GetDesc1Fn = std::mem::transmute(*adapter_vtable.add(10));
                let adapter_release: AdapterReleaseFn = std::mem::transmute(*adapter_vtable.add(2));

                let mut desc: DxgiAdapterDesc1 = std::mem::zeroed();
                if get_desc1(adapter_ptr, &mut desc) >= 0 {
                    let is_software = (desc.flags & DXGI_ADAPTER_FLAG_SOFTWARE) != 0;
                    let nul_pos = desc
                        .description
                        .iter()
                        .position(|&c| c == 0)
                        .unwrap_or(desc.description.len());
                    let name = String::from_utf16_lossy(&desc.description[..nul_pos]);

                    let dedicated_vram_mb = (desc.dedicated_video_memory / (1024 * 1024)) as u32;
                    let shared_vram_mb = (desc.shared_system_memory / (1024 * 1024)) as u32;

                    adapters.push(GpuAdapterInfo {
                        name: name.trim().to_string(),
                        vendor_id: desc.vendor_id,
                        device_id: desc.device_id,
                        dedicated_vram_mb,
                        shared_vram_mb,
                        is_software,
                    });
                }

                adapter_release(adapter_ptr);
                index += 1;
            }

            factory_release(factory_ptr);
            FreeLibrary(dxgi_dll);
            adapters
        }
    }
    #[cfg(not(windows))]
    {
        Vec::new()
    }
}

/// Sample dedicated GPU VRAM used and total in MB.
pub fn sample_gpu_memory(adapter: Option<&GpuAdapterInfo>) -> (Option<u32>, Option<u32>) {
    let Some(info) = adapter else {
        return (None, None);
    };

    if info.dedicated_vram_mb > 0 {
        // Dedicated VRAM total is known; usage requires DXGI 1.4 query or PDH
        (None, Some(info.dedicated_vram_mb))
    } else {
        (None, None)
    }
}

/// Persistent Performance Data Helper (PDH) query collector for GPU utilization (WIN-01).
pub struct GpuPerformanceCollector {
    #[cfg(windows)]
    query: isize,
    #[cfg(windows)]
    counter: isize,
    #[cfg(windows)]
    is_valid: bool,
}

impl GpuPerformanceCollector {
    /// Create a new persistent PDH GPU query collector.
    pub fn new() -> Self {
        #[cfg(windows)]
        {
            use windows_sys::Win32::System::Performance::{PdhAddEnglishCounterW, PdhOpenQueryW};

            let mut query: isize = 0;
            let status = unsafe { PdhOpenQueryW(std::ptr::null(), 0, &mut query) };
            if status != 0 {
                return Self {
                    query: 0,
                    counter: 0,
                    is_valid: false,
                };
            }

            // Path for GPU Engine utilization across all engines
            let counter_path: Vec<u16> = r"\GPU Engine(*)\Utilization Percentage"
                .encode_utf16()
                .chain(std::iter::once(0))
                .collect();

            let mut counter: isize = 0;
            let counter_status =
                unsafe { PdhAddEnglishCounterW(query, counter_path.as_ptr(), 0, &mut counter) };

            if counter_status != 0 {
                unsafe {
                    windows_sys::Win32::System::Performance::PdhCloseQuery(query);
                }
                return Self {
                    query: 0,
                    counter: 0,
                    is_valid: false,
                };
            }

            // Perform initial sample collection to establish delta baseline
            unsafe {
                windows_sys::Win32::System::Performance::PdhCollectQueryData(query);
            }

            Self {
                query,
                counter,
                is_valid: true,
            }
        }
        #[cfg(not(windows))]
        {
            Self {}
        }
    }

    /// Sample aggregate GPU utilization percentage.
    pub fn sample_utilization(&mut self) -> Option<f32> {
        #[cfg(windows)]
        {
            use windows_sys::Win32::System::Performance::{
                PDH_FMT_COUNTERVALUE, PDH_FMT_DOUBLE, PdhCollectQueryData,
                PdhGetFormattedCounterValue,
            };

            if !self.is_valid {
                return None;
            }

            let collect_status = unsafe { PdhCollectQueryData(self.query) };
            if collect_status != 0 {
                return None;
            }

            let mut value: PDH_FMT_COUNTERVALUE = unsafe { std::mem::zeroed() };
            let status = unsafe {
                PdhGetFormattedCounterValue(
                    self.counter,
                    PDH_FMT_DOUBLE,
                    std::ptr::null_mut(),
                    &mut value,
                )
            };

            if status != 0 {
                return None;
            }

            let val = unsafe { value.Anonymous.doubleValue };
            if val.is_nan() || val < 0.0 {
                None
            } else {
                Some((val as f32).clamp(0.0, 100.0))
            }
        }
        #[cfg(not(windows))]
        {
            None
        }
    }
}

impl Default for GpuPerformanceCollector {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for GpuPerformanceCollector {
    fn drop(&mut self) {
        #[cfg(windows)]
        {
            if self.is_valid && self.query != 0 {
                unsafe {
                    windows_sys::Win32::System::Performance::PdhCloseQuery(self.query);
                }
            }
        }
    }
}

/// Sample GPU engine utilization percentage using a temporary collector.
pub fn sample_gpu_utilization() -> Option<f32> {
    let mut collector = GpuPerformanceCollector::new();
    collector.sample_utilization()
}

/// Read GPU core temperature. Returns None if unsupported rather than fabricating 0.
pub fn read_gpu_temperature(_vendor_id: u32) -> Option<f32> {
    None
}
