//! Read-only HP WMI diagnostic probe (WIN-04 Task 4).
//!
//! Inspects real WMI metadata in `ROOT\WMI`, validates method signatures,
//! queries `hpqBIntM` instances, and prints/returns a structured, non-mutating report.

use std::fmt;

/// Categories for WMI failure diagnostics.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub enum WmiDiagnosticCategory {
    NamespaceMissing,
    ClassMissing,
    InstanceMissing,
    MethodMissing,
    InputClassMissing,
    OutputClassMissing,
    ComFailure,
    ProviderFailure,
    AccessDenied,
    Healthy,
}

impl fmt::Display for WmiDiagnosticCategory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NamespaceMissing => write!(f, "Namespace missing"),
            Self::ClassMissing => write!(f, "Class missing"),
            Self::InstanceMissing => write!(f, "Instance missing"),
            Self::MethodMissing => write!(f, "Method missing"),
            Self::InputClassMissing => write!(f, "Input class missing"),
            Self::OutputClassMissing => write!(f, "Output class missing"),
            Self::ComFailure => write!(f, "COM failure"),
            Self::ProviderFailure => write!(f, "Provider failure"),
            Self::AccessDenied => write!(f, "Access denied"),
            Self::Healthy => write!(f, "Healthy"),
        }
    }
}

/// Comprehensive, sanitized diagnostic report for HP BIOS WMI.
#[derive(Debug, Clone, serde::Serialize)]
pub struct WmiDiagnosticReport {
    pub com_initialized: bool,
    pub namespace_connected: bool,
    pub namespace_hresult: Option<i32>,
    pub hpqbintm_class_found: bool,
    pub hpqbintm_hresult: Option<i32>,
    pub hpqbdatain_class_found: bool,
    pub hpqbdataout128_class_found: bool,
    pub methods_found: Vec<String>,
    pub instance_status: String,
    pub instance_path: Option<String>,
    pub instance_hresult: Option<i32>,
    pub readonly_probe_status: Option<String>,
    pub category: WmiDiagnosticCategory,
    pub summary: String,
}

#[cfg(windows)]
pub fn run_wmi_diagnostic() -> WmiDiagnosticReport {
    use crate::service::windows::hp_wmi::*;
    use crate::service::windows::wmi_ffi::*;
    use std::ffi::c_void;
    use std::ptr::null_mut;

    unsafe {
        // 1. Initialise COM
        let hr = CoInitializeEx(null_mut(), 0);
        let com_initialized = hr == S_OK || hr == S_FALSE || hr == RPC_E_CHANGED_MODE;
        if !com_initialized {
            return WmiDiagnosticReport {
                com_initialized: false,
                namespace_connected: false,
                namespace_hresult: None,
                hpqbintm_class_found: false,
                hpqbintm_hresult: None,
                hpqbdatain_class_found: false,
                hpqbdataout128_class_found: false,
                methods_found: Vec::new(),
                instance_status: "Skipped".to_string(),
                instance_path: None,
                instance_hresult: None,
                readonly_probe_status: None,
                category: WmiDiagnosticCategory::ComFailure,
                summary: format!("CoInitializeEx failed with HRESULT {hr:#010x}"),
            };
        }

        let _ = CoInitializeSecurity(
            null_mut(),
            -1,
            null_mut(),
            null_mut(),
            0,
            3,
            null_mut(),
            0,
            null_mut(),
        );

        // 2. Connect IWbemLocator -> ROOT\WMI
        let mut locator_raw: *mut c_void = null_mut();
        let hr = CoCreateInstance(
            &CLSID_WBEM_LOCATOR,
            null_mut(),
            1,
            &IID_IWBEM_LOCATOR,
            &mut locator_raw,
        );
        if hr != S_OK || locator_raw.is_null() {
            return WmiDiagnosticReport {
                com_initialized: true,
                namespace_connected: false,
                namespace_hresult: Some(hr),
                hpqbintm_class_found: false,
                hpqbintm_hresult: None,
                hpqbdatain_class_found: false,
                hpqbdataout128_class_found: false,
                methods_found: Vec::new(),
                instance_status: "Skipped".to_string(),
                instance_path: None,
                instance_hresult: None,
                readonly_probe_status: None,
                category: WmiDiagnosticCategory::ComFailure,
                summary: format!("CoCreateInstance(IWbemLocator) failed with HRESULT {hr:#010x}"),
            };
        }
        let locator = ComPtr::from_raw(locator_raw as *mut *const IWbemLocatorVtbl);

        let ns_bstr = Bstr::new(r"ROOT\WMI");
        let mut services_raw: *mut c_void = null_mut();
        let hr = ((*(*locator.as_ptr())).ConnectServer)(
            locator.as_ptr() as *mut c_void,
            ns_bstr.as_ptr(),
            null_mut(),
            null_mut(),
            null_mut(),
            0,
            null_mut(),
            null_mut(),
            &mut services_raw,
        );
        if hr != S_OK || services_raw.is_null() {
            let cat = if hr == WBEM_E_ACCESS_DENIED {
                WmiDiagnosticCategory::AccessDenied
            } else if hr == WBEM_E_NOT_FOUND || hr == WBEM_E_INVALID_CLASS {
                WmiDiagnosticCategory::NamespaceMissing
            } else {
                WmiDiagnosticCategory::ProviderFailure
            };
            return WmiDiagnosticReport {
                com_initialized: true,
                namespace_connected: false,
                namespace_hresult: Some(hr),
                hpqbintm_class_found: false,
                hpqbintm_hresult: None,
                hpqbdatain_class_found: false,
                hpqbdataout128_class_found: false,
                methods_found: Vec::new(),
                instance_status: "Skipped".to_string(),
                instance_path: None,
                instance_hresult: None,
                readonly_probe_status: None,
                category: cat,
                summary: format!("ConnectServer(ROOT\\WMI) failed with HRESULT {hr:#010x}"),
            };
        }
        let services = ComPtr::from_raw(services_raw as *mut *const IWbemServicesVtbl);

        // 3. Inspect hpqBIntM class
        let class_bstr = Bstr::new(HP_WMI_CLASS);
        let mut class_obj_raw: *mut c_void = null_mut();
        let hr_class = ((*(*services.as_ptr())).GetObject)(
            services.as_ptr() as *mut c_void,
            class_bstr.as_ptr(),
            0,
            null_mut(),
            &mut class_obj_raw,
            null_mut(),
        );
        let hpqbintm_class_found = hr_class == S_OK && !class_obj_raw.is_null();
        if !hpqbintm_class_found {
            return WmiDiagnosticReport {
                com_initialized: true,
                namespace_connected: true,
                namespace_hresult: Some(S_OK),
                hpqbintm_class_found: false,
                hpqbintm_hresult: Some(hr_class),
                hpqbdatain_class_found: false,
                hpqbdataout128_class_found: false,
                methods_found: Vec::new(),
                instance_status: "Skipped".to_string(),
                instance_path: None,
                instance_hresult: None,
                readonly_probe_status: None,
                category: WmiDiagnosticCategory::ClassMissing,
                summary: format!("Class {HP_WMI_CLASS} not found: HRESULT {hr_class:#010x}"),
            };
        }
        let class_obj = ComPtr::from_raw(class_obj_raw as *mut *const IWbemClassObjectVtbl);

        // 4. Inspect input class hpqBDataIn
        let datain_bstr = Bstr::new("hpqBDataIn");
        let mut datain_raw: *mut c_void = null_mut();
        let hr_datain = ((*(*services.as_ptr())).GetObject)(
            services.as_ptr() as *mut c_void,
            datain_bstr.as_ptr(),
            0,
            null_mut(),
            &mut datain_raw,
            null_mut(),
        );
        let hpqbdatain_class_found = hr_datain == S_OK && !datain_raw.is_null();
        if !datain_raw.is_null() {
            let _ = ComPtr::from_raw(datain_raw as *mut *const IWbemClassObjectVtbl);
        }

        // 5. Inspect output class hpqBDataOut128
        let dataout_bstr = Bstr::new("hpqBDataOut128");
        let mut dataout_raw: *mut c_void = null_mut();
        let hr_dataout = ((*(*services.as_ptr())).GetObject)(
            services.as_ptr() as *mut c_void,
            dataout_bstr.as_ptr(),
            0,
            null_mut(),
            &mut dataout_raw,
            null_mut(),
        );
        let hpqbdataout128_class_found = hr_dataout == S_OK && !dataout_raw.is_null();
        if !dataout_raw.is_null() {
            let _ = ComPtr::from_raw(dataout_raw as *mut *const IWbemClassObjectVtbl);
        }

        // 6. Enumerate documented methods on hpqBIntM
        let documented_methods = [
            "hpqBIOSInt0",
            "hpqBIOSInt4",
            "hpqBIOSInt128",
            "hpqBIOSInt1024",
            "hpqBIOSInt4096",
        ];
        let mut methods_found = Vec::new();
        for m_name in &documented_methods {
            let m_bstr = Bstr::new(m_name);
            let mut in_sig: *mut c_void = null_mut();
            let mut out_sig: *mut c_void = null_mut();
            let hr_m = ((*(*class_obj.as_ptr())).GetMethod)(
                class_obj.as_ptr() as *mut c_void,
                m_bstr.as_ptr(),
                0,
                &mut in_sig,
                &mut out_sig,
            );
            if hr_m == S_OK {
                methods_found.push((*m_name).to_string());
            }
            if !in_sig.is_null() {
                let _ = ComPtr::from_raw(in_sig as *mut *const IWbemClassObjectVtbl);
            }
            if !out_sig.is_null() {
                let _ = ComPtr::from_raw(out_sig as *mut *const IWbemClassObjectVtbl);
            }
        }

        // 7. Enumerate instances of hpqBIntM via ExecQuery
        let query_bstr = Bstr::new(&format!("SELECT * FROM {HP_WMI_CLASS}"));
        let wql_bstr = Bstr::new("WQL");
        let mut enum_raw: *mut c_void = null_mut();
        let hr_query = ((*(*services.as_ptr())).ExecQuery)(
            services.as_ptr() as *mut c_void,
            wql_bstr.as_ptr(),
            query_bstr.as_ptr(),
            WBEM_FLAG_FORWARD_ONLY | WBEM_FLAG_RETURN_IMMEDIATELY,
            null_mut(),
            &mut enum_raw,
        );

        let mut instance_status = "Unknown".to_string();
        let mut instance_path = None;
        let mut instance_hresult = Some(hr_query);

        if hr_query == WBEM_E_ACCESS_DENIED {
            instance_status = "AccessDenied (0x80041003: Elevation required)".to_string();
        } else if hr_query == S_OK && !enum_raw.is_null() {
            let enumerator = ComPtr::from_raw(enum_raw as *mut *const IEnumWbemClassObjectVtbl);
            let mut inst_raw: *mut c_void = null_mut();
            let mut returned = 0u32;
            let hr_next = ((*(*enumerator.as_ptr())).Next)(
                enumerator.as_ptr() as *mut c_void,
                WBEM_INFINITE as i32,
                1,
                &mut inst_raw,
                &mut returned,
            );
            instance_hresult = Some(hr_next);
            if hr_next == S_OK && !inst_raw.is_null() && returned > 0 {
                let inst = ComPtr::from_raw(inst_raw as *mut *const IWbemClassObjectVtbl);
                let path_prop = Bstr::new("__RELPATH");
                let mut path_var = ScopedVariant::empty();
                let hr_get = ((*(*inst.as_ptr())).Get)(
                    inst.as_ptr() as *mut c_void,
                    path_prop.as_ptr(),
                    0,
                    &mut path_var.inner,
                    null_mut(),
                    null_mut(),
                );
                if hr_get == S_OK
                    && let Some(s) = path_var.extract_bstr()
                {
                    instance_status = "Found".to_string();
                    instance_path = Some(s);
                }
            } else if hr_next == WBEM_E_ACCESS_DENIED {
                instance_status = "AccessDenied (0x80041003: Elevation required)".to_string();
            } else if returned == 0 && (hr_next == S_OK || hr_next == WBEM_S_FALSE) {
                instance_status = "No instances returned".to_string();
            } else {
                instance_status = format!("Enumerator error: HRESULT {hr_next:#010x}");
            }
        } else {
            instance_status = format!("ExecQuery error: HRESULT {hr_query:#010x}");
        }

        // 8. If an instance is found, attempt a non-mutating, read-only probe (FAN_COUNT)
        let mut readonly_probe_status = None;
        if instance_path.is_some() {
            match RealWmiAdapter::connect() {
                Ok(adapter) => match adapter.bios_call(REQ_READ, commands::FAN_COUNT, &[], 128) {
                    Ok(payload) => {
                        readonly_probe_status =
                            Some(format!("Success (FAN_COUNT payload: {payload:02x?})"));
                    }
                    Err(e) => {
                        readonly_probe_status = Some(format!("Failed: {e}"));
                    }
                },
                Err(e) => {
                    readonly_probe_status = Some(format!("Adapter connect failed: {e}"));
                }
            }
        }

        // Categorize result
        let category = if !hpqbdatain_class_found {
            WmiDiagnosticCategory::InputClassMissing
        } else if !hpqbdataout128_class_found {
            WmiDiagnosticCategory::OutputClassMissing
        } else if methods_found.is_empty() {
            WmiDiagnosticCategory::MethodMissing
        } else if instance_status.starts_with("AccessDenied") {
            WmiDiagnosticCategory::AccessDenied
        } else if instance_path.is_none() {
            WmiDiagnosticCategory::InstanceMissing
        } else {
            WmiDiagnosticCategory::Healthy
        };

        let summary = format!(
            "HP WMI Diagnostic: Category={}, Methods=[{}], Instance={}, Probe={}",
            category,
            methods_found.join(", "),
            instance_status,
            readonly_probe_status.as_deref().unwrap_or("Skipped")
        );

        WmiDiagnosticReport {
            com_initialized: true,
            namespace_connected: true,
            namespace_hresult: Some(S_OK),
            hpqbintm_class_found: true,
            hpqbintm_hresult: Some(S_OK),
            hpqbdatain_class_found,
            hpqbdataout128_class_found,
            methods_found,
            instance_status,
            instance_path,
            instance_hresult,
            readonly_probe_status,
            category,
            summary,
        }
    }
}

#[cfg(not(windows))]
pub fn run_wmi_diagnostic() -> WmiDiagnosticReport {
    WmiDiagnosticReport {
        com_initialized: false,
        namespace_connected: false,
        namespace_hresult: None,
        hpqbintm_class_found: false,
        hpqbintm_hresult: None,
        hpqbdatain_class_found: false,
        hpqbdataout128_class_found: false,
        methods_found: Vec::new(),
        instance_status: "Non-Windows environment".to_string(),
        instance_path: None,
        instance_hresult: None,
        readonly_probe_status: None,
        category: WmiDiagnosticCategory::NamespaceMissing,
        summary: "HP WMI is only supported on Windows".to_string(),
    }
}

/// Print formatted human-readable WMI diagnostics to stdout.
pub fn print_wmi_diagnostic() {
    let report = run_wmi_diagnostic();
    println!("══════════════════════════════════════════════════════════════");
    println!("             HP BIOS WMI DIAGNOSTIC REPORT                    ");
    println!("══════════════════════════════════════════════════════════════");
    println!(" COM Library Initialized : {}", report.com_initialized);
    println!(
        " Namespace ROOT\\WMI      : {}",
        if report.namespace_connected {
            "Connected"
        } else {
            "FAILED"
        }
    );
    if let Some(hr) = report.namespace_hresult {
        println!(" Namespace HRESULT       : {hr:#010x}");
    }
    println!(
        " Class hpqBIntM          : {}",
        if report.hpqbintm_class_found {
            "Found"
        } else {
            "MISSING"
        }
    );
    println!(
        " Class hpqBDataIn        : {}",
        if report.hpqbdatain_class_found {
            "Found"
        } else {
            "MISSING"
        }
    );
    println!(
        " Class hpqBDataOut128    : {}",
        if report.hpqbdataout128_class_found {
            "Found"
        } else {
            "MISSING"
        }
    );
    println!(
        " Callable Methods Found  : {}",
        if report.methods_found.is_empty() {
            "NONE".to_string()
        } else {
            report.methods_found.join(", ")
        }
    );
    println!(" Instance Query Status   : {}", report.instance_status);
    if let Some(ref path) = report.instance_path {
        println!(" Active Instance Path    : {path}");
    }
    if let Some(ref probe) = report.readonly_probe_status {
        println!(" Read-only Probe Status  : {probe}");
    }
    println!(" Overall Diagnosis       : {}", report.category);
    println!(" Summary                 : {}", report.summary);
    println!("══════════════════════════════════════════════════════════════");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_diagnostic_category_display() {
        assert_eq!(
            WmiDiagnosticCategory::NamespaceMissing.to_string(),
            "Namespace missing"
        );
        assert_eq!(
            WmiDiagnosticCategory::ClassMissing.to_string(),
            "Class missing"
        );
        assert_eq!(
            WmiDiagnosticCategory::InstanceMissing.to_string(),
            "Instance missing"
        );
        assert_eq!(
            WmiDiagnosticCategory::MethodMissing.to_string(),
            "Method missing"
        );
        assert_eq!(
            WmiDiagnosticCategory::InputClassMissing.to_string(),
            "Input class missing"
        );
        assert_eq!(
            WmiDiagnosticCategory::OutputClassMissing.to_string(),
            "Output class missing"
        );
        assert_eq!(WmiDiagnosticCategory::ComFailure.to_string(), "COM failure");
        assert_eq!(
            WmiDiagnosticCategory::ProviderFailure.to_string(),
            "Provider failure"
        );
        assert_eq!(
            WmiDiagnosticCategory::AccessDenied.to_string(),
            "Access denied"
        );
        assert_eq!(WmiDiagnosticCategory::Healthy.to_string(), "Healthy");
    }

    #[test]
    fn test_run_wmi_diagnostic_does_not_panic() {
        let rep = run_wmi_diagnostic();
        assert!(!rep.summary.is_empty());
    }
}
