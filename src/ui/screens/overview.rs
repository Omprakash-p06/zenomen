//! System overview screen model for GPUI-CE.

use crate::core::state::AppState;
use crate::ui::components::MetricCard;

#[derive(Debug, Clone, PartialEq)]
pub struct OverviewScreen {
    pub cpu_temp_card: MetricCard,
    pub cpu_util_card: MetricCard,
    pub gpu_temp_card: MetricCard,
    pub memory_card: MetricCard,
    pub fan_rpm_cards: Vec<MetricCard>,
    pub battery_card: Option<MetricCard>,
}

impl OverviewScreen {
    pub fn from_state(state: &AppState) -> Self {
        let snap = state.latest_snapshot.as_ref();

        let cpu_temp_str = snap
            .and_then(|s| s.cpu_temperature_c)
            .map(|t| format!("{t:.1}"))
            .unwrap_or_else(|| "--".to_string());
        let cpu_temp_card = MetricCard::new("CPU Temperature", cpu_temp_str, "°C");

        let cpu_util_str = snap
            .map(|s| format!("{:.1}", s.cpu_utilization))
            .unwrap_or_else(|| "--".to_string());
        let cpu_util_card = MetricCard::new("CPU Utilization", cpu_util_str, "%");

        let gpu_temp_str = snap
            .and_then(|s| s.gpu_temperature_c)
            .map(|t| format!("{t:.1}"))
            .unwrap_or_else(|| "--".to_string());
        let gpu_temp_card = MetricCard::new("GPU Temperature", gpu_temp_str, "°C");

        let memory_str = snap
            .map(|s| format!("{}", s.ram_used_mb))
            .unwrap_or_else(|| "--".to_string());
        let memory_card = MetricCard::new("Memory Used", memory_str, "MB");

        let mut fan_rpm_cards = Vec::new();
        if let Some(s) = snap {
            for (idx, rpm) in s.fan_rpms.iter().enumerate() {
                fan_rpm_cards.push(MetricCard::new(
                    format!("Fan {}", idx + 1),
                    format!("{rpm}"),
                    "RPM",
                ));
            }
        }

        let battery_card = snap.and_then(|s| s.battery.as_ref()).map(|b| {
            let status = match b.charging {
                Some(true) => "Charging",
                Some(false) => "Discharging",
                None => "Connected",
            };
            let pct = b
                .charge_percent
                .map(|p| format!("{p}"))
                .unwrap_or_else(|| "--".to_string());
            MetricCard::new("Battery", pct, "%").with_subtitle(status)
        });

        Self {
            cpu_temp_card,
            cpu_util_card,
            gpu_temp_card,
            memory_card,
            fan_rpm_cards,
            battery_card,
        }
    }
}
