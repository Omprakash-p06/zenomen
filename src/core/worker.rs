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

            // 3. Deadline loop with channel timeout scheduling
            let mut deadline = Instant::now() + SAMPLING_INTERVAL;

            loop {
                let now = Instant::now();
                let timeout = deadline.saturating_duration_since(now);

                match command_receiver.recv_timeout(timeout) {
                    Ok(command) => match command {
                        AppCommand::RefreshTelemetry => {
                            tracing::debug!("immediate telemetry refresh triggered");
                            if !sample_and_dispatch(&provider, &event_sender) {
                                tracing::debug!("event receiver disconnected; exiting worker");
                                break;
                            }
                            // Reset deadline to full interval after immediate sample
                            deadline = Instant::now() + SAMPLING_INTERVAL;
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
                        }
                    },
                    Err(RecvTimeoutError::Timeout) => {
                        // Periodic 1 Hz sample
                        if !sample_and_dispatch(&provider, &event_sender) {
                            tracing::debug!("event receiver disconnected; exiting worker");
                            break;
                        }

                        deadline += SAMPLING_INTERVAL;
                        // Avoid deadline drift if stalled or suspended
                        if deadline <= Instant::now() {
                            deadline = Instant::now() + SAMPLING_INTERVAL;
                        }
                    }
                    Err(RecvTimeoutError::Disconnected) => {
                        tracing::debug!("command channel disconnected; exiting worker");
                        break;
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
            true
        }
    }
}
