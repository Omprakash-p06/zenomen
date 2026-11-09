# [PATCH] platform/x86: hp-wmi: fix platform profile values for Victus 15-fb0xxx (8A3D)

**Source:** <https://lists.openwall.net/linux-kernel/2026/10/24/810>
**Fetched:** 2026-11-09
**Status:** posted to `platform-driver-x86` / LKML — **not merged** in the vendored
`hp-wmi-upstream.c` snapshot (which still gives `8A3D` → `victus_s_board_params`).

```
Message-ID: <20261024202658.13149-1-nonameblank007@gmail.com>
Date: Sun, 25 Oct 2026 01:56:58 +0530
From: Sourav Nayak <nonameblank007@...il.com>
To: ilpo.jarvinen@...ux.intel.com
Cc: platform-driver-x86@...r.kernel.org,
    linux-kernel@...r.kernel.org,
    Sourav Nayak <nonameblank007@...il.com>
Subject: [PATCH] platform/x86: hp-wmi: fix platform profile values for Victus 15-fb0xxx (8A3D)

Currently Victus 15-fb0xxx (8A3D) takes victus_s_thermal_params,
which causes it to register as incorrect EC layout while cold boot.
- On other hand windows Omen Gaming Hub registers EC offset as 0x95,
- which is set in driver as HP_OMEN_EC_THERMAL_PROFILE_OFFSET.
- So, address it by adding victus_s_ec_thermal_params.
- Also perfcontrol modes as performance (0x1), default (0x0), quiet
(0x3) registers. So, add support for quiet mode.
Also, hp_wmi_fan_speed_max_reset() caused fans to freeze during loads at
0RPM, introduce a 'if' statement to resolve it by calling hp_wmi_fan_speed_max_set(0),
and passing platform active again to reactive curves. Test data:
auto/max:
hp-isa-0000
Adapter: ISA adapter
fan1: 5400 RPM
fan2: 5200 RPM
mannual[255]:
hp-isa-0000
Adapter: ISA adapter
fan1: 5800 RPM
fan2: 5800 RPM
Tested-by: Sourav Nayak <nonameblank007@...il.com>
Signed-off-by: Sourav Nayak <nonameblank007@...il.com>
---
 drivers/platform/x86/hp/hp-wmi.c | 30 ++++++++++++++++++++++++++++--
 1 file changed, 28 insertions(+), 2 deletions(-)

diff --git a/drivers/platform/x86/hp/hp-wmi.c b/drivers/platform/x86/hp/hp-wmi.c
index 615b4cf6fc45..c35bbe1427e3 100644
--- a/drivers/platform/x86/hp/hp-wmi.c
+++ b/drivers/platform/x86/hp/hp-wmi.c
@@ -121,6 +121,13 @@ static const struct thermal_profile_params victus_s_thermal_params = {
 	.ec_tp_offset = HP_EC_OFFSET_UNKNOWN,
 };
+static const struct thermal_profile_params victus_s_ec_thermal_params = {
+	.performance	= HP_VICTUS_S_THERMAL_PROFILE_PERFORMANCE,
+	.balanced	= HP_VICTUS_S_THERMAL_PROFILE_DEFAULT,
+	.low_power	= HP_VICTUS_S_THERMAL_PROFILE_DEFAULT,
+	.ec_tp_offset	= HP_OMEN_EC_THERMAL_PROFILE_OFFSET,
+};
+
 static const struct thermal_profile_params omen_v1_thermal_params = {
 	.performance	= HP_OMEN_V1_THERMAL_PROFILE_PERFORMANCE,
 	.balanced	= HP_OMEN_V1_THERMAL_PROFILE_DEFAULT,
@@ -164,6 +171,11 @@ static const struct hp_wmi_board_params victus_s_board_params = {
 	.fan_profile		= &victus_s_fan_profile_params,
 };
+static const struct hp_wmi_board_params victus_s_ec_board_params = {
+	.thermal_profile	= &victus_s_ec_thermal_params,
+	.fan_profile		= &victus_s_fan_profile_params,
+};
+
 static const struct hp_wmi_board_params omen_v1_board_params = {
 	.thermal_profile	= &omen_v1_thermal_params,
 	.fan_profile		= &victus_s_fan_profile_params,
@@ -245,7 +257,7 @@ static const struct dmi_system_id hp_wmi_feature_boards[] __initconst = {
 	},
 	{
 		.matches = { DMI_MATCH(DMI_BOARD_NAME, "8A3D") },
-		.driver_data = (void *)&victus_s_board_params,
+		.driver_data = (void *)&victus_s_ec_board_params,
 	},
 	{
 		.matches = { DMI_MATCH(DMI_BOARD_NAME, "8A44") },
@@ -2121,10 +2133,14 @@ static int platform_profile_victus_s_get_ec(enum platform_profile_option *profil
 		 * but logically it should have tp == 0x30 || tp == 0x31, as
 		 * corrected by the Omen Gaming Hub on windows. Hence accept both
 		 * of these values.
+		 * Since boards like 8A3D write 0x3 to EC offset 0x95 to
+		 * engage quiet mode. Map this to PLATFORM_PROFILE_LOW_POWER.
 		 */
 		if (tp == victus_s_thermal_params.performance ||
 		    tp == omen_v1_thermal_params.performance) {
 			*profile = PLATFORM_PROFILE_PERFORMANCE;
+		} else if (tp == HP_VICTUS_THERMAL_PROFILE_QUIET) {
+			*profile = PLATFORM_PROFILE_LOW_POWER;
 		} else if (tp == victus_s_thermal_params.balanced ||
 			   tp == omen_v1_thermal_params.balanced) {
 			/*
@@ -2657,7 +2673,17 @@ static int hp_wmi_apply_fan_settings(struct hp_wmi_hwmon_priv *priv)
 			secs_to_jiffies(KEEP_ALIVE_DELAY_SECS));
 		return 0;
 	case PWM_MODE_AUTO:
-		if (hp_wmi_fan_control_supported()) {
+		/*
+		 * Victus models like 8A3D, calling hp_wmi_fan_speed_max_reset()
+		 * freezes fans at 0 RPM under load. Instead, disable max fans and re-apply
+		 * the active platform profile, so the EC firmware thermal curves regain control.
+		 */
+		if (hp_wmi_thermal_profile() == &victus_s_ec_thermal_params) {
+			ret = hp_wmi_fan_speed_max_set(0);
+			if (ret < 0)
+				return ret;
+			ret = platform_profile_victus_s_set_ec(active_platform_profile);
+		} else if (hp_wmi_fan_control_supported()) {
 			ret = hp_wmi_get_fan_count_userdefine_trigger();
 			if (ret < 0)
 				return ret;
base-commit: 5b0764a53235d3c77e1ed674e836e1b3fe1a4b1f
--
2.55.0
```

## What this establishes

1. `8A3D`'s EC thermal-profile register is **`0x95`** (`HP_OMEN_EC_THERMAL_PROFILE_OFFSET`),
   not `0x59`.
2. `8A3D` profile values: balanced `0x00`, performance `0x01`, quiet `0x03`.
3. `hp_wmi_fan_speed_max_reset()` (`0x27 = 0` + `0x2E {0,0}`) is the operation that freezes
   `8A3D` fans at 0 RPM under load.
4. The replacement is `0x27 = 0` followed by `platform_profile_victus_s_set_ec(active)`,
   which reads the profile from EC `0x95` but **writes** it with WMI `0x1A`
   (`{0xFF, tp}`) and then `0x22` for GPU CTGP/PPAB. It never sends `0x2E`.
5. This is a *posted* patch; it is not in the vendored master (see §5 of `REFERENCES.md`).
