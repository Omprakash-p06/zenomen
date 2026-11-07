//! Historical telemetry chart visualizer definition for GPUI-CE.

#[derive(Debug, Clone, PartialEq)]
pub struct TelemetryChart {
    pub title: String,
    pub series_name: String,
    pub unit: String,
    pub history_seconds: usize,
    pub data_points: Vec<Option<f32>>,
    pub min_val: f32,
    pub max_val: f32,
}

impl TelemetryChart {
    pub fn new(
        title: impl Into<String>,
        series_name: impl Into<String>,
        unit: impl Into<String>,
        history_seconds: usize,
        data_points: Vec<Option<f32>>,
        min_val: f32,
        max_val: f32,
    ) -> Self {
        Self {
            title: title.into(),
            series_name: series_name.into(),
            unit: unit.into(),
            history_seconds,
            data_points,
            min_val,
            max_val,
        }
    }

    pub fn latest_value(&self) -> Option<f32> {
        self.data_points.iter().rev().flatten().copied().next()
    }
}
