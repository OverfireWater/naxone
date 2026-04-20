//! Embedded package manifest for the in-app software store.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Manifest {
    pub packages: Vec<PackageEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackageEntry {
    pub name: String,         // "nginx"
    pub display_name: String, // "Nginx"
    /// "web" | "db" | "php" | "cache"
    pub category: String,
    pub brand_color: String, // "#009639"
    pub brand_letter: String,
    #[serde(default)]
    pub homepage: String,
    #[serde(default)]
    pub description: String,
    pub versions: Vec<PackageVersion>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackageVersion {
    pub version: String,
    /// 主 URL（内嵌 packages.json 只有这一个字段，是兼容写法）
    #[serde(default)]
    pub url: String,
    /// 完整的候选 URL 列表（镜像 manifest 用这个）。
    /// 若非空 → Installer 按顺序尝试，任一成功即止；全败才报错。
    /// 若为空 → 退化成只用 `url`。
    #[serde(default)]
    pub download_urls: Vec<String>,
    #[serde(default)]
    pub sha256: Option<String>,
    #[serde(default)]
    pub size_mb: Option<u32>,
    /// Relative to the unzipped root (or single-subdir wrapper)
    pub exe_rel: String,
    #[serde(default)]
    pub variant: Option<String>,
}

impl PackageVersion {
    /// 返回该版本的候选下载 URL 列表，调用方按顺序尝试。
    /// `download_urls` 非空时用它；否则退化为单 `url`。
    pub fn candidate_urls(&self) -> Vec<String> {
        if !self.download_urls.is_empty() {
            self.download_urls.clone()
        } else if !self.url.is_empty() {
            vec![self.url.clone()]
        } else {
            vec![]
        }
    }
}

pub const PACKAGES_JSON: &str = include_str!("packages.json");

/// Parse the embedded packages.json. Panics if the build-time JSON is malformed.
pub fn load_manifest() -> Manifest {
    serde_json::from_str(PACKAGES_JSON).expect("packages.json is malformed")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manifest_parses() {
        let m = load_manifest();
        assert!(
            !m.packages.is_empty(),
            "manifest should have at least one package"
        );
    }

    #[test]
    fn manifest_has_all_five_categories() {
        let m = load_manifest();
        let cats: std::collections::HashSet<_> =
            m.packages.iter().map(|p| p.category.as_str()).collect();
        assert!(cats.contains("web"), "missing 'web' category");
        assert!(cats.contains("db"), "missing 'db' category");
        assert!(cats.contains("php"), "missing 'php' category");
        assert!(cats.contains("cache"), "missing 'cache' category");
    }

    #[test]
    fn manifest_versions_sane() {
        let m = load_manifest();
        for p in &m.packages {
            assert!(!p.versions.is_empty(), "{} has no versions", p.name);
            for v in &p.versions {
                let urls = v.candidate_urls();
                assert!(!urls.is_empty(), "{} v{} has no candidate URLs", p.name, v.version);
                assert!(urls[0].starts_with("http"), "{} bad url {}", p.name, urls[0]);
                assert!(
                    !v.exe_rel.is_empty(),
                    "{} v{} missing exe_rel",
                    p.name,
                    v.version
                );
            }
        }
    }

    #[test]
    fn manifest_has_expected_software() {
        // Just make sure each package has at least one version. Exact counts
        // change as new releases land — don't hard-code them.
        let m = load_manifest();
        for name in ["nginx", "apache", "mysql", "php", "redis"] {
            let pkg = m.packages.iter().find(|p| p.name == name);
            assert!(pkg.is_some(), "missing package {}", name);
            assert!(
                !pkg.unwrap().versions.is_empty(),
                "{} has no versions",
                name
            );
        }
    }

    #[test]
    fn manifest_php_has_major_84() {
        // Regression: PHP 8.4 should be offered
        let m = load_manifest();
        let php = m.packages.iter().find(|p| p.name == "php").unwrap();
        assert!(
            php.versions.iter().any(|v| v.version.starts_with("8.4")),
            "PHP 8.4.x not in manifest"
        );
    }
}
