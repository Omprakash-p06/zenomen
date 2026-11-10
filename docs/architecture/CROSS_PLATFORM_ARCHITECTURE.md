# Cross-Platform Architecture: Linux & Windows 10/11

ZenOmen is designed around a three-tier, decoupled architecture ensuring that neither platform leaks operating-system-specific details into core business logic or presentation components.

```
┌─────────────────────────────────────────────────────────────┐
│                    Presentation Layer                       │
│     Slint 1.18 (Production) / GPUI-CE 0.3.0 (Target)       │
└──────────────────────────────▲──────────────────────────────┘
                               │ Typed AppCommand / AppEvent
┌──────────────────────────────▼──────────────────────────────┐
│                        Core Layer                           │
│  State, Telemetry Buffers, Diagnostics, Event Queue Guard   │
└──────────────────────────────▲──────────────────────────────┘
                               │ HardwareProvider Trait
┌──────────────────────────────▼──────────────────────────────┐
│                    Hardware Layer                           │
│     Linux Provider (sysfs)   │   Windows Provider (Win32)   │
└──────────────────────────────▲──────────────────────────────┘
                               │ IPC (D-Bus / Named Pipe)
┌──────────────────────────────▼──────────────────────────────┐
│                  Privileged Services                        │
│   Linux: zenomen-hw (D-Bus)  │  Windows: zenomen-hw (Pipe)  │
└─────────────────────────────────────────────────────────────┘
```

## Architectural Principles

1. **Strict Seam Isolation (`ARCH-01`)**:
   - Platform selection is strictly compile-time via `#[cfg(target_os = "...")]`.
   - Linux binaries link no Win32/WMI libraries; Windows binaries link no zbus/sysfs/systemd modules.
   - Non-supported platforms compile cleanly using `UnavailableHardwareProvider`.

2. **Honest Hardware Reporting (`ARCH-03`, `FOUND-04`)**:
   - Zero guessing: missing sensors or unreadable telemetry return `None` or `SensorStatus::Unavailable`.
   - Never fabricate `0 RPM`, `0°C`, or fake battery states.

3. **Two-Process Boundary (`FOUND-05`)**:
   - The desktop GUI process is strictly unprivileged (Standard User) and never requires administrator rights.
   - All privileged BIOS and hardware mutations route through an isolated background service (`zenomen-hw`) guarded by strict caller authorization.
