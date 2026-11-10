//! Non-blocking monitoring worker thread.
//!
//! Owns the sampling cadence (1 Hz deadline loop) and receives [`AppCommand`]s
//! from the UI via channel timeout.
//!
//! Two core guarantees:
//!
//! 1. Responsiveness to on-demand refresh (`01-REVIEWS.md:57`):
//!    The loop never sleeps with `thread::sleep`. Instead, it uses
//!    `command_receiver.recv_timeout(deadline - now)`. An incoming
//!    [`AppCommand::RefreshTelemetry`] breaks the wait immediately, samples the
//!    hardware, sends the snapshot, and resets the next deadline.
//! 2. Thread isolation (`PERF-03`, `FOUND-05`):
//!    All hardware provider sampling runs on this dedicated background thread.
//!    The Slint UI thread never touches hardware, never waits on sensor I/O,
//!    and only receives plain events through the bridge.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use crate::core::events::{AppCommand, AppEvent};
use crate::hardware::HardwareProvider;
use crate::service::HardwareServiceClient;

/// Target sampling interval: 1 Hz (1 second) per project specification.
pub const SAMPLING_INTERVAL: Duration = Duration::from_secs(1);

/// Spawns the dedicated monitoring worker thread.
///
/// Dispatches initial capabilities and first reading, then maintains the 1 Hz
/// sampling cadence while responding immediately to UI refresh requests.
pub fn spawn_monitoring_worker(
    provider: Arc<dyn HardwareProvider>,
    event_sender: Sender<AppEvent>,
    command_receiver: Receiver<AppCommand>,
) -> JoinHandle<()> {
    thread::Builder::new()
        .name("zenomen-monitor-worker".to_string())
        .spawn(move || {
            tracing::info!("monitoring worker started");

            // One hardware mutation at a time. Owned by this thread, not a
            // `static`: the worker's lifetime is the only lifetime it needs.
            let mutation_in_flight = Arc::new(AtomicBool::new(false));

            // 1. Initial report of hardware capabilities
            let capabilities = provider.capabilities();
            if event_sender
                .send(AppEvent::CapabilitiesLoaded(capabilities))
                .is_err()
            {
                tracing::warn!(
                    "event receiver dropped before initial capabilities; exiting worker"
                );
                return;
            }

            // 2. Hardware Service client initialization
            let client = crate::service::HardwareServiceClient::new();

            // 3. Initial sample on cold start so the UI doesn't remain empty for 1s
            if !sample_and_dispatch(&provider, &event_sender, &client) {
                tracing::warn!("event receiver dropped before initial sample; exiting worker");
                return;
            }

            // 4. Dispatch initial service probe status
            let initial_status = client.status();
            let _ = event_sender.send(AppEvent::ServiceStatusChanged(initial_status.clone()));

            if initial_status.is_connected() {
                let polkit = client
                    .check_polkit_status()
                    .unwrap_or_else(|e| format!("Check failed: {e}"));
                let _ = event_sender.send(AppEvent::PolkitStatusChanged(polkit));
                if let Ok((mode, _)) = client.get_fan_status() {
                    let _ = event_sender.send(AppEvent::FanModeChanged(Ok(mode)));
                }
                if let Ok(profile) = client.get_power_profile() {
                    let _ = event_sender.send(AppEvent::PowerProfileChanged(Ok(profile)));
                }
            } else {
                let _ = event_sender.send(AppEvent::PolkitStatusChanged(
                    "Service disconnected".to_string(),
                ));
            }

            // Initial dual-process memory sample (WIN-08 / MON-12)
            let init_gui_mem = crate::hardware::process_memory::sample_current_process_memory(
                crate::hardware::process_memory::ProcessRole::Gui,
            );
            let init_svc_mem = client.get_service_memory().unwrap_or_else(|_| {
                crate::hardware::process_memory::ProcessMemoryInfo {
                    role: Some(crate::hardware::process_memory::ProcessRole::HardwareService),
                    available: false,
                    ..Default::default()
                }
            });
            let _ = event_sender.send(AppEvent::MemoryTelemetryUpdated(
                crate::hardware::process_memory::ZenomenMemoryTelemetry::new(
                    init_gui_mem,
                    init_svc_mem,
                ),
            ));

            // 4. Deadline loop with channel timeout scheduling
            let (mut current_config, _) = crate::config::default_config_path()
                .as_deref()
                .map(crate::config::load_or_default)
                .unwrap_or_else(|| (crate::config::AppConfig::default(), None));
            let mut sampling_interval = Duration::from_millis(
                current_config
                    .telemetry
                    .polling_interval_ms
                    .clamp(500, 5000),
            );
            let mut deadline = Instant::now() + sampling_interval;
            let mut status_check_tick: u32 = 0;
            let mut memory_sample_tick: u32 = 0;
            let mut ui_visible = true;

            loop {
                let command_opt = if ui_visible {
                    let now = Instant::now();
                    let timeout = deadline.saturating_duration_since(now);
                    match command_receiver.recv_timeout(timeout) {
                        Ok(cmd) => Some(cmd),
                        Err(RecvTimeoutError::Timeout) => None,
                        Err(RecvTimeoutError::Disconnected) => {
                            tracing::debug!("command channel disconnected; exiting worker");
                            break;
                        }
                    }
                } else {
                    match command_receiver.recv() {
                        Ok(cmd) => Some(cmd),
                        Err(_) => {
                            tracing::debug!("command channel disconnected; exiting worker");
                            break;
                        }
                    }
                };

                if let Some(command) = command_opt {
                    match command {
                        AppCommand::SetUiVisible(visible) => {
                            tracing::debug!(visible, "worker UI visibility changed");
                            let was_hidden = !ui_visible;
                            ui_visible = visible;
                            if ui_visible && was_hidden {
                                // Immediate sample upon restoring window so UI frame is fresh
                                if !sample_and_dispatch(&provider, &event_sender, &client) {
                                    tracing::debug!("event receiver disconnected; exiting worker");
                                    break;
                                }
                                deadline = Instant::now() + sampling_interval;
                            }
                        }
                        AppCommand::RefreshTelemetry => {
                            tracing::debug!("immediate telemetry refresh triggered");
                            if !sample_and_dispatch(&provider, &event_sender, &client) {
                                tracing::debug!("event receiver disconnected; exiting worker");
                                break;
                            }
                            // Reset deadline to full interval after immediate sample
                            deadline = Instant::now() + sampling_interval;
                        }
                        AppCommand::Navigate(screen) => {
                            tracing::debug!(screen = screen.title(), "navigation forwarded");
                            if event_sender.send(AppEvent::ScreenChanged(screen)).is_err() {
                                tracing::debug!("event receiver disconnected; exiting worker");
                                break;
                            }
                        }
                        AppCommand::SetHistoryWindow(samples) => {
                            tracing::debug!(samples, "history window updated");
                            current_config.telemetry.history_window = samples;
                            current_config.sanitize();
                            if let Some(path) = crate::config::default_config_path() {
                                let _ = crate::config::save_atomic(&path, &current_config);
                            }
                            if event_sender
                                .send(AppEvent::HistoryWindowChanged(samples))
                                .is_err()
                            {
                                tracing::debug!("event receiver disconnected; exiting worker");
                                break;
                            }
                            let _ =
                                event_sender.send(AppEvent::ConfigLoaded(current_config.clone()));
                        }
                        AppCommand::UpdateConfig(config) => {
                            let mut new_config = config;
                            new_config.sanitize();
                            if let Some(path) = crate::config::default_config_path()
                                && let Err(e) = crate::config::save_atomic(&path, &new_config)
                            {
                                tracing::error!(error = %e, "failed to persist config");
                            }
                            let _ = crate::desktop::autostart::set_autostart_enabled(
                                new_config.general.autostart,
                            );
                            let ms = new_config.telemetry.polling_interval_ms;
                            sampling_interval = Duration::from_millis(ms);
                            deadline = Instant::now() + sampling_interval;
                            current_config = new_config.clone();
                            let _ = event_sender.send(AppEvent::ConfigLoaded(new_config));
                        }
                        AppCommand::SetPollingInterval(ms) => {
                            current_config.telemetry.polling_interval_ms = ms;
                            current_config.sanitize();
                            if let Some(path) = crate::config::default_config_path() {
                                let _ = crate::config::save_atomic(&path, &current_config);
                            }
                            sampling_interval =
                                Duration::from_millis(current_config.telemetry.polling_interval_ms);
                            deadline = Instant::now() + sampling_interval;
                            let _ =
                                event_sender.send(AppEvent::ConfigLoaded(current_config.clone()));
                        }
                        AppCommand::SetAutostart(enabled) => {
                            current_config.general.autostart = enabled;
                            let _ = crate::desktop::autostart::set_autostart_enabled(enabled);
                            if let Some(path) = crate::config::default_config_path() {
                                let _ = crate::config::save_atomic(&path, &current_config);
                            }
                            let _ =
                                event_sender.send(AppEvent::ConfigLoaded(current_config.clone()));
                        }
                        AppCommand::SetStartMinimized(enabled) => {
                            current_config.general.start_minimized = enabled;
                            if let Some(path) = crate::config::default_config_path() {
                                let _ = crate::config::save_atomic(&path, &current_config);
                            }
                            let _ =
                                event_sender.send(AppEvent::ConfigLoaded(current_config.clone()));
                        }
                        AppCommand::SetCloseToTray(enabled) => {
                            current_config.general.close_to_tray = enabled;
                            if let Some(path) = crate::config::default_config_path() {
                                let _ = crate::config::save_atomic(&path, &current_config);
                            }
                            let _ =
                                event_sender.send(AppEvent::ConfigLoaded(current_config.clone()));
                        }
                        AppCommand::SetFanMode(mode) => {
                            tracing::info!(mode = mode.as_str(), "applying fan mode via service");
                            // Announce the in-flight write before the D-Bus call:
                            // the Polkit prompt can take seconds, and the UI must
                            // not look idle while the hardware is being changed.
                            let _ = event_sender.send(AppEvent::FanModeRequested(mode.clone()));
                            let c = client.clone();
                            let tx = event_sender.clone();
                            execute_mutating_action(
                                "fan",
                                tx,
                                &mutation_in_flight,
                                move || c.set_fan_mode(&mode),
                                AppEvent::FanModeChanged,
                            );
                        }
                        AppCommand::SetPowerProfile(profile) => {
                            tracing::info!(
                                profile = profile.as_str(),
                                "applying power profile via service"
                            );
                            let c = client.clone();
                            let tx = event_sender.clone();
                            execute_mutating_action(
                                "power",
                                tx,
                                &mutation_in_flight,
                                move || c.set_power_profile(&profile),
                                AppEvent::PowerProfileChanged,
                            );
                        }
                        AppCommand::ApplyPreset(preset) => {
                            tracing::info!(preset = preset.as_str(), "applying preset via service");
                            let c = client.clone();
                            let tx = event_sender.clone();
                            execute_mutating_action(
                                "preset",
                                tx,
                                &mutation_in_flight,
                                move || c.apply_preset(&preset),
                                AppEvent::PresetApplied,
                            );
                        }
                        AppCommand::ApplyRgbProfile(profile) => {
                            tracing::info!(
                                profile = profile.name.as_str(),
                                "applying RGB profile via service"
                            );
                            let c = client.clone();
                            let tx = event_sender.clone();
                            let p = profile.clone();
                            execute_mutating_action(
                                "rgb-prof",
                                tx,
                                &mutation_in_flight,
                                move || c.apply_rgb_profile(&p),
                                move |res| AppEvent::RgbProfileApplied(res.map(|_| profile)),
                            );
                        }
                        AppCommand::SetRgbPower(enabled) => {
                            tracing::info!(enabled, "setting RGB power via service");
                            let c = client.clone();
                            let tx = event_sender.clone();
                            execute_mutating_action(
                                "rgb-pwr",
                                tx,
                                &mutation_in_flight,
                                move || c.set_rgb_enabled(enabled),
                                move |res| AppEvent::RgbPowerChanged(res.map(|_| enabled)),
                            );
                        }
                        AppCommand::DismissControlError => {
                            let _ = event_sender.send(AppEvent::ControlErrorDismissed);
                        }
                        AppCommand::Shutdown => {
                            tracing::debug!("shutdown command received; exiting worker");
                            break;
                        }
                    }
                } else {
                    // Periodic sample (reached only via timeout when ui_visible is true)
                    if !sample_and_dispatch(&provider, &event_sender, &client) {
                        tracing::debug!("event receiver disconnected; exiting worker");
                        break;
                    }

                    // Check service status every 5 ticks
                    status_check_tick = status_check_tick.wrapping_add(1);
                    if status_check_tick.is_multiple_of(5) {
                        let status = client.status();
                        let _ = event_sender.send(AppEvent::ServiceStatusChanged(status.clone()));
                        if status.is_connected() {
                            let polkit = client
                                .check_polkit_status()
                                .unwrap_or_else(|e| format!("Check failed: {e}"));
                            let _ = event_sender.send(AppEvent::PolkitStatusChanged(polkit));
                        } else {
                            let _ = event_sender.send(AppEvent::PolkitStatusChanged(
                                "Service disconnected".to_string(),
                            ));
                        }
                    }

                    // Sample dual-process memory every 2 ticks (~2s) (WIN-08 / MON-12)
                    memory_sample_tick = memory_sample_tick.wrapping_add(1);
                    if memory_sample_tick.is_multiple_of(2) {
                        let gui_mem =
                            crate::hardware::process_memory::sample_current_process_memory(
                                crate::hardware::process_memory::ProcessRole::Gui,
                            );
                        let service_mem = client.get_service_memory().unwrap_or_else(|_| {
                            crate::hardware::process_memory::ProcessMemoryInfo {
                                role: Some(
                                    crate::hardware::process_memory::ProcessRole::HardwareService,
                                ),
                                available: false,
                                ..Default::default()
                            }
                        });
                        let _ = event_sender.send(AppEvent::MemoryTelemetryUpdated(
                            crate::hardware::process_memory::ZenomenMemoryTelemetry::new(
                                gui_mem,
                                service_mem,
                            ),
                        ));
                    }

                    deadline += sampling_interval;
                    // Avoid deadline drift if stalled or suspended
                    if deadline <= Instant::now() {
                        deadline = Instant::now() + sampling_interval;
                    }
                }
            }

            tracing::info!("monitoring worker stopped");
        })
        .expect("spawning monitoring worker thread must succeed")
}

/// Sample telemetry from the provider and forward the snapshot to the event sender.
/// Returns `false` if the event receiver has been dropped, signaling worker termination.
fn sample_and_dispatch(
    provider: &Arc<dyn HardwareProvider>,
    event_sender: &Sender<AppEvent>,
    client: &HardwareServiceClient,
) -> bool {
    match provider.sample_telemetry() {
        Ok(mut snapshot) => {
            // On Windows (or where tachometers are managed via privileged service),
            // if provider has no fan tachometers, enrich with live fan telemetry from service.
            if snapshot.fan_rpms.is_empty()
                && client.status().is_connected()
                && let Ok((mode, rpms)) = client.get_fan_status()
            {
                if !rpms.is_empty() {
                    snapshot.fan_rpms = rpms;
                }
                if !mode.is_empty() {
                    snapshot.fan_mode = mode;
                }
            }
            event_sender
                .send(AppEvent::TelemetryUpdated(snapshot))
                .is_ok()
        }
        Err(err) => {
            tracing::warn!(%err, "telemetry sampling failed");
            event_sender
                .send(AppEvent::TelemetrySamplingFailed(err))
                .is_ok()
        }
    }
}

/// Offload mutating hardware operations with a 30s timeout to a dedicated background
/// worker, ensuring the monitoring loop is never blocked by polkit prompts (WR-01).
///
/// Only one mutation may be in flight. A request that arrives while another write
/// is still running is answered immediately with an error instead of queueing a
/// second write of the same sysfs attribute, which would otherwise apply out of
/// order (two rapid fan-mode toggles). The slot is released by a guard so a panic
/// in the RPC thread cannot leave the hardware permanently locked out.
fn execute_mutating_action<F, R>(
    name: &'static str,
    event_sender: Sender<AppEvent>,
    in_flight: &Arc<AtomicBool>,
    f: F,
    event_builder: impl FnOnce(Result<R, String>) -> AppEvent + Send + 'static,
) where
    F: FnOnce() -> Result<R, String> + Send + 'static,
    R: Send + 'static,
{
    execute_mutating_action_with_timeout(
        name,
        event_sender,
        in_flight,
        f,
        Duration::from_secs(30),
        event_builder,
    );
}

fn execute_mutating_action_with_timeout<F, R>(
    name: &'static str,
    event_sender: Sender<AppEvent>,
    in_flight: &Arc<AtomicBool>,
    f: F,
    timeout: Duration,
    event_builder: impl FnOnce(Result<R, String>) -> AppEvent + Send + 'static,
) where
    F: FnOnce() -> Result<R, String> + Send + 'static,
    R: Send + 'static,
{
    if in_flight.swap(true, Ordering::SeqCst) {
        tracing::warn!(
            action = name,
            "rejected: another hardware change is in flight"
        );
        let _ = event_sender.send(event_builder(Err(
            "Another hardware change is still in progress".to_string(),
        )));
        return;
    }

    let guard = InFlightGuard(Arc::clone(in_flight));
    let _ = std::thread::Builder::new()
        .name(format!("zenomen-ctrl-{name}"))
        .spawn(move || {
            let (tx, rx) = mpsc::channel();
            let _ = std::thread::Builder::new()
                .name(format!("zenomen-rpc-{name}"))
                .spawn(move || {
                    let res = f();
                    // Release the guard as soon as the hardware operation finishes.
                    drop(guard);
                    let _ = tx.send(res);
                });
            let res = match rx.recv_timeout(timeout) {
                Ok(r) => r,
                Err(mpsc::RecvTimeoutError::Timeout) => {
                    tracing::error!(
                        function = "execute_mutating_action",
                        action = name,
                        timeout_ms = timeout.as_millis(),
                        "mutating hardware action timed out"
                    );
                    Err(
                        "Operation timed out waiting for authorization or hardware response"
                            .to_string(),
                    )
                }
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    tracing::error!(
                        function = "execute_mutating_action",
                        action = name,
                        "mutating hardware RPC thread disconnected unexpectedly"
                    );
                    Err("Control RPC thread exited prematurely".to_string())
                }
            };
            if let Err(ref e) = res {
                tracing::error!(
                    function = "execute_mutating_action",
                    action = name,
                    error = %e,
                    "hardware mutating action failed"
                );
            } else if res.is_ok() {
                tracing::info!(
                    function = "execute_mutating_action",
                    action = name,
                    "hardware mutating action completed successfully"
                );
            }
            let _ = event_sender.send(event_builder(res));
        });
}

/// Releases the single in-flight slot when the mutating action ends, however it ends.
struct InFlightGuard(Arc<AtomicBool>);

impl Drop for InFlightGuard {
    fn drop(&mut self) {
        self.0.store(false, Ordering::SeqCst);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_second_mutation_is_rejected_while_one_is_in_flight() {
        let gate = Arc::new(AtomicBool::new(false));
        let (tx, rx) = mpsc::channel();

        // Hold the first action open until the test releases it.
        let hold = Arc::new(AtomicBool::new(true));
        let hold_for_action = Arc::clone(&hold);
        execute_mutating_action(
            "hold",
            tx.clone(),
            &gate,
            move || {
                while hold_for_action.load(Ordering::SeqCst) {
                    thread::sleep(Duration::from_millis(2));
                }
                Ok("max".to_string())
            },
            AppEvent::FanModeChanged,
        );

        // Arrives while the first write is still running.
        execute_mutating_action(
            "reject",
            tx.clone(),
            &gate,
            || Ok("auto".to_string()),
            AppEvent::FanModeChanged,
        );

        match rx
            .recv_timeout(Duration::from_secs(2))
            .expect("rejection must be reported")
        {
            AppEvent::FanModeChanged(Err(msg)) => {
                assert!(msg.contains("still in progress"), "message: {msg}");
            }
            other => panic!("expected a rejection, got {other:?}"),
        }

        hold.store(false, Ordering::SeqCst);
        assert_eq!(
            rx.recv_timeout(Duration::from_secs(2))
                .expect("first action must complete"),
            AppEvent::FanModeChanged(Ok("max".to_string()))
        );

        // The slot was released, so a later change is accepted.
        execute_mutating_action(
            "after",
            tx,
            &gate,
            || Ok("auto".to_string()),
            AppEvent::FanModeChanged,
        );
        assert_eq!(
            rx.recv_timeout(Duration::from_secs(2))
                .expect("follow-up action must run"),
            AppEvent::FanModeChanged(Ok("auto".to_string()))
        );
    }

    #[test]
    fn in_flight_guard_remains_active_across_caller_timeout_until_operation_finishes() {
        let gate = Arc::new(AtomicBool::new(false));
        let (tx, rx) = mpsc::channel();

        let hold = Arc::new(AtomicBool::new(true));
        let hold_for_action = Arc::clone(&hold);

        // Run with a very short timeout (30ms) while the hardware action is held for longer
        execute_mutating_action_with_timeout(
            "slow-hw",
            tx.clone(),
            &gate,
            move || {
                while hold_for_action.load(Ordering::SeqCst) {
                    thread::sleep(Duration::from_millis(5));
                }
                Ok("done".to_string())
            },
            Duration::from_millis(30),
            AppEvent::FanModeChanged,
        );

        // Caller receives timeout error event
        match rx
            .recv_timeout(Duration::from_secs(2))
            .expect("timeout event must be sent")
        {
            AppEvent::FanModeChanged(Err(msg)) => {
                assert!(
                    msg.contains("timed out"),
                    "expected timeout error, got: {msg}"
                );
            }
            other => panic!("expected timeout error, got: {other:?}"),
        }

        // Even though the caller timed out, the hardware operation is still running!
        // The gate must STILL be active (true).
        assert!(
            gate.load(Ordering::SeqCst),
            "guard must stay active while the slow hardware write continues in the background"
        );

        // Any second mutation during this time MUST be rejected
        execute_mutating_action(
            "concurrent-after-timeout",
            tx.clone(),
            &gate,
            || Ok("rejected".to_string()),
            AppEvent::FanModeChanged,
        );
        match rx
            .recv_timeout(Duration::from_secs(1))
            .expect("concurrent request must be rejected")
        {
            AppEvent::FanModeChanged(Err(msg)) => {
                assert!(msg.contains("still in progress"));
            }
            other => panic!("expected rejection, got {other:?}"),
        }

        // Now let the original operation complete
        hold.store(false, Ordering::SeqCst);

        // Wait a moment for the rpc thread to finish and drop the guard
        let deadline = Instant::now() + Duration::from_secs(2);
        while gate.load(Ordering::SeqCst) && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(5));
        }

        assert!(
            !gate.load(Ordering::SeqCst),
            "guard must be released after the hardware operation finishes"
        );
    }
}
