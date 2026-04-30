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
        format!("{}_{}.conf", self.server_name, self.listen_port)
    }
}
