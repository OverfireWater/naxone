use serde::{Deserialize, Serialize};

/// A PHP extension (e.g., redis, gd, mbstring)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhpExtension {
    pub name: String,
    pub file_name: String,
    pub enabled: bool,
    pub is_zend: bool,
}

/// Common php.ini settings exposed in the GUI
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhpIniSettings {
    // Basic
    pub memory_limit: String,
    pub upload_max_filesize: String,
    pub post_max_size: String,
    pub max_execution_time: u32,
    pub max_input_time: i32,
    pub display_errors: bool,
    pub error_reporting: String,
    pub date_timezone: String,
    // Upload & Files
    pub file_uploads: bool,
    pub short_open_tag: bool,
    pub allow_url_fopen: bool,
    pub allow_url_include: bool,
    // Security
    pub disable_functions: String,
    pub expose_php: bool,
    pub open_basedir: String,
    // OPCache
    pub opcache_enable: bool,
    pub opcache_memory_consumption: u32,
    pub opcache_max_accelerated_files: u32,
    pub opcache_validate_timestamps: bool,
    pub opcache_revalidate_freq: u32,
    // Session
    pub session_save_handler: String,
    pub session_save_path: String,
    pub session_gc_maxlifetime: u32,
    pub session_cookie_lifetime: u32,
    pub session_name: String,
    pub session_use_cookies: bool,
    pub session_use_only_cookies: bool,
    pub session_use_strict_mode: bool,
    pub session_cookie_httponly: bool,
    pub session_cookie_samesite: String,
    // Extra
    pub output_buffering: String,
    pub default_charset: String,
    pub max_file_uploads: u32,
    pub default_socket_timeout: u32,
}

impl Default for PhpIniSettings {
    fn default() -> Self {
        Self {
            memory_limit: "256M".into(),
            upload_max_filesize: "64M".into(),
            post_max_size: "64M".into(),
            max_execution_time: 300,
            max_input_time: 60,
            display_errors: true,
            error_reporting: "E_ALL".into(),
            date_timezone: "Asia/Shanghai".into(),
            file_uploads: true,
            short_open_tag: true,
            allow_url_fopen: true,
            allow_url_include: false,
            disable_functions: String::new(),
            expose_php: true,
            open_basedir: String::new(),
            opcache_enable: false,
            opcache_memory_consumption: 128,
            opcache_max_accelerated_files: 10000,
            opcache_validate_timestamps: true,
            opcache_revalidate_freq: 2,
            session_save_handler: "files".into(),
            session_save_path: String::new(),
            session_gc_maxlifetime: 1440,
            session_cookie_lifetime: 0,
            session_name: "PHPSESSID".into(),
            session_use_cookies: true,
            session_use_only_cookies: true,
            session_use_strict_mode: false,
            session_cookie_httponly: false,
            session_cookie_samesite: String::new(),
            output_buffering: "8192".into(),
            default_charset: "UTF-8".into(),
            max_file_uploads: 20,
            default_socket_timeout: 60,
        }
    }
}
