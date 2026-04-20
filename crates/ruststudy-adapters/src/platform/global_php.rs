//! 全局 PHP CLI 版本切换 —— 用 .cmd 包装器 + HKCU PATH。
//!
//! 原理：
//! - 在 `%USERPROFILE%\.ruststudy\bin\` 维护几个 .cmd 文件（php.cmd、php-cgi.cmd、phpize.cmd），
//!   内容形如 `@"<绝对路径>\php.exe" %*`
//! - 切换"全局版本" = 重写这些 .cmd 文件指向新的 PHP 目录
//! - 首次设置时把 `~/.ruststudy/bin` 追加到用户 PATH（HKCU\Environment\Path）
//! - 广播 `WM_SETTINGCHANGE`，让新开的 cmd 窗口立即生效（旧窗口是 OS 限制，改不了）
//!
//! 为什么用 .cmd 不用 symlink/junction：
//! - Junction 不能跨卷，PHP 在 D:、用户 profile 在 C: → 无法创建
//! - Symlink 需要管理员或启用"开发者模式"，非所有用户可用
//! - .cmd 纯文本，无权限门槛，跨卷无关

use std::path::{Path, PathBuf};

/// 我们管理的 .cmd 文件列表。源文件名 → shim 文件名。
const SHIMS: &[(&str, &str)] = &[
    ("php.exe", "php.cmd"),
    ("php-cgi.exe", "php-cgi.cmd"),
    ("phpize.bat", "phpize.cmd"),
];

/// `%USERPROFILE%\.ruststudy\bin`
pub fn bin_dir() -> PathBuf {
    let home = std::env::var("USERPROFILE").unwrap_or_else(|_| "C:\\Users\\Default".into());
    PathBuf::from(home).join(".ruststudy").join("bin")
}

/// 为 php_install_path 下的可执行文件生成 .cmd 包装器到 bin_dir。
/// 只为**实际存在**的源文件生成 shim（php.exe 总有、phpize.bat 可能没有）。
pub fn write_shims(php_install_path: &Path) -> std::io::Result<()> {
    let bin = bin_dir();
    std::fs::create_dir_all(&bin)?;

    // 先清掉我们之前管理的 shim，避免残留（比如老版本有 phpize，新版本没）
    for (_, shim_name) in SHIMS {
        let _ = std::fs::remove_file(bin.join(shim_name));
    }

    for (source_name, shim_name) in SHIMS {
        let source = php_install_path.join(source_name);
        if !source.exists() {
            continue;
        }
        let shim_path = bin.join(shim_name);
        // @echo off 静默转发；"%*" 原样传递所有参数
        let content = format!(
            "@echo off\r\n\"{}\" %*\r\n",
            source.display()
        );
        std::fs::write(&shim_path, content)?;
    }
    Ok(())
}

/// 读当前 shim 指向的 PHP 目录（通过 parse php.cmd 内容回推）。
/// 找不到或格式不符返回 None。
pub fn read_active_php_dir() -> Option<PathBuf> {
    let content = std::fs::read_to_string(bin_dir().join("php.cmd")).ok()?;
    // 期望格式：`"C:\...\php855nts\php.exe" %*`
    let start = content.find('"')?;
    let rest = &content[start + 1..];
    let end = rest.find('"')?;
    let exe_path = PathBuf::from(&rest[..end]);
    exe_path.parent().map(|p| p.to_path_buf())
}

/// 确认 `~/.ruststudy/bin` 在用户 PATH（HKCU\Environment\Path）里。
/// 不存在就追加，返回 true；已存在返回 false。
/// 追加后广播 WM_SETTINGCHANGE，让新 shell 立即感知。
pub fn ensure_path_in_user_env() -> std::io::Result<bool> {
    let bin = bin_dir();
    let bin_str = bin.display().to_string();

    use winreg::enums::{HKEY_CURRENT_USER, KEY_READ, KEY_SET_VALUE};
    use winreg::RegKey;

    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let env_key = hkcu.open_subkey_with_flags("Environment", KEY_READ | KEY_SET_VALUE)?;

    let current: String = env_key.get_value("Path").unwrap_or_default();
    if path_contains(&current, &bin_str) {
        return Ok(false);
    }

    let new_path = if current.trim_end_matches(';').is_empty() {
        bin_str.clone()
    } else {
        format!("{};{}", current.trim_end_matches(';'), bin_str)
    };
    env_key.set_value("Path", &new_path)?;

    broadcast_env_change();
    Ok(true)
}

/// 查询 bin_dir 是否已经注册到用户 PATH
pub fn is_path_registered() -> bool {
    use winreg::enums::{HKEY_CURRENT_USER, KEY_READ};
    use winreg::RegKey;
    let bin = bin_dir().display().to_string();
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let current: String = hkcu
        .open_subkey_with_flags("Environment", KEY_READ)
        .ok()
        .and_then(|k| k.get_value::<String, _>("Path").ok())
        .unwrap_or_default();
    path_contains(&current, &bin)
}

/// 检测系统 PATH (HKLM) 里会屏蔽我们 shim 的 PHP 目录。
///
/// 背景：Windows 解析 PATH 时**系统 PATH 在前、用户 PATH 在后**。
/// PHPStudy 安装时往系统 PATH 塞的 PHP 目录（如 `D:\phpstudy_pro\Extensions\php\php84nts`）
/// 会先被匹配到，我们放在用户 PATH 里的 shim 永远轮不上。
///
/// 返回所有"系统 PATH 中含 php.exe"的目录（我们自己的 bin_dir 除外）。
pub fn detect_masking_paths() -> Vec<PathBuf> {
    use winreg::enums::{HKEY_LOCAL_MACHINE, KEY_READ};
    use winreg::RegKey;

    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    let env_key = match hklm.open_subkey_with_flags(
        r"SYSTEM\CurrentControlSet\Control\Session Manager\Environment",
        KEY_READ,
    ) {
        Ok(k) => k,
        Err(_) => return Vec::new(),
    };
    let system_path: String = env_key.get_value("Path").unwrap_or_default();
    let our_bin_norm = normalize_path(&bin_dir().display().to_string());

    let mut out = Vec::new();
    for entry in system_path.split(';') {
        let trimmed = entry.trim();
        if trimmed.is_empty() {
            continue;
        }
        // 展开 %SystemRoot% 等常见环境变量引用
        let expanded = expand_env_vars(trimmed);
        let norm = normalize_path(&expanded);
        if norm == our_bin_norm {
            continue;
        }
        let candidate = PathBuf::from(&expanded);
        if candidate.join("php.exe").exists() {
            out.push(candidate);
        }
    }
    out
}

/// 用 UAC 提权的方式从系统 PATH (HKLM) 里移除指定目录条目。
///
/// 策略：生成一段 PowerShell 脚本写到临时文件，再用 `Start-Process -Verb RunAs`
/// 触发 UAC。用户点"是" → 脚本以管理员身份跑一趟改 HKLM Path 然后广播 WM_SETTINGCHANGE。
/// 用户点"否" → PowerShell 会返回错误，我们捕获后转成友好信息。
pub fn fix_masking_paths(to_remove: &[PathBuf]) -> Result<(), String> {
    if to_remove.is_empty() {
        return Ok(());
    }

    // 构建 PowerShell 里的数组字面量。单引号字符串，内部单引号双写转义。
    let paths_ps = to_remove
        .iter()
        .map(|p| format!("'{}'", p.display().to_string().replace('\'', "''")))
        .collect::<Vec<_>>()
        .join(", ");

    // 主脚本：读 HKLM Path → 过滤掉待删条目（大小写不敏感 + 去尾反斜杠）→ 写回
    // 然后广播 WM_SETTINGCHANGE 让新 shell 感知
    let script = format!(
        r#"$ErrorActionPreference = 'Stop'
$current = [Environment]::GetEnvironmentVariable('Path', 'Machine')
$toRemove = @({paths_ps})
$normalize = {{ param($s) ($s.Trim() -replace '/', '\').TrimEnd('\').ToLower() }}
$removeSet = $toRemove | ForEach-Object {{ & $normalize $_ }}
$kept = @()
foreach ($entry in $current -split ';') {{
    $trimmed = $entry.Trim()
    if ($trimmed -eq '') {{ continue }}
    if ($removeSet -contains (& $normalize $trimmed)) {{ continue }}
    $kept += $trimmed
}}
$newPath = $kept -join ';'
[Environment]::SetEnvironmentVariable('Path', $newPath, 'Machine')
# 广播环境变量变化
Add-Type @"
using System;
using System.Runtime.InteropServices;
public static class Env {{
    [DllImport("user32.dll", CharSet=CharSet.Auto)]
    public static extern IntPtr SendMessageTimeout(IntPtr hWnd, uint Msg, UIntPtr wParam, string lParam, uint fuFlags, uint uTimeout, out UIntPtr lpdwResult);
}}
"@
[UIntPtr]$result = [UIntPtr]::Zero
[Env]::SendMessageTimeout([IntPtr]0xffff, 0x1A, [UIntPtr]::Zero, 'Environment', 2, 3000, [ref]$result) | Out-Null
"#
    );

    // 写到临时 .ps1
    let temp = std::env::temp_dir().join("ruststudy-path-fix.ps1");
    std::fs::write(&temp, script.as_bytes())
        .map_err(|e| format!("写脚本失败: {}", e))?;

    // 用 PowerShell 的 Start-Process -Verb RunAs 提权。-WindowStyle Hidden 减少视觉干扰。
    // 不等待其退出 —— UAC 弹窗本身就是阻塞用户操作，等不等都行；我们等会轮询 get_*
    let temp_quoted = temp.display().to_string().replace('\'', "''");
    let launcher = format!(
        "Start-Process powershell -Verb RunAs -Wait -WindowStyle Hidden -ArgumentList '-NoProfile','-ExecutionPolicy','Bypass','-File','{}'",
        temp_quoted
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
        .map_err(|e| format!("调用 PowerShell 失败: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        let combined = format!("{}{}", stdout.trim(), stderr.trim());
        // 用户点"否"或关掉 UAC 弹窗时 PowerShell 会报 "操作已取消"
        if combined.contains("取消")
            || combined.to_lowercase().contains("cancel")
            || combined.to_lowercase().contains("user declined")
        {
            return Err("已取消（未确认管理员权限）".into());
        }
        return Err(format!("修复失败: {}", combined));
    }
    Ok(())
}

/// 直接打开 Windows "系统属性 → 环境变量" 窗口，让用户手动编辑。
/// `rundll32 sysdm.cpl,EditEnvironmentVariables` 是系统自带的快捷入口，不需要管理员。
pub fn open_env_editor() -> Result<(), String> {
    std::process::Command::new("rundll32.exe")
        .args(["sysdm.cpl,EditEnvironmentVariables"])
        .spawn()
        .map_err(|e| format!("打开环境变量编辑器失败: {}", e))?;
    Ok(())
}

/// 简易 %VAR% 展开：只替换大小写不敏感的环境变量引用。
/// 找不到的变量保持原样，不报错。
fn expand_env_vars(s: &str) -> String {
    let mut result = String::new();
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            if let Some(end) = s[i + 1..].find('%') {
                let var_name = &s[i + 1..i + 1 + end];
                if !var_name.is_empty() {
                    if let Ok(v) = std::env::var(var_name) {
                        result.push_str(&v);
                        i += end + 2;
                        continue;
                    }
                }
            }
        }
        result.push(bytes[i] as char);
        i += 1;
    }
    result
}

fn normalize_path(s: &str) -> String {
    s.trim()
        .trim_end_matches('\\')
        .replace('/', "\\")
        .to_ascii_lowercase()
}

/// 用户 PATH 里是否已包含 target（不区分大小写 + 去空格 + 去结尾反斜杠）
fn path_contains(haystack: &str, target: &str) -> bool {
    let normalize = |s: &str| {
        s.trim()
            .trim_end_matches('\\')
            .replace('/', "\\")
            .to_ascii_lowercase()
    };
    let t = normalize(target);
    haystack.split(';').any(|p| normalize(p) == t)
}

/// 发送 WM_SETTINGCHANGE 广播，通知所有顶层窗口"环境变量变了"。
/// 新开的 cmd / explorer 会重新读 PATH。
fn broadcast_env_change() {
    // 用最小化的 windows API 调用；避免拉 windows-sys 依赖的话可以直接 FFI
    #[link(name = "user32")]
    unsafe extern "system" {
        fn SendMessageTimeoutW(
            hwnd: isize,
            msg: u32,
            wparam: usize,
            lparam: *const u16,
            flags: u32,
            timeout: u32,
            result: *mut usize,
        ) -> isize;
    }
    const HWND_BROADCAST: isize = 0xffff;
    const WM_SETTINGCHANGE: u32 = 0x001A;
    const SMTO_ABORTIFHUNG: u32 = 0x0002;

    // "Environment" as UTF-16
    let msg: Vec<u16> = "Environment\0".encode_utf16().collect();
    let mut result: usize = 0;
    unsafe {
        SendMessageTimeoutW(
            HWND_BROADCAST,
            WM_SETTINGCHANGE,
            0,
            msg.as_ptr(),
            SMTO_ABORTIFHUNG,
            3000,
            &mut result,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn path_contains_handles_variants() {
        assert!(path_contains(r"C:\a;C:\Users\x\.ruststudy\bin;C:\b", r"C:\Users\x\.ruststudy\bin"));
        assert!(path_contains(r"C:\Users\x\.ruststudy\bin\", r"C:\Users\x\.ruststudy\bin"));
        assert!(path_contains(r"C:\USERS\X\.RUSTSTUDY\BIN", r"c:\users\x\.ruststudy\bin"));
        assert!(path_contains(r"C:\Users\x/.ruststudy/bin", r"C:\Users\x\.ruststudy\bin"));
        assert!(!path_contains(r"C:\a;C:\b", r"C:\Users\x\.ruststudy\bin"));
        assert!(!path_contains("", r"C:\Users\x\.ruststudy\bin"));
    }

    #[test]
    fn expand_env_vars_replaces_known() {
        std::env::set_var("RS_TEST_VAR", "C:\\x");
        assert_eq!(expand_env_vars("%RS_TEST_VAR%\\bin"), "C:\\x\\bin");
        // 未知变量保持原样
        assert_eq!(expand_env_vars("%NO_SUCH_VAR%\\a"), "%NO_SUCH_VAR%\\a");
        // 多个变量
        std::env::set_var("RS_TEST_VAR2", "D:");
        assert_eq!(expand_env_vars("%RS_TEST_VAR2%\\foo\\%RS_TEST_VAR%"), "D:\\foo\\C:\\x");
    }
}
