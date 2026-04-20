use std::path::{Path, PathBuf};

use ruststudy_core::domain::service::ServiceInstance;
use ruststudy_core::domain::service::ServiceKind;
use ruststudy_core::domain::vhost::{VhostSource, VirtualHost};
use ruststudy_core::error::Result;
use ruststudy_core::ports::config_io::ConfigIO;

/// Scans existing Nginx vhost config files and parses them into VirtualHost structs
pub struct VhostScanner;

impl VhostScanner {
    /// Scan Nginx vhosts directory and parse all .conf files
    pub fn scan(
        config_io: &dyn ConfigIO,
        nginx_vhosts_dir: &Path,
        php_instances: &[ServiceInstance],
    ) -> Result<Vec<VirtualHost>> {
        let conf_files = config_io.list_files(nginx_vhosts_dir, "conf")?;
        // Also include disabled vhosts
        let all_entries: Vec<_> = std::fs::read_dir(nginx_vhosts_dir)
            .ok()
            .map(|rd| rd.flatten().map(|e| e.path())
                .filter(|p| p.file_name().and_then(|n| n.to_str())
                    .map(|s| s.ends_with(".conf.disabled")).unwrap_or(false))
                .collect())
            .unwrap_or_default();

        let mut vhosts = Vec::new();
        for file in conf_files.into_iter().chain(all_entries.into_iter()) {
            let filename = file.file_name().and_then(|n| n.to_str()).unwrap_or_default().to_string();
            if filename.starts_with("default-") { continue; }

            let is_disabled = filename.ends_with(".disabled");
            let content = match config_io.read_text(&file) {
                Ok(c) if !c.trim().is_empty() => c,
                _ => continue,
            };

            if let Some(mut vhost) = Self::parse_nginx_conf(&content, php_instances) {
                if is_disabled { vhost.enabled = false; }
                vhosts.push(vhost);
            }
        }

        Ok(vhosts)
    }

    /// Parse a single Nginx vhost config file into a VirtualHost
    fn parse_nginx_conf(content: &str, php_instances: &[ServiceInstance]) -> Option<VirtualHost> {
        let port = Self::extract_regex(content, r"listen\s+(\d+)")?
            .parse::<u16>()
            .ok()?;

        let server_name_line = Self::extract_regex(content, r"server_name\s+(.+);")?;
        let names: Vec<&str> = server_name_line.split_whitespace().collect();
        let domain = names.first()?.to_string();
        let aliases: Vec<String> = names[1..].iter().map(|s| s.to_string()).collect();

        let doc_root_str = Self::extract_regex(content, r#"root\s+"([^"]+)""#)?;
        let document_root = PathBuf::from(&doc_root_str);

        // Extract PHP FastCGI port
        let php_fastcgi_port = Self::extract_regex(content, r"fastcgi_pass\s+127\.0\.0\.1:(\d+)")
            .and_then(|s| s.parse::<u16>().ok());

        // Match PHP port to PHP instance to get version and install path
        let (php_version, php_install_path) = if let Some(php_port) = php_fastcgi_port {
            let php_inst = php_instances
                .iter()
                .find(|s| s.kind == ServiceKind::Php && s.port == php_port);
            match php_inst {
                Some(inst) => {
                    let ver = if let Some(ref variant) = inst.variant {
                        format!("php{}{}", inst.version.replace('.', ""), variant)
                    } else {
                        format!("php{}", inst.version.replace('.', ""))
                    };
                    (Some(ver), Some(inst.install_path.clone()))
                }
                None => (None, None),
            }
        } else {
            (None, None)
        };

        // Read rewrite rules from nginx.htaccess file (PHPStudy stores rules there)
        let htaccess_path = document_root.join("nginx.htaccess");
        let rewrite_rule = if htaccess_path.exists() {
            std::fs::read_to_string(&htaccess_path).unwrap_or_default().trim().to_string()
        } else {
            // Fallback: try to extract from conf content (e.g. dawnframe uses try_files directly)
            Self::extract_location_rewrite(content)
        };

        let id = format!("{}_{}", domain, port);

        Some(VirtualHost {
            id,
            server_name: domain,
            aliases,
            listen_port: port,
            document_root,
            php_version,
            php_fastcgi_port,
            php_install_path,
            index_files: "index.php index.html".into(),
            rewrite_rule,
            autoindex: false,
            ssl: None,
            custom_directives: None,
            access_log: None,
            enabled: true,
            created_at: String::new(),
            expires_at: String::new(),
            sync_hosts: true,
            source: VhostSource::PhpStudy,
        })
    }

    /// Extract rewrite rules from the location / { ... } block.
    /// Returns a full "location / { ... }" block if rewrite/try_files/if rules are found, empty string otherwise.
    fn extract_location_rewrite(content: &str) -> String {
        // Find "location / {" and extract lines until closing "}"
        let mut in_location = false;
        let mut brace_depth = 0;
        let mut rewrite_lines: Vec<String> = Vec::new();
        let mut has_rewrite = false;

        for line in content.lines() {
            let trimmed = line.trim();

            if !in_location {
                if trimmed.starts_with("location / {") || trimmed == "location / {" {
                    in_location = true;
                    brace_depth = 1;
                }
                continue;
            }

            // Count braces
            for ch in trimmed.chars() {
                if ch == '{' { brace_depth += 1; }
                if ch == '}' { brace_depth -= 1; }
            }

            if brace_depth <= 0 {
                break;
            }

            // Skip standard lines (index, autoindex, error_page, include fastcgi)
            if trimmed.starts_with("index ")
                || trimmed.starts_with("autoindex")
                || trimmed.starts_with("error_page")
                || trimmed.is_empty()
            {
                continue;
            }

            // These are rewrite-related lines
            if trimmed.starts_with("try_files")
                || trimmed.starts_with("rewrite")
                || trimmed.starts_with("if ")
                || trimmed.starts_with("if(")
                || trimmed.starts_with("break")
                || trimmed.starts_with("return ")
                || trimmed.starts_with("include") && !trimmed.contains("fastcgi")
            {
                has_rewrite = true;
            }
            rewrite_lines.push(trimmed.to_string());
        }

        if !has_rewrite || rewrite_lines.is_empty() {
            return String::new();
        }

        // Build a full location block
        let inner = rewrite_lines.iter().map(|l| format!("    {}", l)).collect::<Vec<_>>().join("\n");
        format!("location / {{\n{}\n}}", inner)
    }

    /// Simple regex-like extraction (manual parsing to avoid regex dependency)
    fn extract_regex(content: &str, pattern: &str) -> Option<String> {
        // We use a simple approach: find the pattern prefix, then extract the capture group
        // This avoids pulling in the regex crate

        // Split pattern at the capture group
        let (before, _after) = if let Some(pos) = pattern.find('(') {
            let end = pattern.find(')')?;
            (&pattern[..pos], &pattern[end + 1..])
        } else {
            return None;
        };

        // Extract the capture group pattern to determine what to match
        let _capture_pattern = &pattern[pattern.find('(')? + 1..pattern.find(')')?];

        for line in content.lines() {
            let trimmed = line.trim();

            // Find the "before" part using whitespace-flexible matching
            let before_parts: Vec<&str> = before.split("\\s+").collect();
            if before_parts.is_empty() {
                continue;
            }

            // Check if line starts with the literal prefix (ignoring regex whitespace)
            let literal_prefix = before_parts[0].replace("\\.", ".").replace(r#"\""#, "\"");
            if literal_prefix.is_empty() || !trimmed.contains(&literal_prefix) {
                continue;
            }

            // For our specific patterns, use targeted extraction
            if pattern.contains("listen") && trimmed.starts_with("listen") {
                // Extract port number after "listen"
                let rest = trimmed.strip_prefix("listen")?.trim().trim_end_matches(';');
                return Some(rest.trim().to_string());
            }

            if pattern.contains("server_name") && trimmed.starts_with("server_name") {
                let rest = trimmed
                    .strip_prefix("server_name")?
                    .trim()
                    .trim_end_matches(';');
                return Some(rest.trim().to_string());
            }

            if pattern.contains("root") && trimmed.starts_with("root") && !trimmed.contains("$") {
                // Extract path between quotes
                let start = trimmed.find('"')? + 1;
                let end = trimmed[start..].find('"')? + start;
                return Some(trimmed[start..end].to_string());
            }

            if pattern.contains("fastcgi_pass") && trimmed.starts_with("fastcgi_pass") {
                // Extract port from 127.0.0.1:PORT
                let colon_pos = trimmed.rfind(':')?;
                let rest = trimmed[colon_pos + 1..].trim_end_matches(';').trim();
                return Some(rest.to_string());
            }
        }

        None
    }
}
