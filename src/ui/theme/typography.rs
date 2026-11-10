//! ZenOmen typography styles and font configurations for GPUI-CE.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FontWeight {
    Normal,
    Medium,
    SemiBold,
    Bold,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TextStyle {
    pub font_family: &'static str,
    pub size_px: f32,
    pub weight: FontWeight,
    pub line_height_px: f32,
}

pub struct Typography;

impl Typography {
    pub const HEADING_LARGE: TextStyle = TextStyle {
        font_family: "Segoe UI, Inter, sans-serif",
        size_px: 24.0,
        weight: FontWeight::Bold,
        line_height_px: 32.0,
    };

    pub const HEADING_MEDIUM: TextStyle = TextStyle {
        font_family: "Segoe UI, Inter, sans-serif",
        size_px: 18.0,
        weight: FontWeight::SemiBold,
        line_height_px: 24.0,
    };

    pub const BODY: TextStyle = TextStyle {
        font_family: "Segoe UI, Inter, sans-serif",
        size_px: 14.0,
        weight: FontWeight::Normal,
        line_height_px: 20.0,
    };

    pub const CAPTION: TextStyle = TextStyle {
        font_family: "Segoe UI, Inter, sans-serif",
        size_px: 12.0,
        weight: FontWeight::Medium,
        line_height_px: 16.0,
    };

    pub const MONO_READOUT: TextStyle = TextStyle {
        font_family: "Consolas, JetBrains Mono, monospace",
        size_px: 20.0,
        weight: FontWeight::Bold,
        line_height_px: 28.0,
    };
}
