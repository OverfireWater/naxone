use std::path::{Path, PathBuf};

use serde::Serialize;
use tauri::State;

use crate::commands::logger::logged;
use crate::state::{resolve_packages_root, AppState};

// ─── 返回类型 ────────────────────────────────────────────────────────────

#[derive(Serialize)]
pub struct DevToolsInfo {
    pub composer: Option<ComposerToolInfo>,
    pub nvm: Option<NvmToolInfo>,
    pub mysql: Option<MysqlToolInfo>,
}

#[derive(Serialize)]
pub struct MysqlToolInfo {
    /// 当前 user/system PATH 中匹配到的 MySQL 版本（多个匹配时取第一个）
    pub active_version: Option<String>,
    pub available: Vec<MysqlOption>,
    /// 系统 PATH (HKLM) 中含 mysqld.exe 但不属于"当前选中要设全局"的版本的目录
    /// —— 它们会屏蔽用户 PATH 的切换。前端用来展示一键修复横条。
    pub conflicts: Vec<String>,
}

#[derive(Serialize, Clone)]
pub struct MysqlOption {
    pub version: String,
    pub install_path: String,
    pub data_dir: String,
    pub bin_dir: String,
    pub port: u16,
    pub initialized: bool,
    pub root_password: String,
    /// "store" / "phpstudy" / "manual" / "system"
    pub source: String,
}

#[derive(Serialize)]
pub struct ComposerToolInfo {
    pub active_version: Option<String>,
    pub available: Vec<ComposerOption>,
}

#[derive(Serialize, Clone)]
pub struct ComposerOption {
    pub version: String,
    pub source: String,
    pub phar_path: String,
}

#[derive(Serialize)]
pub struct NvmToolInfo {
    pub nvm_version: String,
    pub nvm_source: String,
    pub nvm_home: String,
    pub current_node: Option<String>,
    pub installed_nodes: Vec<String>,
}

// ─── Commands ────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn get_dev_tools_info(state: State<'_, AppState>) -> Result<DevToolsInfo, String> {
    let config = state.config.read().await;
    let packages_root = resolve_packages_root(&config);
    drop(config);

    let services_snap = state.services.read().await.clone();

    let composer = build_composer_info(&packages_root);
    let nvm = build_nvm_info(&packages_root);
    let mysql = build_mysql_info(&packages_root, &services_snap);
    Ok(DevToolsInfo { composer, nvm, mysql })
}

#[tauri::command]
pub async fn switch_node_version(
    version: String,
    state: State<'_, AppState>,
) -> Result<NvmToolInfo, String> {
    let label = format!("切换 Node.js 到 v{}", version);
    let result = do_switch_node(&version, &state).await;
    logged(&state, "tool", label, result).await
}

async fn do_switch_node(version: &str, state: &AppState) -> Result<NvmToolInfo, String> {
    use naxone_adapters::package::tool_detect;

    let nvm_home =
        tool_detect::get_nvm_home().ok_or_else(|| "NVM_HOME 未设置".to_string())?;
    let nvm_exe = nvm_home.join("nvm.exe");
    if !nvm_exe.exists() {
        return Err("nvm.exe 不存在".into());
    }

    tool_detect::switch_node(&nvm_exe, version)?;

    let config = state.config.read().await;
    let packages_root = resolve_packages_root(&config);
    drop(config);

    build_nvm_info(&packages_root).ok_or_else(|| "切换后读取 NVM 信息失败".into())
}

#[tauri::command]
pub async fn set_global_composer(
    version: String,
    state: State<'_, AppState>,
) -> Result<DevToolsInfo, String> {
    let label = format!("设置全局 Composer 为 v{}", version);
    let result = do_set_global_composer(&version, &state).await;
    logged(&state, "tool", label, result).await
}

async fn do_set_global_composer(version: &str, state: &AppState) -> Result<DevToolsInfo, String> {
    let config = state.config.read().await;
    let packages_root = resolve_packages_root(&config);
    drop(config);
    let services_snap = state.services.read().await.clone();

    let info = build_composer_info(&packages_root)
        .ok_or_else(|| "未找到任何 Composer 安装".to_string())?;

    let selected = info
        .available
        .iter()
        .find(|a| a.version == version)
        .ok_or_else(|| format!("未找到 Composer v{}", version))?;

    let phar_path = PathBuf::from(&selected.phar_path);
    if !phar_path.exists() {
        return Err(format!("composer.phar 不存在: {}", selected.phar_path));
    }

    let bin_dir = naxone_adapters::platform::global_php::bin_dir();
    std::fs::create_dir_all(&bin_dir)
        .map_err(|e| format!("创建 bin 目录失败: {}", e))?;

    let php_shim = bin_dir.join("php.cmd");
    let content = format!(
        "@echo off\r\n\"{}\" \"{}\" %*\r\n",
        php_shim.display(),
        phar_path.display()
    );
    std::fs::write(bin_dir.join("composer.bat"), content.as_bytes())
        .map_err(|e| format!("写 composer shim 失败: {}", e))?;

    let _ = naxone_adapters::platform::global_php::ensure_path_in_user_env();

    let composer = build_composer_info(&packages_root);
    let nvm = build_nvm_info(&packages_root);
    let mysql = build_mysql_info(&packages_root, &services_snap);
    Ok(DevToolsInfo { composer, nvm, mysql })
}

// ─── 构建逻辑 ────────────────────────────────────────────────────────────

fn build_composer_info(packages_root: &Path) -> Option<ComposerToolInfo> {
    use naxone_adapters::package::tool_detect;

    let mut available: Vec<ComposerOption> = Vec::new();

    if let Some(sys) = tool_detect::detect("composer", packages_root) {
        let phar_path = PathBuf::from(&sys.install_path).join("composer.phar");
        if phar_path.exists() {
            available.push(ComposerOption {
                version: sys.version,
                source: "system".into(),
                phar_path: phar_path.display().to_string(),
            });
        }
    }

    let tools_dir = packages_root.join("tools");
    if let Ok(entries) = std::fs::read_dir(&tools_dir) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if let Some(ver) = name.strip_prefix("composer-") {
                let phar = entry.path().join("composer.phar");
                if phar.exists() {
                    available.push(ComposerOption {
                        version: ver.to_string(),
                        source: "store".into(),
                        phar_path: phar.display().to_string(),
                    });
                }
            }
        }
    }

    if available.is_empty() {
        return None;
    }

    let active_version = detect_active_composer(&available);
    Some(ComposerToolInfo { active_version, available })
}

fn detect_active_composer(available: &[ComposerOption]) -> Option<String> {
    let shim = naxone_adapters::platform::global_php::bin_dir().join("composer.bat");
    if let Ok(content) = std::fs::read_to_string(&shim) {
        let content_norm = content.replace('/', "\\").to_lowercase();
        for opt in available {
            let phar_norm = opt.phar_path.replace('/', "\\").to_lowercase();
            if content_norm.contains(&phar_norm) {
                return Some(opt.version.clone());
            }
        }
    }
    // shim 不存在或不匹配 → 返回第一个可用版本
    available.first().map(|a| a.version.clone())
}

// ─── MySQL Commands ──────────────────────────────────────────────────────

/// 复用 PHP 的 UAC 一键清理逻辑：从 HKLM PATH 删除指定目录。
#[tauri::command]
pub async fn fix_mysql_path_conflicts(
    paths: Vec<String>,
    state: State<'_, AppState>,
) -> Result<DevToolsInfo, String> {
    let label = format!("一键清理系统 PATH 中的 MySQL 冲突 ({} 条)", paths.len());
    let result = do_fix_mysql_path_conflicts(&paths, &state).await;
    logged(&state, "tool", label, result).await
}

async fn do_fix_mysql_path_conflicts(
    paths: &[String],
    state: &AppState,
) -> Result<DevToolsInfo, String> {
    if paths.is_empty() {
        return Err("没有需要清理的路径".into());
    }
    let buf: Vec<PathBuf> = paths.iter().map(PathBuf::from).collect();
    naxone_adapters::platform::global_php::fix_masking_paths(&buf)?;

    // 清理后重读返回最新状态
    let config = state.config.read().await;
    let packages_root = resolve_packages_root(&config);
    drop(config);
    let services_snap = state.services.read().await.clone();
    Ok(DevToolsInfo {
        composer: build_composer_info(&packages_root),
        nvm: build_nvm_info(&packages_root),
        mysql: build_mysql_info(&packages_root, &services_snap),
    })
}

#[tauri::command]
pub async fn set_global_mysql(
    version: String,
    state: State<'_, AppState>,
) -> Result<DevToolsInfo, String> {
    let label = format!("设置全局 MySQL 为 v{}", version);
    let result = do_set_global_mysql(&version, &state).await;
    logged(&state, "tool", label, result).await
}

async fn do_set_global_mysql(version: &str, state: &AppState) -> Result<DevToolsInfo, String> {
    let config = state.config.read().await;
    let packages_root = resolve_packages_root(&config);
    drop(config);
    let services_snap = state.services.read().await.clone();

    let info = build_mysql_info(&packages_root, &services_snap)
        .ok_or_else(|| "未找到任何 MySQL 安装".to_string())?;
    let selected = info
        .available
        .iter()
        .find(|a| a.version == version)
        .ok_or_else(|| format!("未找到 MySQL v{}", version))?
        .clone();

    // 1) 把所有其他 MySQL bin 从用户 PATH 移除（保证只有一个全局）
    for opt in &info.available {
        if opt.version == version { continue; }
        let _ = naxone_adapters::platform::user_env::remove_from_user_path(&opt.bin_dir);
    }
    // 2) 追加目标 bin
    naxone_adapters::platform::user_env::append_to_user_path(&selected.bin_dir)
        .map_err(|e| format!("写用户 PATH 失败: {}", e))?;

    Ok(DevToolsInfo {
        composer: build_composer_info(&packages_root),
        nvm: build_nvm_info(&packages_root),
        mysql: build_mysql_info(&packages_root, &services_snap),
    })
}

/// 修改 MySQL root 密码。`current_password` 在 .naxone-root.txt 缺失
/// （如 PHPStudy MySQL）时由前端让用户输入；为 None 时回退读 .naxone-root.txt。
#[tauri::command]
pub async fn set_mysql_password(
    version: String,
    new_password: String,
    current_password: Option<String>,
    state: State<'_, AppState>,
) -> Result<DevToolsInfo, String> {
    let label = format!("修改 MySQL v{} root 密码", version);
    let result = do_set_mysql_password(&version, &new_password, current_password.as_deref(), &state).await;
    logged(&state, "tool", label, result).await
}

async fn do_set_mysql_password(
    version: &str,
    new_password: &str,
    current_password: Option<&str>,
    state: &AppState,
) -> Result<DevToolsInfo, String> {
    use naxone_adapters::package::tool_detect;

    if new_password.is_empty() {
        return Err("新密码不能为空".into());
    }

    let config = state.config.read().await;
    let packages_root = resolve_packages_root(&config);
    drop(config);
    let services_snap = state.services.read().await.clone();

    let info = build_mysql_info(&packages_root, &services_snap)
        .ok_or_else(|| "未找到任何 MySQL 安装".to_string())?;
    let selected = info
        .available
        .iter()
        .find(|a| a.version == version)
        .ok_or_else(|| format!("未找到 MySQL v{}", version))?
        .clone();

    let install = PathBuf::from(&selected.install_path);
    // 优先用前端传的当前密码；缺省时读 .naxone-root.txt
    let current = current_password
        .map(|s| s.to_string())
        .unwrap_or_else(|| tool_detect::read_mysql_root_password(&install));

    // 必须 mysqld 在跑（端口可连）才能改
    if !port_is_open("127.0.0.1", selected.port) {
        return Err(format!(
            "MySQL 服务未运行（127.0.0.1:{} 未监听），请先在仪表盘启动它再修改密码",
            selected.port
        ));
    }

    tool_detect::change_mysql_root_password(&install, selected.port, &current, new_password)
        .map_err(|e| format!("ALTER USER 失败: {}", e))?;
    tool_detect::write_mysql_root_password(&install, new_password)
        .map_err(|e| format!("写 .naxone-root.txt 失败: {}", e))?;

    Ok(DevToolsInfo {
        composer: build_composer_info(&packages_root),
        nvm: build_nvm_info(&packages_root),
        mysql: build_mysql_info(&packages_root, &services_snap),
    })
}

fn port_is_open(host: &str, port: u16) -> bool {
    use std::net::TcpStream;
    TcpStream::connect_timeout(
        &format!("{}:{}", host, port).parse().unwrap(),
        std::time::Duration::from_millis(500),
    )
    .is_ok()
}

fn build_mysql_info(
    packages_root: &Path,
    services: &[naxone_core::domain::service::ServiceInstance],
) -> Option<MysqlToolInfo> {
    use naxone_adapters::package::tool_detect;
    use naxone_core::domain::service::{ServiceKind, ServiceOrigin};
    use std::collections::HashSet;

    let mut available: Vec<MysqlOption> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();

    let combined_path = combined_paths();

    // 1) 商店扫描（store 目录下的 MySQL*）
    for m in tool_detect::list_installed_mysql(packages_root) {
        let install = PathBuf::from(&m.install_path);
        let key = normalize_install(&install);
        if !seen.insert(key) { continue; }
        let bin_dir = install.join("bin").display().to_string();
        let root_password = tool_detect::read_mysql_root_password(&install);
        available.push(MysqlOption {
            version: m.version,
            install_path: m.install_path,
            data_dir: m.data_dir,
            bin_dir,
            port: m.port,
            initialized: m.initialized,
            root_password,
            source: "store".into(),
        });
    }

    // 2) services 里所有 MySQL（PHPStudy / manual / system）合并进来
    for svc in services.iter().filter(|s| s.kind == ServiceKind::Mysql) {
        let key = normalize_install(&svc.install_path);
        if !seen.insert(key) { continue; }
        let install = svc.install_path.clone();
        let bin_dir = install.join("bin").display().to_string();
        let data_dir = install.join("data");
        let initialized = data_dir.join("mysql").exists();
        let root_password = tool_detect::read_mysql_root_password(&install);
        let source = match svc.origin {
            ServiceOrigin::Store => "store",
            ServiceOrigin::PhpStudy => "phpstudy",
            ServiceOrigin::Manual => "manual",
            ServiceOrigin::System => "system",
        };
        available.push(MysqlOption {
            version: svc.version.clone(),
            install_path: install.display().to_string(),
            data_dir: data_dir.display().to_string(),
            bin_dir,
            port: svc.port,
            initialized,
            root_password,
            source: source.into(),
        });
    }

    if available.is_empty() {
        return None;
    }

    // 活跃判定：用户 + 系统 PATH 合一
    let mut active_version: Option<String> = None;
    for opt in &available {
        if naxone_adapters::platform::user_env::path_contains(&combined_path, &opt.bin_dir) {
            active_version = Some(opt.version.clone());
            break;
        }
    }

    // 系统 PATH 冲突：HKLM 里所有含 mysqld.exe 的目录（除当前活跃版本本身）
    let conflicts = detect_mysql_system_conflicts(active_version.as_deref().and_then(|v| {
        available.iter().find(|o| o.version == *v).map(|o| o.bin_dir.as_str())
    }));

    Some(MysqlToolInfo { active_version, available, conflicts })
}

/// 规整 install_path：小写 + 反斜杠 + 去尾分隔符，作为 dedupe key
fn normalize_install(p: &Path) -> String {
    p.display()
        .to_string()
        .replace('/', "\\")
        .trim_end_matches('\\')
        .to_ascii_lowercase()
}

/// 扫 HKLM PATH 里所有"含 mysqld.exe 的目录"，排除等于 keep_bin 的那个。
/// 返回的目录将屏蔽用户 PATH 的 MySQL（系统 PATH 优先于用户 PATH）。
fn detect_mysql_system_conflicts(keep_bin: Option<&str>) -> Vec<String> {
    #[cfg(not(target_os = "windows"))]
    {
        let _ = keep_bin;
        return Vec::new();
    }
    #[cfg(target_os = "windows")]
    {
        let system_path = naxone_adapters::platform::user_env::read_system_path();
        let keep_norm = keep_bin.map(|s| {
            s.replace('/', "\\").trim_end_matches('\\').to_ascii_lowercase()
        });
        let mut out = Vec::new();
        for entry in system_path.split(';') {
            let trimmed = entry.trim();
            if trimmed.is_empty() { continue; }
            let p = PathBuf::from(trimmed);
            if !p.join("mysqld.exe").is_file() { continue; }
            let norm = trimmed.replace('/', "\\").trim_end_matches('\\').to_ascii_lowercase();
            if keep_norm.as_deref() == Some(norm.as_str()) { continue; }
            out.push(trimmed.to_string());
        }
        out
    }
}

/// 读用户 PATH + 系统 PATH 拼成一个 ;-分隔的串供 path_contains 查询。
fn combined_paths() -> String {
    #[cfg(target_os = "windows")]
    {
        let user = naxone_adapters::platform::user_env::read_user_path();
        let system = naxone_adapters::platform::user_env::read_system_path();
        return format!("{};{}", user, system);
    }
    #[cfg(not(target_os = "windows"))]
    String::new()
}

// ─── NVM ────────────────────────────────────────────────────────────────

fn build_nvm_info(packages_root: &Path) -> Option<NvmToolInfo> {
    use naxone_adapters::package::tool_detect;

    let nvm_home = tool_detect::get_nvm_home()?;
    let nvm_exe = nvm_home.join("nvm.exe");
    if !nvm_exe.exists() {
        return None;
    }

    let nvm_version = tool_detect::detect("nvm", packages_root)
        .map(|d| d.version)
        .unwrap_or_else(|| "?".into());

    let naxone_tools = packages_root.join("tools");
    let source = if nvm_home.starts_with(&naxone_tools) { "store" } else { "system" };

    let installed_nodes = tool_detect::list_node_versions(&nvm_home);
    let current_node = tool_detect::get_current_node_version();

    Some(NvmToolInfo {
        nvm_version,
        nvm_source: source.into(),
        nvm_home: nvm_home.display().to_string(),
        current_node,
        installed_nodes,
    })
}
