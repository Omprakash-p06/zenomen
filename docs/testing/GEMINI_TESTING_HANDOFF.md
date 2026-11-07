# External Verification & Regression Testing Handoff

- **Target Audience:** Independent validation agents, automated test orchestrators, QA engineers
- **Release Target:** ZenOmen v1.0.0 Unified Cross-Platform Release (Windows 10/11 + Linux)
- **Git Branch:** `fix/zenomen-reliability` / `feat/cross-platform-release`

---

## 1. Quick Verification Pipeline

Before reviewing or merging code, execute the full non-destructive automated verification suite:

```bash
# 1. Format check
cargo fmt --all -- --check

# 2. Git whitespace check
git diff --check

# 3. Clippy on all host targets and features
cargo clippy --all-targets --all-features -- -D warnings

# 4. Cross-compilation typecheck for Windows targets
cargo clippy --target x86_64-pc-windows-msvc --bin zenomen -- -D warnings
cargo clippy --target x86_64-pc-windows-msvc --bin zenomen-hw -- -D warnings

# 5. Icon specification & dimensions validator
python3 ./scripts/validate-icons.py

# 6. Complete automated test suite (144+ tests)
xvfb-run -a cargo test --all
```

All 6 commands **must exit with code 0** with zero warnings and zero failed tests.

---

## 2. Invariant Checklist for Reviewers

When inspecting changes, ensure the following core architectural invariants are strictly preserved:

1. **Platform Seam Separation (`ARCH-01`):**
   - Pure Linux builds must never import Win32 APIs or WMI headers.
   - Pure Windows builds must never attempt reading `/sys/` or `/proc/`, and must never link `zbus`.
   - Shared business logic resides exclusively in `src/core/` and uses `HardwareProvider` trait abstraction.
2. **Honest Sensor Telemetry:**
   - Missing or offline sensors (e.g. absent dGPU, missing battery, single fan) must return `None` or `SensorStatus::Unavailable`.
   - Never fabricate `0.0%` or dummy RPM numbers when hardware is missing.
3. **Restricted Privilege Boundary:**
   - The GUI executable (`zenomen.exe`) must run as standard user (`asInvoker` manifest).
   - All elevated hardware writes on Windows pass through `\\.\pipe\ZenOmen.Hardware.v1` with DACL checks and session ID validation.
4. **Non-Destructive Shutdown Guarantee:**
   - In both Linux (`zenomen-hw.service`) and Windows (`ZenOmenHardwareService`), daemon teardown must reset fan curves to `Auto`.
5. **Memory and Resource Budget:**
   - GUI process peak RSS must remain <= 40MB; idle CPU <= 0.1%.
   - Dev and test cargo profiles must maintain `debug = 0`, `incremental = false`.

---

## 3. GitHub CI/CD Workflows

Verify that both CI and release matrix workflows reflect dual-OS testing:
- `.github/workflows/ci.yml`: Runs on `ubuntu-latest` and `windows-latest` across all targets.
- `.github/workflows/release.yml`: Builds release packages:
  - Linux: Debian `.deb` (`cargo-deb`) and standalone AppImage (`appimagetool`).
  - Windows: WiX v4/v5 MSI installer (`target/wix/ZenOmen-1.0.0-x64.msi`) and `.zip` archive.
  - Consolidates SHA256 checksums into `SHA256SUMS.txt`.
