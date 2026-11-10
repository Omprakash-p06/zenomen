//! HP BIOS WMI firmware interface driver (`hpqBIntM` / `hpqBIOSInt*`) (WIN-04).
//!
//! This module implements **real** WMI communication with the HP BIOS firmware on
//! Windows. On non-Windows targets all public functions return
//! `HardwareControlError::InterfaceMissing`.
//!
//! ## Architecture
//!
//! All firmware operations go through the [`WmiAdapter`] trait so that unit tests
//! can inject a fake adapter without touching COM or real hardware. Production
//! code uses [`RealWmiAdapter`], which connects to `ROOT\WMI` and executes the
//! `hpqBIntM::hpqBIOSInt*` method whose output size matches the request.
//!
//! ## Protocol (HP BIOS mailbox)
//!
//! The transport envelope is identical for reads and writes, matching
//! `hp_wmi_perform_query()` in `drivers/platform/x86/hp/hp-wmi.c`:
//!
//! - `Sign`        = `53 45 43 55` (ASCII `SECU`)
//! - `Command`     = [`HPWMI_GM`] (`0x20008`) for **every** gaming/fan command
//! - `CommandType` = the operation (`0x10`, `0x26`, `0x27`, `0x2D`, ...)
//! - `Size`        = number of *meaningful* payload bytes
//! - `hpqBData`    = payload; omitted entirely when `Size == 0`
//!
//! `Command = 0x20009` is the keyboard-lighting family, **not** a write flag.
//! Using it for fan writes is rejected by firmware as an unknown command.
//!
//! Board `8A3D` is a Victus S board: fan speed comes from command `0x2D`
//! (100 RPM units) and manual levels from `0x2E`. See [`crate::service::windows::victus_s`].
//!
//! ## Invariants
//!
//! - `connect_hp_wmi` returns `Err(InterfaceMissing)` when WMI or the class is unreachable.
//! - `query_fan_status` reads firmware state; it **never** reads process-local variables.
//! - `set_fan_mode` reads firmware **after** writing; success is returned only after a matching
//!   read-back. If read-back disagrees the error is `VerificationFailed`.
//! - Every property `Put` result is checked. A rejected input object surfaces as a precise
//!   `ProtocolError` naming the property instead of an opaque `0x80041008`.
//! - No process-local fan or power statics exist.

use std::sync::OnceLock;

use crate::hardware::HardwareCapabilities;
use crate::rgb::schema::RgbProfile;
use crate::service::types::HardwareControlError;
use crate::service::windows::victus_s::{self, FanTable};

// ─────────────────────────── Public constants ──────────────────────────────

/// WMI namespace for HP ACPI BIOS objects.
pub const HP_WMI_NAMESPACE: &str = r"ROOT\WMI";

/// WMI class name for HP BIOS mailbox interface.
pub const HP_WMI_CLASS: &str = "hpqBIntM";

/// WMI method name for 128-byte payloads on `hpqBIntM`.
pub const HP_WMI_METHOD_128: &str = "hpqBIOSInt128";
/// WMI method name for 4-byte payloads.
pub const HP_WMI_METHOD_4: &str = "hpqBIOSInt4";
/// WMI method name for zero-length payloads (no `Data` output property).
pub const HP_WMI_METHOD_0: &str = "hpqBIOSInt0";
/// WMI method name for 1024-byte payloads.
pub const HP_WMI_METHOD_1024: &str = "hpqBIOSInt1024";
/// WMI method name for 4096-byte payloads.
pub const HP_WMI_METHOD_4096: &str = "hpqBIOSInt4096";

/// HP BIOS input and output class names.
pub const HP_WMI_DATA_IN_CLASS: &str = "hpqBDataIn";
pub const HP_WMI_DATA_OUT_128_CLASS: &str = "hpqBDataOut128";
pub const HP_WMI_DATA_OUT_4_CLASS: &str = "hpqBDataOut4";

/// HP BIOS WMI GUID (`5FB7F034-2C63-45E9-BE91-3D44E2C707E4`).
pub const HPWMI_BIOS_GUID: &str = "5FB7F034-2C63-45E9-BE91-3D44E2C707E4";

/// `Sign` property: ASCII "SECU".
pub const SIGN_SECU_BYTES: [u8; 4] = *b"SECU";

/// `HPWMI_GM` — command family for every gaming/thermal/fan operation (read and write).
///
/// Upstream `hp_wmi_perform_query()` passes `HPWMI_GM` for fan count, fan speed,
/// Max Fan get/set, thermal profile and all Victus S queries.
pub const HPWMI_GM: u32 = 0x0002_0008;

/// `0x20009` — keyboard-lighting command family. Not a write flag.
pub const HPWMI_KB_LIGHTING: u32 = 0x0002_0009;

/// Upper bound on a plausible firmware fan count.
pub const MAX_FAN_COUNT: u8 = 8;

/// HP Gaming command family identifiers (`enum hp_wmi_gm_commandtype`).
pub mod commands {
    /// `HPWMI_FAN_COUNT_GET_QUERY` — also arms the 120 s user-defined fan state.
    pub const FAN_COUNT: u32 = 0x10;
    /// `HPWMI_FAN_SPEED_GET_QUERY` — generic fan RPM (not used on Victus S).
    pub const FAN_SPEEDS: u32 = 0x11;
    /// `HPWMI_SET_PERFORMANCE_MODE` — thermal profile write.
    pub const PERFORMANCE_MODE: u32 = 0x1A;
    /// `HPWMI_FAN_SPEED_MAX_GET_QUERY` — read Max Fan state.
    pub const MAX_FAN_STATUS: u32 = 0x26;
    /// `HPWMI_FAN_SPEED_MAX_SET_QUERY` — write Max Fan state.
    pub const MAX_FAN_SET: u32 = 0x27;
    /// `HPWMI_GET_SYSTEM_DESIGN_DATA` — platform capability block.
    pub const SYSTEM_DESIGN_DATA: u32 = 0x28;
    /// `HPWMI_THERMAL_PROFILE_QUERY` (opcode `0x4C`) — candidate thermal profile query.
    ///
    /// Documented in upstream Linux `hp-wmi` (`thermal_profile_get()`). On platforms
    /// with ACPI/WMI-managed platform profiles, this returns the active profile byte/int.
    /// On board `8A3D`, the active profile is stored in EC register `0x95` instead, so
    /// `0x4C` is candidate diagnostic evidence only, never used for production writes.
    pub const THERMAL_PROFILE_QUERY: u32 = 0x4C;
}

/// Map a firmware return code onto a typed error.
///
/// Uses the documented `hp_return_value` values: `2` wrong signature,
/// `3` unknown command, `4` unknown command type, `5` invalid parameters.
/// Do not fold these into generic COM errors — they identify the failing layer.
pub fn firmware_error(command_type: u32, code: u32) -> HardwareControlError {
    match code {
        2 => HardwareControlError::ProtocolError(format!(
            "firmware rejected the BIOS signature ('SECU') for command type {command_type:#04x} \
             (status 2 = wrong signature)"
        )),
        3 => HardwareControlError::UnsupportedMode(format!(
            "firmware reports unknown command for command type {command_type:#04x} \
             (status 3 = unknown command)"
        )),
        4 => HardwareControlError::UnsupportedMode(format!(
            "firmware reports unknown command type {command_type:#04x} \
             (status 4 = unknown command type)"
        )),
        5 => HardwareControlError::InvalidParameter(format!(
            "firmware reports invalid parameters for command type {command_type:#04x} \
             (status 5 = invalid parameters)"
        )),
        other => HardwareControlError::ProtocolError(format!(
            "firmware returned undocumented status {other} for command type {command_type:#04x}"
        )),
    }
}

/// Select the `hpqBIOSInt*` method for a requested output size.
///
/// Matches `encode_outsize_for_pvsz()` in the upstream driver.
pub fn method_for_output_size(out_size: u32) -> &'static str {
    match out_size {
        0 => HP_WMI_METHOD_0,
        1..=4 => HP_WMI_METHOD_4,
        5..=128 => HP_WMI_METHOD_128,
        129..=1024 => HP_WMI_METHOD_1024,
        _ => HP_WMI_METHOD_4096,
    }
}

/// Describes a property to be populated in the `hpqBDataIn` WMI envelope.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlannedEnvelopeProperty {
    pub name: String,
    pub vt: u16,
    pub expected_cim: i32,
    pub vt_type_arg: i32,
}

/// Plan the properties for an `hpqBDataIn` envelope without touching COM.
///
/// Deterministically computes the sequence of `Put` operations required for
/// `hpqBDataIn`, enforcing:
/// 1. `Sign` uses `VT_ARRAY | VT_UI1` and `CIM_UINT8 | CIM_FLAG_ARRAY`.
/// 2. `Command`, `CommandType`, and `Size` use `VT_I4` and `CIM_UINT32`.
/// 3. If `payload` is empty, `hpqBData` is omitted entirely.
/// 4. If `payload` is non-empty, `hpqBData` is included with `VT_ARRAY | VT_UI1`.
pub fn plan_hpq_input_envelope(
    command: u32,
    command_type: u32,
    payload: &[u8],
) -> Vec<PlannedEnvelopeProperty> {
    const VT_I4: u16 = 3;
    const VT_UI1: u16 = 17;
    const VT_ARRAY: u16 = 0x2000;
    const CIM_UINT8: i32 = 17;
    const CIM_UINT32: i32 = 19;
    const CIM_FLAG_ARRAY: i32 = 0x2000;

    let _ = (command, command_type);
    let mut props = vec![
        PlannedEnvelopeProperty {
            name: "Sign".to_string(),
            vt: VT_ARRAY | VT_UI1,
            expected_cim: CIM_UINT8 | CIM_FLAG_ARRAY,
            vt_type_arg: 0,
        },
        PlannedEnvelopeProperty {
            name: "Command".to_string(),
            vt: VT_I4,
            expected_cim: CIM_UINT32,
            vt_type_arg: 0,
        },
        PlannedEnvelopeProperty {
            name: "CommandType".to_string(),
            vt: VT_I4,
            expected_cim: CIM_UINT32,
            vt_type_arg: 0,
        },
        PlannedEnvelopeProperty {
            name: "Size".to_string(),
            vt: VT_I4,
            expected_cim: CIM_UINT32,
            vt_type_arg: 0,
        },
    ];

    if !payload.is_empty() {
        props.push(PlannedEnvelopeProperty {
            name: "hpqBData".to_string(),
            vt: VT_ARRAY | VT_UI1,
            expected_cim: CIM_UINT8 | CIM_FLAG_ARRAY,
            vt_type_arg: 0,
        });
    }

    props
}

/// Abstract property sink for deterministic unit testing of WMI object construction.
pub trait EnvelopeSink {
    fn put_prop(
        &mut self,
        name: &str,
        vt: u16,
        expected_cim: i32,
        vt_type_arg: i32,
    ) -> Result<(), HardwareControlError>;
}

/// Populates an envelope using the given sink, immediately halting on any error.
pub fn populate_hpq_envelope<S: EnvelopeSink>(
    sink: &mut S,
    command: u32,
    command_type: u32,
    payload: &[u8],
) -> Result<(), HardwareControlError> {
    let plan = plan_hpq_input_envelope(command, command_type, payload);
    for prop in plan {
        sink.put_prop(&prop.name, prop.vt, prop.expected_cim, prop.vt_type_arg)?;
    }
    Ok(())
}

// ──────────────────────────── WmiAdapter trait ─────────────────────────────

/// Injectable WMI adapter abstraction — real COM on Windows, fake in tests.
///
/// The only requirement is that `bios_call` mirrors the semantics of
/// `IWbemServices::ExecMethod` for the `hpqBIntM` class.
pub trait WmiAdapter: Send + Sync {
    /// Execute an HP BIOS call using the real WMI interface `hpqBIOSInt*`.
    ///
    /// `command` is the command family ([`HPWMI_GM`]); `command_type` is the
    /// operation (`0x10`, `0x27`, `0x2D`, ...). `out_size` selects the method.
    fn bios_call(
        &self,
        command: u32,
        command_type: u32,
        payload: &[u8],
        out_size: u32,
    ) -> Result<Vec<u8>, HardwareControlError>;

    /// Return `true` if the adapter is connected and the class exists.
    fn is_available(&self) -> bool;
}

// ─────────────────────── Real WMI adapter (Windows only) ──────────────────

/// Real COM/WMI adapter that connects to `ROOT\WMI` and executes `hpqBIOSInt*`.
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
            let ns_bstr = Bstr::new(HP_WMI_NAMESPACE);
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

/// Set one property on an `IWbemClassObject`, reporting the COM error text on failure.
///
/// # Safety
///
/// `obj` must be a live `IWbemClassObject` interface obtained from WMI, and the
/// owning [`crate::service::windows::wmi_ffi::ComPtr`] must outlive this call.
#[cfg(windows)]
pub(crate) unsafe fn put_property(
    obj: *mut *const crate::service::windows::wmi_ffi::IWbemClassObjectVtbl,
    name: &str,
    value: &crate::service::windows::wmi_ffi::VARIANT,
    expected_cim: i32,
    vt_type_arg: i32,
) -> Result<(), HardwareControlError> {
    use crate::service::windows::wmi_ffi::{Bstr, S_OK, last_error_description};
    use std::ffi::c_void;

    tracing::debug!(
        property = name,
        actual_vt = value.vt,
        expected_cim = expected_cim,
        vt_type_arg = vt_type_arg,
        "Setting WMI object property via IWbemClassObject::Put"
    );

    let name_bstr = Bstr::new(name);
    // SAFETY: see the function contract — `obj` is a live WMI object.
    let hr = unsafe {
        ((*(*obj)).Put)(
            obj as *mut c_void,
            name_bstr.as_ptr(),
            0,
            value,
            vt_type_arg,
        )
    };
    if hr != S_OK {
        let detail = last_error_description().unwrap_or_else(|| "no IErrorInfo".to_string());
        tracing::warn!(
            property = name,
            hr = hr,
            actual_vt = value.vt,
            expected_cim = expected_cim,
            detail = %detail,
            "IWbemClassObject::Put failed"
        );
        return Err(HardwareControlError::ProtocolError(format!(
            "IWbemClassObject::Put({name}) failed: HRESULT {hr:#010x} (actual vt={}, expected cim={expected_cim}: {detail}); \
             the hpqBDataIn envelope was rejected before ExecMethod",
            value.vt
        )));
    }
    Ok(())
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

        let method_name = method_for_output_size(out_size);
        let method_bstr = Bstr::new(method_name);

        unsafe {
            // 1. Create IWbemLocator and connect to ROOT\WMI.
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

            let ns_bstr = Bstr::new(HP_WMI_NAMESPACE);
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

            // 2. Fetch the hpqBIntM class definition and the method's in-signature.
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

            // 3. Build the hpqBDataIn instance and populate every property.
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

            let sign_var = ScopedVariant::from_bytes(&SIGN_SECU_BYTES)?;
            put_property(
                datain_inst.as_ptr(),
                "Sign",
                &sign_var.inner,
                CIM_UINT8 | CIM_FLAG_ARRAY,
                0,
            )?;

            let cmd_var = ScopedVariant::from_cim_uint32(command);
            put_property(
                datain_inst.as_ptr(),
                "Command",
                &cmd_var.inner,
                CIM_UINT32,
                0,
            )?;

            let cmdtype_var = ScopedVariant::from_cim_uint32(command_type);
            put_property(
                datain_inst.as_ptr(),
                "CommandType",
                &cmdtype_var.inner,
                CIM_UINT32,
                0,
            )?;

            // `Size` is the number of meaningful payload bytes, not the buffer size.
            let size_var = ScopedVariant::from_cim_uint32(payload.len() as u32);
            put_property(datain_inst.as_ptr(), "Size", &size_var.inner, CIM_UINT32, 0)?;

            // `hpqBData` is a variable-length array: omit it entirely for empty
            // requests. A zero-element SAFEARRAY is not the same thing and is
            // rejected by the provider.
            // Keep the payload VARIANT alive until ExecMethod returns.
            let _data_var = if payload.is_empty() {
                None
            } else {
                let var = ScopedVariant::from_bytes(payload)?;
                put_property(
                    datain_inst.as_ptr(),
                    "hpqBData",
                    &var.inner,
                    CIM_UINT8 | CIM_FLAG_ARRAY,
                    0,
                )?;
                Some(var)
            };

            // 4. Attach the embedded object as the method's `InData` parameter.
            let indata_var = ScopedVariant::from_unknown(datain_inst.as_ptr() as *mut c_void);
            put_property(
                in_params.as_ptr(),
                "InData",
                &indata_var.inner,
                CIM_OBJECT,
                CIM_OBJECT,
            )?;

            // 5. Resolve a namespace-relative object path for the active instance.
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
                        "Access denied querying hpqBIntM instance (administrative privileges required)"
                            .to_string(),
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
                        "Access denied querying hpqBIntM instance (administrative privileges required)"
                            .to_string(),
                    ));
                }
                return Err(HardwareControlError::InterfaceMissing(format!(
                    "No {HP_WMI_CLASS} instance found on this machine: {hr:#010x}"
                )));
            }
            let instance = ComPtr::from_raw(instance_raw as *mut *const IWbemClassObjectVtbl);

            // `__RELPATH` is namespace-relative and is what ExecMethod requires.
            // `__PATH` is only a diagnostic fallback: it is server-qualified.
            let relpath = {
                let mut var = ScopedVariant::empty();
                let prop = Bstr::new("__RELPATH");
                let hr = ((*(*instance.as_ptr())).Get)(
                    instance.as_ptr() as *mut c_void,
                    prop.as_ptr(),
                    0,
                    &mut var.inner,
                    null_mut(),
                    null_mut(),
                );
                if hr == S_OK { var.extract_bstr() } else { None }
            };
            let relpath = match relpath {
                Some(path) => path,
                None => {
                    let mut var = ScopedVariant::empty();
                    let prop = Bstr::new("__PATH");
                    let hr = ((*(*instance.as_ptr())).Get)(
                        instance.as_ptr() as *mut c_void,
                        prop.as_ptr(),
                        0,
                        &mut var.inner,
                        null_mut(),
                        null_mut(),
                    );
                    let path = if hr == S_OK { var.extract_bstr() } else { None };
                    let path = path.ok_or_else(|| {
                        HardwareControlError::ProtocolError(
                            "Could not retrieve __RELPATH or __PATH of hpqBIntM instance"
                                .to_string(),
                        )
                    })?;
                    tracing::warn!(
                        object_path = %path,
                        "__RELPATH unavailable; falling back to server-qualified __PATH"
                    );
                    path
                }
            };
            let path_bstr = Bstr::new(&relpath);

            tracing::debug!(
                method = method_name,
                object_path = %relpath,
                command = format!("{command:#010x}"),
                command_type = format!("{command_type:#010x}"),
                payload_len = payload.len(),
                out_size = out_size,
                "Calling IWbemServices::ExecMethod"
            );

            // 6. Synchronous ExecMethod: lFlags = 0, ppOutParams non-NULL, ppCallResult NULL.
            let mut out_params_raw: *mut c_void = null_mut();
            let hr = ((*(*services.as_ptr())).ExecMethod)(
                services.as_ptr() as *mut c_void,
                path_bstr.as_ptr(),
                method_bstr.as_ptr(),
                0,
                null_mut(),
                in_params.as_ptr() as *mut c_void,
                &mut out_params_raw,
                null_mut(),
            );

            if hr != S_OK || out_params_raw.is_null() {
                let detail =
                    last_error_description().unwrap_or_else(|| "no IErrorInfo".to_string());
                if hr == WBEM_E_ACCESS_DENIED {
                    return Err(HardwareControlError::AuthorizationFailed(format!(
                        "Access denied executing {method_name} (elevation required)"
                    )));
                }
                return Err(HardwareControlError::ConnectionFailed(format!(
                    "ExecMethod({method_name}) failed: HRESULT {hr:#010x} ({detail}); \
                     command type {command_type:#04x}"
                )));
            }
            let out_params = ComPtr::from_raw(out_params_raw as *mut *const IWbemClassObjectVtbl);

            // 7. Extract the OutData embedded instance.
            let outdata_name = Bstr::new("OutData");
            let mut outdata_var = ScopedVariant::empty();
            let hr = ((*(*out_params.as_ptr())).Get)(
                out_params.as_ptr() as *mut c_void,
                outdata_name.as_ptr(),
                0,
                &mut outdata_var.inner,
                null_mut(),
                null_mut(),
            );
            if hr != S_OK
                || (outdata_var.inner.vt != VT_UNKNOWN && outdata_var.inner.vt != VT_DISPATCH)
                || outdata_var.inner.data == 0
            {
                return Err(HardwareControlError::ProtocolError(format!(
                    "OutData embedded instance missing in {method_name} response \
                     (HRESULT {hr:#010x}, vt {:#x})",
                    outdata_var.inner.vt
                )));
            }
            // `outdata_var` keeps the embedded instance alive and releases it via VariantClear.
            let outdata_ptr = outdata_var.inner.data as *mut *const IWbemClassObjectVtbl;

            // 8. Firmware status. A missing status is an error, never "assume success".
            let rc_name = Bstr::new("rwReturnCode");
            let mut rc_var = ScopedVariant::empty();
            let hr = ((*(*outdata_ptr)).Get)(
                outdata_ptr as *mut c_void,
                rc_name.as_ptr(),
                0,
                &mut rc_var.inner,
                null_mut(),
                null_mut(),
            );
            if hr != S_OK {
                return Err(HardwareControlError::ProtocolError(format!(
                    "OutData.rwReturnCode missing in {method_name} response: HRESULT {hr:#010x}"
                )));
            }
            let return_code = rc_var.extract_u32().ok_or_else(|| {
                HardwareControlError::ProtocolError(format!(
                    "OutData.rwReturnCode had unexpected VARIANT type {:#x}",
                    rc_var.inner.vt
                ))
            })?;

            tracing::debug!(
                method = method_name,
                return_code = return_code,
                command_type = format!("{command_type:#04x}"),
                "Firmware status received"
            );

            if return_code != 0 {
                return Err(firmware_error(command_type, return_code));
            }

            // `hpqBIOSInt0` carries no `Data` property at all.
            if out_size == 0 {
                return Ok(Vec::new());
            }

            // 9. Response payload.
            let data_name = Bstr::new("Data");
            let mut out_data_var = ScopedVariant::empty();
            let hr = ((*(*outdata_ptr)).Get)(
                outdata_ptr as *mut c_void,
                data_name.as_ptr(),
                0,
                &mut out_data_var.inner,
                null_mut(),
                null_mut(),
            );
            if hr != S_OK {
                return Err(HardwareControlError::ProtocolError(format!(
                    "OutData.Data missing in {method_name} response: HRESULT {hr:#010x}"
                )));
            }

            let mut bytes = out_data_var.extract_bytes().ok_or_else(|| {
                HardwareControlError::ProtocolError(format!(
                    "OutData.Data was not a byte array (vt {:#x})",
                    out_data_var.inner.vt
                ))
            })?;
            bytes.truncate(out_size as usize);

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

/// One firmware call recorded by [`FakeWmiAdapter`], for protocol regression tests.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordedCall {
    /// Command family passed to `bios_call`.
    pub command: u32,
    /// Operation identifier passed to `bios_call`.
    pub command_type: u32,
    /// Payload handed to the firmware.
    pub payload: Vec<u8>,
    /// Requested output size (selects the WMI method).
    pub out_size: u32,
}

/// A deterministic fake WMI adapter for unit tests.
pub struct FakeWmiAdapter {
    responses: std::sync::Mutex<std::collections::VecDeque<FakeResponse>>,
    calls: std::sync::Mutex<Vec<RecordedCall>>,
    available: bool,
}

/// A single pre-loaded response for the fake adapter.
pub enum FakeResponse {
    /// Return a successful output buffer carrying the given payload bytes.
    Success { payload: Vec<u8> },
    /// Return a firmware error status word (`hp_return_value`).
    FirmwareError { status: u32 },
    /// Simulate a WMI-layer failure (the method could not be called at all).
    WmiError { message: String },
}

impl FakeWmiAdapter {
    /// Create a new fake adapter, optionally marking it unavailable.
    pub fn new(available: bool) -> Self {
        Self {
            responses: std::sync::Mutex::new(std::collections::VecDeque::new()),
            calls: std::sync::Mutex::new(Vec::new()),
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

    /// Every firmware call this adapter has served, in order.
    pub fn recorded_calls(&self) -> Vec<RecordedCall> {
        self.calls.lock().unwrap_or_else(|p| p.into_inner()).clone()
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
        command: u32,
        command_type: u32,
        payload: &[u8],
        out_size: u32,
    ) -> Result<Vec<u8>, HardwareControlError> {
        self.calls
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .push(RecordedCall {
                command,
                command_type,
                payload: payload.to_vec(),
                out_size,
            });

        let mut queue = self.responses.lock().unwrap_or_else(|p| p.into_inner());
        let resp = queue.pop_front().ok_or_else(|| {
            HardwareControlError::ProtocolError(
                "FakeWmiAdapter: no more pre-loaded responses".to_string(),
            )
        })?;
        match resp {
            FakeResponse::Success { payload } => Ok(payload),
            FakeResponse::FirmwareError { status } => Err(firmware_error(command_type, status)),
            FakeResponse::WmiError { message } => {
                Err(HardwareControlError::ConnectionFailed(message))
            }
        }
    }
}

/// Lets tests keep a shared handle to the fake adapter for post-hoc assertions.
impl WmiAdapter for std::sync::Arc<FakeWmiAdapter> {
    fn is_available(&self) -> bool {
        (**self).is_available()
    }

    fn bios_call(
        &self,
        command: u32,
        command_type: u32,
        payload: &[u8],
        out_size: u32,
    ) -> Result<Vec<u8>, HardwareControlError> {
        (**self).bios_call(command, command_type, payload, out_size)
    }
}

// ───────────────────────── HpWmiConnection ────────────────────────────────

/// An active handle to the HP BIOS WMI firmware interface.
pub struct HpWmiConnection {
    adapter: Box<dyn WmiAdapter>,
    board_id: Option<String>,
    /// Cached fan count. Command `0x10` re-arms the firmware's 120 s manual-fan
    /// window, so telemetry must not send it on every poll.
    fan_count: OnceLock<u8>,
}

impl HpWmiConnection {
    /// Create from an already-constructed adapter without board identity (tests).
    pub fn from_adapter(adapter: impl WmiAdapter + 'static) -> Self {
        Self {
            adapter: Box::new(adapter),
            board_id: None,
            fan_count: OnceLock::new(),
        }
    }

    /// Create from an adapter tagged with the detected motherboard ID.
    pub fn from_adapter_for_board(adapter: impl WmiAdapter + 'static, board_id: &str) -> Self {
        Self {
            adapter: Box::new(adapter),
            board_id: Some(board_id.trim().to_uppercase()),
            fan_count: OnceLock::new(),
        }
    }

    /// Return `true` if the underlying adapter is connected.
    pub fn is_available(&self) -> bool {
        self.adapter.is_available()
    }

    /// Motherboard ID this connection was resolved against, if known.
    pub fn board_id(&self) -> Option<&str> {
        self.board_id.as_deref()
    }

    /// `true` when the board uses the dedicated Victus S fan protocol (`0x2D`/`0x2E`/`0x2F`).
    pub fn is_victus_s(&self) -> bool {
        self.board_id
            .as_deref()
            .is_some_and(victus_s::is_victus_s_board)
    }

    /// Read one fan's RPM speed. Returns `Ok(Some(rpm))` on success, `Ok(None)` if out of range.
    pub fn get_fan_rpm(&self, fan_index: u8) -> Result<Option<u32>, HardwareControlError> {
        get_fan_rpm(self, fan_index)
    }

    fn ensure_available(&self, operation: &str) -> Result<(), HardwareControlError> {
        if self.is_available() {
            Ok(())
        } else {
            Err(HardwareControlError::InterfaceMissing(format!(
                "HP WMI interface not connected; cannot {operation}"
            )))
        }
    }
}

// ─────────────────────── Public firmware operations ───────────────────────

/// Connect to the HP BIOS WMI interface and tag the connection with the detected board.
pub fn connect_hp_wmi() -> Result<HpWmiConnection, HardwareControlError> {
    let adapter = RealWmiAdapter::connect()?;
    let board_id = {
        #[cfg(windows)]
        {
            crate::hardware::windows::device::read_board_id()
        }
        #[cfg(not(windows))]
        {
            None
        }
    };
    let mut conn = HpWmiConnection::from_adapter(adapter);
    conn.board_id = board_id;
    Ok(conn)
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

    let fan_count = get_fan_count(conn).ok();
    let power_ok = probe_performance_mode(conn).unwrap_or(false);

    Ok(HardwareCapabilities {
        cpu_metrics: true,
        gpu_metrics: true,
        ram_metrics: true,
        fan_rpm: fan_count.is_some(),
        fan_control: fan_count.is_some(),
        power_profiles: power_ok,
        rgb_lighting: false,
        rgb_variant: None,
        battery_metrics: true,
    })
}

/// Query actual fan mode and RPM speeds from firmware (WIN-04).
///
/// Fan count is cached after the first query: command `0x10` re-arms the
/// firmware's 120-second user-defined fan window, so a 1 Hz telemetry poll must
/// not resend it.
pub fn query_fan_status(
    conn: &HpWmiConnection,
) -> Result<(String, Vec<u32>), HardwareControlError> {
    conn.ensure_available("read fan status")?;

    let mode = read_fan_mode_from_firmware(conn)?;

    let fan_count = match cached_fan_count(conn) {
        Ok(count) => count,
        Err(e) => {
            tracing::warn!(error = %e, "fan count unavailable; reporting no tachometers");
            0
        }
    };

    let mut rpms = Vec::new();
    for index in 0..fan_count {
        match get_fan_rpm(conn, index) {
            Ok(Some(rpm)) => rpms.push(rpm),
            Ok(None) => tracing::debug!(fan = index, "fan RPM unavailable for index"),
            Err(e) => tracing::warn!(fan = index, error = %e, "fan RPM read failed"),
        }
    }

    Ok((mode, rpms))
}

/// Set fan mode ("auto" or "max") with firmware read-back verification (WIN-04).
pub fn set_fan_mode(conn: &HpWmiConnection, mode: &str) -> Result<String, HardwareControlError> {
    set_fan_mode_with_profile(conn, mode, None)
}

/// Set fan mode with optional explicit thermal profile for boards requiring EC profile restore.
pub fn set_fan_mode_with_profile(
    conn: &HpWmiConnection,
    mode: &str,
    active_profile: Option<victus_s::ThermalProfile>,
) -> Result<String, HardwareControlError> {
    conn.ensure_available("set fan mode")?;

    let normalized = mode.trim().to_lowercase();
    if normalized != "auto" && normalized != "max" {
        return Err(HardwareControlError::InvalidParameter(format!(
            "Invalid fan mode '{normalized}'. Supported: 'auto', 'max'"
        )));
    }

    let before = read_fan_mode_from_firmware(conn)?;
    tracing::debug!(before = %before, target = %normalized, "Fan mode: pre-write read-back");

    if before == normalized {
        tracing::debug!("Fan mode already matches target; skipping write");
        return Ok(normalized);
    }

    if normalized == "max" {
        set_max_fan_state(conn, true)?;
    } else {
        restore_automatic_fan_control(conn, active_profile)?;
    }

    std::thread::sleep(std::time::Duration::from_millis(150));
    let after = read_fan_mode_from_firmware(conn)?;
    tracing::debug!(after = %after, target = %normalized, "Fan mode: post-write read-back");

    if after != normalized {
        return Err(HardwareControlError::VerificationFailed {
            target: normalized.clone(),
            actual: after,
            retries: 1,
        });
    }

    tracing::info!(mode = %normalized, "Fan mode confirmed by firmware read-back");
    Ok(normalized)
}

/// Read the firmware fan count (command `0x10`).
///
/// This query also arms the firmware's 120-second user-defined fan state, so it
/// is deliberately reserved for capability probing and the manual-write arm step.
pub fn get_fan_count(conn: &HpWmiConnection) -> Result<u8, HardwareControlError> {
    conn.ensure_available("read fan count")?;

    let payload = conn
        .adapter
        .bios_call(HPWMI_GM, commands::FAN_COUNT, &[0u8], 4)?;

    let count = payload.first().copied().ok_or_else(|| {
        HardwareControlError::ProtocolError("Fan count query returned no payload bytes".to_string())
    })?;

    if count == 0 || count > MAX_FAN_COUNT {
        return Err(HardwareControlError::ProtocolError(format!(
            "Firmware reported implausible fan count {count} (expected 1..={MAX_FAN_COUNT})"
        )));
    }

    let _ = conn.fan_count.set(count);
    tracing::debug!(fan_count = count, "Firmware reported fan count");
    Ok(count)
}

/// Read the firmware fan count without re-arming the manual-fan window after the
/// first successful query. Use this from periodic telemetry.
pub fn cached_fan_count(conn: &HpWmiConnection) -> Result<u8, HardwareControlError> {
    if let Some(count) = conn.fan_count.get() {
        return Ok(*count);
    }
    get_fan_count(conn)
}

/// Diagnostic probe for opcode `0x4C` (`HPWMI_THERMAL_PROFILE_QUERY`).
///
/// Upstream Linux `hp-wmi` driver exposes `HPWMI_THERMAL_PROFILE_QUERY = 0x4C`
/// to read the active thermal profile on platforms where the profile is exposed
/// via ACPI/WMI rather than the Embedded Controller.
///
/// On motherboard `8A3D`, the active thermal profile is maintained in EC register `0x95`
/// (unreachable from unprivileged userland without an EC driver), so this probe is
/// strictly diagnostic. It must never be used to assume write capability or bypass
/// board validation.
pub fn probe_thermal_profile_query(
    conn: &HpWmiConnection,
) -> Result<Vec<u8>, HardwareControlError> {
    conn.ensure_available("diagnostic probe thermal profile (0x4C)")?;
    conn.adapter
        .bios_call(HPWMI_GM, commands::THERMAL_PROFILE_QUERY, &[], 4)
}

/// Read one fan's RPM.
///
/// Returns:
/// - `Ok(Some(rpm))` when the firmware returns a valid reading (including `0` when stopped).
/// - `Ok(None)` when the requested fan index is out of range for the hardware.
/// - `Err(...)` on transport failure, protocol error, or firmware rejection.
///
/// Victus S boards (`8A3D`) answer `0x2D` with one level byte per fan in 100 RPM
/// units (index 0 = CPU fan, index 1 = GPU fan). Other boards use the generic `0x11`
/// response, where RPM is the big-endian word at bytes 2..4.
pub fn get_fan_rpm(
    conn: &HpWmiConnection,
    fan_index: u8,
) -> Result<Option<u32>, HardwareControlError> {
    conn.ensure_available("read fan RPM")?;

    let max_fans = conn
        .fan_count
        .get()
        .copied()
        .unwrap_or_else(|| if conn.is_victus_s() { 2 } else { MAX_FAN_COUNT });

    if fan_index >= max_fans {
        return Ok(None);
    }

    if conn.is_victus_s() {
        let levels = get_victus_fan_levels(conn)?;
        if (fan_index as usize) < levels.len() {
            return Ok(Some(victus_s::rpm_for_level(levels[fan_index as usize])));
        } else {
            return Err(HardwareControlError::ProtocolError(format!(
                "Victus S fan level response too short ({} bytes) for fan index {fan_index}",
                levels.len()
            )));
        }
    }

    let payload = conn
        .adapter
        .bios_call(HPWMI_GM, commands::FAN_SPEEDS, &[fan_index], 4)?;
    if payload.len() < 4 {
        return Err(HardwareControlError::ProtocolError(format!(
            "Fan speed query returned {} bytes, expected 4",
            payload.len()
        )));
    }
    let raw_rpm = u32::from(payload[2]) << 8 | u32::from(payload[3]);
    Ok(Some(raw_rpm))
}

/// Read the raw Victus S per-fan level table (command `0x2D`).
///
/// Each byte is the fan's current level in 100 RPM units; `0x00` means the fan
/// is under automatic firmware control.
pub fn get_victus_fan_levels(conn: &HpWmiConnection) -> Result<Vec<u8>, HardwareControlError> {
    conn.ensure_available("read Victus S fan levels")?;

    let payload = conn
        .adapter
        .bios_call(HPWMI_GM, victus_s::FAN_LEVEL_GET_QUERY, &[0u8], 128)?;

    if payload.is_empty() {
        return Err(HardwareControlError::ProtocolError(
            "Victus S fan level query returned no payload".to_string(),
        ));
    }
    Ok(payload)
}

/// Query both fans atomically in a single 0x2D call for Victus S boards.
pub fn get_victus_dual_fan_rpm(
    conn: &HpWmiConnection,
) -> Result<victus_s::DualFanTelemetry, HardwareControlError> {
    conn.ensure_available("read Victus S fan levels")?;
    let payload = get_victus_fan_levels(conn)?;
    victus_s::parse_dual_fan_levels(&payload)
}

/// Read and parse the Victus S fan table (command `0x2F`).
pub fn get_victus_fan_table(conn: &HpWmiConnection) -> Result<FanTable, HardwareControlError> {
    conn.ensure_available("read Victus S fan table")?;

    let payload = conn
        .adapter
        .bios_call(HPWMI_GM, victus_s::FAN_TABLE_QUERY, &[0u8; 4], 128)?;
    victus_s::parse_fan_table(&payload)
}

/// Read the current Max Fan state (command `0x26`).
pub fn get_max_fan_state(conn: &HpWmiConnection) -> Result<bool, HardwareControlError> {
    conn.ensure_available("read Max Fan state")?;

    let payload = conn
        .adapter
        .bios_call(HPWMI_GM, commands::MAX_FAN_STATUS, &[0u8], 4)?;

    let state = payload.first().copied().ok_or_else(|| {
        HardwareControlError::ProtocolError(
            "Max Fan state query returned no payload bytes".to_string(),
        )
    })?;

    match state {
        0 => Ok(false),
        1 => Ok(true),
        other => Err(HardwareControlError::ProtocolError(format!(
            "Firmware returned unexpected Max Fan state 0x{other:02x}"
        ))),
    }
}

/// Write the Max Fan state (command `0x27`).
///
/// The firmware takes a 4-byte little-endian integer, matching the upstream
/// driver's `int enabled` and OMEN Gaming Hub's `[0|1, 0, 0, 0]` payload.
pub fn set_max_fan_state(
    conn: &HpWmiConnection,
    enabled: bool,
) -> Result<(), HardwareControlError> {
    conn.ensure_available("set Max Fan state")?;

    // On Victus S boards, calling 0x10 arms the user-defined state before applying max fan,
    // matching upstream hp_wmi_apply_fan_settings(PWM_MODE_MAX).
    if enabled && conn.is_victus_s() {
        let _ = get_fan_count(conn);
    }

    let payload = if enabled {
        [1u8, 0, 0, 0]
    } else {
        [0u8, 0, 0, 0]
    };
    conn.adapter
        .bios_call(HPWMI_GM, commands::MAX_FAN_SET, &payload, 0)?;
    Ok(())
}

/// Return a board to automatic cooling without stranding its fans.
///
/// Most boards just clear Max Fan (`0x27 = 0`). Victus S boards additionally
/// need the fan-level sentinel (`0x2E {0x00, 0x00}`) so the EC regains the curve.
///
/// Board `8A3D` is the exception. The 2026-10-24 upstream patch reports that the
/// `0x2E {0,0}` reset freezes its fans at 0 RPM under load; upstream restores it
/// with `0x27 = 0` followed by re-applying the active EC thermal profile through
/// `0x1A`. That profile is only readable from EC register `0x95`, which needs a
/// kernel driver, so when `active_profile` is unknown nothing is written at all
/// rather than risking a fan stall.
///
/// The GPU CTGP/PPAB bytes that upstream's `platform_profile_victus_s_set_ec`
/// also writes (`0x22`) are deliberately left untouched: ZenOmen never changes
/// them, so re-applying the thermal curve is enough to hand cooling back.
pub fn restore_automatic_fan_control(
    conn: &HpWmiConnection,
    active_profile: Option<victus_s::ThermalProfile>,
) -> Result<(), HardwareControlError> {
    restore_automatic_fan_control_with_target(
        conn,
        active_profile.map(victus_s::ProfileRestoreTarget::Known),
    )
}

/// Restore automatic fan control with explicit target handling (known profile vs safe fallback).
pub fn restore_automatic_fan_control_with_target(
    conn: &HpWmiConnection,
    target: Option<victus_s::ProfileRestoreTarget>,
) -> Result<(), HardwareControlError> {
    conn.ensure_available("restore automatic fan control")?;

    let board = conn.board_id().unwrap_or("unknown");
    if victus_s::requires_thermal_profile_restore(board) {
        let restore_target = target.ok_or_else(|| {
            HardwareControlError::UnsupportedMode(format!(
                "Board {board} restores automatic cooling through its EC thermal profile \
                 (offset 0x{:02X}); the active profile is not readable from userland, so the \
                 unsafe 0x2E automatic-level reset is refused",
                victus_s::EC_THERMAL_PROFILE_OFFSET
            ))
        })?;

        let profile = restore_target.profile();
        if restore_target.is_known() {
            tracing::info!(
                profile = profile.as_str(),
                "Restoring known EC thermal profile for automatic fan control"
            );
        } else {
            tracing::warn!(
                fallback_profile = profile.as_str(),
                "Original profile was unknown; applying explicit safe fallback profile"
            );
        }

        // No firmware write happens before this point: an unknown profile must
        // leave the machine exactly as it was found.
        set_max_fan_state(conn, false)?;
        get_fan_count(conn)?;
        let payload = victus_s::thermal_profile_payload(profile);
        conn.adapter
            .bios_call(HPWMI_GM, commands::PERFORMANCE_MODE, &payload, 0)?;
        tracing::info!(
            profile = profile.as_str(),
            "Re-applied the EC thermal profile to restore automatic fan control"
        );
        return Ok(());
    }

    if conn.is_victus_s() {
        return set_victus_fan_levels(
            conn,
            victus_s::FAN_SPEED_AUTOMATIC,
            victus_s::FAN_SPEED_AUTOMATIC,
        );
    }

    set_max_fan_state(conn, false)
}

/// Apply manual Victus S fan levels using the upstream command ordering.
///
/// 1. `0x10` — refresh/arm the firmware's user-defined fan state.
/// 2. `0x27` = 0 — Max Fan must be explicitly disabled.
/// 3. `0x2F` — read the fan table and validate the requested levels.
/// 4. `0x2E {cpu, gpu}` — write both levels in one request.
///
/// `FAN_SPEED_AUTOMATIC` (`0x00`) hands a fan back to the firmware curve.
pub fn set_victus_fan_levels(
    conn: &HpWmiConnection,
    cpu_level: u8,
    gpu_level: u8,
) -> Result<(), HardwareControlError> {
    conn.ensure_available("set Victus S fan levels")?;

    if !conn.is_victus_s() {
        return Err(HardwareControlError::UnsupportedMode(format!(
            "Board {} does not use the Victus S fan-level protocol",
            conn.board_id().unwrap_or("unknown")
        )));
    }

    let board = conn.board_id().unwrap_or("unknown");
    if cpu_level == victus_s::FAN_SPEED_AUTOMATIC
        && gpu_level == victus_s::FAN_SPEED_AUTOMATIC
        && victus_s::requires_thermal_profile_restore(board)
    {
        return Err(HardwareControlError::UnsupportedMode(format!(
            "Board {board} freezes fans at 0 RPM on 0x2E automatic reset; \
             use restore_automatic_fan_control with the active thermal profile instead"
        )));
    }

    // 1. Arm / refresh the manual fan state (0x10).
    get_fan_count(conn)?;

    // 2. Max Fan must be off before manual levels are accepted (0x27 = 0).
    set_max_fan_state(conn, false)?;

    // 3. Validate against the firmware's own level range before writing.
    // If the fan table query fails, fall back to safe 1..=100 range validation.
    let max_level = match get_victus_fan_table(conn) {
        Ok(table) => {
            victus_s::validate_level("cpu", cpu_level, &table)?;
            victus_s::validate_level("gpu", gpu_level, &table)?;
            table.max_rpm
        }
        Err(e) => {
            tracing::warn!(error = %e, "Victus S fan table query (0x2F) failed; falling back to 1..=100 range validation");
            if cpu_level != victus_s::FAN_SPEED_AUTOMATIC && !(1..=100).contains(&cpu_level) {
                return Err(HardwareControlError::InvalidParameter(format!(
                    "cpu fan level {cpu_level} outside fallback range 1..=100 (0x00 = automatic)"
                )));
            }
            if gpu_level != victus_s::FAN_SPEED_AUTOMATIC && !(1..=100).contains(&gpu_level) {
                return Err(HardwareControlError::InvalidParameter(format!(
                    "gpu fan level {gpu_level} outside fallback range 1..=100 (0x00 = automatic)"
                )));
            }
            100
        }
    };

    // 4. One 0x2E request carries both fan levels.
    conn.adapter.bios_call(
        HPWMI_GM,
        victus_s::FAN_LEVEL_SET_QUERY,
        &[cpu_level, gpu_level],
        0,
    )?;

    tracing::info!(
        cpu_level = cpu_level,
        gpu_level = gpu_level,
        max_level = max_level,
        "Victus S fan levels submitted"
    );
    Ok(())
}

/// Query active ACPI platform performance profile from firmware.
pub fn query_power_profile(conn: &HpWmiConnection) -> Result<String, HardwareControlError> {
    conn.ensure_available("read power profile")?;
    read_performance_mode_from_firmware(conn)
}

/// Set ACPI performance profile with firmware read-back verification.
pub fn set_performance_mode(
    conn: &HpWmiConnection,
    profile: &str,
) -> Result<String, HardwareControlError> {
    conn.ensure_available("set performance mode")?;

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

    // Upstream `omen_thermal_profile_set()` sends two bytes: a leading 0xFF
    // (matching OMEN Gaming Hub) followed by the profile value.
    conn.adapter.bios_call(
        HPWMI_GM,
        commands::PERFORMANCE_MODE,
        &[0xFF, target_byte],
        0,
    )?;

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

/// Report that 4-zone keyboard lighting is not implemented.
///
/// The previous implementation sent `0x20009` — a command *family*, not a
/// command type — in the `CommandType` field, which the firmware rejects. No
/// verified 4-zone payload contract exists for these boards yet.
pub fn set_rgb_profile(
    conn: &HpWmiConnection,
    profile: &RgbProfile,
) -> Result<String, HardwareControlError> {
    conn.ensure_available("set RGB profile")?;
    Err(HardwareControlError::UnsupportedMode(format!(
        "4-zone keyboard lighting has no verified firmware contract; \
         refusing to send an unvalidated payload for profile '{}'",
        profile.name
    )))
}

// ────────────────────── Internal firmware helpers ─────────────────────────

/// Read the current fan mode from firmware via the Max Fan state query (`0x26`).
fn read_fan_mode_from_firmware(conn: &HpWmiConnection) -> Result<String, HardwareControlError> {
    let mode = if get_max_fan_state(conn)? {
        "max"
    } else {
        "auto"
    };
    Ok(mode.to_string())
}

/// Read the current performance mode from firmware.
///
/// Victus S boards expose the thermal profile through the EC — register `0x95`
/// for board `8A3D` after the 2026-10-24 upstream patch
/// (`victus_s_ec_thermal_params`), `0x59` where `omen_v1_thermal_params` applies —
/// which requires a kernel driver. There is no WMI read path, so this reports
/// `UnsupportedMode` instead of guessing at the value.
fn read_performance_mode_from_firmware(
    conn: &HpWmiConnection,
) -> Result<String, HardwareControlError> {
    if conn.is_victus_s() {
        return Err(HardwareControlError::UnsupportedMode(
            "Victus S thermal profile is read from the EC; no WMI read path exists".to_string(),
        ));
    }

    let payload = conn
        .adapter
        .bios_call(HPWMI_GM, commands::PERFORMANCE_MODE, &[0u8], 4)?;

    let byte = payload.first().copied().ok_or_else(|| {
        HardwareControlError::ProtocolError(
            "Performance mode read returned zero-length payload".to_string(),
        )
    })?;

    let profile = match byte {
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

/// Probe performance mode command `0x1A` to check availability.
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
    use std::sync::Arc;

    fn write_ack() -> FakeResponse {
        FakeResponse::Success { payload: vec![] }
    }

    fn max_fan_state(on: bool) -> FakeResponse {
        FakeResponse::Success {
            payload: vec![u8::from(on)],
        }
    }

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
    fn test_command_family_and_opcodes() {
        // One command family for reads and writes; 0x20009 is keyboard lighting.
        assert_eq!(HPWMI_GM, 0x0002_0008);
        assert_eq!(HPWMI_KB_LIGHTING, 0x0002_0009);
        assert_eq!(commands::FAN_COUNT, 0x10);
        assert_eq!(commands::FAN_SPEEDS, 0x11);
        assert_eq!(commands::PERFORMANCE_MODE, 0x1A);
        assert_eq!(commands::MAX_FAN_STATUS, 0x26);
        assert_eq!(commands::MAX_FAN_SET, 0x27);
        assert_eq!(commands::SYSTEM_DESIGN_DATA, 0x28);
        assert_eq!(victus_s::FAN_LEVEL_GET_QUERY, 0x2D);
        assert_eq!(victus_s::FAN_LEVEL_SET_QUERY, 0x2E);
        assert_eq!(victus_s::FAN_TABLE_QUERY, 0x2F);
    }

    #[test]
    fn test_method_selection_matches_upstream_output_sizes() {
        assert_eq!(method_for_output_size(0), HP_WMI_METHOD_0);
        assert_eq!(method_for_output_size(4), HP_WMI_METHOD_4);
        assert_eq!(method_for_output_size(128), HP_WMI_METHOD_128);
        assert_eq!(method_for_output_size(1024), HP_WMI_METHOD_1024);
        assert_eq!(method_for_output_size(4096), HP_WMI_METHOD_4096);
    }

    #[test]
    fn test_firmware_return_codes_map_to_documented_layers() {
        assert!(matches!(
            firmware_error(0x27, 2),
            HardwareControlError::ProtocolError(_)
        ));
        assert!(matches!(
            firmware_error(0x27, 3),
            HardwareControlError::UnsupportedMode(_)
        ));
        assert!(matches!(
            firmware_error(0x27, 4),
            HardwareControlError::UnsupportedMode(_)
        ));
        assert!(matches!(
            firmware_error(0x27, 5),
            HardwareControlError::InvalidParameter(_)
        ));
        assert!(matches!(
            firmware_error(0x27, 9),
            HardwareControlError::ProtocolError(_)
        ));
    }

    #[test]
    fn test_fake_adapter_records_fan_count_call() {
        let adapter = FakeWmiAdapter::new(true);
        adapter.expect_call(FakeResponse::Success {
            payload: vec![0x02],
        });
        let conn = HpWmiConnection::from_adapter(adapter);
        assert_eq!(get_fan_count(&conn).expect("fan count"), 2);
    }

    #[test]
    fn test_fan_count_rejects_implausible_values() {
        for value in [0u8, MAX_FAN_COUNT + 1] {
            let adapter = FakeWmiAdapter::new(true);
            adapter.expect_call(FakeResponse::Success {
                payload: vec![value],
            });
            let conn = HpWmiConnection::from_adapter(adapter);
            assert!(matches!(
                get_fan_count(&conn).unwrap_err(),
                HardwareControlError::ProtocolError(_)
            ));
        }
    }

    #[test]
    fn test_cached_fan_count_does_not_rearm_firmware_state() {
        let adapter = Arc::new(FakeWmiAdapter::new(true));
        adapter.expect_call(FakeResponse::Success {
            payload: vec![0x02],
        });
        let conn = HpWmiConnection::from_adapter(adapter.clone());

        assert_eq!(cached_fan_count(&conn).expect("first"), 2);
        assert_eq!(cached_fan_count(&conn).expect("cached"), 2);
        // Only one 0x10 query reached firmware.
        assert_eq!(adapter.recorded_calls().len(), 1);
    }

    #[test]
    fn test_read_fan_mode_uses_max_fan_status_opcode() {
        let adapter = Arc::new(FakeWmiAdapter::new(true));
        adapter.expect_call(max_fan_state(false));
        let conn = HpWmiConnection::from_adapter(adapter.clone());
        assert_eq!(read_fan_mode_from_firmware(&conn).unwrap(), "auto");

        let call = &adapter.recorded_calls()[0];
        assert_eq!(call.command, HPWMI_GM);
        assert_eq!(call.command_type, commands::MAX_FAN_STATUS);
    }

    #[test]
    fn test_read_fan_mode_empty_payload_err() {
        let adapter = FakeWmiAdapter::new(true);
        adapter.expect_call(FakeResponse::Success {
            payload: Vec::new(),
        });
        let conn = HpWmiConnection::from_adapter(adapter);
        assert!(matches!(
            read_fan_mode_from_firmware(&conn).unwrap_err(),
            HardwareControlError::ProtocolError(_)
        ));
    }

    #[test]
    fn test_read_fan_mode_unknown_byte_err() {
        let adapter = FakeWmiAdapter::new(true);
        adapter.expect_call(FakeResponse::Success {
            payload: vec![0x99],
        });
        let conn = HpWmiConnection::from_adapter(adapter);
        assert!(matches!(
            read_fan_mode_from_firmware(&conn).unwrap_err(),
            HardwareControlError::ProtocolError(_)
        ));
    }

    #[test]
    fn test_set_fan_mode_invalid_string_rejected() {
        let adapter = FakeWmiAdapter::new(true);
        let conn = HpWmiConnection::from_adapter(adapter);
        let err = set_fan_mode(&conn, "turbo_extreme").unwrap_err();
        assert!(matches!(err, HardwareControlError::InvalidParameter(_)));
    }

    #[test]
    fn test_generic_fan_rpm_decodes_big_endian_word() {
        let adapter = FakeWmiAdapter::new(true);
        // Real 8A4F capture: 36 3C 0F A7 -> 4007 RPM.
        adapter.expect_call(FakeResponse::Success {
            payload: vec![0x36, 0x3C, 0x0F, 0xA7],
        });
        let conn = HpWmiConnection::from_adapter(adapter);
        assert_eq!(get_fan_rpm(&conn, 0).expect("rpm"), Some(4007));
    }

    #[test]
    fn test_victus_s_fan_rpm_uses_0x2d_hundred_rpm_units() {
        let adapter = Arc::new(FakeWmiAdapter::new(true));
        let mut levels = vec![0u8; 128];
        levels[0] = 0x27; // 3900 RPM
        levels[1] = 0x23; // 3500 RPM
        adapter.expect_call(FakeResponse::Success { payload: levels });

        let conn = HpWmiConnection::from_adapter_for_board(adapter.clone(), "8A3D");
        assert_eq!(get_fan_rpm(&conn, 0).expect("fan 0"), Some(3900));
        let call = &adapter.recorded_calls()[0];
        assert_eq!(call.command_type, victus_s::FAN_LEVEL_GET_QUERY);
        assert_eq!(call.out_size, 128);
    }

    #[test]
    fn test_set_max_fan_state_sends_four_byte_integer_without_output() {
        let adapter = Arc::new(FakeWmiAdapter::new(true));
        adapter.expect_call(write_ack());
        let conn = HpWmiConnection::from_adapter(adapter.clone());

        set_max_fan_state(&conn, true).expect("set max fan");
        let call = &adapter.recorded_calls()[0];
        assert_eq!(call.command, HPWMI_GM);
        assert_eq!(call.command_type, commands::MAX_FAN_SET);
        assert_eq!(call.payload, vec![1, 0, 0, 0]);
        assert_eq!(call.out_size, 0);
    }

    #[test]
    fn test_victus_s_level_write_follows_upstream_sequence() {
        let adapter = Arc::new(FakeWmiAdapter::new(true));
        adapter.expect_call(FakeResponse::Success {
            payload: vec![0x02],
        }); // 0x10
        adapter.expect_call(write_ack()); // 0x27 = 0
        let mut table = vec![0u8; 128];
        table[0] = 2;
        table[2] = 0x35; // max cpu level 53
        table[3] = 0x2E;
        adapter.expect_call(FakeResponse::Success { payload: table }); // 0x2F
        adapter.expect_call(write_ack()); // 0x2E

        let conn = HpWmiConnection::from_adapter_for_board(adapter.clone(), "8A3D");
        set_victus_fan_levels(&conn, 40, 35).expect("set levels");

        let calls = adapter.recorded_calls();
        let opcodes: Vec<u32> = calls.iter().map(|c| c.command_type).collect();
        assert_eq!(
            opcodes,
            vec![
                commands::FAN_COUNT,
                commands::MAX_FAN_SET,
                victus_s::FAN_TABLE_QUERY,
                victus_s::FAN_LEVEL_SET_QUERY,
            ]
        );
        assert!(calls.iter().all(|c| c.command == HPWMI_GM));
        assert_eq!(calls[1].payload, vec![0, 0, 0, 0]);
        assert_eq!(calls[3].payload, vec![40, 35]);
        assert_eq!(calls[3].out_size, 0);
    }

    #[test]
    fn test_victus_s_level_write_rejects_out_of_range_without_writing() {
        let adapter = Arc::new(FakeWmiAdapter::new(true));
        adapter.expect_call(FakeResponse::Success {
            payload: vec![0x02],
        });
        adapter.expect_call(write_ack());
        let mut table = vec![0u8; 128];
        table[0] = 2;
        table[2] = 0x35;
        adapter.expect_call(FakeResponse::Success { payload: table });

        let conn = HpWmiConnection::from_adapter_for_board(adapter.clone(), "8A3D");
        let err = set_victus_fan_levels(&conn, 200, 35).unwrap_err();
        assert!(matches!(err, HardwareControlError::InvalidParameter(_)));
        // No 0x2E request was issued.
        assert!(
            adapter
                .recorded_calls()
                .iter()
                .all(|c| c.command_type != victus_s::FAN_LEVEL_SET_QUERY)
        );
    }

    #[test]
    fn test_victus_s_level_write_refused_on_non_victus_board() {
        let adapter = FakeWmiAdapter::new(true);
        let conn = HpWmiConnection::from_adapter(adapter);
        let err = set_victus_fan_levels(&conn, 10, 10).unwrap_err();
        assert!(matches!(err, HardwareControlError::UnsupportedMode(_)));
    }

    #[test]
    fn test_victus_s_level_write_refuses_0x2e_auto_sentinel_on_8a3d() {
        let adapter = FakeWmiAdapter::new(true);
        let conn = HpWmiConnection::from_adapter_for_board(adapter, "8A3D");
        let err = set_victus_fan_levels(&conn, 0, 0).unwrap_err();
        assert!(matches!(err, HardwareControlError::UnsupportedMode(_)));
    }

    #[test]
    fn test_victus_s_thermal_profile_read_is_explicitly_unsupported() {
        let adapter = FakeWmiAdapter::new(true);
        let conn = HpWmiConnection::from_adapter_for_board(adapter, "8A3D");
        let err = read_performance_mode_from_firmware(&conn).unwrap_err();
        assert!(matches!(err, HardwareControlError::UnsupportedMode(_)));
    }
}
