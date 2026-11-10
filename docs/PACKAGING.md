# ZenOmen Packaging & Distribution Guide

ZenOmen is distributed for Linux as a native Debian package (`.deb`) and a standalone portable bundle (`.AppImage`). Packaging is configured via [Cargo Packager](https://github.com/crabnebula-dev/cargo-packager) declared directly in `Cargo.toml`.

---

## 1. Package Architecture & Delivered Files

ZenOmen follows a two-process architecture: an unprivileged desktop UI (`zenomen`) and a privileged system D-Bus hardware daemon (`zenomen-hw`).

Both packaging formats deliver or manage the following core system paths:

| File | Target System Path | Purpose |
|---|---|---|
| Desktop GUI binary | `/usr/bin/zenomen` | Unprivileged Slint desktop UI |
| Hardware daemon binary | `/usr/bin/zenomen-hw` | Privileged system D-Bus daemon |
| Desktop entry | `/usr/share/applications/zenomen.desktop` | Application launcher and menu entry |
| systemd unit | `/usr/lib/systemd/system/zenomen-hw.service` | Automatic background daemon lifecycle |
| D-Bus configuration | `/usr/share/dbus-1/system.d/com.zenomen.Hardware1.conf` | System bus name ownership (`com.zenomen.Hardware1`) |
| polkit policy | `/usr/share/polkit-1/actions/com.zenomen.hardware.policy` | Privilege escalation rules (`com.zenomen.hardware.control`) |
| Application Icons | `/usr/share/icons/hicolor/*/apps/zenomen.png` | Scalable and multi-resolution application icons |

---

## 2. Debian Package (`.deb`)

### Building Locally

```bash
cargo install cargo-packager --locked
cargo packager --release --formats deb
```

The output package will be placed in `dist/zenomen_<version>_amd64.deb`.

### Verifying Contents

Inspect package structure:

```bash
dpkg -c dist/*.deb
```

Verify that all binaries, desktop entry, systemd unit, and D-Bus/polkit policies land in the expected `/usr` hierarchy.

### Installation & Verification

On Debian/Ubuntu systems:

```bash
sudo apt update
sudo apt install ./dist/zenomen_*.deb
```

Verify service status:

```bash
systemctl daemon-reload
systemctl is-active zenomen-hw
```

The daemon will be active and running under systemd. The ZenOmen application can then be launched from the application menu or via command line `zenomen`.

---

## 3. Standalone AppImage

### Building Locally

```bash
cargo packager --release --formats appimage
```

The output bundle is generated in `dist/ZenOmen_<version>_amd64.AppImage`.

### Running Standalone

The AppImage bundles all required user libraries (including Slint, femtovg, and Fontconfig dependencies) and runs without installing to the system:

```bash
chmod +x dist/ZenOmen_*.AppImage
./dist/ZenOmen_*.AppImage
```

### System Integration via AppImage

Because an AppImage cannot write to `/etc` or `/usr/share` at launch, ZenOmen includes an embedded root-only integration command:

```bash
# Install systemd unit, polkit policy, D-Bus conf, and desktop entry
sudo ./ZenOmen_*.AppImage --install-system-integration

# Uninstall all system integration files
sudo ./ZenOmen_*.AppImage --uninstall-system-integration
```

When run as non-root, this command immediately refuses with an explicit error and writes nothing.

---

## 4. Automated Tagged Release Workflow

Releases are fully automated via GitHub Actions (`.github/workflows/release.yml`), superseding the legacy Python `how-to-release.txt` PyInstaller process.

### Creating a Release

1. Update the version in `Cargo.toml`.
2. Commit changes and push to `main`:
   ```bash
   git add Cargo.toml Cargo.lock
   git commit -m "chore(release): bump version to v1.0.0"
   git push origin main
   ```
3. Tag the release with a `v` prefix:
   ```bash
   git tag v1.0.0
   git push origin v1.0.0
   ```

### Quality Gates & Verification

The release workflow enforces strict quality gates in order before packaging:
1. `cargo fmt --all -- --check`
2. `cargo clippy --all-targets --all-features -- -D warnings`
3. `xvfb-run --auto-servernum cargo test --all`
4. `cargo audit` (blocking on security advisories)
5. `cargo packager --release --formats deb,appimage`
6. `sha256sum *.deb *.AppImage > SHA256SUMS`

### Verifying Downloaded Assets

Users can verify cryptographic integrity of downloaded artifacts using the published checksums:

```bash
sha256sum -c SHA256SUMS
```
