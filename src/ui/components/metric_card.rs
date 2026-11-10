//! Modular telemetry metric card component for GPUI-CE.

use crate::ui::theme::RgbaColor;

#[derive(Debug, Clone, PartialEq)]
pub struct MetricCard {
    pub title: String,
    pub formatted_value: String,
    pub unit: String,
    pub subtitle: Option<String>,
    pub status_color: Option<RgbaColor>,
    pub progress_ratio: Option<f32>,
}

impl MetricCard {
    pub fn new(
        title: impl Into<String>,
        value: impl Into<String>,
        unit: impl Into<String>,
    ) -> Self {
        Self {
            title: title.into(),
            formatted_value: value.into(),
            unit: unit.into(),
            subtitle: None,
            status_color: None,
            progress_ratio: None,
        }
    }

    pub fn with_subtitle(mut self, subtitle: impl Into<String>) -> Self {
        self.subtitle = Some(subtitle.into());
        self
    }

    pub fn with_progress(mut self, ratio: f32) -> Self {
        self.progress_ratio = Some(ratio.clamp(0.0, 1.0));
        self
    }

    pub fn with_status_color(mut self, color: RgbaColor) -> Self {
        self.status_color = Some(color);
        self
    }
}
