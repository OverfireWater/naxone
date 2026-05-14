//! 检测"陌生"进程：占着我们关心的端口（80/443/3306/6379）但 exe 路径不在已扫描安装目录里的。
//! 用途：用户系统里有第三方/独立装的 nginx/mysql/redis 跑着，没在 NaxOne 的安装列表中，
//! 会和我们管理的服务冲突。前端把这些拎出来告诉用户，让用户决定要不要 kill。

use std::path::PathBuf;

use serde::Serialize;
use tauri::State;

use crate::commands::logger::push_log;
use crate::state::AppState;
use naxone_adapters::process::windows::{get_process_exe_path, netstat_snapshot};
use naxone_core::domain::log::LogLevel;

#[derive(Serialize, Clone, Debug)]
pub struct StrangerService {
    pub kind: String,
    pub port: u16,
    pub pid: u32,
    pub exe_path: String,
    pub label: String,
}

/// 端口 → 期望 kind 的映射。同 kind 不同 exe 名也都列上（如 nginx 走 80/443，apache 也是）。
const WATCHED_PORTS: &[(u16, &str)] = &[
    (80, "web"),     // nginx 或 apache
    (443, "web"),
    (3306, "mysql"),
    (6379, "redis"),
];

fn classify_exe(exe_lower: &str) -> Option<&'static str> {
    if exe_lower.contains("nginx.exe") {
        Some("Nginx")
    } else if exe_lower.contains("httpd.exe") {
        Some("Apache")
    } else if exe_lower.contains("mysqld.exe") || exe_lower.contains("mariadbd.exe") {
        Some("MySQL")
    } else if exe_lower.contains("redis-server.exe") {
        Some("Redis")
    } else {
        None
    }
}

#[tauri::command]
pub async fn scan_running_strangers(
    state: State<'_, AppState>,
) -> Result<Vec<StrangerService>, String> {
    // 1. 已知安装路径集合（每条都规范化到 lowercase）
    let known_paths: Vec<String> = state
        .services
        .read()
        .await
        .iter()
        .map(|s| s.install_path.to_string_lossy().to_lowercase())
        .filter(|s| !s.is_empty())
        .collect();

    // 2. 一次 netstat 拿所有 LISTENING 端口 → PID
    let snap = netstat_snapshot().await;

    let mut out = Vec::new();
    for (port, _purpose) in WATCHED_PORTS {
        let Some(&pid) = snap.get(port) else { continue };
        if pid == 0 {
            continue;
        }
        let Some(exe_path) = get_process_exe_path(pid).await else { continue };
        let exe_lower = exe_path.to_lowercase();

        // 3. 是不是我们认识的 kind
        let Some(kind) = classify_exe(&exe_lower) else { continue };

        // 4. exe 是否落在某个已扫到的 install_path 下 → 是 = 已知服务，不算陌生
        let exe_path_buf = PathBuf::from(&exe_path);
        let exe_dir_lower = exe_path_buf
            .parent()
            .map(|p| p.to_string_lossy().to_lowercase())
            .unwrap_or_default();
        let in_known = known_paths
            .iter()
            .any(|p| !p.is_empty() && (exe_dir_lower == *p || exe_dir_lower.starts_with(&format!("{}/", p)) || exe_dir_lower.starts_with(&format!("{}\\", p))));
        if in_known {
            continue;
        }

        let label = format!(
            "{} 占用端口 {}（{}, PID {}）",
            kind, port, exe_path, pid
        );
        out.push(StrangerService {
            kind: kind.to_string(),
            port: *port,
            pid,
            exe_path,
            label,
        });
    }

    Ok(out)
}

#[tauri::command]
pub async fn kill_stranger(pid: u32, state: State<'_, AppState>) -> Result<(), String> {
    if pid == 0 {
        return Err("无效 PID".into());
    }

    // 白名单：只允许杀已知 web/db 工具进程，禁止误杀系统进程（svchost、explorer 等）
    let exe_name = naxone_adapters::process::windows::get_process_name(pid).await
        .unwrap_or_default()
        .to_lowercase();
    const ALLOWED_EXES: &[&str] = &[
        "nginx.exe",
        "httpd.exe",
        "mysqld.exe",
        "mysqld-nt.exe",
        "redis-server.exe",
        "php-cgi.exe",
        "php.exe",
        "xp.cn_cgi.exe", // PHPStudy 的 php-cgi 改名
    ];
    if exe_name.is_empty() {
        return Err(format!("无法读取 PID {} 的进程信息（可能已退出或权限不足）", pid));
    }
    if !ALLOWED_EXES.iter().any(|allowed| &exe_name == allowed) {
        return Err(format!(
            "拒绝结束 PID {}（{}）：不在 NaxOne 可管理的服务进程白名单内",
            pid, exe_name
        ));
    }

    let out = {
        let mut cmd = std::process::Command::new("taskkill");
        cmd.args(["/F", "/PID", &pid.to_string()]);
        #[cfg(target_os = "windows")]
        {
            use std::os::windows::process::CommandExt;
            cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
        }
        cmd.output().map_err(|e| format!("调用 taskkill 失败: {}", e))?
    };
    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr).trim().to_string();
        let lower = stderr.to_lowercase();
        let msg = if lower.contains("access is denied") || stderr.contains("拒绝访问") {
            format!("结束 PID {} 失败：权限不足（请以管理员身份运行 NaxOne）", pid)
        } else if lower.contains("not be found") || stderr.contains("找不到") {
            // 已经退出了 → 视为成功
            push_log(
                &state,
                LogLevel::Info,
                "stranger",
                format!("外部进程 PID {} 已不存在（无需结束）", pid),
                None,
                None,
            )
            .await;
            return Ok(());
        } else {
            format!("结束 PID {} 失败: {}", pid, stderr)
        };
        push_log(
            &state,
            LogLevel::Error,
            "stranger",
            format!("结束外部进程 PID {} 失败", pid),
            Some(stderr),
            None,
        )
        .await;
        return Err(msg);
    }

    push_log(
        &state,
        LogLevel::Success,
        "stranger",
        format!("已结束外部进程 PID {}", pid),
        None,
        None,
    )
    .await;
    Ok(())
}
