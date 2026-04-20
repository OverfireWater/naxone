use async_trait::async_trait;
use std::path::Path;

use crate::domain::service::ServiceKind;
use crate::domain::service::ServiceInstance;
use crate::error::Result;

/// Discover and manage software packages
#[async_trait]
pub trait PackageStore: Send + Sync {
    /// Scan a directory to discover installed packages
    fn scan_installed(&self, base_path: &Path) -> Result<Vec<ServiceInstance>>;

    /// List available versions for a given service kind
    async fn list_available(&self, kind: ServiceKind) -> Result<Vec<String>>;
}
