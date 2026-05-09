//! 站点模板初始化：在新建 vhost 后给空目录铺起始代码。
//!
//! 流程：前端创建 vhost 成功后，若用户选了非 None 模板，调用 init_site_template。
//! 后端按模板分发，过程中通过 Tauri event "site-template-log" 流式推日志到前端。

use std::path::{Path, PathBuf};
use std::process::Stdio;

use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command as TokioCommand;

const CREATE_NO_WINDOW: u32 = 0x0800_0000;
const WP_ZIP_URL: &str = "https://cn.wordpress.org/latest-zh_CN.zip";

#[derive(Debug, Deserialize, Clone, Copy)]
#[serde(rename_all = "lowercase")]
pub enum SiteTemplate {
    Blank,
    Wordpress,
    Laravel,
    Thinkphp,
}

fn emit_log(app: &AppHandle, line: String) {
    let _ = app.emit("site-template-log", line);
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct ProgressEvent {
    phase: &'static str,
    current: Option<u64>,
    total: Option<u64>,
    unit: Option<&'static str>,
}

fn emit_progress(
    app: &AppHandle,
    phase: &'static str,
    current: Option<u64>,
    total: Option<u64>,
    unit: Option<&'static str>,
) {
    let _ = app.emit(
        "site-template-progress",
        ProgressEvent { phase, current, total, unit },
    );
}

#[tauri::command]
pub async fn init_site_template(
    app: AppHandle,
    target_dir: String,
    template: SiteTemplate,
) -> Result<(), String> {
    let dir = PathBuf::from(&target_dir);
    if !dir.exists() {
        return Err(format!("目录不存在: {}", target_dir));
    }
    // 校验目录为空 —— 避免覆盖用户已有项目
    let mut entries = std::fs::read_dir(&dir).map_err(|e| format!("读取目录失败: {}", e))?;
    if entries.next().is_some() {
        return Err("目录非空，请先清空再选择模板".into());
    }

    emit_log(&app, format!("▶ 开始初始化模板: {:?}", template));
    emit_log(&app, format!("目标目录: {}", dir.display()));

    let result = match template {
        SiteTemplate::Blank => {
            emit_progress(&app, "running", None, None, None);
            init_blank(&app, &dir).await
        }
        SiteTemplate::Wordpress => init_wordpress(&app, &dir).await,
        SiteTemplate::Laravel => {
            emit_progress(&app, "running", None, None, None);
            init_composer(&app, &dir, "laravel/laravel").await
        }
        SiteTemplate::Thinkphp => {
            emit_progress(&app, "running", None, None, None);
            init_composer(&app, &dir, "topthink/think").await
        }
    };

    match &result {
        Ok(_) => emit_log(&app, "✔ 初始化完成".to_string()),
        Err(e) => emit_log(&app, format!("✗ 初始化失败: {}", e)),
    }
    result
}

async fn init_blank(app: &AppHandle, dir: &Path) -> Result<(), String> {
    let path = dir.join("index.php");
    let body = "<?php\nphpinfo();\n";
    std::fs::write(&path, body).map_err(|e| format!("写入 index.php 失败: {}", e))?;
    emit_log(app, format!("已创建 {}", path.display()));
    Ok(())
}

async fn init_wordpress(app: &AppHandle, dir: &Path) -> Result<(), String> {
    emit_log(app, format!("下载 WordPress: {}", WP_ZIP_URL));
    emit_progress(app, "downloading", Some(0), None, Some("MB"));

    let resp = reqwest::get(WP_ZIP_URL)
        .await
        .map_err(|e| format!("下载失败: {}", e))?;
    if !resp.status().is_success() {
        return Err(format!("下载失败: HTTP {}", resp.status()));
    }
    let total = resp.content_length();
    emit_progress(app, "downloading", Some(0), total, Some("MB"));

    // 流式下载：每 1MB emit 一次进度，让用户看到下载在动
    let mut bytes: Vec<u8> = Vec::with_capacity(total.unwrap_or(25 * 1024 * 1024) as usize);
    let mut stream = resp.bytes_stream();
    let mut last_emit: u64 = 0;
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| format!("下载中断: {}", e))?;
        bytes.extend_from_slice(&chunk);
        let cur = bytes.len() as u64;
        if cur - last_emit >= 1024 * 1024 {
            emit_progress(app, "downloading", Some(cur), total, Some("MB"));
            last_emit = cur;
        }
    }
    let downloaded = bytes.len() as u64;
    emit_progress(app, "downloading", Some(downloaded), total.or(Some(downloaded)), Some("MB"));
    emit_log(
        app,
        format!("已下载 {:.1} MB，开始解压…", downloaded as f64 / 1024.0 / 1024.0),
    );

    // 解压：每 100 个 entry emit 一次进度
    let cursor = std::io::Cursor::new(bytes.as_slice());
    let mut archive =
        zip::ZipArchive::new(cursor).map_err(|e| format!("zip 解析失败: {}", e))?;
    let total_entries = archive.len() as u64;
    emit_progress(app, "extracting", Some(0), Some(total_entries), Some("文件"));

    let mut wrote = 0usize;
    for i in 0..archive.len() {
        let mut entry = archive
            .by_index(i)
            .map_err(|e| format!("读取 zip 条目 {} 失败: {}", i, e))?;
        let name = match entry.enclosed_name() {
            Some(p) => p.to_path_buf(),
            None => continue,
        };
        let mut comps = name.components();
        // 剥掉顶层 wordpress/
        match comps.next() {
            Some(c) if c.as_os_str() == "wordpress" => {}
            _ => continue,
        }
        let rel: PathBuf = comps.collect();
        if rel.as_os_str().is_empty() {
            continue;
        }
        let dest = dir.join(rel);

        if entry.is_dir() {
            std::fs::create_dir_all(&dest)
                .map_err(|e| format!("创建目录 {} 失败: {}", dest.display(), e))?;
        } else {
            if let Some(parent) = dest.parent() {
                std::fs::create_dir_all(parent)
                    .map_err(|e| format!("创建父目录 {} 失败: {}", parent.display(), e))?;
            }
            let mut out = std::fs::File::create(&dest)
                .map_err(|e| format!("创建文件 {} 失败: {}", dest.display(), e))?;
            std::io::copy(&mut entry, &mut out)
                .map_err(|e| format!("写入 {} 失败: {}", dest.display(), e))?;
            wrote += 1;
        }
        let done = (i + 1) as u64;
        if done % 100 == 0 {
            emit_progress(app, "extracting", Some(done), Some(total_entries), Some("文件"));
        }
    }
    emit_progress(app, "extracting", Some(total_entries), Some(total_entries), Some("文件"));

    emit_log(app, format!("已解压 {} 个文件", wrote));
    emit_log(app, "提示：浏览器访问站点会进入 WordPress 安装向导".to_string());
    Ok(())
}

async fn init_composer(app: &AppHandle, dir: &Path, package: &str) -> Result<(), String> {
    emit_log(
        app,
        format!("执行: composer create-project {} . --no-interaction", package),
    );

    // Windows 上 composer 通常是 .bat，必须经 cmd.exe 才能执行；
    // 同时 cmd.exe 会按 PATH 解析 composer，省去自己定位 composer.exe/.bat 的麻烦。
    let mut cmd = TokioCommand::new("cmd.exe");
    cmd.current_dir(dir)
        .arg("/C")
        .arg("composer")
        .arg("create-project")
        .arg(package)
        .arg(".")
        .arg("--no-interaction")
        .creation_flags(CREATE_NO_WINDOW)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let mut child = cmd
        .spawn()
        .map_err(|e| format!("启动 composer 失败（PATH 中可能没有 composer）: {}", e))?;
    let stdout = child.stdout.take().ok_or("无法获取 stdout")?;
    let stderr = child.stderr.take().ok_or("无法获取 stderr")?;

    let app_out = app.clone();
    let stdout_task = tokio::spawn(async move {
        let mut lines = BufReader::new(stdout).lines();
        while let Ok(Some(line)) = lines.next_line().await {
            let _ = app_out.emit("site-template-log", line);
        }
    });
    let app_err = app.clone();
    let stderr_task = tokio::spawn(async move {
        let mut lines = BufReader::new(stderr).lines();
        while let Ok(Some(line)) = lines.next_line().await {
            let _ = app_err.emit("site-template-log", format!("[stderr] {}", line));
        }
    });

    let status = child
        .wait()
        .await
        .map_err(|e| format!("等待 composer 失败: {}", e))?;
    let _ = stdout_task.await;
    let _ = stderr_task.await;

    if !status.success() {
        return Err(format!(
            "composer create-project 失败 (exit {})\n请确认「全局环境」已配置 composer 且 PHP 在 PATH 中可用",
            status
        ));
    }
    Ok(())
}
