use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;

use ruststudy_adapters::config::fs_config::FsConfigIO;
use ruststudy_adapters::package::composite::CompositeScanner;
use ruststudy_adapters::platform::windows::WindowsPlatform;
use ruststudy_adapters::process::NativeProcessManager;
use ruststudy_adapters::template::SimpleTemplateEngine;
use ruststudy_adapters::vhost::VhostScanner;
use ruststudy_core::config::AppConfig;
use ruststudy_core::domain::service::ServiceInstance;
use ruststudy_core::domain::log::LogEntry;
use ruststudy_core::domain::vhost::VirtualHost;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicU64};
use ruststudy_core::ports::config_io::ConfigIO;
use ruststudy_core::ports::process::ProcessManager;
use ruststudy_core::ports::template::TemplateEngine;
use ruststudy_core::ports::platform::PlatformOps;
use ruststudy_core::use_cases::config_editor::ConfigEditor;
use ruststudy_core::use_cases::php_mgr::PhpManager;
use ruststudy_core::use_cases::service_mgr::ServiceManager;
use ruststudy_core::use_cases::vhost_mgr::VhostManager;

pub struct AppState {
    pub services: Arc<RwLock<Vec<ServiceInstance>>>,
    pub service_manager: ServiceManager,
    pub vhost_manager: VhostManager,
    pub php_manager: PhpManager,
    pub config_editor: ConfigEditor,
    pub vhosts: Arc<RwLock<Vec<VirtualHost>>>,
    pub config: Arc<RwLock<AppConfig>>,
    pub startup_errors: Arc<RwLock<Vec<String>>>,
    pub logs: Arc<RwLock<VecDeque<LogEntry>>>,
    pub log_id_counter: Arc<AtomicU64>,
    pub log_writer_tx: Arc<tokio::sync::Mutex<Option<tokio::sync::mpsc::UnboundedSender<LogEntry>>>>,
    /// 后台 status 刷新的 single-flight 标志：已有刷新在跑时跳过新触发
    pub refresh_in_flight: Arc<AtomicBool>,
    /// 平台相关操作（hosts 文件、防火墙），命令层也要直接用
    pub platform_ops: Arc<dyn PlatformOps>,
}

impl AppState {
    /// Create a shallow clone (Arc clones) suitable for moving into async tasks
    pub fn clone_shallow(&self) -> Self {
        Self {
            services: self.services.clone(),
            service_manager: self.service_manager.clone(),
            vhost_manager: self.vhost_manager.clone(),
            php_manager: self.php_manager.clone(),
            config_editor: self.config_editor.clone(),
            vhosts: self.vhosts.clone(),
            config: self.config.clone(),
            startup_errors: self.startup_errors.clone(),
            logs: self.logs.clone(),
            log_id_counter: self.log_id_counter.clone(),
            log_writer_tx: self.log_writer_tx.clone(),
            refresh_in_flight: self.refresh_in_flight.clone(),
            platform_ops: self.platform_ops.clone(),
        }
    }

    pub fn new() -> Self {
        let config_io = Arc::new(FsConfigIO) as Arc<dyn ConfigIO>;
        let template_engine = Arc::new(SimpleTemplateEngine) as Arc<dyn TemplateEngine>;
        let platform_ops = Arc::new(WindowsPlatform) as Arc<dyn PlatformOps>;
        let process_mgr = Arc::new(NativeProcessManager::new()) as Arc<dyn ProcessManager>;

        let service_manager = ServiceManager::new(process_mgr.clone());
        let vhost_manager = VhostManager::new(
            config_io.clone(),
            template_engine,
            platform_ops.clone(),
            process_mgr,
        );
        let php_manager = PhpManager::new(config_io.clone());
        let config_editor = ConfigEditor::new(config_io.clone());

        // Try to load config or create default
        let cfg_path = config_path();
        let mut config = if cfg_path.exists() {
            AppConfig::load(&cfg_path).unwrap_or_else(|_| default_config())
        } else {
            let cfg = default_config();
            let _ = cfg.save(&cfg_path);
            cfg
        };

        let resolved_www_root = resolve_default_www_root();
        // 自动迁移：legacy 默认值；以及 dev 模式下写入但当前已切到 release 的脏值
        // （路径落在 cargo 的 target\debug 或 target\release 下，绝大多数情况都是
        // 上一次 cargo tauri dev 留下的，正式安装版应当指向自己同级的 www）
        let needs_migrate = config.general.www_root == legacy_default_www_root()
            || is_cargo_target_path(&config.general.www_root);
        if needs_migrate {
            tracing::info!(
                old = %config.general.www_root.display(),
                new = %resolved_www_root.display(),
                "迁移过期的 www_root（legacy 或 cargo target 路径）",
            );
            config.general.www_root = resolved_www_root.clone();
            let _ = config.save(&cfg_path);
        }

        // 保持 RustStudy 自己管理默认站点目录，不再自动改回 PHPStudy 的 WWW。

        // 确保 www_root 目录存在（新建站点时默认指向它，空目录也无妨）
        let _ = std::fs::create_dir_all(&config.general.www_root);

        // Scan for installed services from all sources.
        let ext_path = config
            .general
            .phpstudy_path
            .as_ref()
            .map(|p| p.join("Extensions"));
        let store_ext = resolve_packages_root(&config);
        let legacy_root = legacy_packages_root();
        tracing::info!(
            store_extensions = ?store_ext,
            "Resolved store extensions root",
        );
        let services = CompositeScanner::scan(
            ext_path.as_deref(),
            Some(&store_ext),
            &config.general.extra_install_paths,
            Some(&legacy_root),
        );
        tracing::info!(
            service_count = services.len(),
            phpstudy_ext = ?ext_path,
            store_ext = ?store_ext,
            "Initial scan completed",
        );

        // Load saved vhost metadata + scan .conf files, then merge
        let vhosts_json_path = vhosts_json_path();
        let saved_vhosts = VhostManager::load_vhosts_json(&vhosts_json_path);
        let scanned_vhosts = if let Some(phpstudy_path) = &config.general.phpstudy_path {
            let ext_path = phpstudy_path.join("Extensions");
            let nginx_vhosts_dir = find_extension_dir(&ext_path, "Nginx")
                .map(|d| d.join("conf").join("vhosts"));
            if let Some(dir) = nginx_vhosts_dir {
                VhostScanner::scan(config_io.as_ref(), &dir, &services).unwrap_or_default()
            } else {
                Vec::new()
            }
        } else {
            Vec::new()
        };
        let vhosts = VhostManager::merge_vhosts(scanned_vhosts, saved_vhosts);

        Self {
            services: Arc::new(RwLock::new(services)),
            service_manager,
            vhost_manager,
            php_manager,
            config_editor,
            vhosts: Arc::new(RwLock::new(vhosts)),
            config: Arc::new(RwLock::new(config)),
            startup_errors: Arc::new(RwLock::new(Vec::new())),
            logs: Arc::new(RwLock::new(VecDeque::with_capacity(1000))),
            log_id_counter: Arc::new(AtomicU64::new(0)),
            log_writer_tx: Arc::new(tokio::sync::Mutex::new(None)),
            refresh_in_flight: Arc::new(AtomicBool::new(false)),
            platform_ops,
        }
    }
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

/// Resolve the store install root. We model this after PHPStudy's layout —
/// the directory ends up looking like a PHPStudy `Extensions/`, so users
/// can read it with familiar intuition.
///
/// Resolution order:
///   1. `config.general.package_install_root` if the user set one explicitly.
///   2. `{exe_dir}/Extensions/` if the exe's folder is writable (portable /
///      dev-time case).
///   3. `%APPDATA%/RustStudy/Extensions/` fallback (Program Files install).
pub fn resolve_packages_root(config: &AppConfig) -> PathBuf {
    if let Some(custom) = &config.general.package_install_root {
        if !custom.as_os_str().is_empty() {
            return custom.clone();
        }
    }

    if let Some(exe_dir) = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.to_path_buf()))
    {
        if is_writable_dir(&exe_dir) {
            return exe_dir.join("Extensions");
        }
    }

    let appdata = std::env::var("APPDATA")
        .ok()
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            let home = std::env::var("USERPROFILE").unwrap_or_else(|_| "C:\\Users\\Default".into());
            PathBuf::from(home).join("AppData").join("Roaming")
        });
    appdata.join("RustStudy").join("Extensions")
}

/// 新用户 / 迁移时使用的默认 WWW 根目录。
/// 策略同 packages_root：
///   1. exe 同级可写 → `{exe_dir}/www/`（便携模式、开发模式）
///   2. 不可写（Program Files）→ `%APPDATA%/RustStudy/www/`
pub fn resolve_default_www_root() -> PathBuf {
    if let Some(exe_dir) = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.to_path_buf()))
    {
        if is_writable_dir(&exe_dir) {
            return exe_dir.join("www");
        }
    }
    let appdata = std::env::var("APPDATA")
        .ok()
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            let home = std::env::var("USERPROFILE").unwrap_or_else(|_| "C:\\Users\\Default".into());
            PathBuf::from(home).join("AppData").join("Roaming")
        });
    appdata.join("RustStudy").join("www")
}

/// Legacy root from the first store prototype (`%APPDATA%/RustStudy/Packages/`).
/// Returned so the scanner can still pick up packages installed before the
/// PHPStudy-style refactor.
pub fn legacy_packages_root() -> PathBuf {
    let appdata = std::env::var("APPDATA")
        .ok()
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            let home = std::env::var("USERPROFILE").unwrap_or_else(|_| "C:\\Users\\Default".into());
            PathBuf::from(home).join("AppData").join("Roaming")
        });
    appdata.join("RustStudy").join("Packages")
}

/// Probe: can we create a file in this directory? Cheap, avoids relying on
/// metadata flags that lie on Windows.
fn is_writable_dir(dir: &std::path::Path) -> bool {
    if !dir.exists() {
        return std::fs::create_dir_all(dir).is_ok();
    }
    let probe = dir.join(format!(
        ".ruststudy-write-probe-{}.tmp",
        std::process::id()
    ));
    match std::fs::write(&probe, b"") {
        Ok(_) => {
            let _ = std::fs::remove_file(&probe);
            true
        }
        Err(_) => false,
    }
}

/// Resolve the log directory: custom from config, or default next to exe
pub fn resolve_log_dir(config: &AppConfig) -> PathBuf {
    if let Some(custom) = &config.general.log_dir {
        if !custom.as_os_str().is_empty() {
            return custom.clone();
        }
    }
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.to_path_buf()))
        .unwrap_or_else(|| PathBuf::from("."))
        .join("logs")
}

/// 判断路径是不是落在 cargo 编译产物目录下（`...\target\debug\...` 或
/// `...\target\release\...`）。用于识别 dev 时写下、正式版启动后已无意义的脏路径。
fn is_cargo_target_path(path: &std::path::Path) -> bool {
    let s = path.to_string_lossy().replace('/', "\\").to_lowercase();
    s.contains("\\target\\debug\\") || s.contains("\\target\\release\\")
}

fn legacy_default_www_root() -> PathBuf {
    let home = std::env::var("USERPROFILE").unwrap_or_else(|_| "C:\\Users\\Default".into());
    PathBuf::from(home).join(".ruststudy").join("www")
}

pub fn vhosts_json_path() -> PathBuf {
    let home = std::env::var("USERPROFILE").unwrap_or_else(|_| "C:\\Users\\Default".into());
    PathBuf::from(home).join(".ruststudy").join("vhosts.json")
}

pub fn config_path() -> PathBuf {
    let home = std::env::var("USERPROFILE").unwrap_or_else(|_| "C:\\Users\\Default".into());
    PathBuf::from(home)
        .join(".ruststudy")
        .join("ruststudy.toml")
}

fn default_config() -> AppConfig {
    let www_root = resolve_default_www_root();
    let phpstudy_path = PathBuf::from(r"D:\phpstudy_pro");
    if phpstudy_path.exists() {
        AppConfig::default_with_phpstudy(phpstudy_path, www_root)
    } else {
        let home = std::env::var("USERPROFILE").unwrap_or_else(|_| "C:\\Users\\Default".into());
        AppConfig::default_with_phpstudy(PathBuf::from(home).join(".ruststudy"), www_root)
    }
}
