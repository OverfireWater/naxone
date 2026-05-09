//! PHP 扩展安装（PIE 路线）。
//!
//! 调用流程：
//! 1. 找一个 PHP ≥ 8.1 当作 PIE runtime（PIE 自己最低需 8.1）
//! 2. 调 `php pie.phar search <kw>` / `pie install ... --with-php-path=<目标 php>`
//! 3. 装好后由前端再触发 reload php-fpm

use std::path::PathBuf;
use std::process::Stdio;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command as TokioCommand;

use crate::commands::logger::push_log;
use crate::state::AppState;
use naxone_core::domain::log::LogLevel;
use naxone_core::domain::service::{ServiceInstance, ServiceKind, ServiceOrigin};

const CREATE_NO_WINDOW: u32 = 0x0800_0000;

#[derive(Serialize)]
pub struct PieExtensionInfo {
    pub name: String,
    pub description: String,
}

#[derive(Serialize)]
pub struct PieRuntimeInfo {
    /// 找得到 PHP ≥ 8.1 时返回它的 install_path（PIE 用它来跑），否则 None
    pub runtime_php_path: Option<String>,
    pub runtime_version: Option<String>,
}

/// 解析 PHP 版本（如 "8.1.10" / "7.4.3"）为 (major, minor) 二元组
fn parse_version(v: &str) -> Option<(u32, u32)> {
    let mut parts = v.split('.');
    let major = parts.next()?.parse().ok()?;
    let minor = parts.next()?.parse().ok()?;
    Some((major, minor))
}

/// 在已安装服务中找 PHP ≥ 8.1，返回 (install_path, version)。
/// 优先 NaxOne 软件商店装的（Store），回退到 PhpStudy / Manual。
/// 软件商店装包时已 harden ini（启用 openssl/curl/mbstring/zip/fileinfo 等），
/// 与 PHPStudy 自带配置同等可用。
fn find_pie_runtime(services: &[ServiceInstance]) -> Option<(PathBuf, String)> {
    let candidates: Vec<_> = services
        .iter()
        .filter(|s| s.kind == ServiceKind::Php)
        .filter_map(|s| {
            let v = parse_version(&s.version)?;
            if v >= (8, 1) {
                Some((v, s.install_path.clone(), s.version.clone(), s.origin.clone()))
            } else {
                None
            }
        })
        .collect();

    // 1. 优先 Store（NaxOne 自管，长期方向）
    if let Some((_, path, ver, _)) = candidates
        .iter()
        .filter(|(_, _, _, o)| matches!(o, ServiceOrigin::Store))
        .max_by_key(|(v, _, _, _)| *v)
    {
        return Some((path.clone(), ver.clone()));
    }

    // 2. 回退到任意来源最高版本
    candidates
        .into_iter()
        .max_by_key(|(v, _, _, _)| *v)
        .map(|(_, path, ver, _)| (path, ver))
}

/// Windows 上 `app.path().resolve()` 可能返回 verbatim 长路径 `\\?\D:\...`，
/// PHP 的 phar 流处理器不识别 → 必须 strip 掉。
fn strip_verbatim(p: PathBuf) -> PathBuf {
    let s = p.to_string_lossy();
    if let Some(rest) = s.strip_prefix(r"\\?\") {
        PathBuf::from(rest)
    } else {
        p
    }
}

fn pie_phar_path(app: &AppHandle) -> Result<PathBuf, String> {
    // 正常路径：BaseDirectory::Resource（release 走这里）
    if let Ok(p) = app
        .path()
        .resolve("resources/pie.phar", tauri::path::BaseDirectory::Resource)
    {
        if p.exists() {
            return Ok(strip_verbatim(p));
        }
    }
    // Dev fallback：Tauri 2 dev 不一定把 resources copy 到 target/，直接用源目录
    #[cfg(debug_assertions)]
    {
        let dev = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources").join("pie.phar");
        if dev.exists() {
            return Ok(dev);
        }
    }
    Err("找不到 pie.phar，请检查 resources/pie.phar 是否存在".into())
}

#[tauri::command]
pub async fn pie_runtime_info(state: State<'_, AppState>) -> Result<PieRuntimeInfo, String> {
    let services = state.services.read().await;
    Ok(match find_pie_runtime(&services) {
        Some((path, ver)) => PieRuntimeInfo {
            runtime_php_path: Some(path.display().to_string()),
            runtime_version: Some(ver),
        },
        None => PieRuntimeInfo {
            runtime_php_path: None,
            runtime_version: None,
        },
    })
}

/// 走 Packagist 搜索 API（PIE 1.x 没有 search 子命令）。
/// 同时筛 type=php-ext 和 php-ext-zend-extension 两类。
#[tauri::command]
pub async fn pie_search(keyword: String) -> Result<Vec<PieExtensionInfo>, String> {
    let kw = keyword.trim();

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .map_err(|e| format!("创建 HTTP client 失败: {}", e))?;

    let mut all = Vec::<PieExtensionInfo>::new();
    let mut seen = std::collections::HashSet::<String>::new();
    for ext_type in ["php-ext", "php-ext-zend-extension"] {
        let resp = client
            .get("https://packagist.org/search.json")
            .query(&[("q", kw), ("type", ext_type), ("per_page", "30")])
            .send()
            .await
            .map_err(|e| format!("请求 Packagist 失败: {}", e))?;
        if !resp.status().is_success() {
            return Err(format!("Packagist 返回 {}", resp.status()));
        }
        let json: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| format!("解析 Packagist 响应失败: {}", e))?;
        if let Some(arr) = json.get("results").and_then(|v| v.as_array()) {
            for item in arr {
                let name = item
                    .get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                if name.is_empty() || !seen.insert(name.clone()) {
                    continue;
                }
                let description = item
                    .get("description")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                all.push(PieExtensionInfo { name, description });
            }
        }
    }
    Ok(all)
}

/// pie install <package> --with-php-path=<目标 php.exe>
/// target_php_install_path 是用户在 UI 里选的那个 PHP 实例的 install_path
#[tauri::command]
pub async fn pie_install(
    package: String,
    target_php_install_path: String,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<String, String> {
    let label = format!("装 PHP 扩展 {} 到 {}", package, target_php_install_path);
    push_log(&state, LogLevel::Info, "extension", format!("开始：{}", label), None, None).await;
    let result = pie_install_inner(package, target_php_install_path, &app, &state).await;
    match &result {
        Ok(combined) => {
            push_log(
                &state,
                LogLevel::Success,
                "extension",
                format!("完成：{}", label),
                Some(combined.clone()),
                None,
            )
            .await
        }
        Err(e) => {
            push_log(
                &state,
                LogLevel::Error,
                "extension",
                format!("失败：{}", label),
                Some(e.clone()),
                None,
            )
            .await
        }
    }
    result
}

async fn pie_install_inner(
    package: String,
    target_php_install_path: String,
    app: &AppHandle,
    state: &State<'_, AppState>,
) -> Result<String, String> {
    let services = state.services.read().await;
    let (runtime_php, _) = find_pie_runtime(&services)
        .ok_or_else(|| "未找到 PHP 8.1 或更高版本，PIE 无法运行。请先在软件商店安装 PHP ≥ 8.1。".to_string())?;
    drop(services);

    let phar = pie_phar_path(&app)?;
    let runtime_exe = runtime_php.join("php.exe");
    let target_exe = PathBuf::from(&target_php_install_path).join("php.exe");

    if !target_exe.exists() {
        return Err(format!("目标 PHP 不存在: {}", target_exe.display()));
    }

    // 显式指定 runtime PHP 的 php.ini，避免子进程加载不到。
    // 同时覆盖 extension_dir 为 runtime PHP 的 ext 目录 —— NaxOne 软件商店装的 PHP
    // 自带的 php.ini 可能把 extension_dir 写成默认值 C:\php\ext（错误路径），
    // 导致 openssl 等扩展加载失败。
    let runtime_ini = runtime_php.join("php.ini");
    let runtime_ext = runtime_php.join("ext");

    let mut cmd = TokioCommand::new(&runtime_exe);
    if runtime_ini.exists() {
        cmd.arg("-c").arg(&runtime_ini);
    }
    // 显式覆盖 extension_dir 并启用 PIE 必备扩展，
    // 让"裸 ini"的 PHP（NaxOne 商店历史装的、所有 ;extension= 都注释的）也能跑。
    cmd.arg("-d")
        .arg(format!("extension_dir={}", runtime_ext.display()));
    for ext in [
        "openssl",
        "curl",
        "mbstring",
        "fileinfo",
        "zip",
        "intl",
    ] {
        cmd.arg("-d").arg(format!("extension={}", ext));
    }
    // 流式：每行 stdout/stderr 通过 Tauri event "pie-install-log" 推到前端
    let mut child = cmd
        .arg(&phar)
        .arg("install")
        .arg(&package)
        .arg(format!("--with-php-path={}", target_exe.display()))
        .arg("--no-interaction")
        .creation_flags(CREATE_NO_WINDOW)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("启动 PIE 失败: {}", e))?;

    let stdout = child.stdout.take().ok_or("无法获取 PIE stdout")?;
    let stderr = child.stderr.take().ok_or("无法获取 PIE stderr")?;

    let app_out = app.clone();
    let collected = std::sync::Arc::new(tokio::sync::Mutex::new(String::new()));
    let collected_out = collected.clone();
    let stdout_task = tokio::spawn(async move {
        let mut lines = BufReader::new(stdout).lines();
        while let Ok(Some(line)) = lines.next_line().await {
            let _ = app_out.emit("pie-install-log", line.clone());
            let mut g = collected_out.lock().await;
            g.push_str(&line);
            g.push('\n');
        }
    });

    let app_err = app.clone();
    let collected_err = collected.clone();
    let stderr_task = tokio::spawn(async move {
        let mut lines = BufReader::new(stderr).lines();
        while let Ok(Some(line)) = lines.next_line().await {
            let _ = app_err.emit("pie-install-log", format!("[stderr] {}", line));
            let mut g = collected_err.lock().await;
            g.push_str("[stderr] ");
            g.push_str(&line);
            g.push('\n');
        }
    });

    let status = child
        .wait()
        .await
        .map_err(|e| format!("等待 PIE 进程失败: {}", e))?;
    let _ = stdout_task.await;
    let _ = stderr_task.await;

    let combined = collected.lock().await.clone();

    if !status.success() {
        let hint = humanize_pie_error(&combined);
        let prefix = match hint {
            Some(h) => format!("{}\n\n──── 原始日志 ────\n", h),
            None => format!("PIE install 失败 (exit {}):\n", status),
        };
        return Err(format!("{}{}", prefix, combined.trim()));
    }
    Ok(combined.trim().to_string())
}

/// 把 PIE 常见错误翻译成中文，帮用户区分"产品 bug"和"扩展生态局限"
fn humanize_pie_error(combined: &str) -> Option<String> {
    if combined.contains("Could not find release by tag name") {
        return Some(
            "❌ 该扩展作者未在 GitHub release 上传 Windows 安装包，PIE 无法安装。\n建议：换其他扩展，或等待作者上传 Windows binary。".into(),
        );
    }
    if combined.contains("Windows archive with prebuilt extension")
        && combined.contains("was not attached on release")
    {
        return Some(
            "❌ 该扩展未提供匹配你 PHP 版本的 Windows 安装包。\n建议：把目标 PHP 切到该扩展支持的版本（看 packagist 上的 README），或换其他扩展。".into(),
        );
    }
    if combined.contains("No package found:") {
        return Some(
            "❌ Packagist 找不到这个包名。检查包名是否写对（精选清单是验证过的，搜索结果可能不准）。".into(),
        );
    }
    if combined.contains("Composer detected issues in your platform") {
        return Some(
            "❌ PIE 检查 PHP 平台依赖失败。可能 runtime PHP 缺核心扩展（openssl/curl 等），NaxOne 已尝试覆盖但仍未通过。请检查 runtime PHP 的 ini。".into(),
        );
    }
    None
}
