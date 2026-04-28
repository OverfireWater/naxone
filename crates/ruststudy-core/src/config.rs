use crate::domain::service::ServiceKind;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Main application configuration (ruststudy.toml)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub general: GeneralConfig,
    #[serde(default)]
    pub web_server: WebServerConfig,
    #[serde(default)]
    pub mysql: MysqlConfig,
    #[serde(default)]
    pub redis: RedisConfig,
    #[serde(default)]
    pub php_instances: HashMap<String, PhpInstanceConfig>,
}

/// A user-specified extra install path for a standalone (non-PHPStudy) service.
/// E.g. the user has Nginx installed somewhere outside PHPStudy and wants
/// RustStudy to manage it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtraInstallPath {
    /// Stable id for frontend remove operations
    pub id: String,
    pub kind: ServiceKind,
    pub path: PathBuf,
    /// Optional user-friendly label
    #[serde(default)]
    pub label: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneralConfig {
    pub data_dir: PathBuf,
    pub www_root: PathBuf,
    #[serde(default)]
    pub phpstudy_path: Option<PathBuf>,
    #[serde(default = "default_auto_start")]
    pub auto_start: Vec<String>,
    #[serde(default)]
    pub log_dir: Option<PathBuf>,
    #[serde(default = "default_log_retention")]
    pub log_retention_days: u32,
    /// Paths to manually-added standalone installs
    #[serde(default)]
    pub extra_install_paths: Vec<ExtraInstallPath>,
    /// Root dir where packages installed via the in-app store go.
    /// Default (None) resolves to %APPDATA%/RustStudy/Packages/.
    #[serde(default)]
    pub package_install_root: Option<PathBuf>,
    /// CLI 层全局 PHP 版本。None 表示未设置；设置后在
    /// %USERPROFILE%\.ruststudy\bin\php.cmd 里把 php 命令指向这个版本。
    /// 不影响 vhost 绑定的 PHP 版本，只影响用户命令行里敲 `php -v`。
    #[serde(default)]
    pub global_php_version: Option<String>,
    /// 托盘“退出”时是否先停止全部服务。
    #[serde(default)]
    pub stop_services_on_exit: bool,
}

fn default_auto_start() -> Vec<String> {
    vec![]
}

fn default_log_retention() -> u32 {
    7
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebServerConfig {
    #[serde(default = "default_web_server")]
    pub active: String,
    pub nginx_version: Option<String>,
    pub apache_version: Option<String>,
}

fn default_web_server() -> String {
    "nginx".into()
}

impl Default for WebServerConfig {
    fn default() -> Self {
        Self {
            active: "nginx".into(),
            nginx_version: None,
            apache_version: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MysqlConfig {
    pub version: Option<String>,
    #[serde(default = "default_mysql_port")]
    pub port: u16,
}

fn default_mysql_port() -> u16 {
    3306
}

impl Default for MysqlConfig {
    fn default() -> Self {
        Self {
            version: None,
            port: 3306,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedisConfig {
    pub version: Option<String>,
    #[serde(default = "default_redis_port")]
    pub port: u16,
}

fn default_redis_port() -> u16 {
    6379
}

impl Default for RedisConfig {
    fn default() -> Self {
        Self {
            version: None,
            port: 6379,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhpInstanceConfig {
    pub port: u16,
    #[serde(default = "default_workers")]
    pub workers: u16,
    #[serde(default)]
    pub auto_start: bool,
}

fn default_workers() -> u16 {
    16
}

impl AppConfig {
    /// Load config from a TOML file
    pub fn load(path: &Path) -> crate::error::Result<Self> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| crate::error::RustStudyError::Config(format!("Failed to read config: {e}")))?;
        let config: Self = toml::from_str(&content)
            .map_err(|e| crate::error::RustStudyError::Config(format!("Failed to parse config: {e}")))?;
        Ok(config)
    }

    /// Save config to a TOML file
    pub fn save(&self, path: &Path) -> crate::error::Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let content = toml::to_string_pretty(self)
            .map_err(|e| crate::error::RustStudyError::Config(format!("Failed to serialize config: {e}")))?;
        std::fs::write(path, content)?;
        Ok(())
    }

    /// Create a default config for a fresh PHPStudy-compatible setup.
    /// `www_root` 由调用方传入（tauri 层计算：优先 exe 同级便携目录，
    /// 不可写时 fallback %APPDATA%）。core 不知道 exe 路径，所以不自己决定。
    pub fn default_with_phpstudy(phpstudy_path: PathBuf, www_root: PathBuf) -> Self {
        Self {
            general: GeneralConfig {
                data_dir: phpstudy_path.clone(),
                www_root,
                phpstudy_path: Some(phpstudy_path),
                auto_start: vec!["nginx".into(), "mysql".into()],
                log_dir: None,
                log_retention_days: 7,
                extra_install_paths: Vec::new(),
                package_install_root: None,
                global_php_version: None,
                stop_services_on_exit: false,
            },
            web_server: WebServerConfig::default(),
            mysql: MysqlConfig::default(),
            redis: RedisConfig::default(),
            php_instances: HashMap::new(),
        }
    }
}
