//! Named pipe hardware request dispatch server (WIN-03).

use crate::rgb::schema::RgbProfile;
use crate::service::types::{HardwareControlError, PresetResult};
use crate::service::windows::authorization::authorize_mutation;
use crate::service::windows::hp_wmi::{self, HpWmiConnection};
use crate::service::windows::profiles::{BoardProfile, BoardValidation};
use crate::service::windows::protocol::{HardwareRequest, HardwareResponse};

/// Dispatch and execute a single hardware request against the board profile and WMI driver.
pub fn handle_request(
    req: HardwareRequest,
    board: &BoardProfile,
    validation: Option<&BoardValidation>,
    conn: &HpWmiConnection,
) -> HardwareResponse {
    // 1. Authorize the mutation against board profile safety invariants and physical validation record
    if let Err(e) = authorize_mutation(&req, board, validation) {
        return HardwareResponse::Error(e.to_string());
    }

    // 2. Dispatch authorized request
    match req {
        HardwareRequest::GetCapabilities => match hp_wmi::query_hp_capabilities(conn) {
            Ok(caps) => HardwareResponse::Capabilities(caps),
            Err(e) => HardwareResponse::Error(e.to_string()),
        },
        HardwareRequest::GetFanStatus => match hp_wmi::query_fan_status(conn) {
            Ok((mode, rpms)) => HardwareResponse::FanStatus { mode, rpms },
            Err(e) => HardwareResponse::Error(e.to_string()),
        },
        HardwareRequest::SetFanMode { mode } => match hp_wmi::set_fan_mode(conn, &mode) {
            Ok(m) => HardwareResponse::FanModeSet(m),
            Err(e) => HardwareResponse::Error(e.to_string()),
        },
        HardwareRequest::GetPowerProfiles => {
            HardwareResponse::PowerProfiles(board.supported_power_profiles.clone())
        }
        HardwareRequest::GetPowerProfile => HardwareResponse::PowerProfile(
            board
                .supported_power_profiles
                .first()
                .cloned()
                .unwrap_or_else(|| "balanced".to_string()),
        ),
        HardwareRequest::SetPowerProfile { profile } => {
            match hp_wmi::set_performance_mode(conn, &profile) {
                Ok(p) => HardwareResponse::PowerProfileSet(p),
                Err(e) => HardwareResponse::Error(e.to_string()),
            }
        }
        HardwareRequest::ApplyPreset { preset } => {
            let normalized = preset.trim().to_lowercase();
            let (target_power, target_fan) = match normalized.as_str() {
                "quiet" => ("quiet", "auto"),
                "balanced" => ("balanced", "auto"),
                "gaming" => ("performance", "max"),
                other => {
                    return HardwareResponse::Error(format!("Unknown preset '{other}'"));
                }
            };

            let p_res = hp_wmi::set_performance_mode(conn, target_power);
            let f_res = hp_wmi::set_fan_mode(conn, target_fan);

            let power_applied = p_res.is_ok();
            let fan_applied = f_res.is_ok();
            let partial_success = power_applied ^ fan_applied;

            let message = if power_applied && fan_applied {
                format!("Applied combined preset '{normalized}' successfully")
            } else if power_applied {
                "Applied power profile, but fan mode change failed".to_string()
            } else if fan_applied {
                "Applied fan mode, but power profile change failed".to_string()
            } else {
                "Failed to apply preset".to_string()
            };

            HardwareResponse::PresetApplied(PresetResult {
                preset_name: normalized,
                power_applied,
                confirmed_power_profile: p_res.ok(),
                fan_applied,
                confirmed_fan_mode: f_res.ok(),
                partial_success,
                message,
            })
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
        HardwareRequest::SetRgbEnabled { enabled } => HardwareResponse::RgbEnabledSet(enabled),
    }
}

/// Run the named pipe hardware server loop on \\.\pipe\ZenOmen.Hardware.v1 (WIN-03).
///
/// Listens for client connections, verifies client session security, decodes incoming
/// length-prefixed frames, dispatches authorized hardware commands, and writes responses.
/// Runs until `stop_signal` is set to true.
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

        // Resolve active motherboard
        let board_id =
            crate::hardware::windows::device::read_board_id().unwrap_or_else(|| "88F8".to_string());
        let board = crate::service::windows::profiles::resolve_board_profile(&board_id)
            .unwrap_or_else(|| crate::service::windows::profiles::BoardProfile {
                board_id: board_id.clone(),
                model_family: "Generic HP OMEN / Victus".to_string(),
                fan_count: 2,
                has_max_fan: true,
                has_custom_curves: false,
                supported_power_profiles: vec!["balanced".to_string(), "performance".to_string()],
                rgb_zone_count: 0,
            });

        let validation = BoardValidation::new_unvalidated(&board.board_id, "F.00");
        let conn = hp_wmi::connect_hp_wmi().unwrap_or(HpWmiConnection {
            is_available: false,
        });

        tracing::info!(
            board = %board.board_id,
            model = %board.model_family,
            wmi_available = conn.is_available,
            "Named pipe hardware server started listening"
        );

        let mut first_instance = true;

        while !stop_signal.load(Ordering::Relaxed) {
            let pipe = match create_server_pipe(first_instance) {
                Ok(h) => h,
                Err(e) => {
                    tracing::error!("Failed to create named pipe instance: {e}");
                    std::thread::sleep(std::time::Duration::from_millis(500));
                    continue;
                }
            };
            first_instance = false;

            // Wait for client to connect
            let connected = unsafe { ConnectNamedPipe(pipe, std::ptr::null_mut()) };
            if connected == 0 {
                let err = unsafe { windows_sys::Win32::Foundation::GetLastError() };
                if err != ERROR_PIPE_CONNECTED {
                    unsafe { CloseHandle(pipe) };
                    continue;
                }
            }

            // Verify caller session to prevent cross-session / RDP tampering
            if let Err(e) = verify_client_session(pipe) {
                tracing::warn!("Rejecting unauthorized client connection: {e}");
                unsafe {
                    DisconnectNamedPipe(pipe);
                    CloseHandle(pipe);
                }
                continue;
            }

            // Read 4-byte length prefix
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

            if success == 0 || bytes_read != 4 {
                unsafe {
                    DisconnectNamedPipe(pipe);
                    CloseHandle(pipe);
                }
                continue;
            }

            let payload_len = u32::from_be_bytes(len_buf) as usize;
            if payload_len > MAX_PAYLOAD_SIZE {
                tracing::warn!(
                    payload_len,
                    "Client frame exceeds MAX_PAYLOAD_SIZE, dropping"
                );
                unsafe {
                    DisconnectNamedPipe(pipe);
                    CloseHandle(pipe);
                }
                continue;
            }

            // Read payload bytes
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
                if ok == 0 || chunk_read == 0 {
                    read_failed = true;
                    break;
                }
                total_read += chunk_read as usize;
            }

            if read_failed || total_read != payload_len {
                unsafe {
                    DisconnectNamedPipe(pipe);
                    CloseHandle(pipe);
                }
                continue;
            }

            // Reconstruct full frame for decode
            let mut frame = Vec::with_capacity(4 + payload_len);
            frame.extend_from_slice(&len_buf);
            frame.extend_from_slice(&payload);

            let (request, _): (HardwareRequest, usize) = match decode_frame(&frame) {
                Ok(r) => r,
                Err(e) => {
                    tracing::error!("Failed to decode client request frame: {e}");
                    unsafe {
                        DisconnectNamedPipe(pipe);
                        CloseHandle(pipe);
                    }
                    continue;
                }
            };

            // Process request and produce response
            let response = handle_request(request, &board, Some(&validation), &conn);

            // Encode and transmit response
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

            unsafe {
                DisconnectNamedPipe(pipe);
                CloseHandle(pipe);
            }
        }

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
