use std::path::PathBuf;

use crate::error::Result;

/// Platform-specific operations
pub trait PlatformOps: Send + Sync {
    /// Path to the system hosts file
    fn hosts_file_path(&self) -> PathBuf;

    /// 批量更新 hosts 文件：一次性应用 additions（追加）+ removals（删除）。
    /// Windows 实现会在权限不足时自动通过 UAC 提权一次性写入，避免多次弹框。
    /// `additions`: (hostname, ip) 列表
    /// `removals`: hostname 列表
    fn apply_hosts_changes(
        &self,
        additions: &[(String, String)],
        removals: &[String],
    ) -> Result<()>;

    /// Get the default data directory for NaxOne
    fn data_dir(&self) -> PathBuf;

    /// 在系统防火墙里放行一个 TCP 端口的入站访问。
    /// 默认不做任何事（不支持的平台直接成功返回）。
    /// 实现应该是**幂等**的：同一端口多次调用不会报错，也不会产生多条规则。
    fn add_firewall_port(&self, _port: u16) -> Result<()> {
        Ok(())
    }

    /// 移除之前 add_firewall_port 放行的规则。不存在时应返回 Ok。
    fn remove_firewall_port(&self, _port: u16) -> Result<()> {
        Ok(())
    }
}
