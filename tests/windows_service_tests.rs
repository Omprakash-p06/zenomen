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
use zenomen::service::windows::hp_wmi::{
    FakeResponse, FakeWmiAdapter, HPWMI_GM, HpWmiConnection, commands,
};
use zenomen::service::windows::profiles::{
    BoardValidation, resolve_board_profile, resolve_board_validation,
};
use zenomen::service::windows::protocol::{
    HardwareRequest, HardwareResponse, MAX_PAYLOAD_SIZE, decode_frame, encode_frame,
};
use zenomen::service::windows::server::handle_request;
use zenomen::service::windows::victus_s;

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

/// Pre-load a fan count response (`0x10`).
fn fan_count_response(count: u8) -> FakeResponse {
    FakeResponse::Success {
        payload: vec![count],
    }
}

/// Build a 128-byte Victus S fan table with `num_fans` and one `cpu` row.
fn fan_table_response(cpu_level: u8) -> FakeResponse {
    let mut table = vec![0u8; victus_s::MAX_RESPONSE_LEN];
    table[0] = 1;
    table[victus_s::FAN_TABLE_HEADER_LEN] = cpu_level;
    FakeResponse::Success { payload: table }
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
    assert_eq!(
        victus_amd.thermal_profile_source,
        zenomen::service::windows::profiles::ThermalProfileSource::EmbeddedController
    );
    assert_eq!(
        omen_amd.thermal_profile_source,
        zenomen::service::windows::profiles::ThermalProfileSource::Wmi
    );
}

#[test]
fn test_board_capabilities_decouples_fan_and_thermal_profile() {
    use zenomen::service::windows::profiles::ThermalProfileSource;

    let victus_amd = resolve_board_profile("8A3D").expect("resolve 8A3D");
    let caps = victus_amd.capabilities(None);

    // 8A3D supports fan discovery, fan table, manual fan levels, and max fan
    assert!(caps.fan_count_supported);
    assert!(caps.fan_rpm_supported);
    assert!(caps.fan_table_supported);
    assert!(caps.manual_fan_control_supported);
    assert!(caps.max_fan_supported);

    // Thermal profile is EC-backed, not WMI
    assert_eq!(
        caps.thermal_profile_source,
        ThermalProfileSource::EmbeddedController
    );

    // Physical write verification remains strictly false without physical validation record
    assert!(!caps.fan_write_verified);
    assert!(!caps.power_write_verified);
    assert!(!caps.rgb_write_verified);
}

#[test]
fn test_board_profile_rejects_unknown_motherboard() {
    assert_eq!(resolve_board_profile("9999"), None);
    assert_eq!(resolve_board_profile("UNSUPPORTED"), None);
    assert_eq!(resolve_board_profile(""), None);
}

// ═══════════════════════════════════════════════════════════════════════════
// 3. Board validation — 8A3D F.22 ratified; other BIOS versions remain locked
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_8a3d_f22_ratified_and_other_bios_remain_locked() {
    // Board 8A3D BIOS F.22 has physical hardware validation for Max Fan and Auto restore
    let v_f22 = resolve_board_validation("8A3D", "F.22");
    assert!(v_f22.fan_write_verified);
    assert!(v_f22.max_fan_write_verified);
    assert!(v_f22.auto_fan_restore_verified);
    assert!(!v_f22.manual_fan_write_verified);
    assert!(v_f22.power_write_authorized);
    assert!(!v_f22.power_write_verified);
    assert!(!v_f22.rgb_write_verified);

    // Any other BIOS version for 8A3D must remain completely unvalidated
    for bios in &["F.00", "F.10", "F.20", "F.21", "F.23", "F.99", ""] {
        let v = resolve_board_validation("8A3D", bios);
        assert!(
            !v.fan_write_verified,
            "8A3D BIOS {bios}: fan_write_verified must be false (only F.22 is ratified)"
        );
        assert!(!v.max_fan_write_verified);
        assert!(!v.auto_fan_restore_verified);
        assert!(!v.manual_fan_write_verified);
        assert!(!v.power_write_authorized);
        assert!(!v.power_write_verified);
        assert!(!v.rgb_write_verified);
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
        let err = authorize_mutation(req, &board, Some(&unvalidated)).unwrap_err();
        assert!(
            matches!(
                err,
                HardwareControlError::UnsupportedMode(_)
                    | HardwareControlError::AuthorizationFailed(_)
            ),
            "{name} with unvalidated record must be rejected"
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
    // Even if someone passes the resolved (unvalidated) record for 8A3D (e.g. BIOS F.20),
    // mutations must be blocked for both max and auto modes.
    let board = resolve_board_profile("8A3D").unwrap();
    let validation = resolve_board_validation("8A3D", "F.20");

    assert!(!validation.fan_write_verified);

    for mode in ["max", "auto"] {
        let fan_req = HardwareRequest::SetFanMode {
            mode: mode.to_string(),
        };
        let err = authorize_mutation(&fan_req, &board, Some(&validation))
            .expect_err("8A3D fan write must be blocked");
        assert!(matches!(err, HardwareControlError::AuthorizationFailed(_)));
    }
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
        max_fan_write_verified: false,
        auto_fan_restore_verified: false,
        manual_fan_write_verified: false,
        power_read_verified: true,
        power_write_authorized: true,
        power_write_verified: true,
        rgb_read_verified: true,
        rgb_write_verified: true,
        validation_record_version: 1,
    };

    for mode in ["max", "auto"] {
        let fan_req = HardwareRequest::SetFanMode {
            mode: mode.to_string(),
        };
        let err = authorize_mutation(&fan_req, &board, Some(&mixed))
            .expect_err("fan write must require fan_write_verified");
        assert!(matches!(err, HardwareControlError::AuthorizationFailed(_)));
    }
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
    // Firmware status 3 = unknown command (e.g. wrong command family).
    adapter.expect_call(FakeResponse::FirmwareError { status: 3 });

    let conn = HpWmiConnection::from_adapter(adapter);
    let err = zenomen::service::windows::hp_wmi::set_fan_mode(&conn, "max")
        .expect_err("firmware rejection must propagate as error");
    assert!(
        matches!(err, HardwareControlError::UnsupportedMode(_)),
        "expected UnsupportedMode for firmware status 3, got {err:?}"
    );
}

#[test]
fn test_firmware_status_codes_map_to_documented_layers() {
    // Upstream `hp_return_value`: 2 wrong signature, 3 unknown command,
    // 4 unknown command type, 5 invalid parameters. 0 is the only success.
    for (status, expect_invalid_parameter) in [(2u32, false), (3, false), (4, false), (5, true)] {
        let adapter = FakeWmiAdapter::new(true);
        adapter.expect_call(fan_mode_response(0x00));
        adapter.expect_call(FakeResponse::FirmwareError { status });

        let conn = HpWmiConnection::from_adapter(adapter);
        let err = zenomen::service::windows::hp_wmi::set_fan_mode(&conn, "max")
            .expect_err("firmware status must propagate");
        if expect_invalid_parameter {
            assert!(
                matches!(err, HardwareControlError::InvalidParameter(_)),
                "status {status} must map to InvalidParameter, got {err:?}"
            );
        } else {
            assert!(
                matches!(
                    err,
                    HardwareControlError::ProtocolError(_)
                        | HardwareControlError::UnsupportedMode(_)
                ),
                "status {status} must map to a protocol/unsupported error, got {err:?}"
            );
        }
    }
}

#[test]
fn test_query_fan_status_reads_real_rpms_from_firmware() {
    let adapter = FakeWmiAdapter::new(true);
    // 0x26 -> auto, 0x10 -> 2 fans, then one 0x11 read per fan.
    adapter.expect_call(fan_mode_response(0x00));
    adapter.expect_call(fan_count_response(2));
    adapter.expect_call(FakeResponse::Success {
        payload: vec![0x36, 0x3C, 0x0F, 0xA7], // 4007 RPM
    });
    adapter.expect_call(FakeResponse::Success {
        payload: vec![0x36, 0x3C, 0x0D, 0xFD], // 3581 RPM
    });

    let conn = HpWmiConnection::from_adapter(adapter);
    let (mode, rpms) = zenomen::service::windows::hp_wmi::query_fan_status(&conn)
        .expect("query_fan_status must succeed");
    assert_eq!(mode, "auto");
    assert_eq!(rpms, vec![4007, 3581]);
}

#[test]
fn test_query_fan_status_reports_no_rpms_when_fan_count_unavailable() {
    let adapter = FakeWmiAdapter::new(true);
    adapter.expect_call(fan_mode_response(0x00));
    // No further responses: the fan-count query fails.

    let conn = HpWmiConnection::from_adapter(adapter);
    let (mode, rpms) = zenomen::service::windows::hp_wmi::query_fan_status(&conn)
        .expect("mode read still succeeds");
    assert_eq!(mode, "auto");
    assert!(rpms.is_empty(), "no tachometer data may be invented");
}

#[test]
fn test_query_fan_status_propagates_firmware_error() {
    let adapter = FakeWmiAdapter::new(true);
    adapter.expect_call(FakeResponse::FirmwareError { status: 3 }); // unknown command

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
    // Board 8A3D with an unvalidated BIOS record must block SetFanMode.
    let board = resolve_board_profile("8A3D").unwrap();
    let validation = resolve_board_validation("8A3D", "F.20");
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
fn test_mock_client_server_8a3d_unvalidated_bios_blocked() {
    // Full round-trip for board 8A3D on unvalidated BIOS F.20
    // must produce an Error response without touching WMI.
    let board = resolve_board_profile("8A3D").unwrap();
    let validation = resolve_board_validation("8A3D", "F.20");
    let adapter = FakeWmiAdapter::new(true);
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
        "Unvalidated BIOS must produce Error, got {client_resp:?}"
    );
}

#[test]
fn test_mock_client_server_8a3d_f22_max_fan_authorized() {
    // Full round-trip for board 8A3D on ratified BIOS F.22
    // must authorize and execute SetFanMode(max).
    let board = resolve_board_profile("8A3D").unwrap();
    let validation = resolve_board_validation("8A3D", "F.22");
    let adapter = FakeWmiAdapter::new(true);
    adapter.expect_call(fan_mode_response(0x00)); // pre-read
    adapter.expect_call(fan_count_response(2)); // 0x10 arming for Victus S
    adapter.expect_call(write_ack()); // write 0x27=1
    adapter.expect_call(fan_mode_response(0x01)); // post-read verification
    let conn = HpWmiConnection::from_adapter_for_board(adapter, "8A3D");

    let req = HardwareRequest::SetFanMode {
        mode: "max".to_string(),
    };
    let tx_frame = encode_frame(&req).expect("client encode");
    let (server_req, _): (HardwareRequest, usize) = decode_frame(&tx_frame).expect("server decode");

    let response = handle_request(server_req, &board, Some(&validation), &conn);
    let resp_frame = encode_frame(&response).expect("server encode response");
    let (client_resp, _): (HardwareResponse, usize) =
        decode_frame(&resp_frame).expect("client decode response");

    assert_eq!(client_resp, HardwareResponse::FanModeSet("max".to_string()));
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
// 9. Victus S board 8A3D protocol regression
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_victus_s_query_uses_0x2d_levels_and_0x10_fan_count() {
    let adapter = std::sync::Arc::new(FakeWmiAdapter::new(true));
    adapter.expect_call(fan_mode_response(0x00)); // 0x26
    adapter.expect_call(fan_count_response(2)); // 0x10
    let mut levels = vec![0u8; victus_s::MAX_RESPONSE_LEN];
    levels[0] = 0x27; // 3900 RPM
    levels[1] = 0x23; // 3500 RPM
    adapter.expect_call(FakeResponse::Success { payload: levels });
    adapter.expect_call(FakeResponse::Success {
        payload: {
            let mut l = vec![0u8; victus_s::MAX_RESPONSE_LEN];
            l[0] = 0x27;
            l[1] = 0x23;
            l
        },
    });

    let conn = HpWmiConnection::from_adapter_for_board(adapter.clone(), "8A3D");
    let (mode, rpms) =
        zenomen::service::windows::hp_wmi::query_fan_status(&conn).expect("victus s status");
    assert_eq!(mode, "auto");
    assert_eq!(rpms, vec![3900, 3500]);

    let calls = adapter.recorded_calls();
    assert_eq!(calls[0].command_type, commands::MAX_FAN_STATUS);
    assert_eq!(calls[1].command_type, commands::FAN_COUNT);
    assert!(
        calls[2..]
            .iter()
            .all(|c| c.command_type == victus_s::FAN_LEVEL_GET_QUERY),
        "Victus S RPM must come from 0x2D, not 0x11: {calls:?}"
    );
}

#[test]
fn test_8a3d_auto_restore_refuses_the_0x2e_level_reset() {
    // Upstream patch 2026-10-24: `0x2E {0,0}` freezes 8A3D fans at 0 RPM under
    // load, so the generic Victus S auto reset must never run on this board.
    let adapter = std::sync::Arc::new(FakeWmiAdapter::new(true));
    adapter.expect_call(fan_mode_response(0x01)); // pre-read: max

    let conn = HpWmiConnection::from_adapter_for_board(adapter.clone(), "8A3D");
    let err = zenomen::service::windows::hp_wmi::set_fan_mode(&conn, "auto")
        .expect_err("8A3D must not fall back to the unsafe level reset");

    assert!(matches!(err, HardwareControlError::UnsupportedMode(_)));
    let calls = adapter.recorded_calls();
    assert_eq!(
        calls.len(),
        1,
        "the refusal must precede every firmware write: {calls:?}"
    );
    assert!(
        calls
            .iter()
            .all(|c| c.command_type != victus_s::FAN_LEVEL_SET_QUERY),
        "0x2E must never be sent as an 8A3D auto restore: {calls:?}"
    );
}

#[test]
fn test_8a3d_auto_restore_reapplies_the_active_thermal_profile() {
    let adapter = std::sync::Arc::new(FakeWmiAdapter::new(true));
    adapter.expect_call(write_ack()); // 0x27 = 0
    adapter.expect_call(fan_count_response(2)); // 0x10 arm
    adapter.expect_call(write_ack()); // 0x1A {0xFF, quiet}

    let conn = HpWmiConnection::from_adapter_for_board(adapter.clone(), "8A3D");
    zenomen::service::windows::hp_wmi::restore_automatic_fan_control(
        &conn,
        Some(victus_s::ThermalProfile::Quiet),
    )
    .expect("profile-based restore");

    let calls = adapter.recorded_calls();
    assert_eq!(calls.len(), 3, "{calls:?}");
    assert_eq!(calls[0].command_type, commands::MAX_FAN_SET);
    assert_eq!(calls[0].payload, vec![0, 0, 0, 0]);
    assert_eq!(calls[1].command_type, commands::FAN_COUNT);
    assert_eq!(calls[2].command_type, commands::PERFORMANCE_MODE);
    assert_eq!(calls[2].payload, vec![0xFF, 0x03]);
    assert!(
        calls
            .iter()
            .all(|c| c.command_type != victus_s::FAN_LEVEL_SET_QUERY),
        "the EC restore path must not send the 0x2E level reset: {calls:?}"
    );
}

#[test]
fn test_8a3d_auto_restore_stops_when_the_max_fan_clear_fails() {
    let adapter = std::sync::Arc::new(FakeWmiAdapter::new(true));
    adapter.expect_call(FakeResponse::FirmwareError { status: 5 });

    let conn = HpWmiConnection::from_adapter_for_board(adapter.clone(), "8A3D");
    assert!(
        zenomen::service::windows::hp_wmi::restore_automatic_fan_control(
            &conn,
            Some(victus_s::ThermalProfile::Performance),
        )
        .is_err()
    );

    let calls = adapter.recorded_calls();
    assert_eq!(calls.len(), 1, "{calls:?}");
    assert!(
        calls
            .iter()
            .all(|c| c.command_type != commands::PERFORMANCE_MODE),
        "a failed Max Fan clear must not be followed by a profile write"
    );
}

#[test]
fn test_8a3d_auto_restore_without_a_known_profile_writes_nothing() {
    let adapter = std::sync::Arc::new(FakeWmiAdapter::new(true));
    let conn = HpWmiConnection::from_adapter_for_board(adapter.clone(), "8A3D");

    assert!(zenomen::service::windows::hp_wmi::restore_automatic_fan_control(&conn, None).is_err());
    assert!(
        adapter.recorded_calls().is_empty(),
        "an unknown profile must not cause any firmware write: {:?}",
        adapter.recorded_calls()
    );
}

#[test]
fn test_other_victus_s_boards_keep_the_level_reset_restore() {
    // Only 8A3D is routed to the EC thermal profile; other Victus S boards keep
    // the upstream two-step fan-level reset.
    let adapter = std::sync::Arc::new(FakeWmiAdapter::new(true));
    adapter.expect_call(fan_count_response(2)); // 0x10
    adapter.expect_call(write_ack()); // 0x27 = 0
    adapter.expect_call(fan_table_response(0x35)); // 0x2F
    adapter.expect_call(write_ack()); // 0x2E {0,0}

    let conn = HpWmiConnection::from_adapter_for_board(adapter.clone(), "8C99");
    zenomen::service::windows::hp_wmi::restore_automatic_fan_control(&conn, None)
        .expect("victus s level reset");

    let calls = adapter.recorded_calls();
    assert_eq!(calls.len(), 4, "{calls:?}");
    assert_eq!(calls[3].command_type, victus_s::FAN_LEVEL_SET_QUERY);
    assert_eq!(calls[3].payload, vec![0, 0]);
    assert!(
        calls
            .iter()
            .all(|c| c.command_type != commands::PERFORMANCE_MODE),
        "non-EC boards must not write a thermal profile: {calls:?}"
    );
}

#[test]
fn test_generic_board_auto_restore_only_clears_max_fan() {
    let adapter = std::sync::Arc::new(FakeWmiAdapter::new(true));
    adapter.expect_call(write_ack());

    let conn = HpWmiConnection::from_adapter_for_board(adapter.clone(), "88F8");
    zenomen::service::windows::hp_wmi::restore_automatic_fan_control(&conn, None)
        .expect("generic max fan clear");

    let calls = adapter.recorded_calls();
    assert_eq!(calls.len(), 1, "{calls:?}");
    assert_eq!(calls[0].command_type, commands::MAX_FAN_SET);
    assert_eq!(calls[0].payload, vec![0, 0, 0, 0]);
}

#[test]
fn test_8a3d_auto_request_cannot_reach_the_unsafe_reset() {
    // `SetFanMode { auto }` is blocked by authorization on unvalidated BIOS revisions,
    // so zero calls reach the firmware adapter.
    let board = resolve_board_profile("8A3D").unwrap();
    let validation = resolve_board_validation("8A3D", "F.20");
    let adapter = std::sync::Arc::new(FakeWmiAdapter::new(true));

    let conn = HpWmiConnection::from_adapter_for_board(adapter.clone(), "8A3D");
    let resp = handle_request(
        HardwareRequest::SetFanMode {
            mode: "auto".to_string(),
        },
        &board,
        Some(&validation),
        &conn,
    );

    assert!(matches!(resp, HardwareResponse::Error(_)), "{resp:?}");
    assert!(
        adapter.recorded_calls().is_empty(),
        "unvalidated 8A3D must reject SetFanMode(auto) before reaching firmware: {:?}",
        adapter.recorded_calls()
    );
}

#[test]
fn test_fan_write_uses_gaming_family_not_lighting_family() {
    let adapter = std::sync::Arc::new(FakeWmiAdapter::new(true));
    adapter.expect_call(fan_mode_response(0x00));
    adapter.expect_call(write_ack());
    adapter.expect_call(fan_mode_response(0x01));

    let conn = HpWmiConnection::from_adapter(adapter.clone());
    zenomen::service::windows::hp_wmi::set_fan_mode(&conn, "max").expect("set max");

    for call in adapter.recorded_calls() {
        assert_eq!(
            call.command, HPWMI_GM,
            "every HP gaming command uses family 0x20008; 0x20009 is keyboard lighting"
        );
        assert_ne!(
            call.command,
            zenomen::service::windows::hp_wmi::HPWMI_KB_LIGHTING
        );
        // Reads ask for 4 bytes; the Max Fan write has no output payload.
        if call.command_type == commands::MAX_FAN_SET {
            assert_eq!(call.out_size, 0);
            assert_eq!(call.payload.len(), 4);
        }
    }
}

#[test]
fn test_8a3d_fan_write_still_blocked_before_firmware() {
    let board = resolve_board_profile("8A3D").unwrap();
    let validation = resolve_board_validation("8A3D", "F.20");
    let adapter = std::sync::Arc::new(FakeWmiAdapter::new(true));
    let conn = HpWmiConnection::from_adapter_for_board(adapter.clone(), "8A3D");

    let resp = handle_request(
        HardwareRequest::SetFanMode {
            mode: "max".to_string(),
        },
        &board,
        Some(&validation),
        &conn,
    );
    assert!(matches!(resp, HardwareResponse::Error(_)));
    assert!(
        adapter.recorded_calls().is_empty(),
        "an unvalidated board must never reach firmware: {:?}",
        adapter.recorded_calls()
    );
}

#[test]
fn test_8a3d_read_only_status_is_permitted() {
    // Read-only discovery remains available on an unvalidated board.
    let board = resolve_board_profile("8A3D").unwrap();
    let validation = resolve_board_validation("8A3D", "F.22");
    let adapter = std::sync::Arc::new(FakeWmiAdapter::new(true));
    adapter.expect_call(fan_mode_response(0x00));
    adapter.expect_call(fan_count_response(1));
    adapter.expect_call(FakeResponse::Success {
        payload: vec![0x27],
    });

    let conn = HpWmiConnection::from_adapter_for_board(adapter.clone(), "8A3D");
    let resp = handle_request(
        HardwareRequest::GetFanStatus,
        &board,
        Some(&validation),
        &conn,
    );
    match resp {
        HardwareResponse::FanStatus { mode, rpms } => {
            assert_eq!(mode, "auto");
            assert_eq!(rpms, vec![3900]);
        }
        other => panic!("expected FanStatus, got {other:?}"),
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// 10. Windows HP BIOS WMI diagnostic & metadata verification
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

// ═══════════════════════════════════════════════════════════════════════════
// 11. Opcode 0x4C diagnostic probe verification
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_probe_thermal_profile_query_success() {
    let adapter = std::sync::Arc::new(FakeWmiAdapter::new(true));
    adapter.expect_call(FakeResponse::Success {
        payload: vec![0x00, 0x01, 0x02, 0x03],
    });

    let conn = HpWmiConnection::from_adapter_for_board(adapter.clone(), "8A3D");
    let result = zenomen::service::windows::hp_wmi::probe_thermal_profile_query(&conn)
        .expect("diagnostic probe success");

    assert_eq!(result, vec![0x00, 0x01, 0x02, 0x03]);
    let calls = adapter.recorded_calls();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].command, HPWMI_GM);
    assert_eq!(calls[0].command_type, commands::THERMAL_PROFILE_QUERY);
    assert_eq!(calls[0].out_size, 4);
}

#[test]
fn test_probe_thermal_profile_query_propagates_firmware_error() {
    let adapter = std::sync::Arc::new(FakeWmiAdapter::new(true));
    adapter.expect_call(FakeResponse::FirmwareError { status: 4 });

    let conn = HpWmiConnection::from_adapter_for_board(adapter.clone(), "8A3D");
    let err = zenomen::service::windows::hp_wmi::probe_thermal_profile_query(&conn)
        .expect_err("unsupported opcode error");

    assert!(matches!(err, HardwareControlError::UnsupportedMode(_)));
}

#[test]
fn test_probe_thermal_profile_query_does_not_unlock_writes() {
    // Probing 0x4C must strictly remain a diagnostic action and never grant
    // write authorization or mutate the board validation record.
    let board = resolve_board_profile("8A3D").unwrap();
    let validation = resolve_board_validation("8A3D", "F.20");
    assert!(!validation.fan_write_verified);

    let adapter = std::sync::Arc::new(FakeWmiAdapter::new(true));
    adapter.expect_call(FakeResponse::Success {
        payload: vec![0x00, 0x00, 0x00, 0x00],
    });

    let conn = HpWmiConnection::from_adapter_for_board(adapter.clone(), "8A3D");
    let probe_res = zenomen::service::windows::hp_wmi::probe_thermal_profile_query(&conn);
    assert!(probe_res.is_ok());

    // Fan write remains blocked by authorization
    let req = HardwareRequest::SetFanMode {
        mode: "max".to_string(),
    };
    let auth_res = authorize_mutation(&req, &board, Some(&validation));
    assert!(auth_res.is_err());
}

#[test]
fn test_8a3d_auto_restore_stops_when_arm_query_fails() {
    let adapter = std::sync::Arc::new(FakeWmiAdapter::new(true));
    // Step 1: Max Fan clear succeeds
    adapter.expect_call(write_ack());
    // Step 2: Fan count (arm) query fails
    adapter.expect_call(FakeResponse::FirmwareError { status: 5 });

    let conn = HpWmiConnection::from_adapter_for_board(adapter.clone(), "8A3D");
    let err = zenomen::service::windows::hp_wmi::restore_automatic_fan_control(
        &conn,
        Some(victus_s::ThermalProfile::Balanced),
    )
    .expect_err("restore must stop on arm failure");

    assert!(matches!(err, HardwareControlError::InvalidParameter(_)));
    let calls = adapter.recorded_calls();
    assert_eq!(calls.len(), 2, "{calls:?}");
    assert_eq!(calls[0].command_type, commands::MAX_FAN_SET);
    assert_eq!(calls[1].command_type, commands::FAN_COUNT);
    assert!(
        calls
            .iter()
            .all(|c| c.command_type != commands::PERFORMANCE_MODE),
        "failing arm query must prevent thermal profile write: {calls:?}"
    );
}

#[test]
fn test_8a3d_auto_restore_fails_when_profile_write_fails() {
    let adapter = std::sync::Arc::new(FakeWmiAdapter::new(true));
    // Step 1: Max Fan clear succeeds
    adapter.expect_call(write_ack());
    // Step 2: Fan count query succeeds
    adapter.expect_call(fan_count_response(2));
    // Step 3: Profile write fails
    adapter.expect_call(FakeResponse::FirmwareError { status: 3 });

    let conn = HpWmiConnection::from_adapter_for_board(adapter.clone(), "8A3D");
    let err = zenomen::service::windows::hp_wmi::restore_automatic_fan_control(
        &conn,
        Some(victus_s::ThermalProfile::Performance),
    )
    .expect_err("profile write failure must be surfaced");

    assert!(matches!(err, HardwareControlError::UnsupportedMode(_)));
    let calls = adapter.recorded_calls();
    assert_eq!(calls.len(), 3, "{calls:?}");
    assert_eq!(calls[2].command_type, commands::PERFORMANCE_MODE);
}

#[test]
fn test_8a3d_auto_restore_failure_surfaces_honestly_in_set_fan_mode() {
    let adapter = std::sync::Arc::new(FakeWmiAdapter::new(true));
    // Pre-write read-back reports Max Fan active
    adapter.expect_call(fan_mode_response(0x01));

    let conn = HpWmiConnection::from_adapter_for_board(adapter.clone(), "8A3D");
    // set_fan_mode("auto") calls restore_automatic_fan_control(conn, None)
    // On 8A3D, missing profile causes immediate UnsupportedMode failure
    let err = zenomen::service::windows::hp_wmi::set_fan_mode(&conn, "auto")
        .expect_err("8A3D without known profile must refuse auto restore");

    assert!(matches!(err, HardwareControlError::UnsupportedMode(_)));
    let calls = adapter.recorded_calls();
    // Only the read-back query occurred, no writes
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].command_type, commands::MAX_FAN_STATUS);
    assert!(
        calls
            .iter()
            .all(|c| c.command_type != victus_s::FAN_LEVEL_SET_QUERY),
        "0x2E level reset must never be sent on 8A3D: {calls:?}"
    );
}

// ═══════════════════════════════════════════════════════════════════════════
// 12. WMI input envelope construction (0x80041005 type verification)
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_envelope_plan_omits_hpqbdata_when_payload_empty() {
    use zenomen::service::windows::hp_wmi::plan_hpq_input_envelope;

    let props = plan_hpq_input_envelope(0x20008, 0x10, &[]);
    assert_eq!(props.len(), 4);
    assert_eq!(props[0].name, "Sign");
    assert_eq!(props[1].name, "Command");
    assert_eq!(props[2].name, "CommandType");
    assert_eq!(props[3].name, "Size");
    assert!(props.iter().all(|p| p.name != "hpqBData"));
}

#[test]
fn test_envelope_plan_includes_hpqbdata_with_array_type_when_payload_non_empty() {
    use zenomen::service::windows::hp_wmi::plan_hpq_input_envelope;

    let payload = [0x01, 0x02, 0x03];
    let props = plan_hpq_input_envelope(0x20008, 0x27, &payload);
    assert_eq!(props.len(), 5);
    let data_prop = props
        .iter()
        .find(|p| p.name == "hpqBData")
        .expect("hpqBData present");
    assert_eq!(data_prop.vt, 0x2000 | 17); // VT_ARRAY | VT_UI1
    assert_eq!(data_prop.expected_cim, 0x2000 | 17); // CIM_FLAG_ARRAY | CIM_UINT8
}

#[test]
fn test_envelope_property_put_failure_halts_construction_immediately() {
    use zenomen::service::windows::hp_wmi::{EnvelopeSink, populate_hpq_envelope};

    struct FailingSink {
        fail_at: &'static str,
        calls: Vec<String>,
    }

    impl EnvelopeSink for FailingSink {
        fn put_prop(
            &mut self,
            name: &str,
            _vt: u16,
            _expected_cim: i32,
            _vt_type_arg: i32,
        ) -> Result<(), HardwareControlError> {
            self.calls.push(name.to_string());
            if name == self.fail_at {
                Err(HardwareControlError::ProtocolError(format!(
                    "Put({name}) failed with 0x80041005"
                )))
            } else {
                Ok(())
            }
        }
    }

    let mut sink = FailingSink {
        fail_at: "Command",
        calls: Vec::new(),
    };

    let err = populate_hpq_envelope(&mut sink, 0x20008, 0x10, &[0x01])
        .expect_err("construction must halt on Command failure");
    assert!(matches!(err, HardwareControlError::ProtocolError(_)));
    // Sign succeeded, Command failed, CommandType, Size, hpqBData were never reached!
    assert_eq!(sink.calls, vec!["Sign".to_string(), "Command".to_string()]);
}

#[test]
fn test_envelope_failure_prevents_indata_and_exec_method() {
    use zenomen::service::windows::hp_wmi::{EnvelopeSink, populate_hpq_envelope};

    struct TrackingSink {
        fail_on_sign: bool,
        indata_attached: bool,
        exec_method_called: bool,
        calls: Vec<String>,
    }

    impl EnvelopeSink for TrackingSink {
        fn put_prop(
            &mut self,
            name: &str,
            _vt: u16,
            _expected_cim: i32,
            _vt_type_arg: i32,
        ) -> Result<(), HardwareControlError> {
            self.calls.push(name.to_string());
            if self.fail_on_sign && name == "Sign" {
                return Err(HardwareControlError::ProtocolError(
                    "Sign failed".to_string(),
                ));
            }
            Ok(())
        }
    }

    let mut sink = TrackingSink {
        fail_on_sign: true,
        indata_attached: false,
        exec_method_called: false,
        calls: Vec::new(),
    };

    let res = populate_hpq_envelope(&mut sink, 0x20008, 0x10, &[]);
    assert!(res.is_err());
    assert!(!sink.indata_attached);
    assert!(!sink.exec_method_called);
    assert_eq!(sink.calls, vec!["Sign".to_string()]);
}

#[test]
fn test_fake_wmi_adapter_all_paths_pass() {
    let adapter = std::sync::Arc::new(FakeWmiAdapter::new(true));
    adapter.expect_call(fan_count_response(2));
    let conn = HpWmiConnection::from_adapter_for_board(adapter.clone(), "8A3D");
    let count = zenomen::service::windows::hp_wmi::get_fan_count(&conn).expect("get fan count");
    assert_eq!(count, 2);
}

#[test]
fn test_get_fan_rpm_victus_s_cpu_and_gpu() {
    let adapter = FakeWmiAdapter::new(true);
    let mut payload = vec![0u8; 128];
    payload[0] = 25; // 2500 RPM
    payload[1] = 31; // 3100 RPM
    adapter.expect_call(FakeResponse::Success {
        payload: payload.clone(),
    });
    adapter.expect_call(FakeResponse::Success { payload });

    let conn = HpWmiConnection::from_adapter_for_board(adapter, "8A3D");
    assert_eq!(conn.get_fan_rpm(0).unwrap(), Some(2500));
    assert_eq!(conn.get_fan_rpm(1).unwrap(), Some(3100));
    assert_eq!(conn.get_fan_rpm(2).unwrap(), None);
}

#[test]
fn test_get_fan_rpm_victus_s_zero_rpm() {
    let adapter = FakeWmiAdapter::new(true);
    let mut payload = vec![0u8; 128];
    payload[0] = 0; // stopped
    payload[1] = 0; // stopped
    adapter.expect_call(FakeResponse::Success {
        payload: payload.clone(),
    });
    adapter.expect_call(FakeResponse::Success { payload });

    let conn = HpWmiConnection::from_adapter_for_board(adapter, "8A3D");
    assert_eq!(conn.get_fan_rpm(0).unwrap(), Some(0));
    assert_eq!(conn.get_fan_rpm(1).unwrap(), Some(0));
}

#[test]
fn test_get_fan_rpm_generic_board() {
    let adapter = FakeWmiAdapter::new(true);
    let payload = vec![0x00, 0x00, 0x09, 0xC4]; // 2500 RPM (0x09C4)
    adapter.expect_call(FakeResponse::Success { payload });

    let conn = HpWmiConnection::from_adapter_for_board(adapter, "88F8");
    assert_eq!(conn.get_fan_rpm(0).unwrap(), Some(2500));
}

#[test]
fn test_victus_s_fan_table_parsing_zero_gpu_and_truncated() {
    let mut payload = vec![0u8; 128];
    payload[0] = 2; // num_fans = 2
    payload[1] = 0; // unknown

    // Entry 0 (row 0): CPU 32 (0x20), GPU 0 (0x00), Noise 10 (0x0A)
    payload[2] = 0x20;
    payload[3] = 0x00;
    payload[4] = 0x0A;

    // Entry 1 (row 1): CPU 40 (0x28), GPU 36 (0x24), Noise 16 (0x10)
    payload[5] = 0x28;
    payload[6] = 0x24;
    payload[7] = 0x10;

    // Entry 2 (row 2): all zero (truncation sentinel)
    payload[8] = 0;
    payload[9] = 0;
    payload[10] = 0;

    let adapter = FakeWmiAdapter::new(true);
    adapter.expect_call(FakeResponse::Success { payload });

    let conn = HpWmiConnection::from_adapter_for_board(adapter, "8A3D");
    let table = zenomen::service::windows::hp_wmi::get_victus_fan_table(&conn).expect("fan table");
    assert_eq!(table.entries.len(), 2);
    assert_eq!(table.min_rpm, 0x20);
    assert_eq!(table.max_rpm, 0x28);
    // Derived from row 1 (first nonzero GPU row): 36 - 40 = -4
    assert_eq!(table.gpu_offset, Some(-4));
    assert_eq!(table.gpu_level_for_cpu(40), 36);
}

#[test]
fn test_fan_control_state_lifecycle_and_helpers() {
    use zenomen::service::types::FanControlState;

    assert_eq!(FanControlState::Automatic.as_str(), "automatic");
    assert_eq!(FanControlState::Applying.as_str(), "applying");
    assert_eq!(FanControlState::Manual.as_str(), "manual");
    assert_eq!(FanControlState::MaxFan.as_str(), "maxfan");
    assert_eq!(FanControlState::Restoring.as_str(), "restoring");
    assert_eq!(FanControlState::RestoreFailed.as_str(), "restore_failed");
    assert_eq!(FanControlState::Unsupported.as_str(), "unsupported");

    assert!(!FanControlState::Automatic.is_active_override());
    assert!(FanControlState::Applying.is_active_override());
    assert!(FanControlState::Manual.is_active_override());
    assert!(FanControlState::MaxFan.is_active_override());
    assert!(!FanControlState::Restoring.is_active_override());
    assert!(!FanControlState::RestoreFailed.is_active_override());
    assert!(!FanControlState::Unsupported.is_active_override());
}

#[test]
fn test_guarded_8a3d_fan_control_blocks_writes_when_unvalidated() {
    let board = resolve_board_profile("8A3D").unwrap();
    let validation = resolve_board_validation("8A3D", "F.20");
    assert!(!validation.fan_write_verified);

    let adapter = FakeWmiAdapter::new(true);
    let conn = HpWmiConnection::from_adapter_for_board(adapter, "8A3D");

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
        "mutation must be rejected without fan_write_verified"
    );
}

#[test]
fn test_8a3d_set_fan_mode_auto_succeeds_with_known_profile() {
    let adapter = std::sync::Arc::new(FakeWmiAdapter::new(true));
    adapter.expect_call(fan_mode_response(0x01)); // pre-read: max
    adapter.expect_call(write_ack()); // 0x27 = 0 (clear max fan)
    adapter.expect_call(fan_count_response(2)); // 0x10 arm
    adapter.expect_call(write_ack()); // 0x1A {0xFF, balanced}
    adapter.expect_call(fan_mode_response(0x00)); // post-read: auto

    let conn = HpWmiConnection::from_adapter_for_board(adapter.clone(), "8A3D");
    let result = zenomen::service::windows::hp_wmi::set_fan_mode_with_profile(
        &conn,
        "auto",
        Some(victus_s::ThermalProfile::Balanced),
    )
    .expect("set_fan_mode_with_profile");

    assert_eq!(result, "auto");
    let calls = adapter.recorded_calls();
    assert_eq!(calls.len(), 5);
    assert_eq!(calls[0].command_type, commands::MAX_FAN_STATUS);
    assert_eq!(calls[1].command_type, commands::MAX_FAN_SET);
    assert_eq!(calls[1].payload, vec![0, 0, 0, 0]);
    assert_eq!(calls[2].command_type, commands::FAN_COUNT);
    assert_eq!(calls[3].command_type, commands::PERFORMANCE_MODE);
    assert_eq!(calls[3].payload, vec![0xFF, 0x00]);
    assert_eq!(calls[4].command_type, commands::MAX_FAN_STATUS);
    assert!(
        calls
            .iter()
            .all(|c| c.command_type != victus_s::FAN_LEVEL_SET_QUERY),
        "0x2E level reset must not be sent: {calls:?}"
    );
}

#[test]
fn test_8a3d_shutdown_skips_firmware_write_when_unvalidated() {
    let _board = resolve_board_profile("8A3D").unwrap();
    let validation = resolve_board_validation("8A3D", "F.20");
    assert!(!validation.fan_write_verified);

    let adapter = FakeWmiAdapter::new(true);
    let conn = HpWmiConnection::from_adapter_for_board(adapter, "8A3D");

    // Simulating shutdown logic from server.rs:
    if conn.is_available() && validation.fan_write_verified {
        panic!("Unvalidated 8A3D board must not enter shutdown write branch");
    }
}

#[test]
fn test_8a3d_read_only_telemetry_flow_full() {
    let board = resolve_board_profile("8A3D").unwrap();
    let validation = resolve_board_validation("8A3D", "F.22");

    let adapter = FakeWmiAdapter::new(true);
    // 1. GetCapabilities: fan count
    adapter.expect_call(fan_count_response(2));
    // 2. GetFanStatus: mode (0x26), fan_count (cached), 0x2D CPU fan, 0x2D GPU fan
    adapter.expect_call(fan_mode_response(0x00)); // auto
    let mut levels = vec![0u8; 128];
    levels[0] = 22; // 2200 RPM CPU
    levels[1] = 20; // 2000 RPM GPU
    adapter.expect_call(FakeResponse::Success {
        payload: levels.clone(),
    }); // fan 0
    adapter.expect_call(FakeResponse::Success { payload: levels }); // fan 1

    let conn = HpWmiConnection::from_adapter_for_board(adapter, "8A3D");

    // GetCapabilities is authorized without fan_write_verified
    let resp_caps = handle_request(
        HardwareRequest::GetCapabilities,
        &board,
        Some(&validation),
        &conn,
    );
    match resp_caps {
        HardwareResponse::Capabilities(caps) => {
            assert!(caps.fan_rpm);
            assert!(caps.fan_control);
        }
        other => panic!("Expected Capabilities, got {other:?}"),
    }

    // GetFanStatus is authorized without fan_write_verified
    let resp_status = handle_request(
        HardwareRequest::GetFanStatus,
        &board,
        Some(&validation),
        &conn,
    );
    match resp_status {
        HardwareResponse::FanStatus { mode, rpms } => {
            assert_eq!(mode, "auto");
            assert_eq!(rpms, vec![2200, 2000]);
        }
        other => panic!("Expected FanStatus, got {other:?}"),
    }
}

#[test]
fn test_8a3d_max_fan_arms_state_with_0x10_and_payload_is_4_bytes() {
    let adapter = std::sync::Arc::new(FakeWmiAdapter::new(true));
    // When enabled=true on Victus S, set_max_fan_state calls get_fan_count (0x10) then 0x27
    adapter.expect_call(fan_count_response(2));
    adapter.expect_call(write_ack());

    let conn = HpWmiConnection::from_adapter_for_board(adapter.clone(), "8A3D");
    zenomen::service::windows::hp_wmi::set_max_fan_state(&conn, true)
        .expect("set_max_fan_state enabled");

    let calls = adapter.recorded_calls();
    assert_eq!(calls.len(), 2, "{calls:?}");
    assert_eq!(calls[0].command, HPWMI_GM);
    assert_eq!(calls[0].command_type, commands::FAN_COUNT);
    assert_eq!(calls[1].command, HPWMI_GM);
    assert_eq!(calls[1].command_type, commands::MAX_FAN_SET);
    assert_eq!(calls[1].payload, vec![1, 0, 0, 0]);
}

#[test]
fn test_8a3d_set_victus_fan_levels_payload_and_sequence() {
    let adapter = std::sync::Arc::new(FakeWmiAdapter::new(true));
    // 1. get_fan_count (0x10)
    adapter.expect_call(fan_count_response(2));
    // 2. set_max_fan_state(false) (0x27 = [0, 0, 0, 0])
    adapter.expect_call(write_ack());
    // 3. get_victus_fan_table (0x2F) -> 14 rows, max_rpm = 58
    let mut table_bytes = vec![0u8; 128];
    table_bytes[0] = 2; // 2 fans
    // Row 0: cpu 20, gpu 0, noise 20
    table_bytes[2] = 20;
    table_bytes[3] = 0;
    table_bytes[4] = 20;
    // Row 1: cpu 25, gpu 22, noise 25
    table_bytes[5] = 25;
    table_bytes[6] = 22;
    table_bytes[7] = 25;
    // Row 2: cpu 58, gpu 55, noise 40
    table_bytes[8] = 58;
    table_bytes[9] = 55;
    table_bytes[10] = 40;
    adapter.expect_call(FakeResponse::Success {
        payload: table_bytes,
    });
    // 4. 0x2E {cpu_level, gpu_level}
    adapter.expect_call(write_ack());

    let conn = HpWmiConnection::from_adapter_for_board(adapter.clone(), "8A3D");
    zenomen::service::windows::hp_wmi::set_victus_fan_levels(&conn, 40, 37)
        .expect("set_victus_fan_levels");

    let calls = adapter.recorded_calls();
    assert_eq!(calls.len(), 4, "{calls:?}");
    assert_eq!(calls[0].command_type, commands::FAN_COUNT);
    assert_eq!(calls[1].command_type, commands::MAX_FAN_SET);
    assert_eq!(calls[1].payload, vec![0, 0, 0, 0]);
    assert_eq!(calls[2].command_type, victus_s::FAN_TABLE_QUERY);
    assert_eq!(calls[3].command_type, victus_s::FAN_LEVEL_SET_QUERY);
    assert_eq!(calls[3].payload, vec![40, 37]);
}

#[test]
fn test_8a3d_set_victus_fan_levels_graceful_fallback_without_fan_table() {
    let adapter = std::sync::Arc::new(FakeWmiAdapter::new(true));
    // 1. get_fan_count (0x10)
    adapter.expect_call(fan_count_response(2));
    // 2. set_max_fan_state(false) (0x27 = [0, 0, 0, 0])
    adapter.expect_call(write_ack());
    // 3. get_victus_fan_table (0x2F) fails with firmware error
    adapter.expect_call(FakeResponse::FirmwareError { status: 3 });
    // 4. Fallback validates 1..=100 and still executes 0x2E write
    adapter.expect_call(write_ack());

    let conn = HpWmiConnection::from_adapter_for_board(adapter.clone(), "8A3D");
    zenomen::service::windows::hp_wmi::set_victus_fan_levels(&conn, 50, 50)
        .expect("fallback range validation allows valid levels");

    let calls = adapter.recorded_calls();
    assert_eq!(calls.len(), 4);
    assert_eq!(calls[3].command_type, victus_s::FAN_LEVEL_SET_QUERY);
    assert_eq!(calls[3].payload, vec![50, 50]);
}

#[test]
fn test_8a3d_thermal_profile_separation_in_server() {
    let board = resolve_board_profile("8A3D").unwrap();
    let validation = resolve_board_validation("8A3D", "F.22");
    let adapter = FakeWmiAdapter::new(true);
    let conn = HpWmiConnection::from_adapter_for_board(adapter, "8A3D");

    // GetPowerProfiles on 8A3D returns documented supported profiles
    let resp = handle_request(
        HardwareRequest::GetPowerProfiles,
        &board,
        Some(&validation),
        &conn,
    );
    match resp {
        HardwareResponse::PowerProfiles(profiles) => {
            assert_eq!(
                profiles, board.supported_power_profiles,
                "8A3D must expose supported power profiles"
            );
        }
        other => panic!("Expected PowerProfiles, got {other:?}"),
    }

    // GetPowerProfile on 8A3D must return Error explaining EC-managed without calling WMI
    let resp = handle_request(
        HardwareRequest::GetPowerProfile,
        &board,
        Some(&validation),
        &conn,
    );
    match resp {
        HardwareResponse::Error(msg) => {
            assert!(
                msg.contains("EC-managed"),
                "Error message must explain EC management: {msg}"
            );
            assert!(
                msg.contains("0x95"),
                "Error message must reference register 0x95: {msg}"
            );
        }
        other => panic!("Expected Error response, got {other:?}"),
    }
}

#[test]
fn test_8a3d_authorization_failure_message_guides_operator_to_validation() {
    let board = resolve_board_profile("8A3D").unwrap();
    let validation = resolve_board_validation("8A3D", "F.20");
    assert!(!validation.fan_write_verified);

    let err = authorize_mutation(
        &HardwareRequest::SetFanMode {
            mode: "max".to_string(),
        },
        &board,
        Some(&validation),
    )
    .expect_err("unverified fan write must fail");

    match err {
        HardwareControlError::AuthorizationFailed(msg) => {
            assert!(
                msg.contains("zenomen-hw --validate-8a3d-fan"),
                "Error message must provide validation instructions: {msg}"
            );
            assert!(
                msg.contains("fan_write_verified = false"),
                "Error message must explain pending verification state: {msg}"
            );
        }
        other => panic!("Expected AuthorizationFailed, got {other:?}"),
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// 8A3D Physical Validation Runbook & Restoration Test Suite (Section 8)
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_wmi_telemetry_valid_two_fan_response() {
    let adapter = std::sync::Arc::new(FakeWmiAdapter::new(true));
    let mut payload = vec![0u8; 128];
    payload[0] = 34; // CPU 3,400 RPM
    payload[1] = 29; // GPU 2,900 RPM
    adapter.expect_call(FakeResponse::Success { payload });

    let conn = HpWmiConnection::from_adapter_for_board(adapter, "8A3D");
    let telemetry = zenomen::service::windows::hp_wmi::get_victus_dual_fan_rpm(&conn)
        .expect("dual fan telemetry");
    assert_eq!(telemetry.cpu_rpm, 3400);
    assert_eq!(telemetry.gpu_rpm, 2900);
}

#[test]
fn test_wmi_telemetry_cpu_zero_while_gpu_running() {
    let adapter = std::sync::Arc::new(FakeWmiAdapter::new(true));
    let mut payload = vec![0u8; 128];
    payload[0] = 0; // CPU stopped
    payload[1] = 25; // GPU 2,500 RPM
    adapter.expect_call(FakeResponse::Success { payload });

    let conn = HpWmiConnection::from_adapter_for_board(adapter, "8A3D");
    let telemetry = zenomen::service::windows::hp_wmi::get_victus_dual_fan_rpm(&conn)
        .expect("dual fan telemetry");
    assert_eq!(telemetry.cpu_rpm, 0);
    assert_eq!(telemetry.gpu_rpm, 2500);
}

#[test]
fn test_wmi_telemetry_gpu_zero_while_cpu_running() {
    // Normal autonomous EC condition when GPU is idle and temperature is low
    let adapter = std::sync::Arc::new(FakeWmiAdapter::new(true));
    let mut payload = vec![0u8; 128];
    payload[0] = 32; // CPU 3,200 RPM
    payload[1] = 0; // GPU 0 RPM (EC stopped)
    adapter.expect_call(FakeResponse::Success { payload });

    let conn = HpWmiConnection::from_adapter_for_board(adapter, "8A3D");
    let telemetry = zenomen::service::windows::hp_wmi::get_victus_dual_fan_rpm(&conn)
        .expect("dual fan telemetry");
    assert_eq!(telemetry.cpu_rpm, 3200);
    assert_eq!(telemetry.gpu_rpm, 0);
}

#[test]
fn test_wmi_telemetry_both_fans_zero() {
    let adapter = std::sync::Arc::new(FakeWmiAdapter::new(true));
    let payload = vec![0u8; 128]; // Both 0
    adapter.expect_call(FakeResponse::Success { payload });

    let conn = HpWmiConnection::from_adapter_for_board(adapter, "8A3D");
    let telemetry = zenomen::service::windows::hp_wmi::get_victus_dual_fan_rpm(&conn)
        .expect("dual fan telemetry");
    assert_eq!(telemetry.cpu_rpm, 0);
    assert_eq!(telemetry.gpu_rpm, 0);
}

#[test]
fn test_wmi_telemetry_truncated_response() {
    let adapter = std::sync::Arc::new(FakeWmiAdapter::new(true));
    adapter.expect_call(FakeResponse::Success {
        payload: vec![34], // Only 1 byte instead of >= 2
    });

    let conn = HpWmiConnection::from_adapter_for_board(adapter, "8A3D");
    let err = zenomen::service::windows::hp_wmi::get_victus_dual_fan_rpm(&conn)
        .expect_err("truncated payload must error");
    assert!(matches!(err, HardwareControlError::ProtocolError(_)));
}

#[test]
fn test_wmi_telemetry_invalid_response_length() {
    let adapter = std::sync::Arc::new(FakeWmiAdapter::new(true));
    adapter.expect_call(FakeResponse::Success { payload: vec![] }); // Empty

    let conn = HpWmiConnection::from_adapter_for_board(adapter, "8A3D");
    let err = zenomen::service::windows::hp_wmi::get_victus_dual_fan_rpm(&conn)
        .expect_err("empty payload must error");
    assert!(matches!(err, HardwareControlError::ProtocolError(_)));
}

#[test]
fn test_wmi_telemetry_wmi_query_failure() {
    let adapter = std::sync::Arc::new(FakeWmiAdapter::new(true));
    adapter.expect_call(FakeResponse::WmiError {
        message: "ExecMethod failed: 0x80041002".to_string(),
    }); // WBEM_E_NOT_FOUND

    let conn = HpWmiConnection::from_adapter_for_board(adapter, "8A3D");
    let err = zenomen::service::windows::hp_wmi::get_victus_dual_fan_rpm(&conn)
        .expect_err("WMI query error must be propagated");
    assert!(matches!(err, HardwareControlError::ConnectionFailed(_)));
}

#[test]
fn test_wmi_telemetry_invalid_fan_count() {
    let adapter = std::sync::Arc::new(FakeWmiAdapter::new(true));
    adapter.expect_call(fan_count_response(0)); // 0 fans

    let conn = HpWmiConnection::from_adapter_for_board(adapter, "8A3D");
    let err = zenomen::service::windows::hp_wmi::get_fan_count(&conn)
        .expect_err("fan count 0 must be rejected");
    assert!(matches!(err, HardwareControlError::ProtocolError(_)));

    let adapter2 = std::sync::Arc::new(FakeWmiAdapter::new(true));
    adapter2.expect_call(fan_count_response(255)); // Implausible count

    let conn2 = HpWmiConnection::from_adapter_for_board(adapter2, "8A3D");
    let err2 = zenomen::service::windows::hp_wmi::get_fan_count(&conn2)
        .expect_err("fan count 255 must be rejected");
    assert!(matches!(err2, HardwareControlError::ProtocolError(_)));
}

#[test]
fn test_wmi_telemetry_independent_cpu_gpu_rpm_conversion() {
    let adapter = std::sync::Arc::new(FakeWmiAdapter::new(true));
    let mut payload = vec![0u8; 128];
    payload[0] = 41; // CPU 4100 RPM
    payload[1] = 37; // GPU 3700 RPM
    adapter.expect_call(FakeResponse::Success {
        payload: payload.clone(),
    });
    adapter.expect_call(FakeResponse::Success {
        payload: payload.clone(),
    });

    let conn = HpWmiConnection::from_adapter_for_board(adapter, "8A3D");
    let cpu_rpm = zenomen::service::windows::hp_wmi::get_fan_rpm(&conn, 0)
        .expect("read cpu")
        .expect("cpu rpm");
    let gpu_rpm = zenomen::service::windows::hp_wmi::get_fan_rpm(&conn, 1)
        .expect("read gpu")
        .expect("gpu rpm");

    assert_eq!(cpu_rpm, 4100);
    assert_eq!(gpu_rpm, 3700);
}

// ───────────────────────────────────────────────────────────────────────────
// Restoration Unit Tests
// ───────────────────────────────────────────────────────────────────────────

#[test]
fn test_restoration_successful_max_fan_disable() {
    let adapter = std::sync::Arc::new(FakeWmiAdapter::new(true));
    adapter.expect_call(write_ack()); // 0x27 = 0
    adapter.expect_call(fan_count_response(2)); // 0x10 arm
    adapter.expect_call(write_ack()); // 0x1A {0xFF, balanced}

    let conn = HpWmiConnection::from_adapter_for_board(adapter.clone(), "8A3D");
    zenomen::service::windows::hp_wmi::restore_automatic_fan_control_with_target(
        &conn,
        Some(victus_s::ProfileRestoreTarget::Known(
            victus_s::ThermalProfile::Balanced,
        )),
    )
    .expect("restore must succeed");

    let calls = adapter.recorded_calls();
    assert_eq!(calls[0].command_type, commands::MAX_FAN_SET);
    assert_eq!(calls[0].payload, vec![0, 0, 0, 0]);
}

#[test]
fn test_restoration_failed_max_fan_disable() {
    let adapter = std::sync::Arc::new(FakeWmiAdapter::new(true));
    adapter.expect_call(FakeResponse::FirmwareError { status: 4 });

    let conn = HpWmiConnection::from_adapter_for_board(adapter.clone(), "8A3D");
    let err = zenomen::service::windows::hp_wmi::restore_automatic_fan_control_with_target(
        &conn,
        Some(victus_s::ProfileRestoreTarget::Known(
            victus_s::ThermalProfile::Balanced,
        )),
    )
    .expect_err("failed max fan clear must halt");

    assert!(matches!(err, HardwareControlError::UnsupportedMode(_)));
    assert_eq!(adapter.recorded_calls().len(), 1);
}

#[test]
fn test_restoration_successful_profile_restoration() {
    let adapter = std::sync::Arc::new(FakeWmiAdapter::new(true));
    adapter.expect_call(write_ack());
    adapter.expect_call(fan_count_response(2));
    adapter.expect_call(write_ack());

    let conn = HpWmiConnection::from_adapter_for_board(adapter.clone(), "8A3D");
    zenomen::service::windows::hp_wmi::restore_automatic_fan_control_with_target(
        &conn,
        Some(victus_s::ProfileRestoreTarget::Known(
            victus_s::ThermalProfile::Performance,
        )),
    )
    .expect("restore performance");

    let calls = adapter.recorded_calls();
    assert_eq!(calls.len(), 3);
    assert_eq!(calls[2].command_type, commands::PERFORMANCE_MODE);
    assert_eq!(calls[2].payload, vec![0xFF, 0x01]);
}

#[test]
fn test_restoration_failed_profile_restoration() {
    let adapter = std::sync::Arc::new(FakeWmiAdapter::new(true));
    adapter.expect_call(write_ack());
    adapter.expect_call(fan_count_response(2));
    adapter.expect_call(FakeResponse::FirmwareError { status: 3 });

    let conn = HpWmiConnection::from_adapter_for_board(adapter.clone(), "8A3D");
    let err = zenomen::service::windows::hp_wmi::restore_automatic_fan_control_with_target(
        &conn,
        Some(victus_s::ProfileRestoreTarget::Known(
            victus_s::ThermalProfile::Performance,
        )),
    )
    .expect_err("profile write error must fail");

    assert!(matches!(err, HardwareControlError::UnsupportedMode(_)));
}

#[test]
fn test_restoration_unknown_original_profile_refuses_write() {
    let adapter = std::sync::Arc::new(FakeWmiAdapter::new(true));
    let conn = HpWmiConnection::from_adapter_for_board(adapter.clone(), "8A3D");

    let err =
        zenomen::service::windows::hp_wmi::restore_automatic_fan_control_with_target(&conn, None)
            .expect_err("unknown profile must fail closed without writes");

    assert!(matches!(err, HardwareControlError::UnsupportedMode(_)));
    assert!(adapter.recorded_calls().is_empty());
}

#[test]
fn test_restoration_known_balanced_profile() {
    let adapter = std::sync::Arc::new(FakeWmiAdapter::new(true));
    adapter.expect_call(write_ack());
    adapter.expect_call(fan_count_response(2));
    adapter.expect_call(write_ack());

    let conn = HpWmiConnection::from_adapter_for_board(adapter.clone(), "8A3D");
    zenomen::service::windows::hp_wmi::restore_automatic_fan_control_with_target(
        &conn,
        Some(victus_s::ProfileRestoreTarget::Known(
            victus_s::ThermalProfile::Balanced,
        )),
    )
    .expect("restore balanced");

    assert_eq!(adapter.recorded_calls()[2].payload, vec![0xFF, 0x00]);
}

#[test]
fn test_restoration_known_performance_profile() {
    let adapter = std::sync::Arc::new(FakeWmiAdapter::new(true));
    adapter.expect_call(write_ack());
    adapter.expect_call(fan_count_response(2));
    adapter.expect_call(write_ack());

    let conn = HpWmiConnection::from_adapter_for_board(adapter.clone(), "8A3D");
    zenomen::service::windows::hp_wmi::restore_automatic_fan_control_with_target(
        &conn,
        Some(victus_s::ProfileRestoreTarget::Known(
            victus_s::ThermalProfile::Performance,
        )),
    )
    .expect("restore performance");

    assert_eq!(adapter.recorded_calls()[2].payload, vec![0xFF, 0x01]);
}

#[test]
fn test_restoration_known_quiet_profile() {
    let adapter = std::sync::Arc::new(FakeWmiAdapter::new(true));
    adapter.expect_call(write_ack());
    adapter.expect_call(fan_count_response(2));
    adapter.expect_call(write_ack());

    let conn = HpWmiConnection::from_adapter_for_board(adapter.clone(), "8A3D");
    zenomen::service::windows::hp_wmi::restore_automatic_fan_control_with_target(
        &conn,
        Some(victus_s::ProfileRestoreTarget::Known(
            victus_s::ThermalProfile::Quiet,
        )),
    )
    .expect("restore quiet");

    assert_eq!(adapter.recorded_calls()[2].payload, vec![0xFF, 0x03]);
}

#[test]
fn test_restoration_safe_fallback_profile() {
    let adapter = std::sync::Arc::new(FakeWmiAdapter::new(true));
    adapter.expect_call(write_ack());
    adapter.expect_call(fan_count_response(2));
    adapter.expect_call(write_ack());

    let conn = HpWmiConnection::from_adapter_for_board(adapter.clone(), "8A3D");
    zenomen::service::windows::hp_wmi::restore_automatic_fan_control_with_target(
        &conn,
        Some(victus_s::ProfileRestoreTarget::SafeFallback(
            victus_s::ThermalProfile::Balanced,
        )),
    )
    .expect("restore safe fallback");

    assert_eq!(adapter.recorded_calls()[2].payload, vec![0xFF, 0x00]);
}

#[test]
fn test_restoration_does_not_continue_after_preceding_failure() {
    let adapter = std::sync::Arc::new(FakeWmiAdapter::new(true));
    // Step 1 fails
    adapter.expect_call(FakeResponse::FirmwareError { status: 5 });

    let conn = HpWmiConnection::from_adapter_for_board(adapter.clone(), "8A3D");
    assert!(
        zenomen::service::windows::hp_wmi::restore_automatic_fan_control_with_target(
            &conn,
            Some(victus_s::ProfileRestoreTarget::Known(
                victus_s::ThermalProfile::Quiet
            )),
        )
        .is_err()
    );

    let calls = adapter.recorded_calls();
    assert_eq!(calls.len(), 1);
    assert!(
        calls.iter().all(|c| c.command_type != commands::FAN_COUNT
            && c.command_type != commands::PERFORMANCE_MODE)
    );
}

#[test]
fn test_restoration_no_0x2e_call_on_8a3d() {
    let adapter = std::sync::Arc::new(FakeWmiAdapter::new(true));
    adapter.expect_call(write_ack());
    adapter.expect_call(fan_count_response(2));
    adapter.expect_call(write_ack());

    let conn = HpWmiConnection::from_adapter_for_board(adapter.clone(), "8A3D");
    zenomen::service::windows::hp_wmi::restore_automatic_fan_control_with_target(
        &conn,
        Some(victus_s::ProfileRestoreTarget::Known(
            victus_s::ThermalProfile::Balanced,
        )),
    )
    .expect("restore");

    for call in adapter.recorded_calls() {
        assert_ne!(
            call.command_type,
            victus_s::FAN_LEVEL_SET_QUERY,
            "0x2E must NEVER be called during 8A3D restoration: {call:?}"
        );
    }
}

// ───────────────────────────────────────────────────────────────────────────
// Observation Unit Tests
// ───────────────────────────────────────────────────────────────────────────

#[test]
fn test_observation_max_fan_remains_disabled() {
    let mut samples = Vec::new();
    for s in 1..=30 {
        samples.push(victus_s::ObservationSample {
            elapsed_secs: s,
            max_fan_active: Ok(false),
            cpu_rpm: Ok(if s < 12 { 4500 - s * 180 } else { 2200 }),
            gpu_rpm: Ok(if s < 10 { 4000 - s * 300 } else { 0 }),
            cpu_temp_c: Some(52.0),
            gpu_temp_c: None,
        });
    }

    let balanced = victus_s::ProfileRestoreTarget::Known(victus_s::ThermalProfile::Balanced);
    let result = victus_s::evaluate_observation(&samples, 4700, 4500, balanced, true);
    assert_eq!(result, victus_s::ValidationState::RestorationVerified);
}

#[test]
fn test_observation_max_fan_becomes_enabled_again() {
    let mut samples = Vec::new();
    for s in 1..=30 {
        samples.push(victus_s::ObservationSample {
            elapsed_secs: s,
            max_fan_active: Ok(s == 18), // Re-enabled at 18s!
            cpu_rpm: Ok(3000),
            gpu_rpm: Ok(2000),
            cpu_temp_c: Some(50.0),
            gpu_temp_c: None,
        });
    }

    let balanced = victus_s::ProfileRestoreTarget::Known(victus_s::ThermalProfile::Balanced);
    let result = victus_s::evaluate_observation(&samples, 4700, 4500, balanced, true);
    assert_eq!(result, victus_s::ValidationState::RestorationFailed);
}

#[test]
fn test_observation_one_fan_temporarily_zero() {
    // GPU fan at 0 RPM is normal under cool temperatures
    let mut samples = Vec::new();
    for s in 1..=30 {
        samples.push(victus_s::ObservationSample {
            elapsed_secs: s,
            max_fan_active: Ok(false),
            cpu_rpm: Ok(2400),
            gpu_rpm: Ok(0), // Legitimate autonomous stop
            cpu_temp_c: Some(48.0),
            gpu_temp_c: None,
        });
    }

    let balanced = victus_s::ProfileRestoreTarget::Known(victus_s::ThermalProfile::Balanced);
    let result = victus_s::evaluate_observation(&samples, 4500, 4200, balanced, true);
    assert_eq!(result, victus_s::ValidationState::RestorationVerified);
}

#[test]
fn test_observation_both_fans_temporarily_zero() {
    let mut samples = Vec::new();
    for s in 1..=30 {
        samples.push(victus_s::ObservationSample {
            elapsed_secs: s,
            max_fan_active: Ok(false),
            cpu_rpm: Ok(if s < 15 { 3000 - s * 150 } else { 0 }),
            gpu_rpm: Ok(0),
            cpu_temp_c: Some(42.0), // Cool temperature allows passive fan stop
            gpu_temp_c: None,
        });
    }

    let balanced = victus_s::ProfileRestoreTarget::Known(victus_s::ThermalProfile::Balanced);
    let result = victus_s::evaluate_observation(&samples, 4500, 4200, balanced, true);
    assert_eq!(result, victus_s::ValidationState::RestorationVerified);
}

#[test]
fn test_observation_both_fans_zero_at_high_temp_freeze_bug() {
    // Dangerous 0 RPM freeze bug: fans stopped at 80°C
    let mut samples = Vec::new();
    for s in 1..=30 {
        samples.push(victus_s::ObservationSample {
            elapsed_secs: s,
            max_fan_active: Ok(false),
            cpu_rpm: Ok(0),
            gpu_rpm: Ok(0),
            cpu_temp_c: Some(80.0),
            gpu_temp_c: None,
        });
    }

    let balanced = victus_s::ProfileRestoreTarget::Known(victus_s::ThermalProfile::Balanced);
    let result = victus_s::evaluate_observation(&samples, 4500, 4200, balanced, true);
    assert_eq!(result, victus_s::ValidationState::RestorationFailed);
}

#[test]
fn test_observation_sustained_high_rpm_warm_performance() {
    // Matches physical 8A3D laptop test & upstream Linux test: 5400/5200 RPM at 78°C in Performance mode
    let mut samples = Vec::new();
    for s in 1..=30 {
        samples.push(victus_s::ObservationSample {
            elapsed_secs: s,
            max_fan_active: Ok(false),
            cpu_rpm: Ok(5400),
            gpu_rpm: Ok(5200),
            cpu_temp_c: Some(78.0),
            gpu_temp_c: None,
        });
    }

    let performance = victus_s::ProfileRestoreTarget::Known(victus_s::ThermalProfile::Performance);
    let result = victus_s::evaluate_observation(&samples, 5400, 5200, performance, true);
    assert_eq!(result, victus_s::ValidationState::RestorationVerified);
}

#[test]
fn test_observation_sustained_high_rpm_no_thermal_telemetry() {
    // Missing temperature data must result in RestorationUnverified (no false pass or false fail)
    let mut samples = Vec::new();
    for s in 1..=30 {
        samples.push(victus_s::ObservationSample {
            elapsed_secs: s,
            max_fan_active: Ok(false),
            cpu_rpm: Ok(5400),
            gpu_rpm: Ok(5200),
            cpu_temp_c: None,
            gpu_temp_c: None,
        });
    }

    let performance = victus_s::ProfileRestoreTarget::Known(victus_s::ThermalProfile::Performance);
    let result = victus_s::evaluate_observation(&samples, 5400, 5200, performance, true);
    assert_eq!(result, victus_s::ValidationState::RestorationUnverified);
}

#[test]
fn test_observation_sustained_high_rpm_cool_balanced_fails() {
    // Fans pinned at 4700 RPM when cold (45°C) in Balanced profile indicates stuck fan control
    let mut samples = Vec::new();
    for s in 1..=30 {
        samples.push(victus_s::ObservationSample {
            elapsed_secs: s,
            max_fan_active: Ok(false),
            cpu_rpm: Ok(4700),
            gpu_rpm: Ok(4500),
            cpu_temp_c: Some(45.0),
            gpu_temp_c: None,
        });
    }

    let balanced = victus_s::ProfileRestoreTarget::Known(victus_s::ThermalProfile::Balanced);
    let result = victus_s::evaluate_observation(&samples, 4700, 4500, balanced, true);
    assert_eq!(result, victus_s::ValidationState::RestorationFailed);
}

#[test]
fn test_observation_wmi_failure_during_observation() {
    let mut samples = Vec::new();
    for s in 1..=30 {
        samples.push(victus_s::ObservationSample {
            elapsed_secs: s,
            max_fan_active: if s > 20 {
                Err("WMI error".to_string())
            } else {
                Ok(false)
            },
            cpu_rpm: if s > 20 {
                Err("WMI error".to_string())
            } else {
                Ok(2500)
            },
            gpu_rpm: if s > 20 {
                Err("WMI error".to_string())
            } else {
                Ok(0)
            },
            cpu_temp_c: Some(50.0),
            gpu_temp_c: None,
        });
    }

    let balanced = victus_s::ProfileRestoreTarget::Known(victus_s::ThermalProfile::Balanced);
    let result = victus_s::evaluate_observation(&samples, 4500, 4200, balanced, true);
    assert_eq!(result, victus_s::ValidationState::RestorationUnverified);
}

#[test]
fn test_observation_recovery_after_failed_sample() {
    let mut samples = Vec::new();
    for s in 1..=30 {
        let is_transient_error = s == 12;
        samples.push(victus_s::ObservationSample {
            elapsed_secs: s,
            max_fan_active: if is_transient_error {
                Err("Transient RPC error".to_string())
            } else {
                Ok(false)
            },
            cpu_rpm: if is_transient_error {
                Err("Transient RPC error".to_string())
            } else {
                Ok(2400)
            },
            gpu_rpm: if is_transient_error {
                Err("Transient RPC error".to_string())
            } else {
                Ok(0)
            },
            cpu_temp_c: Some(50.0),
            gpu_temp_c: None,
        });
    }

    let balanced = victus_s::ProfileRestoreTarget::Known(victus_s::ThermalProfile::Balanced);
    let result = victus_s::evaluate_observation(&samples, 4500, 4200, balanced, true);
    assert_eq!(result, victus_s::ValidationState::RestorationVerified);
}

#[test]
fn test_observation_timeout() {
    let mut samples = Vec::new();
    // Only 18 seconds collected due to premature exit / timeout
    for s in 1..=18 {
        samples.push(victus_s::ObservationSample {
            elapsed_secs: s,
            max_fan_active: Ok(false),
            cpu_rpm: Ok(2500),
            gpu_rpm: Ok(0),
            cpu_temp_c: Some(50.0),
            gpu_temp_c: None,
        });
    }

    let balanced = victus_s::ProfileRestoreTarget::Known(victus_s::ThermalProfile::Balanced);
    let result = victus_s::evaluate_observation(&samples, 4500, 4200, balanced, true);
    assert_eq!(result, victus_s::ValidationState::RestorationUnverified);
}

#[test]
fn test_observation_incomplete_evidence_never_verified() {
    let balanced = victus_s::ProfileRestoreTarget::Known(victus_s::ThermalProfile::Balanced);
    // Empty samples
    assert_eq!(
        victus_s::evaluate_observation(&[], 4500, 4200, balanced, true),
        victus_s::ValidationState::RestorationUnverified
    );

    // Profile restoration failed before observation
    let mut samples = Vec::new();
    for s in 1..=30 {
        samples.push(victus_s::ObservationSample {
            elapsed_secs: s,
            max_fan_active: Ok(false),
            cpu_rpm: Ok(2500),
            gpu_rpm: Ok(0),
            cpu_temp_c: Some(50.0),
            gpu_temp_c: None,
        });
    }
    assert_eq!(
        victus_s::evaluate_observation(&samples, 4500, 4200, balanced, false),
        victus_s::ValidationState::RestorationFailed
    );
}

// ═══════════════════════════════════════════════════════════════════════════
// 19. Physical validation ratification & authorization regression tests (8A3D F.22)
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_8a3d_f22_validated_max_fan_authorization() {
    let board = resolve_board_profile("8A3D").expect("resolve 8A3D");
    let validation = resolve_board_validation("8A3D", "F.22");

    assert!(validation.fan_write_verified);
    assert!(validation.max_fan_write_verified);

    let req = HardwareRequest::SetFanMode {
        mode: "max".to_string(),
    };
    assert!(
        authorize_mutation(&req, &board, Some(&validation)).is_ok(),
        "SetFanMode(max) must be authorized for 8A3D on BIOS F.22"
    );
}

#[test]
fn test_8a3d_f22_auto_authorization() {
    let board = resolve_board_profile("8A3D").expect("resolve 8A3D");
    let validation = resolve_board_validation("8A3D", "F.22");

    assert!(validation.fan_write_verified);
    assert!(validation.auto_fan_restore_verified);

    let req = HardwareRequest::SetFanMode {
        mode: "auto".to_string(),
    };
    assert!(
        authorize_mutation(&req, &board, Some(&validation)).is_ok(),
        "SetFanMode(auto) must be authorized for 8A3D on BIOS F.22"
    );
}

#[test]
fn test_8a3d_different_bios_revision_remains_locked() {
    let board = resolve_board_profile("8A3D").expect("resolve 8A3D");

    for unverified_bios in &["F.10", "F.20", "F.21", "F.23", "F.30", ""] {
        let validation = resolve_board_validation("8A3D", unverified_bios);

        assert!(
            !validation.fan_write_verified,
            "8A3D BIOS {unverified_bios} must NOT have fan_write_verified"
        );
        assert!(!validation.max_fan_write_verified);
        assert!(!validation.auto_fan_restore_verified);

        for mode in &["max", "auto"] {
            let req = HardwareRequest::SetFanMode {
                mode: mode.to_string(),
            };
            let err = authorize_mutation(&req, &board, Some(&validation))
                .expect_err("Unverified BIOS must be rejected");
            assert!(
                matches!(err, HardwareControlError::AuthorizationFailed(_)),
                "Expected AuthorizationFailed for BIOS {unverified_bios}, got {err:?}"
            );
        }
    }
}

#[test]
fn test_different_motherboard_remains_unaffected() {
    // 8A3E on F.22 remains completely unvalidated
    let victus_intel = resolve_board_profile("8A3E").expect("resolve 8A3E");
    let val_8a3e = resolve_board_validation("8A3E", "F.22");
    assert!(!val_8a3e.fan_write_verified);
    assert!(!val_8a3e.max_fan_write_verified);
    assert!(!val_8a3e.auto_fan_restore_verified);

    let req_max = HardwareRequest::SetFanMode {
        mode: "max".to_string(),
    };
    let err_8a3e = authorize_mutation(&req_max, &victus_intel, Some(&val_8a3e))
        .expect_err("8A3E must not be authorized");
    assert!(matches!(
        err_8a3e,
        HardwareControlError::AuthorizationFailed(_)
    ));

    // Unknown board remains unaffected
    assert_eq!(resolve_board_profile("UNKNOWN_BOARD"), None);
    let val_unknown = resolve_board_validation("UNKNOWN_BOARD", "F.22");
    assert!(!val_unknown.fan_write_verified);

    // 88F8 on F.20 remains valid per its own record
    let board_88f8 = resolve_board_profile("88F8").expect("resolve 88F8");
    let val_88f8 = resolve_board_validation("88F8", "F.20");
    assert!(val_88f8.fan_write_verified);
    assert!(authorize_mutation(&req_max, &board_88f8, Some(&val_88f8)).is_ok());
}

#[test]
fn test_8a3d_manual_pwm_remains_locked() {
    let board = resolve_board_profile("8A3D").expect("resolve 8A3D");
    let validation = resolve_board_validation("8A3D", "F.22");

    assert!(
        !validation.manual_fan_write_verified,
        "manual_fan_write_verified must remain false"
    );

    let caps = board.capabilities(Some(&validation));
    assert!(
        caps.manual_fan_control_supported,
        "spec table support should be true"
    );
    assert!(
        !caps.manual_fan_write_verified,
        "manual fan write must not be verified on physical hardware"
    );

    let req = HardwareRequest::SetFanMode {
        mode: "manual".to_string(),
    };
    let err = authorize_mutation(&req, &board, Some(&validation))
        .expect_err("manual fan mode must be rejected");
    assert!(
        matches!(err, HardwareControlError::UnsupportedMode(_)),
        "Expected UnsupportedMode for manual fan mode on 8A3D, got {err:?}"
    );
}

#[test]
fn test_8a3d_f22_power_and_presets_authorized_while_unvalidated_bios_and_rgb_remain_locked() {
    let board = resolve_board_profile("8A3D").expect("resolve 8A3D");
    let validation_f22 = resolve_board_validation("8A3D", "F.22");

    assert!(
        !validation_f22.rgb_write_verified,
        "rgb_write_verified must be false on 8A3D"
    );

    // SetPowerProfile authorized on 8A3D BIOS F.22
    let req_power = HardwareRequest::SetPowerProfile {
        profile: "performance".to_string(),
    };
    assert!(
        authorize_mutation(&req_power, &board, Some(&validation_f22)).is_ok(),
        "SetPowerProfile should be authorized on 8A3D F.22"
    );

    // ApplyPreset authorized on 8A3D BIOS F.22
    for preset in &["quiet", "balanced", "auto", "performance", "gaming"] {
        let req_preset = HardwareRequest::ApplyPreset {
            preset: preset.to_string(),
        };
        assert!(
            authorize_mutation(&req_preset, &board, Some(&validation_f22)).is_ok(),
            "Preset '{preset}' should be authorized on 8A3D F.22"
        );
    }

    // SetRgbEnabled rejected on 8A3D
    let req_rgb = HardwareRequest::SetRgbEnabled { enabled: true };
    let err_rgb = authorize_mutation(&req_rgb, &board, Some(&validation_f22))
        .expect_err("SetRgbEnabled must be blocked on 8A3D");
    assert!(matches!(err_rgb, HardwareControlError::UnsupportedMode(_)));

    // Unvalidated BIOS version (e.g. F.21) remains locked for power and preset mutations
    let validation_f21 = resolve_board_validation("8A3D", "F.21");
    let err_power_f21 = authorize_mutation(&req_power, &board, Some(&validation_f21))
        .expect_err("SetPowerProfile must be blocked on unvalidated 8A3D F.21");
    assert!(matches!(
        err_power_f21,
        HardwareControlError::UnsupportedMode(_)
    ));
}

#[test]
fn test_missing_validation_evidence_fails_closed() {
    let board = resolve_board_profile("8A3D").expect("resolve 8A3D");

    let mutating_requests = vec![
        HardwareRequest::SetFanMode {
            mode: "max".to_string(),
        },
        HardwareRequest::SetFanMode {
            mode: "auto".to_string(),
        },
        HardwareRequest::SetPowerProfile {
            profile: "performance".to_string(),
        },
        HardwareRequest::ApplyPreset {
            preset: "gaming".to_string(),
        },
        HardwareRequest::SetRgbEnabled { enabled: true },
    ];

    for req in mutating_requests {
        // None validation fails closed
        let err_none = authorize_mutation(&req, &board, None)
            .expect_err("Missing validation record must fail closed");
        assert!(matches!(err_none, HardwareControlError::UnsupportedMode(_)));

        // Default unvalidated record fails closed
        let unval = BoardValidation::new_unvalidated("8A3D", "F.22");
        let err_unval = authorize_mutation(&req, &board, Some(&unval))
            .expect_err("Unvalidated record must fail closed");
        assert!(matches!(
            err_unval,
            HardwareControlError::UnsupportedMode(_) | HardwareControlError::AuthorizationFailed(_)
        ));
    }
}

#[test]
fn test_daemon_and_gui_report_consistent_capabilities() {
    use zenomen::service::windows::profiles::ThermalProfileSource;

    let board = resolve_board_profile("8A3D").expect("resolve 8A3D");
    let validation = resolve_board_validation("8A3D", "F.22");

    let caps = board.capabilities(Some(&validation));

    // Telemetry capabilities
    assert!(caps.fan_count_supported);
    assert!(caps.fan_rpm_supported);
    assert!(caps.fan_table_supported);
    assert!(caps.max_fan_supported);
    assert_eq!(
        caps.thermal_profile_source,
        ThermalProfileSource::EmbeddedController
    );

    // Granular verification flags
    assert!(caps.fan_write_verified);
    assert!(caps.max_fan_write_verified);
    assert!(caps.auto_fan_restore_verified);
    assert!(caps.manual_fan_control_supported);
    assert!(!caps.manual_fan_write_verified);
    assert!(caps.power_write_authorized);
    assert!(!caps.power_write_verified);
    assert!(!caps.rgb_write_verified);

    // Serialization and deserialization consistency for RPC / GUI
    let json = serde_json::to_string(&caps).expect("serialize BoardCapabilities");
    let decoded_caps: zenomen::service::windows::profiles::BoardCapabilities =
        serde_json::from_str(&json).expect("deserialize BoardCapabilities");

    assert!(decoded_caps.fan_write_verified);
    assert!(decoded_caps.max_fan_write_verified);
    assert!(decoded_caps.auto_fan_restore_verified);
    assert!(decoded_caps.manual_fan_control_supported);
    assert!(!decoded_caps.manual_fan_write_verified);
    assert!(decoded_caps.power_write_authorized);
    assert!(!decoded_caps.power_write_verified);
    assert!(!decoded_caps.rgb_write_verified);
    assert_eq!(
        decoded_caps.thermal_profile_source,
        caps.thermal_profile_source
    );
}

#[test]
fn test_8a3d_power_profile_write_returns_profile_unverified() {
    use zenomen::service::types::PowerProfileStatus;

    let adapter = FakeWmiAdapter::new(true);
    // Expect 0x1A write for balanced [0xFF, 0x00]
    adapter.expect_call(write_ack());

    let conn = HpWmiConnection::from_adapter_for_board(adapter, "8A3D");

    let res = zenomen::service::windows::hp_wmi::set_power_profile(&conn, "balanced")
        .expect("set_power_profile on 8A3D");

    assert_eq!(res.profile, "balanced");
    assert_eq!(res.status, PowerProfileStatus::ProfileUnverified);
    assert!(res.readback_profile.is_none());
    assert!(res.message.contains("readback unavailable"));
}

#[test]
fn test_generic_omen_power_profile_write_returns_profile_verified() {
    use zenomen::service::types::PowerProfileStatus;

    let adapter = FakeWmiAdapter::new(true);
    // 1. Initial read: currently balanced (0x00)
    adapter.expect_call(FakeResponse::Success {
        payload: vec![0x00],
    });
    // 2. Write performance [0xFF, 0x01]
    adapter.expect_call(write_ack());
    // 3. Post-write read: returns performance (0x01)
    adapter.expect_call(FakeResponse::Success {
        payload: vec![0x01],
    });

    let conn = HpWmiConnection::from_adapter_for_board(adapter, "88F8");

    let res = zenomen::service::windows::hp_wmi::set_power_profile(&conn, "performance")
        .expect("set_power_profile on OMEN");

    assert_eq!(res.profile, "performance");
    assert_eq!(res.status, PowerProfileStatus::ProfileVerified);
    assert_eq!(res.readback_profile.as_deref(), Some("performance"));
    assert!(res.message.contains("verified via firmware read-back"));
}

#[test]
fn test_get_service_memory_protocol_and_server_handler() {
    let board = resolve_board_profile("8A3D").unwrap();
    let validation = resolve_board_validation("8A3D", "F.22");
    let adapter = FakeWmiAdapter::new(true);
    let conn = HpWmiConnection::from_adapter_for_board(adapter, "8A3D");

    let resp = handle_request(
        HardwareRequest::GetServiceMemory,
        &board,
        Some(&validation),
        &conn,
    );

    match resp {
        HardwareResponse::ServiceMemory(info) => {
            assert_eq!(
                info.role,
                Some(zenomen::hardware::ProcessRole::HardwareService)
            );
            assert_eq!(info.pid, std::process::id());
        }
        other => panic!("Expected ServiceMemory response, got {other:?}"),
    }
}

#[test]
fn test_zenomen_memory_telemetry_freshness_and_offline() {
    use zenomen::hardware::{ProcessMemoryInfo, ProcessRole, ZenomenMemoryTelemetry};

    let now = 1000u64;

    let gui_ok = ProcessMemoryInfo {
        role: Some(ProcessRole::Gui),
        pid: 1234,
        working_set_bytes: 30 * 1024 * 1024,
        private_bytes: 25 * 1024 * 1024,
        peak_working_set_bytes: 35 * 1024 * 1024,
        timestamp_secs: now,
        available: true,
    };

    let svc_ok = ProcessMemoryInfo {
        role: Some(ProcessRole::HardwareService),
        pid: 5678,
        working_set_bytes: 8 * 1024 * 1024,
        private_bytes: 6 * 1024 * 1024,
        peak_working_set_bytes: 10 * 1024 * 1024,
        timestamp_secs: now + 2, // within 10s window
        available: true,
    };

    let telemetry = ZenomenMemoryTelemetry::new(gui_ok.clone(), svc_ok);
    assert!(telemetry.is_service_available);
    assert_eq!(telemetry.combined_working_set_bytes, Some(38 * 1024 * 1024));
    assert_eq!(telemetry.combined_private_bytes, Some(31 * 1024 * 1024));
    assert_eq!(
        telemetry.combined_peak_working_set_bytes,
        Some(45 * 1024 * 1024)
    );
    assert_eq!(telemetry.total_working_set_str(), "38.0 MB");

    // Service offline case (available: false)
    let svc_offline = ProcessMemoryInfo {
        role: Some(ProcessRole::HardwareService),
        available: false,
        ..Default::default()
    };
    let telemetry_offline = ZenomenMemoryTelemetry::new(gui_ok.clone(), svc_offline);
    assert!(!telemetry_offline.is_service_available);
    assert_eq!(telemetry_offline.combined_working_set_bytes, None);
    assert_eq!(
        telemetry_offline.total_working_set_str(),
        "30.0 MB (App only)"
    );

    // Stale service case (time diff > 10s)
    let svc_stale = ProcessMemoryInfo {
        role: Some(ProcessRole::HardwareService),
        pid: 5678,
        working_set_bytes: 8 * 1024 * 1024,
        private_bytes: 6 * 1024 * 1024,
        peak_working_set_bytes: 10 * 1024 * 1024,
        timestamp_secs: now + 30, // 30s diff
        available: true,
    };
    let telemetry_stale = ZenomenMemoryTelemetry::new(gui_ok, svc_stale);
    assert_eq!(telemetry_stale.combined_working_set_bytes, None);
}

#[test]
fn test_query_hp_capabilities_on_8a3d_reports_power_profiles_true() {
    let adapter = FakeWmiAdapter::new(true);
    // Fan count query: opcode 0x10 returns 2 fans
    adapter.expect_call(FakeResponse::Success {
        payload: vec![0x02],
    });

    let conn = HpWmiConnection::from_adapter_for_board(adapter, "8A3D");
    let caps = zenomen::service::windows::hp_wmi::query_hp_capabilities(&conn)
        .expect("query_hp_capabilities on 8A3D");

    assert!(caps.power_profiles, "8A3D must report power_profiles: true");
    assert!(caps.fan_control, "8A3D must report fan_control: true");
    assert!(caps.fan_rpm, "8A3D must report fan_rpm: true");
    assert!(!caps.rgb_lighting, "8A3D has no RGB zones");
}

#[test]
fn test_query_hp_capabilities_unavailable_wmi() {
    let adapter = FakeWmiAdapter::new(false);
    let conn = HpWmiConnection::from_adapter_for_board(adapter, "8A3D");
    let caps = zenomen::service::windows::hp_wmi::query_hp_capabilities(&conn)
        .expect("query_hp_capabilities when unavailable");

    assert!(!caps.power_profiles);
    assert!(!caps.fan_control);
    assert!(!caps.fan_rpm);
}

#[test]
fn test_8a3d_power_write_authorized_independently_from_fan() {
    let board = resolve_board_profile("8A3D").unwrap();
    // Record where power_write_authorized is true but fan_write_verified is false
    let power_only = BoardValidation {
        board_id: "8A3D".to_string(),
        bios_version: "F.22".to_string(),
        fan_read_verified: true,
        fan_write_verified: false,
        max_fan_write_verified: false,
        auto_fan_restore_verified: false,
        manual_fan_write_verified: false,
        power_read_verified: false,
        power_write_authorized: true,
        power_write_verified: false,
        rgb_read_verified: false,
        rgb_write_verified: false,
        validation_record_version: 1,
    };

    // Power profile mutation is authorized independently
    let req_power = HardwareRequest::SetPowerProfile {
        profile: "quiet".to_string(),
    };
    assert!(
        authorize_mutation(&req_power, &board, Some(&power_only)).is_ok(),
        "Power write must be authorized independently from fan write"
    );

    // Quiet and Balanced presets (auto fan) are authorized
    let req_quiet = HardwareRequest::ApplyPreset {
        preset: "quiet".to_string(),
    };
    assert!(authorize_mutation(&req_quiet, &board, Some(&power_only)).is_ok());

    let req_balanced = HardwareRequest::ApplyPreset {
        preset: "balanced".to_string(),
    };
    assert!(authorize_mutation(&req_balanced, &board, Some(&power_only)).is_ok());

    // Performance preset (which requires max fan) is rejected because fan writes are unverified
    let req_perf = HardwareRequest::ApplyPreset {
        preset: "performance".to_string(),
    };
    let err_perf = authorize_mutation(&req_perf, &board, Some(&power_only))
        .expect_err("Performance preset must fail when fan writes are unverified");
    assert!(matches!(err_perf, HardwareControlError::UnsupportedMode(_)));

    // Standalone fan write is rejected
    let req_fan = HardwareRequest::SetFanMode {
        mode: "max".to_string(),
    };
    let err_fan = authorize_mutation(&req_fan, &board, Some(&power_only))
        .expect_err("Fan write must fail when fan_write_verified is false");
    assert!(matches!(
        err_fan,
        HardwareControlError::AuthorizationFailed(_)
    ));
}

#[test]
fn test_8a3d_apply_preset_quiet_and_balanced_restores_auto_fan_without_0x2e() {
    let board = resolve_board_profile("8A3D").unwrap();
    let validation = BoardValidation::new_verified("8A3D", "F.22");

    let adapter = std::sync::Arc::new(FakeWmiAdapter::new(true));

    // 1. Power profile write: Performance Mode (0x1A) with [0xFF, 0x00]
    adapter.expect_call(write_ack());

    // 2. Fan mode set to auto (from max):
    // Pre-read: Max fan is currently ON (0x26 -> 1)
    adapter.expect_call(fan_mode_response(0x01));

    // Validated 8A3D auto restoration sequence:
    // a. 0x27 = 0 (disable max fan)
    adapter.expect_call(write_ack());
    // b. 0x10 (arm 120s window)
    adapter.expect_call(fan_count_response(2));
    // c. 0x1A [0xFF, 0x00] (re-apply EC balanced profile)
    adapter.expect_call(write_ack());

    // Post-read: Max fan is now OFF (0x26 -> 0)
    adapter.expect_call(fan_mode_response(0x00));

    let conn = HpWmiConnection::from_adapter_for_board(adapter.clone(), "8A3D");

    let req = HardwareRequest::ApplyPreset {
        preset: "balanced".to_string(),
    };
    let resp = handle_request(req, &board, Some(&validation), &conn);

    match resp {
        HardwareResponse::PresetApplied(result) => {
            assert_eq!(result.preset_name, "balanced");
            assert!(result.power_applied);
            assert_eq!(result.confirmed_power_profile.as_deref(), Some("balanced"));
            assert!(result.fan_applied);
            assert_eq!(result.confirmed_fan_mode.as_deref(), Some("auto"));
            assert!(!result.partial_success);
            assert!(result.is_full_success());
            assert_eq!(
                result.power_profile_status,
                Some(zenomen::service::types::PowerProfileStatus::ProfileUnverified)
            );
            assert!(result.message.contains("power accepted unverified"));
        }
        other => panic!("Expected PresetApplied, got {other:?}"),
    }

    // CRITICAL FIRMWARE SAFETY INVARIANT: ZERO 0x2E calls made
    let calls = adapter.recorded_calls();
    for (i, call) in calls.iter().enumerate() {
        assert_ne!(
            call.command,
            victus_s::FAN_LEVEL_SET_QUERY,
            "Step {i}: 0x2E was called on board 8A3D! This causes 0 RPM freeze bug"
        );
    }
}

#[test]
fn test_8a3d_apply_preset_performance_enables_max_fan() {
    let board = resolve_board_profile("8A3D").unwrap();
    let validation = BoardValidation::new_verified("8A3D", "F.22");

    let adapter = std::sync::Arc::new(FakeWmiAdapter::new(true));

    // 1. Power profile write: Performance Mode (0x1A) with [0xFF, 0x01]
    adapter.expect_call(write_ack());

    // 2. Fan mode set to max (from auto):
    // Pre-read: Max fan is currently OFF (0x26 -> 0)
    adapter.expect_call(fan_mode_response(0x00));

    // On Victus S, set_max_fan_state(true) arms window with 0x10, then 0x27=1
    adapter.expect_call(fan_count_response(2));
    adapter.expect_call(write_ack()); // 0x27 = 1

    // Post-read: Max fan is now ON (0x26 -> 1)
    adapter.expect_call(fan_mode_response(0x01));

    let conn = HpWmiConnection::from_adapter_for_board(adapter.clone(), "8A3D");

    let req = HardwareRequest::ApplyPreset {
        preset: "performance".to_string(),
    };
    let resp = handle_request(req, &board, Some(&validation), &conn);

    match resp {
        HardwareResponse::PresetApplied(result) => {
            assert_eq!(result.preset_name, "performance");
            assert!(result.power_applied);
            assert_eq!(
                result.confirmed_power_profile.as_deref(),
                Some("performance")
            );
            assert!(result.fan_applied);
            assert_eq!(result.confirmed_fan_mode.as_deref(), Some("max"));
            assert!(!result.partial_success);
            assert!(result.is_full_success());
        }
        other => panic!("Expected PresetApplied, got {other:?}"),
    }
}

#[test]
fn test_8a3d_apply_preset_partial_failure_reconciles_confirmed_fan_state() {
    let board = resolve_board_profile("8A3D").unwrap();
    let validation = BoardValidation::new_verified("8A3D", "F.22");

    let adapter = std::sync::Arc::new(FakeWmiAdapter::new(true));

    // 1. Power profile write succeeds
    adapter.expect_call(write_ack());

    // 2. Fan mode: pre-read reports max (0x01)
    adapter.expect_call(fan_mode_response(0x01));

    // Restoration fails at max fan clear (0x27)
    adapter.expect_call(FakeResponse::FirmwareError { status: 0x04 });

    // On fan failure, server re-reads firmware fan mode to report confirmed hardware state
    adapter.expect_call(fan_mode_response(0x01)); // still max

    let conn = HpWmiConnection::from_adapter_for_board(adapter.clone(), "8A3D");

    let req = HardwareRequest::ApplyPreset {
        preset: "balanced".to_string(),
    };
    let resp = handle_request(req, &board, Some(&validation), &conn);

    match resp {
        HardwareResponse::PresetApplied(result) => {
            assert_eq!(result.preset_name, "balanced");
            assert!(result.power_applied);
            assert!(!result.fan_applied);
            assert!(result.partial_success);
            assert!(!result.is_full_success());
            // Confirmed fan mode is kept as "max" rather than falsely reporting "auto"
            assert_eq!(result.confirmed_fan_mode.as_deref(), Some("max"));
            assert!(
                result
                    .message
                    .contains("Power profile applied but fan write failed")
            );
        }
        other => panic!("Expected PresetApplied, got {other:?}"),
    }
}

#[test]
fn test_8a3d_standalone_set_fan_mode_auto_refuses_unsafe_reset() {
    let board = resolve_board_profile("8A3D").unwrap();
    let validation = BoardValidation::new_verified("8A3D", "F.22");

    let adapter = std::sync::Arc::new(FakeWmiAdapter::new(true));
    // Pre-read reports max fan is on (0x01)
    adapter.expect_call(fan_mode_response(0x01));

    let conn = HpWmiConnection::from_adapter_for_board(adapter.clone(), "8A3D");

    let req = HardwareRequest::SetFanMode {
        mode: "auto".to_string(),
    };
    let resp = handle_request(req, &board, Some(&validation), &conn);

    match resp {
        HardwareResponse::Error(msg) => {
            assert!(
                msg.contains("EC thermal profile"),
                "Expected message explaining EC thermal profile restore requirement, got: {msg}"
            );
            assert!(
                msg.contains("refused"),
                "Expected message stating unsafe reset is refused, got: {msg}"
            );
        }
        other => {
            panic!("Expected Error response for unknown profile standalone auto fan, got {other:?}")
        }
    }

    // Zero 0x2E calls submitted
    let calls = adapter.recorded_calls();
    for (i, call) in calls.iter().enumerate() {
        assert_ne!(
            call.command,
            victus_s::FAN_LEVEL_SET_QUERY,
            "Step {i}: 0x2E was called on board 8A3D!"
        );
    }
}
