//! Privileged hardware control daemon for HP OMEN and Victus laptops.
//!
//! Provides the `com.zenomen.Hardware1` D-Bus system service.
//! Every mutating call is gated by Polkit authorization (`com.zenomen.hardware.control`).

use std::error::Error;
use tracing::info;
use zenomen::service::{HardwareBackend, HardwareService};

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
