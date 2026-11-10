//! System diagnostics inspection screen model for GPUI-CE.

use crate::core::diagnostics::DiagnosticsReport;
use crate::core::state::AppState;
use crate::hardware::registry::SensorRegistry;

#[derive(Debug, Clone, PartialEq)]
pub struct DiagnosticsScreen {
    pub service_name: String,
    pub service_status: String,
    pub authorization_status: String,
    pub detected_sensors: Vec<String>,
    pub export_status: Option<String>,
}

impl DiagnosticsScreen {
    pub fn from_state(state: &AppState, registry: Option<&SensorRegistry>) -> Self {
        let mut detected_sensors = Vec::new();
        if let Some(reg) = registry {
            if let Some(cpu) = &reg.cpu_temp {
                detected_sensors.push(format!("CPU: {} [{}]", cpu.driver, cpu.label));
            }
            for fan in &reg.fan_tachometers {
                detected_sensors.push(format!("Fan {}: [{}]", fan.index, fan.label));
            }
        }

        Self {
            service_name: "Hardware Service".to_string(),
            service_status: state.service_status.description().to_string(),
            authorization_status: state.polkit_status.clone(),
            detected_sensors,
            export_status: None,
        }
    }

    pub fn generate_report(
        &self,
        state: &AppState,
        registry: Option<&SensorRegistry>,
    ) -> DiagnosticsReport {
        crate::core::collect_diagnostics(state, registry)
    }
}
