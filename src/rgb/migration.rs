//! Non-destructive migration engine for legacy OpenOMEN RGB profiles and built-in presets.
//!
//! Enforces:
//! - Strictly read-only access to `~/.config/openomen/rgb/` (RGB-05)
//! - Source profiles are never deleted, overwritten, or modified
//! - Graceful extraction of unsupported animation fields into `unsupported_fields` (RGB-07)
//! - Built-in default presets (`White`, `Gaming`, `Rainbow`, `Off`) (RGB-03)

use std::fs;
use std::path::{Path, PathBuf};

use crate::rgb::schema::{RgbColor, RgbError, RgbProfile};

/// Result of migrating OpenOMEN profiles from legacy directory.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct MigrationReport {
    /// Successfully imported profile IDs.
    pub imported: Vec<String>,
    /// Files preserved because target profile already existed.
    pub preserved: Vec<(String, String)>,
    /// Files skipped due to parsing or validation errors, with reason.
    pub skipped: Vec<(String, String)>,
    /// Total count of source files scanned.
    pub total_scanned: usize,
}

/// Known animation/timing field names to detect and flag as unsupported in v1 (RGB-07).
const ANIMATION_FIELD_NAMES: &[&str] = &[
    "brightness",
    "speed",
    "length_ms",
    "loop",
    "animation",
    "effect",
    "direction",
    "fade_time",
    "hold_time",
    "color_count",
];

/// Resolve the default OpenOMEN legacy profile directory (`~/.config/openomen/rgb`).
pub fn default_openomen_rgb_dir() -> Option<PathBuf> {
    directories::BaseDirs::new().map(|dirs| dirs.config_dir().join("openomen").join("rgb"))
}

/// Resolve the ZenOmen RGB profile storage directory (`~/.config/zenomen/rgb`).
pub fn default_zenomen_rgb_dir() -> Option<PathBuf> {
    crate::config::project_dirs().map(|dirs| dirs.config_dir().join("rgb"))
}

/// Return standard static built-in RGB presets (RGB-03).
pub fn builtin_presets() -> Vec<RgbProfile> {
    vec![
        RgbProfile::new(
            "white",
            "White",
            "Clean static white illumination on all 4 zones",
            [
                RgbColor::new(255, 255, 255),
                RgbColor::new(255, 255, 255),
                RgbColor::new(255, 255, 255),
                RgbColor::new(255, 255, 255),
            ],
        ),
        RgbProfile::new(
            "gaming",
            "Gaming",
            "Dual-tone red flanks with vibrant blue center cluster",
            [
                RgbColor::new(255, 0, 0), // Zone 0: Left flank
                RgbColor::new(0, 0, 255), // Zone 1: Center-left (WASD)
                RgbColor::new(0, 0, 255), // Zone 2: Center-right
                RgbColor::new(255, 0, 0), // Zone 3: Right flank
            ],
        ),
        RgbProfile::new(
            "rainbow",
            "Rainbow",
            "Four-color spectrum transition across the keyboard",
            [
                RgbColor::new(255, 0, 0),   // Zone 0: Red
                RgbColor::new(255, 127, 0), // Zone 1: Orange
                RgbColor::new(0, 255, 0),   // Zone 2: Green
                RgbColor::new(0, 0, 255),   // Zone 3: Blue
            ],
        ),
        RgbProfile::new(
            "off",
            "Off",
            "Turn off keyboard illumination across all zones",
            [
                RgbColor::new(0, 0, 0),
                RgbColor::new(0, 0, 0),
                RgbColor::new(0, 0, 0),
                RgbColor::new(0, 0, 0),
            ],
        ),
    ]
}

/// Parse a raw OpenOMEN JSON string into a validated `RgbProfile`.
///
/// Handles both zone dictionary maps `{"0": "#FF0000", ...}` and array lists.
/// Flags any animation or timing keys into `unsupported_fields` (RGB-07).
pub fn parse_openomen_profile(file_stem: &str, content: &str) -> Result<RgbProfile, RgbError> {
    let value: serde_json::Value = serde_json::from_str(content)
        .map_err(|e| RgbError::ProfileParseError(format!("Malformed JSON: {e}")))?;

    let obj = value
        .as_object()
        .ok_or_else(|| RgbError::ProfileParseError("Root JSON must be an object".to_string()))?;

    // Extract human name or default to file stem
    let name = obj
        .get("name")
        .and_then(|v| v.as_str())
        .unwrap_or(file_stem)
        .to_string();

    let description = obj
        .get("description")
        .and_then(|v| v.as_str())
        .unwrap_or("Migrated from OpenOMEN")
        .to_string();

    // Check for animation / timing fields to flag as unsupported in v1 (RGB-07)
    let mut unsupported_fields = Vec::new();
    for &key in ANIMATION_FIELD_NAMES {
        if obj.contains_key(key) {
            unsupported_fields.push(key.to_string());
        }
    }

    // Extract zone colors
    let mut extracted_colors: [Option<RgbColor>; 4] = [None, None, None, None];

    if let Some(zones_val) = obj.get("zones").or_else(|| obj.get("colors")) {
        if let Some(zone_map) = zones_val.as_object() {
            // Mapping like {"0": "#FF0000", "1": "#00FF00", "2": "#0000FF", "3": "#FFFFFF"}
            for (k, v) in zone_map {
                if let Ok(idx) = k.parse::<usize>()
                    && idx < 4
                    && let Some(hex_str) = v.as_str()
                {
                    let color = RgbColor::from_hex(hex_str)?;
                    extracted_colors[idx] = Some(color);
                }
            }
        } else if let Some(zone_arr) = zones_val.as_array() {
            // Array like ["#FF0000", "#00FF00", "#0000FF", "#FFFFFF"]
            for (idx, item) in zone_arr.iter().take(4).enumerate() {
                if let Some(hex_str) = item.as_str() {
                    let color = RgbColor::from_hex(hex_str)?;
                    extracted_colors[idx] = Some(color);
                }
            }
        }
    }

    // Ensure all 4 zones are resolved, otherwise fail with descriptive error
    let mut final_zones = [RgbColor::default(); 4];
    for (i, zone_opt) in extracted_colors.iter().enumerate() {
        match zone_opt {
            Some(c) => final_zones[i] = *c,
            None => {
                return Err(RgbError::ProfileParseError(format!(
                    "Zone {i} color is missing in profile '{file_stem}'"
                )));
            }
        }
    }

    Ok(RgbProfile {
        id: file_stem.to_lowercase().replace(' ', "-"),
        name,
        description,
        zones: final_zones,
        unsupported_fields,
    })
}

/// Safely migrate profiles from `source_dir` to `target_dir` (RGB-05).
///
/// Strictly guarantees that no files in `source_dir` are deleted, overwritten, or modified.
pub fn migrate_openomen_profiles(source_dir: &Path, target_dir: &Path) -> MigrationReport {
    let mut report = MigrationReport::default();

    if !source_dir.is_dir() {
        return report;
    }

    let entries = match fs::read_dir(source_dir) {
        Ok(e) => e,
        Err(err) => {
            tracing::warn!(error = %err, path = %source_dir.display(), "could not read OpenOMEN RGB directory");
            return report;
        }
    };

    let _ = fs::create_dir_all(target_dir);

    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
            continue;
        }

        report.total_scanned += 1;
        let file_stem = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown");

        let content = match fs::read_to_string(&path) {
            Ok(c) => c,
            Err(e) => {
                report
                    .skipped
                    .push((file_stem.to_string(), format!("Read error: {e}")));
                continue;
            }
        };

        match parse_openomen_profile(file_stem, &content) {
            Ok(profile) => {
                let target_path = target_dir.join(format!("{}.json", profile.id));
                // Only write if target does not already exist, to preserve user modifications
                if target_path.exists() {
                    report.preserved.push((
                        file_stem.to_string(),
                        "target exists; kept existing profile".to_string(),
                    ));
                } else {
                    match serde_json::to_string_pretty(&profile) {
                        Ok(json_str) => {
                            if let Err(e) = fs::write(&target_path, json_str) {
                                report
                                    .skipped
                                    .push((file_stem.to_string(), format!("Write error: {e}")));
                                continue;
                            }
                            report.imported.push(profile.id);
                        }
                        Err(e) => {
                            report
                                .skipped
                                .push((file_stem.to_string(), format!("Serialization error: {e}")));
                            continue;
                        }
                    }
                }
            }
            Err(err) => {
                report
                    .skipped
                    .push((file_stem.to_string(), err.to_string()));
            }
        }
    }

    tracing::info!(
        scanned = report.total_scanned,
        imported = report.imported.len(),
        preserved = report.preserved.len(),
        skipped = report.skipped.len(),
        "OpenOMEN profile migration complete"
    );

    report
}
