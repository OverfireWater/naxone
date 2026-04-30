use async_trait::async_trait;

use crate::domain::service::{ServiceInstance, ServiceStatus};
use crate::error::Result;

/// Abstraction over OS-level process lifecycle management
#[async_trait]
pub trait ProcessManager: Send + Sync {
    /// Start a service process, returns the OS PID
    async fn start(&self, instance: &ServiceInstance) -> Result<u32>;

    /// Stop a running service
    async fn stop(&self, instance: &ServiceInstance) -> Result<()>;

    /// Restart a service (stop + start)
    async fn restart(&self, instance: &ServiceInstance) -> Result<u32>;

    /// Check current status of a service
    async fn status(&self, instance: &ServiceInstance) -> Result<ServiceStatus>;

    /// Send a reload signal (e.g., nginx -s reload)
    async fn reload(&self, instance: &ServiceInstance) -> Result<()>;
}
