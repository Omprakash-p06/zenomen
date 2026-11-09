# HP OMEN BIOS communication notes

[Русская версия](hp-bios-protocol.ru.md)

> [!CAUTION]
> This is an independent reverse-engineering note, not an HP specification. The
> interface is undocumented, privileged, and model-dependent. A command that is
> valid on one system board can be rejected or behave differently on another.
> Never probe unknown write commands on hardware you cannot afford to lose.

Research snapshot: **2026-09-08**. The only hardware on which this project has
performed BIOS writes is an HP Victus 15-fa0xxx with system board `8A4F` and HP
Thermal Policy `V0`.

## Evidence labels

| Label | Meaning |
|---|---|
| **8A4F verified** | Read from the test laptop or exercised by Victus Mode Switch with a firmware success code. |
| **OGH observed** | Reconstructed from the locally installed OMEN Gaming Hub package and HSA components listed below. |
| **Upstream** | Independently implemented by the Linux `hp-wmi` driver or an open-source OMEN project. |
| **Candidate** | Plausible and useful, but deliberately not invoked on the test laptop. |

No HP binary, decompiled source, keys, or package content is committed to this
repository. This document records interface shapes and behavioral observations.

## Communication path

The current OMEN stack has several layers:

```text
OMEN Gaming Hub MSIX
  -> WMISDK client
  -> local HP RPC endpoint HPSysInfoRpcEndpoint / service WMIService
  -> HP Omen HSA Service (HPOmenCap)
  -> HP WMI/ACPI provider
  -> embedded controller and firmware
```

Observed installed components:

| Component | Version on the test laptop |
|---|---|
| System BIOS | `F.29` |
| OMEN Gaming Hub package | `1101.2605.2.0` |
| HP Omen HSA Service (`HPOmenCap`) | `OmenCap.exe` `1.0.838.0` |
| HP WMISDK managed client | `1.3.10` |
| HP device capability component | `3.0.1.4580` |

OMEN Gaming Hub does not need its full UI process to remain open for every
firmware operation. Its managed client serializes a request, sends local RPC
command `2` to endpoint `HPSysInfoRpcEndpoint` with service name `WMIService`,
and consumes `returnCode` plus `returnData` from the reply.

The RPC request body has this shape:

```json
{
  "command": 131080,
  "commandType": 40,
  "inputDataSize": 4,
  "inputData": [0, 0, 0, 0],
  "returnDataSize": 128
}
```

This example is `command=0x20008`, `commandType=0x28`, the System Design Data
query. The RPC endpoint is a private local HP capability interface, not a public
network protocol. Current HSA binaries also enforce caller/package trust. Victus
Mode Switch does not try to impersonate Gaming Hub or bypass those checks.

Instead, Victus Mode Switch uses the narrower WMI path directly and exposes only
allowlisted operations through its own short-lived elevated broker:

```text
normal per-user tray process
  -> current-user-only named pipe
  -> scheduled elevated BIOS broker
  -> root\wmi / hpqBIntM
  -> firmware
```

## HP WMI transport

The interface discovered on the test laptop is:

| Item | Value |
|---|---|
| Namespace | `root\wmi` |
| BIOS WMI GUID | `5FB7F034-2C63-45E9-BE91-3D44E2C707E4` |
| Event WMI GUID | `95F24279-4D7B-4334-9387-ACCDC67EF61C` |
| Method instance class | `hpqBIntM` |
| Input class | `hpqBDataIn` |
| Output classes | `hpqBDataOut0`, `hpqBDataOut4`, `hpqBDataOut128`, `hpqBDataOut1024`, `hpqBDataOut4096` |
| Event class | `hpqBEvnt` |
| Methods present | `hpqBIOSInt0`, `hpqBIOSInt4`, `hpqBIOSInt128`, `hpqBIOSInt1024`, `hpqBIOSInt4096` |

`hpqBDataIn` carries this logical envelope:

| Field | Type | Meaning |
|---|---|---|
| `Sign` | byte array | `53 45 43 55`, ASCII `SECU` |
| `Command` | `UInt32` | Command family, normally `0x20008` for gaming controls |
| `CommandType` | `UInt32` | Operation inside the command family |
| `Size` | `UInt32` | Number of meaningful payload bytes |
| `hpqBData` | byte array | Input payload |

The selected method name determines the output bucket. `OutData` contains
`rwReturnCode` and, except for the zero-length method, `Data`.

| Requested output | Method |
|---:|---|
| 0 bytes | `hpqBIOSInt0` |
| 1 to 4 bytes | `hpqBIOSInt4` |
| 5 to 128 bytes | `hpqBIOSInt128` |
| 129 to 1024 bytes | `hpqBIOSInt1024` |
| More than 1024 bytes, up to 4096 | `hpqBIOSInt4096` |

Known firmware return codes, corroborated by the upstream Linux driver:

| Code | Meaning |
|---:|---|
| `0` | Success |
| `2` | Wrong signature |
| `3` | Unknown command |
| `4` | Unknown command type |
| `5` | Invalid parameters |

Some HP callers send a four-byte zero buffer for reads while other callers use
`Size=0`. Both forms are seen in working implementations. This is one example of
why payload length must be treated as part of each board-specific contract.

### Alternate HP driver frame

Current Gaming Hub code also contains an alternate driver path. It creates a
4,116-byte in/out buffer:

| Offset | Length | Value |
|---:|---:|---|
| `0x0000` | 4 | `SECU` |
| `0x0004` | 4 | `Command`, little-endian |
| `0x0008` | 4 | `CommandType`, little-endian |
| `0x000C` | 4 | Input bucket size: 0, 4, 128, 1024, or 4096 |
| `0x0010` | 4096 | Payload area |
| `0x1010` | 1 | Output selector: 1=0, 2=4, 3=128, 4=1024, 5=4096 |

On return, the observed client reads a status signature from bytes `0..3`, the
firmware return code from byte `4`, and output data starting at byte `8`. This
path is documented for comparison only and is not used by Victus Mode Switch.

## OMEN button event

The diamond key arrives through `hpqBEvnt`:

| Field | Observed value |
|---|---:|
| `EventID` | `29` (`0x1D`, `HPWMI_OMEN_KEY`) |
| `EventData` | `8613` on the test laptop |

Firmware emits one pulse per press on board `8A4F`; it does not emit a release
event or a repeat stream that would make a reliable hold gesture possible.

## System Design Data on 8A4F

Gaming Hub reads `0x20008 / 0x28` into a 128-byte block and caches it at:

```text
HKCU\Software\HP\OMEN Ally\Settings\SystemDesignData
```

The current cache on the test laptop begins with `C8 00 00 00 00 00 00 00 00`;
all remaining bytes are zero. Gaming Hub `1101.2605.2.0` interprets the relevant
fields as follows:

| Offset | Interpretation | 8A4F value | Consequence |
|---:|---|---:|---|
| `0..1` | Shipping adapter rating, little-endian watts | `200` | Meets Gaming Hub's threshold for BIOS Performance mode. |
| `3` | Thermal Policy version | `0` | Use V0 profile values. |
| `4`, bit 0 | Software/manual fan control | `0` | Do not expose manual fan curves. |
| `4`, bit 1 | Extreme mode supported | `0` | Do not expose Extreme mode. |
| `4`, bit 2 | Extreme mode unlocked | `0` | No Extreme unlock path. |
| `6` | BIOS overclocking capability flag | `0` | Do not expose BIOS overclocking. |
| `7` | graphics switching capability flag | `0` | Do not expose a GPU MUX switch. |
| `8` | default concurrent TDP field | `0` | No usable platform value was advertised. |

The installed device catalog calls board `8A4F` platform `Roku`, cycle `22C1`,
GPU SKU `N20P`, and explicitly allows Performance Control. It lists `FourZone`
at the wider Roku family level, but runtime capability checks still decide
whether a particular keyboard supports it.

For Thermal Policy V0 the known firmware profiles are:

| Profile | Value |
|---|---:|
| Default | `0x00` |
| Performance | `0x01` |
| Cool | `0x02` |

For V1 systems the observed values are Default `0x30`, Performance `0x31`, and
Cool `0x50`. Victus Mode Switch blocks V1 rather than assuming compatibility.

There is no distinct BIOS Eco profile advertised for this V0 Victus. Gaming Hub
maps Eco to Default, and Victus Mode Switch does the same. The app's additional
Eco behavior comes from optional Windows power settings and disabling Max Fan.

## Gaming command map

Unless stated otherwise, the outer command is `0x20008` (`HPWMI_GM`). A `get`
name means the operation is nominally read-only, not that it is guaranteed to be
free of firmware side effects.

| Type | Direction | Known shape | Evidence and project policy |
|---:|---|---|---|
| `0x10` | Get fan count | Four-byte input; count in output byte 0 | OGH observed, upstream. Linux notes that this query can arm/refresh a custom-fan state with a 120-second fallback. Candidate only. |
| `0x11` | Get fan RPM | Input byte 0 is fan index; RPM is output bytes 2..3, big-endian | **8A4F verified 2026-09-26.** Used for read-only tray telemetry, indexes 0 and 1. |
| `0x1A` | Set thermal profile | OGH-style payload `[FF, profile, fanControlByBios, 00]` | **8A4F verified.** Only V0 values 0 and 1 are allowlisted. |
| `0x21` | Get GPU thermal/power state | Four output bytes: cTGP, PPAB, D-state, slowdown temperature | OGH observed, upstream. Candidate telemetry. |
| `0x22` | Set GPU thermal/power state | Same four logical fields | OGH observed, upstream. Disabled: values are GPU- and BIOS-specific. |
| `0x23` | Get platform sensor temperature | Input byte 0 selects a sensor; output byte 0 is temperature | OGH observed. IR, ambient, PCH, and VR indices appear in current code, but the mapping is provisional. |
| `0x26` | Get Max Fan | State in output byte 0 | **8A4F verified.** |
| `0x27` | Set Max Fan | First payload byte is 0 or 1 | **8A4F verified.** |
| `0x28` | Get System Design Data | Four zero bytes in OGH; 128-byte output | **8A4F verified.** Used as a write guard. |
| `0x29` | Set power limits | Four-byte platform structure | OGH observed, upstream. Disabled: field ordering and valid ranges vary across generations. |
| `0x2A` | Get power data | 128-byte output; current OGH reads PL4 at byte 6 and concurrent TDP at byte 7 | OGH observed. Candidate telemetry, layout not verified on 8A4F. |
| `0x2B` | Get keyboard type | Four-byte output | OGH observed. Candidate capability query. |
| `0x2C` | Get fan types/capabilities | 128-byte output; fan-type nibbles in bytes 0..3 and flags in bytes 8..9 | OGH observed. Candidate capability query. |
| `0x2D` | Get Victus fan levels | 128-byte output; upstream Victus-S path treats each fan byte as 100 RPM units | OGH observed, upstream. Candidate telemetry. |
| `0x2E` | Set Victus fan levels | Two or more fan-level bytes | OGH observed, upstream. Disabled because custom fan state requires model-specific limits and keepalive behavior. |
| `0x2F` | Get fan table | 128-byte output | OGH observed, upstream. Candidate research query. |
| `0x31` | Set idle behavior | First byte used as a boolean in current OGH | OGH observed. Semantics are incomplete; disabled. |
| `0x35` | Diagnostics/support subcommands | Subcommand-dependent four-byte input and output | OGH observed. Provisional; disabled until each subcommand is understood. |
| `0x36` / `0x37` | Get/set extended power configuration | Newer two-byte power and load-line fields | OGH observed. Not advertised by this system; writes disabled. |

Other command families observed in current Gaming Hub include `0x20009` for
keyboard lighting and `0x2000B` for a USB-C scenario query. A generic HP read
with `Command=1`, `CommandType=0x52` queries graphics-switcher state. None is
currently exposed because the exact capability and payload contract has not been
verified on this laptop.

## Operations enabled by this project

The elevated broker accepts no arbitrary command IDs or byte arrays. Before each
write it verifies board `8A4F`, reads System Design Data, and requires Thermal
Policy V0. Its complete firmware surface is:

| User action | BIOS sequence |
|---|---|
| Eco | `0x1A [FF 00 00 00]`, then `0x27 [00 00 00 00]`, then verify with `0x26` |
| Standard | `0x1A [FF 00 00 00]`, preserve requested Max Fan state, verify with `0x26` |
| Performance | `0x1A [FF 01 00 00]`, preserve requested Max Fan state, verify with `0x26` |
| Max Fan toggle | Reapply current profile, call `0x27` with 0 or 1, verify with `0x26` |

The application creates a current-user-only pipe named
`VictusModeSwitch.Bios.<SID-with-dots>` and launches scheduled task
`Victus Mode Switch BIOS`. One newline-delimited JSON request contains `Id`,
`Mode`, `MaxFanEnabled`, and `CreatedAt`; the response contains `Id`, `Success`,
`Message`, and confirmed `MaxFanEnabled`. The broker rejects stale timestamps,
unknown modes, unsupported hardware, and mismatched response state.

## Useful next features

### Telemetry verification: 2026-09-26

On the test board, `0x11` returned `36 3C 0F A7` (4007 RPM) and
`36 3C 0D FD` (3581 RPM). A separate `0x2D` read returned `27 23`,
consistent with the coarse 3900/3500 RPM levels at a slightly later sample.
`0x23 [01 00 00 00]` returned success but all-zero data, so it is not used
as a temperature source. `0x2C` began with `01` and `0x2F` with `01 0C 18`;
these responses do not establish a usable manual fan range.

Version 2.5.0 adds a fixed `ReadTelemetry` broker operation. It checks the board
and thermal policy, reads only `0x11` for fan indexes 0/1, and reads Windows
`MSAcpi_ThermalZoneTemperature`. It never calls `0x10` or writes fan settings.
The response adds optional `Telemetry` containing `SystemTemperatureCelsius`,
`Fan1Rpm`, and `Fan2Rpm`. The tray process reads GPU temperature independently
through NVIDIA NVML. ACPI values are labelled as system thermal-zone readings,
not CPU package temperature. Invalid/unavailable readings remain null.

One sample is primed after hardware startup. Further sampling happens on hover,
at most every five seconds, and stops when the pointer moves away or after
15 seconds without another mouse event. Cached values expire after 15 seconds.
For diagnostics, `VictusModeSwitch.exe --telemetry <existing-directory>/sample.json`
writes one read-only sample using the installed broker.

Manual speed control remains disabled. An independent
[8A4F owner report](https://github.com/seakyy/OmenMon-Reborn/issues/163)
describes EC lockup with its manual 100% fan path. This does not establish a
failure of our existing `0x27` Max Fan toggle, but rules out assuming that
arbitrary 0-100% values or the Victus-S fan table work on this board.

### Remaining research

The safest useful expansion is a read-mostly diagnostics layer, in this order:

1. Decode and display board capabilities and cached System Design Data.
2. Test fan RPM reads behind an explicit board profile and conservative polling.
3. Add temperatures and current GPU power-state telemetry only after comparing
   each result with an independent sensor tool.
4. Probe keyboard-lighting support separately, without enabling writes until the
   exact keyboard type and payload are verified.

Manual fan curves, GPU power writes, PL1/PL2/PL4 writes, Extreme mode, and a GPU
MUX control should not be enabled on `8A4F` from the current evidence. The
firmware advertises none of the required capability flags, and the cost of a bad
guess is much higher than the benefit.

## Read-only inventory

Run the repository script to collect a shareable JSON snapshot:

```powershell
.\scripts\Inspect-HpBiosInterface.ps1 -OutputPath .\artifacts\hp-bios-interface.json
```

It queries Windows hardware metadata, WMI class definitions, installed OMEN/HSA
versions, and Gaming Hub's registry cache. It intentionally never invokes an
`hpqBIOSInt*` method and therefore does not read from or write to firmware.
Serial numbers and the Windows user SID are not included.

## Sources

- [Linux `hp-wmi` driver](https://github.com/torvalds/linux/blob/master/drivers/platform/x86/hp/hp-wmi.c), the primary public implementation of the HP WMI transport and several OMEN/Victus commands.
- [HP OMEN Gaming Hub feature compatibility](https://support.hp.com/us-en/document/ish_9237683-9237735-16), confirming that performance, fan, and Extreme controls are model-dependent.
- [OmenMon at `d89340e`](https://github.com/OmenMon/OmenMon/tree/d89340e6d4dc9802b609390729b0bfaec05563e2).
- [OmenHwCtl at `b3eda0b`](https://github.com/GeographicCone/OmenHwCtl/tree/b3eda0b93269fedf329b9d55409d7ff020e956ac).
- [OmenFlow at `d83cde3`](https://github.com/yunusemreyl/OmenFlow/tree/d83cde390959cde41ed3001ff774f29e642a5e22).
- [OmenCore at `477a685`](https://github.com/theantipopau/omencore/tree/477a68533ce7b575108cb399e1f59759bf70861c).
- Static interface-shape inspection of OMEN Gaming Hub `1101.2605.2.0`, WMISDK `1.3.10`, and OmenCap `1.0.838.0` installed on the test laptop.

Open-source projects are corroborating evidence, not a compatibility guarantee.
Reports from other owners were useful discovery leads, but unsupported-model
write examples were not treated as authoritative evidence.
