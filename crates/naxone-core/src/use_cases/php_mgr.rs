use std::path::Path;
use std::sync::Arc;

use crate::domain::php::{PhpExtension, PhpIniSettings};
use crate::error::Result;
use crate::ports::config_io::ConfigIO;

#[derive(Clone)]
pub struct PhpManager {
    config_io: Arc<dyn ConfigIO>,
}

impl PhpManager {
    pub fn new(config_io: Arc<dyn ConfigIO>) -> Self {
        Self { config_io }
    }

    /// List all PHP extensions for a given PHP install, comparing ext/ dir with php.ini
    pub fn list_extensions(&self, php_install_path: &Path) -> Result<Vec<PhpExtension>> {
        let ini_path = php_install_path.join("php.ini");
        let ini_content = self.config_io.read_text(&ini_path)?;

        // Scan ext/ directory for available .dll files
        let ext_dir = php_install_path.join("ext");
        let dll_files = self.config_io.list_files(&ext_dir, "dll")?;

        let mut extensions = Vec::new();

        for dll_path in &dll_files {
            let file_name = dll_path
                .file_stem()
                .and_then(|n| n.to_str())
                .unwrap_or_default()
                .to_string();

            // Skip non-extension DLLs
            if !file_name.starts_with("php_") && file_name != "opcache" && file_name != "xdebug" {
                continue;
            }

            let display_name = file_name
                .strip_prefix("php_")
                .unwrap_or(&file_name)
                .to_string();

            let is_zend =
                display_name == "opcache" || display_name == "xdebug";

            let enabled = Self::is_extension_enabled(&ini_content, &display_name, is_zend);

            extensions.push(PhpExtension {
                name: display_name,
                file_name: file_name.clone(),
                enabled,
                is_zend,
            });
        }

        // Sort alphabetically only (don't sort by enabled to avoid list jumping on toggle)
        extensions.sort_by(|a, b| a.name.cmp(&b.name));

        Ok(extensions)
    }

    /// Toggle a PHP extension on/off by modifying php.ini
    pub fn toggle_extension(
        &self,
        php_install_path: &Path,
        ext_name: &str,
        enable: bool,
        is_zend: bool,
    ) -> Result<()> {
        let ini_path = php_install_path.join("php.ini");

        // Backup first
        self.config_io.backup(&ini_path)?;

        let content = self.config_io.read_text(&ini_path)?;
        let directive = if is_zend { "zend_extension" } else { "extension" };

        // Possible line patterns to match
        let patterns = vec![
            format!("{}={}", directive, ext_name),
            format!("{}=php_{}", directive, ext_name),
            format!("{} = {}", directive, ext_name),
            format!("{} = php_{}", directive, ext_name),
        ];

        let mut lines: Vec<String> = content.lines().map(|l| l.to_string()).collect();
        let mut found = false;

        for line in lines.iter_mut() {
            let trimmed = line.trim();
            let uncommented = if trimmed.starts_with(';') {
                trimmed[1..].trim()
            } else {
                trimmed
            };

            if patterns.iter().any(|p| uncommented == p || uncommented.starts_with(&format!("{} ", p))) {
                found = true;
                if enable {
                    // Remove leading semicolons
                    *line = line.trim_start_matches(';').to_string();
                } else {
                    // Add semicolon if not already commented
                    if !line.trim().starts_with(';') {
                        *line = format!(";{}", line);
                    }
                }
                break;
            }
        }

        // If not found and enabling, add a new line
        if !found && enable {
            // Find the extension block to insert near
            let insert_pos = lines
                .iter()
                .rposition(|l| {
                    let t = l.trim().trim_start_matches(';').trim();
                    t.starts_with("extension=") || t.starts_with("zend_extension=")
                })
                .map(|p| p + 1)
                .unwrap_or(lines.len());

            lines.insert(insert_pos, format!("{}={}", directive, ext_name));
        }

        let new_content = lines.join("\n");
        self.config_io.write_text(&ini_path, &new_content)
    }

    /// Read common php.ini settings
    pub fn read_ini_settings(&self, php_install_path: &Path) -> Result<PhpIniSettings> {
        let ini_path = php_install_path.join("php.ini");
        let content = self.config_io.read_text(&ini_path)?;

        let bool_val = |key: &str, def: bool| -> bool {
            Self::get_ini_value(&content, key)
                .map(|v| v == "On" || v == "on" || v == "1")
                .unwrap_or(def)
        };
        let str_val = |key: &str, def: &str| -> String {
            Self::get_ini_value(&content, key).unwrap_or_else(|| def.into())
        };
        let u32_val = |key: &str, def: u32| -> u32 {
            Self::get_ini_value(&content, key).and_then(|v| v.parse().ok()).unwrap_or(def)
        };

        Ok(PhpIniSettings {
            memory_limit: str_val("memory_limit", "256M"),
            upload_max_filesize: str_val("upload_max_filesize", "64M"),
            post_max_size: str_val("post_max_size", "64M"),
            max_execution_time: u32_val("max_execution_time", 300),
            max_input_time: Self::get_ini_value(&content, "max_input_time").and_then(|v| v.parse().ok()).unwrap_or(60),
            display_errors: bool_val("display_errors", true),
            error_reporting: str_val("error_reporting", "E_ALL"),
            date_timezone: str_val("date.timezone", "Asia/Shanghai"),
            file_uploads: bool_val("file_uploads", true),
            short_open_tag: bool_val("short_open_tag", true),
            allow_url_fopen: bool_val("allow_url_fopen", true),
            allow_url_include: bool_val("allow_url_include", false),
            disable_functions: str_val("disable_functions", ""),
            expose_php: bool_val("expose_php", true),
            open_basedir: str_val("open_basedir", ""),
            opcache_enable: bool_val("opcache.enable", false),
            opcache_memory_consumption: u32_val("opcache.memory_consumption", 128),
            opcache_max_accelerated_files: u32_val("opcache.max_accelerated_files", 10000),
            opcache_validate_timestamps: bool_val("opcache.validate_timestamps", true),
            opcache_revalidate_freq: u32_val("opcache.revalidate_freq", 2),
            session_save_handler: str_val("session.save_handler", "files"),
            session_save_path: str_val("session.save_path", ""),
            session_gc_maxlifetime: u32_val("session.gc_maxlifetime", 1440),
            session_cookie_lifetime: u32_val("session.cookie_lifetime", 0),
            session_name: str_val("session.name", "PHPSESSID"),
            session_use_cookies: bool_val("session.use_cookies", true),
            session_use_only_cookies: bool_val("session.use_only_cookies", true),
            session_use_strict_mode: bool_val("session.use_strict_mode", false),
            session_cookie_httponly: bool_val("session.cookie_httponly", false),
            session_cookie_samesite: str_val("session.cookie_samesite", ""),
            output_buffering: str_val("output_buffering", "8192"),
            default_charset: str_val("default_charset", "UTF-8"),
            max_file_uploads: u32_val("max_file_uploads", 20),
            default_socket_timeout: u32_val("default_socket_timeout", 60),
        })
    }

    /// Save common php.ini settings
    pub fn save_ini_settings(
        &self,
        php_install_path: &Path,
        settings: &PhpIniSettings,
    ) -> Result<()> {
        let ini_path = php_install_path.join("php.ini");
        self.config_io.backup(&ini_path)?;

        let content = self.config_io.read_text(&ini_path)?;
        let mut result = content.clone();

        let set = |r: String, k: &str, v: &str| -> String { Self::set_ini_value(&r, k, v) };
        let set_bool = |r: String, k: &str, v: bool| -> String { Self::set_ini_value(&r, k, if v { "On" } else { "Off" }) };

        // Basic
        result = set(result, "memory_limit", &settings.memory_limit);
        result = set(result, "upload_max_filesize", &settings.upload_max_filesize);
        result = set(result, "post_max_size", &settings.post_max_size);
        result = set(result, "max_execution_time", &settings.max_execution_time.to_string());
        result = set(result, "max_input_time", &settings.max_input_time.to_string());
        result = set_bool(result, "display_errors", settings.display_errors);
        result = set(result, "error_reporting", &settings.error_reporting);
        result = set(result, "date.timezone", &settings.date_timezone);
        // Upload & Files
        result = set_bool(result, "file_uploads", settings.file_uploads);
        result = set_bool(result, "short_open_tag", settings.short_open_tag);
        result = set_bool(result, "allow_url_fopen", settings.allow_url_fopen);
        result = set_bool(result, "allow_url_include", settings.allow_url_include);
        // Security
        result = set(result, "disable_functions", &settings.disable_functions);
        result = set_bool(result, "expose_php", settings.expose_php);
        if !settings.open_basedir.is_empty() {
            result = set(result, "open_basedir", &settings.open_basedir);
        }
        // OPCache
        result = set(result, "opcache.enable", if settings.opcache_enable { "1" } else { "0" });
        result = set(result, "opcache.memory_consumption", &settings.opcache_memory_consumption.to_string());
        result = set(result, "opcache.max_accelerated_files", &settings.opcache_max_accelerated_files.to_string());
        result = set(result, "opcache.validate_timestamps", if settings.opcache_validate_timestamps { "1" } else { "0" });
        result = set(result, "opcache.revalidate_freq", &settings.opcache_revalidate_freq.to_string());
        // Session
        result = set(result, "session.save_handler", &settings.session_save_handler);
        if !settings.session_save_path.is_empty() {
            result = set(result, "session.save_path", &settings.session_save_path);
        }
        result = set(result, "session.gc_maxlifetime", &settings.session_gc_maxlifetime.to_string());
        result = set(result, "session.cookie_lifetime", &settings.session_cookie_lifetime.to_string());
        result = set(result, "session.name", &settings.session_name);
        result = set_bool(result, "session.use_cookies", settings.session_use_cookies);
        result = set_bool(result, "session.use_only_cookies", settings.session_use_only_cookies);
        result = set(result, "session.use_strict_mode", if settings.session_use_strict_mode { "1" } else { "0" });
        result = set_bool(result, "session.cookie_httponly", settings.session_cookie_httponly);
        if !settings.session_cookie_samesite.is_empty() {
            result = set(result, "session.cookie_samesite", &settings.session_cookie_samesite);
        }
        // Extra
        result = set(result, "output_buffering", &settings.output_buffering);
        result = set(result, "default_charset", &format!("\"{}\"", settings.default_charset));
        result = set(result, "max_file_uploads", &settings.max_file_uploads.to_string());
        result = set(result, "default_socket_timeout", &settings.default_socket_timeout.to_string());

        self.config_io.write_text(&ini_path, &result)
    }

    // --- Helpers ---

    fn is_extension_enabled(ini_content: &str, ext_name: &str, is_zend: bool) -> bool {
        let directive = if is_zend { "zend_extension" } else { "extension" };
        for line in ini_content.lines() {
            let trimmed = line.trim();
            // Skip commented lines
            if trimmed.starts_with(';') {
                continue;
            }
            // Match extension=name or extension=php_name
            if trimmed.starts_with(directive) {
                let value = trimmed
                    .splitn(2, '=')
                    .nth(1)
                    .map(|v| v.trim())
                    .unwrap_or_default();
                let value_clean = value
                    .trim_matches('"')
                    .rsplit(['/', '\\'])
                    .next()
                    .unwrap_or(value)
                    .strip_suffix(".dll")
                    .unwrap_or(value)
                    .strip_prefix("php_")
                    .unwrap_or(value);
                if value_clean == ext_name
                    || value.trim_matches('"') == ext_name
                    || value.trim_matches('"') == format!("php_{}", ext_name)
                {
                    return true;
                }
            }
        }
        false
    }

    fn get_ini_value(content: &str, key: &str) -> Option<String> {
        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with(';') || trimmed.is_empty() {
                continue;
            }
            if let Some((k, v)) = trimmed.split_once('=') {
                if k.trim() == key {
                    return Some(v.trim().to_string());
                }
            }
        }
        None
    }

    fn set_ini_value(content: &str, key: &str, value: &str) -> String {
        let mut lines: Vec<String> = content.lines().map(|l| l.to_string()).collect();
        let mut found = false;

        for line in lines.iter_mut() {
            let trimmed = line.trim();
            // Match both active and commented lines
            let check = if trimmed.starts_with(';') {
                trimmed[1..].trim()
            } else {
                trimmed
            };

            if let Some((k, _)) = check.split_once('=') {
                if k.trim() == key {
                    *line = format!("{}={}", key, value);
                    found = true;
                    break;
                }
            }
        }

        if !found {
            lines.push(format!("{}={}", key, value));
        }

        lines.join("\n")
    }
}
