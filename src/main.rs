//! ZenOmen — lightweight hardware control center for HP OMEN and Victus laptops.
//!
//! Phase 1 scaffold: initialise structured logging, then hand control to the
//! Slint event loop. Everything the window displays is owned by the core and
//! reaches the UI as typed events; this file must never read a sysfs path or
//! spawn a hardware command.

mod hardware;

slint::include_modules!();

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // `RUST_LOG` selects verbosity; without it the default filter is used.
    tracing_subscriber::fmt::init();

    tracing::info!(version = env!("CARGO_PKG_VERSION"), "starting ZenOmen");

    AppWindow::new()?.run()?;

    tracing::info!("ZenOmen shut down cleanly");
    Ok(())
}
