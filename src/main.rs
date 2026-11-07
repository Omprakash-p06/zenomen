//! ZenOmen — lightweight hardware control center for HP OMEN and Victus laptops.
//!
//! Main entry point: manages single-instance enforcement on session D-Bus,
//! system tray integration, close-to-tray lifecycle, and monitoring worker orchestration.

use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::sync::mpsc;

use slint::ComponentHandle;
use zenomen::AppWindow;
use zenomen::app::ZenOmenApp;
use zenomen::core::{log_benchmark, spawn_monitoring_worker};
use zenomen::desktop::{
    DesktopAction, SingleInstanceResult, ensure_single_instance, register_tray,
};
use zenomen::hardware::{HardwareProvider, MockHardware};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Install logging before anything that can log. A GUI launched from a
    // desktop entry has no terminal, so stderr alone leaves no evidence; the
    // file layer is what makes a support report possible. Held for the process
    // lifetime so the non-blocking writer flushes on exit.
    let _logging_guard = zenomen::logging::init();

    // Ensure UTF-8 locale for xkbcommon Compose parsing when host locale lacks charset suffix
    #[cfg(target_os = "linux")]
    if std::env::var("LC_CTYPE").is_err()
        && std::env::var("LC_ALL").is_err()
        && let Ok(lang) = std::env::var("LANG")
        && !lang.to_lowercase().contains("utf-8")
        && !lang.to_lowercase().contains("utf8")
    {
        // SAFETY: setting LC_CTYPE before starting any worker threads or window creation
        unsafe {
            std::env::set_var("LC_CTYPE", "C.UTF-8");
        }
    }

    // Check for root-only system integration commands before launching GUI (REL-03)
    if std::env::args().any(|a| a == "--install-system-integration") {
        match zenomen::desktop::install_system_integration() {
            Ok(()) => {
                println!("ZenOmen system integration installed successfully.");
                std::process::exit(0);
            }
            Err(e) => {
                eprintln!("Error installing system integration: {e}");
                std::process::exit(1);
            }
        }
    }

    if std::env::args().any(|a| a == "--uninstall-system-integration") {
        match zenomen::desktop::uninstall_system_integration() {
            Ok(()) => {
                println!("ZenOmen system integration uninstalled successfully.");
                std::process::exit(0);
            }
            Err(e) => {
                eprintln!("Error uninstalling system integration: {e}");
                std::process::exit(1);
            }
        }
    }

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
            let _ = register_tray(&conn, action_sender.clone());
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
    drop(action_sender);

    tracing::info!(version = env!("CARGO_PKG_VERSION"), "starting ZenOmen");
    log_benchmark("cold-start");

    // Migrate legacy OpenOMEN RGB profiles non-destructively if present (RGB-05, IN-08)
    if let (Some(source_dir), Some(target_dir)) = (
        zenomen::rgb::default_openomen_rgb_dir(),
        zenomen::rgb::default_zenomen_rgb_dir(),
    ) {
        let report = zenomen::rgb::migrate_openomen_profiles(&source_dir, &target_dir);
        if report.total_scanned > 0 {
            tracing::info!(
                scanned = report.total_scanned,
                imported = report.imported.len(),
                preserved = report.preserved.len(),
                skipped = report.skipped.len(),
                "OpenOMEN RGB profiles migration report"
            );
        }
    }

    let ui = AppWindow::new()?;
    ui.set_app_version(env!("CARGO_PKG_VERSION").into());

    let (app, commands) = ZenOmenApp::new(&ui);
    let command_sender = app.command_sender();
    app.install_callbacks();

    // 3. Close-to-tray lifecycle management (SYS-04)
    let close_to_tray_flag = app.close_to_tray_flag();
    close_to_tray_flag.store(config.general.close_to_tray, Ordering::Relaxed);
    let close_commands = command_sender.clone();
    ui.window().on_close_requested(move || {
        if close_to_tray_flag.load(Ordering::Relaxed) {
            tracing::info!("window close requested; minimizing to tray");
            let _ = close_commands.try_send(zenomen::core::AppCommand::SetUiVisible(false));
            log_benchmark("tray-minimized");
            slint::CloseRequestResponse::HideWindow
        } else {
            tracing::info!("window close requested; quitting application");
            let _ = slint::quit_event_loop();
            slint::CloseRequestResponse::HideWindow
        }
    });

    // 4. Handle desktop actions (PresentWindow / Quit from D-Bus or Tray)
    let ui_weak = ui.as_weak();
    let present_commands = command_sender.clone();
    let desktop_action_thread = std::thread::Builder::new()
        .name("zenomen-desktop-actions".to_string())
        .spawn(move || {
            for action in action_receiver {
                let ui_weak = ui_weak.clone();
                let present_commands = present_commands.clone();
                match action {
                    DesktopAction::PresentWindow => {
                        let _ = slint::invoke_from_event_loop(move || {
                            if let Some(ui) = ui_weak.upgrade() {
                                let _ = ui.show();
                                ui.window().set_minimized(false);
                                let _ = present_commands
                                    .try_send(zenomen::core::AppCommand::SetUiVisible(true));
                                log_benchmark("dashboard-restored");
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
    let mock_scenario = std::env::var("ZENOMEN_MOCK_SCENARIO").ok().and_then(|val| {
        let trimmed = val.trim();
        if trimmed.is_empty() {
            None
        } else if let Some(scenario) = zenomen::hardware::MockScenario::from_name(trimmed) {
            Some(scenario)
        } else {
            tracing::warn!(
                value = %trimmed,
                "unrecognized ZENOMEN_MOCK_SCENARIO value; ignoring and using real Linux provider"
            );
            None
        }
    });

    let (provider, scenario_name, registry_opt): (
        Arc<dyn HardwareProvider>,
        Option<&'static str>,
        Option<Arc<zenomen::hardware::SensorRegistry>>,
    ) = if let Some(scenario) = mock_scenario {
        let mock = Arc::new(MockHardware::new(scenario));
        let scenario_str = mock.scenario().name();
        (mock, Some(scenario_str), None)
    } else {
        #[cfg(target_os = "linux")]
        {
            let linux = Arc::new(zenomen::hardware::LinuxHardwareProvider::new());
            let reg = Arc::clone(linux.registry());
            (linux, None, Some(reg))
        }
        #[cfg(not(target_os = "linux"))]
        {
            // No backend yet: report nothing instead of reaching for sysfs.
            (
                Arc::new(zenomen::hardware::UnavailableHardwareProvider::new())
                    as Arc<dyn HardwareProvider>,
                None,
                None,
            )
        }
    };

    if let Some(scenario) = scenario_name {
        tracing::info!(scenario, "mock hardware provider initialized");
        ui.set_provider_mode("mock".into());
        app.set_provider_label("Mock provider");
    } else {
        #[cfg(target_os = "linux")]
        {
            tracing::info!("linux hardware provider initialized");
            ui.set_provider_mode("linux".into());
            app.set_provider_label("Linux provider");
        }
        #[cfg(not(target_os = "linux"))]
        {
            tracing::warn!("no hardware provider is implemented for this platform");
            ui.set_provider_mode("unavailable".into());
            app.set_provider_label("No hardware provider");
        }
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
        let _ = command_sender.try_send(zenomen::core::AppCommand::SetUiVisible(false));
        log_benchmark("tray-minimized");
        slint::run_event_loop_until_quit()?;
    } else {
        ui.show()?;
        slint::run_event_loop_until_quit()?;
    }

    // Clean teardown: explicit shutdown signal, then drop senders in dependency order (CR-01)
    let _ = command_sender.try_send(zenomen::core::AppCommand::Shutdown);
    drop(command_sender);
    drop(ui);
    drop(app);
    let _ = worker.join();
    let _ = event_pump.join();
    drop(_session_conn);
    let _ = desktop_action_thread.join();

    tracing::info!("ZenOmen shut down cleanly");
    Ok(())
}
