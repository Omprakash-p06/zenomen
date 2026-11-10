//! Theme system combining colors, typography, and spacing for ZenOmen GPUI.

pub mod colors;
pub mod spacing;
pub mod typography;

pub use colors::{Palette, RgbaColor};
pub use spacing::Spacing;
pub use typography::{FontWeight, TextStyle, Typography};

#[derive(Debug, Clone, PartialEq)]
pub struct Theme {
    pub dark_mode: bool,
}

impl Default for Theme {
    fn default() -> Self {
        Self { dark_mode: true }
    }
}
