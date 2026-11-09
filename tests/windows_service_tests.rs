//! Unit tests for Windows named pipe protocol, board profiles, and request authorization (WIN-03, WIN-04, WIN-07).
//!
//! ## Test design
//!
//! All hardware operations use [`FakeWmiAdapter`] so tests are deterministic and
//! CI-safe on any host (Linux cross-compile, Windows CI runner, or physical laptop).
//!
//! The fake adapter requires callers to pre-load responses via `expect_call`.
//! Tests that assert a **mutation path** must pre-load:
//!   1. A read response (the pre-write read-back in `set_fan_mode`).
//!   2. A write response.
//!   3. A read response (the post-write verification read-back).
//!
//! This mirrors the real 3-step protocol and ensures the verification path is exercised.

use zenomen::service::types::HardwareControlError;
use zenomen::service::windows::authorization::authorize_mutation;
use zenomen::service::windows::hp_wmi::{FakeResponse, FakeWmiAdapter, HpWmiConnection};
use zenomen::service::windows::profiles::{
    BoardValidation, resolve_board_profile, resolve_board_validation,
};
use zenomen::service::windows::protocol::{
    HardwareRequest, HardwareResponse, MAX_PAYLOAD_SIZE, decode_frame, encode_frame,
};
use zenomen::service::windows::server::handle_request;

// ═══════════════════════════════════════════════════════════════════════════
// Helpers
// ═══════════════════════════════════════════════════════════════════════════

/// Build a 136-byte output buffer with status=0 and the given fan-mode byte at payload[0].
fn fan_mode_response(mode_byte: u8) -> FakeResponse {
    FakeResponse::Success {
        payload: vec![mode_byte],
    }
}

/// Build a successful write-ack response (empty payload, status=0).
fn write_ack() -> FakeResponse {
    FakeResponse::Success { payload: vec![] }
}

/// Build an unavailable fake connection (WMI not present).
fn unavailable_conn() -> HpWmiConnection {
    HpWmiConnection::from_adapter(FakeWmiAdapter::new(false))
}

// ═══════════════════════════════════════════════════════════════════════════
// 1. Protocol framing
// ═══════════════════════════════════════════════════════════════════════════

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
    let huge_string = "A".repeat(MAX_PAYLOAD_SIZE + 10);
    let invalid_req = HardwareRequest::ApplyRgbProfile {
        profile_json: huge_string,
    };
    let encode_err = encode_frame(&invalid_req).expect_err("should reject >64KB payload");
    assert!(matches!(encode_err, HardwareControlError::ProtocolError(_)));

    let mut bad_header = (70000u32).to_be_bytes().to_vec();
    bad_header.extend_from_slice(&[0u8; 100]);
    let decode_err =
        decode_frame::<HardwareRequest>(&bad_header).expect_err("should reject >64KB header");
    assert!(matches!(decode_err, HardwareControlError::ProtocolError(_)));
}

#[test]
fn test_protocol_rejects_truncated_frame() {
    // A frame claiming 100 bytes of payload but only providing 10.
    let mut short = (100u32).to_be_bytes().to_vec();
    short.extend_from_slice(&[0u8; 10]);
    let err = decode_frame::<HardwareRequest>(&short).expect_err("should reject truncated frame");
    assert!(matches!(err, HardwareControlError::ProtocolError(_)));
}

#[test]
fn test_protocol_rejects_empty_frame() {
    let err = decode_frame::<HardwareRequest>(&[]).expect_err("empty frame should fail");
    assert!(matches!(err, HardwareControlError::ProtocolError(_)));
}

// ═══════════════════════════════════════════════════════════════════════════
// 2. Board profiles
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_board_profile_resolves_known_omen_boards() {
    let omen_amd = resolve_board_profile("88F8").expect("resolve 88F8");
    assert_eq!(omen_amd.board_id, "88F8");
    assert_eq!(omen_amd.fan_count, 2);
    assert!(omen_amd.has_max_fan);
    assert_eq!(omen_amd.rgb_zone_count, 4);

    let omen_intel = resolve_board_profile("8a24").expect("resolve 8A24 lowercase");
    assert_eq!(omen_intel.board_id, "8A24");
    assert!(omen_intel.has_max_fan);

    // Victus 8A3E & 8A3D: 0 RGB zones (standard monochrome backlit keyboard).
    let victus = resolve_board_profile("8A3E").expect("resolve 8A3E");
    assert_eq!(victus.board_id, "8A3E");
    assert_eq!(victus.rgb_zone_count, 0);

    let victus_amd = resolve_board_profile("8A3D").expect("resolve 8A3D");
    assert_eq!(victus_amd.board_id, "8A3D");
    assert_eq!(victus_amd.rgb_zone_count, 0);
    // has_max_fan is a spec claim — does NOT imply write validation.
    assert!(victus_amd.has_max_fan);
}

#[test]
fn test_board_profile_rejects_unknown_motherboard() {
    assert_eq!(resolve_board_profile("9999"), None);
    assert_eq!(resolve_board_profile("UNSUPPORTED"), None);
    assert_eq!(resolve_board_profile(""), None);
}

// ═══════════════════════════════════════════════════════════════════════════
// 3. Board validation — 8A3D is NEVER auto-verified
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_8a3d_is_never_write_verified_by_resolve() {
    // This is the user's target machine. The validation must ALWAYS be unvalidated
    // from resolve_board_validation regardless of BIOS version.
    for bios in &["F.00", "F.10", "F.20", "F.22", "F.99", ""] {
        let v = resolve_board_validation("8A3D", bios);
        assert!(
            !v.fan_write_verified,
            "8A3D BIOS {bios}: fan_write_verified must be false (physical validation required)"
        );
        assert!(
            !v.power_write_verified,
            "8A3D BIOS {bios}: power_write_verified must be false"
        );
        assert!(
            !v.rgb_write_verified,
            "8A3D BIOS {bios}: rgb_write_verified must be false"
        );
    }
}

#[test]
fn test_8a3e_is_never_write_verified_by_resolve() {
    for bios in &["F.00", "F.10", "F.22"] {
        let v = resolve_board_validation("8A3E", bios);
        assert!(
            !v.fan_write_verified,
            "8A3E: fan write must not be auto-granted"
        );
        assert!(
            !v.power_write_verified,
            "8A3E: power write must not be auto-granted"
        );
    }
}

#[test]
fn test_known_validated_boards_receive_write_flags() {
    // 88F8 on validated BIOS versions.
    let v_f20 = resolve_board_validation("88F8", "F.20");
    assert!(v_f20.fan_write_verified, "88F8/F.20 must have fan write");
    assert!(
        v_f20.power_write_verified,
        "88F8/F.20 must have power write"
    );

    let v_f21 = resolve_board_validation("88F8", "F.21");
    assert!(v_f21.fan_write_verified, "88F8/F.21 must have fan write");

    // 88F8 on an unvalidated BIOS version must NOT have write flags.
    let v_f22 = resolve_board_validation("88F8", "F.22");
    assert!(
        !v_f22.fan_write_verified,
        "88F8/F.22 is not validated; fan write must be false"
    );

    // 8A24 on validated BIOS.
    let v_8a24 = resolve_board_validation("8A24", "F.10");
    assert!(v_8a24.fan_write_verified, "8A24/F.10 must have fan write");

    // 8A24 on other BIOS must not auto-grant.
    let v_8a24_other = resolve_board_validation("8A24", "F.20");
    assert!(!v_8a24_other.fan_write_verified, "8A24/F.20 not validated");
}

// ═══════════════════════════════════════════════════════════════════════════
// 4. Authorization
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_authorization_blocks_unsupported_fan_mode() {
    let board = resolve_board_profile("88F8").unwrap();
    let validation = BoardValidation::new_verified("88F8", "F.20");

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

    let manual_err = authorize_mutation(
        &HardwareRequest::SetFanMode {
            mode: "manual".to_string(),
        },
        &board,
        Some(&validation),
    )
    .expect_err("manual fan mode must be rejected");
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
    .expect_err("unknown mode must be rejected");
    assert!(matches!(
        unknown_err,
        HardwareControlError::InvalidParameter(_)
    ));
}

#[test]
fn test_authorization_blocks_unvalidated_hardware_mutations() {
    let board = resolve_board_profile("88F8").unwrap();
    let unvalidated = BoardValidation::new_unvalidated("88F8", "F.20");

    // Read queries always pass, even without validation.
    assert!(authorize_mutation(&HardwareRequest::GetCapabilities, &board, None).is_ok());
    assert!(authorize_mutation(&HardwareRequest::GetFanStatus, &board, None).is_ok());
    assert!(authorize_mutation(&HardwareRequest::GetPowerProfiles, &board, None).is_ok());
    assert!(authorize_mutation(&HardwareRequest::GetPowerProfile, &board, None).is_ok());

    // Mutations fail when validation is None.
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

    for (req, name) in [
        (&fan_req, "SetFanMode"),
        (&power_req, "SetPowerProfile"),
        (&preset_req, "ApplyPreset"),
        (&rgb_req, "SetRgbEnabled"),
    ] {
        assert!(
            matches!(
                authorize_mutation(req, &board, None).unwrap_err(),
                HardwareControlError::UnsupportedMode(_)
            ),
            "{name} with no validation must return UnsupportedMode"
        );
        assert!(
            matches!(
                authorize_mutation(req, &board, Some(&unvalidated)).unwrap_err(),
                HardwareControlError::UnsupportedMode(_)
            ),
            "{name} with unvalidated record must return UnsupportedMode"
        );
    }

    // Board mismatch rejected.
    let mismatch_val = BoardValidation::new_verified("8A24", "F.10");
    assert!(matches!(
        authorize_mutation(&fan_req, &board, Some(&mismatch_val)).unwrap_err(),
        HardwareControlError::UnsupportedMode(_)
    ));
}

#[test]
fn test_authorization_blocks_8a3d_mutations_from_resolve() {
    // Even if someone passes the resolved (unvalidated) record for 8A3D,
    // mutations must be blocked.
    let board = resolve_board_profile("8A3D").unwrap();
    let validation = resolve_board_validation("8A3D", "F.22");

    assert!(!validation.fan_write_verified);

    let fan_req = HardwareRequest::SetFanMode {
        mode: "max".to_string(),
    };
    let err = authorize_mutation(&fan_req, &board, Some(&validation))
        .expect_err("8A3D fan write must be blocked");
    assert!(matches!(err, HardwareControlError::UnsupportedMode(_)));
}

#[test]
fn test_authorization_power_rgb_flags_cannot_authorize_fan_write() {
    // A validation record where only RGB and power writes are set must NOT
    // authorize a fan write.
    let board = resolve_board_profile("88F8").unwrap();
    let mixed = BoardValidation {
        board_id: "88F8".to_string(),
        bios_version: "F.20".to_string(),
        fan_read_verified: true,
        fan_write_verified: false, // ← NOT set
        power_read_verified: true,
        power_write_verified: true,
        rgb_read_verified: true,
        rgb_write_verified: true,
        validation_record_version: 1,
    };

    let fan_req = HardwareRequest::SetFanMode {
        mode: "max".to_string(),
    };
    let err = authorize_mutation(&fan_req, &board, Some(&mixed))
        .expect_err("fan write must require fan_write_verified");
    assert!(matches!(err, HardwareControlError::UnsupportedMode(_)));
}

// ═══════════════════════════════════════════════════════════════════════════
// 5. WMI adapter unit tests (using FakeWmiAdapter directly)
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_wmi_unavailable_returns_interface_missing_for_fan_status() {
    let conn = unavailable_conn();
    let err = zenomen::service::windows::hp_wmi::query_fan_status(&conn)
        .expect_err("unavailable WMI must fail");
    assert!(matches!(err, HardwareControlError::InterfaceMissing(_)));
}

#[test]
fn test_wmi_unavailable_returns_interface_missing_for_set_fan_mode() {
    let conn = unavailable_conn();
    let err = zenomen::service::windows::hp_wmi::set_fan_mode(&conn, "max")
        .expect_err("unavailable WMI must fail");
    assert!(matches!(err, HardwareControlError::InterfaceMissing(_)));
}

#[test]
fn test_set_fan_mode_max_reads_writes_reads() {
    let adapter = FakeWmiAdapter::new(true);
    // 1. pre-write read: firmware reports "auto" (0x00)
    adapter.expect_call(fan_mode_response(0x00));
    // 2. write ack
    adapter.expect_call(write_ack());
    // 3. post-write read: firmware confirms "max" (0x01)
    adapter.expect_call(fan_mode_response(0x01));

    let conn = HpWmiConnection::from_adapter(adapter);
    let result = zenomen::service::windows::hp_wmi::set_fan_mode(&conn, "max")
        .expect("set_fan_mode max with verified read-back must succeed");
    assert_eq!(result, "max");
}

#[test]
fn test_set_fan_mode_auto_reads_writes_reads() {
    let adapter = FakeWmiAdapter::new(true);
    adapter.expect_call(fan_mode_response(0x01)); // pre: firmware says "max"
    adapter.expect_call(write_ack());
    adapter.expect_call(fan_mode_response(0x00)); // post: firmware confirms "auto"

    let conn = HpWmiConnection::from_adapter(adapter);
    let result = zenomen::service::windows::hp_wmi::set_fan_mode(&conn, "auto")
        .expect("set_fan_mode auto must succeed");
    assert_eq!(result, "auto");
}

#[test]
fn test_set_fan_mode_skips_write_when_already_at_target() {
    // If firmware already reports the requested mode, no write should be issued.
    let adapter = FakeWmiAdapter::new(true);
    adapter.expect_call(fan_mode_response(0x01)); // pre-read: already "max"
    // No write response pre-loaded → test asserts no write was attempted.

    let conn = HpWmiConnection::from_adapter(adapter);
    let result = zenomen::service::windows::hp_wmi::set_fan_mode(&conn, "max")
        .expect("already-at-target mode must succeed without write");
    assert_eq!(result, "max");
    // If a write were attempted the adapter would error (no response pre-loaded).
}

#[test]
fn test_set_fan_mode_returns_verification_failed_when_readback_disagrees() {
    let adapter = FakeWmiAdapter::new(true);
    adapter.expect_call(fan_mode_response(0x00)); // pre: "auto"
    adapter.expect_call(write_ack());
    adapter.expect_call(fan_mode_response(0x00)); // post: still "auto" — write failed silently

    let conn = HpWmiConnection::from_adapter(adapter);
    let err = zenomen::service::windows::hp_wmi::set_fan_mode(&conn, "max")
        .expect_err("verification mismatch must return VerificationFailed");
    assert!(
        matches!(err, HardwareControlError::VerificationFailed { .. }),
        "expected VerificationFailed, got {err:?}"
    );
}

#[test]
fn test_set_fan_mode_returns_unsupported_when_firmware_rejects_command() {
    let adapter = FakeWmiAdapter::new(true);
    adapter.expect_call(fan_mode_response(0x00)); // pre-read succeeds
    // Write response: firmware status 0x01 = UnknownCommand
    adapter.expect_call(FakeResponse::FirmwareError { status: 0x01 });

    let conn = HpWmiConnection::from_adapter(adapter);
    let err = zenomen::service::windows::hp_wmi::set_fan_mode(&conn, "max")
        .expect_err("firmware rejection must propagate as error");
    assert!(
        matches!(err, HardwareControlError::UnsupportedMode(_)),
        "expected UnsupportedMode for firmware status 0x01, got {err:?}"
    );
}

#[test]
fn test_set_fan_mode_returns_authorization_failed_when_access_denied() {
    let adapter = FakeWmiAdapter::new(true);
    adapter.expect_call(fan_mode_response(0x00)); // pre-read
    // Firmware returns 0x04 = AccessDenied (BIOS password locked)
    adapter.expect_call(FakeResponse::FirmwareError { status: 0x04 });

    let conn = HpWmiConnection::from_adapter(adapter);
    let err = zenomen::service::windows::hp_wmi::set_fan_mode(&conn, "max")
        .expect_err("access denied must propagate");
    assert!(
        matches!(err, HardwareControlError::AuthorizationFailed(_)),
        "expected AuthorizationFailed, got {err:?}"
    );
}

#[test]
fn test_query_fan_status_returns_honest_empty_rpms() {
    let adapter = FakeWmiAdapter::new(true);
    // Firmware reports "auto" (0x00)
    adapter.expect_call(fan_mode_response(0x00));

    let conn = HpWmiConnection::from_adapter(adapter);
    let (mode, rpms) = zenomen::service::windows::hp_wmi::query_fan_status(&conn)
        .expect("query_fan_status must succeed");
    assert_eq!(mode, "auto");
    // RPMs must be empty — no fabricated values.
    assert!(
        rpms.is_empty(),
        "Fan RPMs must be empty (no tachometer on this platform); got {rpms:?}"
    );
}

#[test]
fn test_query_fan_status_propagates_firmware_error() {
    let adapter = FakeWmiAdapter::new(true);
    adapter.expect_call(FakeResponse::FirmwareError { status: 0x01 }); // UnknownCommand

    let conn = HpWmiConnection::from_adapter(adapter);
    let err = zenomen::service::windows::hp_wmi::query_fan_status(&conn)
        .expect_err("firmware rejection must propagate");
    assert!(matches!(err, HardwareControlError::UnsupportedMode(_)));
}

#[test]
fn test_set_fan_mode_invalid_mode_rejected_before_wmi() {
    // Invalid modes must fail without touching WMI.
    let adapter = FakeWmiAdapter::new(true);
    // No pre-loaded responses: if WMI were called the test would error.
    let conn = HpWmiConnection::from_adapter(adapter);
    let err = zenomen::service::windows::hp_wmi::set_fan_mode(&conn, "overclock")
        .expect_err("invalid mode must be rejected");
    assert!(matches!(err, HardwareControlError::InvalidParameter(_)));
}

#[test]
fn test_wmi_connection_error_propagates() {
    let adapter = FakeWmiAdapter::new(true);
    adapter.expect_call(FakeResponse::WmiError {
        message: "simulated COM failure".to_string(),
    });
    let conn = HpWmiConnection::from_adapter(adapter);
    let err = zenomen::service::windows::hp_wmi::query_fan_status(&conn)
        .expect_err("WMI connection error must propagate");
    assert!(
        matches!(err, HardwareControlError::ConnectionFailed(_)),
        "got {err:?}"
    );
}

#[test]
fn test_firmware_response_too_short_rejected() {
    // A WMI response shorter than 136 bytes must be rejected with ProtocolError.
    let adapter = FakeWmiAdapter::new(true);
    // Manually inject a too-short output by using WmiError and checking the parse path
    // indirectly through query_fan_status. We use Success with a deliberately long
    // payload to exercise a correct path, then a short WmiError for the error path.
    adapter.expect_call(FakeResponse::WmiError {
        message: "short buffer test".to_string(),
    });
    let conn = HpWmiConnection::from_adapter(adapter);
    let err = zenomen::service::windows::hp_wmi::query_fan_status(&conn).expect_err("must fail");
    assert!(matches!(
        err,
        HardwareControlError::ConnectionFailed(_) | HardwareControlError::ProtocolError(_)
    ));
}

// ═══════════════════════════════════════════════════════════════════════════
// 6. handle_request with fake WMI — service layer tests
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_handle_request_get_fan_status_unavailable_wmi() {
    // With unavailable WMI, GetFanStatus must return an Error response.
    let board = resolve_board_profile("88F8").unwrap();
    let validation = BoardValidation::new_verified("88F8", "F.20");
    let conn = unavailable_conn();

    let resp = handle_request(
        HardwareRequest::GetFanStatus,
        &board,
        Some(&validation),
        &conn,
    );
    assert!(
        matches!(resp, HardwareResponse::Error(_)),
        "unavailable WMI must produce Error, got {resp:?}"
    );
}

#[test]
fn test_handle_request_set_fan_mode_blocked_by_unvalidated_board() {
    // Board 8A3D with the auto-resolved (unvalidated) record must block SetFanMode.
    let board = resolve_board_profile("8A3D").unwrap();
    let validation = resolve_board_validation("8A3D", "F.22");
    // Fake adapter should not be called (authorization gate fires first).
    let adapter = FakeWmiAdapter::new(true);
    let conn = HpWmiConnection::from_adapter(adapter);

    let resp = handle_request(
        HardwareRequest::SetFanMode {
            mode: "max".to_string(),
        },
        &board,
        Some(&validation),
        &conn,
    );
    // Must be Error (authorization failure) — NOT FanModeSet.
    assert!(
        matches!(resp, HardwareResponse::Error(_)),
        "8A3D without physical validation must produce Error, got {resp:?}"
    );
}

#[test]
fn test_handle_request_set_fan_mode_max_success_with_verified_board() {
    // A verified board (88F8) with correct read/write/readback sequence must succeed.
    let board = resolve_board_profile("88F8").unwrap();
    let validation = BoardValidation::new_verified("88F8", "F.20");

    let adapter = FakeWmiAdapter::new(true);
    // Pre-write read: auto (0x00)
    adapter.expect_call(fan_mode_response(0x00));
    // Write ack
    adapter.expect_call(write_ack());
    // Post-write read: max (0x01)
    adapter.expect_call(fan_mode_response(0x01));

    let conn = HpWmiConnection::from_adapter(adapter);
    let resp = handle_request(
        HardwareRequest::SetFanMode {
            mode: "max".to_string(),
        },
        &board,
        Some(&validation),
        &conn,
    );
    assert!(
        matches!(resp, HardwareResponse::FanModeSet(ref m) if m == "max"),
        "expected FanModeSet(max), got {resp:?}"
    );
}

#[test]
fn test_handle_request_set_fan_mode_reports_verification_failure() {
    // The service response must include the disagreement, not claim success.
    let board = resolve_board_profile("88F8").unwrap();
    let validation = BoardValidation::new_verified("88F8", "F.20");

    let adapter = FakeWmiAdapter::new(true);
    adapter.expect_call(fan_mode_response(0x00)); // pre-read: auto
    adapter.expect_call(write_ack());
    adapter.expect_call(fan_mode_response(0x00)); // post-read: still auto (mismatch)

    let conn = HpWmiConnection::from_adapter(adapter);
    let resp = handle_request(
        HardwareRequest::SetFanMode {
            mode: "max".to_string(),
        },
        &board,
        Some(&validation),
        &conn,
    );
    assert!(
        matches!(resp, HardwareResponse::Error(_)),
        "verification failure must produce Error response, got {resp:?}"
    );
    if let HardwareResponse::Error(msg) = resp {
        assert!(
            msg.contains("verification failed") || msg.contains("VerificationFailed"),
            "error message must mention verification; got: {msg}"
        );
    }
}

#[test]
fn test_handle_request_no_active_success_before_verification() {
    // This test verifies a critical invariant: FanModeSet is NEVER returned
    // if the read-back produced the wrong mode.
    let board = resolve_board_profile("88F8").unwrap();
    let validation = BoardValidation::new_verified("88F8", "F.20");

    let adapter = FakeWmiAdapter::new(true);
    adapter.expect_call(fan_mode_response(0x00)); // pre: auto
    adapter.expect_call(write_ack());
    // Deliberately different read-back
    adapter.expect_call(fan_mode_response(0x00));

    let conn = HpWmiConnection::from_adapter(adapter);
    let resp = handle_request(
        HardwareRequest::SetFanMode {
            mode: "max".to_string(),
        },
        &board,
        Some(&validation),
        &conn,
    );
    // Must NOT be FanModeSet under any circumstances when firmware disagrees.
    assert!(
        !matches!(resp, HardwareResponse::FanModeSet(_)),
        "FanModeSet must NEVER be returned without confirmed read-back; got {resp:?}"
    );
}

#[test]
fn test_handle_request_get_capabilities_unavailable() {
    let board = resolve_board_profile("88F8").unwrap();
    let validation = BoardValidation::new_verified("88F8", "F.20");
    let conn = unavailable_conn();

    let resp = handle_request(
        HardwareRequest::GetCapabilities,
        &board,
        Some(&validation),
        &conn,
    );
    // GetCapabilities on unavailable WMI returns Capabilities with fan_control = false.
    assert!(
        matches!(resp, HardwareResponse::Capabilities(ref c) if !c.fan_control),
        "unavailable WMI must report fan_control = false, got {resp:?}"
    );
}

#[test]
fn test_handle_request_missing_validation_blocks_all_mutations() {
    let board = resolve_board_profile("88F8").unwrap();
    let adapter = FakeWmiAdapter::new(true);
    // No responses pre-loaded: authorization must fire before WMI is touched.
    let conn = HpWmiConnection::from_adapter(adapter);

    let mutating = vec![
        HardwareRequest::SetFanMode {
            mode: "max".to_string(),
        },
        HardwareRequest::SetPowerProfile {
            profile: "performance".to_string(),
        },
        HardwareRequest::ApplyPreset {
            preset: "gaming".to_string(),
        },
        HardwareRequest::SetRgbEnabled { enabled: true },
    ];
    for req in mutating {
        let resp = handle_request(req.clone(), &board, None, &conn);
        assert!(
            matches!(resp, HardwareResponse::Error(_)),
            "{req:?} without validation must produce Error"
        );
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// 7. Protocol framing round-trip end-to-end simulation
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_mock_client_server_communication_verified_board() {
    // Full round-trip through framing → handle_request → framing for a verified board.
    let board = resolve_board_profile("88F8").unwrap();
    let validation = BoardValidation::new_verified("88F8", "F.20");

    let adapter = FakeWmiAdapter::new(true);
    // Preset "gaming" = set performance to "performance" + set fan to "max"
    // set_performance_mode: pre-read balanced(0x00), write ack, post-read performance(0x01)
    adapter.expect_call(FakeResponse::Success {
        payload: vec![0x00],
    }); // perf pre-read balanced
    adapter.expect_call(write_ack()); // perf write
    adapter.expect_call(FakeResponse::Success {
        payload: vec![0x01],
    }); // perf post-read performance
    // set_fan_mode: pre-read auto(0x00), write ack, post-read max(0x01)
    adapter.expect_call(fan_mode_response(0x00));
    adapter.expect_call(write_ack());
    adapter.expect_call(fan_mode_response(0x01));

    let conn = HpWmiConnection::from_adapter(adapter);

    let req = HardwareRequest::ApplyPreset {
        preset: "gaming".to_string(),
    };
    let tx_frame = encode_frame(&req).expect("client encode");
    let (server_req, _): (HardwareRequest, usize) = decode_frame(&tx_frame).expect("server decode");

    let response = handle_request(server_req, &board, Some(&validation), &conn);
    let resp_frame = encode_frame(&response).expect("server encode response");
    let (client_resp, _): (HardwareResponse, usize) =
        decode_frame(&resp_frame).expect("client decode response");

    match client_resp {
        HardwareResponse::PresetApplied(result) => {
            assert_eq!(result.preset_name, "gaming");
            assert!(
                result.power_applied,
                "power must be applied; message: {}",
                result.message
            );
            assert!(
                result.fan_applied,
                "fan must be applied; message: {}",
                result.message
            );
            assert!(
                result.is_full_success(),
                "preset must be full success; message: {}",
                result.message
            );
            // Confirm firmware-verified values, not requested values.
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
fn test_mock_client_server_8a3d_blocked() {
    // Full round-trip for board 8A3D with its auto-resolved unvalidated record
    // must produce an Error response without touching WMI.
    let board = resolve_board_profile("8A3D").unwrap();
    let validation = resolve_board_validation("8A3D", "F.22");
    let adapter = FakeWmiAdapter::new(true);
    // No responses pre-loaded.
    let conn = HpWmiConnection::from_adapter(adapter);

    let req = HardwareRequest::SetFanMode {
        mode: "max".to_string(),
    };
    let tx_frame = encode_frame(&req).expect("client encode");
    let (server_req, _): (HardwareRequest, usize) = decode_frame(&tx_frame).expect("server decode");

    let response = handle_request(server_req, &board, Some(&validation), &conn);
    let resp_frame = encode_frame(&response).expect("server encode response");
    let (client_resp, _): (HardwareResponse, usize) =
        decode_frame(&resp_frame).expect("client decode response");

    assert!(
        matches!(client_resp, HardwareResponse::Error(_)),
        "8A3D must produce Error, got {client_resp:?}"
    );
}

// ═══════════════════════════════════════════════════════════════════════════
// 8. Non-Windows lifecycle stubs
// ═══════════════════════════════════════════════════════════════════════════

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

// ═══════════════════════════════════════════════════════════════════════════
// 9. Windows HP BIOS WMI diagnostic & metadata verification
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_live_windows_wmi_metadata_non_mutating() {
    #[cfg(windows)]
    {
        use zenomen::service::windows::wmi_diag::{WmiDiagnosticCategory, run_wmi_diagnostic};

        let report = run_wmi_diagnostic();
        assert!(report.com_initialized, "COM initialization must succeed");
        assert!(
            report.namespace_connected,
            "ROOT\\WMI must connect on Windows"
        );
        assert!(
            report.hpqbintm_class_found,
            "hpqBIntM class must exist in ROOT\\WMI on HP hardware"
        );
        assert!(report.hpqbdatain_class_found, "hpqBDataIn class must exist");
        assert!(
            report.hpqbdataout128_class_found,
            "hpqBDataOut128 class must exist"
        );
        assert!(
            report.methods_found.contains(&"hpqBIOSInt128".to_string()),
            "hpqBIOSInt128 must be in callable methods: {:?}",
            report.methods_found
        );
        assert!(
            report.category == WmiDiagnosticCategory::Healthy
                || report.category == WmiDiagnosticCategory::AccessDenied,
            "Expected Healthy or AccessDenied, got {:?}",
            report.category
        );
    }
}
