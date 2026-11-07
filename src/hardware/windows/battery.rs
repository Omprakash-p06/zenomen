//! Windows battery telemetry sampling via GetSystemPowerStatus (WIN-01).

use crate::hardware::BatteryTelemetry;

/// Parse raw Windows `SYSTEM_POWER_STATUS` fields into domain `BatteryTelemetry`.
///
/// Rules:
/// - If `flag == 128` (No system battery), returns `None`.
/// - `charge_percent`: if `percent <= 100`, `Some(percent)`, else `None` (255 = unknown).
/// - `charging`: `(flag & 8) != 0` indicates charging.
/// - `remaining_seconds`: if `life_time != 0xFFFFFFFF`, `Some(life_time)`, else `None`.
pub fn parse_battery_status(flag: u8, percent: u8, life_time: u32) -> Option<BatteryTelemetry> {
    if flag == 128 {
        return None;
    }

    let charge_percent = if percent <= 100 { Some(percent) } else { None };

    let charging = Some((flag & 8) != 0);

    let remaining_seconds = if life_time != 0xFFFFFFFF {
        Some(life_time)
    } else {
        None
    };

    Some(BatteryTelemetry {
        present: true,
        charge_percent,
        charging,
        remaining_seconds,
    })
}

/// Determine whether a physical battery is present on the machine.
pub fn read_battery_presence() -> bool {
    sample_battery_telemetry()
        .map(|b| b.present)
        .unwrap_or(false)
}

/// Sample battery presence, charge level, charging state, and remaining seconds.
pub fn sample_battery_telemetry() -> Option<BatteryTelemetry> {
    #[cfg(windows)]
    {
        use windows_sys::Win32::System::Power::{GetSystemPowerStatus, SYSTEM_POWER_STATUS};

        unsafe {
            let mut status: SYSTEM_POWER_STATUS = std::mem::zeroed();
            if GetSystemPowerStatus(&mut status) != 0 {
                parse_battery_status(
                    status.BatteryFlag,
                    status.BatteryLifePercent,
                    status.BatteryLifeTime,
                )
            } else {
                None
            }
        }
    }
    #[cfg(not(windows))]
    {
        None
    }
}
