# Reference Corpus — HP Victus 15-fb0xxx (board `8A3D`) fan control

**Captured:** 2026-11-09
**Purpose:** Primary-source evidence used to debug ZenOmen's Windows HP BIOS WMI backend.
Every claim in `.planning/debug/victus-s-8a3d-fan-protocol.md` traces back to one of these files.

| File | Source | What it establishes |
|---|---|---|
| `hp-wmi-upstream.c` | `torvalds/linux` @ master, `drivers/platform/x86/hp/hp-wmi.c` | Board `8A3D` → `victus_s_board_params`. Command IDs, envelope, fan table layout, keepalive, return codes. |
| `VictusModeSwitch-hp-bios-protocol.md` | `Juanbod/VictusModeSwitch` docs | Independent reverse-engineering of the same WMI interface. Confirms `0x20009` is the **keyboard-lighting** family, `0x27` payload is 4 bytes, `0x11` RPM byte order. Physically verified on `8A4F` only. |
| `FanControl-HpOmenPlugin-OmenBios.cs` | `inorixu/FanControl.HpOmenPlugin` | Working `hpqBDataIn` construction: one command constant `0x20008`, `Sign = {53 45 43 55}`, `hpqBData`/`Size` set only when a payload exists, method name `hpqBIOSInt{outputSize}`. |
| `docs/architecture/HP_WMI_PROTOCOL.md` §2, §3 (in-repo) | Microsoft `IWbemServices::ExecMethod` + MS-WMI | Synchronous semantics: `lFlags = 0`, `ppOutParams` non-NULL, `ppCallResult` NULL. |
| `openwall-8a3d-thermal-patch.md` | `lists.openwall.net/linux-kernel/2026/10/24/810` (fetched 2026-11-09) | **Posted, not merged.** Routes `8A3D` to `victus_s_ec_thermal_params` (EC offset `0x95`), adds quiet `0x03`, and records that the `0x2E {0,0}` reset freezes `8A3D` fans at 0 RPM under load. |

Also consulted (not vendored):
- `https://learn.microsoft.com/en-us/windows/win32/api/wbemcli/nf-wbemcli-iwbemservices-execmethod`
- `https://learn.microsoft.com/en-us/openspecs/windows_protocols/ms-wmi/cbce28cd-fd45-4df1-9ec8-416e6bb33691`
- `https://nullcon.net/wp-content/uploads/2026/04/Reverse-Engineering-RGB-Keyboard-Backlights-With-Linux-Kernel-Drivers.pdf`
  (decompiled HP client: builds `root\wmi:hpqBDataIn`, `GetMethodParameters("hpqBIOSInt128")`,
  sets `InData`, invokes, reads `OutData.Data` + `OutData.rwReturnCode`.)

---

## 1. Board routing (upstream, authoritative)

```c
// hp-wmi.c:253
{ .matches = { DMI_MATCH(DMI_BOARD_NAME, "8A3D") },
  .driver_data = (void *)&victus_s_board_params },
```

```c
static const struct hp_wmi_fan_profile_params victus_s_fan_profile_params = {
    .get_fan_speed = hp_wmi_get_fan_speed_victus_s,   // 0x2D, not 0x11
    .fan_table     = true,                            // 0x2F supported
};
```

`victus_s_board_params` is also used by `8B2F`, `8BBE`, `8BD4`, `8BD5`, `8C99`, `8C9C`, `8DE1`.
The pasted investigation claimed a "GPU fan offset from the fan table" — **the driver has no such
step**. It maps `cpu_pwm` and `gpu_pwm` independently through `pwm_to_rpm()`; there is no GPU delta.

---

## 2. Wire envelope (upstream `hp_wmi_perform_query`)

```c
args->signature  = 0x55434553;   // 'SECU' as a LE u32 -> 53 45 43 55
args->command    = command;      // HPWMI_GM = 0x20008  (read *and* write)
args->commandtype= query;        // 0x10, 0x11, 0x26, 0x27, 0x2D, 0x2E, 0x2F ...
args->datasize   = insize;       // meaningful bytes, *not* the buffer size
```

`actual_insize = max(insize, 128)` — the data area is always at least 128 bytes in the ACPI
driver's flat buffer. The `hpqBDataIn` WMI class mirrors this envelope as named properties.

**Interface selection is by output size** (`encode_outsize_for_pvsz`):
`0→Int0`, `≤4→Int4`, `≤128→Int128`, `≤1024→Int1024`, `≤4096→Int4096`.
`hpqBIOSInt0` has **no** `Data` output property.

---

## 3. Command map (upstream + independent confirmation)

| Cmd | Operation | Input | Output | Notes |
|---:|---|---|---|---|
| `0x10` | Fan count | 1 byte (0) | 4 bytes, count = `[0]` | **Arms** the 120 s user-defined fan state |
| `0x11` | Fan RPM (generic) | 1 byte = fan index | 4 bytes, RPM = `[2]<<8 \| [3]` | Not used for `8A3D` |
| `0x1A` | Set thermal profile | 2 bytes `{0xFF, mode}` | none | Victus S: default `0x00`, performance `0x01` |
| `0x26` | Get Max Fan | none | 4 bytes, state = `[0]` | |
| `0x27` | Set Max Fan | 4-byte int `0/1` | none | |
| `0x2D` | Victus S fan speed | 1 byte (0) | 128 bytes, RPM = `[fan] * 100` | `0` = automatic |
| `0x2E` | Victus S fan levels | 2 bytes `{cpu, gpu}` | none | Values are fan-table RPM units |
| `0x2F` | Victus S fan table | 4 bytes | 128 bytes | header `{num_fans, unknown}`, then `{cpu_rpm, gpu_rpm, noise_db}×3` packed |

Firmware return codes (`enum hp_return_value`):
`2 = wrong signature`, `3 = unknown command`, `4 = unknown command type`,
`5 = invalid parameters`. **`0` is the only success.**

---

## 4. Manual-control sequence and keepalive

```c
hp_wmi_get_fan_count_userdefine_trigger();  // 0x10 — arm / refresh
hp_wmi_fan_speed_max_set(0);                // 0x27 = 0 — Max Fan must be off
hp_wmi_perform_query(0x2E, HPWMI_GM, fan_speed, 2, 0);
```

- `fan_speed[cpu]`/`[gpu]` = `pwm_to_rpm(pwm)` mapped over `0..max_rpm` read from the fan table,
  or `HP_FAN_SPEED_AUTOMATIC = 0x00`.
- Firmware falls back to its automatic curve after ~120 s: `KEEP_ALIVE_DELAY_SECS = 90`.
- Restoring Auto on Victus S needs **two** steps (`hp_wmi_fan_speed_max_reset`): `0x27 = 0`
  followed by `0x2E {0, 0}`.

---

## 5. Board `8A3D` thermal profile (patch 2026-10-24)

Source: `[PATCH] platform/x86: hp-wmi: fix platform profile values for Victus 15-fb0xxx (8A3D)`,
Sourav Nayak, Message-ID `<20261024202658.13149-1-nonameblank007@gmail.com>`, base-commit
`5b0764a53235d3c77e1ed674e836e1b3fe1a4b1f`. Vendored as `openwall-8a3d-thermal-patch.md`.

Status: **posted to the list, not merged.** The vendored master snapshot still routes `8A3D`
to `victus_s_board_params` → `victus_s_thermal_params` with `.ec_tp_offset = HP_EC_OFFSET_UNKNOWN`.

```c
+static const struct thermal_profile_params victus_s_ec_thermal_params = {
+    .performance = HP_VICTUS_S_THERMAL_PROFILE_PERFORMANCE,   // 0x01
+    .balanced    = HP_VICTUS_S_THERMAL_PROFILE_DEFAULT,       // 0x00
+    .low_power   = HP_VICTUS_S_THERMAL_PROFILE_DEFAULT,       // 0x00
+    .ec_tp_offset = HP_OMEN_EC_THERMAL_PROFILE_OFFSET,        // 0x95
+};
```

- `8A3D`'s active profile lives in **EC `0x95`**, not `0x59`. `0x59`
  (`HP_VICTUS_S_EC_THERMAL_PROFILE_OFFSET`) is the offset used by
  `omen_v1_thermal_params`.
- Readback maps `tp == 0x03` (`HP_VICTUS_THERMAL_PROFILE_QUIET`) to
  `PLATFORM_PROFILE_LOW_POWER`, so `8A3D` values are balanced `0x00`, performance `0x01`,
  quiet `0x03`.
- `PWM_MODE_AUTO` on an EC-thermal board replaces `hp_wmi_fan_speed_max_reset()` with
  `hp_wmi_fan_speed_max_set(0)` + `platform_profile_victus_s_set_ec(active_platform_profile)`.
  That helper sends `0x10`, then `0x1A {0xFF, tp}`, then `0x22` (GPU CTGP/PPAB). It never
  sends `0x2E`.
- Commit message: "hp_wmi_fan_speed_max_reset() caused fans to freeze during loads at 0RPM".

**Implication for ZenOmen:** the profile *write* is WMI `0x1A`, so it is implementable; the
profile *read* is EC-only, so a userland Windows process cannot know the active profile.
`restore_automatic_fan_control` therefore refuses `8A3D` auto restore when no profile is
supplied rather than sending the `0x2E` reset.

---

## 6. What this corpus does **not** prove for `8A3D`

- No physical write evidence exists for `8A3D` / BIOS F.22. `VictusModeSwitch` validated on `8A4F`;
  `FanControl.HpOmenPlugin` on an Omen 10.
- Fan-table units, minimum manual level and the 120 s timeout are inherited from upstream, not
  measured on this board. `fan_write_verified` must therefore stay `false`.
