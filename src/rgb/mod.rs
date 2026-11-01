//! RGB keyboard lighting subsystem for HP OMEN and Victus laptops.
//!
//! Provides:
//! - Strict `#RRGGBB` hex color validation and profile definitions (RGB-06, RGB-07)
//! - Non-destructive OpenOMEN profile migration and default presets (RGB-03, RGB-05)

pub mod migration;
pub mod schema;

pub use migration::{
    builtin_presets, default_openomen_rgb_dir, default_zenomen_rgb_dir, migrate_openomen_profiles,
    parse_openomen_profile, MigrationReport,
};
pub use schema::{RgbColor, RgbError, RgbProfile};
