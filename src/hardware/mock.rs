//! Configurable mock hardware provider.
//!
//! Phase 1 has no detector yet, so every reading the window shows comes from
//! here (FOUND-03). The provider is deliberately boring — sine waves around a
//! plausible OMEN laptop — because its job is to prove the whole path from
//! provider to chart without the target machine, and to make the failure paths
//! (`ZeroCapabilities`, `SensorDataGap`) reproducible anywhere.
//!
//! The scenario is chosen by an environment variable so the degraded and gap
//! paths are exercised by launching the real binary rather than by a test-only
//! build:
//!
//! ```text
//! ZENOMEN_MOCK_SCENARIO=zero  cargo run
//! ZENOMEN_MOCK_SCENARIO=gap   cargo run
//! ```
//!
//! The provider is `Send + Sync`: the monitoring worker samples it from its own
//! thread, and the tick counter — the only mutable state — lives in an atomic.

use std::sync::atomic::{AtomicU64, Ordering};

use crate::hardware::provider::{HardwareCapabilities, HardwareProvider, TelemetrySnapshot};

/// How many ticks make one gap cycle, and how many of those ticks lose the GPU.
///
/// Ten ticks at 1 Hz means a visible gap roughly every ten seconds; three
/// ticks is long enough that the chart break and the resume are both visible
/// without the series spending most of its time missing.
const GAP_PERIOD_TICKS: u64 = 10;
const GAP_TICKS: u64 = 3;

/// Installed memory the nominal scenario reports.
const RAM_TOTAL_MB: u64 = 16_384;

const CPU_MODEL: &str = "AMD Ryzen 7 5800H";
const GPU_MODEL: &str = "NVIDIA GeForce RTX 3060 Laptop GPU";

/// Which behaviour the mock reports.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum MockScenario {
    /// Every interface present, readings oscillating in plausible ranges.
    #[default]
    Nominal,
    /// No interface present: every screen must render its unavailable state.
    ZeroCapabilities,
    /// Nominal, except the GPU disappears for three ticks out of every ten.
    SensorDataGap,
}

impl MockScenario {
    /// Every scenario, in declaration order.
    ///
    /// Used by tests and by the warning an unrecognised name produces, so a new
    /// variant cannot be added without being listed here.
    pub const ALL: [Self; 3] = [Self::Nominal, Self::ZeroCapabilities, Self::SensorDataGap];

    /// Parse a scenario name, accepting the enum spelling, lowercase, and the
    /// short aliases used in the launch examples above.
    pub fn from_name(name: &str) -> Option<Self> {
        match name.trim().to_ascii_lowercase().as_str() {
            "" | "nominal" => Some(Self::Nominal),
            "zero" | "zerocapabilities" | "zero-capabilities" => Some(Self::ZeroCapabilities),
            "gap" | "sensordata" | "sensordata-gap" | "sensor-data-gap" => {
                Some(Self::SensorDataGap)
            }
            _ => None,
        }
    }

    /// The scenario named by `ZENOMEN_MOCK_SCENARIO`, defaulting to nominal.
    ///
    /// An unrecognised name is a typo, not a fatal error: the app still launches
    /// on the nominal mock and the warning says what was ignored.
    pub fn from_env() -> Self {
        match std::env::var("ZENOMEN_MOCK_SCENARIO") {
            Ok(name) => Self::from_name(&name).unwrap_or_else(|| {
                tracing::warn!(
                    value = %name,
                    accepted = ?Self::ALL,
                    "unknown ZENOMEN_MOCK_SCENARIO; using nominal"
                );
                Self::Nominal
            }),
            Err(_) => Self::Nominal,
        }
    }

    /// Stable lowercase name, for logs and for the Settings screen later.
    pub fn name(self) -> &'static str {
        match self {
            Self::Nominal => "nominal",
            Self::ZeroCapabilities => "zero-capabilities",
            Self::SensorDataGap => "sensor-data-gap",
        }
    }
}

/// A provider that invents plausible readings instead of reading hardware.
///
/// Cheap to construct and share: the scenario is a `Copy` enum and the tick is
/// an atomic, so the worker can hold it behind an `Arc` with no lock.
#[derive(Debug)]
pub struct MockHardware {
    scenario: MockScenario,
    tick: AtomicU64,
}

impl MockHardware {
    /// Build a provider for `scenario`, starting at tick zero.
    pub fn new(scenario: MockScenario) -> Self {
        Self {
            scenario,
            tick: AtomicU64::new(0),
        }
    }

    /// Build the provider selected by `ZENOMEN_MOCK_SCENARIO`.
    pub fn from_env() -> Self {
        Self::new(MockScenario::from_env())
    }

    /// Which scenario this provider reports.
    pub fn scenario(&self) -> MockScenario {
        self.scenario
    }

    /// How many samples have been taken so far.
    pub fn tick_count(&self) -> u64 {
        self.tick.load(Ordering::Relaxed)
    }

    /// Claim the next tick number.
    ///
    /// `Relaxed` is sufficient: the counter only has to be unique, not to order
    /// any other memory, and no two callers depend on each other's tick.
    fn next_tick(&self) -> u64 {
        self.tick.fetch_add(1, Ordering::Relaxed)
    }
}

impl HardwareProvider for MockHardware {
    fn capabilities(&self) -> HardwareCapabilities {
        match self.scenario {
            // Both live scenarios report a fully equipped laptop; the gap
            // scenario removes *readings*, never the interface itself.
            MockScenario::Nominal | MockScenario::SensorDataGap => {
                HardwareCapabilities::all_supported()
            }
            MockScenario::ZeroCapabilities => HardwareCapabilities::none(),
        }
    }

    fn sample_telemetry(&self) -> Result<TelemetrySnapshot, String> {
        let tick = self.next_tick();

        match self.scenario {
            MockScenario::Nominal => Ok(nominal_snapshot(tick)),
            // Nothing was detected, so nothing is reported. `Default` is the
            // empty snapshot: the UI renders its unavailable state from the
            // capability flags, never a fabricated zero.
            MockScenario::ZeroCapabilities => Ok(TelemetrySnapshot::default()),
            MockScenario::SensorDataGap => Ok(sensor_gap_snapshot(tick)),
        }
    }
}

/// The fully populated sample: every value oscillates inside its declared range.
fn nominal_snapshot(tick: u64) -> TelemetrySnapshot {
    let phase = tick as f32;

    // Ranges from the plan: CPU 15–45 %, GPU 30–70 %, fans 2400–3800 and
    // 2200–3600 RPM. Each is a sine around the midpoint of its band, so the
    // trace moves every tick instead of standing still.
    let cpu_utilization = 30.0 + 15.0 * (phase * 0.35).sin();
    let gpu_utilization = 50.0 + 20.0 * (phase * 0.22 + 1.1).sin();
    let cpu_temperature_c = 62.0 + 6.0 * (phase * 0.19).sin();
    let gpu_temperature_c = 66.0 + 5.0 * (phase * 0.17 + 0.5).sin();
    let cpu_frequency_mhz = (3_200.0 + 400.0 * (phase * 0.40).sin()).round() as u32;

    // RAM is derived the way the real provider will: `MemAvailable` is the
    // number the kernel reports, and used = total - available.
    let available_mb = 8_192.0 + 400.0 * (phase * 0.11).sin();
    let ram_used_mb = (RAM_TOTAL_MB as f32 - available_mb).round() as u64;

    let fan_1_rpm = (3_100.0 + 700.0 * (phase * 0.30).sin()).round() as u32;
    let fan_2_rpm = (2_900.0 + 700.0 * (phase * 0.27 + 0.8).sin()).round() as u32;

    TelemetrySnapshot {
        cpu_name: CPU_MODEL.to_string(),
        cpu_utilization,
        cpu_temperature_c: Some(cpu_temperature_c),
        cpu_frequency_mhz,
        gpu_name: Some(GPU_MODEL.to_string()),
        gpu_utilization: Some(gpu_utilization),
        gpu_temperature_c: Some(gpu_temperature_c),
        ram_total_mb: RAM_TOTAL_MB,
        ram_used_mb,
        fan_rpms: vec![fan_1_rpm, fan_2_rpm],
        fan_mode: "auto".to_string(),
        power_profile: "balanced".to_string(),
        power_profiles: vec![
            "quiet".to_string(),
            "balanced".to_string(),
            "performance".to_string(),
        ],
        rgb_profile: Some("aurora".to_string()),
        battery: Some(crate::hardware::BatteryTelemetry {
            present: true,
            charge_percent: Some(85),
            charging: Some(false),
            remaining_seconds: Some(10800),
        }),
    }
}

/// The nominal sample with the GPU readings removed for the last three ticks of
/// every ten.
///
/// The GPU *name* stays: the interface is still there, this tick just could not
/// be read. That is the difference the chart break has to show — a gap, not a
/// zero and not a vanished series.
fn sensor_gap_snapshot(tick: u64) -> TelemetrySnapshot {
    let mut snapshot = nominal_snapshot(tick);

    if tick % GAP_PERIOD_TICKS >= GAP_PERIOD_TICKS - GAP_TICKS {
        snapshot.gpu_utilization = None;
        snapshot.gpu_temperature_c = None;
        snapshot.battery = None;
    }

    snapshot
}
