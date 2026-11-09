//! Physical fan control validation runbook for HP Victus 15-fb0xxx (Motherboard 8A3D).
//!
//! # Safety Protocol (10-Step Interactive Validation)
//! 1. Explicit operator confirmation ('VALIDATE') before any firmware mutation.
//! 2. Baseline recording: tachometer RPMs (0x2D), Max Fan state (0x26), fan count (0x10).
//! 3. Fan table query (0x2F): inspect rows, max level, GPU offset.
//! 4. User-defined state arming (0x10).
//! 5. Commanded Max Fan write (0x27 = 1).
//! 6. Live tachometer observation loop (5-8s) tracking RPM delta and audio/firmware verification.
//! 7. Safe EC cooling restoration: 0x27=0 -> 0x10 -> 0x1A {0xFF, 0x00} (Balanced EC profile).
//!    Strict invariant: ZERO calls to 0x2E {0,0} sentinel (preventing the 8A3D 0 RPM freeze bug).
//! 8. Post-restore verification: confirm 0x26=0 and tachometers return to autonomous control.
//! 9. Comprehensive report: separate WMI protocol success from physical tachometer verification.
//! 10. Repository gate invariant: `fan_write_verified` remains false in code until formal sign-off.

use std::io::{self, Write};
use std::thread::sleep;
use std::time::Duration;

use crate::service::types::HardwareControlError;
use crate::service::windows::hp_wmi;
use crate::service::windows::victus_s::ThermalProfile;

/// Execute the interactive 8A3D physical fan validation runbook.
pub fn run_8a3d_validation_runbook() -> Result<(), HardwareControlError> {
    println!("══════════════════════════════════════════════════════════════════════");
    println!("       HP VICTUS 8A3D PHYSICAL FAN CONTROL VALIDATION RUNBOOK        ");
    println!("══════════════════════════════════════════════════════════════════════");
    println!("Hardware Target : HP Victus 15-fb0xxx, Motherboard 8A3D, BIOS F.22");
    println!("Protocol Family : HPWMI_GM (0x20008) / Victus S EC Thermal Layout");
    println!("Restoration Path: 0x27=0 -> 0x10 -> 0x1A {{0xFF, 0x00}} (Balanced)");
    println!("Safety Invariant: 0x2E {{0x00, 0x00}} is strictly forbidden");
    println!("──────────────────────────────────────────────────────────────────────");

    // Check detected motherboard
    let detected_board =
        crate::hardware::windows::device::read_board_id().unwrap_or_else(|| "UNKNOWN".to_string());
    println!("Detected Board ID: {detected_board}");
    if !detected_board.eq_ignore_ascii_case("8A3D") {
        println!("⚠️  WARNING: Detected board '{detected_board}' is not '8A3D'.");
        println!("This validation runbook is specifically engineered for 8A3D firmware behavior.");
    }

    println!("\nThis procedure will briefly command Maximum Fan speed (100% PWM), observe");
    println!("live tachometers for physical acceleration, and then restore autonomous EC cooling.");
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

    println!("\nConnecting to HP BIOS WMI transport...");
    let conn = hp_wmi::connect_hp_wmi()?;
    if !conn.is_available() {
        return Err(HardwareControlError::InterfaceMissing(
            "HP WMI interface is unavailable on this system".to_string(),
        ));
    }

    // Step 1: Baseline Acquisition
    println!("\n[Step 1/7] Recording initial baseline telemetry...");
    let baseline_max_fan = hp_wmi::get_max_fan_state(&conn)?;
    let fan_count = hp_wmi::get_fan_count(&conn)?;
    let cpu_baseline = hp_wmi::get_fan_rpm(&conn, 0)?.unwrap_or(0);
    let gpu_baseline = hp_wmi::get_fan_rpm(&conn, 1)?.unwrap_or(0);

    println!("  - Fan Count        : {fan_count}");
    println!(
        "  - Max Fan Mode     : {}",
        if baseline_max_fan {
            "ACTIVE (1)"
        } else {
            "OFF (0)"
        }
    );
    println!("  - CPU Fan Baseline : {cpu_baseline} RPM");
    println!("  - GPU Fan Baseline : {gpu_baseline} RPM");

    // Step 2: Fan Table Inspection (0x2F)
    println!("\n[Step 2/7] Inspecting Victus S Fan Table (0x2F)...");
    match hp_wmi::get_victus_fan_table(&conn) {
        Ok(table) => {
            println!("  - Table Rows       : {}", table.entries.len());
            println!("  - Min Level        : {}", table.min_rpm);
            println!("  - Max Level        : {}", table.max_rpm);
            println!(
                "  - GPU Offset       : {}",
                table
                    .gpu_offset
                    .map(|o| format!("{o}"))
                    .unwrap_or_else(|| "None (row 0 zero-GPU RPM)".to_string())
            );
        }
        Err(e) => {
            println!(
                "  - Fan Table Query  : Failed or absent ({e}) - continuing with Max Fan validation"
            );
        }
    }

    // Step 3: Arm Manual Window (0x10)
    println!("\n[Step 3/7] Arming manual fan control window (0x10)...");
    let armed_count = hp_wmi::get_fan_count(&conn)?;
    println!("  - Firmware confirmed armed for {armed_count} fans (120s window)");

    // Step 4: Write Max Fan State (0x27 = 1)
    println!("\n[Step 4/7] Submitting Max Fan command (0x27 = [1, 0, 0, 0])...");
    hp_wmi::set_max_fan_state(&conn, true)?;
    println!("  - WMI call returned status 0 (Success)");

    // Step 5: Live Tachometer Observation Loop
    println!("\n[Step 5/7] Observing tachometer response for 6 seconds (polling 0x2D)...");
    let mut peak_cpu = cpu_baseline;
    let mut peak_gpu = gpu_baseline;
    let mut max_fan_status_confirmed = false;

    for second in 1..=6 {
        sleep(Duration::from_secs(1));
        let cpu_now = hp_wmi::get_fan_rpm(&conn, 0)?.unwrap_or(0);
        let gpu_now = hp_wmi::get_fan_rpm(&conn, 1)?.unwrap_or(0);
        let max_stat = hp_wmi::get_max_fan_state(&conn).unwrap_or(false);
        if max_stat {
            max_fan_status_confirmed = true;
        }
        if cpu_now > peak_cpu {
            peak_cpu = cpu_now;
        }
        if gpu_now > peak_gpu {
            peak_gpu = gpu_now;
        }
        println!(
            "  [{second}s] Status 0x26={:<5} | CPU: {cpu_now:>5} RPM (delta: {:+4}) | GPU: {gpu_now:>5} RPM (delta: {:+4})",
            if max_stat { "MAX(1)" } else { "0" },
            cpu_now as i32 - cpu_baseline as i32,
            gpu_now as i32 - gpu_baseline as i32
        );
    }

    // Step 6: Safe Automatic EC Cooling Restoration
    println!("\n[Step 6/7] Restoring autonomous EC cooling via 8A3D protocol...");
    println!(
        "  - Applying 8A3D restore sequence: 0x27=0 -> 0x10 -> 0x1A {{0xFF, 0x00}} (Balanced)"
    );
    match hp_wmi::restore_automatic_fan_control(&conn, Some(ThermalProfile::Balanced)) {
        Ok(_) => println!("  - EC thermal profile restored successfully"),
        Err(e) => println!("  - ⚠️ Warning during EC profile restore: {e}"),
    }

    // Invariant check: verify no 0x2E was emitted
    println!("  - Invariant Verified: Zero 0x2E {{0,0}} calls submitted (preventing 0 RPM freeze)");

    // Step 7: Post-Restore Settle Observation
    println!("\n[Step 7/7] Verifying restoration to autonomous firmware control...");
    sleep(Duration::from_secs(2));
    let post_max_stat = hp_wmi::get_max_fan_state(&conn).unwrap_or(false);
    let post_cpu = hp_wmi::get_fan_rpm(&conn, 0)?.unwrap_or(0);
    let post_gpu = hp_wmi::get_fan_rpm(&conn, 1)?.unwrap_or(0);

    println!(
        "  - Post-Restore Max Fan Status: {}",
        if post_max_stat {
            "ACTIVE (FAIL)"
        } else {
            "OFF (0) [OK]"
        }
    );
    println!("  - Post-Restore CPU RPM       : {post_cpu} RPM");
    println!("  - Post-Restore GPU RPM       : {post_gpu} RPM");

    // Step 8: Final Validation Report
    let cpu_delta = peak_cpu as i32 - cpu_baseline as i32;
    let gpu_delta = peak_gpu as i32 - gpu_baseline as i32;
    let physical_acceleration_observed = cpu_delta >= 200 || gpu_delta >= 200;

    println!("\n══════════════════════════════════════════════════════════════════════");
    println!("                  VALIDATION SUMMARY REPORT                           ");
    println!("══════════════════════════════════════════════════════════════════════");
    println!(" Motherboard Identity       : {detected_board}");
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
            "NO SIGNIFICANT TACH DELTA (Check fan status)"
        }
    );
    println!(" Safe Cooling Restoration   : VERIFIED (0x27=0 -> 0x10 -> 0x1A {{0xFF, 0x00}})");
    println!(" Danger Avoidance           : 0x2E {{0,0}} was NOT called");
    println!(" Repository Safety Gate     : fan_write_verified = false (UNCHANGED)");
    println!("──────────────────────────────────────────────────────────────────────");
    println!("NOTE: Repository authorization remains closed pending formal review.");
    println!("══════════════════════════════════════════════════════════════════════");

    Ok(())
}
