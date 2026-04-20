use std::path::Path;

use ruststudy_adapters::config::fs_config::FsConfigIO;
use ruststudy_adapters::template::SimpleTemplateEngine;
use ruststudy_adapters::vhost::VhostScanner;
use ruststudy_core::domain::php::{PhpExtension, PhpIniSettings};
use ruststudy_core::domain::vhost::VirtualHost;
use ruststudy_core::ports::config_io::ConfigIO;
use ruststudy_core::ports::template::TemplateEngine;
use ruststudy_core::use_cases::php_mgr::PhpManager;

use ruststudy_adapters::package::scanner::PhpStudyScanner;

#[test]
fn test_scan_services() {
    let ext_path = Path::new(r"D:\phpstudy_pro\Extensions");
    if !ext_path.exists() {
        println!("SKIP: PHPStudy not installed");
        return;
    }
    let services = PhpStudyScanner::scan(ext_path).unwrap();
    println!("=== Scanned Services ({}) ===", services.len());
    for s in &services {
        println!(
            "  {} {} {:?} port={} path={}",
            s.kind.display_name(),
            s.version,
            s.variant,
            s.port,
            s.install_path.display()
        );
    }
    assert!(!services.is_empty(), "Should find at least one service");
}

#[test]
fn test_scan_vhosts() {
    let ext_path = Path::new(r"D:\phpstudy_pro\Extensions");
    if !ext_path.exists() {
        println!("SKIP: PHPStudy not installed");
        return;
    }
    let services = PhpStudyScanner::scan(ext_path).unwrap();
    let config_io = FsConfigIO;

    // Find nginx vhosts dir
    let nginx_dir = std::fs::read_dir(ext_path)
        .unwrap()
        .flatten()
        .find(|e| e.file_name().to_string_lossy().starts_with("Nginx"))
        .map(|e| e.path().join("conf").join("vhosts"));

    if let Some(dir) = nginx_dir {
        let vhosts = VhostScanner::scan(&config_io, &dir, &services).unwrap();
        println!("\n=== Scanned Vhosts ({}) ===", vhosts.len());
        for vh in &vhosts {
            println!(
                "  {} :{} -> {} php={:?}",
                vh.server_name,
                vh.listen_port,
                vh.document_root.display(),
                vh.php_version
            );
            if !vh.aliases.is_empty() {
                println!("    aliases: {:?}", vh.aliases);
            }
        }
        assert!(!vhosts.is_empty(), "Should find at least one vhost");
    }
}

#[test]
fn test_template_engine_nginx() {
    let engine = SimpleTemplateEngine;
    let vhost = VirtualHost {
        id: "test.nm_80".into(),
        server_name: "test.nm".into(),
        aliases: vec!["www.test.nm".into()],
        listen_port: 80,
        document_root: r"D:\phpstudy_pro\WWW\test".into(),
        php_version: Some("php85nts".into()),
        php_fastcgi_port: Some(9000),
        php_install_path: Some(r"D:\phpstudy_pro\Extensions\php\php85nts".into()),
        index_files: "index.php index.html".into(),
        rewrite_rule: String::new(),
        autoindex: false,
        ssl: None,
        custom_directives: None,
        access_log: None,
        enabled: true,
        created_at: String::new(),
        expires_at: String::new(),
        sync_hosts: true,
        source: ruststudy_core::domain::vhost::VhostSource::Custom,
    };

    let nginx = engine.render_nginx_vhost(&vhost).unwrap();
    println!("\n=== Generated Nginx Config ===\n{}", nginx);
    assert!(nginx.contains("listen        80;"));
    assert!(nginx.contains("server_name  test.nm www.test.nm;"));
    assert!(nginx.contains("fastcgi_pass   127.0.0.1:9000;"));
    assert!(nginx.contains("D:/phpstudy_pro/WWW/test"));

    let apache = engine.render_apache_vhost(&vhost).unwrap();
    println!("\n=== Generated Apache Config ===\n{}", apache);
    assert!(apache.contains("<VirtualHost *:80>"));
    assert!(apache.contains("ServerName test.nm"));
    assert!(apache.contains("ServerAlias www.test.nm"));
    assert!(apache.contains("FcgidWrapper"));
    assert!(apache.contains("php85nts/php-cgi.exe"));
}

#[test]
fn test_php_extensions() {
    let php_path = Path::new(r"D:\phpstudy_pro\Extensions\php\php85nts");
    if !php_path.exists() {
        println!("SKIP: PHP 8.5 not installed");
        return;
    }

    let config_io = std::sync::Arc::new(FsConfigIO);
    let mgr = PhpManager::new(config_io);

    let exts = mgr.list_extensions(php_path).unwrap();
    println!("\n=== PHP 8.5 Extensions ({}) ===", exts.len());
    for ext in &exts {
        println!(
            "  [{}] {} {}",
            if ext.enabled { "ON " } else { "OFF" },
            ext.name,
            if ext.is_zend { "(Zend)" } else { "" }
        );
    }
    assert!(!exts.is_empty(), "Should find extensions");

    // Check some known extensions
    let curl = exts.iter().find(|e| e.name == "curl");
    assert!(curl.is_some(), "curl extension should exist");
}

#[test]
fn test_php_ini_settings() {
    let php_path = Path::new(r"D:\phpstudy_pro\Extensions\php\php85nts");
    if !php_path.exists() {
        println!("SKIP: PHP 8.5 not installed");
        return;
    }

    let config_io = std::sync::Arc::new(FsConfigIO);
    let mgr = PhpManager::new(config_io);

    let settings = mgr.read_ini_settings(php_path).unwrap();
    println!("\n=== PHP 8.5 INI Settings ===");
    println!("  memory_limit: {}", settings.memory_limit);
    println!("  upload_max_filesize: {}", settings.upload_max_filesize);
    println!("  post_max_size: {}", settings.post_max_size);
    println!("  max_execution_time: {}", settings.max_execution_time);
    println!("  display_errors: {}", settings.display_errors);
    println!("  error_reporting: {}", settings.error_reporting);
    println!("  date_timezone: {}", settings.date_timezone);

    assert!(!settings.memory_limit.is_empty());
    assert!(!settings.upload_max_filesize.is_empty());
}
