//! RGB keyboard lighting hardware interface detection.
//!
//! Identifies the active sysfs interface variant:
//! 1. `FourZone`: `/sys/devices/platform/hp-wmi/rgb_zones/zone0{0..3}`
//! 2. `Legacy`: `/sys/devices/platform/hp-wmi/keyboardleds`
//! 3. `KernelPatch`: `/sys/class/leds/hp_omen::kbd_backlight/zone_colors`

use serde::{Deserialize, Serialize};
use std::path::Path;

/// The three sysfs interface variants supported across OMEN/Victus kernels (RGB-01).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RgbInterfaceVariant {
    /// Multi-file interface: `/sys/devices/platform/hp-wmi/rgb_zones/zone0{0..3}`
    FourZone,
    /// Monolithic hex interface: `/sys/devices/platform/hp-wmi/keyboardleds`
    Legacy,
    /// Upstream kernel patch interface: `/sys/class/leds/hp_omen::kbd_backlight/zone_colors`
    KernelPatch,
}

impl RgbInterfaceVariant {
    /// Human-readable interface variant label for diagnostics and UI.
    pub fn name(&self) -> &'static str {
        match self {
            Self::FourZone => "Four-Zone",
            Self::Legacy => "Legacy",
            Self::KernelPatch => "Kernel Patch",
        }
    }
}

/// Detect the active RGB hardware interface under a given sysfs root (RGB-01).
pub fn detect_rgb_interface(sysfs_root: &Path) -> Option<RgbInterfaceVariant> {
    // 1. Check for FourZone: rgb_zones directory with zone00
    let four_zone = sysfs_root.join("sys/devices/platform/hp-wmi/rgb_zones/zone00");
    if four_zone.exists() {
        return Some(RgbInterfaceVariant::FourZone);
    }

    // 2. Check for Legacy: keyboardleds
    let legacy = sysfs_root.join("sys/devices/platform/hp-wmi/keyboardleds");
    if legacy.exists() {
        return Some(RgbInterfaceVariant::Legacy);
    }

    // 3. Check for KernelPatch: hp_omen::kbd_backlight/zone_colors
    let kernel_patch = sysfs_root.join("sys/class/leds/hp_omen::kbd_backlight/zone_colors");
    if kernel_patch.exists() {
        return Some(RgbInterfaceVariant::KernelPatch);
    }

    None
}
