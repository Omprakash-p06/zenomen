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
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use crate::core::events::{AppCommand, AppEvent};
use crate::hardware::HardwareProvider;

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

            // 2. Initial sample on cold start so the UI doesn't remain empty for 1s
            if !sample_and_dispatch(&provider, &event_sender) {
                tracing::warn!("event receiver dropped before initial sample; exiting worker");
                return;
            }

            // 3. Hardware Service client initialization and initial probe
            let client = crate::service::HardwareServiceClient::new();
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
                                if !sample_and_dispatch(&provider, &event_sender) {
                                    tracing::debug!("event receiver disconnected; exiting worker");
                                    break;
                                }
                                deadline = Instant::now() + sampling_interval;
                            }
                        }
                        AppCommand::RefreshTelemetry => {
                            tracing::debug!("immediate telemetry refresh triggered");
                            if !sample_and_dispatch(&provider, &event_sender) {
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
                            let res = client.set_fan_mode(&mode);
                            if event_sender.send(AppEvent::FanModeChanged(res)).is_err() {
                                break;
                            }
                        }
                        AppCommand::SetPowerProfile(profile) => {
                            tracing::info!(
                                profile = profile.as_str(),
                                "applying power profile via service"
                            );
                            let res = client.set_power_profile(&profile);
                            if event_sender
                                .send(AppEvent::PowerProfileChanged(res))
                                .is_err()
                            {
                                break;
                            }
                        }
                        AppCommand::ApplyPreset(preset) => {
                            tracing::info!(preset = preset.as_str(), "applying preset via service");
                            let res = client.apply_preset(&preset);
                            if event_sender.send(AppEvent::PresetApplied(res)).is_err() {
                                break;
                            }
                        }
                        AppCommand::ApplyRgbProfile(profile) => {
                            tracing::info!(
                                profile = profile.name.as_str(),
                                "applying RGB profile via service"
                            );
                            let res = client.apply_rgb_profile(&profile);
                            let event_res = res.map(|_| profile);
                            if event_sender
                                .send(AppEvent::RgbProfileApplied(event_res))
                                .is_err()
                            {
                                break;
                            }
                        }
                        AppCommand::SetRgbPower(enabled) => {
                            tracing::info!(enabled, "setting RGB power via service");
                            let res = client.set_rgb_enabled(enabled);
                            let event_res = res.map(|_| enabled);
                            if event_sender
                                .send(AppEvent::RgbPowerChanged(event_res))
                                .is_err()
                            {
                                break;
                            }
                        }
                        AppCommand::DismissControlError => {
                            let _ = event_sender.send(AppEvent::ControlErrorDismissed);
                        }
                    }
                } else {
                    // Periodic sample (reached only via timeout when ui_visible is true)
                    if !sample_and_dispatch(&provider, &event_sender) {
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
) -> bool {
    match provider.sample_telemetry() {
        Ok(snapshot) => event_sender
            .send(AppEvent::TelemetryUpdated(snapshot))
            .is_ok(),
        Err(err) => {
            tracing::warn!(%err, "telemetry sampling failed");
            event_sender
                .send(AppEvent::TelemetrySamplingFailed(err))
                .is_ok()
        }
    }
}
