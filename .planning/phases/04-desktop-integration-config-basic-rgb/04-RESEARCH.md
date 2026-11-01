# Phase 04: Desktop Integration, Config & Basic RGB — Research

**Phase:** 04 - Desktop Integration, Config & Basic RGB  
**Confidence:** HIGH  

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| **SYS-01** | User can open dashboard, see current profile, open diagnostics, toggle launch at login and quit from a system tray menu | Implemented via `org.kde.StatusNotifierItem` and `com.canonical.dbusmenu` over `zbus` session bus. |
| **SYS-02** | User can enable or disable starting ZenOmen at login (via XDG autostart) | Managed via `~/.config/autostart/zenomen.desktop` with `--minimized` flag; atomic write and existence check. |
| **SYS-03** | User can choose to start minimized | Handled via command-line `--minimized` / `-m` and `start_minimized` config option; window remains hidden until activated. |
| **SYS-04** | Closing the window respects close-to-tray preference, and quitting is a separate explicit action | `AppWindow::hide()` called on close when `close_to_tray` is true; "Quit ZenOmen" action terminates the process. |
| **SYS-05** | Only one instance runs; launching again focuses the existing window via D-Bus activation (`com.zenomen.App` `PresentWindow`) | Service registers `com.zenomen.App` on session D-Bus; second instance invokes `PresentWindow` and exits cleanly, providing 100% reliable fallback on GNOME without tray icons. |
| **SYS-06** | User settings persist across restarts in `config.toml` | Stored in `~/.config/zenomen/config.toml` using `directories::ProjectDirs` and `toml` serialization. |
| **SYS-07** | A corrupt or invalid config loads safe defaults, preserves original file for troubleshooting and informs user | Parsing failure backs up file to `config.toml.bak.<timestamp>`, falls back to `AppConfig::default()`, and logs warning. |
| **SYS-08** | User can select a theme (ZenOmen Dark) | Theme setting stored in `config.toml`; validated against available themes (`dark`). |
| **SYS-09** | User can configure monitoring intervals per metric, clamped to safe bounds | Configurable telemetry interval (clamped to 500ms - 5000ms) with history window (30s, 60s, 120s, 300s). |
| **SYS-10** | The app remains fully reachable when no system tray is available (tested on GNOME, KDE and X11) | Graceful detection of `StatusNotifierWatcher` absence; window closes or remains accessible without errors; single-instance D-Bus focuses existing window. |
| **DIAG-03** | User can export diagnostics to a file in one action, containing no unrelated personal data | Export verified in Settings screen; sanitizes output and writes atomic JSON report. |
| **RGB-01** | User can see whether keyboard RGB is supported, based on detection of hardware interface variant (`four_zone`, `legacy`, `kernel_patch`) | Discovers interface: `four_zone` (`sys/devices/platform/hp-wmi/rgb_zones`), `legacy` (`sys/devices/platform/hp-wmi/keyboardleds`), or `kernel_patch` (`sys/class/leds/hp_omen::kbd_backlight/zone_colors`). |
| **RGB-02** | User can see the currently active RGB profile | Active profile tracked in state and rendered on Lighting screen and StatusNotifierItem menu. |
| **RGB-03** | User can apply an existing RGB preset profile to the keyboard | Preset profiles (e.g. `white`, `rainbow`, `gaming`, `off`) sent via D-Bus `ApplyRgbProfile` to `zenomen-hw`. |
| **RGB-04** | User can enable and disable keyboard lighting | Power toggle invokes D-Bus `SetRgbEnabled` on `zenomen-hw`. |
| **RGB-05** | Existing OpenOMEN RGB profiles in `~/.config/openomen/rgb/` are migrated without deleting, overwriting, or discarding any source file | Non-destructive scan and copy of `*.json` from `~/.config/openomen/rgb/` into `~/.config/zenomen/rgb/`. |
| **RGB-06** | An invalid color value produces a clear error message instead of an uncaught traceback (strict `#RRGGBB` format) | Hex regex/parser validates 6 hex chars `#RRGGBB` per zone; errors return friendly validation messages. |
| **RGB-07** | Profile fields hardware cannot honour (e.g. animation: `brightness`, `speed`, `length_ms`, `change`, `loop`) are clearly identified as unsupported in v1 without failing import | Unsupported keys are recorded in `unsupported_fields: Vec<String>` in metadata; static zone colors are extracted. |
| **RGB-08** | RGB hardware writes applied through privileged service and verified by reading back state wherever interface supports it | Daemon writes to sysfs paths with polkit check, reading back where readable (e.g. `zone_colors` or `zone0{i}`). |
</phase_requirements>

## Standard Stack & Architecture Patterns

### 1. Pure `zbus` StatusNotifierItem & Session D-Bus Activation
- **Memory & Resource Budget Constraint:**
  - Standard tray crates like `tray-icon` and `ksni` pull in heavy C dependencies (`gtk`, `gtk4`, `tao`, `eframe`), adding 50–100 MB of heap/RSS overhead and violating the `<=40 MB` peak RSS budget (PERF-01).
  - `zenomen` already links `zbus = { version = "5", features = ["blocking"] }`.
  - Implementing the standard freedesktop StatusNotifierItem (SNI) protocol (`org.kde.StatusNotifierItem`) directly over `zbus`:
    1. Uses **zero** additional dependencies.
    2. Runs on the user session bus.
    3. Exposes properties (`Category`, `Id`, `Title`, `Status`, `IconName`, `Menu`) and methods (`Activate`, `ContextMenu`).
    4. Registers with `org.kde.StatusNotifierWatcher` (if available on KDE, Cinnamon, XFCE, or GNOME with AppIndicator extension). If the watcher is missing, it logs an info message and does not crash or block.
- **Single-Instance Application (`com.zenomen.App`):**
  - Session D-Bus interface:
    ```rust
    #[zbus::interface(name = "com.zenomen.App")]
    impl ApplicationService {
        async fn present_window(&self) -> zbus::fdo::Result<()>;
        async fn quit(&self) -> zbus::fdo::Result<()>;
    }
    ```
  - When starting up:
    1. Connect to session D-Bus.
    2. Attempt to request name `com.zenomen.App` with `RequestNameFlags::DoNotQueue`.
    3. If name is already owned, call `present_window()` on existing owner and terminate current process with exit code 0.
    4. If name is acquired, run main application loop.

### 2. Configuration Persistence (`config.toml`)
- **Path Resolution:**
  - Resolved via `directories::ProjectDirs::from("", "", "zenomen")` -> `~/.config/zenomen/config.toml`.
- **Atomic File Writing:**
  - Write to `config.toml.tmp` in the same directory, call `sync_all()`, then `fs::rename(tmp, target)`.
  - Prevents corrupt or partially written configuration files during abrupt termination.
- **Corrupt File Recovery:**
  - On deserialization error:
    1. Create backup: `config.toml.bak.<timestamp>`.
    2. Fall back to `AppConfig::default()`.
    3. Dispatch `AppEvent::ConfigCorrupted(backup_path)` to inform user on UI.

### 3. Keyboard RGB Hardware Interfaces
- From reverse-engineering the upstream driver and OpenOMEN:
  1. **`four_zone` interface:**
     - Path: `/sys/devices/platform/hp-wmi/rgb_zones/zone0{0..3}`
     - Value: 6-character hex string without `#` (e.g. `FF0000`).
  2. **`legacy` interface:**
     - Path: `/sys/devices/platform/hp-wmi/keyboardleds`
     - Value: 24-character concatenated hex string (4 zones x 6 hex chars, e.g. `FF000000FF000000FFFFFFFF`).
  3. **`kernel_patch` interface:**
     - Path: `/sys/class/leds/hp_omen::kbd_backlight/zone_colors`
     - Value: 12 raw binary bytes `[r0, g0, b0, r1, g1, b1, r2, g2, b2, r3, g3, b3]`.
- **Read-Back Verification:**
  - For `kernel_patch` and `four_zone`, sysfs files can be read back to verify byte matching.
  - If read-back is unsupported by kernel driver (e.g. write-only attribute in legacy), verify write success via `flush()` and absence of `EIO`.

### 4. OpenOMEN RGB Profile Migration
- Source directory: `~/.config/openomen/rgb/`
- Destination directory: `~/.config/zenomen/rgb/`
- Rules:
  - Source files are strictly **read-only**; never modified or deleted.
  - Each `*.json` file is parsed:
    - If valid JSON with `zones` or `colors`, extract 4-zone colors.
    - Validate each zone color strictly against `^#?[0-9a-fA-F]{6}$`.
    - If unsupported keys exist (`brightness`, `speed`, `length_ms`, `change`, `loop`), note them as unsupported in metadata without failing.
  - Write normalized JSON to `~/.config/zenomen/rgb/`.

### 5. Multi-Platform Logo & Icon Integration
- **Serene Eyeball Logo (`logo/zenomen-logo-v3-indigo-gold.svg`):**
  - Updated by removing the flame from the pupil and replacing it with a calm dark pupil with a delicate gold accent ring and specular reflections.
  - Serves as the single master vector asset across all platforms.
- **Icon Packaging Strategy:**
  - **Linux Desktop & `.deb`/AppImage:** Scalable SVG installed at `data/icons/zenomen.svg` and standard PNG raster icons (16, 32, 48, 64, 128, 256, 512px) in `data/icons/hicolor/`. Referenced by `zenomen.desktop` (`Icon=zenomen`) and StatusNotifierItem (`IconName = "zenomen"`).
  - **Slint GUI Window:** Embedded via `icon: @image-url("../logo/zenomen-logo-v3-indigo-gold.svg")` on `AppWindow`.
  - **Windows `.exe` Binary:** Multi-resolution `.ico` file `data/icons/zenomen.ico` embedded into the PE binary resource table via `winres` in `build.rs` when compiling for Windows target.

---

## Plan Decomposition

| Plan | Focus | Key Deliverables |
|:---|:---|:---|
| **04-01** | Desktop Integration & Single-Instance | Single-instance session D-Bus service (`com.zenomen.App`), `PresentWindow` handler, StatusNotifierItem tray service, close-to-tray lifecycle, XDG autostart helper, and serene logo/icon embedding across Linux and Windows. |
| **04-02** | Config Persistence & Settings UI | `AppConfig` TOML schema, atomic writes, corrupt file backup/recovery, telemetry polling bounds, Settings screen UI binding. |
| **04-03** | Keyboard RGB Lighting & Migration | Hardware variant detection (`four_zone`, `legacy`, `kernel_patch`), strict `#RRGGBB` parser, safe OpenOMEN profile migration, privileged service write/read-back, Lighting screen UI. |

---

## Verification Strategy

1. **Plan 04-01 Tests:**
   - Single-instance activation: launching second instance triggers `PresentWindow` and exits 0.
   - XDG autostart toggle: creates/removes valid `~/.config/autostart/zenomen.desktop`.
   - Close-to-tray: window hide vs process exit.
2. **Plan 04-02 Tests:**
   - Config save and load: round-trip serde tests.
   - Corrupt config handling: verifies backup `.bak` creation and fallback to defaults without panicking.
   - Telemetry interval clamping: checks min/max clamping (500ms - 5000ms).
3. **Plan 04-03 Tests:**
   - RGB color validator: accepts valid `#RRGGBB` and `#rrggbb`, rejects invalid lengths, bad hex, and non-hex inputs with clear errors.
   - OpenOMEN migration: imports valid profiles from mock directory, preserves source files untouched, flags animation fields as unsupported.
   - Hardware writes: tests mock sysfs writes across all 3 interface variants (`four_zone`, `legacy`, `kernel_patch`).
