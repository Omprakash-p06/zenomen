//! RGB keyboard lighting schema and strict hex color validation.
//!
//! Enforces:
//! - Strict `#RRGGBB` hex validation with descriptive errors (RGB-06)
//! - 4-zone keyboard color definitions (Left, Center Left, Center Right, Right)
//! - Unsupported animation field tracking for v2 forward compatibility (RGB-07)

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use thiserror::Error;

/// Errors produced during RGB color parsing or profile processing.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum RgbError {
    #[error("Color string is empty; expected #RRGGBB")]
    EmptyColor,

    #[error("Invalid color format '{0}': expected #RRGGBB (7 characters) or RRGGBB (6 characters)")]
    InvalidLength(String),

    #[error("Invalid hex character in '{0}': each channel must be valid hexadecimal [0-9a-fA-F]")]
    InvalidHexCharacter(String),

    #[error("Expected exactly 4 zones for keyboard lighting, found {0}")]
    InvalidZoneCount(usize),

    #[error("Profile deserialization failed: {0}")]
    ProfileParseError(String),
}

/// A 24-bit RGB color representation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct RgbColor {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl RgbColor {
    /// Construct from individual red, green, and blue bytes.
    pub const fn new(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }

    /// Parse a strict `#RRGGBB` or `RRGGBB` hexadecimal string (RGB-06).
    ///
    /// Case-insensitive. Rejects strings with whitespace, invalid length, or invalid hex digits.
    pub fn from_hex(s: &str) -> Result<Self, RgbError> {
        let trimmed = s.trim();
        if trimmed.is_empty() {
            return Err(RgbError::EmptyColor);
        }

        let hex_body = if let Some(stripped) = trimmed.strip_prefix('#') {
            stripped
        } else {
            trimmed
        };

        if hex_body.len() != 6 {
            return Err(RgbError::InvalidLength(trimmed.to_string()));
        }

        if !hex_body.chars().all(|c| c.is_ascii_hexdigit()) {
            return Err(RgbError::InvalidHexCharacter(trimmed.to_string()));
        }

        let r = u8::from_str_radix(&hex_body[0..2], 16)
            .map_err(|_| RgbError::InvalidHexCharacter(trimmed.to_string()))?;
        let g = u8::from_str_radix(&hex_body[2..4], 16)
            .map_err(|_| RgbError::InvalidHexCharacter(trimmed.to_string()))?;
        let b = u8::from_str_radix(&hex_body[4..6], 16)
            .map_err(|_| RgbError::InvalidHexCharacter(trimmed.to_string()))?;

        Ok(Self { r, g, b })
    }

    /// Format as uppercase `#RRGGBB` hex string.
    pub fn to_hex(&self) -> String {
        format!("#{:02X}{:02X}{:02X}", self.r, self.g, self.b)
    }

    /// Format as 6-character hex without prefix (for legacy/sysfs writes).
    pub fn to_hex_raw(&self) -> String {
        format!("{:02X}{:02X}{:02X}", self.r, self.g, self.b)
    }

    /// Convert to Slint UI color representation.
    pub fn to_slint_color(&self) -> slint::Color {
        slint::Color::from_argb_u8(255, self.r, self.g, self.b)
    }
}

impl Serialize for RgbColor {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.to_hex())
    }
}

impl<'de> Deserialize<'de> for RgbColor {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        RgbColor::from_hex(&s).map_err(serde::de::Error::custom)
    }
}

/// Standard 4-zone keyboard profile definition.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RgbProfile {
    /// Unique profile identifier (e.g., "white", "gaming", "custom-1").
    pub id: String,
    /// Human-readable profile name.
    pub name: String,
    /// Brief description of the color scheme.
    #[serde(default)]
    pub description: String,
    /// Exact 4-zone colors in order [Zone 0 (Left), Zone 1, Zone 2, Zone 3 (Right)].
    pub zones: [RgbColor; 4],
    /// Unsupported animation/timing fields discovered in profile (RGB-07).
    #[serde(default)]
    pub unsupported_fields: Vec<String>,
}

impl RgbProfile {
    /// Create a new 4-zone static profile.
    pub fn new<I, N, D>(id: I, name: N, description: D, zones: [RgbColor; 4]) -> Self
    where
        I: Into<String>,
        N: Into<String>,
        D: Into<String>,
    {
        Self {
            id: id.into(),
            name: name.into(),
            description: description.into(),
            zones,
            unsupported_fields: Vec::new(),
        }
    }

    /// Check if all 4 zones are black (`#000000`).
    pub fn is_off(&self) -> bool {
        self.zones.iter().all(|c| c.r == 0 && c.g == 0 && c.b == 0)
    }
}
