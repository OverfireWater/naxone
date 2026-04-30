//! 在线更新检查：启动时静默查 Gitee Release，比较版本号，有新版通知前端。

use serde::{Deserialize, Serialize};

/// Gitee Releases API 返回的 release 信息（只取我们需要的字段）
#[derive(Deserialize)]
struct GiteeRelease {
    tag_name: String,
    #[serde(default)]
    body: Option<String>,
    #[serde(default)]
    html_url: Option<String>,
    #[serde(default)]
    assets: Vec<GiteeAsset>,
}

#[derive(Deserialize)]
struct GiteeAsset {
    name: String,
    browser_download_url: String,
}

/// 返回给前端的更新信息
#[derive(Debug, Clone, Serialize)]
pub struct UpdateInfo {
    pub available: bool,
    pub latest_version: String,
    pub current_version: String,
    pub release_url: String,
    pub release_notes: String,
    pub download_url: Option<String>,
}

const GITEE_API: &str = "https://gitee.com/api/v5/repos/kz_y/naxone/releases/latest";
const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");

/// 检查 Gitee 上是否有比当前更新的 Release。
/// 失败时静默返回 `available: false`，不弹错误。
#[tauri::command]
pub async fn check_for_updates() -> Result<UpdateInfo, String> {
    let not_available = UpdateInfo {
        available: false,
        latest_version: CURRENT_VERSION.to_string(),
        current_version: CURRENT_VERSION.to_string(),
        release_url: String::new(),
        release_notes: String::new(),
        download_url: None,
    };

    let client = reqwest::Client::builder()
        .user_agent("NaxOne-Updater/1.0")
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| e.to_string())?;

    let resp = match client.get(GITEE_API).send().await {
        Ok(r) if r.status().is_success() => r,
        _ => return Ok(not_available),
    };

    let release: GiteeRelease = match resp.json().await {
        Ok(r) => r,
        Err(_) => return Ok(not_available),
    };

    let latest = release.tag_name.trim_start_matches('v').trim();
    if latest.is_empty() {
        return Ok(not_available);
    }

    if !is_newer(latest, CURRENT_VERSION) {
        return Ok(not_available);
    }

    // 找 .exe 安装包 asset
    let download_url = release
        .assets
        .iter()
        .find(|a| a.name.ends_with("-setup.exe") || a.name.ends_with(".exe"))
        .map(|a| a.browser_download_url.clone());

    Ok(UpdateInfo {
        available: true,
        latest_version: latest.to_string(),
        current_version: CURRENT_VERSION.to_string(),
        release_url: release
            .html_url
            .unwrap_or_else(|| format!("https://gitee.com/kz_y/naxone/releases/tag/{}", release.tag_name)),
        release_notes: release.body.unwrap_or_default(),
        download_url,
    })
}

/// 简单 semver 比较：`latest > current` 返回 true。
/// 支持 "0.2.0" > "0.1.0"、"1.0.0" > "0.99.99" 等。
fn is_newer(latest: &str, current: &str) -> bool {
    let parse = |s: &str| -> Vec<u32> {
        s.split('.')
            .map(|seg| seg.parse::<u32>().unwrap_or(0))
            .collect()
    };
    let l = parse(latest);
    let c = parse(current);
    for i in 0..l.len().max(c.len()) {
        let lv = l.get(i).copied().unwrap_or(0);
        let cv = c.get(i).copied().unwrap_or(0);
        if lv > cv {
            return true;
        }
        if lv < cv {
            return false;
        }
    }
    false // equal
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_comparison() {
        assert!(is_newer("0.2.0", "0.1.0"));
        assert!(is_newer("1.0.0", "0.99.99"));
        assert!(is_newer("0.1.1", "0.1.0"));
        assert!(!is_newer("0.1.0", "0.1.0")); // equal
        assert!(!is_newer("0.1.0", "0.2.0")); // older
        assert!(!is_newer("0.0.9", "0.1.0"));
    }
}
