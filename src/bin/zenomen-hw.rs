//! Privileged hardware control daemon for HP OMEN and Victus laptops.
//!
//! Provides the `com.zenomen.Hardware1` D-Bus system service.
//! Every mutating call is gated by Polkit authorization (`com.zenomen.hardware.control`).

use std::error::Error;
use tracing::info;
#[cfg(target_os = "linux")]
use zenomen::service::{HardwareBackend, HardwareService};

#[cfg(target_os = "windows")]
fn main() -> Result<(), Box<dyn Error>> {
    let _logging_guard = zenomen::logging::init();

    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|a| a == "--install-service") {
        info!("Registering ZenOmen hardware service in SCM...");
        zenomen::service::windows::lifecycle::register_service()?;
        println!("Service successfully registered.");
        return Ok(());
    }

    if args.iter().any(|a| a == "--uninstall-service") {
        info!("Uninstalling ZenOmen hardware service from SCM...");
        zenomen::service::windows::lifecycle::uninstall_service()?;
        println!("Service successfully uninstalled.");
        return Ok(());
    }

    if args.iter().any(|a| a == "--console" || a == "-c") {
        info!("Starting zenomen-hw in standalone console mode...");
        zenomen::service::windows::lifecycle::run_console_service()?;
        return Ok(());
    }

    if args.iter().any(|a| a == "--wmi-diag" || a == "--diag") {
        info!("Running read-only HP BIOS WMI diagnostics...");
        zenomen::service::windows::wmi_diag::print_wmi_diagnostic();
        return Ok(());
    }

    if args.iter().any(|a| a == "--validate-8a3d-fan") {
        info!("Launching HP Victus 8A3D physical fan control validation runbook...");
        zenomen::service::windows::validation_runbook::run_8a3d_validation_runbook()?;
        return Ok(());
    }

    if args.iter().any(|a| a == "--validate-8a3d-power") {
        info!("Launching HP Victus 8A3D physical power profile validation runbook...");
        zenomen::service::windows::power_validation_runbook::run_8a3d_power_validation_runbook()?;
        return Ok(());
    }

    info!(
        "Starting zenomen-hw privileged hardware control service v{}",
        env!("CARGO_PKG_VERSION")
    );

    zenomen::service::windows::lifecycle::run_service()?;
    Ok(())
}

#[cfg(target_os = "linux")]
fn main() -> Result<(), Box<dyn Error>> {
    // First statement: a failure to bind D-Bus or load the backend is exactly
    // what the log file needs to record. Journald still captures stderr.
    let _logging_guard = zenomen::logging::init();

    info!(
        "Starting zenomen-hw privileged hardware control service v{}",
        env!("CARGO_PKG_VERSION")
    );

    zbus::block_on(async {
        let backend = HardwareBackend::default();

        let conn_builder = if std::env::var("ZENOMEN_HW_SESSION").is_ok() {
            zbus::connection::Builder::session()?
        } else {
            zbus::connection::Builder::system()?
        };
        let service = HardwareService::new(backend);

        let conn = conn_builder
            .name("com.zenomen.Hardware1")?
            .serve_at("/com/zenomen/Hardware1", service.clone())?
            .build()
            .await?;

        // Provide the active connection to the service so it can verify polkit callers
        service.set_connection(conn.clone());

        info!("com.zenomen.Hardware1 service registered on system D-Bus");

        // Keep service alive until terminated
        std::future::pending::<()>().await;
        Ok::<(), Box<dyn Error>>(())
    })?;

    Ok(())
}
