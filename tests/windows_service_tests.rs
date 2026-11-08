//! Unit tests for Windows named pipe protocol, board profiles, and request authorization (WIN-03, WIN-04, WIN-07).

use zenomen::service::types::HardwareControlError;
use zenomen::service::windows::authorization::authorize_mutation;
use zenomen::service::windows::hp_wmi::HpWmiConnection;
use zenomen::service::windows::profiles::{BoardValidation, resolve_board_profile};
use zenomen::service::windows::protocol::{
    HardwareRequest, HardwareResponse, MAX_PAYLOAD_SIZE, decode_frame, encode_frame,
};
use zenomen::service::windows::server::handle_request;

#[test]
fn test_named_pipe_protocol_roundtrip_all_requests() {
    let requests = vec![
        HardwareRequest::GetCapabilities,
        HardwareRequest::GetFanStatus,
        HardwareRequest::SetFanMode {
            mode: "max".to_string(),
        },
        HardwareRequest::GetPowerProfiles,
        HardwareRequest::GetPowerProfile,
        HardwareRequest::SetPowerProfile {
            profile: "performance".to_string(),
        },
        HardwareRequest::ApplyPreset {
            preset: "gaming".to_string(),
        },
        HardwareRequest::ApplyRgbProfile {
            profile_json:
                r##"{"name":"Static White","zones":["#FFFFFF","#FFFFFF","#FFFFFF","#FFFFFF"]}"##
                    .to_string(),
        },
        HardwareRequest::SetRgbEnabled { enabled: true },
    ];

    for req in requests {
        let frame = encode_frame(&req).expect("encoding request frame");
        assert!(frame.len() >= 4);

        let (decoded, consumed) =
            decode_frame::<HardwareRequest>(&frame).expect("decoding request frame");
        assert_eq!(consumed, frame.len());
        assert_eq!(decoded, req);
    }
}

#[test]
fn test_protocol_rejects_payload_exceeding_64kb() {
    // Construct request with payload exceeding 64 KB (65536 bytes)
    let huge_string = "A".repeat(MAX_PAYLOAD_SIZE + 10);
    let invalid_req = HardwareRequest::ApplyRgbProfile {
        profile_json: huge_string,
    };

    let encode_err = encode_frame(&invalid_req).expect_err("should reject >64KB payload");
    assert!(matches!(encode_err, HardwareControlError::ProtocolError(_)));

    // Test decoding a frame with header length > MAX_PAYLOAD_SIZE
    let mut bad_header = (70000u32).to_be_bytes().to_vec();
    bad_header.extend_from_slice(&[0u8; 100]);
    let decode_err =
        decode_frame::<HardwareRequest>(&bad_header).expect_err("should reject >64KB header");
    assert!(matches!(decode_err, HardwareControlError::ProtocolError(_)));
}

#[test]
fn test_board_profile_resolves_known_omen_boards() {
    // 88F8: OMEN 15/16 AMD
    let omen_amd = resolve_board_profile("88F8").expect("resolve 88F8");
    assert_eq!(omen_amd.board_id, "88F8");
    assert_eq!(omen_amd.fan_count, 2);
    assert!(omen_amd.has_max_fan);
    assert_eq!(omen_amd.rgb_zone_count, 4);

    // Case-insensitivity check ("8a24" lowercase)
    let omen_intel = resolve_board_profile("8a24").expect("resolve 8A24 lowercase");
    assert_eq!(omen_intel.board_id, "8A24");
    assert!(omen_intel.has_max_fan);

    // Victus 8A3E: 1 RGB zone
    let victus = resolve_board_profile("8A3E").expect("resolve 8A3E");
    assert_eq!(victus.board_id, "8A3E");
    assert_eq!(victus.rgb_zone_count, 1);
}

#[test]
fn test_board_profile_rejects_unknown_motherboard() {
    assert_eq!(resolve_board_profile("9999"), None);
    assert_eq!(resolve_board_profile("UNSUPPORTED"), None);
    assert_eq!(resolve_board_profile(""), None);
}

#[test]
fn test_authorization_blocks_unsupported_fan_mode() {
    let board = resolve_board_profile("88F8").unwrap();
    let validation = BoardValidation::new_verified("88F8", "F.20");

    // Valid modes
    assert!(
        authorize_mutation(
            &HardwareRequest::SetFanMode {
                mode: "auto".to_string()
            },
            &board,
            Some(&validation)
        )
        .is_ok()
    );
    assert!(
        authorize_mutation(
            &HardwareRequest::SetFanMode {
                mode: "max".to_string()
            },
            &board,
            Some(&validation)
        )
        .is_ok()
    );

    // Invalid modes rejected with typed errors
    let manual_err = authorize_mutation(
        &HardwareRequest::SetFanMode {
            mode: "manual".to_string(),
        },
        &board,
        Some(&validation),
    )
    .expect_err("reject manual");
    assert!(matches!(
        manual_err,
        HardwareControlError::UnsupportedMode(_)
    ));

    let unknown_err = authorize_mutation(
        &HardwareRequest::SetFanMode {
            mode: "extreme_turbo".to_string(),
        },
        &board,
        Some(&validation),
    )
    .expect_err("reject extreme_turbo");
    assert!(matches!(
        unknown_err,
        HardwareControlError::InvalidParameter(_)
    ));
}

#[test]
fn test_authorization_blocks_unvalidated_hardware_mutations() {
    let board = resolve_board_profile("88F8").unwrap();
    let unvalidated = BoardValidation::new_unvalidated("88F8", "F.20");

    // Read requests always pass even without validation
    assert!(authorize_mutation(&HardwareRequest::GetCapabilities, &board, None).is_ok());
    assert!(authorize_mutation(&HardwareRequest::GetFanStatus, &board, None).is_ok());
    assert!(authorize_mutation(&HardwareRequest::GetPowerProfiles, &board, None).is_ok());
    assert!(authorize_mutation(&HardwareRequest::GetPowerProfile, &board, None).is_ok());

    // Mutations fail when validation is None
    let fan_req = HardwareRequest::SetFanMode {
        mode: "max".to_string(),
    };
    let power_req = HardwareRequest::SetPowerProfile {
        profile: "performance".to_string(),
    };
    let preset_req = HardwareRequest::ApplyPreset {
        preset: "gaming".to_string(),
    };
    let rgb_req = HardwareRequest::SetRgbEnabled { enabled: true };

    assert!(matches!(
        authorize_mutation(&fan_req, &board, None).unwrap_err(),
        HardwareControlError::UnsupportedMode(_)
    ));
    assert!(matches!(
        authorize_mutation(&power_req, &board, None).unwrap_err(),
        HardwareControlError::UnsupportedMode(_)
    ));
    assert!(matches!(
        authorize_mutation(&preset_req, &board, None).unwrap_err(),
        HardwareControlError::UnsupportedMode(_)
    ));
    assert!(matches!(
        authorize_mutation(&rgb_req, &board, None).unwrap_err(),
        HardwareControlError::UnsupportedMode(_)
    ));

    // Mutations fail when validation record exists but write flags are false
    assert!(matches!(
        authorize_mutation(&fan_req, &board, Some(&unvalidated)).unwrap_err(),
        HardwareControlError::UnsupportedMode(_)
    ));
    assert!(matches!(
        authorize_mutation(&power_req, &board, Some(&unvalidated)).unwrap_err(),
        HardwareControlError::UnsupportedMode(_)
    ));
    assert!(matches!(
        authorize_mutation(&preset_req, &board, Some(&unvalidated)).unwrap_err(),
        HardwareControlError::UnsupportedMode(_)
    ));
    assert!(matches!(
        authorize_mutation(&rgb_req, &board, Some(&unvalidated)).unwrap_err(),
        HardwareControlError::UnsupportedMode(_)
    ));

    // Board mismatch rejected
    let mismatch_val = BoardValidation::new_verified("8A24", "F.10");
    assert!(matches!(
        authorize_mutation(&fan_req, &board, Some(&mismatch_val)).unwrap_err(),
        HardwareControlError::UnsupportedMode(_)
    ));
}

#[test]
fn test_mock_named_pipe_client_server_communication() {
    let board = resolve_board_profile("88F8").unwrap();
    let validation = BoardValidation::new_verified("88F8", "F.20");
    let conn = HpWmiConnection { is_available: true };

    // 1. Client builds request and serializes frame
    let req = HardwareRequest::ApplyPreset {
        preset: "gaming".to_string(),
    };
    let tx_frame = encode_frame(&req).expect("client encode");

    // 2. Server decodes frame from stream
    let (server_req, _): (HardwareRequest, usize) = decode_frame(&tx_frame).expect("server decode");
    assert_eq!(server_req, req);

    // 3. Server handles request and generates response
    let response = handle_request(server_req, &board, Some(&validation), &conn);

    // 4. Server encodes response
    let resp_frame = encode_frame(&response).expect("server encode response");

    // 5. Client decodes response
    let (client_resp, _): (HardwareResponse, usize) =
        decode_frame(&resp_frame).expect("client decode response");

    match client_resp {
        HardwareResponse::PresetApplied(result) => {
            assert_eq!(result.preset_name, "gaming");
            assert!(result.is_full_success());
            assert_eq!(result.confirmed_fan_mode, Some("max".to_string()));
            assert_eq!(
                result.confirmed_power_profile,
                Some("performance".to_string())
            );
        }
        other => panic!("Unexpected response: {other:?}"),
    }
}

#[test]
fn test_lifecycle_and_server_guards_on_non_windows() {
    #[cfg(not(windows))]
    {
        use std::sync::Arc;
        use std::sync::atomic::AtomicBool;
        use zenomen::service::windows::lifecycle::{
            register_service, run_console_service, run_service, uninstall_service,
        };
        use zenomen::service::windows::pipe::create_server_pipe;
        use zenomen::service::windows::server::run_pipe_server;

        assert!(matches!(
            register_service().unwrap_err(),
            HardwareControlError::InterfaceMissing(_)
        ));
        assert!(matches!(
            uninstall_service().unwrap_err(),
            HardwareControlError::InterfaceMissing(_)
        ));
        assert!(matches!(
            run_service().unwrap_err(),
            HardwareControlError::InterfaceMissing(_)
        ));
        assert!(matches!(
            run_console_service().unwrap_err(),
            HardwareControlError::InterfaceMissing(_)
        ));
        assert!(matches!(
            create_server_pipe(true).unwrap_err(),
            HardwareControlError::InterfaceMissing(_)
        ));
        assert!(matches!(
            run_pipe_server(Arc::new(AtomicBool::new(true))).unwrap_err(),
            HardwareControlError::InterfaceMissing(_)
        ));
    }
}
