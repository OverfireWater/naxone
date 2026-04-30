// Linux process manager - placeholder for Phase 5
use async_trait::async_trait;

use naxone_core::domain::service::{ServiceInstance, ServiceStatus};
use naxone_core::error::{Result, NaxOneError};
use naxone_core::ports::process::ProcessManager;

pub struct LinuxProcessManager;

#[async_trait]
impl ProcessManager for LinuxProcessManager {
    async fn start(&self, _instance: &ServiceInstance) -> Result<u32> {
        Err(NaxOneError::Process("Linux support not yet implemented".into()))
    }

    async fn stop(&self, _instance: &ServiceInstance) -> Result<()> {
        Err(NaxOneError::Process("Linux support not yet implemented".into()))
    }

    async fn restart(&self, _instance: &ServiceInstance) -> Result<u32> {
        Err(NaxOneError::Process("Linux support not yet implemented".into()))
    }

    async fn status(&self, _instance: &ServiceInstance) -> Result<ServiceStatus> {
        Ok(ServiceStatus::Stopped)
    }

    async fn reload(&self, _instance: &ServiceInstance) -> Result<()> {
        Err(NaxOneError::Process("Linux support not yet implemented".into()))
    }
}
