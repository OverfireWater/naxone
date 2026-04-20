//! Pull the current Windows PHP versions from the official releases.json.
//!
//! Endpoint: https://windows.php.net/downloads/releases/releases.json
//! (redirects to downloads.php.net/~windows/releases/releases.json — reqwest
//!  follows the redirect transparently.)
//!
//! Response shape (trimmed):
//! ```json
//! {
//!   "8.3": {
//!     "version": "8.3.30",
//!     "nts-vs16-x64": {
//!       "mtime": "...",
//!       "zip": { "path": "php-8.3.30-nts-Win32-vs16-x64.zip",
//!                "size": "30.76MB",
//!                "sha256": "..." }
//!     }
//!   },
//!   "8.4": { ... },
//!   ...
//! }
//! ```
//!
//! Only the *latest* build per branch is advertised here. Historical / EOL
//! versions come from the embedded `packages.json`.

use std::collections::HashMap;
use std::time::Duration;

use serde::Deserialize;

use crate::package::manifest::PackageVersion;

const ENDPOINT: &str = "https://windows.php.net/downloads/releases/releases.json";
const BASE_DL: &str = "https://windows.php.net/downloads/releases/";

#[derive(Debug, Deserialize)]
struct Branch {
    #[serde(default)]
    version: Option<String>,
    #[serde(rename = "nts-vs17-x64", default)]
    nts_vs17_x64: Option<Variant>,
    #[serde(rename = "nts-vs16-x64", default)]
    nts_vs16_x64: Option<Variant>,
    #[serde(rename = "nts-vc15-x64", default)]
    nts_vc15_x64: Option<Variant>,
}

#[derive(Debug, Deserialize)]
struct Variant {
    #[serde(default)]
    zip: Option<ZipInfo>,
}

#[derive(Debug, Deserialize)]
struct ZipInfo {
    path: String,
    #[serde(default)]
    size: Option<String>,
    #[serde(default)]
    sha256: Option<String>,
}

/// Fetch the current set of official Windows PHP versions. One `PackageVersion`
/// per active branch (8.5 / 8.4 / 8.3 ...). Returns an error if the network is
/// down or the JSON can't be parsed.
pub async fn fetch() -> Result<Vec<PackageVersion>, String> {
    let client = reqwest::Client::builder()
        .user_agent("RustStudy/0.1.0")
        .timeout(Duration::from_secs(10))
        .build()
        .map_err(|e| format!("HTTP 客户端初始化失败: {}", e))?;

    let resp = client
        .get(ENDPOINT)
        .send()
        .await
        .map_err(|e| format!("请求 releases.json 失败: {}", e))?;

    if !resp.status().is_success() {
        return Err(format!("HTTP {} from {}", resp.status(), ENDPOINT));
    }

    let data: HashMap<String, Branch> = resp
        .json()
        .await
        .map_err(|e| format!("解析 releases.json 失败: {}", e))?;

    let mut out = Vec::new();
    for (branch_key, branch) in data {
        // Skip any non-version keys the API may add later
        let Some(first_char) = branch_key.chars().next() else {
            continue;
        };
        if !first_char.is_ascii_digit() {
            continue;
        }

        let version = match branch.version {
            Some(v) => v,
            None => continue,
        };

        // Prefer newer compiler: vs17 > vs16 > vc15
        let (compiler, variant) = if let Some(v) = branch.nts_vs17_x64 {
            ("vs17", v)
        } else if let Some(v) = branch.nts_vs16_x64 {
            ("vs16", v)
        } else if let Some(v) = branch.nts_vc15_x64 {
            ("vc15", v)
        } else {
            continue;
        };

        let zip = match variant.zip {
            Some(z) => z,
            None => continue,
        };

        out.push(PackageVersion {
            version,
            url: format!("{}{}", BASE_DL, zip.path),
            download_urls: Vec::new(),
            sha256: zip.sha256,
            size_mb: zip.size.as_deref().and_then(parse_size_mb),
            exe_rel: "php-cgi.exe".to_string(),
            variant: Some(format!("x64-NTS · {}", compiler)),
        });
    }

    // Newest first
    out.sort_by(|a, b| cmp_semver_desc(&a.version, &b.version));

    Ok(out)
}

/// Parse sizes like "30.76MB" or "225.4 MB" into integer megabytes.
fn parse_size_mb(s: &str) -> Option<u32> {
    let trimmed = s.trim();
    // Tolerate "MB", "mb", "M", space-separated, etc.
    let num_part = trimmed
        .trim_end_matches(|c: char| c.is_alphabetic() || c.is_whitespace())
        .trim();
    num_part.parse::<f32>().ok().map(|f| f.round() as u32)
}

/// Compare two dotted-numeric version strings numerically, descending.
/// Non-numeric segments fall through to string compare.
fn cmp_semver_desc(a: &str, b: &str) -> std::cmp::Ordering {
    let mut ai = a.split('.').map(|s| s.parse::<u32>().ok());
    let mut bi = b.split('.').map(|s| s.parse::<u32>().ok());
    loop {
        match (ai.next(), bi.next()) {
            (None, None) => return std::cmp::Ordering::Equal,
            (Some(Some(x)), Some(Some(y))) => {
                if x != y {
                    // descending
                    return y.cmp(&x);
                }
            }
            (Some(_), None) => return std::cmp::Ordering::Less,
            (None, Some(_)) => return std::cmp::Ordering::Greater,
            _ => return b.cmp(a),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_size_various() {
        assert_eq!(parse_size_mb("30.76MB"), Some(31));
        assert_eq!(parse_size_mb("225.4 MB"), Some(225));
        assert_eq!(parse_size_mb("7M"), Some(7));
        assert_eq!(parse_size_mb("nonsense"), None);
    }

    #[test]
    fn cmp_desc_orders_correctly() {
        let mut v = vec!["8.1.31", "8.4.2", "8.3.30", "8.2.26"];
        v.sort_by(|a, b| cmp_semver_desc(a, b));
        assert_eq!(v, vec!["8.4.2", "8.3.30", "8.2.26", "8.1.31"]);
    }
}
