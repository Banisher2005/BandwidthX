use bandwidthx_common::ServiceStatus;
use crate::service::wfp_engine::WfpEngine;

pub struct ServiceClient {
    pub local_engine: WfpEngine,
}

impl ServiceClient {
    pub fn new() -> Self {
        Self {
            local_engine: WfpEngine::new(),
        }
    }

    pub fn get_service_status(&self) -> ServiceStatus {
        let is_elevated = WfpEngine::is_admin();
        let exclusive_active = self.local_engine.get_exclusive_state().is_active;
        let count = self.local_engine.get_active_filter_count();

        ServiceStatus {
            is_running: true,
            is_elevated,
            is_service_connected: true,
            exclusive_mode_active: exclusive_active,
            active_filter_count: count,
            version: "1.0.0".to_string(),
        }
    }
}
