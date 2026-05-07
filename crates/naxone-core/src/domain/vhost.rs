use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SslConfig {
    pub cert_path: PathBuf,
    pub key_path: PathBuf,
    pub force_https: bool,
}

/// Where a vhost originated from. Used by the UI to group
/// PHPStudy-scanned vhosts separately from user-created ones.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase", tag = "kind", content = "name")]
pub enum VhostSource {
    /// Scanned from PHPStudy's Nginx vhosts directory
    PhpStudy,
    /// Created inside NaxOne (default)
    Custom,
    /// Discovered under a standalone install (the string is the package name, e.g. "nginx")
    Standalone(String),
}

impl Default for VhostSource {
    fn default() -> Self {
        Self::Custom
    }
}

/// A virtual host configuration (dual-write: both Nginx and Apache)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VirtualHost {
    pub id: String,
    pub server_name: String,
    pub aliases: Vec<String>,
    pub listen_port: u16,
    pub document_root: PathBuf,
    pub php_version: Option<String>,
    pub php_fastcgi_port: Option<u16>,
    pub php_install_path: Option<PathBuf>,
    #[serde(default = "default_index_files")]
    pub index_files: String,
    #[serde(default)]
    pub rewrite_rule: String,
    #[serde(default)]
    pub autoindex: bool,
    pub ssl: Option<SslConfig>,
    pub custom_directives: Option<String>,
    pub access_log: Option<String>,
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    /// ISO 8601 timestamp when this vhost was created
    #[serde(default)]
    pub created_at: String,
    /// Expiry date (ISO 8601), empty = never expires
    #[serde(default)]
    pub expires_at: String,
    /// Whether to sync hosts file
    #[serde(default = "default_enabled")]
    pub sync_hosts: bool,
    /// Where this vhost was found / created
    #[serde(default)]
    pub source: VhostSource,
}

fn default_enabled() -> bool {
    true
}

fn default_index_files() -> String {
    "index.php index.html".into()
}

impl VirtualHost {
    pub fn config_filename(&self) -> String {
        let safe = sanitize_filename(&self.server_name);
        format!("{}_{}.conf", safe, self.listen_port)
    }

    /// 校验此 vhost 的所有用户输入字段，注入安全检查。
    /// 失败返回错误描述（中文）。
    pub fn validate(&self) -> std::result::Result<(), String> {
        if !is_safe_hostname(&self.server_name) {
            return Err(format!("域名格式不合法：{}", self.server_name));
        }
        for a in &self.aliases {
            if a.is_empty() { continue; }
            if !is_safe_hostname(a) {
                return Err(format!("域名别名格式不合法：{}", a));
            }
        }
        if !is_safe_oneline_value(&self.index_files) {
            return Err("默认首页字段含非法字符".to_string());
        }
        if let Some(ref log) = self.access_log {
            if !is_safe_path_like(log) {
                return Err("日志路径含非法字符".to_string());
            }
        }
        if let Some(ref custom) = self.custom_directives {
            if !is_safe_custom_directives(custom) {
                return Err("自定义指令含不平衡的花括号或受限关键字".to_string());
            }
        }
        Ok(())
    }
}

/// 合法 hostname：ASCII 字母数字、点、连字符、下划线、星号（通配）。
/// 拒绝空、过长、连续点、空格、换行、控制字符、引号、分号、花括号等。
pub fn is_safe_hostname(s: &str) -> bool {
    if s.is_empty() || s.len() > 253 { return false; }
    if s.contains("..") { return false; }
    s.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_' | '*'))
}

/// 单行字段（如 index_files）：禁止换行/分号/引号/花括号，允许空格用于分隔。
pub fn is_safe_oneline_value(s: &str) -> bool {
    !s.chars().any(|c| matches!(c, '\n' | '\r' | ';' | '"' | '\'' | '{' | '}' | '<' | '>' | '\0'))
}

/// 路径类字段：允许字母数字、空格、ASCII 标点的常见子集，禁止换行/分号/引号/大括号。
pub fn is_safe_path_like(s: &str) -> bool {
    !s.chars().any(|c| matches!(c, '\n' | '\r' | ';' | '"' | '\'' | '{' | '}' | '<' | '>' | '\0'))
}

/// 自定义 nginx 指令：允许多行，但花括号必须配对，且不允许引入 server/http 等顶层块关键字。
pub fn is_safe_custom_directives(s: &str) -> bool {
    if s.contains('\0') { return false; }
    let opens = s.matches('{').count();
    let closes = s.matches('}').count();
    if opens != closes { return false; }
    let lc = s.to_ascii_lowercase();
    // 防止从 location 内部跳出去重写 server / http 顶层块
    if lc.contains("server {") || lc.contains("http {") || lc.contains("server\t{") || lc.contains("http\t{") {
        return false;
    }
    true
}

/// 文件名规范化：替换 / \ : * ? " < > | 等不能用作 Windows 文件名的字符为 _，
/// 同时避免 ".." 路径穿越。
pub fn sanitize_filename(s: &str) -> String {
    let cleaned: String = s.chars().map(|c| match c {
        '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' | '\0' => '_',
        c if c.is_control() => '_',
        c => c,
    }).collect();
    // 去掉前后点号和空格（Windows 不允许）+ 拆 ..
    cleaned.trim_matches(|c: char| c == '.' || c == ' ')
        .replace("..", "__")
}
