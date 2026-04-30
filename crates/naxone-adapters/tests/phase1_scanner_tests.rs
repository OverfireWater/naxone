//! Phase 1: verify the StandaloneScanner and CompositeScanner behavior.
//! Uses `tempfile` to build realistic on-disk layouts.

use std::fs;
use std::path::PathBuf;

use naxone_adapters::package::composite::CompositeScanner;
use naxone_adapters::package::standalone::StandaloneScanner;
use naxone_core::config::ExtraInstallPath;
use naxone_core::domain::service::{ServiceKind, ServiceOrigin};

/// Build a temp directory we control. `tempfile` isn't pulled in as a dep,
/// so use a deterministic dir under the OS temp folder and clean it up.
struct Scratch(PathBuf);

impl Scratch {
    fn new(tag: &str) -> Self {
        let base = std::env::temp_dir()
            .join("naxone-phase1-tests")
            .join(format!("{}-{}", tag, std::process::id()));
        let _ = fs::remove_dir_all(&base);
        fs::create_dir_all(&base).expect("create scratch dir");
        Self(base)
    }
    fn path(&self) -> &PathBuf {
        &self.0
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn touch(path: &PathBuf) {
    if let Some(p) = path.parent() {
        fs::create_dir_all(p).unwrap();
    }
    fs::write(path, b"").unwrap();
}

// ========== StandaloneScanner: extra_install_paths ==========

#[test]
fn standalone_scanner_detects_nginx_via_extra_path() {
    let scratch = Scratch::new("nginx");
    let nginx_root = scratch.path().join("my-nginx");
    fs::create_dir_all(&nginx_root).unwrap();
    touch(&nginx_root.join("nginx.exe"));
    touch(&nginx_root.join("conf").join("nginx.conf"));

    let extras = vec![ExtraInstallPath {
        id: "e1".into(),
        kind: ServiceKind::Nginx,
        path: nginx_root.clone(),
        label: None,
    }];
    let out = StandaloneScanner::scan(&extras, None);
    assert_eq!(out.len(), 1);
    assert_eq!(out[0].kind, ServiceKind::Nginx);
    assert_eq!(out[0].origin, ServiceOrigin::Manual);
    assert_eq!(out[0].install_path, nginx_root);
    assert!(out[0].config_path.is_some());
}

#[test]
fn standalone_scanner_follows_single_subdir_wrapper() {
    // Simulate an extracted zip: "C:/foo/" contains one subdir "nginx-1.25.3"
    // with the real install inside.
    let scratch = Scratch::new("wrapper");
    let wrapper = scratch.path().join("wrap");
    let real = wrapper.join("nginx-1.25.3");
    fs::create_dir_all(&real).unwrap();
    touch(&real.join("nginx.exe"));

    let extras = vec![ExtraInstallPath {
        id: "e1".into(),
        kind: ServiceKind::Nginx,
        path: wrapper.clone(),
        label: None,
    }];
    let out = StandaloneScanner::scan(&extras, None);
    assert_eq!(out.len(), 1);
    // install_path should be the inner dir, not the wrapper
    assert_eq!(out[0].install_path, real);
}

#[test]
fn standalone_scanner_skips_invalid_path() {
    let scratch = Scratch::new("invalid");
    let empty = scratch.path().join("no-binaries");
    fs::create_dir_all(&empty).unwrap();

    let extras = vec![ExtraInstallPath {
        id: "e1".into(),
        kind: ServiceKind::Nginx,
        path: empty,
        label: None,
    }];
    let out = StandaloneScanner::scan(&extras, None);
    assert_eq!(out.len(), 0);
}

#[test]
fn standalone_scanner_detects_mysql_and_redis() {
    let scratch = Scratch::new("db");
    let mysql_root = scratch.path().join("my-mysql");
    fs::create_dir_all(&mysql_root).unwrap();
    touch(&mysql_root.join("bin").join("mysqld.exe"));
    touch(&mysql_root.join("my.ini"));

    let redis_root = scratch.path().join("my-redis");
    fs::create_dir_all(&redis_root).unwrap();
    touch(&redis_root.join("redis-server.exe"));

    let extras = vec![
        ExtraInstallPath {
            id: "m".into(),
            kind: ServiceKind::Mysql,
            path: mysql_root.clone(),
            label: None,
        },
        ExtraInstallPath {
            id: "r".into(),
            kind: ServiceKind::Redis,
            path: redis_root.clone(),
            label: None,
        },
    ];
    let out = StandaloneScanner::scan(&extras, None);
    assert_eq!(out.len(), 2);
    assert!(out.iter().any(|s| s.kind == ServiceKind::Mysql));
    assert!(out.iter().any(|s| s.kind == ServiceKind::Redis));
}

// ========== StandaloneScanner: packages_root (%APPDATA%/NaxOne/Packages/) ==========

#[test]
fn standalone_scanner_reads_packages_root_layout() {
    let scratch = Scratch::new("pkgroot");
    let root = scratch.path().join("Packages");
    // Layout: Packages/nginx/1.25.3/nginx.exe
    let nginx_ver = root.join("nginx").join("1.25.3");
    fs::create_dir_all(&nginx_ver).unwrap();
    touch(&nginx_ver.join("nginx.exe"));

    // Layout: Packages/mysql/8.0.36/bin/mysqld.exe
    let mysql_ver = root.join("mysql").join("8.0.36");
    fs::create_dir_all(&mysql_ver).unwrap();
    touch(&mysql_ver.join("bin").join("mysqld.exe"));

    let out = StandaloneScanner::scan(&[], Some(&root));
    assert_eq!(out.len(), 2);
    for s in &out {
        assert_eq!(s.origin, ServiceOrigin::Store);
    }
    let nginx = out.iter().find(|s| s.kind == ServiceKind::Nginx).unwrap();
    assert_eq!(nginx.install_path, nginx_ver);
}

// ========== CompositeScanner: dedup ==========

#[test]
fn composite_scanner_dedupes_same_install_path() {
    // Create a fake PHPStudy Extensions layout AND also reference the same
    // Nginx path via extras. Should only appear once, as PhpStudy-origin.
    let scratch = Scratch::new("dedup");
    let ext = scratch.path().join("Extensions");
    let nginx_dir = ext.join("Nginx1.25.3");
    fs::create_dir_all(&nginx_dir).unwrap();
    touch(&nginx_dir.join("nginx.exe"));
    touch(&nginx_dir.join("conf").join("nginx.conf"));

    let extras = vec![ExtraInstallPath {
        id: "dup".into(),
        kind: ServiceKind::Nginx,
        path: nginx_dir.clone(),
        label: None,
    }];

    let out = CompositeScanner::scan(Some(&ext), None, &extras, None);
    let nginxes: Vec<_> = out.iter().filter(|s| s.kind == ServiceKind::Nginx).collect();
    assert_eq!(
        nginxes.len(),
        1,
        "Nginx at same path should not appear twice"
    );
    assert_eq!(nginxes[0].origin, ServiceOrigin::PhpStudy);
}

#[test]
fn composite_scanner_merges_distinct_sources() {
    let scratch = Scratch::new("merge");
    let ext = scratch.path().join("Extensions");
    let phpstudy_nginx = ext.join("Nginx1.15.11");
    fs::create_dir_all(&phpstudy_nginx).unwrap();
    touch(&phpstudy_nginx.join("nginx.exe"));

    let standalone_mysql = scratch.path().join("my-mysql");
    fs::create_dir_all(&standalone_mysql).unwrap();
    touch(&standalone_mysql.join("bin").join("mysqld.exe"));

    let extras = vec![ExtraInstallPath {
        id: "m".into(),
        kind: ServiceKind::Mysql,
        path: standalone_mysql.clone(),
        label: None,
    }];

    let out = CompositeScanner::scan(Some(&ext), None, &extras, None);
    assert_eq!(out.len(), 2);
    let nginx = out.iter().find(|s| s.kind == ServiceKind::Nginx).unwrap();
    let mysql = out.iter().find(|s| s.kind == ServiceKind::Mysql).unwrap();
    assert_eq!(nginx.origin, ServiceOrigin::PhpStudy);
    assert_eq!(mysql.origin, ServiceOrigin::Manual);
}

#[test]
fn composite_scanner_handles_missing_phpstudy() {
    let scratch = Scratch::new("no-phpstudy");
    let nginx = scratch.path().join("my-nginx");
    fs::create_dir_all(&nginx).unwrap();
    touch(&nginx.join("nginx.exe"));

    let extras = vec![ExtraInstallPath {
        id: "n".into(),
        kind: ServiceKind::Nginx,
        path: nginx.clone(),
        label: None,
    }];

    let out = CompositeScanner::scan(None, None, &extras, None);
    assert_eq!(out.len(), 1);
    assert_eq!(out[0].origin, ServiceOrigin::Manual);
}

// ========== CompositeScanner: PHP port avoidance ==========

#[test]
fn composite_scanner_assigns_unique_php_ports() {
    let scratch = Scratch::new("php-ports");
    let ext = scratch.path().join("Extensions");
    // Two PHP versions in the PHPStudy dir → PhpStudyScanner assigns 9000, 9001
    for php_dir in ["php/php74nts", "php/php83nts"] {
        let p = ext.join(php_dir);
        fs::create_dir_all(&p).unwrap();
        touch(&p.join("php-cgi.exe"));
    }
    // One standalone PHP via extras
    let standalone_php = scratch.path().join("my-php");
    fs::create_dir_all(&standalone_php).unwrap();
    touch(&standalone_php.join("php-cgi.exe"));

    let extras = vec![ExtraInstallPath {
        id: "p".into(),
        kind: ServiceKind::Php,
        path: standalone_php.clone(),
        label: None,
    }];

    let out = CompositeScanner::scan(Some(&ext), None, &extras, None);
    let php_ports: Vec<u16> = out
        .iter()
        .filter(|s| s.kind == ServiceKind::Php)
        .map(|s| s.port)
        .collect();

    // 3 PHP instances, each should have a unique port
    assert_eq!(php_ports.len(), 3);
    let mut sorted = php_ports.clone();
    sorted.sort();
    sorted.dedup();
    assert_eq!(
        sorted.len(),
        3,
        "expected distinct PHP ports, got {:?}",
        php_ports
    );
}
