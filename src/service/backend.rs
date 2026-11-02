//! Hardware backend executing verified sysfs writes with retry and read-back validation.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::service::types::{FanMode, HardwareControlError, PresetResult};

/// Verified write helper with exponential backoff retry.
/// Writes `target` to `path`, sleeps `initial_delay`, and checks read-back.
/// If read-back fails to match after `max_retries`, returns `VerificationFailed`.
pub fn write_and_verify(
    path: &Path,
    target: &str,
    max_retries: usize,
    initial_delay: Duration,
) -> Result<(), HardwareControlError> {
    let target_trimmed = target.trim();
    let mut delay = initial_delay;

    for _attempt in 1..=max_retries {
        fs::write(path, target_trimmed).map_err(|e| {
            HardwareControlError::IoError(format!("Write to {} failed: {e}", path.display()))
        })?;

        std::thread::sleep(delay);

        let read_back = fs::read_to_string(path).map_err(|e| {
            HardwareControlError::IoError(format!("Read-back from {} failed: {e}", path.display()))
        })?;

        if read_back.trim().eq_ignore_ascii_case(target_trimmed) {
            return Ok(());
        }

        delay = delay.saturating_mul(2);
    }

    let actual = fs::read_to_string(path)
        .unwrap_or_default()
        .trim()
        .to_string();
    Err(HardwareControlError::VerificationFailed {
        target: target_trimmed.to_string(),
        actual,
        retries: max_retries,
    })
}

/// Backend hardware controller targeting sysfs interfaces under a configurable root directory.
#[derive(Debug, Clone)]
pub struct HardwareBackend {
    sysfs_root: PathBuf,
}

impl Default for HardwareBackend {
    fn default() -> Self {
        Self::new("/")
    }
}

impl HardwareBackend {
    /// Construct a new hardware backend using `root` as the base filesystem prefix.
    pub fn new<P: Into<PathBuf>>(root: P) -> Self {
        Self {
            sysfs_root: root.into(),
        }
    }

    /// Resolve base sysfs path by prefixing `sysfs_root`.
    fn resolve_path(&self, rel: &str) -> PathBuf {
        let trimmed = rel.trim_start_matches('/');
        self.sysfs_root.join(trimmed)
    }

    /// Discover the `pwm1_enable` control path for HP WMI fan control.
    pub fn find_fan_control_path(&self) -> Result<PathBuf, HardwareControlError> {
        // 1. Direct path check under /sys/devices/platform/hp-wmi/hwmon/hwmon*/pwm1_enable
        let hp_wmi_hwmon = self.resolve_path("sys/devices/platform/hp-wmi/hwmon");
        if hp_wmi_hwmon.is_dir()
            && let Ok(entries) = fs::read_dir(&hp_wmi_hwmon)
        {
            for entry in entries.flatten() {
                let pwm_path = entry.path().join("pwm1_enable");
                if pwm_path.exists() {
                    return Ok(pwm_path);
                }
            }
        }

        // 2. Fallback: search /sys/class/hwmon/hwmon* for name == "hp" or "hp-wmi" with pwm1_enable
        let class_hwmon = self.resolve_path("sys/class/hwmon");
        if class_hwmon.is_dir()
            && let Ok(entries) = fs::read_dir(&class_hwmon)
        {
            for entry in entries.flatten() {
                let dir = entry.path();
                let name_path = dir.join("name");
                let pwm_path = dir.join("pwm1_enable");

                if pwm_path.exists()
                    && let Ok(name) = fs::read_to_string(&name_path)
                {
                    let name_trim = name.trim();
                    if name_trim == "hp" || name_trim == "hp-wmi" {
                        return Ok(pwm_path);
                    }
                }
            }
        }

        Err(HardwareControlError::InterfaceMissing(
            "No writable pwm1_enable fan control interface found".to_string(),
        ))
    }

    /// Read current fan mode from sysfs.
    pub fn get_fan_mode(&self) -> Result<FanMode, HardwareControlError> {
        let path = self.find_fan_control_path()?;
        let content = fs::read_to_string(&path).map_err(|e| {
            HardwareControlError::IoError(format!("Failed to read {}: {e}", path.display()))
        })?;
        Ok(FanMode::from_sysfs(&content))
    }

    /// Read current fan RPMs from the fan hwmon device.
    pub fn get_fan_rpms(&self) -> Vec<u32> {
        let mut rpms = Vec::new();
        // Check hp-wmi hwmon directory
        let search_dirs = [
            self.resolve_path("sys/devices/platform/hp-wmi/hwmon"),
            self.resolve_path("sys/class/hwmon"),
        ];

        for base in search_dirs {
            if !base.is_dir() {
                continue;
            }
            if let Ok(entries) = fs::read_dir(&base) {
                for entry in entries.flatten() {
                    let dir = entry.path();
                    for idx in 1..=4 {
                        let fan_input = dir.join(format!("fan{idx}_input"));
                        if fan_input.is_file()
                            && let Ok(content) = fs::read_to_string(&fan_input)
                            && let Ok(rpm) = content.trim().parse::<u32>()
                        {
                            rpms.push(rpm);
                        }
                    }
                    if !rpms.is_empty() {
                        return rpms;
                    }
                }
            }
        }
        rpms
    }

    /// Set fan mode with verification.
    /// Strictly supports "auto" and "max" in v1.
    pub fn set_fan_mode(&self, mode_str: &str) -> Result<String, HardwareControlError> {
        let target_mode = FanMode::parse_requested(mode_str)?;
        let sysfs_val = target_mode.as_sysfs_value().ok_or_else(|| {
            HardwareControlError::UnsupportedMode(format!(
                "Mode '{mode_str}' has no valid sysfs mapping"
            ))
        })?;

        let path = self.find_fan_control_path()?;
        write_and_verify(&path, sysfs_val, 3, Duration::from_millis(50))?;

        // Re-read confirmed mode
        let confirmed = self.get_fan_mode()?;
        Ok(confirmed.as_str().to_string())
    }

    /// Read advertised ACPI platform profile choices.
    pub fn get_power_profile_choices(&self) -> Result<Vec<String>, HardwareControlError> {
        let path = self.resolve_path("sys/firmware/acpi/platform_profile_choices");
        if !path.exists() {
            return Err(HardwareControlError::InterfaceMissing(
                "ACPI platform_profile_choices interface not found".to_string(),
            ));
        }

        let content = fs::read_to_string(&path).map_err(|e| {
            HardwareControlError::IoError(format!("Failed to read {}: {e}", path.display()))
        })?;

        let choices: Vec<String> = content
            .split_whitespace()
            .map(|s| s.to_string())
            .filter(|s| !s.is_empty())
            .collect();

        Ok(choices)
    }

    /// Read currently active ACPI platform profile.
    pub fn get_power_profile(&self) -> Result<String, HardwareControlError> {
        let path = self.resolve_path("sys/firmware/acpi/platform_profile");
        if !path.exists() {
            return Err(HardwareControlError::InterfaceMissing(
                "ACPI platform_profile interface not found".to_string(),
            ));
        }

        let content = fs::read_to_string(&path).map_err(|e| {
            HardwareControlError::IoError(format!("Failed to read {}: {e}", path.display()))
        })?;

        Ok(content.trim().to_string())
    }

    /// Set ACPI platform profile with verification against advertised choices.
    pub fn set_power_profile(&self, requested: &str) -> Result<String, HardwareControlError> {
        if requested.contains('\n')
            || requested.contains('\r')
            || requested.contains('/')
            || requested.contains('\0')
        {
            return Err(HardwareControlError::InvalidParameter(format!(
                "Invalid power profile name '{requested}': contains illegal characters"
            )));
        }

        let target = requested.trim().to_lowercase();
        if target.is_empty() {
            return Err(HardwareControlError::InvalidParameter(
                "Power profile name cannot be empty".to_string(),
            ));
        }

        let choices = self.get_power_profile_choices()?;
        if !choices.iter().any(|c| c.eq_ignore_ascii_case(&target)) {
            return Err(HardwareControlError::InvalidParameter(format!(
                "Profile '{target}' is not advertised by platform_profile_choices ({:?})",
                choices
            )));
        }

        let path = self.resolve_path("sys/firmware/acpi/platform_profile");
        write_and_verify(&path, &target, 3, Duration::from_millis(50))?;

        let confirmed = self.get_power_profile()?;
        Ok(confirmed)
    }

    /// Apply a combined preset (Quiet, Balanced, Gaming) coordinating power profile and fan mode.
    /// Returns a structured `PresetResult` detailing what was applied without false claims of atomicity.
    pub fn apply_preset(&self, preset_name: &str) -> Result<PresetResult, HardwareControlError> {
        let normalized = preset_name.trim().to_lowercase();
        let choices = self.get_power_profile_choices().unwrap_or_default();

        let (target_power, target_fan) = match normalized.as_str() {
            "gaming" => ("performance", "max"),
            "balanced" => ("balanced", "auto"),
            "quiet" => {
                // If "quiet" is supported, use it; otherwise fallback to "cool" or "low-power"
                if choices.iter().any(|c| c == "quiet") {
                    ("quiet", "auto")
                } else if choices.iter().any(|c| c == "cool") {
                    ("cool", "auto")
                } else if choices.iter().any(|c| c == "low-power") {
                    ("low-power", "auto")
                } else {
                    ("balanced", "auto")
                }
            }
            other => {
                return Err(HardwareControlError::InvalidParameter(format!(
                    "Unknown preset '{other}'. Supported presets are 'quiet', 'balanced', 'gaming'"
                )));
            }
        };

        let mut power_applied = false;
        let mut fan_applied = false;
        let mut error_messages = Vec::new();

        // 1. Apply power profile
        match self.set_power_profile(target_power) {
            Ok(_) => power_applied = true,
            Err(e) => error_messages.push(format!("Power profile '{target_power}' failed: {e}")),
        }

        // 2. Apply fan mode
        match self.set_fan_mode(target_fan) {
            Ok(_) => fan_applied = true,
            Err(e) => error_messages.push(format!("Fan mode '{target_fan}' failed: {e}")),
        }

        let partial_success = (power_applied != fan_applied) && (power_applied || fan_applied);
        let message = if power_applied && fan_applied {
            format!(
                "Preset '{normalized}' successfully applied (profile: {target_power}, fan: {target_fan})"
            )
        } else if partial_success {
            format!(
                "Preset '{normalized}' partially applied. {}",
                error_messages.join("; ")
            )
        } else {
            format!(
                "Preset '{normalized}' failed. {}",
                error_messages.join("; ")
            )
        };

        Ok(PresetResult {
            preset_name: normalized,
            power_applied,
            fan_applied,
            partial_success,
            message,
        })
    }

    /// Discover the active RGB lighting interface variant and path (RGB-01).
    pub fn find_rgb_interface(
        &self,
    ) -> Result<(crate::hardware::rgb::RgbInterfaceVariant, PathBuf), HardwareControlError> {
        // 1. FourZone: /sys/devices/platform/hp-wmi/rgb_zones/zone00
        let four_zone_dir = self.resolve_path("sys/devices/platform/hp-wmi/rgb_zones");
        if four_zone_dir.join("zone00").exists() {
            return Ok((
                crate::hardware::rgb::RgbInterfaceVariant::FourZone,
                four_zone_dir,
            ));
        }

        // 2. Legacy: /sys/devices/platform/hp-wmi/keyboardleds
        let legacy = self.resolve_path("sys/devices/platform/hp-wmi/keyboardleds");
        if legacy.exists() {
            return Ok((crate::hardware::rgb::RgbInterfaceVariant::Legacy, legacy));
        }

        // 3. KernelPatch: /sys/class/leds/hp_omen::kbd_backlight/zone_colors
        let kernel_patch = self.resolve_path("sys/class/leds/hp_omen::kbd_backlight/zone_colors");
        if kernel_patch.exists() {
            return Ok((
                crate::hardware::rgb::RgbInterfaceVariant::KernelPatch,
                kernel_patch,
            ));
        }

        Err(HardwareControlError::InterfaceMissing(
            "No supported HP keyboard RGB interface detected".to_string(),
        ))
    }

    /// Read active 4-zone colors from sysfs (if supported by active interface variant).
    pub fn get_rgb_colors(&self) -> Result<[crate::rgb::RgbColor; 4], HardwareControlError> {
        let (variant, path) = self.find_rgb_interface()?;
        match variant {
            crate::hardware::rgb::RgbInterfaceVariant::FourZone => {
                let mut colors = [crate::rgb::RgbColor::default(); 4];
                for (i, slot) in colors.iter_mut().enumerate() {
                    let zone_path = path.join(format!("zone0{i}"));
                    let content = fs::read_to_string(&zone_path).map_err(|e| {
                        HardwareControlError::IoError(format!(
                            "Failed to read {}: {e}",
                            zone_path.display()
                        ))
                    })?;
                    *slot = crate::rgb::RgbColor::from_hex(&content)
                        .map_err(|e| HardwareControlError::InvalidParameter(e.to_string()))?;
                }
                Ok(colors)
            }
            crate::hardware::rgb::RgbInterfaceVariant::Legacy => {
                let content = fs::read_to_string(&path).map_err(|e| {
                    HardwareControlError::IoError(format!("Failed to read {}: {e}", path.display()))
                })?;
                let trimmed = content.trim();
                if trimmed.len() != 24 {
                    return Err(HardwareControlError::InvalidParameter(format!(
                        "Legacy keyboardleds returned invalid length (expected 24, got {})",
                        trimmed.len()
                    )));
                }
                let mut colors = [crate::rgb::RgbColor::default(); 4];
                for i in 0..4 {
                    let hex_slice = &trimmed[i * 6..(i + 1) * 6];
                    colors[i] = crate::rgb::RgbColor::from_hex(hex_slice)
                        .map_err(|e| HardwareControlError::InvalidParameter(e.to_string()))?;
                }
                Ok(colors)
            }
            crate::hardware::rgb::RgbInterfaceVariant::KernelPatch => {
                let bytes = fs::read(&path).map_err(|e| {
                    HardwareControlError::IoError(format!("Failed to read {}: {e}", path.display()))
                })?;
                if bytes.len() != 12 {
                    return Err(HardwareControlError::InvalidParameter(format!(
                        "KernelPatch zone_colors returned invalid length (expected 12 bytes, got {})",
                        bytes.len()
                    )));
                }
                let mut colors = [crate::rgb::RgbColor::default(); 4];
                for i in 0..4 {
                    colors[i] =
                        crate::rgb::RgbColor::new(bytes[i * 3], bytes[i * 3 + 1], bytes[i * 3 + 2]);
                }
                Ok(colors)
            }
        }
    }

    /// Apply 4-zone colors to hardware using the discovered variant with verification (RGB-08).
    pub fn apply_rgb_hardware(
        &self,
        colors: &[crate::rgb::RgbColor; 4],
    ) -> Result<(), HardwareControlError> {
        let (variant, path) = self.find_rgb_interface()?;
        match variant {
            crate::hardware::rgb::RgbInterfaceVariant::FourZone => {
                for (i, color) in colors.iter().enumerate() {
                    let zone_path = path.join(format!("zone0{i}"));
                    let hex = color.to_hex_raw();
                    write_and_verify(&zone_path, &hex, 3, Duration::from_millis(10))?;
                }
                Ok(())
            }
            crate::hardware::rgb::RgbInterfaceVariant::Legacy => {
                let payload = format!(
                    "{}{}{}{}",
                    colors[0].to_hex_raw(),
                    colors[1].to_hex_raw(),
                    colors[2].to_hex_raw(),
                    colors[3].to_hex_raw()
                );
                write_and_verify(&path, &payload, 3, Duration::from_millis(10))?;
                Ok(())
            }
            crate::hardware::rgb::RgbInterfaceVariant::KernelPatch => {
                let bytes: [u8; 12] = [
                    colors[0].r,
                    colors[0].g,
                    colors[0].b,
                    colors[1].r,
                    colors[1].g,
                    colors[1].b,
                    colors[2].r,
                    colors[2].g,
                    colors[2].b,
                    colors[3].r,
                    colors[3].g,
                    colors[3].b,
                ];
                fs::write(&path, bytes).map_err(|e| {
                    HardwareControlError::IoError(format!(
                        "Write to {} failed: {e}",
                        path.display()
                    ))
                })?;
                let read_back = fs::read(&path).map_err(|e| {
                    HardwareControlError::IoError(format!(
                        "Read-back from {} failed: {e}",
                        path.display()
                    ))
                })?;
                if read_back != bytes {
                    return Err(HardwareControlError::VerificationFailed {
                        target: format!("{bytes:?}"),
                        actual: format!("{read_back:?}"),
                        retries: 1,
                    });
                }
                Ok(())
            }
        }
    }
}
