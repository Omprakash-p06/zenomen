//! Bounded telemetry history and the SVG path generator behind every chart.
//!
//! Two responsibilities, both deliberately pure:
//!
//! * [`TelemetryHistory`] keeps the last [`MAX_SAMPLES`] points in a `VecDeque`
//!   and evicts with `pop_front()` before `push_back()`. The bound is the whole
//!   memory story of the charts (MON-09, and the T-01-05 threat): a two-hour
//!   session holds the same 120 points as a two-minute one.
//! * [`generate_svg_path`] turns a series into `M x y L x y …` commands. The
//!   `.slint` layer only renders the string — it never computes geometry — so
//!   the gap and empty-state rules can be tested without a display server.
//!
//! A `None` sample is a *gap*, not a zero: the generator lifts the pen (`M`
//! again on the next value) instead of drawing a line through a reading that
//! was never taken. A series with fewer than two values produces no path at all,
//! which the UI renders as "Waiting for first reading".

use std::collections::VecDeque;
use std::time::Instant;

use crate::hardware::TelemetrySnapshot;

/// How many samples every chart keeps.
///
/// 120 at 1 Hz is the two-minute window fixed by the project's memory
/// constraint (MON-09, MON-11).
pub const MAX_SAMPLES: usize = 120;

/// Logical plot size for the wide chart.
///
/// The UI scales this to the width the window gives it through the `Path`
/// `viewbox` properties, so the core never needs to know the window size and a
/// resize never requires regenerating a path.
pub const CHART_WIDTH: f32 = 800.0;
pub const CHART_HEIGHT: f32 = 140.0;

/// Logical plot size for a card sparkline (28px tall, like the component).
pub const SPARKLINE_WIDTH: f32 = 200.0;
pub const SPARKLINE_HEIGHT: f32 = 28.0;

/// One sample, reduced to the four series the charts can plot.
///
/// `None` means "not reported this tick" — the distinction the chart break and
/// the unavailable state both depend on.
#[derive(Debug, Clone, PartialEq)]
pub struct TelemetryPoint {
    /// When the sample was taken. Kept for future windowing (MON-11); the
    /// charts currently plot by position because the tick is a fixed 1 Hz.
    pub timestamp: Instant,
    /// CPU utilization, percent.
    pub cpu: Option<f32>,
    /// GPU utilization, percent.
    pub gpu: Option<f32>,
    /// RAM used, percent of installed.
    pub ram: Option<f32>,
    /// First fan tachometer, RPM. Stored for a later cooling chart; the three
    /// Overview series are CPU, GPU and RAM.
    pub fan: Option<f32>,
}

impl TelemetryPoint {
    /// Reduce a provider snapshot to the chartable series.
    ///
    /// Presence is decided by the *sensor*, not by the value: a zero-percent
    /// CPU reading is a real reading, while an empty model name means the CPU
    /// was never reported and must become `None`.
    pub fn from_snapshot(snapshot: &TelemetrySnapshot) -> Self {
        Self {
            timestamp: Instant::now(),
            cpu: (!snapshot.cpu_name.is_empty()).then_some(snapshot.cpu_utilization),
            gpu: snapshot.gpu_utilization,
            ram: (snapshot.ram_total_mb > 0)
                .then(|| (snapshot.ram_used_mb as f32 / snapshot.ram_total_mb as f32) * 100.0),
            fan: snapshot.fan_rpms.first().map(|rpm| *rpm as f32),
        }
    }
}

/// The six path strings one tick produces: three chart series and the three
/// card sparklines that share the same history.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TelemetryPaths {
    pub cpu_path: String,
    pub gpu_path: String,
    pub ram_path: String,
    pub cpu_sparkline: String,
    pub gpu_sparkline: String,
    pub ram_sparkline: String,
}

/// The bounded ring buffer behind the charts.
#[derive(Debug, Clone, Default)]
pub struct TelemetryHistory {
    points: VecDeque<TelemetryPoint>,
}

impl TelemetryHistory {
    /// The capacity, exposed next to the buffer so callers do not reach for the
    /// module constant and a future configurable window has one place to change.
    pub const MAX_SAMPLES: usize = MAX_SAMPLES;

    /// An empty history pre-sized to the bound, so the 120th push is not the
    /// first allocation.
    pub fn new() -> Self {
        Self {
            points: VecDeque::with_capacity(MAX_SAMPLES),
        }
    }

    /// Append one sample, evicting the oldest when the buffer is full.
    ///
    /// `pop_front()` runs *before* `push_back()`, so the deque never holds more
    /// than [`MAX_SAMPLES`] points even for an instant (T-01-05).
    pub fn push(&mut self, point: TelemetryPoint) {
        if self.points.len() >= Self::MAX_SAMPLES {
            self.points.pop_front();
        }
        self.points.push_back(point);
    }

    /// How many samples are retained, never more than [`MAX_SAMPLES`].
    pub fn sample_count(&self) -> usize {
        self.points.len()
    }

    /// The retained samples, oldest first.
    pub fn points(&self) -> &VecDeque<TelemetryPoint> {
        &self.points
    }

    /// Paths for the wide chart: `(cpu, gpu, ram)`.
    pub fn generate_chart_paths(&self, width: f32, height: f32) -> (String, String, String) {
        (
            self.path(|point| point.cpu, width, height),
            self.path(|point| point.gpu, width, height),
            self.path(|point| point.ram, width, height),
        )
    }

    /// Paths for the three card sparklines, at their own logical size.
    pub fn generate_sparkline_paths(&self) -> (String, String, String) {
        self.generate_chart_paths(SPARKLINE_WIDTH, SPARKLINE_HEIGHT)
    }

    /// Extract one series and turn it into a path.
    ///
    /// The full window is passed as the capacity, so a history shorter than 120
    /// samples is plotted right-aligned to "now" rather than stretched across
    /// the width: the left edge stays empty instead of being padded with a
    /// reading that was never taken (UI-SPEC "Data & Text States").
    fn path(&self, pick: fn(&TelemetryPoint) -> Option<f32>, width: f32, height: f32) -> String {
        let series: Vec<Option<f32>> = self.points.iter().map(pick).collect();
        generate_svg_path_windowed(&series, width, height, MAX_SAMPLES)
    }
}

/// Turn a series into SVG path commands.
///
/// `0 %` maps to the bottom of the plot (`y = height`) and `100 %` to the top
/// (`y = 0`), so a higher reading is drawn higher. The samples are spread across
/// the full `width`, which is what a standalone series wants; the chart itself
/// uses the windowed form below so a short history sits at the right edge.
///
/// Returns `""` for fewer than two reported values — the UI renders "Waiting
/// for first reading" from the empty string instead of a flat line at zero.
pub fn generate_svg_path(samples: &[Option<f32>], width: f32, height: f32) -> String {
    let capacity = samples.len().max(2);
    generate_svg_path_windowed(samples, width, height, capacity)
}

/// The windowed form: `capacity` slots, of which the newest `samples.len()` are
/// plotted at the right edge.
fn generate_svg_path_windowed(
    samples: &[Option<f32>],
    width: f32,
    height: f32,
    capacity: usize,
) -> String {
    let reported = samples.iter().filter(|sample| sample.is_some()).count();
    if reported < 2 || width <= 0.0 || height <= 0.0 {
        return String::new();
    }

    let capacity = capacity.max(samples.len()).max(2);
    let step = width / (capacity - 1) as f32;
    // Right-align: the oldest retained sample is placed as if the buffer were
    // already full, so the trace scrolls in from the right.
    let start = width - step * samples.len().saturating_sub(1) as f32;

    let mut path = String::with_capacity(samples.len() * 16);
    let mut pen_down = false;

    for (index, sample) in samples.iter().enumerate() {
        let Some(value) = sample else {
            // A gap lifts the pen; the next value resumes with `M` so the line
            // never crosses a reading that was not taken.
            pen_down = false;
            continue;
        };

        let x = start + step * index as f32;
        let y = height * (1.0 - value.clamp(0.0, 100.0) / 100.0);

        if !path.is_empty() {
            path.push(' ');
        }
        path.push(if pen_down { 'L' } else { 'M' });
        path.push_str(&format!(" {x:.1} {y:.1}"));
        pen_down = true;
    }

    path
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn history_evicts_oldest_beyond_max_samples() {
        let mut history = TelemetryHistory::new();
        for i in 0..150 {
            history.push(TelemetryPoint {
                timestamp: Instant::now(),
                cpu: Some(i as f32),
                gpu: None,
                ram: None,
                fan: None,
            });
        }
        assert_eq!(history.sample_count(), MAX_SAMPLES);
        assert_eq!(history.sample_count(), 120);

        // First point in deque should be sample 30 (150 - 120)
        let first = history.points().front().unwrap();
        assert_eq!(first.cpu, Some(30.0));
        let last = history.points().back().unwrap();
        assert_eq!(last.cpu, Some(149.0));
    }

    #[test]
    fn svg_path_empty_for_fewer_than_two_samples() {
        assert_eq!(generate_svg_path(&[], 800.0, 140.0), "");
        assert_eq!(generate_svg_path(&[Some(50.0)], 800.0, 140.0), "");
        assert_eq!(
            generate_svg_path(&[None, Some(50.0), None], 800.0, 140.0),
            ""
        );
    }

    #[test]
    fn svg_path_renders_valid_commands() {
        let samples = vec![Some(0.0), Some(50.0), Some(100.0)];
        let path = generate_svg_path(&samples, 100.0, 100.0);
        assert!(!path.is_empty());
        assert!(path.starts_with('M'), "path must start with M: {path}");
        assert!(path.contains('L'), "path must contain L: {path}");
    }

    #[test]
    fn svg_path_breaks_on_none_gaps() {
        let samples = vec![Some(10.0), Some(20.0), None, Some(40.0), Some(50.0)];
        let path = generate_svg_path(&samples, 100.0, 100.0);
        // Should have an initial M, then L, then gap, then second M, then L
        let m_count = path.matches('M').count();
        assert_eq!(m_count, 2, "Expected 2 M segments due to gap, got: {path}");
    }
}
