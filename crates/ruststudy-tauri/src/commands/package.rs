//! Tauri commands powering the in-app software store.
//!
//! The list and installed endpoints are synchronous-cheap. `install_package`
//! spawns a background task and streams progress to the frontend via the
//! `install-progress` event name.

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, State};

use crate::commands::logger::push_log;
use crate::state::{resolve_packages_root, AppState};
use ruststudy_adapters::package::cache::DiskCache;
use ruststudy_adapters::package::installer::{phpstudy_style_dir_name, InstallEvent, Installer};
use ruststudy_adapters::package::manifest::{load_manifest, PackageEntry, PackageVersion};
use ruststudy_adapters::package::sources::{github_mirror, php_official};

/// How long upstream version indices stay fresh on disk.
const PACKAGE_CACHE_TTL: std::time::Duration = std::time::Duration::from_secs(6 * 3600);

/// Cached PHP releases file (relative to the cache dir).
const PHP_CACHE_FILE: &str = "php-releases.json";
/// Cached full mirror manifest (all software, all versions).
const MIRROR_CACHE_FILE: &str = "mirror-manifest.json";
use ruststudy_core::domain::log::LogLevel;

/// Returned by `get_installed_packages`. Mirrors what `StandaloneScanner` found
/// under `%APPDATA%/RustStudy/Packages/`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstalledPackage {
    pub name: String,
    pub version: String,
    pub install_path: String,
}

/// Returns the package catalog: embedded manifest, enriched with live data
/// from upstream sources where available (currently: PHP via windows.php.net).
///
/// Lookup order for each dynamic source:
///   1. Fresh disk cache (< 6h old) — instant
///   2. Live fetch from upstream — store in cache on success
///   3. Stale disk cache — fallback when upstream is unreachable
///   4. Embedded-only — ultimate fallback
#[tauri::command]
pub async fn list_packages() -> Result<Vec<PackageEntry>, String> {
    list_packages_impl(false).await
}

/// Forces a refresh of dynamic sources, bypassing the cache. The frontend's
/// "🔄 刷新" button calls this to pull the very latest PHP list on demand.
#[tauri::command]
pub async fn refresh_package_index() -> Result<Vec<PackageEntry>, String> {
    list_packages_impl(true).await
}

async fn list_packages_impl(force: bool) -> Result<Vec<PackageEntry>, String> {
    Ok(load_merged_manifest(force).await.packages)
}

/// 加载内嵌清单 + 合并**镜像源**（GitHub 镜像仓库）最新版本。
/// 策略：
///   1. 先拉 github_mirror（全套软件 + 多 URL 下载链路）
///   2. 镜像失败 → 退化到 php_official 直连 php.net（只有 PHP）
///   3. 两者都失败 → 只用内嵌 packages.json
/// 镜像版本会**覆盖**同 version 的内嵌版本（因为镜像带 download_urls 多路加速）。
async fn load_merged_manifest(force: bool) -> crate::commands::package::manifest_types::Manifest {
    let mut manifest = load_manifest();

    // 1) 尝试镜像
    if let Some(mirror) = load_mirror_manifest(force).await {
        for (software, versions) in mirror {
            if let Some(pkg) = manifest.packages.iter_mut().find(|p| p.name == software) {
                merge_versions(&mut pkg.versions, versions);
            }
            // 镜像里有但内嵌没定义的软件 → 忽略（UI 元数据无从生成）
        }
        return manifest;
    }

    // 2) 镜像不可用 → 退到 php_official（仅 PHP）
    if let Some(live_versions) = load_php_versions(force).await {
        if let Some(php) = manifest.packages.iter_mut().find(|p| p.name == "php") {
            merge_versions(&mut php.versions, live_versions);
        }
    }

    manifest
}

/// 拉镜像 manifest，带 6h 本地缓存兜底。
async fn load_mirror_manifest(
    force: bool,
) -> Option<std::collections::HashMap<String, Vec<PackageVersion>>> {
    let cache = DiskCache::new(cache_dir().join(MIRROR_CACHE_FILE), PACKAGE_CACHE_TTL);
    if !force {
        if let Some(cached) = cache.read_fresh::<std::collections::HashMap<String, Vec<PackageVersion>>>() {
            tracing::debug!("Mirror manifest served from fresh cache");
            return Some(cached);
        }
    } else {
        cache.invalidate();
    }
    match github_mirror::fetch().await {
        Ok(fresh) => {
            if let Err(e) = cache.write(&fresh) {
                tracing::warn!("Failed to persist mirror cache: {}", e);
            }
            Some(fresh)
        }
        Err(e) => {
            tracing::warn!("镜像 manifest 获取失败: {}。尝试 stale 缓存。", e);
            cache.read_stale::<std::collections::HashMap<String, Vec<PackageVersion>>>()
        }
    }
}

/// Re-export the Manifest type path so `load_merged_manifest` has a visible
/// return type without cluttering the public API. `load_manifest()` lives in
/// the adapters crate — this is just a thin alias to keep the signature stable.
mod manifest_types {
    pub type Manifest = ruststudy_adapters::package::manifest::Manifest;
}

/// Resolve PHP versions with cache-first strategy.
async fn load_php_versions(force: bool) -> Option<Vec<PackageVersion>> {
    let cache = php_cache();

    if !force {
        if let Some(cached) = cache.read_fresh::<Vec<PackageVersion>>() {
            tracing::debug!("PHP versions served from fresh cache");
            return Some(cached);
        }
    } else {
        cache.invalidate();
    }

    match php_official::fetch().await {
        Ok(fresh) => {
            if let Err(e) = cache.write(&fresh) {
                tracing::warn!("Failed to persist PHP cache: {}", e);
            }
            Some(fresh)
        }
        Err(e) => {
            tracing::warn!("PHP upstream failed: {}. Trying stale cache.", e);
            cache.read_stale::<Vec<PackageVersion>>()
        }
    }
}

fn php_cache() -> DiskCache {
    DiskCache::new(cache_dir().join(PHP_CACHE_FILE), PACKAGE_CACHE_TTL)
}

fn cache_dir() -> std::path::PathBuf {
    let home = std::env::var("USERPROFILE").unwrap_or_else(|_| "C:\\Users\\Default".into());
    std::path::PathBuf::from(home).join(".ruststudy").join("cache")
}

/// Merge 镜像版本到现有列表。策略：
/// - 镜像的版本**覆盖**内嵌同 version（镜像带 `download_urls`，是带加速链路的版本）
/// - 镜像不包含的内嵌版本保留（EOL 版本等）
/// - 合并后**按版本号降序整体排序**，避免镜像段和内嵌段视觉上出现 7.4 → 8.4 跳跃
fn merge_versions(existing: &mut Vec<PackageVersion>, incoming: Vec<PackageVersion>) {
    let incoming_versions: std::collections::HashSet<String> =
        incoming.iter().map(|v| v.version.clone()).collect();

    let mut result: Vec<PackageVersion> = Vec::with_capacity(existing.len() + incoming.len());
    result.extend(incoming);
    for v in existing.drain(..) {
        if !incoming_versions.contains(&v.version) {
            result.push(v);
        }
    }
    // 统一按语义版本降序排列（8.5.5 > 8.4.20 > 8.4.2 > 8.1.27 > 7.4.33）
    result.sort_by(|a, b| semver_cmp_desc(&a.version, &b.version));
    *existing = result;
}

/// 两段版本号按数字段降序比较。"8.4.20" > "8.4.2" > "8.3.30"。
/// 非数字段落到字符串对比。
fn semver_cmp_desc(a: &str, b: &str) -> std::cmp::Ordering {
    let mut ai = a.split('.');
    let mut bi = b.split('.');
    loop {
        match (ai.next(), bi.next()) {
            (None, None) => return std::cmp::Ordering::Equal,
            (Some(_), None) => return std::cmp::Ordering::Less, // 长版本 > 短版本 → 降序先来
            (None, Some(_)) => return std::cmp::Ordering::Greater,
            (Some(x), Some(y)) => match (x.parse::<u32>(), y.parse::<u32>()) {
                (Ok(xn), Ok(yn)) if xn != yn => return yn.cmp(&xn), // 降序
                (Ok(xn), Ok(yn)) if xn == yn => continue,
                _ => {
                    // 一段含非数字 → 字符串对比（降序）
                    let ord = y.cmp(x);
                    if ord != std::cmp::Ordering::Equal {
                        return ord;
                    }
                }
            },
        }
    }
}

/// Report which packages are installed via the store. Derived from the
/// already-scanned service list — keeps a single source of truth and handles
/// both the new PHPStudy-style layout and legacy `%APPDATA%/Packages/` dirs
/// for free.
///
/// The returned `name` uses the manifest key convention (lowercase:
/// "nginx"/"mysql"/"php"/...), and `version` is the parsed semver string.
#[tauri::command]
pub async fn get_installed_packages(
    state: State<'_, AppState>,
) -> Result<Vec<InstalledPackage>, String> {
    use ruststudy_core::domain::service::{ServiceKind, ServiceOrigin};

    let services = state.services.read().await;
    let mut out: Vec<InstalledPackage> = Vec::new();

    for svc in services.iter() {
        // Only count services sourced from the store or legacy Packages root.
        // Exclude PhpStudy origin: PHPStudy's own installs aren't "ours".
        match svc.origin {
            ServiceOrigin::Store | ServiceOrigin::Manual => {}
            ServiceOrigin::PhpStudy => continue,
        }

        let name = match svc.kind {
            ServiceKind::Nginx => "nginx",
            ServiceKind::Apache => "apache",
            ServiceKind::Mysql => "mysql",
            ServiceKind::Redis => "redis",
            ServiceKind::Php => "php",
        };

        out.push(InstalledPackage {
            name: name.to_string(),
            version: svc.version.clone(),
            install_path: svc.install_path.display().to_string(),
        });
    }

    Ok(out)
}

/// Kick off an install in the background. Returns immediately after validation;
/// progress and completion come via the `install-progress` Tauri event.
#[tauri::command]
pub async fn install_package(
    name: String,
    version: String,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    use ruststudy_core::domain::service::ServiceKind;

    let manifest = load_merged_manifest(false).await;
    let pkg = manifest
        .packages
        .iter()
        .find(|p| p.name == name)
        .ok_or_else(|| format!("未知软件: {}", name))?
        .clone();
    let ver = pkg
        .versions
        .iter()
        .find(|v| v.version == version)
        .ok_or_else(|| format!("未知版本: {} v{}", name, version))?
        .clone();

    // Guard: if this exact (kind, version) service is already running, refuse
    // the install cleanly — overwriting would race with the OS file lock and
    // produce a cryptic "directory can't be deleted" error deep in the
    // installer. Catching it here lets us return a friendly message.
    let target_kind = match name.as_str() {
        "nginx" => Some(ServiceKind::Nginx),
        "apache" => Some(ServiceKind::Apache),
        "mysql" => Some(ServiceKind::Mysql),
        "redis" => Some(ServiceKind::Redis),
        "php" => Some(ServiceKind::Php),
        _ => None,
    };
    if let Some(kind) = target_kind {
        let services = state.services.read().await;
        let running_conflict = services
            .iter()
            .any(|s| s.kind == kind && s.version == version && s.status.is_running());
        drop(services);
        if running_conflict {
            let msg = format!(
                "{} v{} 正在运行，请先停止后再重新安装",
                pkg.display_name, version
            );
            push_log(&state, LogLevel::Warn, "store", msg.clone(), None, None).await;
            return Err(msg);
        }
    }

    let config = state.config.read().await;
    let packages_root = resolve_packages_root(&config);
    drop(config);

    // Make sure the root exists before spawning (early-fail in the main task so
    // errors surface to the caller immediately).
    if let Err(e) = std::fs::create_dir_all(&packages_root) {
        return Err(format!(
            "创建包安装根目录失败 {}: {}",
            packages_root.display(),
            e
        ));
    }

    push_log(
        &state,
        LogLevel::Info,
        "store",
        format!("开始安装 {} v{}", pkg.display_name, version),
        None,
        None,
    )
    .await;

    // Spawn the install in the background. The Installer emits InstallEvents via
    // an mpsc channel; a forwarder task pumps each event onto the Tauri bus.
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<InstallEvent>();
    let installer = Installer::new(packages_root);
    let state_snap = std::sync::Arc::new(state.inner().clone_shallow());

    tauri::async_runtime::spawn(async move {
        let _ = installer.install(&pkg, &ver, tx).await;
    });

    let app_handle = app.clone();
    tauri::async_runtime::spawn(async move {
        while let Some(event) = rx.recv().await {
            // Log terminal events
            match &event {
                InstallEvent::Done { name, version, .. } => {
                    push_log(
                        &state_snap,
                        LogLevel::Success,
                        "store",
                        format!("{} v{} 安装成功", name, version),
                        None,
                        None,
                    )
                    .await;
                    // Trigger a services rescan so the newly installed package
                    // shows up in the Dashboard.
                    let _ = rescan_services_inner(&state_snap).await;
                    // 主动通知前端刷新，不用等 5s 轮询
                    let _ = app_handle.emit("services-changed", ());
                }
                InstallEvent::Failed {
                    name,
                    version,
                    reason,
                } => {
                    push_log(
                        &state_snap,
                        LogLevel::Error,
                        "store",
                        format!("{} v{} 安装失败", name, version),
                        Some(reason.clone()),
                        None,
                    )
                    .await;
                }
                _ => {}
            }

            let _ = app_handle.emit("install-progress", &event);
        }
    });

    Ok(())
}

/// Internal rescan after install — mirrors `commands::settings::rescan_services`
/// but operates on a shallow-cloned state handle (no Tauri State required).
async fn rescan_services_inner(
    state: &std::sync::Arc<AppState>,
) -> Result<(), String> {
    use ruststudy_adapters::config::fs_config::FsConfigIO;
    use ruststudy_adapters::package::composite::CompositeScanner;
    use ruststudy_adapters::vhost::VhostScanner;
    use ruststudy_core::use_cases::vhost_mgr::VhostManager;

    let config = state.config.read().await;
    let phpstudy_opt = config.general.phpstudy_path.clone();
    let extras = config.general.extra_install_paths.clone();
    let store_ext = resolve_packages_root(&config);
    let legacy_root = crate::state::legacy_packages_root();
    drop(config);

    let ext_path = phpstudy_opt.as_ref().map(|p| p.join("Extensions"));
    let new_services = CompositeScanner::scan(
        ext_path.as_deref(),
        Some(&store_ext),
        &extras,
        Some(&legacy_root),
    );

    let saved_vhosts = VhostManager::load_vhosts_json(&crate::state::vhosts_json_path());
    let scanned_vhosts = if let Some(ref phpstudy_path) = phpstudy_opt {
        let ext_path = phpstudy_path.join("Extensions");
        let nginx_dir = std::fs::read_dir(&ext_path).ok().and_then(|rd| {
            rd.flatten()
                .find(|e| {
                    e.file_name().to_string_lossy().starts_with("Nginx") && e.path().is_dir()
                })
                .map(|e| e.path().join("conf").join("vhosts"))
        });
        if let Some(dir) = nginx_dir {
            VhostScanner::scan(&FsConfigIO, &dir, &new_services).unwrap_or_default()
        } else {
            Vec::new()
        }
    } else {
        Vec::new()
    };
    let merged = VhostManager::merge_vhosts(scanned_vhosts, saved_vhosts);

    {
        let mut services = state.services.write().await;
        *services = new_services;
    }
    {
        let mut vhosts = state.vhosts.write().await;
        *vhosts = merged;
    }
    Ok(())
}

/// Remove an installed package. Only operates on the store root —
/// PHPStudy-native and user-manual installs are off-limits (we didn't
/// put those there).
///
/// Safety checks:
///   1. The manifest must know this (name, version) — no wildcard deletes.
///   2. The matching service must not be running.
///   3. The target directory must be under our packages root (no escape).
#[tauri::command]
pub async fn uninstall_package(
    name: String,
    version: String,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    use ruststudy_core::domain::service::ServiceKind;

    let manifest = load_merged_manifest(false).await;
    let pkg = manifest
        .packages
        .iter()
        .find(|p| p.name == name)
        .ok_or_else(|| format!("未知软件: {}", name))?
        .clone();
    let _ver = pkg
        .versions
        .iter()
        .find(|v| v.version == version)
        .ok_or_else(|| format!("未知版本: {} v{}", name, version))?;

    let target_kind = match name.as_str() {
        "nginx" => ServiceKind::Nginx,
        "apache" => ServiceKind::Apache,
        "mysql" => ServiceKind::Mysql,
        "redis" => ServiceKind::Redis,
        "php" => ServiceKind::Php,
        _ => return Err(format!("未知软件类型: {}", name)),
    };

    // Origin 校验 + 运行检查（一次读锁搞定）
    {
        use ruststudy_core::domain::service::ServiceOrigin;
        let services = state.services.read().await;
        let matches: Vec<_> = services
            .iter()
            .filter(|s| s.kind == target_kind && s.version == version)
            .collect();
        // 若该版本只存在 PhpStudy 源（没有商店源副本），明确拒绝
        // 仅靠下面的 packages_root 路径边界也能挡住，但先给用户友好提示
        if !matches.is_empty() && matches.iter().all(|s| s.origin == ServiceOrigin::PhpStudy) {
            let msg = "不能卸载 PHPStudy 自带的软件".to_string();
            push_log(&state, LogLevel::Warn, "store", msg.clone(), None, None).await;
            return Err(msg);
        }
        let running = matches.iter().any(|s| s.status.is_running());
        if running {
            let msg = format!("{} v{} 正在运行，请先停止后再卸载", pkg.display_name, version);
            push_log(&state, LogLevel::Warn, "store", msg.clone(), None, None).await;
            return Err(msg);
        }
    }

    // Compute the target dir under our packages root
    let config = state.config.read().await;
    let packages_root = resolve_packages_root(&config);
    drop(config);
    let dir_name = phpstudy_style_dir_name(&name, &version);
    let target_dir = packages_root.join(&dir_name);

    // Boundary: the resolved path must still live under packages_root
    if !target_dir.starts_with(&packages_root) {
        return Err("安全检查失败：目标路径逃出安装根".into());
    }

    if !target_dir.exists() {
        let msg = format!("{} v{} 未安装（目录不存在）", pkg.display_name, version);
        push_log(&state, LogLevel::Info, "store", msg.clone(), None, None).await;
        return Err(msg);
    }

    // Actually delete
    match std::fs::remove_dir_all(&target_dir) {
        Ok(_) => {
            push_log(
                &state,
                LogLevel::Success,
                "store",
                format!("{} v{} 已卸载", pkg.display_name, version),
                Some(format!("路径: {}", target_dir.display())),
                None,
            )
            .await;
        }
        Err(e) => {
            let msg = format!(
                "卸载 {} v{} 失败: {} (可能有文件被占用)",
                pkg.display_name, version, e
            );
            push_log(&state, LogLevel::Error, "store", msg.clone(), None, None).await;
            return Err(msg);
        }
    }

    // PHP parent dir: if we just removed the last php/phpXXXnts/ subdir,
    // the shared `php/` directory is now empty. Clean it up for tidiness
    // (best-effort; ignore errors).
    if name == "php" {
        let php_parent = packages_root.join("php");
        if let Ok(mut rd) = std::fs::read_dir(&php_parent) {
            if rd.next().is_none() {
                let _ = std::fs::remove_dir(&php_parent);
            }
        }
    }

    // Refresh services list so the Dashboard drops the removed card
    // immediately. Use the shallow-cloned state pattern used elsewhere.
    let state_snap = std::sync::Arc::new(state.inner().clone_shallow());
    let _ = rescan_services_inner(&state_snap).await;
    let _ = app.emit("services-changed", ());

    Ok(())
}
