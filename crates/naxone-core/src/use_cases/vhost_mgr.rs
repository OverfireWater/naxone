use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::domain::service::{ServiceInstance, ServiceKind};
use crate::domain::vhost::VirtualHost;
use crate::error::{Result, NaxOneError};
use crate::ports::config_io::ConfigIO;
use crate::ports::platform::PlatformOps;
use crate::ports::process::ProcessManager;
use crate::ports::template::TemplateEngine;

#[derive(Clone)]
pub struct VhostManager {
    config_io: Arc<dyn ConfigIO>,
    template_engine: Arc<dyn TemplateEngine>,
    platform_ops: Arc<dyn PlatformOps>,
    process_mgr: Arc<dyn ProcessManager>,
}

/// 原子写入事务：每完成一步就压入一个补偿动作；失败时反向执行。
enum RollbackAction {
    /// 恢复文件：original=None 表示原本不存在 → 回滚时删除；否则写回原内容
    RestoreFile {
        path: PathBuf,
        original: Option<String>,
    },
    /// 撤销 hosts 批量追加：一次性把这些 hostname 从 hosts 中删除
    RevertHostsAdditions { hostnames: Vec<String> },
}

struct Rollback<'a> {
    actions: Vec<RollbackAction>,
    config_io: &'a dyn ConfigIO,
    platform_ops: &'a dyn PlatformOps,
}

impl<'a> Rollback<'a> {
    fn new(config_io: &'a dyn ConfigIO, platform_ops: &'a dyn PlatformOps) -> Self {
        Self {
            actions: Vec::new(),
            config_io,
            platform_ops,
        }
    }

    /// 写入前先记录原状态，再执行写入；成功后返回 Ok
    fn write_text(&mut self, path: &Path, content: &str) -> Result<()> {
        let original = if self.config_io.exists(path) {
            Some(self.config_io.read_text(path)?)
        } else {
            None
        };
        self.config_io.write_text(path, content)?;
        self.actions.push(RollbackAction::RestoreFile {
            path: path.to_path_buf(),
            original,
        });
        Ok(())
    }

    /// 记录一批已成功追加的 hostname，回滚时一次性删除（最多再触发一次 UAC）。
    fn record_hosts_additions(&mut self, hostnames: Vec<String>) {
        if !hostnames.is_empty() {
            self.actions
                .push(RollbackAction::RevertHostsAdditions { hostnames });
        }
    }

    /// 成功提交：抛弃所有补偿动作
    fn commit(mut self) {
        self.actions.clear();
    }

    /// 反向执行所有补偿动作。单个动作失败只 warn，不中断后续回滚。
    fn rollback(self) {
        for action in self.actions.into_iter().rev() {
            match action {
                RollbackAction::RestoreFile { path, original } => {
                    let r = match original {
                        Some(content) => self.config_io.write_text(&path, &content),
                        None => self.config_io.delete(&path),
                    };
                    if let Err(e) = r {
                        tracing::warn!("回滚文件 {} 失败: {}", path.display(), e);
                    }
                }
                RollbackAction::RevertHostsAdditions { hostnames } => {
                    if let Err(e) = self.platform_ops.apply_hosts_changes(&[], &hostnames) {
                        tracing::warn!("回滚 hosts 批量追加失败: {}", e);
                    }
                }
            }
        }
    }
}

impl VhostManager {
    pub fn new(
        config_io: Arc<dyn ConfigIO>,
        template_engine: Arc<dyn TemplateEngine>,
        platform_ops: Arc<dyn PlatformOps>,
        process_mgr: Arc<dyn ProcessManager>,
    ) -> Self {
        Self {
            config_io,
            template_engine,
            platform_ops,
            process_mgr,
        }
    }

    /// Create a new virtual host: write configs, update hosts, reload web server.
    /// 原子操作：任一步骤失败时回滚已完成的文件写入 / hosts 条目。
    pub async fn create_vhost(
        &self,
        vhost: &VirtualHost,
        nginx_vhosts_dir: Option<&Path>,
        apache_vhosts_dir: Option<&Path>,
        apache_listen_conf: Option<&Path>,
        running_web_server: Option<&ServiceInstance>,
    ) -> Result<()> {
        // 深度防御：use_case 入口再校验一遍。即便 commands 层已经 validate，
        // 任何绕过 commands 直调 use_case 的路径也走不出注入。
        vhost.validate().map_err(NaxOneError::Config)?;
        let mut rb = Rollback::new(self.config_io.as_ref(), self.platform_ops.as_ref());
        match self
            .write_vhost_with_rollback(
                vhost,
                nginx_vhosts_dir,
                apache_vhosts_dir,
                apache_listen_conf,
                running_web_server,
                &mut rb,
            )
            .await
        {
            Ok(()) => {
                rb.commit();
                Ok(())
            }
            Err(e) => {
                tracing::warn!("vhost 写入失败，开始回滚: {}", e);
                rb.rollback();
                Err(e)
            }
        }
    }

    async fn write_vhost_with_rollback(
        &self,
        vhost: &VirtualHost,
        nginx_vhosts_dir: Option<&Path>,
        apache_vhosts_dir: Option<&Path>,
        apache_listen_conf: Option<&Path>,
        running_web_server: Option<&ServiceInstance>,
        rb: &mut Rollback<'_>,
    ) -> Result<()> {
        let filename = vhost.config_filename();

        // 1. Nginx vhost 配置
        if let Some(nginx_vhosts_dir) = nginx_vhosts_dir {
            let nginx_conf = self.template_engine.render_nginx_vhost(vhost)?;
            rb.write_text(&nginx_vhosts_dir.join(&filename), &nginx_conf)?;
        }

        // 2. Apache vhost 配置
        if let Some(apache_vhosts_dir) = apache_vhosts_dir {
            let apache_conf = self.template_engine.render_apache_vhost(vhost)?;
            rb.write_text(&apache_vhosts_dir.join(&filename), &apache_conf)?;
        }

        // 3. 伪静态规则（.htaccess / nginx.htaccess），文档根目录可能被用户独占，失败不回滚全量
        let nginx_htaccess = vhost.document_root.join("nginx.htaccess");
        let apache_htaccess = vhost.document_root.join(".htaccess");
        if !vhost.rewrite_rule.is_empty() {
            rb.write_text(&nginx_htaccess, &vhost.rewrite_rule)?;
            rb.write_text(&apache_htaccess, &vhost.rewrite_rule)?;
        } else {
            if self.config_io.exists(&nginx_htaccess) {
                rb.write_text(&nginx_htaccess, "")?;
            }
            if self.config_io.exists(&apache_htaccess) {
                rb.write_text(&apache_htaccess, "")?;
            }
        }

        // 4. Apache Listen.conf
        if vhost.listen_port != 80 {
            if let Some(apache_listen_conf) = apache_listen_conf {
                self.add_listen_port_tracked(apache_listen_conf, vhost.listen_port, rb)?;
            }
        }

        // 5. hosts 文件：一次性批量写入。Windows 在非管理员下会弹一次 UAC。
        if vhost.sync_hosts {
            let mut adds: Vec<(String, String)> =
                vec![(vhost.server_name.clone(), "127.0.0.1".to_string())];
            for alias in &vhost.aliases {
                if !alias.is_empty() {
                    adds.push((alias.clone(), "127.0.0.1".to_string()));
                }
            }
            if let Err(e) = self.platform_ops.apply_hosts_changes(&adds, &[]) {
                let msg = match &e {
                    NaxOneError::PermissionDenied(_) => format!(
                        "hosts 文件写入被拒绝（请在 UAC 弹窗中点【是】，或以管理员身份运行 NaxOne）: {}",
                        e
                    ),
                    _ => format!("hosts 文件写入失败: {}", e),
                };
                return Err(NaxOneError::Config(msg));
            }
            rb.record_hosts_additions(adds.into_iter().map(|(h, _)| h).collect());
        }

        // 6. Reload web server。失败必须回滚所有之前的写入。
        if let Some(instance) = running_web_server {
            if instance.status.is_running() {
                self.process_mgr.reload(instance).await?;
            }
        }

        Ok(())
    }

    /// Update an existing virtual host. 先写新的，成功 reload 后再删旧的；
    /// 若新配置写入或 reload 失败，旧配置不动。
    pub async fn update_vhost(
        &self,
        old: &VirtualHost,
        new: &VirtualHost,
        nginx_vhosts_dir: Option<&Path>,
        apache_vhosts_dir: Option<&Path>,
        apache_listen_conf: Option<&Path>,
        all_vhosts: &[VirtualHost],
        running_web_server: Option<&ServiceInstance>,
    ) -> Result<()> {
        let old_filename = old.config_filename();
        let new_filename = new.config_filename();

        self.create_vhost(
            new,
            nginx_vhosts_dir,
            apache_vhosts_dir,
            apache_listen_conf,
            running_web_server,
        )
        .await?;

        if old_filename != new_filename {
            if let Some(nginx_vhosts_dir) = nginx_vhosts_dir {
                let _ = self.config_io.delete(&nginx_vhosts_dir.join(&old_filename));
            }
            if let Some(apache_vhosts_dir) = apache_vhosts_dir {
                let _ = self.config_io.delete(&apache_vhosts_dir.join(&old_filename));
            }

            let mut removals: Vec<String> = vec![old.server_name.clone()];
            for alias in &old.aliases {
                if !alias.is_empty() {
                    removals.push(alias.clone());
                }
            }
            let _ = self.platform_ops.apply_hosts_changes(&[], &removals);

            if old.listen_port != 80 && old.listen_port != new.listen_port {
                let port_still_used = all_vhosts
                    .iter()
                    .any(|v| v.id != old.id && v.listen_port == old.listen_port);
                if !port_still_used {
                    if let Some(apache_listen_conf) = apache_listen_conf {
                        let _ = self.remove_listen_port(apache_listen_conf, old.listen_port);
                    }
                }
            }
        }

        Ok(())
    }

    /// Delete a virtual host
    pub async fn delete_vhost(
        &self,
        vhost: &VirtualHost,
        nginx_vhosts_dir: Option<&Path>,
        apache_vhosts_dir: Option<&Path>,
        apache_listen_conf: Option<&Path>,
        all_vhosts: &[VirtualHost],
        running_web_server: Option<&ServiceInstance>,
    ) -> Result<()> {
        let filename = vhost.config_filename();

        if let Some(nginx_vhosts_dir) = nginx_vhosts_dir {
            let _ = self.config_io.delete(&nginx_vhosts_dir.join(&filename));
        }
        if let Some(apache_vhosts_dir) = apache_vhosts_dir {
            let _ = self.config_io.delete(&apache_vhosts_dir.join(&filename));
        }

        if vhost.listen_port != 80 {
            let port_still_used = all_vhosts
                .iter()
                .any(|v| v.id != vhost.id && v.listen_port == vhost.listen_port);
            if !port_still_used {
                if let Some(apache_listen_conf) = apache_listen_conf {
                    self.remove_listen_port(apache_listen_conf, vhost.listen_port)?;
                }
            }
        }

        let mut removals: Vec<String> = vec![vhost.server_name.clone()];
        for alias in &vhost.aliases {
            if !alias.is_empty() {
                removals.push(alias.clone());
            }
        }
        let _ = self.platform_ops.apply_hosts_changes(&[], &removals);

        let _ = running_web_server;

        Ok(())
    }

    // --- Apache Listen.conf helpers ---

    /// add_listen_port 的可回滚版本
    fn add_listen_port_tracked(
        &self,
        listen_conf: &Path,
        port: u16,
        rb: &mut Rollback<'_>,
    ) -> Result<()> {
        let content = if self.config_io.exists(listen_conf) {
            self.config_io.read_text(listen_conf)?
        } else {
            String::from("Listen 80\n")
        };

        let listen_line = format!("Listen {}", port);
        if content.lines().any(|l| l.trim() == listen_line) {
            return Ok(());
        }

        let new_content = format!("{}\n{}\n", content.trim_end(), listen_line);
        rb.write_text(listen_conf, &new_content)
    }

    fn remove_listen_port(&self, listen_conf: &Path, port: u16) -> Result<()> {
        if !self.config_io.exists(listen_conf) {
            return Ok(());
        }

        let content = self.config_io.read_text(listen_conf)?;
        let listen_line = format!("Listen {}", port);
        let filtered: Vec<&str> = content
            .lines()
            .filter(|l| l.trim() != listen_line)
            .collect();

        self.config_io
            .write_text(listen_conf, &filtered.join("\n"))
    }

    /// 端口冲突检查。如果端口已被 NaxOne 自身管理的某个 vhost 使用，则视为安全
    /// （nginx/apache 统一用同一个监听进程）；否则尝试试绑 0.0.0.0:port，失败说明被外部进程占用。
    ///
    /// - `port`: 目标端口
    /// - `all_vhosts`: 当前已存在的 vhost 列表
    /// - `self_id`: 如果是 update 流程，传入当前 vhost 的 id（排除自身同端口复用）
    pub fn check_port_conflict(
        port: u16,
        all_vhosts: &[VirtualHost],
        self_id: Option<&str>,
    ) -> Result<()> {
        // 80 永远是 web 服务器默认监听，不单独检查
        if port == 80 {
            return Ok(());
        }
        // 已被我方 vhost 占用 → 安全
        let owned_by_us = all_vhosts.iter().any(|v| {
            v.listen_port == port && self_id.map(|id| v.id != id).unwrap_or(true)
        });
        if owned_by_us {
            return Ok(());
        }
        // 未被我方使用 → 试绑
        use std::net::{Ipv4Addr, SocketAddr, TcpListener};
        let addr = SocketAddr::from((Ipv4Addr::UNSPECIFIED, port));
        match TcpListener::bind(addr) {
            Ok(listener) => {
                drop(listener);
                Ok(())
            }
            Err(e) => {
                tracing::warn!("端口 {} 试绑失败: {}", port, e);
                Err(NaxOneError::PortInUse { port, by: None })
            }
        }
    }

    /// Find the running web server instance (Nginx or Apache) from the service list
    pub fn find_running_web_server(services: &[ServiceInstance]) -> Option<&ServiceInstance> {
        services.iter().find(|s| {
            (s.kind == ServiceKind::Nginx || s.kind == ServiceKind::Apache) && s.status.is_running()
        })
    }

    /// Reload a web server
    pub async fn reload_web_server(&self, instance: &ServiceInstance) -> Result<()> {
        if instance.status.is_running() {
            self.process_mgr.reload(instance).await?;
        }
        Ok(())
    }

    /// 只重新生成 .conf 文件（nginx + apache），不写 hosts、不 reload。
    /// 用于 nginx/apache 重装后从 vhosts.json 元数据恢复站点配置。
    /// 已存在的 .conf 文件**不会**被覆盖（避免覆盖用户手动改过的）。
    pub fn regenerate_configs(
        &self,
        vhost: &VirtualHost,
        nginx_vhosts_dir: Option<&Path>,
        apache_vhosts_dir: Option<&Path>,
    ) -> Result<bool> {
        let filename = vhost.config_filename();
        let mut wrote = false;
        if let Some(dir) = nginx_vhosts_dir {
            let target = dir.join(&filename);
            if !self.config_io.exists(&target) {
                let conf = self.template_engine.render_nginx_vhost(vhost)?;
                self.config_io.write_text(&target, &conf)?;
                wrote = true;
            }
        }
        if let Some(dir) = apache_vhosts_dir {
            let target = dir.join(&filename);
            if !self.config_io.exists(&target) {
                let conf = self.template_engine.render_apache_vhost(vhost)?;
                self.config_io.write_text(&target, &conf)?;
                wrote = true;
            }
        }
        Ok(wrote)
    }

    /// Save vhost metadata to JSON file
    pub fn save_vhosts_json(&self, path: &Path, vhosts: &[VirtualHost]) -> Result<()> {
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let json = serde_json::to_string_pretty(vhosts).map_err(|e| {
            crate::error::NaxOneError::Config(format!("Failed to serialize vhosts: {e}"))
        })?;
        std::fs::write(path, json)?;
        Ok(())
    }

    /// Load vhost metadata from JSON file
    pub fn load_vhosts_json(path: &Path) -> Vec<VirtualHost> {
        if !path.exists() {
            return Vec::new();
        }
        let content = match std::fs::read_to_string(path) {
            Ok(c) => c,
            Err(_) => return Vec::new(),
        };
        serde_json::from_str(&content).unwrap_or_default()
    }

    /// Merge scanned vhosts (from .conf files) with saved metadata (from JSON).
    /// Also includes saved-only vhosts that weren't scanned (e.g. their .conf was
    /// manually deleted, or they live in a directory the scanner can't see).
    pub fn merge_vhosts(scanned: Vec<VirtualHost>, saved: Vec<VirtualHost>) -> Vec<VirtualHost> {
        let mut result = Vec::new();
        let mut seen_ids = std::collections::HashSet::new();

        for mut scan in scanned {
            if let Some(meta) = saved.iter().find(|s| s.id == scan.id) {
                scan.created_at = meta.created_at.clone();
                scan.expires_at = meta.expires_at.clone();
                scan.enabled = meta.enabled;
                scan.sync_hosts = meta.sync_hosts;
                scan.source = meta.source.clone();
            }
            seen_ids.insert(scan.id.clone());
            result.push(scan);
        }

        for meta in saved {
            if !seen_ids.contains(&meta.id) {
                result.push(meta);
            }
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::vhost::{VhostSource, VirtualHost};
    use std::path::PathBuf;

    fn mk_vhost(id: &str, port: u16) -> VirtualHost {
        VirtualHost {
            id: id.to_string(),
            server_name: format!("{id}.test"),
            aliases: vec![],
            listen_port: port,
            document_root: PathBuf::from("C:/www"),
            php_version: None,
            php_fastcgi_port: None,
            php_install_path: None,
            index_files: "index.php".into(),
            rewrite_rule: String::new(),
            autoindex: false,
            ssl: None,
            custom_directives: None,
            access_log: None,
            enabled: true,
            created_at: String::new(),
            expires_at: String::new(),
            sync_hosts: true,
            source: VhostSource::Custom,
        }
    }

    #[test]
    fn port_80_is_always_ok() {
        assert!(VhostManager::check_port_conflict(80, &[], None).is_ok());
    }

    #[test]
    fn port_owned_by_existing_vhost_is_ok() {
        let existing = vec![mk_vhost("a", 8080)];
        // 新 vhost 也用 8080 应通过（同一 nginx 进程承载多个 vhost）
        assert!(VhostManager::check_port_conflict(8080, &existing, None).is_ok());
    }

    #[test]
    fn updating_self_on_same_port_is_ok() {
        let existing = vec![mk_vhost("a", 8080)];
        // 更新 id="a" 自身的端口仍为 8080 → 试绑会失败但自占用应短路
        assert!(VhostManager::check_port_conflict(8080, &existing, Some("a")).is_ok());
    }
}
