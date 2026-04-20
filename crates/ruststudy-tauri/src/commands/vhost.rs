use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tauri::State;

use crate::commands::logger::push_log;
use crate::state::AppState;
use ruststudy_core::domain::log::LogLevel;
use ruststudy_core::domain::service::ServiceKind;
use ruststudy_core::domain::vhost::{VhostSource, VirtualHost};
use ruststudy_core::use_cases::vhost_mgr::VhostManager;

fn persist_vhosts(vhosts: &[VirtualHost], state: &AppState) {
    let path = crate::state::vhosts_json_path();
    let _ = state.vhost_manager.save_vhosts_json(&path, vhosts);
}

/// Best-effort：尝试给端口加 Windows 防火墙入站放行。
/// 80 端口通常由 nginx/apache 首次运行时系统已自动提示过，不重复处理。
async fn try_open_firewall(port: u16, state: &AppState) {
    if port == 80 {
        return;
    }
    match state.platform_ops.add_firewall_port(port) {
        Ok(_) => {
            push_log(
                state,
                LogLevel::Info,
                "vhost",
                format!("已放行防火墙端口 {}（RustStudy port {}）", port, port),
                Some("其他设备/手机可通过本机 IP 访问此端口".to_string()),
                None,
            )
            .await;
        }
        Err(e) => {
            push_log(
                state,
                LogLevel::Warn,
                "vhost",
                format!("防火墙端口 {} 未放行", port),
                Some(format!("{}。可手动在 Windows 防火墙入站规则中为 TCP 端口 {} 放行。", e, port)),
                None,
            )
            .await;
        }
    }
}

/// Best-effort：端口不再被任何 vhost 使用时关闭对应防火墙规则。
async fn try_close_firewall(port: u16, remaining_vhosts: &[VirtualHost], state: &AppState) {
    if port == 80 {
        return;
    }
    let still_used = remaining_vhosts.iter().any(|v| v.listen_port == port);
    if still_used {
        return;
    }
    if let Err(e) = state.platform_ops.remove_firewall_port(port) {
        push_log(
            state,
            LogLevel::Warn,
            "vhost",
            format!("防火墙端口 {} 关闭失败", port),
            Some(e.to_string()),
            None,
        )
        .await;
    }
}

// --- DTOs ---

#[derive(Debug, Clone, Serialize)]
pub struct VhostInfo {
    pub id: String,
    pub server_name: String,
    pub aliases: Vec<String>,
    pub listen_port: u16,
    pub document_root: String,
    pub php_version: Option<String>,
    pub index_files: String,
    pub rewrite_rule: String,
    pub autoindex: bool,
    pub has_ssl: bool,
    pub ssl_cert: String,
    pub ssl_key: String,
    pub force_https: bool,
    pub custom_directives: String,
    pub access_log: String,
    pub enabled: bool,
    pub created_at: String,
    pub expires_at: String,
    /// "phpstudy" | "custom" | "standalone"
    pub source: String,
    /// Package name when source = "standalone" (e.g. "nginx")
    pub source_name: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CreateVhostRequest {
    pub server_name: String,
    pub aliases: String,
    pub listen_port: u16,
    pub document_root: String,
    pub php_version: Option<String>,
    #[serde(default = "default_index_files")]
    pub index_files: String,
    #[serde(default)]
    pub rewrite_rule: String,
    #[serde(default)]
    pub autoindex: bool,
    pub ssl_cert: Option<String>,
    pub ssl_key: Option<String>,
    #[serde(default)]
    pub force_https: bool,
    pub custom_directives: Option<String>,
    pub access_log: Option<String>,
    #[serde(default = "default_sync_hosts")]
    pub sync_hosts: bool,
    #[serde(default)]
    pub expires_at: String,
}

fn default_sync_hosts() -> bool {
    true
}

fn default_index_files() -> String {
    "index.php index.html".into()
}

#[derive(Debug, Clone, Serialize)]
pub struct PhpVersionInfo {
    pub label: String,
    pub version: String,
    pub port: u16,
    pub install_path: String,
}

// --- Conversions ---

fn to_info(v: &VirtualHost) -> VhostInfo {
    let (source, source_name) = match &v.source {
        VhostSource::PhpStudy => ("phpstudy".into(), None),
        VhostSource::Custom => ("custom".into(), None),
        VhostSource::Standalone(name) => ("standalone".into(), Some(name.clone())),
    };
    VhostInfo {
        id: v.id.clone(),
        server_name: v.server_name.clone(),
        aliases: v.aliases.clone(),
        listen_port: v.listen_port,
        document_root: v.document_root.display().to_string(),
        php_version: v.php_version.clone(),
        index_files: v.index_files.clone(),
        rewrite_rule: v.rewrite_rule.clone(),
        autoindex: v.autoindex,
        has_ssl: v.ssl.is_some(),
        ssl_cert: v.ssl.as_ref().map(|s| s.cert_path.display().to_string()).unwrap_or_default(),
        ssl_key: v.ssl.as_ref().map(|s| s.key_path.display().to_string()).unwrap_or_default(),
        force_https: v.ssl.as_ref().map(|s| s.force_https).unwrap_or(false),
        custom_directives: v.custom_directives.clone().unwrap_or_default(),
        access_log: v.access_log.clone().unwrap_or_default(),
        enabled: v.enabled,
        created_at: v.created_at.clone(),
        expires_at: v.expires_at.clone(),
        source,
        source_name,
    }
}

fn all_infos(vhosts: &[VirtualHost]) -> Vec<VhostInfo> {
    vhosts.iter().map(to_info).collect()
}

/// Build a VirtualHost from a CreateVhostRequest, resolving PHP version to port and path
fn build_vhost(
    req: &CreateVhostRequest,
    _state: &AppState,
    services: &[ruststudy_core::domain::service::ServiceInstance],
) -> VirtualHost {
    let aliases: Vec<String> = req
        .aliases
        .split_whitespace()
        .map(|s| s.to_string())
        .collect();

    let id = format!("{}_{}", req.server_name, req.listen_port);

    // Resolve PHP version to port and install path
    let (php_fastcgi_port, php_install_path) = if let Some(ref php_ver) = req.php_version {
        let php_inst = services.iter().find(|s| {
            if s.kind != ServiceKind::Php {
                return false;
            }
            let inst_ver = if let Some(ref variant) = s.variant {
                format!("php{}{}", s.version.replace('.', ""), variant)
            } else {
                format!("php{}", s.version.replace('.', ""))
            };
            &inst_ver == php_ver
        });
        match php_inst {
            Some(inst) => (Some(inst.port), Some(inst.install_path.clone())),
            None => {
                // PHP version selected but not found in services - warn but don't fail
                tracing::warn!("PHP version '{}' not found in installed services", php_ver);
                (None, None)
            },
        }
    } else {
        (None, None)
    };

    let ssl = match (&req.ssl_cert, &req.ssl_key) {
        (Some(cert), Some(key)) if !cert.is_empty() && !key.is_empty() => {
            Some(ruststudy_core::domain::vhost::SslConfig {
                cert_path: PathBuf::from(cert),
                key_path: PathBuf::from(key),
                force_https: req.force_https,
            })
        }
        _ => None,
    };

    VirtualHost {
        id,
        server_name: req.server_name.clone(),
        aliases,
        listen_port: req.listen_port,
        document_root: PathBuf::from(&req.document_root),
        php_version: req.php_version.clone(),
        php_fastcgi_port,
        php_install_path,
        index_files: if req.index_files.is_empty() { "index.php index.html".into() } else { req.index_files.clone() },
        rewrite_rule: req.rewrite_rule.clone(),
        autoindex: req.autoindex,
        ssl,
        custom_directives: req.custom_directives.clone().filter(|s| !s.is_empty()),
        access_log: req.access_log.clone().filter(|s| !s.is_empty()),
        enabled: true,
        created_at: chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
        expires_at: req.expires_at.clone(),
        sync_hosts: req.sync_hosts,
        source: VhostSource::Custom,
    }
}

// --- Helper: resolve vhost dirs ---

struct VhostDirs {
    nginx_vhosts: PathBuf,
    apache_vhosts: PathBuf,
    apache_listen: PathBuf,
}

async fn resolve_dirs(state: &AppState) -> Result<VhostDirs, String> {
    // Prefer deriving the vhosts directories from the currently scanned services,
    // which works for PHPStudy AND standalone/store installs. Fall back to the
    // PHPStudy config path for backward compatibility.
    let services = state.services.read().await;
    let nginx_install = services
        .iter()
        .find(|s| s.kind == ServiceKind::Nginx)
        .map(|s| s.install_path.clone());
    let apache_install = services
        .iter()
        .find(|s| s.kind == ServiceKind::Apache)
        .map(|s| s.install_path.clone());
    drop(services);

    let (nginx_dir, apache_dir) = match (nginx_install, apache_install) {
        (Some(n), Some(a)) => (n, a),
        (n, a) => {
            // Try PHPStudy fallback if either is missing from the services list
            let config = state.config.read().await;
            let phpstudy = config
                .general
                .phpstudy_path
                .as_ref()
                .ok_or("未检测到 Nginx 或 Apache — 请先在设置中指定 PHPStudy 路径或添加独立安装路径")?;
            let ext = phpstudy.join("Extensions");
            let nginx_dir = n.or_else(|| find_extension_dir(&ext, "Nginx"))
                .ok_or("未找到 Nginx 安装")?;
            let apache_dir = a.or_else(|| find_extension_dir(&ext, "Apache"))
                .ok_or("未找到 Apache 安装")?;
            (nginx_dir, apache_dir)
        }
    };

    Ok(VhostDirs {
        nginx_vhosts: nginx_dir.join("conf").join("vhosts"),
        apache_vhosts: apache_dir.join("conf").join("vhosts"),
        apache_listen: apache_dir.join("conf").join("vhosts").join("Listen.conf"),
    })
}

fn find_extension_dir(ext_path: &std::path::Path, prefix: &str) -> Option<PathBuf> {
    if let Ok(entries) = std::fs::read_dir(ext_path) {
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name_str = name.to_string_lossy();
            if name_str.starts_with(prefix) && entry.path().is_dir() {
                return Some(entry.path());
            }
        }
    }
    None
}

// --- Commands ---

#[tauri::command]
pub async fn get_vhosts(state: State<'_, AppState>) -> Result<Vec<VhostInfo>, String> {
    let vhosts = state.vhosts.read().await;
    Ok(all_infos(&vhosts))
}

#[tauri::command]
pub async fn create_vhost(
    req: CreateVhostRequest,
    state: State<'_, AppState>,
) -> Result<Vec<VhostInfo>, String> {
    let dirs = resolve_dirs(&state).await?;
    let services = state.services.read().await;
    let vhost = build_vhost(&req, &state, &services);

    let running_ws = VhostManager::find_running_web_server(&services);

    let domain_port = format!("{}:{}", vhost.server_name, vhost.listen_port);

    // 端口冲突预检（避免写入配置后 reload 失败导致不一致）
    {
        let existing = state.vhosts.read().await;
        if let Err(e) = VhostManager::check_port_conflict(vhost.listen_port, &existing, None) {
            let msg = e.to_string();
            push_log(&state, LogLevel::Error, "vhost", format!("创建站点 {} 失败", domain_port), Some(msg.clone()), None).await;
            return Err(msg);
        }
    }

    match state.vhost_manager.create_vhost(&vhost, &dirs.nginx_vhosts, &dirs.apache_vhosts, &dirs.apache_listen, running_ws).await {
        Ok(_) => {
            let details = format!("文档根: {}\nPHP: {}\n伪静态: {}\nSSL: {}",
                vhost.document_root.display(),
                vhost.php_version.as_deref().unwrap_or("无"),
                if vhost.rewrite_rule.is_empty() { "无" } else { "已设置" },
                if vhost.ssl.is_some() { "已启用" } else { "未启用" });
            push_log(&state, LogLevel::Success, "vhost", format!("创建站点 {}", domain_port), Some(details), None).await;
        }
        Err(e) => {
            let msg = e.to_string();
            push_log(&state, LogLevel::Error, "vhost", format!("创建站点 {} 失败", domain_port), Some(msg.clone()), None).await;
            return Err(msg);
        }
    }

    drop(services);

    // 防火墙放行（best-effort，失败不阻止站点创建）
    try_open_firewall(vhost.listen_port, &state).await;

    let mut vhosts = state.vhosts.write().await;
    vhosts.push(vhost);
    persist_vhosts(&vhosts, &state);
    Ok(all_infos(&vhosts))
}

#[tauri::command]
pub async fn update_vhost(
    id: String,
    req: CreateVhostRequest,
    state: State<'_, AppState>,
) -> Result<Vec<VhostInfo>, String> {
    let dirs = resolve_dirs(&state).await?;
    let services = state.services.read().await;

    let mut vhosts = state.vhosts.write().await;
    let idx = vhosts
        .iter()
        .position(|v| v.id == id)
        .ok_or_else(|| format!("Vhost not found: {}", id))?;

    let old = vhosts[idx].clone();
    let new_vhost = build_vhost(&req, &state, &services);
    let running_ws = VhostManager::find_running_web_server(&services);

    let domain = new_vhost.server_name.clone();

    // 端口冲突预检（仅当端口变更时）
    if old.listen_port != new_vhost.listen_port {
        if let Err(e) = VhostManager::check_port_conflict(new_vhost.listen_port, &vhosts, Some(&id)) {
            let msg = e.to_string();
            push_log(&state, LogLevel::Error, "vhost", format!("更新站点 {} 失败", domain), Some(msg.clone()), None).await;
            return Err(msg);
        }
    }

    if let Err(e) = state.vhost_manager.update_vhost(&old, &new_vhost, &dirs.nginx_vhosts, &dirs.apache_vhosts, &dirs.apache_listen, &vhosts, running_ws).await {
        let msg = e.to_string();
        push_log(&state, LogLevel::Error, "vhost", format!("更新站点 {} 失败", domain), Some(msg.clone()), None).await;
        return Err(msg);
    }
    push_log(&state, LogLevel::Success, "vhost", format!("更新站点 {}", domain), None, None).await;

    let port_changed = old.listen_port != new_vhost.listen_port;
    vhosts[idx] = new_vhost.clone();
    persist_vhosts(&vhosts, &state);

    // 端口变更 → 新端口放行、旧端口若无人用则关闭
    if port_changed {
        let vhosts_snap = vhosts.clone();
        drop(vhosts);
        try_open_firewall(new_vhost.listen_port, &state).await;
        try_close_firewall(old.listen_port, &vhosts_snap, &state).await;
        let vhosts = state.vhosts.read().await;
        return Ok(all_infos(&vhosts));
    }
    Ok(all_infos(&vhosts))
}

#[tauri::command]
pub async fn delete_vhost(
    id: String,
    state: State<'_, AppState>,
) -> Result<Vec<VhostInfo>, String> {
    let dirs = resolve_dirs(&state).await?;
    let services = state.services.read().await;

    let mut vhosts = state.vhosts.write().await;
    let idx = vhosts
        .iter()
        .position(|v| v.id == id)
        .ok_or_else(|| format!("Vhost not found: {}", id))?;

    let vhost = &vhosts[idx];
    let running_ws = VhostManager::find_running_web_server(&services);

    let domain = vhost.server_name.clone();
    let deleted_port = vhost.listen_port;
    if let Err(e) = state.vhost_manager.delete_vhost(vhost, &dirs.nginx_vhosts, &dirs.apache_vhosts, &dirs.apache_listen, &vhosts, running_ws).await {
        let msg = e.to_string();
        push_log(&state, LogLevel::Error, "vhost", format!("删除站点 {} 失败", domain), Some(msg.clone()), None).await;
        return Err(msg);
    }
    push_log(&state, LogLevel::Success, "vhost", format!("删除站点 {}", domain), None, None).await;

    vhosts.remove(idx);
    persist_vhosts(&vhosts, &state);

    // 若此端口不再被其他 vhost 使用 → 关闭防火墙规则（best-effort）
    let vhosts_snap = vhosts.clone();
    drop(vhosts);
    try_close_firewall(deleted_port, &vhosts_snap, &state).await;
    let vhosts = state.vhosts.read().await;
    Ok(all_infos(&vhosts))
}

/// Enable or disable a vhost by renaming .conf to .conf.disabled and vice versa
#[tauri::command]
pub async fn toggle_vhost(
    id: String,
    enabled: bool,
    state: State<'_, AppState>,
) -> Result<Vec<VhostInfo>, String> {
    let dirs = resolve_dirs(&state).await?;
    let mut vhosts = state.vhosts.write().await;
    let idx = vhosts.iter().position(|v| v.id == id).ok_or("Vhost not found")?;

    let filename = vhosts[idx].config_filename();
    let nginx_conf = dirs.nginx_vhosts.join(&filename);
    let apache_conf = dirs.apache_vhosts.join(&filename);
    let nginx_disabled = dirs.nginx_vhosts.join(format!("{}.disabled", filename));
    let apache_disabled = dirs.apache_vhosts.join(format!("{}.disabled", filename));

    // Track renamed files for rollback
    let mut renamed: Vec<(std::path::PathBuf, std::path::PathBuf)> = Vec::new();
    if enabled {
        if nginx_disabled.exists() {
            if std::fs::rename(&nginx_disabled, &nginx_conf).is_ok() { renamed.push((nginx_conf.clone(), nginx_disabled.clone())); }
        }
        if apache_disabled.exists() {
            if std::fs::rename(&apache_disabled, &apache_conf).is_ok() { renamed.push((apache_conf.clone(), apache_disabled.clone())); }
        }
    } else {
        if nginx_conf.exists() {
            if std::fs::rename(&nginx_conf, &nginx_disabled).is_ok() { renamed.push((nginx_disabled.clone(), nginx_conf.clone())); }
        }
        if apache_conf.exists() {
            if std::fs::rename(&apache_conf, &apache_disabled).is_ok() { renamed.push((apache_disabled.clone(), apache_conf.clone())); }
        }
    }

    // Reload web server — if fails, rollback file renames
    let services = state.services.read().await;
    if let Some(ws) = VhostManager::find_running_web_server(&services) {
        if let Err(e) = state.vhost_manager.reload_web_server(ws).await {
            // Rollback
            for (from, to) in renamed.iter().rev() {
                let _ = std::fs::rename(from, to);
            }
            return Err(format!("Reload 失败: {}", e));
        }
    }

    vhosts[idx].enabled = enabled;
    let domain = vhosts[idx].server_name.clone();
    push_log(&state, LogLevel::Success, "vhost",
        if enabled { format!("启用站点 {}", domain) } else { format!("停用站点 {}", domain) },
        None, None).await;

    persist_vhosts(&vhosts, &state);
    Ok(all_infos(&vhosts))
}

/// Check for expired vhosts and auto-disable them
#[tauri::command]
pub async fn check_expired_vhosts(state: State<'_, AppState>) -> Result<Vec<VhostInfo>, String> {
    let today = chrono::Local::now().format("%Y-%m-%d").to_string();
    let dirs = resolve_dirs(&state).await?;
    let mut vhosts = state.vhosts.write().await;
    let mut changed = false;

    for vh in vhosts.iter_mut() {
        if vh.enabled && !vh.expires_at.is_empty() && vh.expires_at <= today {
            // Auto-disable expired vhost
            vh.enabled = false;
            let filename = vh.config_filename();
            let nginx_conf = dirs.nginx_vhosts.join(&filename);
            let apache_conf = dirs.apache_vhosts.join(&filename);
            if nginx_conf.exists() { let _ = std::fs::rename(&nginx_conf, dirs.nginx_vhosts.join(format!("{}.disabled", filename))); }
            if apache_conf.exists() { let _ = std::fs::rename(&apache_conf, dirs.apache_vhosts.join(format!("{}.disabled", filename))); }
            changed = true;
        }
    }

    if changed {
        persist_vhosts(&vhosts, &state);
        // Reload web server
        let services = state.services.read().await;
        if let Some(ws) = VhostManager::find_running_web_server(&services) {
            let _ = state.vhost_manager.reload_web_server(ws).await;
        }
    }

    Ok(all_infos(&vhosts))
}

#[tauri::command]
pub async fn get_php_versions(state: State<'_, AppState>) -> Result<Vec<PhpVersionInfo>, String> {
    let services = state.services.read().await;
    let php_versions: Vec<PhpVersionInfo> = services
        .iter()
        .filter(|s| s.kind == ServiceKind::Php)
        .map(|s| {
            let label = if let Some(ref variant) = s.variant {
                format!("PHP {} ({})", s.version, variant)
            } else {
                format!("PHP {}", s.version)
            };
            let version = if let Some(ref variant) = s.variant {
                format!("php{}{}", s.version.replace('.', ""), variant)
            } else {
                format!("php{}", s.version.replace('.', ""))
            };
            PhpVersionInfo {
                label,
                version,
                port: s.port,
                install_path: s.install_path.display().to_string(),
            }
        })
        .collect();
    Ok(php_versions)
}
