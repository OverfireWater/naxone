use std::path::PathBuf;

use ruststudy_core::error::{Result, RustStudyError};
use ruststudy_core::ports::platform::PlatformOps;

pub struct LinuxPlatform;

impl PlatformOps for LinuxPlatform {
    fn hosts_file_path(&self) -> PathBuf {
        PathBuf::from("/etc/hosts")
    }

    fn add_hosts_entry(&self, _hostname: &str, _ip: &str) -> Result<()> {
        Err(RustStudyError::Process("Linux support not yet implemented".into()))
    }

    fn remove_hosts_entry(&self, _hostname: &str) -> Result<()> {
        Err(RustStudyError::Process("Linux support not yet implemented".into()))
    }

    fn data_dir(&self) -> PathBuf {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/root".into());
        PathBuf::from(home).join(".ruststudy")
    }
}
