//! D-Bus interface contract for com.zenomen.Hardware1.

use zbus::Connection;
use zbus::interface;
use zbus::message::Header;

use crate::service::backend::HardwareBackend;
use crate::service::polkit::{POLKIT_ACTION_CONTROL, PolkitVerifier};

use std::sync::{Arc, RwLock};

/// D-Bus service exposing verified hardware control endpoints under `com.zenomen.Hardware1`.
#[derive(Clone)]
pub struct HardwareService {
    backend: HardwareBackend,
    conn: Arc<RwLock<Option<Connection>>>,
    require_polkit: bool,
    last_active_rgb: Arc<RwLock<Option<[crate::rgb::RgbColor; 4]>>>,
}

impl HardwareService {
    /// Create a new HardwareService wrapping the specified backend.
    pub fn new(backend: HardwareBackend) -> Self {
        Self {
            backend,
            conn: Arc::new(RwLock::new(None)),
            require_polkit: true,
            last_active_rgb: Arc::new(RwLock::new(None)),
        }
    }

    /// Associate an active D-Bus connection for polkit verification queries.
    pub fn set_connection(&self, conn: Connection) {
        if let Ok(mut lock) = self.conn.write() {
            *lock = Some(conn);
        }
    }

    /// Associate an active D-Bus connection (builder pattern).
    pub fn with_connection(self, conn: Connection) -> Self {
        self.set_connection(conn);
        self
    }

    /// Enable or disable polkit requirement (test builds only).
    #[cfg(test)]
    pub fn with_polkit_required(mut self, require_polkit: bool) -> Self {
        self.require_polkit = require_polkit;
        self
    }

    /// Access the underlying backend.
    pub fn backend(&self) -> &HardwareBackend {
        &self.backend
    }

    /// Enforce polkit authorization for mutating operations.
    async fn authorize(&self, hdr: &Header<'_>) -> Result<(), zbus::fdo::Error> {
        if !self.require_polkit {
            return Ok(());
        }

        let sender = hdr.sender().ok_or_else(|| {
            zbus::fdo::Error::AccessDenied("Caller sender missing in D-Bus message".to_string())
        })?;

        let conn = {
            let lock = self
                .conn
                .read()
                .map_err(|e| zbus::fdo::Error::Failed(format!("Lock poisoned: {e}")))?;
            lock.as_ref().cloned().ok_or_else(|| {
                zbus::fdo::Error::Failed("D-Bus connection not initialized on service".to_string())
            })?
        };

        PolkitVerifier::check_authorization(&conn, sender.as_str(), POLKIT_ACTION_CONTROL, true)
            .await
            .map_err(zbus::fdo::Error::from)?;

        Ok(())
    }

    /// Internal implementation for applying an RGB profile.
    pub fn apply_rgb_profile_internal(
        &self,
        profile: &crate::rgb::RgbProfile,
    ) -> Result<String, crate::service::types::HardwareControlError> {
        self.backend.apply_rgb_hardware(&profile.zones)?;
        if let Ok(mut lock) = self.last_active_rgb.write() {
            *lock = Some(profile.zones);
        }
        Ok(format!("RGB profile '{}' applied", profile.name))
    }

    /// Internal implementation for toggling RGB enabled state with profile restoration.
    pub fn set_rgb_enabled_internal(
        &self,
        enabled: bool,
    ) -> Result<String, crate::service::types::HardwareControlError> {
        if enabled {
            let restore_zones = self
                .last_active_rgb
                .read()
                .ok()
                .and_then(|lock| *lock)
                .unwrap_or([crate::rgb::RgbColor::new(255, 255, 255); 4]);

            self.backend.apply_rgb_hardware(&restore_zones)?;
        } else {
            let black = [crate::rgb::RgbColor::new(0, 0, 0); 4];
            self.backend.apply_rgb_hardware(&black)?;
        }
        Ok(format!("RGB enabled set to {enabled}"))
    }
}

#[interface(name = "com.zenomen.Hardware1")]
impl HardwareService {
    /// Return capabilities JSON (e.g. fan control, power profiles availability).
    async fn get_capabilities(&self) -> zbus::fdo::Result<String> {
        let has_fan_control = self.backend.find_fan_control_path().is_ok();
        let has_power_profiles = self.backend.get_power_profile_choices().is_ok();
        let rgb_info = self.backend.find_rgb_interface().ok();
        let has_rgb_lighting = rgb_info.is_some();
        let rgb_variant = rgb_info.map(|(v, _)| v.name());

        let caps = serde_json::json!({
            "has_fan_control": has_fan_control,
            "has_power_profiles": has_power_profiles,
            "has_rgb_lighting": has_rgb_lighting,
            "rgb_variant": rgb_variant,
            "service_version": env!("CARGO_PKG_VERSION"),
        });

        Ok(caps.to_string())
    }

    /// Read active fan mode and current fan RPMs.
    async fn get_fan_status(&self) -> zbus::fdo::Result<(String, Vec<u32>)> {
        let mode = self
            .backend
            .get_fan_mode()
            .map(|m| m.as_str().to_string())
            .unwrap_or_else(|_| "unavailable".to_string());
        let rpms = self.backend.get_fan_rpms();
        Ok((mode, rpms))
    }

    /// Set fan mode with verification.
    /// Mutating method: checks polkit `com.zenomen.hardware.control`.
    async fn set_fan_mode(
        &self,
        #[zbus(header)] hdr: Header<'_>,
        mode: String,
    ) -> zbus::fdo::Result<String> {
        self.authorize(&hdr).await?;
        let confirmed = self
            .backend
            .set_fan_mode(&mode)
            .map_err(zbus::fdo::Error::from)?;
        Ok(confirmed)
    }

    /// List advertised ACPI power profile choices.
    async fn get_power_profiles(&self) -> zbus::fdo::Result<Vec<String>> {
        self.backend
            .get_power_profile_choices()
            .map_err(zbus::fdo::Error::from)
    }

    /// Read active ACPI power profile.
    async fn get_power_profile(&self) -> zbus::fdo::Result<String> {
        self.backend
            .get_power_profile()
            .map_err(zbus::fdo::Error::from)
    }

    /// Set active ACPI power profile with verification.
    /// Mutating method: checks polkit `com.zenomen.hardware.control`.
    async fn set_power_profile(
        &self,
        #[zbus(header)] hdr: Header<'_>,
        profile: String,
    ) -> zbus::fdo::Result<String> {
        self.authorize(&hdr).await?;
        let confirmed = self
            .backend
            .set_power_profile(&profile)
            .map_err(zbus::fdo::Error::from)?;
        Ok(confirmed)
    }

    /// Apply a combined preset (Quiet, Balanced, Gaming) with granular PresetResult.
    /// Mutating method: checks polkit `com.zenomen.hardware.control`.
    async fn apply_preset(
        &self,
        #[zbus(header)] hdr: Header<'_>,
        preset: String,
    ) -> zbus::fdo::Result<String> {
        self.authorize(&hdr).await?;
        let result = self
            .backend
            .apply_preset(&preset)
            .map_err(zbus::fdo::Error::from)?;
        serde_json::to_string(&result).map_err(|e| zbus::fdo::Error::Failed(e.to_string()))
    }

    /// Apply an RGB profile JSON payload.
    /// Mutating method: checks polkit `com.zenomen.hardware.control`.
    async fn apply_rgb_profile(
        &self,
        #[zbus(header)] hdr: Header<'_>,
        profile_json: String,
    ) -> zbus::fdo::Result<String> {
        self.authorize(&hdr).await?;

        let profile: crate::rgb::RgbProfile = serde_json::from_str(&profile_json)
            .map_err(|e| zbus::fdo::Error::InvalidArgs(format!("Invalid RGB profile JSON: {e}")))?;

        self.apply_rgb_profile_internal(&profile)
            .map_err(zbus::fdo::Error::from)
    }

    /// Toggle RGB lighting state (RGB-04).
    /// Mutating method: checks polkit `com.zenomen.hardware.control`.
    async fn set_rgb_enabled(
        &self,
        #[zbus(header)] hdr: Header<'_>,
        enabled: bool,
    ) -> zbus::fdo::Result<String> {
        self.authorize(&hdr).await?;
        self.set_rgb_enabled_internal(enabled)
            .map_err(zbus::fdo::Error::from)
    }
}
