//! Minimal, clean Win32 COM WMI FFI bindings and RAII wrappers for `ROOT\WMI`.
//!
//! Windows-only. All types conform strictly to Microsoft's `WbemCli.h` specifications.

#![cfg(windows)]
#![allow(non_snake_case, non_camel_case_types, non_upper_case_globals, unused)]

use std::ffi::c_void;
use std::ptr::null_mut;

pub type HRESULT = i32;

pub const S_OK: HRESULT = 0;
pub const S_FALSE: HRESULT = 1;
pub const RPC_E_CHANGED_MODE: HRESULT = -2147417850i32; // 0x80010106
pub const WBEM_FLAG_FORWARD_ONLY: i32 = 0x20;
pub const WBEM_FLAG_RETURN_IMMEDIATELY: i32 = 0x10;
pub const WBEM_INFINITE: u32 = 0xFFFF_FFFF;

// VARENUM constants
pub const VT_EMPTY: u16 = 0;
pub const VT_I4: u16 = 3;
pub const VT_BSTR: u16 = 8;
pub const VT_DISPATCH: u16 = 9;
pub const VT_UNKNOWN: u16 = 13;
pub const VT_UI1: u16 = 17;
pub const VT_UI4: u16 = 19;
pub const VT_ARRAY: u16 = 0x2000;

// CIM types (from WbemCli.h)
pub const CIM_ILLEGAL: i32 = 0xFFF;
pub const CIM_UINT8: i32 = 17;
pub const CIM_UINT32: i32 = 19;
pub const CIM_OBJECT: i32 = 13;
pub const CIM_FLAG_ARRAY: i32 = 0x2000;

// WBEM HRESULT error codes
pub const WBEM_E_FAILED: HRESULT = -2147217407i32; // 0x80041001
pub const WBEM_E_NOT_FOUND: HRESULT = -2147217406i32; // 0x80041002
pub const WBEM_E_ACCESS_DENIED: HRESULT = -2147217405i32; // 0x80041003
pub const WBEM_E_PROVIDER_FAILURE: HRESULT = -2147217404i32; // 0x80041004
pub const WBEM_E_TYPE_MISMATCH: HRESULT = -2147217403i32; // 0x80041005
pub const WBEM_E_INVALID_PARAMETER: HRESULT = -2147217400i32; // 0x80041008
pub const WBEM_E_INVALID_PROPERTY_TYPE: HRESULT = -2147217383i32; // 0x80041019
pub const WBEM_E_INVALID_CLASS: HRESULT = -2147217392i32; // 0x80041010
pub const WBEM_E_TRANSPORT_FAILURE: HRESULT = -2147217387i32; // 0x80041015
pub const WBEM_S_NO_ERROR: HRESULT = 0;
pub const WBEM_S_FALSE: HRESULT = 1;

#[repr(C)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct GUID {
    pub data1: u32,
    pub data2: u16,
    pub data3: u16,
    pub data4: [u8; 8],
}

// CLSID_WbemLocator: 4590f811-1d3a-11d0-891f-00aa004b2e24
pub const CLSID_WBEM_LOCATOR: GUID = GUID {
    data1: 0x4590f811,
    data2: 0x1d3a,
    data3: 0x11d0,
    data4: [0x89, 0x1f, 0x00, 0xaa, 0x00, 0x4b, 0x2e, 0x24],
};

// IID_IWbemLocator: dc12a687-737f-11cf-884d-00aa004b2e24
pub const IID_IWBEM_LOCATOR: GUID = GUID {
    data1: 0xdc12a687,
    data2: 0x737f,
    data3: 0x11cf,
    data4: [0x88, 0x4d, 0x00, 0xaa, 0x00, 0x4b, 0x2e, 0x24],
};

#[repr(C)]
pub struct SAFEARRAYBOUND {
    pub cElements: u32,
    pub lLbound: i32,
}

#[repr(C)]
pub struct SAFEARRAY {
    pub cDims: u16,
    pub fFeatures: u16,
    pub cbElements: u32,
    pub cLocks: u32,
    pub pvData: *mut c_void,
    pub rgsabound: [SAFEARRAYBOUND; 1],
}

/// Standard 24-byte VARIANT layout for 64-bit Windows.
#[repr(C)]
pub struct VARIANT {
    pub vt: u16,
    pub wReserved1: u16,
    pub wReserved2: u16,
    pub wReserved3: u16,
    pub data: usize,
    pub data2: usize,
}

impl VARIANT {
    pub fn empty() -> Self {
        Self {
            vt: VT_EMPTY,
            wReserved1: 0,
            wReserved2: 0,
            wReserved3: 0,
            data: 0,
            data2: 0,
        }
    }

    /// Create a VT_I4 VARIANT for CIM_UINT32 properties.
    ///
    /// Microsoft WMI documents `VT_I4` as the required VARIANT representation for `CIM_UINT32`.
    /// Passing `VT_UI4` results in `WBEM_E_TYPE_MISMATCH` (`0x80041005`).
    /// The original `u32` bit pattern is preserved when cast into signed 32-bit `lVal` representation.
    pub fn from_cim_uint32(val: u32) -> Self {
        Self {
            vt: VT_I4,
            wReserved1: 0,
            wReserved2: 0,
            wReserved3: 0,
            data: (val as i32) as isize as usize,
            data2: 0,
        }
    }

    pub fn from_u32(val: u32) -> Self {
        Self::from_cim_uint32(val)
    }

    pub fn from_safearray(sa: *mut SAFEARRAY) -> Self {
        Self {
            vt: VT_ARRAY | VT_UI1,
            wReserved1: 0,
            wReserved2: 0,
            wReserved3: 0,
            data: sa as usize,
            data2: 0,
        }
    }

    pub fn from_unknown(punk: *mut c_void) -> Self {
        Self {
            vt: VT_UNKNOWN,
            wReserved1: 0,
            wReserved2: 0,
            wReserved3: 0,
            data: punk as usize,
            data2: 0,
        }
    }
}

#[link(name = "ole32")]
unsafe extern "system" {
    pub fn CoInitializeEx(pvReserved: *const c_void, dwCoInit: u32) -> HRESULT;
    pub fn CoUninitialize();
    pub fn CoInitializeSecurity(
        pSecDesc: *mut c_void,
        cAuthSvc: i32,
        asAuthSvc: *mut c_void,
        pReserved1: *mut c_void,
        dwAuthnLevel: u32,
        dwImpLevel: u32,
        pAuthList: *mut c_void,
        dwCapabilities: u32,
        pReserved3: *mut c_void,
    ) -> HRESULT;
    pub fn CoCreateInstance(
        rclsid: *const GUID,
        pUnkOuter: *mut c_void,
        dwClsContext: u32,
        riid: *const GUID,
        ppv: *mut *mut c_void,
    ) -> HRESULT;
}

#[link(name = "oleaut32")]
unsafe extern "system" {
    pub fn SysAllocString(psz: *const u16) -> *mut u16;
    pub fn SysFreeString(bstrString: *mut u16);
    pub fn SafeArrayCreate(vt: u16, cDims: u32, rgsabound: *const SAFEARRAYBOUND)
    -> *mut SAFEARRAY;
    pub fn SafeArrayDestroy(psa: *mut SAFEARRAY) -> HRESULT;
    pub fn SafeArrayAccessData(psa: *mut SAFEARRAY, ppvData: *mut *mut c_void) -> HRESULT;
    pub fn SafeArrayUnaccessData(psa: *mut SAFEARRAY) -> HRESULT;
    pub fn SafeArrayGetUBound(psa: *mut SAFEARRAY, nDim: u32, plUbound: *mut i32) -> HRESULT;
    pub fn VariantInit(pvarg: *mut VARIANT);
    pub fn VariantClear(pvarg: *mut VARIANT) -> HRESULT;
    pub fn GetErrorInfo(dwReserved: u32, pperrinfo: *mut *mut c_void) -> HRESULT;
}

/// Minimal `IErrorInfo` vtable — only `GetDescription` is needed for diagnostics.
#[repr(C)]
pub struct IErrorInfoVtbl {
    pub QueryInterface: unsafe extern "system" fn(
        this: *mut c_void,
        riid: *const GUID,
        ppvObject: *mut *mut c_void,
    ) -> HRESULT,
    pub AddRef: unsafe extern "system" fn(this: *mut c_void) -> u32,
    pub Release: unsafe extern "system" fn(this: *mut c_void) -> u32,
    pub GetGUID: usize,
    pub GetSource: usize,
    pub GetDescription:
        unsafe extern "system" fn(this: *mut c_void, pbstrDescription: *mut *mut u16) -> HRESULT,
    pub GetHelpFile: usize,
    pub GetHelpContext: usize,
}

/// Take (and clear) the calling thread's last `IErrorInfo` description.
///
/// Call this immediately after a failing COM call: the error object is owned by
/// the thread and is consumed by `GetErrorInfo`.
pub fn last_error_description() -> Option<String> {
    unsafe {
        let mut info: *mut c_void = null_mut();
        if GetErrorInfo(0, &mut info) != S_OK || info.is_null() {
            return None;
        }
        let vtbl = info as *mut *const IErrorInfoVtbl;
        let mut description: *mut u16 = null_mut();
        let hr = ((*(*vtbl)).GetDescription)(info, &mut description);
        let mut text = None;
        if hr == S_OK && !description.is_null() {
            let mut len = 0;
            while *description.add(len) != 0 {
                len += 1;
            }
            text = String::from_utf16(std::slice::from_raw_parts(description, len)).ok();
            SysFreeString(description);
        }
        ((*(*vtbl)).Release)(info);
        text
    }
}

// ────────────────────── COM VTABLE DEFINITIONS ──────────────────────────────

#[repr(C)]
pub struct IUnknownVtbl {
    pub QueryInterface: unsafe extern "system" fn(
        this: *mut c_void,
        riid: *const GUID,
        ppvObject: *mut *mut c_void,
    ) -> HRESULT,
    pub AddRef: unsafe extern "system" fn(this: *mut c_void) -> u32,
    pub Release: unsafe extern "system" fn(this: *mut c_void) -> u32,
}

#[repr(C)]
pub struct IWbemLocatorVtbl {
    pub QueryInterface: unsafe extern "system" fn(
        this: *mut c_void,
        riid: *const GUID,
        ppvObject: *mut *mut c_void,
    ) -> HRESULT,
    pub AddRef: unsafe extern "system" fn(this: *mut c_void) -> u32,
    pub Release: unsafe extern "system" fn(this: *mut c_void) -> u32,
    pub ConnectServer: unsafe extern "system" fn(
        this: *mut c_void,
        strNetworkResource: *const u16,
        strUser: *const u16,
        strPassword: *const u16,
        strLocale: *const u16,
        lSecurityFlags: i32,
        strAuthority: *const u16,
        pCtx: *mut c_void,
        ppNamespace: *mut *mut c_void,
    ) -> HRESULT,
}

#[repr(C)]
pub struct IWbemServicesVtbl {
    pub QueryInterface: unsafe extern "system" fn(
        this: *mut c_void,
        riid: *const GUID,
        ppvObject: *mut *mut c_void,
    ) -> HRESULT,
    pub AddRef: unsafe extern "system" fn(this: *mut c_void) -> u32,
    pub Release: unsafe extern "system" fn(this: *mut c_void) -> u32,
    pub OpenNamespace: usize,
    pub CancelAsyncCall: usize,
    pub QueryObjectSink: usize,
    pub GetObject: unsafe extern "system" fn(
        this: *mut c_void,
        strObjectPath: *const u16,
        lFlags: i32,
        pCtx: *mut c_void,
        ppObject: *mut *mut c_void,
        ppCallResult: *mut *mut c_void,
    ) -> HRESULT,
    pub GetObjectAsync: usize,
    pub PutClass: usize,
    pub PutClassAsync: usize,
    pub DeleteClass: usize,
    pub DeleteClassAsync: usize,
    pub CreateClassEnum: usize,
    pub CreateClassEnumAsync: usize,
    pub PutInstance: usize,
    pub PutInstanceAsync: usize,
    pub DeleteInstance: usize,
    pub DeleteInstanceAsync: usize,
    pub CreateInstanceEnum: unsafe extern "system" fn(
        this: *mut c_void,
        strFilter: *const u16,
        lFlags: i32,
        pCtx: *mut c_void,
        ppEnum: *mut *mut c_void,
    ) -> HRESULT,
    pub CreateInstanceEnumAsync: usize,
    pub ExecQuery: unsafe extern "system" fn(
        this: *mut c_void,
        strQueryLanguage: *const u16,
        strQuery: *const u16,
        lFlags: i32,
        pCtx: *mut c_void,
        ppEnum: *mut *mut c_void,
    ) -> HRESULT,
    pub ExecQueryAsync: usize,
    pub ExecNotificationQuery: usize,
    pub ExecNotificationQueryAsync: usize,
    pub ExecMethod: unsafe extern "system" fn(
        this: *mut c_void,
        strObjectPath: *const u16,
        strMethodName: *const u16,
        lFlags: i32,
        pCtx: *mut c_void,
        pInParams: *mut c_void,
        ppOutParams: *mut *mut c_void,
        ppCallResult: *mut *mut c_void,
    ) -> HRESULT,
    pub ExecMethodAsync: usize,
}

#[repr(C)]
pub struct IWbemClassObjectVtbl {
    pub QueryInterface: unsafe extern "system" fn(
        this: *mut c_void,
        riid: *const GUID,
        ppvObject: *mut *mut c_void,
    ) -> HRESULT,
    pub AddRef: unsafe extern "system" fn(this: *mut c_void) -> u32,
    pub Release: unsafe extern "system" fn(this: *mut c_void) -> u32,
    pub GetQualifierSet: usize,
    pub Get: unsafe extern "system" fn(
        this: *mut c_void,
        wszName: *const u16,
        lFlags: i32,
        pVal: *mut VARIANT,
        pType: *mut i32,
        plFlavor: *mut i32,
    ) -> HRESULT,
    pub Put: unsafe extern "system" fn(
        this: *mut c_void,
        wszName: *const u16,
        lFlags: i32,
        pVal: *const VARIANT,
        Type: i32,
    ) -> HRESULT,
    pub Delete: usize,
    pub GetNames: usize,
    pub BeginEnumeration: usize,
    pub Next: usize,
    pub EndEnumeration: usize,
    pub GetPropertyQualifierSet: usize,
    pub Clone: usize,
    pub GetObjectText: usize,
    pub SpawnDerivedClass: usize,
    pub SpawnInstance: unsafe extern "system" fn(
        this: *mut c_void,
        lFlags: i32,
        ppNewInstance: *mut *mut c_void,
    ) -> HRESULT,
    pub CompareTo: usize,
    pub GetPropertyOrigin: usize,
    pub InheritsFrom: usize,
    pub GetMethod: unsafe extern "system" fn(
        this: *mut c_void,
        wszName: *const u16,
        lFlags: i32,
        ppInSignature: *mut *mut c_void,
        ppOutSignature: *mut *mut c_void,
    ) -> HRESULT,
    pub PutMethod: usize,
    pub DeleteMethod: usize,
    pub BeginMethodEnumeration: usize,
    pub NextMethod: usize,
    pub EndMethodEnumeration: usize,
    pub GetMethodQualifierSet: usize,
    pub GetMethodOrigin: usize,
}

#[repr(C)]
pub struct IEnumWbemClassObjectVtbl {
    pub QueryInterface: unsafe extern "system" fn(
        this: *mut c_void,
        riid: *const GUID,
        ppvObject: *mut *mut c_void,
    ) -> HRESULT,
    pub AddRef: unsafe extern "system" fn(this: *mut c_void) -> u32,
    pub Release: unsafe extern "system" fn(this: *mut c_void) -> u32,
    pub Reset: usize,
    pub Next: unsafe extern "system" fn(
        this: *mut c_void,
        lTimeout: i32,
        uCount: u32,
        apObjects: *mut *mut c_void,
        puReturned: *mut u32,
    ) -> HRESULT,
    pub NextAsync: usize,
    pub Clone: usize,
    pub Skip: usize,
}

// ────────────────────── SAFE RAII COM TYPES ────────────────────────────────

/// RAII wrapper for any COM interface pointer. Calls `Release` on Drop.
pub struct ComPtr<T> {
    ptr: *mut T,
}

impl<T> ComPtr<T> {
    /// # Safety
    ///
    /// The caller must ensure that `ptr` is either null or a valid COM interface pointer
    /// whose first field points to an `IUnknownVtbl`.
    pub unsafe fn from_raw(ptr: *mut T) -> Self {
        Self { ptr }
    }

    pub fn as_ptr(&self) -> *mut T {
        self.ptr
    }

    pub fn is_null(&self) -> bool {
        self.ptr.is_null()
    }
}

impl<T> Drop for ComPtr<T> {
    fn drop(&mut self) {
        if !self.ptr.is_null() {
            unsafe {
                let unk = self.ptr as *mut *const IUnknownVtbl;
                ((*(*unk)).Release)(self.ptr as *mut c_void);
            }
            self.ptr = null_mut();
        }
    }
}

/// RAII wrapper for a BSTR. Calls `SysFreeString` on Drop.
pub struct Bstr {
    ptr: *mut u16,
}

impl Bstr {
    pub fn new(s: &str) -> Self {
        let wide: Vec<u16> = s.encode_utf16().chain(std::iter::once(0)).collect();
        let ptr = unsafe { SysAllocString(wide.as_ptr()) };
        Self { ptr }
    }

    pub fn as_ptr(&self) -> *const u16 {
        self.ptr
    }
}

impl Drop for Bstr {
    fn drop(&mut self) {
        if !self.ptr.is_null() {
            unsafe { SysFreeString(self.ptr) };
            self.ptr = null_mut();
        }
    }
}

/// RAII wrapper for a VARIANT. Calls `VariantClear` on Drop.
pub struct ScopedVariant {
    pub inner: VARIANT,
}

impl ScopedVariant {
    pub fn empty() -> Self {
        let mut v = VARIANT::empty();
        unsafe { VariantInit(&mut v) };
        Self { inner: v }
    }

    pub fn from_cim_uint32(val: u32) -> Self {
        Self {
            inner: VARIANT::from_cim_uint32(val),
        }
    }

    pub fn from_u32(val: u32) -> Self {
        Self::from_cim_uint32(val)
    }

    pub fn from_safearray(sa: *mut SAFEARRAY) -> Self {
        Self {
            inner: VARIANT::from_safearray(sa),
        }
    }

    /// # Safety
    ///
    /// The caller must ensure `punk` is null or a valid COM interface pointer
    /// whose first field points to an `IUnknownVtbl`.
    pub unsafe fn from_unknown(punk: *mut c_void) -> Self {
        if !punk.is_null() {
            unsafe {
                let unk = punk as *mut *const IUnknownVtbl;
                ((*(*unk)).AddRef)(punk);
            }
        }
        Self {
            inner: VARIANT::from_unknown(punk),
        }
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self, crate::service::types::HardwareControlError> {
        let sa = create_byte_safearray(bytes);
        if sa.is_null() {
            return Err(crate::service::types::HardwareControlError::ProtocolError(
                "SafeArrayCreate failed".to_string(),
            ));
        }
        Ok(Self::from_safearray(sa))
    }

    pub fn extract_u32(&self) -> Option<u32> {
        match self.inner.vt {
            VT_I4 => Some((self.inner.data as i32) as u32),
            VT_UI4 => Some(self.inner.data as u32),
            VT_UI1 => Some((self.inner.data & 0xFF) as u32),
            _ => None,
        }
    }

    pub fn extract_bytes(&self) -> Option<Vec<u8>> {
        if self.inner.vt != (VT_ARRAY | VT_UI1) || self.inner.data == 0 {
            return None;
        }
        let sa = self.inner.data as *mut SAFEARRAY;
        unsafe {
            let mut ubound: i32 = -1;
            let hr = SafeArrayGetUBound(sa, 1, &mut ubound);
            if hr != S_OK {
                return None;
            }
            if ubound < 0 {
                return Some(Vec::new());
            }
            let len = (ubound + 1) as usize;
            let mut data_ptr: *mut c_void = null_mut();
            let hr = SafeArrayAccessData(sa, &mut data_ptr);
            if hr != S_OK || data_ptr.is_null() {
                return None;
            }
            let mut vec = vec![0u8; len];
            std::ptr::copy_nonoverlapping(data_ptr as *const u8, vec.as_mut_ptr(), len);
            SafeArrayUnaccessData(sa);
            Some(vec)
        }
    }

    pub fn extract_bstr(&self) -> Option<String> {
        if self.inner.vt != VT_BSTR || self.inner.data == 0 {
            return None;
        }
        let bstr = self.inner.data as *const u16;
        unsafe {
            let mut len = 0;
            while *bstr.add(len) != 0 {
                len += 1;
            }
            let slice = std::slice::from_raw_parts(bstr, len);
            String::from_utf16(slice).ok()
        }
    }
}

impl Drop for ScopedVariant {
    fn drop(&mut self) {
        unsafe { VariantClear(&mut self.inner) };
    }
}

/// Create a 1D SAFEARRAY of VT_UI1 containing `bytes`.
///
/// Caller is responsible for transferring ownership to `ScopedVariant::from_safearray`
/// or destroying with `SafeArrayDestroy`.
pub fn create_byte_safearray(bytes: &[u8]) -> *mut SAFEARRAY {
    let bound = SAFEARRAYBOUND {
        cElements: bytes.len() as u32,
        lLbound: 0,
    };
    let sa = unsafe { SafeArrayCreate(VT_UI1, 1, &bound) };
    if sa.is_null() {
        return null_mut();
    }
    if !bytes.is_empty() {
        unsafe {
            let mut sa_data: *mut c_void = null_mut();
            if SafeArrayAccessData(sa, &mut sa_data) == S_OK && !sa_data.is_null() {
                std::ptr::copy_nonoverlapping(bytes.as_ptr(), sa_data as *mut u8, bytes.len());
                SafeArrayUnaccessData(sa);
            }
        }
    }
    sa
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_variant_abi_layout() {
        assert_eq!(std::mem::size_of::<VARIANT>(), 24);
        assert_eq!(std::mem::align_of::<VARIANT>(), 8);
        assert_eq!(std::mem::size_of::<SAFEARRAYBOUND>(), 8);
        assert_eq!(std::mem::size_of::<SAFEARRAY>(), 32);
    }

    #[test]
    fn test_scoped_variant_u32_lifecycle() {
        let var = ScopedVariant::from_u32(0x20008);
        assert_eq!(var.inner.vt, VT_I4);
        assert_eq!(var.extract_u32(), Some(0x20008));
    }

    #[test]
    fn test_uint32_helper_creates_vt_i4() {
        let var = VARIANT::from_cim_uint32(42);
        assert_eq!(var.vt, VT_I4);
    }

    #[test]
    fn test_uint32_zero_represented_correctly() {
        let var = VARIANT::from_cim_uint32(0);
        assert_eq!(var.vt, VT_I4);
        assert_eq!(var.data, 0);
        let scoped = ScopedVariant::from_cim_uint32(0);
        assert_eq!(scoped.extract_u32(), Some(0));
    }

    #[test]
    fn test_uint32_preserves_0x20008_bit_pattern() {
        let val = 0x0002_0008u32;
        let var = VARIANT::from_cim_uint32(val);
        assert_eq!(var.vt, VT_I4);
        assert_eq!((var.data as i32) as u32, val);
        let scoped = ScopedVariant::from_cim_uint32(val);
        assert_eq!(scoped.extract_u32(), Some(val));
    }

    #[test]
    fn test_uint32_preserves_0xffffffff_bit_pattern() {
        let val = 0xFFFF_FFFFu32;
        let var = VARIANT::from_cim_uint32(val);
        assert_eq!(var.vt, VT_I4);
        assert_eq!((var.data as i32) as u32, val);
        let scoped = ScopedVariant::from_cim_uint32(val);
        assert_eq!(scoped.extract_u32(), Some(val));
    }

    #[test]
    fn test_byte_arrays_use_vt_array_vt_ui1() {
        let payload = [0x53, 0x45, 0x43, 0x55]; // "SECU"
        let var = ScopedVariant::from_bytes(&payload).expect("create safearray variant");
        assert_eq!(var.inner.vt, VT_ARRAY | VT_UI1);
        let extracted = var.extract_bytes().expect("extract bytes");
        assert_eq!(extracted, payload);
    }
}
