# ADR-004: UI Platform Abstraction and Hardware Worker Decoupling

- **Status:** Accepted (2026-11-07)
- **Date:** 2026-11-07
- **Decision Owner:** ZenOmen UI & Core Architecture Team
- **Related:** `docs/architecture/CROSS_PLATFORM_ARCHITECTURE.md`, `src/core/worker.rs`, `src/app.rs`, `src/ui/`

## Context

ZenOmen supports Linux and Windows 10/11 desktop environments. The user interface must support fast telemetry updates (1000ms polling), real-time SVG charting, user interactions (fan curves, power presets, RGB lighting), and system tray integration, all while maintaining a strict resource ceiling (<= 40MB RSS, <= 0.1% idle CPU).

Coupling the presentation layer directly to OS-specific telemetry mechanisms (sysfs on Linux, PDH/DXGI on Windows) would introduce platform conditional compilation throughout the UI code, making testing and maintainability fragile.

## Decision

**ZenOmen completely decouples the presentation layer from hardware implementation via the `HardwareWorker` and asynchronous command/event message passing (`AppCommand` and `AppEvent`).**

1. **Unidirectional Event Loop:**
   - The UI thread sends strongly typed `AppCommand` values over a bounded channel.
   - The `HardwareWorker` background thread executes the command against the platform-specific `HardwareProvider`.
   - Resulting state updates and telemetry samples are dispatched back to the UI thread as `AppEvent` values.
2. **Abstract Hardware Contract:**
   The UI interacts exclusively with the `HardwareProvider` trait (`src/hardware/provider.rs`), receiving agnostic structs (`SystemMetrics`, `FanSpeed`, `BatteryTelemetry`, `PowerPreset`).
3. **Pluggable Presentation Engine:**
   Slint 1.18.1 remains the battle-tested production UI, while the modular design allows staging the unified GPUI-CE implementation (`src/ui/`) without modifying core hardware logic, telemetry filters, or IPC workers.

## Consequences

**Positive:**
- Zero platform-specific `#cfg(target_os)` branches inside UI components or charting logic.
- UI can be thoroughly verified in headless CI test runs using `MockHardwareProvider` without hardware or root/administrator access.
- Seamless transition path between Slint and GPUI-CE.

**Negative / Accepted:**
- Small IPC channel dispatch overhead (~microsecond latency), which is negligible compared to the 1000ms telemetry cycle.
