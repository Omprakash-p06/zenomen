//! ZenOmen — lightweight hardware control center for HP OMEN and Victus laptops.
//!
//! Phase 1 scaffold: initialise structured logging, wire the Slint window to the
//! core through `app::ZenOmenApp`, then hand control to the Slint event loop.
//! Everything the window displays is owned by the core and reaches the UI as
//! typed events; this file must never read a sysfs path or spawn a hardware
//! command.

mod app;
mod core;
mod hardware;

use std::sync::mpsc::Receiver;
use std::thread::JoinHandle;

use app::ZenOmenApp;
use core::{AppCommand, AppEvent};
use hardware::HardwareCapabilities;

slint::include_modules!();

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // `RUST_LOG` selects verbosity; without it the default filter is used.
    tracing_subscriber::fmt::init();

    tracing::info!(version = env!("CARGO_PKG_VERSION"), "starting ZenOmen");

    let ui = AppWindow::new()?;
    ui.set_app_version(env!("CARGO_PKG_VERSION").into());

    let (app, commands) = ZenOmenApp::new(&ui);
    app.install_callbacks();

    // Phase 1 has no detector yet, so the nominal capability set is declared
    // here. Plan 01-03 replaces this line with the mock provider's own report,
    // sent through `AppEvent::CapabilitiesLoaded` like any other update.
    app.initialize(HardwareCapabilities::all_supported());

    let drain = spawn_command_drain(app.clone(), commands);

    ui.run()?;

    // Closing the window ends the process, and the drain thread is blocked on a
    // channel that a live sender (its own `ZenOmenApp` clone) keeps open. It
    // holds no window — only a `Weak` — and no hardware, so there is nothing to
    // unwind: detach it and let process exit reclaim the thread.
    drop(app);
    drop(drain);

    tracing::info!("ZenOmen shut down cleanly");
    Ok(())
}

/// Consume queued [`AppCommand`]s on a dedicated thread.
///
/// This is the seam plan 01-03 fills in: today every command is logged and
/// navigation is echoed back as a [`AppEvent::ScreenChanged`] so the core's
/// pointer stays in step with the sidebar. Telemetry refresh becomes a provider
/// sample, and nothing in the UI changes when it does.
fn spawn_command_drain(app: ZenOmenApp, commands: Receiver<AppCommand>) -> JoinHandle<()> {
    std::thread::Builder::new()
        .name("zenomen-command-drain".to_string())
        .spawn(move || {
            for command in commands {
                match command {
                    AppCommand::Navigate(screen) => {
                        tracing::debug!(screen = screen.title(), "screen requested");
                        app.handle_event(AppEvent::ScreenChanged(screen));
                    }
                    AppCommand::RefreshTelemetry => {
                        tracing::debug!("telemetry refresh requested");
                    }
                    AppCommand::SetHistoryWindow(samples) => {
                        tracing::debug!(samples, "history window requested");
                    }
                }
            }

            tracing::debug!("command drain stopped");
        })
        .expect("spawning the command drain thread must succeed")
}
