//! The one bridge between the Slint UI and the core.
//!
//! Two directions, no third:
//!
//! * UI -> core: a Slint callback becomes an [`AppCommand`] and is *queued*.
//!   Callbacks never read a sensor, never block, and never wait for an answer
//!   (FOUND-05, threat T-01-03). If the queue is full the request is dropped
//!   and logged — a saturated queue must cost a click, not a frozen window.
//! * core -> UI: an [`AppEvent`] is folded into [`AppState`] and then published
//!   inside [`slint::invoke_from_event_loop`]. Every visible change goes through
//!   that closure, so the UI can never be mutated from a worker thread.
//!
//! The window is held as a `Weak` on purpose: a background thread must not be
//! able to keep a closed window alive.

use std::sync::mpsc::{self, Receiver, SyncSender, TrySendError};
use std::sync::{Arc, Mutex, MutexGuard};

use slint::{ComponentHandle, ModelRc, SharedString, VecModel, Weak};

use crate::AppWindow;
use crate::core::{AppCommand, AppEvent, AppState, ScreenId};
use crate::hardware::registry::SensorRegistry;
use crate::hardware::{HardwareCapabilities, TelemetrySnapshot};

/// Commands the core may have queued before the UI's request is dropped.
///
/// Small on purpose: a person can press a key a handful of times before the
/// core catches up, and anything beyond that is a stuck core, not a user need.
const COMMAND_QUEUE_DEPTH: usize = 8;

/// The Slint-side half of the application.
///
/// Cheap to clone (a `Weak` and `Arc`s), which is what lets a worker thread
/// publish events without owning the window.
#[derive(Clone)]
pub struct ZenOmenApp {
    ui: Weak<AppWindow>,
    commands: SyncSender<AppCommand>,
    state: Arc<Mutex<AppState>>,
    registry: Arc<Mutex<Option<Arc<SensorRegistry>>>>,
}

impl ZenOmenApp {
    /// Build the bridge for `ui`, returning the app and the receiving end of the
    /// command channel. The caller owns the receiver — this type never runs a
    /// command loop of its own.
    pub fn new(ui: &AppWindow) -> (Self, Receiver<AppCommand>) {
        let (commands, command_receiver) = mpsc::sync_channel(COMMAND_QUEUE_DEPTH);

        let app = Self {
            ui: ui.as_weak(),
            commands,
            state: Arc::new(Mutex::new(AppState::default())),
            registry: Arc::new(Mutex::new(None)),
        };

        (app, command_receiver)
    }

    /// Store the canonical sensor registry and populate initial diagnostics properties.
    pub fn set_sensor_registry(&self, registry: Arc<SensorRegistry>) {
        if let Some(ui) = self.ui.upgrade() {
            if let Some(cpu) = &registry.cpu_temp {
                ui.set_cpu_sensor(format!("{} [{}]", cpu.driver, cpu.label).into());
            } else {
                ui.set_cpu_sensor("None detected".into());
            }
            ui.set_fan_count(registry.fan_tachometers.len().to_string().into());
        }
        *self.registry.lock().unwrap_or_else(|p| p.into_inner()) = Some(registry);
    }

    /// Turn the window's callbacks into queued commands.
    ///
    /// Must be called on the UI thread, once, before `run()`.
    pub fn install_callbacks(&self) {
        let ui = self.ui.clone();
        let commands = self.commands.clone();
        if let Some(ui) = ui.upgrade() {
            ui.on_navigate(move |index| {
                // The UI hands over a number; only `ScreenId` can turn it back
                // into a screen, and an unknown number is refused rather than
                // clamped into a wrong screen (threat T-01-04).
                match usize::try_from(index).ok().and_then(ScreenId::from_index) {
                    Some(screen) => send_command(&commands, AppCommand::Navigate(screen)),
                    None => tracing::warn!(index, "ignoring navigation to an unknown screen"),
                }
            });
        }

        let ui = self.ui.clone();
        let commands = self.commands.clone();
        if let Some(ui) = ui.upgrade() {
            ui.on_refresh_telemetry(move || {
                send_command(&commands, AppCommand::RefreshTelemetry);
            });
        }

        let ui = self.ui.clone();
        let commands = self.commands.clone();
        if let Some(ui) = ui.upgrade() {
            ui.on_history_window_changed(move |seconds| {
                if seconds >= 2 {
                    send_command(&commands, AppCommand::SetHistoryWindow(seconds as usize));
                }
            });
        }

        let ui = self.ui.clone();
        let commands = self.commands.clone();
        if let Some(ui) = ui.upgrade() {
            ui.on_set_fan_mode(move |mode| {
                send_command(&commands, AppCommand::SetFanMode(mode.to_string()));
            });
        }

        let ui = self.ui.clone();
        let commands = self.commands.clone();
        if let Some(ui) = ui.upgrade() {
            ui.on_dismiss_control_error(move || {
                send_command(&commands, AppCommand::DismissControlError);
            });
        }

        let ui = self.ui.clone();
        let state = Arc::clone(&self.state);
        let registry_slot = Arc::clone(&self.registry);
        if let Some(ui) = ui.upgrade() {
            let ui_weak = self.ui.clone();
            ui.on_export_diagnostics(move || {
                let current_state = lock_state(&state).clone();
                let reg_guard = registry_slot.lock().unwrap_or_else(|p| p.into_inner());
                let reg = reg_guard.clone();
                drop(reg_guard);

                let ui_weak = ui_weak.clone();
                let _ = std::thread::Builder::new()
                    .name("zenomen-diag-export".to_string())
                    .spawn(move || {
                        let report =
                            crate::core::collect_diagnostics(&current_state, reg.as_deref());

                        let export_dir =
                            directories::ProjectDirs::from("com", "zenomen", "zenomen")
                                .map(|dirs| dirs.config_dir().join("diagnostics"))
                                .unwrap_or_else(|| std::path::PathBuf::from("diagnostics"));

                        let filename = format!(
                            "zenomen-diagnostics-{}.json",
                            report.timestamp.replace(':', "-")
                        );
                        let export_path = export_dir.join(filename);

                        let status_msg =
                            match crate::core::export_diagnostics_to_file(&report, &export_path) {
                                Ok(()) => {
                                    tracing::info!(path = %export_path.display(), "diagnostics report exported successfully");
                                    format!("Exported to {}", export_path.display())
                                }
                                Err(err) => {
                                    tracing::error!(error = %err, "failed to export diagnostics report");
                                    format!("Export failed: {err}")
                                }
                            };

                        let _ = slint::invoke_from_event_loop(move || {
                            if let Some(ui) = ui_weak.upgrade() {
                                ui.set_export_status(status_msg.into());
                            }
                        });
                    });
            });
        }
    }

    /// Publish the provider's capability set before the event loop starts.
    ///
    /// Called on the UI thread with no loop running yet, so the properties are
    /// written directly instead of being queued. That keeps the first frame
    /// honest: the window opens already knowing which panels are unavailable,
    /// rather than flashing nominal values for one frame. Every later update
    /// goes through [`Self::handle_event`].
    pub fn initialize(&self, capabilities: HardwareCapabilities) {
        let event = AppEvent::CapabilitiesLoaded(capabilities);

        {
            let mut state = lock_state(&self.state);
            state.apply(&event);
        }

        if let Some(ui) = self.ui.upgrade() {
            let report = crate::core::collect_diagnostics(&lock_state(&self.state), None);
            ui.set_os_name(report.os_name.into());
            ui.set_kernel_version(report.kernel_version.into());
            if let Some(prod) = report.product_name {
                ui.set_model_name(prod.into());
            } else if let Some(vendor) = report.sys_vendor {
                ui.set_model_name(vendor.into());
            }
            apply_to_ui(&ui, &lock_state(&self.state), &event);
        }
    }

    /// Fold one event into the core state and publish it to the UI.
    ///
    /// Safe to call from any thread. The state update happens immediately on the
    /// caller's thread so the core is always the first to know; the UI update is
    /// queued onto the event loop.
    pub fn handle_event(&self, event: AppEvent) {
        {
            let mut state = lock_state(&self.state);
            state.apply(&event);
        }

        let ui = self.ui.clone();
        let state = Arc::clone(&self.state);
        let queued = slint::invoke_from_event_loop(move || {
            // The window is gone; the state stays current for a restart.
            let Some(ui) = ui.upgrade() else {
                return;
            };
            apply_to_ui(&ui, &lock_state(&state), &event);
        });

        if let Err(error) = queued {
            tracing::warn!(%error, "could not reach the Slint event loop; UI not updated");
        }
    }
}

/// Queue a command without ever blocking the UI thread.
fn send_command(commands: &SyncSender<AppCommand>, command: AppCommand) {
    match commands.try_send(command) {
        Ok(()) => {}
        // Dropping is the intended back-pressure: a repeated refresh press is
        // worth nothing once one is already pending (threat T-01-03).
        Err(TrySendError::Full(dropped)) => {
            tracing::debug!(?dropped, "command queue full; request dropped");
        }
        Err(TrySendError::Disconnected(dropped)) => {
            tracing::warn!(?dropped, "command receiver is gone; request dropped");
        }
    }
}

/// Lock the shared state, tolerating a poisoned mutex.
///
/// The guarded value is plain data with no invariants to break, so a panic in
/// another thread cannot make it unsafe to read; refusing to update the window
/// because of it would be worse than continuing.
fn lock_state(state: &Mutex<AppState>) -> MutexGuard<'_, AppState> {
    state
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Write the core's state into the window's properties.
///
/// Only formatting happens here: the UI receives strings it can display and
/// never a number it would have to interpret.
fn apply_to_ui(ui: &AppWindow, state: &AppState, event: &AppEvent) {
    // Navigation is applied only when the core asked for it. Publishing the
    // pointer on every event would fight the sidebar: the UI moves to a screen
    // the moment it is clicked, and the command that records it travels
    // asynchronously.
    if let AppEvent::ScreenChanged(screen) = event {
        ui.set_active_screen(screen.index() as i32);
    }
    if let AppEvent::HistoryWindowChanged(window_size) = event {
        ui.set_history_window(*window_size as i32);
    }

    let capabilities = state.capabilities;
    ui.set_has_cpu_metrics(capabilities.has_cpu_metrics());
    ui.set_has_gpu_metrics(capabilities.has_gpu_metrics());
    ui.set_has_ram_metrics(capabilities.has_ram_metrics());
    ui.set_has_fan_rpm(capabilities.has_fan_rpm());
    ui.set_has_fan_control(capabilities.has_fan_control());
    ui.set_has_power_profiles(capabilities.has_power_profiles());
    ui.set_has_rgb_lighting(capabilities.has_rgb_lighting());
    ui.set_has_battery_metrics(capabilities.has_battery_metrics());

    ui.set_status_text(state.status_text().into());
    ui.set_status_degraded(state.is_degraded());

    ui.set_service_connected(state.service_status.is_connected());
    let active_mode = if !state.fan_mode.is_empty() {
        state.fan_mode.as_str()
    } else {
        state
            .latest_snapshot
            .as_ref()
            .map(|s| s.fan_mode.as_str())
            .unwrap_or("")
    };
    ui.set_fan_mode(active_mode.into());
    ui.set_pending_fan_mode(state.pending_fan_mode.as_deref().unwrap_or("").into());
    ui.set_control_error_message(state.last_control_error.as_deref().unwrap_or("").into());

    ui.set_cpu_path(state.paths.cpu_path.as_str().into());
    ui.set_gpu_path(state.paths.gpu_path.as_str().into());
    ui.set_ram_path(state.paths.ram_path.as_str().into());
    ui.set_cpu_sparkline(state.paths.cpu_sparkline.as_str().into());
    ui.set_gpu_sparkline(state.paths.gpu_sparkline.as_str().into());
    ui.set_ram_sparkline(state.paths.ram_sparkline.as_str().into());

    // Before the first sample the properties keep their empty values, which the
    // screens render as "Waiting for first reading" — never as a zero.
    let Some(snapshot) = &state.latest_snapshot else {
        return;
    };

    ui.set_cpu_utilization(percent(snapshot.cpu_utilization));
    ui.set_gpu_utilization(snapshot.gpu_utilization.map(percent).unwrap_or_default());
    ui.set_gpu_temperature(snapshot.gpu_temperature_c.map(degrees).unwrap_or_default());

    // A zero total means the memory sensor was not read at all; rendering
    // "0 MB used" would be a fabricated reading.
    ui.set_ram_usage(if snapshot.ram_total_mb == 0 {
        SharedString::new()
    } else {
        snapshot.ram_used_mb.to_string().into()
    });

    ui.set_fan_1_value(fan_speed(snapshot, 0));
    ui.set_fan_2_value(fan_speed(snapshot, 1));
    ui.set_fan_mode(snapshot.fan_mode.as_str().into());
    ui.set_power_profile(snapshot.power_profile.as_str().into());
    ui.set_power_profile_choices(string_model(&snapshot.power_profiles));
    ui.set_rgb_profile(snapshot.rgb_profile.as_deref().unwrap_or_default().into());
}

/// A percentage without its sign — the screens append `%` themselves.
fn percent(value: f32) -> SharedString {
    format!("{value:.0}").into()
}

/// A temperature without its unit — the screens append `°C` themselves.
fn degrees(value: f32) -> SharedString {
    format!("{value:.0}").into()
}

/// The tachometer reading at `index`, or an empty string when that fan is
/// absent. Empty is "not reported"; it is not "0 RPM".
fn fan_speed(snapshot: &TelemetrySnapshot, index: usize) -> SharedString {
    snapshot
        .fan_rpms
        .get(index)
        .map(|rpm| rpm.to_string().into())
        .unwrap_or_default()
}

/// Convert owned strings into the list model Slint expects for a `[string]`.
fn string_model(values: &[String]) -> ModelRc<SharedString> {
    let items: Vec<SharedString> = values.iter().map(|value| value.as_str().into()).collect();
    ModelRc::new(VecModel::from(items))
}
