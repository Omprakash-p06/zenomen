# Cross-Platform IPC Security & Threat Model

- **Status:** Ratified (Wave 6)
- **Target Subsystems:**
  - Linux: `com.zenomen.Hardware1` D-Bus service, `systemd` service unit, Polkit policies
  - Windows: `\\.\pipe\ZenOmen.Hardware.v1` Named Pipe, Security Descriptors, DACL, Session Authorization

---

## 1. Architectural Security Philosophy

ZenOmen is structured around a strict separation of privilege:
1. **Unprivileged Client (GUI):** Runs within the interactive user session with standard desktop user permissions (`asInvoker` on Windows; standard UID > 1000 on Linux). It possesses no direct hardware write access, no raw `/dev` handle access, and no elevated WMI execution privileges.
2. **Elevated Hardware Daemon (`zenomen-hw`):** Runs as a persistent, system-managed service (`systemd` root service on Linux; `NT AUTHORITY\LocalSystem` Windows Service on Windows).
3. **Restricted Domain-Specific IPC:** All mutation requests cross an authenticated, authorized IPC channel that passes strictly typed, bounded commands. Neither arbitrary shell commands, raw sysfs file paths, nor arbitrary WMI payloads can be requested by the client.

```text
+-------------------------------------------------------------+
|                 Interactive GUI (zenomen)                   |
|                   Unprivileged Process                      |
+-------------------------------------------------------------+
                         |
      Linux: D-Bus IPC   |   Windows: Local Named Pipe
      (zbus / Polkit)    |   (\\.\pipe\ZenOmen.Hardware.v1)
                         v
+-------------------------------------------------------------+
|           Hardware Control Service (zenomen-hw)             |
|              (Linux: root / Windows: LocalSystem)           |
|                                                             |
|   +-----------------------------------------------------+   |
|   |         Authorization & Bounds Validation Engine    |   |
|   +-----------------------------------------------------+   |
|         |                                           |       |
|         v (Allowed Whitelist Only)                  v       |
|   sysfs / driver controls                     HP WMI BIOS   |
+-------------------------------------------------------------+
```

---

## 2. Linux IPC Security & Hardening

### 2.1 D-Bus Interface Topology
- **Bus Type:** D-Bus System Bus
- **Well-Known Name:** `com.zenomen.Hardware1`
- **Object Path:** `/com/zenomen/Hardware1`
- **Interface:** `com.zenomen.Hardware1`
- **Activation:** Managed by systemd through D-Bus systemd activation (`/usr/share/dbus-1/system-services/com.zenomen.Hardware1.service` with `SystemdService=zenomen-hw.service`).

### 2.2 Systemd Sandbox Hardening (`zenomen-hw.service`)
The daemon process is locked down using modern systemd security directives:
- `ProtectSystem=strict`: Mounts the entire OS filesystem as read-only.
- `ProtectHome=true`: Completely isolates `/home`, `/root`, and `/run/user`.
- `PrivateTmp=true`: Isolated `/tmp` and `/var/tmp` namespaces.
- `ProtectKernelTunables=true`: Disables modification of `/proc/sys`, `sysfs` tunables outside explicitly whitelisted paths.
- `ReadWritePaths=/sys/devices/platform/hp-wmi /sys/class/power_supply /var/log/zenomen`: Strictly bounds write access to HP platform and power sysfs nodes and application state logging.
- `CapabilityBoundingSet=CAP_SYS_ADMIN`: Drops all unnecessary Linux capabilities.
- `NoNewPrivileges=true`: Prevents privilege escalation via setuid binaries.

### 2.3 Polkit Authorization
Mutating actions (`SetFanMode`, `SetPowerPreset`, `SetRgbColor`) are checked via Polkit action `com.zenomen.hardware.control`. Active local console sessions (`allow_active=yes`) are authorized for interactive users without requiring root password elevation.

---

## 3. Windows IPC Security & Hardening

### 3.1 Named Pipe Endpoint
- **Pipe Name:** `\\.\pipe\ZenOmen.Hardware.v1`
- **Mode:** Byte stream duplex, non-blocking asynchronous I/O (`PIPE_ACCESS_DUPLEX | FILE_FLAG_FIRST_PIPE_INSTANCE | FILE_FLAG_OVERLAPPED`).
- **Buffer Size:** 4096 bytes input / output.

### 3.2 Security Descriptor & DACL
To prevent unprivileged processes, network callers, or unauthorized local processes from tampering with the service, an explicit Discretionary Access Control List (DACL) is assigned upon pipe creation (`src/service/windows/security.rs`):
```text
D:(A;;GA;;;SY)(A;;GA;;;BA)(A;;GRGW;;;IU)
```
- `GA;;;SY`: Generic All (Full Control) granted to `NT AUTHORITY\LocalSystem`.
- `GA;;;BA`: Generic All (Full Control) granted to `Builtin\Administrators`.
- `GRGW;;;IU`: Generic Read + Generic Write granted to `NT AUTHORITY\Interactive Users`.
- **Default Deny:** Network callers (`ANONYMOUS LOGON`, `NETWORK SERVICE`) and remote RPC connections are denied access by default.

### 3.3 Active Console Session Verification
Because multiple users may have sessions on a Windows machine (e.g. Remote Desktop / Fast User Switching), the hardware service verifies caller session identity before processing hardware mutations:
1. When a client connects, the service queries `WTSGetActiveConsoleSessionId()`.
2. The service calls `GetNamedPipeClientSessionId()` or impersonates the client (`ImpersonateNamedPipeClient`) to inspect the caller's session token.
3. If the client session ID does not match the active console session ID (`WTSGetActiveConsoleSessionId()`), the mutation is rejected with `AccessDenied`. This guarantees that background or disconnected remote sessions cannot manipulate fans or power presets.

### 3.4 Request Authorization & Whitelisting
The service parses incoming JSON or binary commands into strictly typed structs (`src/service/windows/protocol.rs`).
- `command`: Checked against valid enum (`SetFanMode`, `SetPowerPreset`, `SetRgbProfile`, `QueryStatus`).
- `payload`: Range-checked and sanitized (e.g. fan mode must be `0` or `1`; RGB color values must be `0..=255`; power preset must match a recognized enum variant).
- Arbitrary byte sequences, raw file paths, and unvalidated WMI method calls are impossible by design (`src/service/windows/authorization.rs`).

---

## 4. Threat Matrix & Mitigations

| Threat | Linux Mitigation | Windows Mitigation |
|:---|:---|:---|
| **Malicious GUI Process** | D-Bus interface only exposes bounded methods; Polkit validates session; systemd restricts sysfs writes. | DACL requires interactive session; pipe protocol enforces strongly typed schema; session ID validation. |
| **Arbitrary Hardware Write / Brick Risk** | Whitelisted sysfs paths; verified HP board profiles; atomic write-checks. | Direct WMI invocation restricted to LocalSystem; payload allowlist verification against board profile. |
| **Denial of Service (Pipe Exhaustion)** | D-Bus system bus manages connection limits and message queues. | Single-instance master pipe with `FILE_FLAG_FIRST_PIPE_INSTANCE`; per-connection timeouts (5000ms). |
| **Session Hijacking / Remote User Manipulation** | Polkit checks `allow_active` console session state. | `WTSGetActiveConsoleSessionId` verification rejects non-active or remote desktop callers. |
| **Privilege Escalation** | `zenomen-hw` runs with `NoNewPrivileges=true` and dropped capabilities. | Named pipe client impersonation only verifies caller token; never executes user-controlled binaries. |
| **Unclean Shutdown / Runaway Hardware State** | Systemd unit cleanup hooks; fan curve restored to `Auto` on daemon exit. | Service `SERVICE_CONTROL_SHUTDOWN` hook restores firmware fan mode to `Auto` before exit. |
