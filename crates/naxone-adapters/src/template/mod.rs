use naxone_core::domain::vhost::VirtualHost;
use naxone_core::error::{NaxOneError, Result};
use naxone_core::ports::template::TemplateEngine;

/// Template engine using format strings, matching PHPStudy's exact config format
pub struct SimpleTemplateEngine;

impl SimpleTemplateEngine {
    fn to_forward_slash(path: &std::path::Path) -> String {
        path.display().to_string().replace('\\', "/")
    }
}

impl TemplateEngine for SimpleTemplateEngine {
    fn render_nginx_vhost(&self, vhost: &VirtualHost) -> Result<String> {
        vhost.validate().map_err(NaxOneError::Template)?;
        let port = vhost.listen_port;
        let domain = &vhost.server_name;
        let doc_root = Self::to_forward_slash(&vhost.document_root);
        let index_files = if vhost.index_files.is_empty() { "index.php index.html" } else { &vhost.index_files };
        let autoindex = if vhost.autoindex { "on" } else { "off" };

        let server_name = if vhost.aliases.is_empty() || vhost.aliases.iter().all(|a| a.is_empty()) {
            format!("{};", domain)
        } else {
            let aliases: Vec<&str> = vhost.aliases.iter().filter(|a| !a.is_empty()).map(|a| a.as_str()).collect();
            format!("{} {};", domain, aliases.join(" "))
        };

        // Rewrite: if has rule, include nginx.htaccess (PHPStudy style)
        let has_rewrite = !vhost.rewrite_rule.is_empty();

        // PHP location block
        let php_block = if let Some(php_port) = vhost.php_fastcgi_port {
            format!(
                r#"        location ~ \.php(.*)$ {{
            fastcgi_pass   127.0.0.1:{php_port};
            fastcgi_index  index.php;
            fastcgi_split_path_info  ^((?U).+\.php)(/?.+)$;
            fastcgi_param  SCRIPT_FILENAME  $document_root$fastcgi_script_name;
            fastcgi_param  PATH_INFO  $fastcgi_path_info;
            fastcgi_param  PATH_TRANSLATED  $document_root$fastcgi_path_info;
            include        fastcgi_params;
        }}"#
            )
        } else {
            String::new()
        };

        // SSL block
        let ssl_block = if let Some(ref ssl) = vhost.ssl {
            let cert = Self::to_forward_slash(&ssl.cert_path);
            let key = Self::to_forward_slash(&ssl.key_path);
            let redirect = if ssl.force_https {
                format!("\n        if ($scheme = http) {{\n            return 301 https://$host$request_uri;\n        }}")
            } else { String::new() };
            format!(
                r#"
        listen        443 ssl;
        ssl_certificate     {cert};
        ssl_certificate_key {key};{redirect}"#
            )
        } else {
            String::new()
        };

        // Access log
        let log_line = if let Some(ref log) = vhost.access_log {
            format!("\n        access_log  {};", Self::to_forward_slash(std::path::Path::new(log)))
        } else {
            String::new()
        };

        // Custom directives
        let custom = if let Some(ref d) = vhost.custom_directives {
            format!("\n{}", d.lines().map(|l| format!("        {}", l)).collect::<Vec<_>>().join("\n"))
        } else {
            String::new()
        };

        // Build the location / block
        let include_line = if has_rewrite {
            format!("\n            include {doc_root}/nginx.htaccess;")
        } else {
            String::new()
        };
        let location_block = format!(r#"        location / {{
            index {index_files} error/index.html;{include_line}
            autoindex  {autoindex};
        }}"#);

        let conf = format!(
            r#"server {{
        listen        {port};{ssl_block}
        server_name  {server_name}
        root   "{doc_root}";{log_line}
{location_block}
{php_block}{custom}
}}"#
        );

        Ok(conf)
    }

    fn render_apache_vhost(&self, vhost: &VirtualHost) -> Result<String> {
        vhost.validate().map_err(NaxOneError::Template)?;
        let port = vhost.listen_port;
        let domain = &vhost.server_name;
        let doc_root = Self::to_forward_slash(&vhost.document_root);
        let index_files = if vhost.index_files.is_empty() { "index.php index.html" } else { &vhost.index_files };

        let aliases = vhost.aliases.iter().filter(|a| !a.is_empty()).map(|a| a.as_str()).collect::<Vec<_>>().join(" ");

        let php_block = if let Some(php_path) = &vhost.php_install_path {
            let php_path_str = Self::to_forward_slash(php_path);
            format!(
                r#"    FcgidInitialEnv PHPRC "{php_path_str}"
    AddHandler fcgid-script .php
    FcgidWrapper "{php_path_str}/php-cgi.exe" .php"#
            )
        } else {
            String::new()
        };

        // SSL
        let ssl_block = if let Some(ref ssl) = vhost.ssl {
            let cert = Self::to_forward_slash(&ssl.cert_path);
            let key = Self::to_forward_slash(&ssl.key_path);
            format!(
                r#"
    SSLEngine on
    SSLCertificateFile "{cert}"
    SSLCertificateKeyFile "{key}""#
            )
        } else {
            String::new()
        };

        let conf = format!(
            r#"<VirtualHost *:{port}>
    DocumentRoot "{doc_root}"
    ServerName {domain}
    ServerAlias {aliases}
{php_block}{ssl_block}
  <Directory "{doc_root}">
      Options FollowSymLinks ExecCGI
      AllowOverride All
      Order allow,deny
      Allow from all
      Require all granted
	  DirectoryIndex {index_files} error/index.html
  </Directory>
  ErrorDocument 400 /error/400.html
  ErrorDocument 403 /error/403.html
  ErrorDocument 404 /error/404.html
  ErrorDocument 500 /error/500.html
  ErrorDocument 501 /error/501.html
  ErrorDocument 502 /error/502.html
  ErrorDocument 503 /error/503.html
  ErrorDocument 504 /error/504.html
  ErrorDocument 505 /error/505.html
  ErrorDocument 506 /error/506.html
  ErrorDocument 507 /error/507.html
  ErrorDocument 510 /error/510.html
</VirtualHost>"#
        );

        Ok(conf)
    }
}
