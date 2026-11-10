//! Windows display refresh-rate and monitor management (WIN-DISPLAY).
//!
//! Enumerates display devices and modes using Win32 GDI APIs (`EnumDisplayDevicesW`,
//! `EnumDisplaySettingsW`, `ChangeDisplaySettingsExW`), validates available refresh
//! rates (e.g. 60 Hz and 144 Hz), safely tests mode changes before applying (`CDS_TEST`),
//! and performs read-back verification against the active display configuration.

use std::mem::zeroed;
use thiserror::Error;
use windows_sys::Win32::Graphics::Gdi::{
    CDS_TEST, CDS_UPDATEREGISTRY, ChangeDisplaySettingsExW, DEVMODEW, DISP_CHANGE_BADDUALVIEW,
    DISP_CHANGE_BADFLAGS, DISP_CHANGE_BADMODE, DISP_CHANGE_BADPARAM, DISP_CHANGE_FAILED,
    DISP_CHANGE_NOTUPDATED, DISP_CHANGE_RESTART, DISP_CHANGE_SUCCESSFUL,
    DISPLAY_DEVICE_ATTACHED_TO_DESKTOP, DISPLAY_DEVICE_PRIMARY_DEVICE, DISPLAY_DEVICEW,
    DM_DISPLAYFREQUENCY, ENUM_CURRENT_SETTINGS, EnumDisplayDevicesW, EnumDisplaySettingsW,
};

/// Errors encountered during Windows display queries and mode switches.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum DisplayError {
    #[error("No active primary display device attached to desktop was found")]
    PrimaryDisplayNotFound,

    #[error("Failed to query current display settings for device '{0}'")]
    QueryCurrentSettingsFailed(String),

    #[error(
        "Requested refresh rate {requested_hz} Hz is unsupported at resolution {width}x{height}. Supported: {supported:?}"
    )]
    UnsupportedRefreshRate {
        requested_hz: u32,
        width: u32,
        height: u32,
        supported: Vec<u32>,
    },

    #[error("ChangeDisplaySettingsEx test failed with error code {code}: {reason}")]
    ModeTestFailed { code: i32, reason: String },

    #[error("ChangeDisplaySettingsEx apply failed with error code {code}: {reason}")]
    ModeApplyFailed { code: i32, reason: String },

    #[error("Display mode change requires a system restart to take effect")]
    RestartRequired,

    #[error(
        "Read-back verification mismatch: requested {expected_hz} Hz, but active display reports {actual_hz} Hz"
    )]
    VerificationMismatch { expected_hz: u32, actual_hz: u32 },
}

/// Information about an attached display adapter and its current resolution/refresh rate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DisplayInfo {
    pub device_name: String,
    pub is_primary: bool,
    pub width: u32,
    pub height: u32,
    pub current_refresh_rate: u32,
    pub supported_refresh_rates: Vec<u32>,
}

/// Detailed result of attempting a display refresh rate modification.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DisplayChangeResult {
    pub device_name: String,
    pub target_refresh_rate: u32,
    pub actual_refresh_rate: u32,
    pub verified: bool,
    pub message: String,
}

/// Query the primary active display device attached to the Windows desktop.
pub fn get_primary_display() -> Result<DisplayInfo, DisplayError> {
    unsafe {
        let mut dev: DISPLAY_DEVICEW = zeroed();
        dev.cb = std::mem::size_of::<DISPLAY_DEVICEW>() as u32;

        let mut index = 0;
        while EnumDisplayDevicesW(std::ptr::null(), index, &mut dev, 0) != 0 {
            let is_attached = (dev.StateFlags & DISPLAY_DEVICE_ATTACHED_TO_DESKTOP) != 0;
            let is_primary = (dev.StateFlags & DISPLAY_DEVICE_PRIMARY_DEVICE) != 0;

            if is_attached && is_primary {
                let device_name = utf16_to_string(&dev.DeviceName);
                return query_display_info(&dev.DeviceName, device_name, is_primary);
            }
            index += 1;
        }

        // Fallback: look for any attached display if primary flag was not explicitly tagged
        index = 0;
        while EnumDisplayDevicesW(std::ptr::null(), index, &mut dev, 0) != 0 {
            let is_attached = (dev.StateFlags & DISPLAY_DEVICE_ATTACHED_TO_DESKTOP) != 0;
            if is_attached {
                let device_name = utf16_to_string(&dev.DeviceName);
                return query_display_info(&dev.DeviceName, device_name, false);
            }
            index += 1;
        }

        Err(DisplayError::PrimaryDisplayNotFound)
    }
}

/// Query the active refresh rate of the primary display.
pub fn get_active_refresh_rate() -> Result<u32, DisplayError> {
    let info = get_primary_display()?;
    Ok(info.current_refresh_rate)
}

/// Check whether a specific refresh rate (e.g. 60 or 144) is supported by the primary display at its current resolution.
pub fn is_refresh_rate_supported(target_hz: u32) -> Result<bool, DisplayError> {
    let info = get_primary_display()?;
    Ok(info.supported_refresh_rates.contains(&target_hz))
}

/// Safely test and set the display refresh rate for the primary monitor.
///
/// Modifies only `dmDisplayFrequency` while strictly preserving width, height,
/// orientation, bit depth, and scaling.
pub fn set_refresh_rate(target_hz: u32) -> Result<DisplayChangeResult, DisplayError> {
    unsafe {
        let mut dev: DISPLAY_DEVICEW = zeroed();
        dev.cb = std::mem::size_of::<DISPLAY_DEVICEW>() as u32;

        let mut primary_device_name: Option<[u16; 32]> = None;
        let mut index = 0;
        while EnumDisplayDevicesW(std::ptr::null(), index, &mut dev, 0) != 0 {
            let is_attached = (dev.StateFlags & DISPLAY_DEVICE_ATTACHED_TO_DESKTOP) != 0;
            let is_primary = (dev.StateFlags & DISPLAY_DEVICE_PRIMARY_DEVICE) != 0;
            if is_attached && (is_primary || primary_device_name.is_none()) {
                primary_device_name = Some(dev.DeviceName);
                if is_primary {
                    break;
                }
            }
            index += 1;
        }

        let dev_name_raw = primary_device_name.ok_or(DisplayError::PrimaryDisplayNotFound)?;
        let device_name_str = utf16_to_string(&dev_name_raw);

        // 1. Get current active DEVMODE
        let mut current_mode: DEVMODEW = zeroed();
        current_mode.dmSize = std::mem::size_of::<DEVMODEW>() as u16;
        if EnumDisplaySettingsW(
            dev_name_raw.as_ptr(),
            ENUM_CURRENT_SETTINGS,
            &mut current_mode,
        ) == 0
        {
            return Err(DisplayError::QueryCurrentSettingsFailed(device_name_str));
        }

        let current_hz = current_mode.dmDisplayFrequency;
        let width = current_mode.dmPelsWidth;
        let height = current_mode.dmPelsHeight;

        // If already at target refresh rate, confirm and return early
        if current_hz == target_hz {
            return Ok(DisplayChangeResult {
                device_name: device_name_str,
                target_refresh_rate: target_hz,
                actual_refresh_rate: current_hz,
                verified: true,
                message: format!("Display already operates at verified {target_hz} Hz"),
            });
        }

        // 2. Validate that target_hz is supported for this exact resolution
        let supported_rates = enumerate_supported_refresh_rates(&dev_name_raw, width, height);
        if !supported_rates.contains(&target_hz) {
            return Err(DisplayError::UnsupportedRefreshRate {
                requested_hz: target_hz,
                width,
                height,
                supported: supported_rates,
            });
        }

        // 3. Prepare target DEVMODE: modify only dmDisplayFrequency and dmFields
        let mut target_mode = current_mode;
        target_mode.dmDisplayFrequency = target_hz;
        target_mode.dmFields |= DM_DISPLAYFREQUENCY;

        // 4. Test mode with CDS_TEST
        let test_ret = ChangeDisplaySettingsExW(
            dev_name_raw.as_ptr(),
            &target_mode,
            std::ptr::null_mut(),
            CDS_TEST,
            std::ptr::null(),
        );

        if test_ret != DISP_CHANGE_SUCCESSFUL {
            return Err(DisplayError::ModeTestFailed {
                code: test_ret,
                reason: map_disp_change_code(test_ret),
            });
        }

        // 5. Apply mode with CDS_UPDATEREGISTRY
        let apply_ret = ChangeDisplaySettingsExW(
            dev_name_raw.as_ptr(),
            &target_mode,
            std::ptr::null_mut(),
            CDS_UPDATEREGISTRY,
            std::ptr::null(),
        );

        if apply_ret == DISP_CHANGE_RESTART {
            return Err(DisplayError::RestartRequired);
        } else if apply_ret != DISP_CHANGE_SUCCESSFUL {
            return Err(DisplayError::ModeApplyFailed {
                code: apply_ret,
                reason: map_disp_change_code(apply_ret),
            });
        }

        // 6. Read back current settings to verify actual frequency
        let mut verify_mode: DEVMODEW = zeroed();
        verify_mode.dmSize = std::mem::size_of::<DEVMODEW>() as u16;
        if EnumDisplaySettingsW(
            dev_name_raw.as_ptr(),
            ENUM_CURRENT_SETTINGS,
            &mut verify_mode,
        ) == 0
        {
            return Err(DisplayError::QueryCurrentSettingsFailed(device_name_str));
        }

        let actual_hz = verify_mode.dmDisplayFrequency;
        if actual_hz != target_hz {
            return Err(DisplayError::VerificationMismatch {
                expected_hz: target_hz,
                actual_hz,
            });
        }

        Ok(DisplayChangeResult {
            device_name: device_name_str,
            target_refresh_rate: target_hz,
            actual_refresh_rate: actual_hz,
            verified: true,
            message: format!(
                "Display refresh rate successfully switched and verified at {actual_hz} Hz"
            ),
        })
    }
}

unsafe fn query_display_info(
    dev_name: &[u16; 32],
    device_name_str: String,
    is_primary: bool,
) -> Result<DisplayInfo, DisplayError> {
    unsafe {
        let mut current_mode: DEVMODEW = zeroed();
        current_mode.dmSize = std::mem::size_of::<DEVMODEW>() as u16;

        if EnumDisplaySettingsW(dev_name.as_ptr(), ENUM_CURRENT_SETTINGS, &mut current_mode) == 0 {
            return Err(DisplayError::QueryCurrentSettingsFailed(device_name_str));
        }

        let width = current_mode.dmPelsWidth;
        let height = current_mode.dmPelsHeight;
        let current_refresh_rate = current_mode.dmDisplayFrequency;
        let supported_refresh_rates = enumerate_supported_refresh_rates(dev_name, width, height);

        Ok(DisplayInfo {
            device_name: device_name_str,
            is_primary,
            width,
            height,
            current_refresh_rate,
            supported_refresh_rates,
        })
    }
}

unsafe fn enumerate_supported_refresh_rates(
    dev_name: &[u16; 32],
    target_width: u32,
    target_height: u32,
) -> Vec<u32> {
    unsafe {
        let mut rates = Vec::new();
        let mut mode_idx = 0;
        let mut mode: DEVMODEW = zeroed();
        mode.dmSize = std::mem::size_of::<DEVMODEW>() as u16;

        while EnumDisplaySettingsW(dev_name.as_ptr(), mode_idx, &mut mode) != 0 {
            if mode.dmPelsWidth == target_width
                && mode.dmPelsHeight == target_height
                && mode.dmDisplayFrequency > 0
                && !rates.contains(&mode.dmDisplayFrequency)
            {
                rates.push(mode.dmDisplayFrequency);
            }
            mode_idx += 1;
        }

        rates.sort_unstable();
        rates
    }
}

fn utf16_to_string(slice: &[u16]) -> String {
    let end = slice.iter().position(|&c| c == 0).unwrap_or(slice.len());
    String::from_utf16_lossy(&slice[..end])
}

fn map_disp_change_code(code: i32) -> String {
    match code {
        DISP_CHANGE_SUCCESSFUL => "The settings change was successful".to_string(),
        DISP_CHANGE_BADMODE => "The graphics mode is not supported by the display".to_string(),
        DISP_CHANGE_FAILED => "The display driver failed the specified graphics mode".to_string(),
        DISP_CHANGE_RESTART => {
            "The computer must be restarted for the graphics mode to work".to_string()
        }
        DISP_CHANGE_BADPARAM => {
            "An invalid parameter was passed to ChangeDisplaySettingsEx".to_string()
        }
        DISP_CHANGE_BADFLAGS => {
            "An invalid set of flags was passed to ChangeDisplaySettingsEx".to_string()
        }
        DISP_CHANGE_NOTUPDATED => "Unable to write settings to the registry".to_string(),
        DISP_CHANGE_BADDUALVIEW => {
            "The settings change was unsuccessful because of DualView".to_string()
        }
        other => format!("Unknown display error ({other})"),
    }
}
