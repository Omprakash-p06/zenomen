//! ZenOmen color design tokens for GPUI-CE presentation layer.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RgbaColor {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl RgbaColor {
    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b, a: 255 }
    }

    pub const fn rgba(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }

    pub fn to_hex(&self) -> String {
        format!("#{:02x}{:02x}{:02x}{:02x}", self.r, self.g, self.b, self.a)
    }
}

pub struct Palette;

impl Palette {
    // Backgrounds
    pub const BG_APP: RgbaColor = RgbaColor::rgb(0x0f, 0x11, 0x15);
    pub const BG_SURFACE: RgbaColor = RgbaColor::rgb(0x18, 0x1b, 0x21);
    pub const BG_SURFACE_HOVER: RgbaColor = RgbaColor::rgb(0x22, 0x26, 0x30);
    pub const BG_SURFACE_ACTIVE: RgbaColor = RgbaColor::rgb(0x2a, 0x30, 0x3c);

    // Borders
    pub const BORDER_SUBTLE: RgbaColor = RgbaColor::rgb(0x28, 0x2c, 0x34);
    pub const BORDER_FOCUS: RgbaColor = RgbaColor::rgb(0x3b, 0x82, 0xf6);

    // Text
    pub const TEXT_PRIMARY: RgbaColor = RgbaColor::rgb(0xf3, 0xf4, 0xf6);
    pub const TEXT_SECONDARY: RgbaColor = RgbaColor::rgb(0x9c, 0xa3, 0xaf);
    pub const TEXT_MUTED: RgbaColor = RgbaColor::rgb(0x6b, 0x72, 0x80);

    // Brand & Status Accents
    pub const ACCENT_OMEN: RgbaColor = RgbaColor::rgb(0xff, 0x00, 0x33);
    pub const ACCENT_CYAN: RgbaColor = RgbaColor::rgb(0x00, 0xe5, 0xff);
    pub const STATUS_SUCCESS: RgbaColor = RgbaColor::rgb(0x10, 0xb9, 0x81);
    pub const STATUS_WARNING: RgbaColor = RgbaColor::rgb(0xf5, 0x9e, 0x0b);
    pub const STATUS_ERROR: RgbaColor = RgbaColor::rgb(0xef, 0x44, 0x44);
    pub const STATUS_INFO: RgbaColor = RgbaColor::rgb(0x3b, 0x82, 0xf6);
}
