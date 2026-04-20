//! 从自建的 GitHub 镜像仓库读取 manifest.json。
//!
//! manifest.json 由镜像仓库的 Actions workflow 自动生成，结构：
//! ```json
//! {
//!   "generated_at": "...",
//!   "repo": "OverfireWater/ruststudy-mirror",
//!   "packages": {
//!     "php": [
//!       {
//!         "version": "8.5.5",
//!         "variant": "x64-NTS vs17",
//!         "filename": "php-8.5.5-nts-Win32-vs17-x64.zip",
//!         "size_bytes": 35380502,
//!         "sha256": "...",
//!         "exe_rel": "php-cgi.exe",
//!         "tag": "php-8.5.5",
//!         "download_urls": [
//!           "https://ghfast.top/https://github.com/.../php-8.5.5-...zip",
//!           "https://ghproxy.net/https://github.com/...",
//!           "https://gh-proxy.com/https://github.com/...",
//!           "https://github.com/..."
//!         ]
//!       }
//!     ],
//!     "nginx": [ ... ],
//!     ...
//!   }
//! }
//! ```
//!
//! 本模块只负责抓 JSON 并转成 `PackageVersion` 列表；类别/颜色等 UI 元数据
//! 由调用方从内嵌 `packages.json` 保留，只把版本列表替换掉。

use std::collections::HashMap;
use std::time::Duration;

use serde::Deserialize;

use crate::package::manifest::PackageVersion;

/// 镜像 manifest 的 URL。走 jsDelivr（服务 git tree 里的文件，完美）。
/// manifest.json 本身不大（几十 KB），更新后 jsDelivr 缓存 12h 左右；
/// 对版本列表够用——新版本发布当天不会立即看到是可以接受的。
const MANIFEST_URL: &str =
    "https://cdn.jsdelivr.net/gh/OverfireWater/ruststudy-mirror@main/manifest.json";

#[derive(Debug, Deserialize)]
struct RootManifest {
    #[allow(dead_code)]
    generated_at: Option<String>,
    packages: HashMap<String, Vec<MirrorEntry>>,
}

#[derive(Debug, Deserialize)]
struct MirrorEntry {
    version: String,
    #[serde(default)]
    variant: Option<String>,
    #[serde(default)]
    #[allow(dead_code)]
    filename: Option<String>,
    #[serde(default)]
    size_bytes: Option<u64>,
    #[serde(default)]
    sha256: Option<String>,
    #[serde(default)]
    exe_rel: String,
    #[serde(default)]
    #[allow(dead_code)]
    tag: Option<String>,
    #[serde(default)]
    download_urls: Vec<String>,
}

/// 拉一次镜像 manifest。失败时上游可能不通/被墙，返回 Err 由调用方 fallback。
pub async fn fetch() -> Result<HashMap<String, Vec<PackageVersion>>, String> {
    let client = reqwest::Client::builder()
        .user_agent("RustStudy/0.1.0")
        .timeout(Duration::from_secs(15))
        .build()
        .map_err(|e| format!("HTTP 客户端初始化失败: {}", e))?;

    let resp = client
        .get(MANIFEST_URL)
        .send()
        .await
        .map_err(|e| format!("拉 manifest 失败: {}", e))?;

    if !resp.status().is_success() {
        return Err(format!("HTTP {} from {}", resp.status(), MANIFEST_URL));
    }

    let root: RootManifest = resp
        .json()
        .await
        .map_err(|e| format!("解析 manifest 失败: {}", e))?;

    let mut out: HashMap<String, Vec<PackageVersion>> = HashMap::new();
    for (software, entries) in root.packages {
        let versions: Vec<PackageVersion> = entries
            .into_iter()
            .filter(|e| !e.download_urls.is_empty())
            .map(|e| PackageVersion {
                version: e.version,
                url: e.download_urls.first().cloned().unwrap_or_default(),
                download_urls: e.download_urls,
                sha256: e.sha256,
                size_mb: e.size_bytes.map(|b| (b / 1024 / 1024) as u32),
                exe_rel: e.exe_rel,
                variant: e.variant,
            })
            .collect();
        if !versions.is_empty() {
            out.insert(software, versions);
        }
    }
    Ok(out)
}
