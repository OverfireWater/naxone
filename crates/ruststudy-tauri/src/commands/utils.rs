use serde::Serialize;
use std::sync::LazyLock;
use std::time::Instant;

static APP_START: LazyLock<Instant> = LazyLock::new(Instant::now);

#[derive(Serialize)]
pub struct AppStats {
    pub pid: u32,
    pub memory_mb: Option<u64>,
    pub uptime_secs: u64,
}

/// RustStudy 自身的进程统计：PID / 工作集内存 / 运行时长
#[tauri::command]
pub async fn get_app_stats() -> Result<AppStats, String> {
    let pid = std::process::id();
    // 只有 Windows 查得到内存（借用 adapters 的 tasklist 辅助）
    #[cfg(target_os = "windows")]
    let memory_mb = {
        let map = ruststudy_adapters::process::windows::tasklist_memory_snapshot().await;
        map.get(&pid).copied()
    };
    #[cfg(not(target_os = "windows"))]
    let memory_mb: Option<u64> = None;

    let uptime_secs = APP_START.elapsed().as_secs();
    Ok(AppStats {
        pid,
        memory_mb,
        uptime_secs,
    })
}

/// Open a URL in the default browser using Windows ShellExecute (handles spaces & special chars)
#[tauri::command]
pub async fn open_in_browser(url: String) -> Result<(), String> {
    // Use rundll32 with url.dll - handles URLs with special chars reliably
    std::process::Command::new("rundll32.exe")
        .args(["url.dll,FileProtocolHandler", &url])
        .spawn()
        .map_err(|e| format!("无法打开浏览器: {}", e))?;
    Ok(())
}

/// Open a folder in the file explorer
#[tauri::command]
pub async fn open_folder(path: String) -> Result<(), String> {
    let win_path = path.replace('/', "\\");
    // Check path exists first
    if !std::path::Path::new(&win_path).exists() {
        return Err(format!("目录不存在: {}", win_path));
    }
    std::process::Command::new("explorer.exe")
        .arg(&win_path)
        .spawn()
        .map_err(|e| format!("无法打开目录: {}", e))?;
    Ok(())
}

/// Open a file with the default editor
#[tauri::command]
pub async fn open_file(path: String) -> Result<(), String> {
    if !std::path::Path::new(&path).exists() {
        return Err(format!("文件不存在: {}", path));
    }
    // rundll32 with shell32 handles any file type via default association
    std::process::Command::new("rundll32.exe")
        .args(["shell32.dll,ShellExec_RunDLL", &path])
        .spawn()
        .map_err(|e| format!("无法打开文件: {}", e))?;
    Ok(())
}

/// Get and clear startup errors (from auto-start)
#[tauri::command]
pub async fn get_startup_errors(state: tauri::State<'_, crate::state::AppState>) -> Result<Vec<String>, String> {
    let mut errs = state.startup_errors.write().await;
    let out = errs.clone();
    errs.clear();
    Ok(out)
}

/// Check if a port is available (not in use)
#[tauri::command]
pub async fn check_port_available(port: u16) -> Result<bool, String> {
    match std::net::TcpListener::bind(format!("127.0.0.1:{}", port)) {
        Ok(_) => Ok(true),
        Err(_) => Ok(false),
    }
}

/// Check if a directory exists
#[tauri::command]
pub async fn dir_exists(path: String) -> Result<bool, String> {
    Ok(std::path::Path::new(&path).is_dir())
}

/// Read last N lines of a log file without loading the whole file.
///
/// 硬字节上限：无论日志多大、单行多长，最多读末尾 `MAX_BYTES`，避免
/// 超长单行（无换行的 JSON dump 等）触发整文件加载。超限时在顶部追加
/// 一条"日志过大"提示，不静默截断。
fn read_tail(path: &std::path::Path, lines: usize) -> std::io::Result<String> {
    use std::io::{Read, Seek, SeekFrom};
    const CHUNK_SIZE: u64 = 8192;
    const MAX_BYTES: u64 = 512 * 1024; // 512 KB 末尾窗口
    let mut file = std::fs::File::open(path)?;
    let file_size = file.seek(SeekFrom::End(0))?;

    if file_size == 0 { return Ok(String::new()); }

    let mut buf = Vec::new();
    let mut newlines = 0;
    let mut pos = file_size;
    let mut truncated_by_cap = false;

    while pos > 0 && newlines <= lines {
        let read_size = CHUNK_SIZE.min(pos);
        pos -= read_size;
        file.seek(SeekFrom::Start(pos))?;
        let mut chunk = vec![0u8; read_size as usize];
        file.read_exact(&mut chunk)?;
        newlines += chunk.iter().filter(|&&b| b == b'\n').count();
        chunk.append(&mut buf);
        buf = chunk;
        if buf.len() as u64 >= MAX_BYTES {
            truncated_by_cap = true;
            break;
        }
    }

    let content = String::from_utf8_lossy(&buf).to_string();
    let all: Vec<&str> = content.lines().collect();
    let start = if all.len() > lines { all.len() - lines } else { 0 };
    let body = all[start..].join("\n");
    if truncated_by_cap {
        Ok(format!(
            "[... 日志过大，仅显示末尾 {} KB ...]\n{}",
            MAX_BYTES / 1024,
            body
        ))
    } else {
        Ok(body)
    }
}

/// Read a log file (last N lines) — efficient for large files
#[tauri::command]
pub async fn read_log_tail(path: String, lines: usize) -> Result<String, String> {
    read_tail(std::path::Path::new(&path), lines).map_err(|e| format!("无法读取日志: {}", e))
}

/// Find and read the latest log file for a service
#[tauri::command]
pub async fn find_and_read_log(service: String, state: tauri::State<'_, crate::state::AppState>) -> Result<String, String> {
    let config = state.config.read().await;
    let phpstudy = config.general.phpstudy_path.as_ref().ok_or("PHPStudy 路径未配置")?;
    let ext = phpstudy.join("Extensions");

    let log_path = match service.as_str() {
        "nginx" => {
            // Find Nginx dir and read error.log
            find_ext_dir(&ext, "Nginx").map(|d| d.join("logs").join("error.log"))
        }
        "mysql" => {
            // Find MySQL dir, then find the latest .err file in data/
            find_ext_dir(&ext, "MySQL").and_then(|d| {
                let data_dir = d.join("data");
                let mut err_files: Vec<_> = std::fs::read_dir(&data_dir).ok()?
                    .flatten()
                    .filter(|e| e.path().extension().and_then(|x| x.to_str()) == Some("err"))
                    .collect();
                err_files.sort_by_key(|e| std::cmp::Reverse(e.metadata().ok().and_then(|m| m.modified().ok())));
                err_files.first().map(|e| e.path())
            })
        }
        _ => None,
    };

    let path = log_path.ok_or(format!("{} 的日志文件未找到", service))?;
    read_tail(&path, 100).map_err(|e| format!("无法读取日志: {}", e))
}

fn find_ext_dir(ext_path: &std::path::Path, prefix: &str) -> Option<std::path::PathBuf> {
    std::fs::read_dir(ext_path).ok()?
        .flatten()
        .find(|e| e.file_name().to_string_lossy().starts_with(prefix) && e.path().is_dir())
        .map(|e| e.path())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn tmp_file(tag: &str, content: &[u8]) -> std::path::PathBuf {
        let dir = std::env::temp_dir();
        let path = dir.join(format!("ruststudy_tail_{}_{}.log", std::process::id(), tag));
        let mut f = std::fs::File::create(&path).unwrap();
        f.write_all(content).unwrap();
        path
    }

    #[test]
    fn empty_file_returns_empty() {
        let p = tmp_file("empty", b"");
        assert_eq!(read_tail(&p, 10).unwrap(), "");
        let _ = std::fs::remove_file(p);
    }

    #[test]
    fn tail_returns_last_n_lines() {
        let body: String = (1..=100).map(|i| format!("line{i}\n")).collect();
        let p = tmp_file("lastn", body.as_bytes());
        let out = read_tail(&p, 5).unwrap();
        assert_eq!(out, "line96\nline97\nline98\nline99\nline100");
        let _ = std::fs::remove_file(p);
    }

    #[test]
    fn huge_single_line_is_capped_not_oom() {
        let mut giant = vec![b'x'; 2 * 1024 * 1024];
        giant.push(b'\n');
        let p = tmp_file("huge", &giant);
        let out = read_tail(&p, 10).unwrap();
        assert!(out.starts_with("[... 日志过大"), "got: {}", &out[..60.min(out.len())]);
        assert!(out.len() < 600 * 1024, "output too large: {}", out.len());
        let _ = std::fs::remove_file(p);
    }
}
