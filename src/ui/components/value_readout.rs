//! High-visibility value readout display component for GPUI-CE.

use crate::ui::theme::{Palette, RgbaColor};

#[derive(Debug, Clone, PartialEq)]
pub struct ValueReadout {
    pub label: String,
    pub numeric_value: f32,
    pub unit: String,
    pub threshold_warn: Option<f32>,
    pub threshold_crit: Option<f32>,
}

impl ValueReadout {
    pub fn new(label: impl Into<String>, value: f32, unit: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            numeric_value: value,
            unit: unit.into(),
            threshold_warn: None,
            threshold_crit: None,
        }
    }

    pub fn with_thresholds(mut self, warn: f32, crit: f32) -> Self {
        self.threshold_warn = Some(warn);
        self.threshold_crit = Some(crit);
        self
    }

    pub fn status_color(&self) -> RgbaColor {
        if let Some(crit) = self.threshold_crit
            && self.numeric_value >= crit
        {
            Palette::STATUS_ERROR
        } else if let Some(warn) = self.threshold_warn
            && self.numeric_value >= warn
        {
            Palette::STATUS_WARNING
        } else {
            Palette::STATUS_SUCCESS
        }
    }
}
