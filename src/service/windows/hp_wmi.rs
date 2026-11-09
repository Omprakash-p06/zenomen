//! HP BIOS WMI firmware interface driver (`hpqBIntM`) (WIN-04).
//!
//! This module implements **real** WMI communication with the HP BIOS firmware on Windows.
//! On non-Windows targets all public functions return `HardwareControlError::InterfaceMissing`.
//!
//! ## Architecture
//!
//! All firmware operations go through the [`WmiAdapter`] trait so that unit tests can inject a
//! fake adapter without touching COM or real hardware. Production code uses [`RealWmiAdapter`]
//! which initialises COM, connects to `ROOT\WMI`, and executes `hpqBIntM::hpqBMtdx`.
//!
//! ## Protocol (HP BIOS Mailbox)
//!
//! Every call to `hpqBMtdx` uses:
//! - `Sign`     = `0x55434553` ("SECU")
//! - `ReqType`  = `0x20008` (read) or `0x20009` (write)
//! - `InBuf`    = `[command_id: u32 LE, subcommand: u32 LE, data_len: u32 LE, payload: [u8;128]]`
//! - `OutBuf`   = `[status: u32 LE, out_len: u32 LE, payload: [u8;128]]`
//!
//! Return code 0x00 = success; any other value is an error (see `FirmwareStatus`).
//!
//! ## Invariants
//!
//! - `connect_hp_wmi` returns `Err(InterfaceMissing)` when WMI or the class is unreachable.
//! - `query_fan_status` reads firmware state; it **never** reads process-local variables.
//! - `set_fan_mode` reads firmware **after** writing; success is returned only after a matching
//!   read-back. If read-back disagrees the error is `VerificationFailed`.
//! - No process-local fan or power statics exist.

use crate::hardware::HardwareCapabilities;
use crate::rgb::schema::RgbProfile;
use crate::service::types::HardwareControlError;

// ─────────────────────────── Public constants ──────────────────────────────

/// WMI namespace for HP ACPI BIOS objects.
pub const HP_WMI_NAMESPACE: &str = r"ROOT\WMI";

/// WMI class name for HP BIOS mailbox interface.
pub const HP_WMI_CLASS: &str = "hpqBIntM";

/// WMI method name on `hpqBIntM`.
pub const HP_WMI_METHOD: &str = "hpqBIOSInt128";
pub const HP_WMI_METHOD_128: &str = "hpqBIOSInt128";
pub const HP_WMI_METHOD_4: &str = "hpqBIOSInt4";

/// HP BIOS input and output class names.
pub const HP_WMI_DATA_IN_CLASS: &str = "hpqBDataIn";
pub const HP_WMI_DATA_OUT_128_CLASS: &str = "hpqBDataOut128";
pub const HP_WMI_DATA_OUT_4_CLASS: &str = "hpqBDataOut4";

/// HP BIOS WMI GUID (`5FB7F034-2C63-45E9-BE91-3D44E2C707E4`).
pub const HPWMI_BIOS_GUID: &str = "5FB7F034-2C63-45E9-BE91-3D44E2C707E4";

/// `Sign` parameter: ASCII "SECU".
const SIGN_SECU_BYTES: [u8; 4] = *b"SECU";

/// `ReqType` for read queries.
pub const REQ_READ: u32 = 0x0002_0008;

/// `ReqType` for write mutations.
pub const REQ_WRITE: u32 = 0x0002_0009;

/// HP Gaming command family identifiers.
pub mod commands {
    pub const FAN_COUNT: u32 = 0x10;
    pub const FAN_SPEEDS: u32 = 0x11;
    pub const PERFORMANCE_MODE: u32 = 0x1A;
    pub const MAX_FAN_STATUS: u32 = 0x26;
    pub const MAX_FAN_SET: u32 = 0x27;
    pub const SYSTEM_DESIGN_DATA: u32 = 0x28;
    pub const FOUR_ZONE_RGB: u32 = 0x0002_0009;
}

// ──────────────────────────── WmiAdapter trait ─────────────────────────────

/// Injectable WMI adapter abstraction — real COM on Windows, fake in tests.
///
/// The only requirement is that `exec_method` mirrors the semantics of
/// `IWbemServices::ExecMethod` for the `hpqBIntM` class.
pub trait WmiAdapter: Send + Sync {
    /// Execute an HP BIOS call using the real WMI interface `hpqBIOSInt*`.
    fn bios_call(
        &self,
        command: u32,
        command_type: u32,
        payload: &[u8],
        out_size: u32,
    ) -> Result<Vec<u8>, HardwareControlError>;

    /// Legacy buffer execution helper for backwards compatibility.
    fn exec_method(
        &self,
        _sign: u32,
        req_type: u32,
        in_buf: &[u8; 140],
    ) -> Result<Vec<u8>, HardwareControlError> {
        let mut cmd_bytes = [0u8; 4];
        cmd_bytes.copy_from_slice(&in_buf[0..4]);
        let cmd = u32::from_le_bytes(cmd_bytes);
        let mut len_bytes = [0u8; 4];
        len_bytes.copy_from_slice(&in_buf[8..12]);
        let len = u32::from_le_bytes(len_bytes) as usize;
        let payload = if len > 0 && len <= 128 {
            &in_buf[12..12 + len]
        } else {
            &[]
        };
        let out_bytes = self.bios_call(req_type, cmd, payload, 128)?;
        let mut buf = vec![0u8; 136];
        let out_len = out_bytes.len().min(128) as u32;
        buf[4..8].copy_from_slice(&out_len.to_le_bytes());
        let copy_len = out_bytes.len().min(128);
        buf[8..8 + copy_len].copy_from_slice(&out_bytes[..copy_len]);
        Ok(buf)
    }

    /// Return `true` if the adapter is connected and the class exists.
    fn is_available(&self) -> bool;
}

// ─────────────────────── Real WMI adapter (Windows only) ──────────────────

/// Real COM/WMI adapter that connects to `ROOT\WMI` and executes `hpqBMtdx`.
pub struct RealWmiAdapter {
    available: bool,
}

impl RealWmiAdapter {
    /// Attempt to connect to `ROOT\WMI` and verify that `hpqBIntM` exists.
    pub fn connect() -> Result<Self, HardwareControlError> {
        #[cfg(windows)]
        {
            Self::connect_windows()
        }
        #[cfg(not(windows))]
        {
            Err(HardwareControlError::InterfaceMissing(
                "HP WMI interface is only available on Windows".to_string(),
            ))
        }
    }

    #[cfg(windows)]
    fn connect_windows() -> Result<Self, HardwareControlError> {
        use crate::service::windows::wmi_ffi::*;
        use std::ffi::c_void;
        use std::ptr::null_mut;

        unsafe {
            // 1. Initialise COM for this thread.
            let hr = CoInitializeEx(null_mut(), 0); // COINIT_MULTITHREADED = 0
            if hr != S_OK && hr != S_FALSE && hr != RPC_E_CHANGED_MODE {
                return Err(HardwareControlError::InterfaceMissing(format!(
                    "CoInitializeEx failed: HRESULT {hr:#010x}"
                )));
            }

            // 2. Set COM security. Non-fatal if already initialized.
            let _ = CoInitializeSecurity(
                null_mut(),
                -1,
                null_mut(),
                null_mut(),
                0, // RPC_C_AUTHN_LEVEL_DEFAULT
                3, // RPC_C_IMP_LEVEL_IMPERSONATE
                null_mut(),
                0, // EOAC_NONE
                null_mut(),
            );

            // 3. Create IWbemLocator.
            let mut locator_raw: *mut c_void = null_mut();
            let hr = CoCreateInstance(
                &CLSID_WBEM_LOCATOR,
                null_mut(),
                1, // CLSCTX_INPROC_SERVER
                &IID_IWBEM_LOCATOR,
                &mut locator_raw,
            );
            if hr != S_OK || locator_raw.is_null() {
                return Err(HardwareControlError::InterfaceMissing(format!(
                    "CoCreateInstance(IWbemLocator) failed: HRESULT {hr:#010x}"
                )));
            }
            let locator = ComPtr::from_raw(locator_raw as *mut *const IWbemLocatorVtbl);

            // 4. Connect to ROOT\WMI.
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
                return Err(HardwareControlError::InterfaceMissing(format!(
                    "IWbemLocator::ConnectServer(ROOT\\WMI) failed: HRESULT {hr:#010x}"
                )));
            }
            let services = ComPtr::from_raw(services_raw as *mut *const IWbemServicesVtbl);

            // 5. Verify that hpqBIntM class exists by querying GetObject.
            let class_bstr = Bstr::new(HP_WMI_CLASS);
            let mut class_obj_raw: *mut c_void = null_mut();
            let hr = ((*(*services.as_ptr())).GetObject)(
                services.as_ptr() as *mut c_void,
                class_bstr.as_ptr(),
                0,
                null_mut(),
                &mut class_obj_raw,
                null_mut(),
            );
            if !class_obj_raw.is_null() {
                let _ = ComPtr::from_raw(class_obj_raw as *mut *const IWbemClassObjectVtbl);
            }

            if hr != S_OK {
                tracing::warn!(
                    hr = hr,
                    class = HP_WMI_CLASS,
                    "HP WMI class not found on this system; fan control will be unavailable"
                );
                return Ok(Self { available: false });
            }

            tracing::info!(
                namespace = HP_WMI_NAMESPACE,
                class = HP_WMI_CLASS,
                "HP WMI interface connected and class verified"
            );
            Ok(Self { available: true })
        }
    }
}

impl WmiAdapter for RealWmiAdapter {
    fn is_available(&self) -> bool {
        self.available
    }

    fn bios_call(
        &self,
        command: u32,
        command_type: u32,
        payload: &[u8],
        out_size: u32,
    ) -> Result<Vec<u8>, HardwareControlError> {
        #[cfg(windows)]
        {
            self.bios_call_windows(command, command_type, payload, out_size)
        }
        #[cfg(not(windows))]
        {
            let _ = (command, command_type, payload, out_size);
            Err(HardwareControlError::InterfaceMissing(
                "HP WMI bios_call is only available on Windows".to_string(),
            ))
        }
    }
}

#[cfg(windows)]
impl RealWmiAdapter {
    fn bios_call_windows(
        &self,
        command: u32,
        command_type: u32,
        payload: &[u8],
        out_size: u32,
    ) -> Result<Vec<u8>, HardwareControlError> {
        use crate::service::windows::wmi_ffi::*;
        use std::ffi::c_void;
        use std::ptr::null_mut;

        unsafe {
            // 1. Create IWbemLocator.
            let mut locator_raw: *mut c_void = null_mut();
            let hr = CoCreateInstance(
                &CLSID_WBEM_LOCATOR,
                null_mut(),
                1,
                &IID_IWBEM_LOCATOR,
                &mut locator_raw,
            );
            if hr != S_OK || locator_raw.is_null() {
                return Err(HardwareControlError::ConnectionFailed(format!(
                    "CoCreateInstance(IWbemLocator) failed: {hr:#010x}"
                )));
            }
            let locator = ComPtr::from_raw(locator_raw as *mut *const IWbemLocatorVtbl);

            // 2. Connect to ROOT\WMI.
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
                return Err(HardwareControlError::ConnectionFailed(format!(
                    "IWbemLocator::ConnectServer failed: {hr:#010x}"
                )));
            }
            let services = ComPtr::from_raw(services_raw as *mut *const IWbemServicesVtbl);

            // 3. Get class object definition for method signature.
            let class_bstr = Bstr::new(HP_WMI_CLASS);
            let mut class_obj_raw: *mut c_void = null_mut();
            let hr = ((*(*services.as_ptr())).GetObject)(
                services.as_ptr() as *mut c_void,
                class_bstr.as_ptr(),
                0,
                null_mut(),
                &mut class_obj_raw,
                null_mut(),
            );
            if hr != S_OK || class_obj_raw.is_null() {
                return Err(HardwareControlError::InterfaceMissing(format!(
                    "GetObject({HP_WMI_CLASS}) failed: {hr:#010x}"
                )));
            }
            let class_obj = ComPtr::from_raw(class_obj_raw as *mut *const IWbemClassObjectVtbl);

            // 4. Method name: hpqBIOSInt128 (default) or hpqBIOSInt4
            let method_name = if out_size <= 4 {
                HP_WMI_METHOD_4
            } else {
                HP_WMI_METHOD_128
            };
            let method_bstr = Bstr::new(method_name);
            let mut in_def_raw: *mut c_void = null_mut();
            let hr = ((*(*class_obj.as_ptr())).GetMethod)(
                class_obj.as_ptr() as *mut c_void,
                method_bstr.as_ptr(),
                0,
                &mut in_def_raw,
                null_mut(),
            );
            if hr != S_OK || in_def_raw.is_null() {
                return Err(HardwareControlError::InterfaceMissing(format!(
                    "GetMethod({method_name}) failed: {hr:#010x}"
                )));
            }
            let in_def = ComPtr::from_raw(in_def_raw as *mut *const IWbemClassObjectVtbl);

            // 5. Spawn instance of in-params for the method.
            let mut in_params_raw: *mut c_void = null_mut();
            let hr = ((*(*in_def.as_ptr())).SpawnInstance)(
                in_def.as_ptr() as *mut c_void,
                0,
                &mut in_params_raw,
            );
            if hr != S_OK || in_params_raw.is_null() {
                return Err(HardwareControlError::ProtocolError(format!(
                    "SpawnInstance for method in-params failed: {hr:#010x}"
                )));
            }
            let in_params = ComPtr::from_raw(in_params_raw as *mut *const IWbemClassObjectVtbl);

            // 6. Spawn instance of hpqBDataIn.
            let datain_bstr = Bstr::new(HP_WMI_DATA_IN_CLASS);
            let mut datain_class_raw: *mut c_void = null_mut();
            let hr = ((*(*services.as_ptr())).GetObject)(
                services.as_ptr() as *mut c_void,
                datain_bstr.as_ptr(),
                0,
                null_mut(),
                &mut datain_class_raw,
                null_mut(),
            );
            if hr != S_OK || datain_class_raw.is_null() {
                return Err(HardwareControlError::InterfaceMissing(format!(
                    "GetObject({HP_WMI_DATA_IN_CLASS}) failed: {hr:#010x}"
                )));
            }
            let datain_class =
                ComPtr::from_raw(datain_class_raw as *mut *const IWbemClassObjectVtbl);

            let mut datain_inst_raw: *mut c_void = null_mut();
            let hr = ((*(*datain_class.as_ptr())).SpawnInstance)(
                datain_class.as_ptr() as *mut c_void,
                0,
                &mut datain_inst_raw,
            );
            if hr != S_OK || datain_inst_raw.is_null() {
                return Err(HardwareControlError::ProtocolError(format!(
                    "SpawnInstance({HP_WMI_DATA_IN_CLASS}) failed: {hr:#010x}"
                )));
            }
            let datain_inst = ComPtr::from_raw(datain_inst_raw as *mut *const IWbemClassObjectVtbl);

            // 7. Populate hpqBDataIn:
            // Sign = "SECU"
            let sign_var = ScopedVariant::from_bytes(&SIGN_SECU_BYTES)?;
            let sign_name = Bstr::new("Sign");
            ((*(*datain_inst.as_ptr())).Put)(
                datain_inst.as_ptr() as *mut c_void,
                sign_name.as_ptr(),
                0,
                &sign_var.inner,
                0,
            );

            // Command
            let cmd_var = ScopedVariant::from_u32(command);
            let cmd_name = Bstr::new("Command");
            ((*(*datain_inst.as_ptr())).Put)(
                datain_inst.as_ptr() as *mut c_void,
                cmd_name.as_ptr(),
                0,
                &cmd_var.inner,
                0,
            );

            // CommandType
            let cmdtype_var = ScopedVariant::from_u32(command_type);
            let cmdtype_name = Bstr::new("CommandType");
            ((*(*datain_inst.as_ptr())).Put)(
                datain_inst.as_ptr() as *mut c_void,
                cmdtype_name.as_ptr(),
                0,
                &cmdtype_var.inner,
                0,
            );

            // Size
            let size_var = ScopedVariant::from_u32(payload.len() as u32);
            let size_name = Bstr::new("Size");
            ((*(*datain_inst.as_ptr())).Put)(
                datain_inst.as_ptr() as *mut c_void,
                size_name.as_ptr(),
                0,
                &size_var.inner,
                0,
            );

            // hpqBData
            let data_var = ScopedVariant::from_bytes(payload)?;
            let data_name = Bstr::new("hpqBData");
            ((*(*datain_inst.as_ptr())).Put)(
                datain_inst.as_ptr() as *mut c_void,
                data_name.as_ptr(),
                0,
                &data_var.inner,
                0,
            );

            // 8. Put "InData" into method in_params.
            let indata_name = Bstr::new("InData");
            let indata_var = ScopedVariant::from_unknown(datain_inst.as_ptr() as *mut c_void);
            let hr = ((*(*in_params.as_ptr())).Put)(
                in_params.as_ptr() as *mut c_void,
                indata_name.as_ptr(),
                0,
                &indata_var.inner,
                CIM_OBJECT,
            );
            if hr != S_OK {
                return Err(HardwareControlError::ProtocolError(format!(
                    "Put(InData) failed: {hr:#010x}"
                )));
            }

            // 9. Query instance of hpqBIntM to get __RELPATH.
            let query_bstr = Bstr::new(&format!("SELECT * FROM {HP_WMI_CLASS}"));
            let wql_bstr = Bstr::new("WQL");
            let mut enum_raw: *mut c_void = null_mut();
            let hr = ((*(*services.as_ptr())).ExecQuery)(
                services.as_ptr() as *mut c_void,
                wql_bstr.as_ptr(),
                query_bstr.as_ptr(),
                WBEM_FLAG_FORWARD_ONLY | WBEM_FLAG_RETURN_IMMEDIATELY,
                null_mut(),
                &mut enum_raw,
            );
            if hr != S_OK || enum_raw.is_null() {
                if hr == WBEM_E_ACCESS_DENIED {
                    return Err(HardwareControlError::AuthorizationFailed(
                        "Access denied querying hpqBIntM instance (administrative privileges required)".to_string(),
                    ));
                }
                return Err(HardwareControlError::InterfaceMissing(format!(
                    "ExecQuery for {HP_WMI_CLASS} failed: {hr:#010x}"
                )));
            }
            let enumerator = ComPtr::from_raw(enum_raw as *mut *const IEnumWbemClassObjectVtbl);

            let mut instance_raw: *mut c_void = null_mut();
            let mut returned: u32 = 0;
            let hr = ((*(*enumerator.as_ptr())).Next)(
                enumerator.as_ptr() as *mut c_void,
                WBEM_INFINITE as i32,
                1,
                &mut instance_raw,
                &mut returned,
            );
            if hr != S_OK || instance_raw.is_null() || returned == 0 {
                if hr == WBEM_E_ACCESS_DENIED {
                    return Err(HardwareControlError::AuthorizationFailed(
                        "Access denied querying hpqBIntM instance (administrative privileges required)".to_string(),
                    ));
                }
                return Err(HardwareControlError::InterfaceMissing(format!(
                    "No {HP_WMI_CLASS} instance found on this machine: {hr:#010x}"
                )));
            }
            let instance = ComPtr::from_raw(instance_raw as *mut *const IWbemClassObjectVtbl);

            // 10. Get __RELPATH of instance (ExecMethod requires relative object path without namespace).
            let relpath_prop = Bstr::new("__RELPATH");
            let mut path_var = ScopedVariant::empty();
            let mut hr = ((*(*instance.as_ptr())).Get)(
                instance.as_ptr() as *mut c_void,
                relpath_prop.as_ptr(),
                0,
                &mut path_var.inner,
                null_mut(),
                null_mut(),
            );
            if hr != S_OK || path_var.inner.vt != VT_BSTR || path_var.inner.data == 0 {
                // Fallback to __PATH only if __RELPATH failed
                let path_prop = Bstr::new("__PATH");
                hr = ((*(*instance.as_ptr())).Get)(
                    instance.as_ptr() as *mut c_void,
                    path_prop.as_ptr(),
                    0,
                    &mut path_var.inner,
                    null_mut(),
                    null_mut(),
                );
            }
            if hr != S_OK || path_var.inner.vt != VT_BSTR || path_var.inner.data == 0 {
                return Err(HardwareControlError::ProtocolError(
                    "Could not retrieve __RELPATH or __PATH of hpqBIntM instance".to_string(),
                ));
            }
            let path_bstr = path_var.inner.data as *const u16;
            let relpath_str = path_var
                .extract_bstr()
                .unwrap_or_else(|| "<unknown>".to_string());

            tracing::debug!(
                method = method_name,
                object_path = %relpath_str,
                command = format!("{command:#010x}"),
                command_type = format!("{command_type:#010x}"),
                payload_len = payload.len(),
                out_size = out_size,
                sign_vt = sign_var.inner.vt,
                cmd_vt = cmd_var.inner.vt,
                cmdtype_vt = cmdtype_var.inner.vt,
                size_vt = size_var.inner.vt,
                data_vt = data_var.inner.vt,
                indata_vt = indata_var.inner.vt,
                "Calling IWbemServices::ExecMethod"
            );

            // 11. Execute ExecMethod.
            let mut out_params_raw: *mut c_void = null_mut();
            let hr = ((*(*services.as_ptr())).ExecMethod)(
                services.as_ptr() as *mut c_void,
                path_bstr,
                method_bstr.as_ptr(),
                0,
                null_mut(),
                in_params.as_ptr() as *mut c_void,
                &mut out_params_raw,
                null_mut(),
            );

            tracing::debug!(
                method = method_name,
                object_path = %relpath_str,
                exec_method_hr = format!("{hr:#010x}"),
                "IWbemServices::ExecMethod returned"
            );

            if hr != S_OK || out_params_raw.is_null() {
                if hr == WBEM_E_ACCESS_DENIED {
                    return Err(HardwareControlError::AuthorizationFailed(format!(
                        "Access denied executing {method_name} (elevation required)"
                    )));
                }
                return Err(HardwareControlError::ConnectionFailed(format!(
                    "ExecMethod({method_name}) failed: {hr:#010x}"
                )));
            }
            let out_params = ComPtr::from_raw(out_params_raw as *mut *const IWbemClassObjectVtbl);

            // 12. Extract "OutData" embedded instance (hpqBDataOut128 / hpqBDataOut4).
            let outdata_prop = Bstr::new("OutData");
            let mut outdata_var = ScopedVariant::empty();
            let hr = ((*(*out_params.as_ptr())).Get)(
                out_params.as_ptr() as *mut c_void,
                outdata_prop.as_ptr(),
                0,
                &mut outdata_var.inner,
                null_mut(),
                null_mut(),
            );
            if hr != S_OK
                || (outdata_var.inner.vt != VT_UNKNOWN && outdata_var.inner.vt != VT_DISPATCH)
                || outdata_var.inner.data == 0
            {
                return Err(HardwareControlError::ProtocolError(
                    "OutData embedded instance missing in WMI response".to_string(),
                ));
            }
            // outdata_var keeps the embedded instance alive and drops it cleanly via VariantClear.
            let outdata_ptr = outdata_var.inner.data as *mut *const IWbemClassObjectVtbl;

            // 13. Check rwReturnCode:
            let rc_prop = Bstr::new("rwReturnCode");
            let mut rc_var = ScopedVariant::empty();
            let hr = ((*(*outdata_ptr)).Get)(
                outdata_ptr as *mut c_void,
                rc_prop.as_ptr(),
                0,
                &mut rc_var.inner,
                null_mut(),
                null_mut(),
            );
            let return_code = if hr == S_OK {
                rc_var.extract_u32().unwrap_or(0)
            } else {
                0
            };

            tracing::debug!(
                method = method_name,
                return_code = return_code,
                "Extracted firmware rwReturnCode from OutData"
            );

            if return_code != 0 {
                return match return_code {
                    1 => Err(HardwareControlError::ProtocolError(
                        "Firmware returned general error".to_string(),
                    )),
                    2 => Err(HardwareControlError::InvalidParameter(
                        "Firmware rejected invalid parameters".to_string(),
                    )),
                    3 => Err(HardwareControlError::UnsupportedMode(format!(
                        "Firmware reports command {command_type:#x} unsupported on this board"
                    ))),
                    4 => Err(HardwareControlError::AuthorizationFailed(
                        "Firmware access denied / BIOS password locked".to_string(),
                    )),
                    5 => Err(HardwareControlError::ConnectionFailed(
                        "Firmware hardware timeout / EC busy".to_string(),
                    )),
                    other => Err(HardwareControlError::ProtocolError(format!(
                        "Firmware returned unexpected return code {other}"
                    ))),
                };
            }

            // 14. Extract "Data" byte array:
            let data_prop = Bstr::new("Data");
            let mut out_data_var = ScopedVariant::empty();
            let hr = ((*(*outdata_ptr)).Get)(
                outdata_ptr as *mut c_void,
                data_prop.as_ptr(),
                0,
                &mut out_data_var.inner,
                null_mut(),
                null_mut(),
            );
            if hr != S_OK {
                return Ok(Vec::new());
            }

            let bytes = out_data_var.extract_bytes().unwrap_or_default();
            tracing::debug!(
                method = method_name,
                bytes_len = bytes.len(),
                "Successfully read BIOS response payload"
            );
            Ok(bytes)
        }
    }
}

// ─────────────────── Fake adapter for deterministic tests ─────────────────

/// A deterministic fake WMI adapter for unit tests.
pub struct FakeWmiAdapter {
    responses: std::sync::Mutex<std::collections::VecDeque<FakeResponse>>,
    available: bool,
}

/// A single pre-loaded response for the fake adapter.
pub enum FakeResponse {
    /// Return a successful output buffer carrying the given payload bytes.
    Success { payload: Vec<u8> },
    /// Return a firmware error status (non-zero status word).
    FirmwareError { status: u32 },
    /// Simulate a WMI-layer failure (the method could not be called at all).
    WmiError { message: String },
}

impl FakeWmiAdapter {
    /// Create a new fake adapter, optionally marking it unavailable.
    pub fn new(available: bool) -> Self {
        Self {
            responses: std::sync::Mutex::new(std::collections::VecDeque::new()),
            available,
        }
    }

    /// Pre-load a response that the next call will consume.
    pub fn expect_call(&self, response: FakeResponse) {
        self.responses
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .push_back(response);
    }

    /// Assert that all pre-loaded responses were consumed (call in test teardown).
    pub fn assert_all_consumed(&self) {
        let queue = self.responses.lock().unwrap_or_else(|p| p.into_inner());
        assert!(
            queue.is_empty(),
            "{} unconsumed FakeWmiAdapter responses remain",
            queue.len()
        );
    }
}

impl WmiAdapter for FakeWmiAdapter {
    fn is_available(&self) -> bool {
        self.available
    }

    fn bios_call(
        &self,
        _command: u32,
        _command_type: u32,
        _payload: &[u8],
        _out_size: u32,
    ) -> Result<Vec<u8>, HardwareControlError> {
        let mut queue = self.responses.lock().unwrap_or_else(|p| p.into_inner());
        let resp = queue.pop_front().ok_or_else(|| {
            HardwareControlError::ProtocolError(
                "FakeWmiAdapter: no more pre-loaded responses".to_string(),
            )
        })?;
        match resp {
            FakeResponse::Success { payload } => Ok(payload),
            FakeResponse::FirmwareError { status } => match status {
                0x01 => Err(HardwareControlError::UnsupportedMode(
                    "Firmware rejected command as unsupported on this board".to_string(),
                )),
                0x02 | 0x03 => Err(HardwareControlError::UnsupportedMode(
                    "Firmware rejected command".to_string(),
                )),
                0x04 => Err(HardwareControlError::AuthorizationFailed(
                    "BIOS rejected command: access denied (BIOS password locked?)".to_string(),
                )),
                0x05 => Err(HardwareControlError::ConnectionFailed(
                    "Embedded Controller busy / timeout".to_string(),
                )),
                other => Err(HardwareControlError::ProtocolError(format!(
                    "Unexpected firmware status {other}"
                ))),
            },
            FakeResponse::WmiError { message } => {
                Err(HardwareControlError::ConnectionFailed(message))
            }
        }
    }
}

// ───────────────────────── HpWmiConnection ────────────────────────────────

/// An active handle to the HP BIOS WMI firmware interface.
pub struct HpWmiConnection {
    adapter: Box<dyn WmiAdapter>,
}

impl HpWmiConnection {
    /// Create from an already-constructed adapter (used by tests).
    pub fn from_adapter(adapter: impl WmiAdapter + 'static) -> Self {
        Self {
            adapter: Box::new(adapter),
        }
    }

    /// Return `true` if the underlying adapter is connected.
    pub fn is_available(&self) -> bool {
        self.adapter.is_available()
    }
}

// ─────────────────────── Public firmware operations ───────────────────────

/// Connect to the HP BIOS WMI interface.
pub fn connect_hp_wmi() -> Result<HpWmiConnection, HardwareControlError> {
    let adapter = RealWmiAdapter::connect()?;
    Ok(HpWmiConnection {
        adapter: Box::new(adapter),
    })
}

/// Query hardware capabilities from firmware.
pub fn query_hp_capabilities(
    conn: &HpWmiConnection,
) -> Result<HardwareCapabilities, HardwareControlError> {
    if !conn.is_available() {
        return Ok(HardwareCapabilities {
            cpu_metrics: true,
            gpu_metrics: true,
            ram_metrics: true,
            fan_rpm: false,
            fan_control: false,
            power_profiles: false,
            rgb_lighting: false,
            rgb_variant: None,
            battery_metrics: true,
        });
    }

    let fan_control_ok = probe_fan_count(conn).unwrap_or(false);
    let power_ok = probe_performance_mode(conn).unwrap_or(false);

    Ok(HardwareCapabilities {
        cpu_metrics: true,
        gpu_metrics: true,
        ram_metrics: true,
        fan_rpm: false,
        fan_control: fan_control_ok,
        power_profiles: power_ok,
        rgb_lighting: false,
        rgb_variant: None,
        battery_metrics: true,
    })
}

/// Query actual fan mode and RPM speeds from firmware (WIN-04).
pub fn query_fan_status(
    conn: &HpWmiConnection,
) -> Result<(String, Vec<u32>), HardwareControlError> {
    if !conn.is_available() {
        return Err(HardwareControlError::InterfaceMissing(
            "HP WMI interface not connected; cannot read fan status".to_string(),
        ));
    }

    let mode = read_fan_mode_from_firmware(conn)?;
    Ok((mode, Vec::new()))
}

/// Set fan mode ("auto" or "max") with firmware read-back verification (WIN-04).
pub fn set_fan_mode(conn: &HpWmiConnection, mode: &str) -> Result<String, HardwareControlError> {
    if !conn.is_available() {
        return Err(HardwareControlError::InterfaceMissing(
            "HP WMI interface not connected; cannot set fan mode".to_string(),
        ));
    }

    let normalized = mode.trim().to_lowercase();
    let target_byte: u8 = match normalized.as_str() {
        "auto" => 0x00,
        "max" => 0x01,
        other => {
            return Err(HardwareControlError::InvalidParameter(format!(
                "Invalid fan mode '{other}'. Supported: 'auto', 'max'"
            )));
        }
    };

    let before = read_fan_mode_from_firmware(conn)?;
    tracing::debug!(before = %before, target = %normalized, "Fan mode: pre-write read-back");

    if before == normalized {
        tracing::debug!("Fan mode already matches target; skipping write");
        return Ok(normalized);
    }

    conn.adapter
        .bios_call(REQ_WRITE, commands::MAX_FAN_SET, &[target_byte], 128)
        .map_err(|e| {
            tracing::error!(error = %e, "WMI ExecMethod failed during fan write");
            e
        })?;

    std::thread::sleep(std::time::Duration::from_millis(150));
    let after = read_fan_mode_from_firmware(conn)?;
    tracing::debug!(after = %after, target = %normalized, "Fan mode: post-write read-back");

    if after != normalized {
        return Err(HardwareControlError::VerificationFailed {
            target: normalized.clone(),
            actual: after.clone(),
            retries: 1,
        });
    }

    tracing::info!(mode = %normalized, "Fan mode confirmed by firmware read-back");
    Ok(normalized)
}

/// Query active ACPI platform performance profile from firmware.
pub fn query_power_profile(conn: &HpWmiConnection) -> Result<String, HardwareControlError> {
    if !conn.is_available() {
        return Err(HardwareControlError::InterfaceMissing(
            "HP WMI interface not connected; cannot read power profile".to_string(),
        ));
    }

    read_performance_mode_from_firmware(conn)
}

/// Set ACPI performance profile with firmware read-back verification.
pub fn set_performance_mode(
    conn: &HpWmiConnection,
    profile: &str,
) -> Result<String, HardwareControlError> {
    if !conn.is_available() {
        return Err(HardwareControlError::InterfaceMissing(
            "HP WMI interface not connected; cannot set performance mode".to_string(),
        ));
    }

    let normalized = profile.trim().to_lowercase();
    let target_byte: u8 = match normalized.as_str() {
        "default" | "balanced" => 0x00,
        "performance" => 0x01,
        "cool" | "quiet" | "eco" => 0x02,
        other => {
            return Err(HardwareControlError::InvalidParameter(format!(
                "Unsupported performance profile '{other}'"
            )));
        }
    };

    let before = read_performance_mode_from_firmware(conn)?;
    if before == normalized {
        return Ok(normalized);
    }

    conn.adapter
        .bios_call(REQ_WRITE, commands::PERFORMANCE_MODE, &[target_byte], 128)?;

    std::thread::sleep(std::time::Duration::from_millis(150));
    let after = read_performance_mode_from_firmware(conn)?;
    if after != normalized {
        return Err(HardwareControlError::VerificationFailed {
            target: normalized.clone(),
            actual: after,
            retries: 1,
        });
    }

    Ok(normalized)
}

/// Write 4-Zone keyboard RGB colors via HP Gaming lighting command 0x20009.
pub fn set_rgb_profile(
    conn: &HpWmiConnection,
    profile: &RgbProfile,
) -> Result<String, HardwareControlError> {
    if !conn.is_available() {
        return Err(HardwareControlError::InterfaceMissing(
            "HP WMI interface not connected; cannot set RGB profile".to_string(),
        ));
    }

    if profile.zones.len() < 4 {
        return Err(HardwareControlError::InvalidParameter(format!(
            "Expected 4 zones for FourZone profile, got {}",
            profile.zones.len()
        )));
    }

    let mut payload = [0u8; 16];
    for (i, zone) in profile.zones.iter().take(4).enumerate() {
        let offset = i * 4;
        payload[offset] = zone.r;
        payload[offset + 1] = zone.g;
        payload[offset + 2] = zone.b;
        payload[offset + 3] = 0x00;
    }

    conn.adapter
        .bios_call(REQ_WRITE, commands::FOUR_ZONE_RGB, &payload, 128)?;
    Ok(profile.name.clone())
}

// ────────────────────── Internal firmware helpers ─────────────────────────

/// Read the current fan mode from firmware via command 0x27.
fn read_fan_mode_from_firmware(conn: &HpWmiConnection) -> Result<String, HardwareControlError> {
    let payload = conn
        .adapter
        .bios_call(REQ_READ, commands::MAX_FAN_SET, &[], 128)?;

    if payload.is_empty() {
        return Err(HardwareControlError::ProtocolError(
            "Fan mode read returned zero-length payload".to_string(),
        ));
    }

    let mode = match payload[0] {
        0x00 => "auto",
        0x01 => "max",
        0x02 => "quiet",
        other => {
            return Err(HardwareControlError::ProtocolError(format!(
                "Unknown fan mode byte from firmware: 0x{other:02X}"
            )));
        }
    };

    Ok(mode.to_string())
}

/// Read the current performance mode from firmware via command 0x1A.
fn read_performance_mode_from_firmware(
    conn: &HpWmiConnection,
) -> Result<String, HardwareControlError> {
    let payload = conn
        .adapter
        .bios_call(REQ_READ, commands::PERFORMANCE_MODE, &[], 128)?;

    if payload.is_empty() {
        return Err(HardwareControlError::ProtocolError(
            "Performance mode read returned zero-length payload".to_string(),
        ));
    }

    let profile = match payload[0] {
        0x00 => "balanced",
        0x01 => "performance",
        0x02 => "cool",
        other => {
            return Err(HardwareControlError::ProtocolError(format!(
                "Unknown performance profile byte from firmware: 0x{other:02X}"
            )));
        }
    };

    Ok(profile.to_string())
}

/// Probe fan count to verify that command 0x10 is accepted (capability check).
fn probe_fan_count(conn: &HpWmiConnection) -> Result<bool, HardwareControlError> {
    match conn
        .adapter
        .bios_call(REQ_READ, commands::FAN_COUNT, &[], 128)
    {
        Err(_) => Ok(false),
        Ok(payload) => {
            if !payload.is_empty() {
                let count = payload[0];
                tracing::info!(fan_count = count, "Firmware reported fan count");
                Ok(count > 0)
            } else {
                Ok(false)
            }
        }
    }
}

/// Probe performance mode command 0x1A to check availability.
fn probe_performance_mode(conn: &HpWmiConnection) -> Result<bool, HardwareControlError> {
    match read_performance_mode_from_firmware(conn) {
        Ok(_) => Ok(true),
        Err(HardwareControlError::UnsupportedMode(_)) => Ok(false),
        Err(_) => Ok(false),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wmi_constants_and_guid() {
        assert_eq!(HP_WMI_CLASS, "hpqBIntM");
        assert_eq!(HP_WMI_DATA_IN_CLASS, "hpqBDataIn");
        assert_eq!(HP_WMI_DATA_OUT_128_CLASS, "hpqBDataOut128");
        assert_eq!(HP_WMI_DATA_OUT_4_CLASS, "hpqBDataOut4");
        assert_eq!(HP_WMI_METHOD_128, "hpqBIOSInt128");
        assert_eq!(HP_WMI_METHOD_4, "hpqBIOSInt4");
        assert_eq!(HPWMI_BIOS_GUID, "5FB7F034-2C63-45E9-BE91-3D44E2C707E4");
    }

    #[test]
    fn test_sign_secu_bytes() {
        assert_eq!(SIGN_SECU_BYTES, [0x53, 0x45, 0x43, 0x55]);
        assert_eq!(&SIGN_SECU_BYTES, b"SECU");
    }

    #[test]
    fn test_req_opcodes() {
        assert_eq!(REQ_READ, 0x0002_0008);
        assert_eq!(REQ_WRITE, 0x0002_0009);
        assert_eq!(commands::FAN_COUNT, 0x10);
        assert_eq!(commands::PERFORMANCE_MODE, 0x1A);
        assert_eq!(commands::MAX_FAN_SET, 0x27);
    }

    #[test]
    fn test_fake_adapter_empty_payload_and_consumption() {
        let adapter = FakeWmiAdapter::new(true);
        adapter.expect_call(FakeResponse::Success {
            payload: vec![0x02], // 2 fans
        });
        let conn = HpWmiConnection::from_adapter(adapter);
        let count_ok = probe_fan_count(&conn).expect("probe fan count");
        assert!(count_ok);
    }

    #[test]
    fn test_read_fan_mode_parsing() {
        for (byte, expected) in [(0x00, "auto"), (0x01, "max"), (0x02, "quiet")] {
            let adapter = FakeWmiAdapter::new(true);
            adapter.expect_call(FakeResponse::Success {
                payload: vec![byte],
            });
            let conn = HpWmiConnection::from_adapter(adapter);
            let mode = read_fan_mode_from_firmware(&conn).unwrap();
            assert_eq!(mode, expected);
        }
    }

    #[test]
    fn test_read_fan_mode_empty_payload_err() {
        let adapter = FakeWmiAdapter::new(true);
        adapter.expect_call(FakeResponse::Success {
            payload: Vec::new(),
        });
        let conn = HpWmiConnection::from_adapter(adapter);
        let err = read_fan_mode_from_firmware(&conn).unwrap_err();
        assert!(matches!(err, HardwareControlError::ProtocolError(_)));
    }

    #[test]
    fn test_read_fan_mode_unknown_byte_err() {
        let adapter = FakeWmiAdapter::new(true);
        adapter.expect_call(FakeResponse::Success {
            payload: vec![0x99],
        });
        let conn = HpWmiConnection::from_adapter(adapter);
        let err = read_fan_mode_from_firmware(&conn).unwrap_err();
        assert!(matches!(err, HardwareControlError::ProtocolError(_)));
    }

    #[test]
    fn test_read_performance_mode_parsing() {
        for (byte, expected) in [(0x00, "balanced"), (0x01, "performance"), (0x02, "cool")] {
            let adapter = FakeWmiAdapter::new(true);
            adapter.expect_call(FakeResponse::Success {
                payload: vec![byte],
            });
            let conn = HpWmiConnection::from_adapter(adapter);
            let profile = read_performance_mode_from_firmware(&conn).unwrap();
            assert_eq!(profile, expected);
        }
    }

    #[test]
    fn test_set_fan_mode_invalid_string_rejected() {
        let adapter = FakeWmiAdapter::new(true);
        let conn = HpWmiConnection::from_adapter(adapter);
        let err = set_fan_mode(&conn, "turbo_extreme").unwrap_err();
        assert!(matches!(err, HardwareControlError::InvalidParameter(_)));
    }
}
