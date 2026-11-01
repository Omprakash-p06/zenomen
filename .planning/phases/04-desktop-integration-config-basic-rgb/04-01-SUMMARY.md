---
phase: "04-desktop-integration-config-basic-rgb"
plan: "01"
subsystem: desktop/integration
tags:
  - rust
  - dbus
  - zbus
  - single-instance
  - sni
  - system-tray
  - autostart
  - icons

requires:
  - 03-03
provides:
  - "Single-instance session D-Bus service com.zenomen.App with PresentWindow and Quit RPC"
  - "Pure zbus StatusNotifierItem (SNI) and DBusMenu system tray service (zero GTK/Qt dependencies)"
  - "Close-to-tray lifecycle management with --minimized / -m CLI startup support"
  - "XDG login autostart manager (~/.config/autostart/zenomen.desktop)"
  - "Application branding with serene eye vector logo and multi-resolution PNG/ICO icon hierarchy"
affects:
  - 04-02 (Configuration persistence, Settings UI)
  - 04-03 (Basic RGB Lighting)
  - 05-01 (Debian and AppImage packaging)

actuals:
  tasks: 6
  tests: 6 added (72 total passing across 12 test suites)

tech-stack:
  added:
    - "src/desktop/autostart.rs (XDG autostart manager)"
    - "src/desktop/single_instance.rs (D-Bus session service com.zenomen.App)"
    - "src/desktop/tray.rs (org.kde.StatusNotifierItem & com.canonical.dbusmenu over pure zbus)"
    - "src/desktop/mod.rs (Desktop subsystem exports)"
    - "data/icons/zenomen.svg & data/icons/zenomen.ico"
    - "data/icons/hicolor/{16, 32, 48, 64, 128, 256, 512}x<size>/apps/zenomen.png"
    - "tests/desktop_integration_tests.rs"
---

# Plan 04-01 Summary: Desktop Integration, Single Instance, Tray & Icons

## Overview

Plan 04-01 establishes ZenOmen's native Linux desktop integration while strictly protecting its lightweight resource budget ($\le 40$ MB peak RSS, $\le 25$ MB minimized in tray). By implementing the freedesktop StatusNotifierItem (SNI) protocol and DBusMenu directly over `zbus`, the system tray runs with zero heavy C toolkit dependencies (GTK, Qt, Tao). Single-instance enforcement on the user session bus (`com.zenomen.App`) provides a 100% reliable fallback on desktop environments like stock GNOME where tray icons are absent: running `zenomen` a second time brings the existing window to the front via `PresentWindow` and exits cleanly.

In addition, the application logo was updated to remove the flame from the eyeball pupil, replacing it with a serene dark pupil with a delicate gold accent ring and specular reflections. The logo is packaged as scalable SVG, multi-resolution PNGs, and Windows `.ico` for `.exe` resource embedding.

## Key Changes

1. **Single-Instance Session D-Bus Service (`src/desktop/single_instance.rs`):**
   - Implements `ensure_single_instance`: attempts to claim `com.zenomen.App` with `DBUS_NAME_FLAG_DO_NOT_QUEUE`.
   - If already running, invokes `PresentWindow` on primary instance and exits immediately with code 0 (SYS-05).
   - Primary instance exposes D-Bus interface `com.zenomen.App` (`/com/zenomen/App`) handling `PresentWindow` and `Quit`.

2. **Pure `zbus` StatusNotifierItem System Tray (`src/desktop/tray.rs`):**
   - Implements `org.kde.StatusNotifierItem` and `com.canonical.dbusmenu` (`/StatusNotifierItem`, `/MenuBar`).
   - Registers with `org.kde.StatusNotifierWatcher` on session bus; tolerates absent watcher gracefully without errors or crashes on stock GNOME (SYS-01, SYS-10).
   - Tray activation and menu events dispatch `DesktopAction::PresentWindow` and `DesktopAction::Quit`.

3. **Close-to-Tray & Startup Lifecycle (`src/main.rs`, `ui/app.slint`):**
   - Handles `--minimized` / `-m` CLI flags to start minimized in tray without showing window on cold start (SYS-03).
   - Attaches `on_close_requested` on `AppWindow`, returning `CloseRequestResponse::HideWindow` to keep the application active in tray when closed (SYS-04).
   - Quitting from tray menu or desktop action cleanly exits the Slint event loop.

4. **XDG Autostart Manager (`src/desktop/autostart.rs`):**
   - Atomically manages `~/.config/autostart/zenomen.desktop` with `Exec=zenomen --minimized` and `Icon=zenomen` (SYS-02).
   - Safely parses and toggles `Hidden=true` / file removal.

5. **Multi-Platform Icon & Branding Integration (`data/icons/`, `ui/app.slint`, `build.rs`):**
   - Updated master logo [`logo/zenomen-logo-v3-indigo-gold.svg`](file:///home/omprakash/Documents/zenomen/logo/zenomen-logo-v3-indigo-gold.svg) to remove the flame pupil.
   - Generated standard hicolor PNG icons (16x16 through 512x512) and `zenomen.ico`.
   - Wired `icon: @image-url("../data/icons/zenomen.png")` in `ui/app.slint`.
   - Configured `winres` in `build.rs` for Windows target icon resource embedding.

## Verification

- `cargo clippy --all-targets -- -D warnings`: 0 warnings.
- `cargo test --test desktop_integration_tests`: 6 tests passing.
- `cargo test --all`: 72 tests passing across 12 test suites.
