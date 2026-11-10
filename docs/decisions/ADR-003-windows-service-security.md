# ADR-003: Restricting Windows Hardware Service Named Pipe Security

- **Status:** Accepted (2026-11-07)
- **Date:** 2026-11-07
- **Decision Owner:** ZenOmen Security Team
- **Related:** `docs/architecture/IPC_SECURITY.md`, `src/service/windows/security.rs`, `src/service/windows/authorization.rs`

## Context

Because mutating ACPI BIOS calls via WMI requires elevated system privileges, ZenOmen runs `zenomen-hw.exe` as a Windows Service under `NT AUTHORITY\LocalSystem`. An unprivileged desktop application (`zenomen.exe`) running under an unprivileged user session communicates with this service via local IPC.

Without explicit access controls, local named pipes in Windows can be vulnerable to:
1. Unauthorized local processes tampering with hardware power or cooling settings.
2. Network callers accessing the named pipe over SMB.
3. Multi-session confusion where a background user or remote RDP user alters hardware for the active physical console user.

## Decision

**ZenOmen configures the hardware service named pipe (`\\.\pipe\ZenOmen.Hardware.v1`) with a strict Discretionary Access Control List (DACL) and active console session validation.**

1. **Security Descriptor & DACL:**
   The pipe is created with the explicit SDDL string:
   `D:(A;;GA;;;SY)(A;;GA;;;BA)(A;;GRGW;;;IU)`
   - Only `LocalSystem` (SY) and `Builtin\Administrators` (BA) have Full Control.
   - `Interactive Users` (IU) have Generic Read/Write.
   - Network logins, anonymous users, and remote callers are denied access by default.
2. **Session Verification:**
   Before executing mutating commands, the service matches the calling pipe client's token session against `WTSGetActiveConsoleSessionId()`. If the caller is not the active physical console session, mutations are rejected.
3. **Payload Whitelisting:**
   The service only accepts strongly typed JSON/binary packets that map to strictly validated Rust enums. No raw bytes, shell invocations, or unverified WMI parameters are accepted.

## Consequences

**Positive:**
- Unprivileged users can manage cooling and performance safely without elevating UAC for the GUI application.
- Immune to network-based named pipe exploitation.
- Guarantees multi-user session isolation.

**Negative / Accepted:**
- Non-interactive batch scripts or SSH sessions cannot control hardware unless run under the active console session ID or with administrative privileges.
