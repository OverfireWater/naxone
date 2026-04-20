use std::path::Path;

use ruststudy_core::domain::service::{ServiceInstance, ServiceKind, ServiceOrigin, ServiceStatus};
use ruststudy_core::error::Result;

/// Scans a PHPStudy-compatible Extensions directory to discover installed packages
pub struct PhpStudyScanner;

impl PhpStudyScanner {
    /// Scan the PHPStudy Extensions directory and return discovered service instances
    pub fn scan(extensions_path: &Path) -> Result<Vec<ServiceInstance>> {
        let mut instances = Vec::new();

        if !extensions_path.exists() {
            return Ok(instances);
        }

        // Scan for PHP versions: Extensions/php/php85nts/, php74nts/, etc.
        let php_dir = extensions_path.join("php");
        if php_dir.exists() {
            if let Ok(entries) = std::fs::read_dir(&php_dir) {
                let mut port = 9000u16;
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.is_dir() && path.join("php-cgi.exe").exists() {
                        if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                            let (version, variant) = parse_php_dir_name(name);
                            let config_path = resolve_config(&path, &["php.ini"]);
                            instances.push(ServiceInstance {
                                kind: ServiceKind::Php,
                                version,
                                variant: Some(variant),
                                install_path: path,
                                config_path,
                                port,
                                status: ServiceStatus::Stopped,
                                auto_start: false,
                                origin: ServiceOrigin::PhpStudy,
                            });
                            port += 1;
                        }
                    }
                }
            }
        }

        // Scan for Nginx: Extensions/Nginx*/
        scan_service_dirs(
            extensions_path,
            "Nginx",
            "nginx.exe",
            ServiceKind::Nginx,
            80,
            &["conf/nginx.conf"],
            &mut instances,
        );

        // Scan for Apache: Extensions/Apache*/
        scan_service_dirs(
            extensions_path,
            "Apache",
            "bin/httpd.exe",
            ServiceKind::Apache,
            80,
            &["conf/httpd.conf"],
            &mut instances,
        );

        // Scan for MySQL: Extensions/MySQL*/
        scan_service_dirs(
            extensions_path,
            "MySQL",
            "bin/mysqld.exe",
            ServiceKind::Mysql,
            3306,
            &["my.ini", "my.cnf"],
            &mut instances,
        );

        // Scan for Redis: Extensions/redis*/
        scan_service_dirs(
            extensions_path,
            "redis",
            "redis-server.exe",
            ServiceKind::Redis,
            6379,
            &["redis.windows.conf", "redis.conf"],
            &mut instances,
        );

        tracing::info!(
            count = instances.len(),
            "Scanned PHPStudy Extensions directory"
        );

        Ok(instances)
    }
}

fn scan_service_dirs(
    extensions_path: &Path,
    prefix: &str,
    exe_relative: &str,
    kind: ServiceKind,
    default_port: u16,
    config_candidates: &[&str],
    instances: &mut Vec<ServiceInstance>,
) {
    if let Ok(entries) = std::fs::read_dir(extensions_path) {
        let prefix_lower = prefix.to_lowercase();
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                let name = path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or_default();
                if name.to_lowercase().starts_with(&prefix_lower) && path.join(exe_relative).exists()
                {
                    let version = extract_version(name, prefix);
                    let config_path = resolve_config(&path, config_candidates);
                    instances.push(ServiceInstance {
                        kind,
                        version,
                        variant: None,
                        install_path: path,
                        config_path,
                        port: default_port,
                        status: ServiceStatus::Stopped,
                        auto_start: false,
                        origin: ServiceOrigin::PhpStudy,
                    });
                }
            }
        }
    }
}

/// Try to find a config file from a list of candidates
fn resolve_config(base: &Path, candidates: &[&str]) -> Option<std::path::PathBuf> {
    for candidate in candidates {
        let p = base.join(candidate);
        if p.exists() {
            return Some(p);
        }
    }
    None
}

/// Parse PHP directory name like "php85nts" -> ("8.5", "nts")
fn parse_php_dir_name(name: &str) -> (String, String) {
    let name = name.strip_prefix("php").unwrap_or(name);

    let (digits, variant) = if let Some(pos) = name.find(|c: char| !c.is_ascii_digit() && c != '.') {
        (&name[..pos], name[pos..].to_string())
    } else {
        (name, "nts".to_string())
    };

    let version = if digits.contains('.') {
        digits.to_string()
    } else if digits.len() >= 2 {
        // "85" -> "8.5", "74" -> "7.4", "734" -> "7.3.4"
        let chars: Vec<char> = digits.chars().collect();
        if chars.len() == 2 {
            format!("{}.{}", chars[0], chars[1])
        } else {
            format!("{}.{}.{}", chars[0], chars[1], &digits[2..])
        }
    } else {
        digits.to_string()
    };

    (version, variant)
}

/// Extract version from directory name like "Nginx1.15.11" -> "1.15.11"
fn extract_version(name: &str, prefix: &str) -> String {
    let after = if name.len() > prefix.len() {
        &name[prefix.len()..]
    } else {
        ""
    };
    after.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_php_dir_name() {
        assert_eq!(parse_php_dir_name("php85nts"), ("8.5".into(), "nts".into()));
        assert_eq!(
            parse_php_dir_name("php7.4.3nts"),
            ("7.4.3".into(), "nts".into())
        );
        assert_eq!(
            parse_php_dir_name("php84nts"),
            ("8.4".into(), "nts".into())
        );
    }
}
