//! ZenOmen — lightweight hardware control center for HP OMEN and Victus laptops.
//!
//! Phase 1 scaffold: initialise structured logging, wire the Slint window to the
//! core through `app::ZenOmenApp`, then hand control to the Slint event loop.
//! Everything the window displays is owned by the core and reaches the UI as
//! typed events; this file must never read a sysfs path or spawn a hardware
//! command.

use std::sync::Arc;
use std::sync::mpsc;

use slint::ComponentHandle;
use zenomen::AppWindow;
use zenomen::app::ZenOmenApp;
use zenomen::core::{log_benchmark, spawn_monitoring_worker};
use zenomen::hardware::{HardwareProvider, MockHardware};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // `RUST_LOG` selects verbosity; without it the default filter is used.
    tracing_subscriber::fmt::init();

    tracing::info!(version = env!("CARGO_PKG_VERSION"), "starting ZenOmen");
    log_benchmark("cold-start");

    let ui = AppWindow::new()?;
    ui.set_app_version(env!("CARGO_PKG_VERSION").into());

    let (app, commands) = ZenOmenApp::new(&ui);
    app.install_callbacks();

    // Select hardware provider: configurable via ZENOMEN_MOCK_SCENARIO env var
    let (provider, scenario_name, registry_opt): (
        Arc<dyn HardwareProvider>,
        Option<&'static str>,
        Option<Arc<zenomen::hardware::SensorRegistry>>,
    ) = if std::env::var("ZENOMEN_MOCK_SCENARIO").is_ok() {
        let mock = Arc::new(MockHardware::from_env());
        let scenario = mock.scenario().name();
        (mock, Some(scenario), None)
    } else {
        let linux = Arc::new(zenomen::hardware::LinuxHardwareProvider::new());
        let reg = Arc::clone(linux.registry());
        (linux, None, Some(reg))
    };

    if let Some(scenario) = scenario_name {
        tracing::info!(scenario, "mock hardware provider initialized");
        ui.set_provider_mode("mock".into());
    } else {
        tracing::info!("linux hardware provider initialized");
        ui.set_provider_mode("linux".into());
    }

    if let Some(reg) = registry_opt {
        app.set_sensor_registry(reg);
    }

    // Initialize capabilities on the UI thread before window opens (honest first frame)
    app.initialize(provider.capabilities());
    log_benchmark("warm-dashboard");

    // Event bridge: channel from worker thread -> UI event handler
    let (event_sender, event_receiver) = mpsc::channel();
    let app_bridge = app.clone();
    let event_pump = std::thread::Builder::new()
        .name("zenomen-event-pump".to_string())
        .spawn(move || {
            for event in event_receiver {
                app_bridge.handle_event(event);
            }
        })
        .expect("spawning event pump must succeed");

    // Spawn monitoring worker on dedicated background thread
    let worker = spawn_monitoring_worker(provider, event_sender, commands);

    ui.run()?;

    // Clean teardown: drop window and app to close channels and terminate worker/pump
    drop(ui);
    drop(app);
    let _ = worker.join();
    let _ = event_pump.join();

    tracing::info!("ZenOmen shut down cleanly");
    Ok(())
}
