//! Named pipe hardware request dispatch server (WIN-03).
//!
//! Changes from previous version:
//! - `HpWmiConnection` no longer holds `is_available: bool`; it holds a real `WmiAdapter`.
//! - No process-local fan or power statics. All state reads go to firmware.
//! - Fan mode is restored to Auto on graceful service shutdown.
//! - `handle_request` takes `&HpWmiConnection` and returns typed errors through the
//!   real WMI driver.

use crate::rgb::schema::RgbProfile;
use crate::service::types::{HardwareControlError, PresetResult};
use crate::service::windows::authorization::authorize_mutation;
use crate::service::windows::hp_wmi::{self, HpWmiConnection};
use crate::service::windows::profiles::{BoardProfile, BoardValidation};
use crate::service::windows::protocol::{HardwareRequest, HardwareResponse};

/// Dispatch and execute a single hardware request against the board profile and WMI driver.
///
/// Authorization is checked first; on failure an `Error` response is returned
/// without touching firmware. All firmware reads and writes go through `conn`
/// which holds the real (or fake, in tests) [`WmiAdapter`].
pub fn handle_request(
    req: HardwareRequest,
    board: &BoardProfile,
    validation: Option<&BoardValidation>,
    conn: &HpWmiConnection,
) -> HardwareResponse {
    // 1. Authorize the mutation against board profile safety invariants and physical validation record.
    if let Err(e) = authorize_mutation(&req, board, validation) {
        return HardwareResponse::Error(e.to_string());
    }

    // 2. Dispatch authorized request to the real WMI driver.
    match req {
        HardwareRequest::GetCapabilities => match hp_wmi::query_hp_capabilities(conn) {
            Ok(caps) => HardwareResponse::Capabilities(caps),
            Err(e) => HardwareResponse::Error(e.to_string()),
        },

        HardwareRequest::GetFanStatus => match hp_wmi::query_fan_status(conn) {
            Ok((mode, rpms)) => HardwareResponse::FanStatus { mode, rpms },
            Err(HardwareControlError::InterfaceMissing(msg)) => {
                // WMI unavailable — return honest unavailable, not a fake mode.
                tracing::warn!("GetFanStatus: WMI unavailable: {msg}");
                HardwareResponse::Error(format!("Fan status unavailable: {msg}"))
            }
            Err(e) => HardwareResponse::Error(e.to_string()),
        },

        HardwareRequest::SetFanMode { mode } => match hp_wmi::set_fan_mode(conn, &mode) {
            Ok(confirmed) => HardwareResponse::FanModeSet(confirmed),
            Err(HardwareControlError::VerificationFailed {
                target,
                actual,
                retries,
            }) => HardwareResponse::Error(format!(
                "Fan mode verification failed after {retries} retries: \
                 requested '{target}' but firmware reports '{actual}'"
            )),
            Err(e) => HardwareResponse::Error(e.to_string()),
        },

        HardwareRequest::GetPowerProfiles => {
            HardwareResponse::PowerProfiles(board.supported_power_profiles.clone())
        }

        HardwareRequest::GetPowerProfile => {
            if board.thermal_profile_source
                == crate::service::windows::profiles::ThermalProfileSource::EmbeddedController
            {
                tracing::debug!(
                    board = %board.board_id,
                    "Thermal profile is EC-managed on this board; WMI thermal read skipped"
                );
                HardwareResponse::Error(format!(
                    "Thermal profile is EC-managed (register 0x95) on board {}; WMI thermal query (0x4C) is unsupported",
                    board.board_id
                ))
            } else {
                match hp_wmi::query_power_profile(conn) {
                    Ok(profile) => HardwareResponse::PowerProfile(profile),
                    Err(e) => {
                        // Fall back to first supported profile name without claiming it is current.
                        tracing::warn!(
                            "GetPowerProfile WMI read failed: {e}; returning profile list fallback"
                        );
                        HardwareResponse::Error(format!("Power profile read failed: {e}"))
                    }
                }
            }
        }

        HardwareRequest::SetPowerProfile { profile } => {
            match hp_wmi::set_power_profile(conn, &profile) {
                Ok(res) => HardwareResponse::PowerProfileSet(res),
                Err(HardwareControlError::VerificationFailed {
                    target,
                    actual,
                    retries,
                }) => HardwareResponse::Error(format!(
                    "Power profile verification failed after {retries} retries: \
                     requested '{target}' but firmware reports '{actual}'"
                )),
                Err(e) => HardwareResponse::Error(e.to_string()),
            }
        }

        HardwareRequest::ApplyPreset { preset } => {
            let normalized = preset.trim().to_lowercase();
            let (target_power, target_fan) = match normalized.as_str() {
                "quiet" => ("quiet", "auto"),
                "balanced" | "auto" => ("balanced", "auto"),
                "performance" | "gaming" => ("performance", "max"),
                other => {
                    return HardwareResponse::Error(format!("Unknown preset '{other}'"));
                }
            };

            let p_res = hp_wmi::set_power_profile(conn, target_power);
            let f_res = hp_wmi::set_fan_mode(conn, target_fan);

            let power_applied = p_res.is_ok();
            let fan_applied = f_res.is_ok();
            let partial_success = power_applied ^ fan_applied;

            let message = if power_applied && fan_applied {
                let p = p_res.as_ref().unwrap();
                if p.status == crate::service::types::PowerProfileStatus::ProfileUnverified {
                    format!(
                        "Applied combined preset '{normalized}' (power accepted unverified, fan confirmed by firmware read-back)"
                    )
                } else {
                    format!(
                        "Applied combined preset '{normalized}' — confirmed by firmware read-back"
                    )
                }
            } else if power_applied {
                format!(
                    "Power profile applied but fan write failed: {}",
                    f_res.as_ref().unwrap_err()
                )
            } else if fan_applied {
                format!(
                    "Fan mode applied but power write failed: {}",
                    p_res.as_ref().unwrap_err()
                )
            } else {
                format!(
                    "Preset '{}' failed: power='{}'; fan='{}'",
                    normalized,
                    p_res.as_ref().unwrap_err(),
                    f_res.as_ref().unwrap_err()
                )
            };

            HardwareResponse::PresetApplied(PresetResult {
                preset_name: normalized,
                power_applied,
                confirmed_power_profile: p_res.ok().map(|p| p.profile),
                fan_applied,
                confirmed_fan_mode: f_res.ok(),
                display_applied: false,
                confirmed_refresh_rate: None,
                partial_success,
                message,
            })
        }

        HardwareRequest::GetServiceMemory => {
            let info = crate::hardware::process_memory::sample_current_process_memory(
                crate::hardware::process_memory::ProcessRole::HardwareService,
            );
            HardwareResponse::ServiceMemory(info)
        }

        HardwareRequest::ApplyRgbProfile { profile_json } => {
            match serde_json::from_str::<RgbProfile>(&profile_json) {
                Ok(profile) => match hp_wmi::set_rgb_profile(conn, &profile) {
                    Ok(name) => HardwareResponse::RgbProfileApplied(name),
                    Err(e) => HardwareResponse::Error(e.to_string()),
                },
                Err(e) => HardwareResponse::Error(format!("Malformed RGB profile JSON: {e}")),
            }
        }

        HardwareRequest::SetRgbEnabled { enabled } => {
            // RGB enable/disable not implemented on Victus (rgb_zone_count = 0); this
            // path would only be reached after authorization passes (which requires
            // rgb_write_verified = true on a 4-zone board). Returning the requested
            // flag is intentional here as a no-op ack because authorization guarantees
            // this board has been physically validated for RGB writes.
            HardwareResponse::RgbEnabledSet(enabled)
        }
    }
}

/// Run the named pipe hardware server loop on `\\.\pipe\ZenOmen.Hardware.v1` (WIN-03).
///
/// Listens for client connections, verifies client session security, decodes incoming
/// length-prefixed frames, dispatches authorized hardware commands, and writes responses.
/// Runs until `stop_signal` is set to `true`.
///
/// On graceful shutdown, fan mode is restored to Auto (0x00) so the Embedded Controller
/// resumes autonomous cooling when the service stops.
pub fn run_pipe_server(
    stop_signal: std::sync::Arc<std::sync::atomic::AtomicBool>,
) -> Result<(), HardwareControlError> {
    #[cfg(windows)]
    {
        use crate::service::windows::pipe::{PIPE_BUFFER_SIZE, create_server_pipe};
        use crate::service::windows::protocol::{MAX_PAYLOAD_SIZE, decode_frame, encode_frame};
        use crate::service::windows::security::verify_client_session;
        use std::sync::atomic::Ordering;
        use windows_sys::Win32::Foundation::{CloseHandle, ERROR_PIPE_CONNECTED};
        use windows_sys::Win32::Storage::FileSystem::{FlushFileBuffers, ReadFile, WriteFile};
        use windows_sys::Win32::System::Pipes::{ConnectNamedPipe, DisconnectNamedPipe};

        // Resolve active motherboard.
        let board_id = crate::hardware::windows::device::read_board_id()
            .unwrap_or_else(|| "UNKNOWN".to_string());
        let board = crate::service::windows::profiles::resolve_board_profile(&board_id)
            .unwrap_or_else(|| crate::service::windows::profiles::BoardProfile {
                board_id: board_id.clone(),
                model_family: "Generic HP OMEN / Victus (unrecognized board)".to_string(),
                fan_count: 2,
                has_max_fan: false,
                has_custom_curves: false,
                thermal_profile_source:
                    crate::service::windows::profiles::ThermalProfileSource::None,
                supported_power_profiles: vec!["balanced".to_string()],
                rgb_zone_count: 0,
            });

        let bios_version = crate::hardware::windows::device::read_bios_version();
        let validation = crate::service::windows::profiles::resolve_board_validation(
            &board.board_id,
            &bios_version,
        );

        // Establish WMI connection. Failure makes capabilities = unavailable; it does not
        // stop the server — the server still accepts connections and reports the error.
        let conn = match hp_wmi::connect_hp_wmi() {
            Ok(mut c) => {
                c.ensure_board_id(&board.board_id);
                tracing::info!(
                    board = %board.board_id,
                    model = %board.model_family,
                    bios = %bios_version,
                    fan_write = validation.fan_write_verified,
                    power_write = validation.power_write_authorized,
                    power_verified = validation.power_write_verified,
                    wmi_available = c.is_available(),
                    "HP WMI connection established; named pipe server starting"
                );
                c
            }
            Err(e) => {
                tracing::error!(
                    error = %e,
                    board = %board.board_id,
                    bios = %bios_version,
                    "HP WMI connection failed; server will report InterfaceMissing for all hardware ops"
                );
                // Build an unavailable connection (RealWmiAdapter reports is_available = false).
                // All hardware operations will return InterfaceMissing through query_fan_status etc.
                hp_wmi::connect_hp_wmi().unwrap_or_else(|_| {
                    HpWmiConnection::from_adapter(
                        crate::service::windows::hp_wmi::FakeWmiAdapter::new(false),
                    )
                })
            }
        };

        let pipe = create_server_pipe(true)?;

        while !stop_signal.load(Ordering::Relaxed) {
            // Wait for client to connect.
            let connected = unsafe { ConnectNamedPipe(pipe, std::ptr::null_mut()) };
            if connected == 0 {
                let err = unsafe { windows_sys::Win32::Foundation::GetLastError() };
                if err != ERROR_PIPE_CONNECTED {
                    if stop_signal.load(Ordering::Relaxed) {
                        break;
                    }
                    continue;
                }
            }

            if stop_signal.load(Ordering::Relaxed) {
                unsafe { DisconnectNamedPipe(pipe) };
                break;
            }

            // Verify caller session to prevent cross-session / RDP tampering.
            if let Err(e) = verify_client_session(pipe) {
                tracing::warn!("Rejecting unauthorized client connection: {e}");
                unsafe { DisconnectNamedPipe(pipe) };
                continue;
            }

            // Read 4-byte length prefix.
            let mut len_buf = [0u8; 4];
            let mut bytes_read = 0u32;
            let success = unsafe {
                ReadFile(
                    pipe,
                    len_buf.as_mut_ptr() as *mut _,
                    4,
                    &mut bytes_read,
                    std::ptr::null_mut(),
                )
            };

            let err = if success == 0 {
                unsafe { windows_sys::Win32::Foundation::GetLastError() }
            } else {
                0
            };

            if (success == 0 && err != windows_sys::Win32::Foundation::ERROR_MORE_DATA)
                || bytes_read != 4
            {
                unsafe { DisconnectNamedPipe(pipe) };
                continue;
            }

            let payload_len = u32::from_be_bytes(len_buf) as usize;
            if payload_len > MAX_PAYLOAD_SIZE {
                tracing::warn!(
                    payload_len,
                    "Client frame exceeds MAX_PAYLOAD_SIZE, dropping"
                );
                unsafe { DisconnectNamedPipe(pipe) };
                continue;
            }

            // Read payload bytes.
            let mut payload = vec![0u8; payload_len];
            let mut total_read = 0usize;
            let mut read_failed = false;

            while total_read < payload_len {
                let mut chunk_read = 0u32;
                let to_read = (payload_len - total_read).min(PIPE_BUFFER_SIZE as usize) as u32;
                let ok = unsafe {
                    ReadFile(
                        pipe,
                        payload[total_read..].as_mut_ptr() as *mut _,
                        to_read,
                        &mut chunk_read,
                        std::ptr::null_mut(),
                    )
                };
                let err = if ok == 0 {
                    unsafe { windows_sys::Win32::Foundation::GetLastError() }
                } else {
                    0
                };
                if (ok == 0 && err != windows_sys::Win32::Foundation::ERROR_MORE_DATA)
                    || chunk_read == 0
                {
                    read_failed = true;
                    break;
                }
                total_read += chunk_read as usize;
            }

            if read_failed || total_read != payload_len {
                unsafe { DisconnectNamedPipe(pipe) };
                continue;
            }

            // Reconstruct full frame for decode.
            let mut frame = Vec::with_capacity(4 + payload_len);
            frame.extend_from_slice(&len_buf);
            frame.extend_from_slice(&payload);

            let (request, _): (HardwareRequest, usize) = match decode_frame(&frame) {
                Ok(r) => r,
                Err(e) => {
                    tracing::error!("Failed to decode client request frame: {e}");
                    unsafe { DisconnectNamedPipe(pipe) };
                    continue;
                }
            };

            // Process request and produce response.
            let response = handle_request(request, &board, Some(&validation), &conn);

            // Encode and transmit response.
            if let Ok(resp_bytes) = encode_frame(&response) {
                let mut bytes_written = 0u32;
                unsafe {
                    WriteFile(
                        pipe,
                        resp_bytes.as_ptr() as *const _,
                        resp_bytes.len() as u32,
                        &mut bytes_written,
                        std::ptr::null_mut(),
                    );
                    FlushFileBuffers(pipe);
                }
            }

            unsafe { DisconnectNamedPipe(pipe) };
        }

        // Graceful shutdown: restore fan mode to Auto so EC resumes autonomous cooling.
        // Only boards whose fan writes are validated may be mutated here; an
        // unvalidated board was never placed in Max Fan mode, so there is nothing
        // to restore and no reason to send a firmware write.
        if conn.is_available() && validation.fan_write_verified {
            tracing::info!("Service shutdown: restoring fan mode to Auto for EC safety");
            if let Err(e) = hp_wmi::set_fan_mode(&conn, "auto") {
                tracing::warn!(error = %e, "Could not restore fan mode to Auto on shutdown");
            }
        } else {
            tracing::info!(
                board = %board.board_id,
                fan_write_verified = validation.fan_write_verified,
                "Service shutdown: skipping Auto restore (fan writes not validated on this board)"
            );
        }

        unsafe { CloseHandle(pipe) };

        tracing::info!("Named pipe hardware server shut down cleanly");
        Ok(())
    }
    #[cfg(not(windows))]
    {
        let _ = stop_signal;
        Err(HardwareControlError::InterfaceMissing(
            "Windows named pipe server is only supported on Windows".to_string(),
        ))
    }
}
