use tauri::State;

use crate::commands::logger::push_log;
use crate::state::AppState;
use ruststudy_core::domain::log::LogLevel;
use ruststudy_core::error::RustStudyError;

const UTF8_BOM: &[u8] = &[0xEF, 0xBB, 0xBF];

fn read_hosts_bytes(path: &std::path::Path) -> Result<(String, bool), RustStudyError> {
    let bytes = std::fs::read(path)
        .map_err(|e| RustStudyError::from_io_with_context(e, "读取 hosts 文件失败"))?;
    let had_bom = bytes.starts_with(UTF8_BOM);
    let slice = if had_bom { &bytes[3..] } else { &bytes[..] };
    Ok((String::from_utf8_lossy(slice).into_owned(), had_bom))
}

fn write_hosts_text(path: &std::path::Path, text: &str) -> Result<(), RustStudyError> {
    let (_, had_bom) = read_hosts_bytes(path)?;

    let mut out: Vec<u8> = Vec::with_capacity(text.len() + 3);
    if had_bom {
        out.extend_from_slice(UTF8_BOM);
    }
    out.extend_from_slice(text.as_bytes());

    std::fs::write(path, out)
        .map_err(|e| RustStudyError::from_io_with_context(e, "写入 hosts 文件失败"))
}

fn normalize_hosts_text(input: &str) -> String {
    input.replace("\r\n", "\n")
}

fn map_hosts_error(e: RustStudyError) -> String {
    match e {
        RustStudyError::PermissionDenied(msg) => format!("PERMISSION_DENIED: {}", msg),
        RustStudyError::FileLocked(msg) => format!("FILE_LOCKED: {}", msg),
        other => other.to_string(),
    }
}

#[tauri::command]
pub async fn get_hosts_file_path(state: State<'_, AppState>) -> Result<String, String> {
    Ok(state.platform_ops.hosts_file_path().display().to_string())
}

#[tauri::command]
pub async fn get_hosts_text(state: State<'_, AppState>) -> Result<String, String> {
    let path = state.platform_ops.hosts_file_path();
    let (text, _) = read_hosts_bytes(&path).map_err(map_hosts_error)?;
    Ok(text)
}

#[tauri::command]
pub async fn save_hosts_text(text: String, state: State<'_, AppState>) -> Result<(), String> {
    let path = state.platform_ops.hosts_file_path();
    let normalized = normalize_hosts_text(&text);

    if let Err(e) = write_hosts_text(&path, &normalized) {
        let msg = map_hosts_error(e);
        push_log(
            &state,
            LogLevel::Error,
            "hosts",
            "保存 hosts 失败",
            Some(msg.clone()),
            None,
        )
        .await;
        return Err(msg);
    }

    push_log(
        &state,
        LogLevel::Success,
        "hosts",
        "保存 hosts 成功",
        Some(path.display().to_string()),
        None,
    )
    .await;
    Ok(())
}

#[tauri::command]
pub async fn save_hosts_text_elevated(text: String, state: State<'_, AppState>) -> Result<(), String> {
    let path = state.platform_ops.hosts_file_path();
    let normalized = normalize_hosts_text(&text);

    let temp_dir = std::env::temp_dir();
    let stamp = format!("{}-{}", std::process::id(), chrono::Local::now().timestamp_millis());
    let temp_path = temp_dir.join(format!("ruststudy-hosts-{}.txt", stamp));
    let script_path = temp_dir.join(format!("ruststudy-hosts-{}.ps1", stamp));

    std::fs::write(&temp_path, normalized.as_bytes())
        .map_err(|e| format!("写入临时文本失败: {}", e))?;

    let host_path_str = path.display().to_string().replace('"', "``\"");
    let src_path_str = temp_path.display().to_string().replace('"', "``\"");

    let script = format!(
        "$ErrorActionPreference = 'Stop'\n$src = \"{}\"\n$dst = \"{}\"\n$old = [IO.File]::ReadAllBytes($dst)\n$bom = $old.Length -ge 3 -and $old[0] -eq 239 -and $old[1] -eq 187 -and $old[2] -eq 191\n$txt = Get-Content -Raw -LiteralPath $src\nif ($bom) {{\n  [IO.File]::WriteAllBytes($dst, [Text.Encoding]::UTF8.GetPreamble() + [Text.Encoding]::UTF8.GetBytes($txt))\n}} else {{\n  [IO.File]::WriteAllText($dst, $txt, [Text.UTF8Encoding]::new($false))\n}}\n",
        src_path_str, host_path_str
    );

    std::fs::write(&script_path, script.as_bytes())
        .map_err(|e| format!("写入提权脚本失败: {}", e))?;

    let launcher = format!(
        "Start-Process powershell -Verb RunAs -Wait -WindowStyle Hidden -ArgumentList '-NoProfile','-ExecutionPolicy','Bypass','-File','{}'",
        script_path.display().to_string().replace('"', "``\"")
    );

    let output = std::process::Command::new("powershell")
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-ExecutionPolicy",
            "Bypass",
            "-Command",
            &launcher,
        ])
        .output()
        .map_err(|e| format!("调用提权保存失败: {}", e))?;

    let _ = std::fs::remove_file(&temp_path);
    let _ = std::fs::remove_file(&script_path);

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        let combined = format!("{}{}", stdout.trim(), stderr.trim());
        if combined.contains("取消")
            || combined.to_lowercase().contains("cancel")
            || combined.to_lowercase().contains("declined")
        {
            return Err("已取消（未确认管理员权限）".into());
        }
        return Err(format!("提权保存失败: {}", combined));
    }

    let (after, _) = read_hosts_bytes(&path).map_err(map_hosts_error)?;
    if after != normalized {
        return Err("提权命令执行完成，但 hosts 内容未变化，请检查杀软/系统策略是否拦截".into());
    }

    push_log(
        &state,
        LogLevel::Success,
        "hosts",
        "提权保存 hosts 成功",
        Some(path.display().to_string()),
        None,
    )
    .await;

    Ok(())
}
