use std::path::PathBuf;

use naxone_core::error::{Result, NaxOneError};
use naxone_core::ports::platform::PlatformOps;

pub struct LinuxPlatform;

impl PlatformOps for LinuxPlatform {
    fn hosts_file_path(&self) -> PathBuf {
        PathBuf::from("/etc/hosts")
    }

    fn apply_hosts_changes(
        &self,
        additions: &[(String, String)],
        removals: &[String],
    ) -> Result<()> {
        if additions.is_empty() && removals.is_empty() {
            return Ok(());
        }
        Err(NaxOneError::Process("Linux support not yet implemented".into()))
    }

    fn data_dir(&self) -> PathBuf {
        // dev / prod 后缀复用通用 helper（dirs 模块 cfg!(debug_assertions) 决定）
        let home = std::env::var("HOME").unwrap_or_else(|_| "/root".into());
        PathBuf::from(home).join(crate::platform::dirs::naxone_home_dirname())
    }
}
