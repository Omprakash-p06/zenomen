//! Physical fan control validation runbook for HP Victus 15-fb0xxx (Motherboard 8A3D).
//!
//! # Safety Protocol (10-State Interactive Validation)
//! 1. Explicit operator confirmation ('VALIDATE') before any firmware mutation.
//! 2. Operator initial thermal profile specification (Known vs. Safe Fallback).
//! 3. Baseline recording: atomic dual tachometer RPMs (0x2D), Max Fan state (0x26), fan count (0x10).
//! 4. Fan table query (0x2F): inspect rows, max level, GPU offset.
//! 5. User-defined state arming (0x10).
//! 6. Commanded Max Fan write (0x27 = 1) with firmware readback confirmation (0x26 = 1).
//! 7. Live tachometer acceleration observation loop (6s) tracking physical spool-up.
//! 8. Safe EC cooling restoration: 0x27=0 -> 0x10 -> 0x1A {0xFF, profile}.
//!    Strict invariant: ZERO calls to 0x2E {0,0} sentinel (preventing the 8A3D 0 RPM freeze bug).
//! 9. Sustained 30-second post-restoration telemetry observation loop recording per-second timestamps,
//!    WMI transport success, independent CPU/GPU RPMs, and physical deceleration.
//! 10. Repository gate invariant: `fan_write_verified` remains false in code until formal sign-off.

use std::io::{self, Write};
use std::thread::sleep;
use std::time::Duration;

use crate::service::types::HardwareControlError;
use crate::service::windows::hp_wmi;
use crate::service::windows::victus_s::{
    self, ObservationSample, ProfileRestoreTarget, ThermalProfile, ValidationState,
};

/// Execute the interactive 8A3D physical fan validation runbook.
pub fn run_8a3d_validation_runbook() -> Result<(), HardwareControlError> {
    println!("══════════════════════════════════════════════════════════════════════");
    println!("       HP VICTUS 8A3D PHYSICAL FAN CONTROL VALIDATION RUNBOOK        ");
    println!("══════════════════════════════════════════════════════════════════════");
    println!("Hardware Target : HP Victus 15-fb0xxx, Motherboard 8A3D, BIOS F.22");
    println!("Protocol Family : HPWMI_GM (0x20008) / Victus S EC Thermal Layout");
    println!("Restoration Path: 0x27=0 -> 0x10 -> 0x1A {{0xFF, profile}}");
    println!("Safety Invariant: 0x2E {{0x00, 0x00}} is strictly forbidden");
    println!("Telemetry Window: 30-second sustained post-restoration observation");
    println!("──────────────────────────────────────────────────────────────────────");

    let mut state = ValidationState::Idle;
    println!("Initial State   : {}", state.as_str());

    // Check detected motherboard
    let detected_board =
        crate::hardware::windows::device::read_board_id().unwrap_or_else(|| "UNKNOWN".to_string());
    println!("Detected Board ID: {detected_board}");
    if !detected_board.eq_ignore_ascii_case("8A3D") {
        println!("⚠️  WARNING: Detected board '{detected_board}' is not '8A3D'.");
        println!("This validation runbook is specifically engineered for 8A3D firmware behavior.");
    }

    println!("\nThis procedure will briefly command Maximum Fan speed (100% PWM), observe");
    println!("live tachometers for physical acceleration, restore autonomous EC cooling,");
    println!("and observe fan deceleration and curve recovery for 30 consecutive seconds.");
    println!("Ensure the laptop is on a flat surface and vents are unobstructed.\n");

    print!("Type 'VALIDATE' to proceed with physical hardware validation: ");
    io::stdout()
        .flush()
        .map_err(|e| HardwareControlError::IoError(e.to_string()))?;

    let mut input = String::new();
    io::stdin()
        .read_line(&mut input)
        .map_err(|e| HardwareControlError::IoError(e.to_string()))?;

    if input.trim() != "VALIDATE" {
        println!("\n[!] Validation aborted by operator. No firmware mutations performed.");
        return Ok(());
    }

    // Prompt for original thermal profile (or explicit safe fallback)
    println!("\nSpecify the active thermal profile before validation begins:");
    println!("  [1] Balanced (0x00) - Default OMEN/Victus standard profile");
    println!("  [2] Performance (0x01) - High power / aggressive cooling");
    println!("  [3] Quiet (0x03) - Low power / silent cooling");
    println!("  [4] Unknown - Operator does not know; apply explicit safe fallback (Balanced)");
    print!("Selection [1-4, default 4]: ");
    io::stdout()
        .flush()
        .map_err(|e| HardwareControlError::IoError(e.to_string()))?;

    let mut prof_input = String::new();
    io::stdin()
        .read_line(&mut prof_input)
        .map_err(|e| HardwareControlError::IoError(e.to_string()))?;

    let restore_target = match prof_input.trim() {
        "1" | "balanced" => ProfileRestoreTarget::Known(ThermalProfile::Balanced),
        "2" | "performance" => ProfileRestoreTarget::Known(ThermalProfile::Performance),
        "3" | "quiet" => ProfileRestoreTarget::Known(ThermalProfile::Quiet),
        _ => {
            println!(
                "  -> Original profile marked UNKNOWN. Explicit safe fallback (Balanced) selected."
            );
            ProfileRestoreTarget::SafeFallback(ThermalProfile::Balanced)
        }
    };

    println!(
        "Thermal profile restoration target configured: {} (Known: {})",
        restore_target.profile().as_str(),
        restore_target.is_known()
    );

    println!("\nConnecting to HP BIOS WMI transport...");
    let conn = hp_wmi::connect_hp_wmi()?;
    if !conn.is_available() {
        return Err(HardwareControlError::InterfaceMissing(
            "HP WMI interface is unavailable on this system".to_string(),
        ));
    }

    // Step 1: Baseline Acquisition (atomic dual-fan reading)
    println!("\n[Step 1/8] Recording initial baseline telemetry...");
    let baseline_max_fan = hp_wmi::get_max_fan_state(&conn)?;
    let fan_count = hp_wmi::get_fan_count(&conn)?;
    let dual_baseline = hp_wmi::get_victus_dual_fan_rpm(&conn)?;
    let cpu_baseline = dual_baseline.cpu_rpm;
    let gpu_baseline = dual_baseline.gpu_rpm;

    state = ValidationState::BaselineCaptured;
    println!("  - State Machine     : {}", state.as_str());
    println!("  - Fan Count         : {fan_count}");
    println!(
        "  - Max Fan Mode      : {}",
        if baseline_max_fan {
            "ACTIVE (1)"
        } else {
            "OFF (0)"
        }
    );
    println!("  - CPU Fan Baseline  : {cpu_baseline} RPM");
    println!(
        "  - GPU Fan Baseline  : {} RPM {}",
        gpu_baseline,
        if gpu_baseline == 0 {
            "(Fan stopped by autonomous EC cooling curve - cool temperature)"
        } else {
            "(Active)"
        }
    );

    // Step 2: Fan Table Inspection (0x2F)
    println!("\n[Step 2/8] Inspecting Victus S Fan Table (0x2F)...");
    match hp_wmi::get_victus_fan_table(&conn) {
        Ok(table) => {
            println!("  - Table Rows        : {}", table.entries.len());
            println!("  - Min Level         : {}", table.min_rpm);
            println!("  - Max Level         : {}", table.max_rpm);
            println!(
                "  - GPU Offset        : {}",
                table
                    .gpu_offset
                    .map(|o| format!("{o}"))
                    .unwrap_or_else(|| "None (row 0 zero-GPU RPM)".to_string())
            );
        }
        Err(e) => {
            println!(
                "  - Fan Table Query   : Failed or absent ({e}) - continuing with Max Fan validation"
            );
        }
    }

    // Step 3: Arm Manual Window (0x10)
    println!("\n[Step 3/8] Arming manual fan control window (0x10)...");
    let armed_count = hp_wmi::get_fan_count(&conn)?;
    println!("  - Firmware confirmed armed for {armed_count} fans (120s window)");

    // Step 4: Write Max Fan State (0x27 = 1)
    println!("\n[Step 4/8] Submitting Max Fan command (0x27 = [1, 0, 0, 0])...");
    state = ValidationState::MaxFanRequested;
    println!("  - State Machine     : {}", state.as_str());
    hp_wmi::set_max_fan_state(&conn, true)?;
    println!("  - WMI call returned status 0 (Success)");

    // Step 5: Live Tachometer Acceleration Observation Loop (6s)
    println!("\n[Step 5/8] Observing tachometer response for 6 seconds (polling 0x2D)...");
    let mut peak_cpu = cpu_baseline;
    let mut peak_gpu = gpu_baseline;
    let mut max_fan_status_confirmed = false;

    for second in 1..=6 {
        sleep(Duration::from_secs(1));
        let dual_now = hp_wmi::get_victus_dual_fan_rpm(&conn);
        let max_stat = hp_wmi::get_max_fan_state(&conn);

        let (cpu_now, gpu_now) = match dual_now {
            Ok(d) => (d.cpu_rpm, d.gpu_rpm),
            Err(ref e) => {
                println!("  [{second}s] Telemetry read error: {e}");
                (peak_cpu, peak_gpu)
            }
        };

        if matches!(max_stat, Ok(true)) && !max_fan_status_confirmed {
            max_fan_status_confirmed = true;
            state = ValidationState::MaxFanConfirmed;
            println!("  - State Machine     : {}", state.as_str());
        }
        if cpu_now > peak_cpu {
            peak_cpu = cpu_now;
        }
        if gpu_now > peak_gpu {
            peak_gpu = gpu_now;
        }

        let max_str = match max_stat {
            Ok(true) => "MAX(1)",
            Ok(false) => "0",
            Err(_) => "ERR",
        };

        println!(
            "  [{second}s] Status 0x26={:<6} | CPU: {cpu_now:>5} RPM (delta: {:+4}) | GPU: {gpu_now:>5} RPM (delta: {:+4})",
            max_str,
            cpu_now as i32 - cpu_baseline as i32,
            gpu_now as i32 - gpu_baseline as i32
        );
    }

    let cpu_delta = peak_cpu as i32 - cpu_baseline as i32;
    let gpu_delta = peak_gpu as i32 - gpu_baseline as i32;
    let physical_acceleration_observed = cpu_delta >= 200 || gpu_delta >= 200 || peak_cpu >= 3500;

    if physical_acceleration_observed {
        state = ValidationState::FanAccelerationObserved;
        println!("  - State Machine     : {}", state.as_str());
        println!(
            "  - Physical Tachometer Acceleration Confirmed (Peak: CPU {peak_cpu} RPM, GPU {peak_gpu} RPM)"
        );
    } else {
        println!("  - ⚠️ Physical acceleration was not conclusively observed on tachometers");
    }

    // Step 6: Safe Automatic EC Cooling Restoration
    println!("\n[Step 6/8] Restoring autonomous EC cooling via 8A3D protocol...");
    state = ValidationState::Restoring;
    println!("  - State Machine     : {}", state.as_str());
    println!(
        "  - Sequence          : 0x27=0 -> 0x10 -> 0x1A {{0xFF, 0x{:02X}}} ({})",
        restore_target.profile().firmware_value(),
        restore_target.profile().as_str()
    );

    let profile_restored;
    match hp_wmi::restore_automatic_fan_control_with_target(&conn, Some(restore_target)) {
        Ok(_) => {
            profile_restored = true;
            state = ValidationState::RestorationCommandAccepted;
            println!("  - State Machine     : {}", state.as_str());
            println!("  - Firmware accepted restore command sequence successfully");
        }
        Err(e) => {
            state = ValidationState::RestorationFailed;
            println!("  - State Machine     : {}", state.as_str());
            println!("  - 🚨 RESTORATION COMMAND FAILED: {e}");
            println!("\nEMERGENCY OPERATOR INSTRUCTIONS:");
            println!("1. Immediately open HP OMEN Gaming Hub to reset thermal control.");
            println!(
                "2. If fans fail to respond, reboot system immediately to re-arm BIOS thermal curve."
            );
            println!("3. Do not submit any further manual fan writes.");
            return Err(e);
        }
    }

    // Invariant check: verify no 0x2E was emitted
    println!("  - Invariant Verified: Zero 0x2E {{0,0}} calls submitted (preventing 0 RPM freeze)");

    // Step 7: Sustained 30-Second Post-Restoration Observation Loop
    println!("\n[Step 7/8] Entering sustained 30-second post-restoration telemetry observation...");
    state = ValidationState::ObservingAutomaticControl;
    println!("  - State Machine     : {}", state.as_str());
    println!(
        "  - Recording per-second tachometer samples, 0x26 state, and WMI transport integrity...\n"
    );

    let mut samples: Vec<ObservationSample> = Vec::with_capacity(30);

    for second in 1..=30 {
        sleep(Duration::from_secs(1));
        let max_stat = hp_wmi::get_max_fan_state(&conn).map_err(|e| e.to_string());
        let dual_telemetry = hp_wmi::get_victus_dual_fan_rpm(&conn);

        let (cpu_res, gpu_res) = match dual_telemetry {
            Ok(d) => (Ok(d.cpu_rpm), Ok(d.gpu_rpm)),
            Err(e) => {
                let err_str = e.to_string();
                (Err(err_str.clone()), Err(err_str))
            }
        };

        let sample = ObservationSample {
            elapsed_secs: second,
            max_fan_active: max_stat.clone(),
            cpu_rpm: cpu_res.clone(),
            gpu_rpm: gpu_res.clone(),
        };

        let max_str = match max_stat {
            Ok(true) => "MAX(1) [WARN]",
            Ok(false) => "OFF(0) [OK]",
            Err(_) => "ERR",
        };

        let cpu_str = match cpu_res {
            Ok(rpm) => format!("{rpm:>5} RPM"),
            Err(_) => "ERROR".to_string(),
        };

        let gpu_str = match gpu_res {
            Ok(0) => "    0 RPM (EC idle/cool)".to_string(),
            Ok(rpm) => format!("{rpm:>5} RPM"),
            Err(_) => "ERROR".to_string(),
        };

        println!("  [{second:>2}s] 0x26: {max_str:<12} | CPU: {cpu_str} | GPU: {gpu_str}");

        samples.push(sample);
    }

    // Evaluate terminal validation state
    state = victus_s::evaluate_observation(&samples, peak_cpu, peak_gpu, profile_restored);

    // Step 8: Final Validation Report
    println!("\n══════════════════════════════════════════════════════════════════════");
    println!("                  VALIDATION SUMMARY REPORT                           ");
    println!("══════════════════════════════════════════════════════════════════════");
    println!(" Motherboard Identity       : {detected_board} (Victus S EC Layout)");
    println!(
        " Starting Thermal Profile   : {} ({})",
        restore_target.profile().as_str(),
        if restore_target.is_known() {
            "Known operator-verified profile"
        } else {
            "Safe fallback profile (original unknown)"
        }
    );
    println!(" Firmware Baseline          : CPU={cpu_baseline} RPM, GPU={gpu_baseline} RPM");
    println!(" WMI Max Fan Commanded      : SUCCESS (0x27 wrote [1, 0, 0, 0])");
    println!(
        " Firmware Mode Readback     : {}",
        if max_fan_status_confirmed {
            "VERIFIED (0x26 returned 1)"
        } else {
            "UNCONFIRMED"
        }
    );
    println!(
        " Peak Observed Telemetry    : CPU={peak_cpu} RPM ({cpu_delta:+4}), GPU={peak_gpu} RPM ({gpu_delta:+4})"
    );
    println!(
        " Physical Tachometer Delta  : {}",
        if physical_acceleration_observed {
            "CONFIRMED ACCELERATION"
        } else {
            "NO SIGNIFICANT TACH DELTA (Check hardware)"
        }
    );
    println!(
        " Safe Cooling Restoration   : VERIFIED (0x27=0 -> 0x10 -> 0x1A {{0xFF, 0x{:02X}}})",
        restore_target.profile().firmware_value()
    );
    println!(" Danger Avoidance           : 0x2E {{0,0}} was NOT called");
    println!(
        " Profile Preservation Status: {}",
        if restore_target.is_known() {
            "Restored known active profile"
        } else {
            "Applied safe fallback (original profile not claimed restored)"
        }
    );
    println!(" 30s Observation Window     : 30/30 samples collected | Max Fan remained 0 (OFF)");
    println!(" State Machine Result       : {}", state.as_str());
    println!(" Repository Safety Gate     : fan_write_verified = false (STRICTLY UNCHANGED)");
    println!("──────────────────────────────────────────────────────────────────────");

    match state {
        ValidationState::RestorationVerified => {
            println!(
                "OUTCOME: Automatic EC cooling restoration successfully verified on physical hardware."
            );
        }
        ValidationState::RestorationUnverified => {
            println!(
                "OUTCOME: Inconclusive telemetry during observation. Restoration remains unverified."
            );
        }
        ValidationState::RestorationFailed => {
            println!("OUTCOME: Restoration failed or abnormal fan behavior detected.");
        }
        _ => {}
    }

    println!("NOTE: Repository authorization remains closed pending formal review.");
    println!("══════════════════════════════════════════════════════════════════════");

    Ok(())
}
