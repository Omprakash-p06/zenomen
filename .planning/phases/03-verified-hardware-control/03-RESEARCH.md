# Phase 03: Verified Hardware Control — Research

**Phase:** 03 - Verified Hardware Control  
**Confidence:** HIGH  

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| **COOL-01** | User can see the RPM of each detected fan, with an unavailable state when a fan sensor is absent | Implemented in Phase 2 via `SensorRegistry`; verified across UI and D-Bus service status. |
| **COOL-02** | User can see current fan mode (Auto, Max, Manual) rather than collapsing unknown values | Kernel reports `0` (Max), `1` (Manual), `2` (Auto) from `pwm1_enable`; mapped into typed enum without lossy conversion. |
| **COOL-03** | User can switch between validated fan modes (Auto and Max for v1; manual mode not offered without curves) | UI exposes Auto and Max; requests validated and sent over D-Bus; unsupported modes rejected with explicit error. |
| **COOL-04** | Every fan-mode write is verified by reading state back with retry/backoff, reporting mismatch as error | Write loop executes write to `pwm1_enable`, waits 50ms, reads back; retries up to 3 times (50ms, 100ms, 200ms). Failure returns `VERIFICATION_FAILED`. |
| **COOL-05** | Fan controls are hidden/disabled with plain explanation when interface is missing or unwritable | Gated on `HardwareCapabilities.has_fan_control` and service connection status; renders explanatory status when absent. |
| **COOL-06** | User is not required to grant privilege or change fan behaviour to view telemetry | Read-only telemetry runs in unprivileged GUI via sysfs/procfs; polkit authorization only prompted on mutating writes. |
| **PWR-01** | User can see ACPI platform profiles dynamically read from `platform_profile_choices` | Enumerated dynamically from sysfs choices file (`quiet`, `balanced`, `performance`, `cool`, etc.); no hardcoded list. |
| **PWR-02** | User can see currently active power profile | Read dynamically from `/sys/firmware/acpi/platform_profile`. |
| **PWR-03** | User can apply an advertised power profile | Mutating method `SetPowerProfile(profile)` sent over D-Bus to `zenomen-hw` after choice validation. |
| **PWR-04** | Applied power profile is verified by re-reading state with retry/backoff | Write loop writes profile string, verifies read-back matching applied profile; fails honestly if not sticking. |
| **PWR-05** | User can apply combined preset (Quiet, Balanced, Gaming) applying profile and fan mode together | Presets map: Gaming -> (performance, max), Balanced -> (balanced, auto), Quiet -> (cool/quiet, auto). |
| **PWR-06** | Combined presets report granular execution status via `PresetResult` | Structured response `{ power_applied: bool, fan_applied: bool, partial_success: bool, message: String }` instead of claiming atomic transactions. |
| **SVC-01** | GUI process runs as normal user — no sudo re-exec, no setuid, no root GUI | GUI executable `zenomen` strictly drops/avoids privilege; mutating calls delegated to background daemon. |
| **SVC-02** | All privileged hardware writes go through `com.zenomen.Hardware1` D-Bus system service | Daemon binary `zenomen-hw` registers name `com.zenomen.Hardware1` on system bus at `/com/zenomen/Hardware1`. |
| **SVC-03** | Mutating actions authorized through polkit for calling user (mandatory) | Daemon queries `org.freedesktop.PolicyKit1.Authority` for action `com.zenomen.hardware.control` using caller's bus unique name. |
| **SVC-04** | Service implements formalized API contract rejecting arbitrary paths or commands | Methods: `GetCapabilities`, `GetFanStatus`, `SetFanMode`, `GetPowerProfiles`, `GetPowerProfile`, `SetPowerProfile`, `ApplyPreset`, `ApplyRgbProfile`, `SetRgbEnabled`. Parameter types strictly validated. |
| **SVC-05** | Service runs under systemd unit sandboxed to required sysfs paths | Systemd unit uses `ProtectSystem=strict`, `ProtectHome=true`, `ReadOnlyPaths=/`, `ReadWritePaths=/sys/devices/platform/hp-wmi /sys/firmware/acpi`. |
| **SVC-06** | GUI stays fully usable for telemetry when service is unavailable or disconnected | D-Bus client handles connection failures gracefully; UI disables control buttons with `Service Disconnected` notice while telemetry continues updating. |
| **DIAG-04** | User can see whether hardware service is connected and permissions are sufficient | Diagnostics report and UI show daemon status (`Connected`, `Disconnected`, `Unavailable`) and polkit test check. |
</phase_requirements>

## Standard Stack & Architecture Patterns

### 1. Two-Process Architecture & D-Bus Contract (`zbus`)
- **System Service (`zenomen-hw`):**
  - Compiled as separate binary in `src/bin/zenomen-hw.rs` sharing `zenomen` core library types.
  - Connects to system bus (`zbus::connection::Builder::system()`).
  - Request name: `com.zenomen.Hardware1`.
  - Object Path: `/com/zenomen/Hardware1`.
  - Interface: `com.zenomen.Hardware1`.
- **D-Bus Interface Methods:**
  ```rust
  #[zbus::interface(name = "com.zenomen.Hardware1")]
  impl HardwareService {
      async fn get_capabilities(&self) -> zbus::fdo::Result<String>;
      async fn get_fan_status(&self) -> zbus::fdo::Result<(String, Vec<u32>)>;
      async fn set_fan_mode(&self, #[zbus(header)] hdr: &Header<'_>, mode: String) -> zbus::fdo::Result<String>;
      async fn get_power_profiles(&self) -> zbus::fdo::Result<Vec<String>>;
      async fn get_power_profile(&self) -> zbus::fdo::Result<String>;
      async fn set_power_profile(&self, #[zbus(header)] hdr: &Header<'_>, profile: String) -> zbus::fdo::Result<String>;
      async fn apply_preset(&self, #[zbus(header)] hdr: &Header<'_>, preset: String) -> zbus::fdo::Result<String>;
      async fn apply_rgb_profile(&self, #[zbus(header)] hdr: &Header<'_>, profile_json: String) -> zbus::fdo::Result<String>;
      async fn set_rgb_enabled(&self, #[zbus(header)] hdr: &Header<'_>, enabled: bool) -> zbus::fdo::Result<String>;
  }
  ```

### 2. Mandatory Polkit Authorization (`org.freedesktop.PolicyKit1`)
- When a client invokes any mutating method (`set_fan_mode`, `set_power_profile`, `apply_preset`, etc.):
  1. Extract caller's unique bus name from message header: `hdr.sender()?.ok_or(...)`.
  2. Invoke `CheckAuthorization` on `org.freedesktop.PolicyKit1` (`/org/freedesktop/PolicyKit1/Authority`):
     - Subject: `("system-bus-name", [("name", Variant(caller_name))])`
     - Action ID: `"com.zenomen.hardware.control"`
     - Details: map with metadata (e.g. `action: "set_fan_mode"`)
     - Flags: `1` (`AllowUserInteraction` — permits polkit agent to prompt for password if required)
     - Cancellation ID: `""`
  3. If authorized (`result.is_authorized == true`), execute write.
  4. If challenged/dismissed or denied, return `zbus::fdo::Error::AccessDenied("Polkit authentication failed or cancelled")`.

### 3. Verified Sysfs Write Loop with Exponential Backoff
- Hardware writes cannot be assumed to succeed just because the file write returned `Ok(())`. Some firmware controllers (EC) take dozens of milliseconds to synchronize register changes, or silently reject commands under certain thermal/battery conditions.
- **Verification Algorithm:**
  ```rust
  pub fn write_and_verify(
      path: &Path,
      target_value: &str,
      max_retries: usize,
      initial_delay: Duration,
  ) -> Result<(), HardwareControlError> {
      let mut delay = initial_delay;
      for attempt in 1..=max_retries {
          std::fs::write(path, target_value)
              .map_err(|e| HardwareControlError::IoError(format!("write to {} failed: {e}", path.display())))?;

          std::thread::sleep(delay);

          let current = std::fs::read_to_string(path)
              .map_err(|e| HardwareControlError::IoError(format!("read-back from {} failed: {e}", path.display())))?;

          if current.trim() == target_value.trim() {
              return Ok(());
          }

          delay = delay.saturating_mul(2);
      }

      Err(HardwareControlError::VerificationFailed {
          target: target_value.to_string(),
          actual: std::fs::read_to_string(path).unwrap_or_default().trim().to_string(),
          retries: max_retries,
      })
  }
  ```

### 4. Fan Modes & Platform Profiles
- **Fan Modes (hp-wmi `pwm1_enable`):**
  - Mode `"max"` maps to `'0'` (disables BIOS automatic control, triggers maximum cooling RPM).
  - Mode `"auto"` maps to `'2'` (enables BIOS automatic curve control).
  - Mode `"manual"` (`'1'`) is **not** exposed in v1 (preventing unsafe thermal states without custom curves).
  - Any parameter other than `"auto"` or `"max"` is rejected with validation error.
- **Power Profiles (ACPI `platform_profile`):**
  - Read choices from `/sys/firmware/acpi/platform_profile_choices`.
  - Validate requested profile against choices before writing to `/sys/firmware/acpi/platform_profile`.
- **Combined Presets (`PresetResult`):**
  - Preset `"gaming"`: `power = "performance"`, `fan = "max"`.
  - Preset `"balanced"`: `power = "balanced"`, `fan = "auto"`.
  - Preset `"quiet"`: `power = "cool"` (or fallback `"quiet"` / `"low-power"` depending on choices), `fan = "auto"`.
  - Execution runs power profile write, then fan mode write.
  - Returns `PresetResult`:
    ```rust
    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct PresetResult {
        pub preset_name: String,
        pub power_applied: bool,
        pub fan_applied: bool,
        pub partial_success: bool,
        pub message: String,
    }
    ```

### 5. D-Bus Client Layer in GUI (`src/service/client.rs`)
- GUI process instantiates `HardwareServiceClient` on startup (or on demand).
- Checks if `com.zenomen.Hardware1` is available on system bus with non-blocking timeout.
- Exposes:
  - `status(&self) -> ServiceConnectionStatus` (`Connected`, `Disconnected`, `Unavailable`).
  - `set_fan_mode(&self, mode: &str) -> Result<String, String>`
  - `set_power_profile(&self, profile: &str) -> Result<String, String>`
  - `apply_preset(&self, preset: &str) -> Result<PresetResult, String>`
- If disconnected or failed, surfaces typed error to UI without crashing.

### 6. Systemd Sandboxing & Installation Assets
- **Systemd Unit (`data/systemd/zenomen-hw.service`):**
  - `Type=dbus`, `BusName=com.zenomen.Hardware1`.
  - Strict sandboxing:
    - `ProtectSystem=strict`
    - `ProtectHome=true`
    - `ReadOnlyPaths=/`
    - `ReadWritePaths=/sys/devices/platform/hp-wmi /sys/firmware/acpi`
    - `PrivateTmp=true`, `ProtectKernelTunables=true`, `ProtectControlGroups=true`.
- **D-Bus Config (`data/dbus/com.zenomen.Hardware1.conf`):**
  - Allows root to own bus name.
  - Allows default users to send messages to destination.
- **Polkit Policy (`data/polkit/com.zenomen.hardware.policy`):**
  - Defines action `com.zenomen.hardware.control`.
  - Defaults: `allow_active=auth_admin_keep`, `allow_inactive=auth_admin`.

---

## Plan Decomposition

- **Plan 03-01 (Wave 1): `zenomen-hw` Service Core & D-Bus Contract**
  - D-Bus contract definition and data types (`src/service/types.rs`, `src/service/contract.rs`).
  - Polkit authorization integration via `org.freedesktop.PolicyKit1.Authority` (`src/service/polkit.rs`).
  - Hardware write and verified read-back engine with retry/backoff (`src/service/backend.rs`).
  - Service binary `src/bin/zenomen-hw.rs`.
  - D-Bus configuration, polkit policy, and systemd unit files in `data/`.
  - Integration tests for verification loop and error handling (`tests/hardware_service_tests.rs`).

- **Plan 03-02 (Wave 2, depends on 03-01): Verified Fan Control & Cooling Screen**
  - Client D-Bus bridge in GUI (`src/service/client.rs`).
  - Wire `AppCommand::SetFanMode(String)` in core events, worker, and state (`src/core/events.rs`, `src/core/state.rs`).
  - Interactive fan mode selector (Auto / Max) in `ui/screens/cooling.slint` and `ui/app.slint`.
  - Surface read-back confirmation and verification failure errors in UI.
  - Integration tests for fan mode switching and read-back verification.

- **Plan 03-03 (Wave 3, depends on 03-02): ACPI Power Profiles, Combined Presets & Diagnostics**
  - Wire `AppCommand::SetPowerProfile(String)` and `AppCommand::ApplyPreset(String)`.
  - Implement `PresetResult` handling in core and UI.
  - Interactive profile choice selector and combined preset buttons (Quiet, Balanced, Gaming) in `ui/screens/performance.slint`.
  - Update `ui/screens/diagnostics.slint` and `src/core/diagnostics.rs` with `DIAG-04` Hardware Service connection and permission status.
  - End-to-end integration tests for presets and power profiles (`tests/power_preset_tests.rs`).
