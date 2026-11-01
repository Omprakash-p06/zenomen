//! ZenOmen — lightweight hardware control center for HP OMEN and Victus laptops.
//!
//! Main entry point: manages single-instance enforcement on session D-Bus,
//! system tray integration, close-to-tray lifecycle, and monitoring worker orchestration.

use std::sync::atomic::Ordering;
use std::sync::mpsc;
use std::sync::Arc;

use slint::ComponentHandle;
use zenomen::app::ZenOmenApp;
use zenomen::core::{log_benchmark, spawn_monitoring_worker};
use zenomen::desktop::{
    ensure_single_instance, register_tray, DesktopAction, SingleInstanceResult,
};
use zenomen::hardware::{HardwareProvider, MockHardware};
use zenomen::AppWindow;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 0. Load persistent configuration (SYS-06, SYS-07)
    let config_path = zenomen::config::default_config_path()
        .unwrap_or_else(|| std::path::PathBuf::from("config.toml"));
    let (config, corrupt_notice) = zenomen::config::load_or_default(&config_path);

    // 1. Check CLI args or config for start-minimized flag (SYS-03)
    let start_minimized = std::env::args().any(|arg| arg == "--minimized" || arg == "-m")
        || config.general.start_minimized;

    // 2. Single-instance enforcement (SYS-05)
    let (action_sender, action_receiver) = mpsc::sync_channel::<DesktopAction>(16);
    let single_instance = ensure_single_instance(action_sender.clone());
    let _session_conn = match single_instance {
        SingleInstanceResult::Primary(conn) => {
            let _ = register_tray(&conn, action_sender);
            Some(conn)
        }
        SingleInstanceResult::AlreadyRunning => {
            // Secondary instance triggered PresentWindow on primary and exits immediately
            return Ok(());
        }
        SingleInstanceResult::Disabled(e) => {
            tracing::warn!(error = %e, "running in standalone mode without session D-Bus");
            None
        }
    };

    // `RUST_LOG` selects verbosity; without it the default filter is used.
    tracing_subscriber::fmt::init();

    tracing::info!(version = env!("CARGO_PKG_VERSION"), "starting ZenOmen");
    log_benchmark("cold-start");

    let ui = AppWindow::new()?;
    ui.set_app_version(env!("CARGO_PKG_VERSION").into());

    let (app, commands) = ZenOmenApp::new(&ui);
    app.install_callbacks();

    // 3. Close-to-tray lifecycle management (SYS-04)
    let close_to_tray_flag = app.close_to_tray_flag();
    close_to_tray_flag.store(config.general.close_to_tray, Ordering::Relaxed);
    ui.window().on_close_requested(move || {
        if close_to_tray_flag.load(Ordering::Relaxed) {
            tracing::info!("window close requested; minimizing to tray");
            slint::CloseRequestResponse::HideWindow
        } else {
            tracing::info!("window close requested; quitting application");
            let _ = slint::quit_event_loop();
            slint::CloseRequestResponse::HideWindow
        }
    });

    // 4. Handle desktop actions (PresentWindow / Quit from D-Bus or Tray)
    let ui_weak = ui.as_weak();
    let desktop_action_thread = std::thread::Builder::new()
        .name("zenomen-desktop-actions".to_string())
        .spawn(move || {
            for action in action_receiver {
                let ui_weak = ui_weak.clone();
                match action {
                    DesktopAction::PresentWindow => {
                        let _ = slint::invoke_from_event_loop(move || {
                            if let Some(ui) = ui_weak.upgrade() {
                                let _ = ui.show();
                                ui.window().set_minimized(false);
                            }
                        });
                    }
                    DesktopAction::Quit => {
                        let _ = slint::invoke_from_event_loop(move || {
                            let _ = slint::quit_event_loop();
                        });
                        break;
                    }
                }
            }
        })
        .expect("spawning desktop action listener must succeed");

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
    if let Some(notice) = corrupt_notice {
        app.handle_event(zenomen::core::AppEvent::ConfigCorrupted(notice));
    }
    app.handle_event(zenomen::core::AppEvent::ConfigLoaded(config));
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

    // 5. Run event loop (start minimized or visible)
    if start_minimized {
        tracing::info!("starting minimized in system tray");
        slint::run_event_loop_until_quit()?;
    } else {
        ui.show()?;
        slint::run_event_loop_until_quit()?;
    }

    // Clean teardown: drop window and app to close channels and terminate worker/pump
    drop(ui);
    drop(app);
    let _ = worker.join();
    let _ = event_pump.join();
    let _ = desktop_action_thread.join();

    tracing::info!("ZenOmen shut down cleanly");
    Ok(())
}
